//! A geometry shader as a compute program (plan section 7): one invocation per input primitive and
//! geometry instance, reading its input vertices from the vertex stage's records and writing the
//! vertices it emits, with the strip indices that join them, at slots of its own, so the output keeps
//! Direct3D's order without counting across invocations. The layout is `crate::stage`'s.

use super::{Lowering, Result, malformed, unsupported};
use crate::build::{self, Builder, Expr};
use crate::ir::{self, Basic, builtin, code, primitive, scalar};
use crate::stage;
use alloc::vec::Vec;
use naga::{
    AddressSpace, BinaryOperator, Binding, BuiltIn, FunctionArgument, Handle, Scalar, ScalarKind,
    Span, StorageAccess,
};
use wcontainers_gpu::{StageExtras, SystemValue};

/// What a geometry shader declares, and the parts of its compute form.
#[derive(Default)]
pub(crate) struct Geometry {
    input: u64,
    output: u64,
    max_vertices: u32,
    instances: u32,
    /// Streams written: one past the highest the output declaration names.
    streams: u32,
    /// The emit function of each stream.
    pub emit: Vec<Handle<naga::Function>>,
    pub cut: Option<Handle<naga::Function>>,
    state: Option<State>,
    counters: Vec<Counters>,
}

/// The private and bound variables the emit functions and the entry point share.
#[derive(Clone, Copy)]
struct State {
    values: Handle<naga::GlobalVariable>,
    vertices: Handle<naga::GlobalVariable>,
    /// The listed primitives after a tessellation stage (`stage::word::LISTED`).
    list: Handle<naga::GlobalVariable>,
    out: Handle<naga::GlobalVariable>,
    /// This invocation's number in the draw, which picks its output slots.
    invocation: Handle<naga::GlobalVariable>,
}

/// One stream's counts in the invocation running. Each stream has its own vertex and index slots: a
/// stream's slots follow the slots of every invocation of the streams before it.
#[derive(Clone, Copy)]
struct Counters {
    /// Vertices emitted so far.
    emitted: Handle<naga::GlobalVariable>,
    /// Indices written so far.
    written: Handle<naga::GlobalVariable>,
    /// Vertices in the strip being made.
    strip: Handle<naga::GlobalVariable>,
}

/// The emit function's name; a later stream's ends with its number.
const EMIT: &str = "gs_emit";
/// A stream past `stage::STREAMS` in the output declaration.
const STREAM_PAST_DIRECT3D: &str = "a geometry stream past Direct3D's";

/// Vertices per input primitive.
fn input_vertices(input: u64) -> Result<u32> {
    Ok(match input {
        primitive::POINTS => 1,
        primitive::LINES => 2,
        primitive::LINES_ADJ => 4,
        primitive::TRIANGLES => 3,
        primitive::TRIANGLES_ADJ => 6,
        // Patches as a geometry shader's input (without tessellation): not built.
        p if p > primitive::PATCH => return Err(unsupported(crate::refusal::GEOMETRY_PATCH_INPUT)),
        _ => return Err(malformed("geometry input primitive".into())),
    })
}

impl Lowering<'_> {
    pub(super) fn geometry_declaration(&mut self, op: &ir::Op) -> Result<()> {
        let g = self
            .geometry
            .as_mut()
            .ok_or_else(|| malformed("a geometry declaration outside a geometry shader".into()))?;
        let value = op.lits.first().copied().unwrap_or(0);
        match op.code {
            code::SET_GS_INSTANCES => g.instances = value as u32,
            code::SET_GS_INPUT_PRIMITIVE => g.input = value,
            code::SET_GS_OUTPUT_VERTICES => g.max_vertices = value as u32,
            _ => {
                g.output = value;
                let mask = op.lits.get(1).copied().unwrap_or(1);
                g.streams = (u64::BITS - mask.leading_zeros()).max(1);
                if g.streams as usize > stage::STREAMS {
                    return Err(malformed(STREAM_PAST_DIRECT3D.into()));
                }
                // Direct3D takes several streams of points only.
                if g.streams > 1 && value != primitive::POINTS {
                    return Err(unsupported(crate::refusal::GEOMETRY_STREAMS));
                }
            }
        }
        Ok(())
    }

    /// The sorted user locations of the inputs (`input`) or outputs.
    fn geometry_locations(&self, input: bool) -> Result<Vec<u32>> {
        let mut locations = Vec::new();
        for (id, io) in &self.io {
            let is_input = matches!(
                self.ir.op(*id)?.code,
                code::DCL_INPUT | code::DCL_INPUT_BUILT_IN
            );
            if let (true, Some((loc, _))) = (is_input == input, io.location)
                && !locations.contains(&loc)
            {
                locations.push(loc);
            }
        }
        locations.sort_unstable();
        Ok(locations)
    }

    /// The layer and viewport outputs' variables, if declared: each vertex record then ends with a
    /// slot holding them (`stage::SELECTION_SLOT`).
    fn selection(&self) -> Result<Option<[Option<Handle<naga::GlobalVariable>>; 2]>> {
        let mut found = [None, None];
        for (id, io) in &self.io {
            if self.ir.op(*id)?.code != code::DCL_OUTPUT_BUILT_IN || io.stream != 0 {
                continue;
            }
            match io.builtin {
                Some(builtin::LAYER_INDEX) => found[0] = Some(io.var),
                Some(builtin::VIEWPORT_INDEX) => found[1] = Some(io.var),
                _ => {}
            }
        }
        Ok((found != [None, None]).then_some(found))
    }

    /// The clip distance outputs' variables and how many distances each holds, in declaration order:
    /// each vertex record ends with their slots, four distances a slot ([`stage::distance_slots`]).
    fn clip_outputs(&self) -> Result<Vec<(Handle<naga::GlobalVariable>, u32)>> {
        let mut found = Vec::new();
        for (id, io) in &self.io {
            if io.builtin != Some(builtin::CLIP_DISTANCE)
                || io.stream != 0
                || self.ir.op(*id)?.code != code::DCL_OUTPUT_BUILT_IN
            {
                continue;
            }
            let element = io.ty.members[..]
                == [Basic {
                    scalar: scalar::F32,
                    size: 1,
                }];
            if !element || io.ty.dims.len() > 1 {
                return Err(unsupported(super::CLIP_DISTANCE_TYPE));
            }
            found.push((io.var, io.ty.dims.iter().product()));
        }
        Ok(found)
    }

    /// The slots after the output locations: the layer and viewport slot, then the clip distances'.
    fn extra_slots(&self) -> Result<usize> {
        let distances: u32 = self.clip_outputs()?.iter().map(|c| c.1).sum();
        Ok(usize::from(self.selection()?.is_some()) + stage::distance_slots(distances) as usize)
    }

    fn strips(&self) -> bool {
        self.geometry
            .as_ref()
            .is_some_and(|g| g.output != primitive::POINTS)
    }

    /// The emit and cut functions, ahead of the shader's own.
    pub(super) fn geometry_functions(&mut self) -> Result<()> {
        let g = self.geometry.as_ref().unwrap();
        if g.max_vertices == 0 {
            return Err(malformed(
                "a geometry shader without its vertex count".into(),
            ));
        }
        input_vertices(g.input)?;
        let (max, streams, gsi) = (g.max_vertices, g.streams.max(1), g.instances.max(1));
        let m = &mut self.module;
        let u32_ty = build::scalar(m, Scalar::U32);
        let values = stage::values(m);
        let uvec4 = build::vec4(m, Scalar::U32);
        let vertex_array = build::runtime_array(m, uvec4, 16);
        let vertices = build::global(
            m,
            "stage_vertices",
            AddressSpace::Storage {
                access: StorageAccess::LOAD,
            },
            Some(stage::binding(stage::VERTICES_BINDING)),
            vertex_array,
        );
        let word_array = build::runtime_array(m, u32_ty, 4);
        let list = build::global(
            m,
            "stage_list",
            AddressSpace::Storage {
                access: StorageAccess::LOAD,
            },
            Some(stage::binding(stage::INDICES_BINDING)),
            word_array,
        );
        let out = build::global(
            m,
            "stage_geometry",
            AddressSpace::Storage {
                access: StorageAccess::LOAD | StorageAccess::STORE,
            },
            Some(stage::binding(stage::GEOMETRY_BINDING)),
            word_array,
        );
        let invocation = build::global(m, "gs_invocation", AddressSpace::Private, None, u32_ty);
        let counters: Vec<Counters> = (0..streams)
            .map(|s| {
                let [emitted, written, strip] =
                    ["gs_emitted", "gs_written", "gs_strip"].map(|name| {
                        let name = match s {
                            0 => name.into(),
                            s => format!("{name}{s}"),
                        };
                        build::global(m, &name, AddressSpace::Private, None, u32_ty)
                    });
                Counters {
                    emitted,
                    written,
                    strip,
                }
            })
            .collect();
        let state = State {
            values,
            vertices,
            list,
            out,
            invocation,
        };
        let outputs = self.geometry_locations(false)?;
        let strips = self.strips();
        let selection = self.selection()?;
        let clips = self.clip_outputs()?;
        let record = stage::record_words(outputs.len() + self.extra_slots()?);

        // emit: the stream's outputs' current values into its next slot, and its index into the strip.
        let mut emits = Vec::new();
        for (stream, &c) in counters.iter().enumerate() {
            let (emitted, written, strip) = (c.emitted, c.written, c.strip);
            let mut b = Builder::new(&match stream {
                0 => EMIT.into(),
                s => format!("{EMIT}{s}"),
            });
            let n = b.global(emitted);
            let n = b.load(n);
            let limit = b.u32(max);
            let room = b.binary(BinaryOperator::Less, n, limit);
            b.open();
            let inv = stream_invocation(&mut b, state, stream as u32, gsi);
            let maxe = b.u32(max);
            let first = b.mul(inv, maxe);
            let vertex = b.add(first, n);
            let words = b.u32(record);
            let base = b.mul(vertex, words);
            let mut slots: Vec<(Expr, ScalarKind)> = Vec::new();
            let position = self
                .io
                .iter()
                .find(|(id, io)| {
                    io.builtin == Some(builtin::POSITION)
                        && io.stream == stream as u32
                        && self
                            .ir
                            .op(*id)
                            .is_ok_and(|o| o.code == code::DCL_OUTPUT_BUILT_IN)
                })
                .map(|(_, io)| (io.var, io.ty.basic().map(|m| m.scalar)));
            if let Some((pos_var, pos_scalar)) = position {
                let pos_scalar =
                    Self::scalar(pos_scalar.ok_or(unsupported(crate::refusal::BUILT_IN_ARRAY))?)?;
                let p = b.global(pos_var);
                let p = b.load(p);
                slots.push((p, pos_scalar.kind));
            } else {
                // A shader feeding only stream output writes no position: the record carries (0, 0, 0,
                // -1), outside every clip volume, as a vertex program without one does.
                let vec4 = build::vec4(&mut self.module, Scalar::F32);
                let zero = b.expr(naga::Expression::Literal(naga::Literal::F32(0.0)));
                let below = b.expr(naga::Expression::Literal(naga::Literal::F32(-1.0)));
                let p = b.compose(vec4, alloc::vec![zero, zero, zero, below]);
                slots.push((p, ScalarKind::Float));
            }
            for &loc in &outputs {
                let v = self.location_vec4(&mut b, loc, stream as u32)?;
                slots.push(v);
            }
            // The layer, viewport and clip distances are the drawn stream's: zeros in the others'.
            if let Some(vars) = selection {
                let vars = if stream == 0 { vars } else { [None, None] };
                let uvec4 = build::vec4(&mut self.module, Scalar::U32);
                let mut comps = Vec::new();
                for var in vars {
                    comps.push(match var {
                        Some(var) => {
                            let p = b.global(var);
                            let v = b.load(p);
                            match self.module.global_variables[var].ty {
                                ty if self.module.types[ty].inner.scalar() == Some(Scalar::U32) => {
                                    v
                                }
                                _ => b.bitcast(v, ScalarKind::Uint),
                            }
                        }
                        None => b.u32(0),
                    });
                }
                let zero = b.u32(0);
                comps.extend([zero, zero]);
                slots.push((b.compose(uvec4, comps), ScalarKind::Uint));
            }
            let mut distances = Vec::new();
            for &(var, count) in &clips {
                if stream != 0 {
                    for _ in 0..count {
                        distances.push(b.expr(naga::Expression::Literal(naga::Literal::F32(0.0))));
                    }
                    continue;
                }
                let p = b.global(var);
                let v = b.load(p);
                match self.module.types[self.module.global_variables[var].ty].inner {
                    naga::TypeInner::Array { .. } => {
                        for i in 0..count {
                            distances.push(b.field(v, i));
                        }
                    }
                    _ => distances.push(v),
                }
            }
            let vec4 = build::vec4(&mut self.module, Scalar::F32);
            for chunk in distances.chunks(build::VEC4 as usize) {
                let mut comps = chunk.to_vec();
                while comps.len() < build::VEC4 as usize {
                    comps.push(b.expr(naga::Expression::Literal(naga::Literal::F32(0.0))));
                }
                slots.push((b.compose(vec4, comps), ScalarKind::Float));
            }
            for (i, (v, kind)) in slots.into_iter().enumerate() {
                let v = if kind == ScalarKind::Uint {
                    v
                } else {
                    b.bitcast(v, ScalarKind::Uint)
                };
                for c in 0..4u32 {
                    let offset = b.u32(i as u32 * 4 + c);
                    let word = b.add(base, offset);
                    let out_ptr = b.global(out);
                    let at = b.at(out_ptr, word);
                    let value = b.field(v, c);
                    b.store(at, value);
                }
            }
            let at = index_slot(&mut b, state, c, max, stream as u32, gsi);
            b.store(at, vertex);
            increment(&mut b, written);
            increment(&mut b, strip);
            let one = b.u32(1);
            let next = b.add(n, one);
            let p = b.global(emitted);
            b.store(p, next);
            let accept = b.close();
            b.stmt(naga::Statement::If {
                condition: room,
                accept,
                reject: naga::Block::new(),
            });
            emits.push(self.module.functions.append(b.finish(), Span::UNDEFINED));
        }

        // cut: ends the strip being made, if it has a vertex (strips have one stream).
        let mut b = Builder::new("gs_cut");
        let Counters { written, strip, .. } = counters[0];
        if strips {
            let s = b.global(strip);
            let s = b.load(s);
            let zero = b.u32(0);
            let open = b.binary(BinaryOperator::Greater, s, zero);
            b.when(open, |b| {
                let at = index_slot(b, state, counters[0], max, 0, gsi);
                let restart = b.u32(u32::MAX);
                b.store(at, restart);
                increment(b, written);
                let p = b.global(strip);
                let zero = b.u32(0);
                b.store(p, zero);
            });
        }
        let cut = self.module.functions.append(b.finish(), Span::UNDEFINED);
        let g = self.geometry.as_mut().unwrap();
        g.emit = emits;
        g.cut = Some(cut);
        g.state = Some(state);
        g.counters = counters;
        Ok(())
    }

    /// A location's output declarations' current values as one four-component value of their scalar
    /// type (zero where none is declared).
    fn location_vec4(
        &mut self,
        b: &mut Builder,
        loc: u32,
        stream: u32,
    ) -> Result<(Expr, ScalarKind)> {
        let decls: Vec<usize> = (0..self.io.len())
            .filter(|&i| {
                self.ir
                    .op(self.io[i].0)
                    .is_ok_and(|o| o.code == code::DCL_OUTPUT)
                    && self.io[i].1.location.is_some_and(|l| l.0 == loc)
                    && self.io[i].1.stream == stream
            })
            .collect();
        let Some(&first) = decls.first() else {
            let zero = b.u32(0);
            let uvec4 = build::vec4(&mut self.module, Scalar::U32);
            let comps = alloc::vec![zero; build::VEC4 as usize];
            return Ok((b.compose(uvec4, comps), ScalarKind::Uint));
        };
        let first = self.io[first]
            .1
            .ty
            .basic()
            .ok_or(unsupported(crate::refusal::OUTPUT_ARRAY))?;
        let s = Self::scalar(first.scalar)?;
        let s = if s == Scalar::BOOL { Scalar::U32 } else { s };
        let mut comps = Vec::new();
        for c in 0..4u32 {
            let found = decls.iter().find(|&&i| {
                let io = &self.io[i].1;
                let (_, comp) = io.location.unwrap();
                let size = io.ty.basic().map_or(1, |b| b.size as u32);
                (comp..comp + size).contains(&c)
            });
            let v = match found {
                Some(&i) => {
                    let io = &self.io[i].1;
                    let (_, comp) = io.location.unwrap();
                    let m = io
                        .ty
                        .basic()
                        .ok_or(unsupported(crate::refusal::OUTPUT_ARRAY))?;
                    let p = b.global(io.var);
                    let v = b.load(p);
                    let v = if m.size > 1 { b.field(v, c - comp) } else { v };
                    let from = Self::scalar(m.scalar)?;
                    if from.kind == s.kind {
                        v
                    } else if from == Scalar::BOOL {
                        let one = b.expr(naga::Expression::Literal(super::ones(s)));
                        let zero = b.expr(naga::Expression::Literal(super::zero(s)));
                        b.select(v, one, zero)
                    } else {
                        b.bitcast(v, s.kind)
                    }
                }
                None => b.expr(naga::Expression::Literal(super::zero(s))),
            };
            comps.push(v);
        }
        let ty = self.vec_ty(4, s)?;
        Ok((b.compose(ty, comps), s.kind))
    }

    /// The compute entry point: one invocation per input primitive and geometry instance (x) and API
    /// instance (y). It fills the inputs from the vertex records, runs the shader, and closes its
    /// slots: unused indices restart, unused point vertices lie outside the clip volume.
    pub(super) fn geometry_entry_point(&mut self) -> Result<()> {
        let g = self.geometry.as_ref().unwrap();
        let (input, output, max, gs_instances) =
            (g.input, g.output, g.max_vertices, g.instances.max(1));
        let state = g.state.unwrap();
        let counters = g.counters.clone();
        let streams = counters.len();
        let n = input_vertices(input)?;
        let strips = self.strips();
        let inputs = self.geometry_locations(true)?;
        let in_words = stage::record_words(inputs.len()) / 4;
        let outputs = self.geometry_locations(false)?;
        let out_words = stage::record_words(outputs.len() + self.extra_slots()?);

        let uvec3 = self.vec_ty(3, Scalar::U32)?;
        let mut b = Builder::new(crate::ENTRY_POINT);
        b.f.arguments.push(FunctionArgument {
            name: Some("id".into()),
            ty: uvec3,
            binding: Some(Binding::BuiltIn(BuiltIn::GlobalInvocationId)),
        });
        let id = b.argument(0);
        let x = b.field(id, 0);
        let api_instance = b.field(id, 1);
        let values = b.global(state.values);
        let primitives = stage::value(&mut b, values, stage::word::PRIMITIVES);
        let gsi = b.u32(gs_instances);
        let total = b.mul(primitives, gsi);
        let past = b.binary(BinaryOperator::GreaterEqual, x, total);
        b.when(past, |b| b.ret(None));
        let prim = b.binary(BinaryOperator::Divide, x, gsi);
        // Listed primitives: the list's first word is its index total, which only the GPU knows, so
        // invocations past it (the dispatch covers the most there can be) run no shader and close
        // their slots.
        let listed = stage::value(&mut b, values, stage::word::LISTED);
        let zero = b.u32(0);
        let is_listed = b.binary(BinaryOperator::NotEqual, listed, zero);
        let list_total = {
            let l = b.global(state.list);
            let first = b.u32(0);
            let p = b.at(l, first);
            let words = b.load(p);
            let nn = b.u32(n);
            b.binary(BinaryOperator::Divide, words, nn)
        };
        let unlisted = b.binary(BinaryOperator::GreaterEqual, prim, list_total);
        let idle = b.binary(BinaryOperator::LogicalAnd, is_listed, unlisted);
        let runs = b.expr(naga::Expression::Unary {
            op: naga::UnaryOperator::LogicalNot,
            expr: idle,
        });
        let gi = b.binary(BinaryOperator::Modulo, x, gsi);
        let before = b.mul(api_instance, total);
        let invocation = b.add(before, x);
        let p = b.global(state.invocation);
        b.store(p, invocation);
        for c in &counters {
            for var in [c.emitted, c.written, c.strip] {
                let p = b.global(var);
                let zero = b.u32(0);
                b.store(p, zero);
            }
        }

        // The input vertices' record numbers: lists take n vertices per primitive; strips step by
        // one, odd triangles taking their last two vertices swapped (i, i + 2, i + 1), which keeps the
        // winding and the first vertex.
        let strip = stage::value(&mut b, values, stage::word::STRIP);
        let zero = b.u32(0);
        let is_strip = b.binary(BinaryOperator::NotEqual, strip, zero);
        let count = stage::value(&mut b, values, stage::word::COUNT);
        let instance_first = b.mul(api_instance, count);
        let one = b.u32(1);
        let odd = b.binary(BinaryOperator::And, prim, one);
        let is_odd = b.binary(BinaryOperator::NotEqual, odd, zero);
        let mut records = Vec::new();
        for k in 0..n {
            let nn = b.u32(n);
            let list_first = b.mul(prim, nn);
            let kk = b.u32(k);
            let list = b.add(list_first, kk);
            let even_k = b.add(prim, kk);
            let along = if n == 3 && k > 0 {
                let other = b.u32(3 - k);
                let odd_k = b.add(prim, other);
                b.select(is_odd, odd_k, even_k)
            } else {
                even_k
            };
            let v = b.select(is_strip, along, list);
            let r = b.add(instance_first, v);
            let header = b.u32(stage::tess::INDEX_HEADER);
            let at = b.add(header, list);
            let l = b.global(state.list);
            let p = b.at(l, at);
            let listed_r = b.load(p);
            let r = b.select(is_listed, listed_r, r);
            let words = b.u32(in_words);
            records.push(b.mul(r, words));
        }

        for i in 0..self.io.len() {
            let (id, ref io) = self.io[i];
            if !matches!(
                self.ir.op(id)?.code,
                code::DCL_INPUT | code::DCL_INPUT_BUILT_IN
            ) {
                continue;
            }
            let (var, location, bi) = (io.var, io.location, io.builtin);
            let member = *io
                .ty
                .members
                .first()
                .ok_or_else(|| malformed("an input without a type".into()))?;
            let to = Self::scalar(member.scalar)?;
            let system = match (location, bi) {
                (Some(_), None) => None,
                (None, Some(builtin::POSITION)) => Some(SystemValue::Position),
                (
                    None,
                    Some(
                        which @ (builtin::PRIMITIVE_ID
                        | builtin::GS_INSTANCE_ID
                        | builtin::GS_VERTEX_COUNT_IN),
                    ),
                ) => {
                    if !io.ty.dims.is_empty() || member.size != 1 || to == Scalar::BOOL {
                        return Err(unsupported(crate::refusal::GEOMETRY_INPUT_SHAPE));
                    }
                    let v = match which {
                        builtin::PRIMITIVE_ID => prim,
                        builtin::GS_INSTANCE_ID => gi,
                        _ => b.u32(n),
                    };
                    let v = if to == Scalar::U32 {
                        v
                    } else {
                        b.bitcast(v, to.kind)
                    };
                    let p = b.global(var);
                    b.store(p, v);
                    if which == builtin::PRIMITIVE_ID {
                        let system = Some(SystemValue::PrimitiveId);
                        self.reflect_value(id, true, None, 0, member, 0, system);
                    }
                    continue;
                }
                _ => return Err(unsupported(crate::refusal::GEOMETRY_INPUT)),
            };
            // A value per input vertex, from its record's slot.
            if io.ty.dims[..] != [n] || to == Scalar::BOOL {
                return Err(unsupported(crate::refusal::GEOMETRY_INPUT_SHAPE));
            }
            let (slot, comp) = match location {
                Some((loc, comp)) => {
                    let i = inputs.iter().position(|&l| l == loc).unwrap();
                    (1 + i as u32, comp)
                }
                None => (0, 0),
            };
            for (k, &record) in records.iter().enumerate() {
                let s = b.u32(slot);
                let at = b.add(record, s);
                let vertices = b.global(state.vertices);
                let p = b.at(vertices, at);
                let raw = b.load(p);
                let v = swizzle(&mut b, raw, comp, member.size)?;
                let v = if to == Scalar::U32 {
                    v
                } else {
                    b.bitcast(v, to.kind)
                };
                let g = b.global(var);
                let p = b.field(g, k as u32);
                b.store(p, v);
            }
            self.reflect_value(id, true, location.map(|l| l.0), comp, member, 0, system);
        }

        let main = self.functions[&self.main];
        b.when(runs, |b| {
            b.call(main, Vec::new(), false);
        });
        // Unused index slots restart. Point output is drawn without indices (its slots serve stream
        // output): unused vertices lie outside the clip volume.
        for (stream, c) in counters.iter().enumerate() {
            let stream = stream as u32;
            let written = b.global(c.written);
            let from = b.load(written);
            let to = b.u32(2 * max);
            b.each(&mut self.module, from, to, |b, i| {
                let inv = match stream {
                    0 => invocation,
                    _ => stream_invocation(b, state, stream, gs_instances),
                };
                let maxe = b.u32(2 * max);
                let base = b.mul(inv, maxe);
                let values = b.global(state.values);
                let index_base = stage::value(b, values, stage::word::INDEX_BASE);
                let start = b.add(index_base, base);
                let at = b.add(start, i);
                let out = b.global(state.out);
                let p = b.at(out, at);
                let restart = b.u32(u32::MAX);
                b.store(p, restart);
            });
            if strips {
                continue;
            }
            let emitted = b.global(c.emitted);
            let from = b.load(emitted);
            let to = b.u32(max);
            b.each(&mut self.module, from, to, |b, i| {
                let inv = match stream {
                    0 => invocation,
                    _ => stream_invocation(b, state, stream, gs_instances),
                };
                let maxe = b.u32(max);
                let first = b.mul(inv, maxe);
                let vertex = b.add(first, i);
                let words = b.u32(out_words);
                let base = b.mul(vertex, words);
                for (c, value) in [2.0f32, 0.0, 0.0, 1.0].into_iter().enumerate() {
                    let offset = b.u32(c as u32);
                    let at = b.add(base, offset);
                    let out = b.global(state.out);
                    let p = b.at(out, at);
                    let v = b.u32(value.to_bits());
                    b.store(p, v);
                }
            });
        }
        b.ret(None);

        // Outputs as a vertex stage's, for linking the pixel stage.
        for i in 0..self.io.len() {
            let (id, ref io) = self.io[i];
            if self.ir.op(id)?.code != code::DCL_OUTPUT
                && self.ir.op(id)?.code != code::DCL_OUTPUT_BUILT_IN
            {
                continue;
            }
            // The layer, viewport and clip distances are the drawn stream's alone.
            if io.stream != 0 && io.location.is_none() && io.builtin != Some(builtin::POSITION) {
                continue;
            }
            if io.builtin == Some(builtin::CLIP_DISTANCE) {
                let member = *io
                    .ty
                    .members
                    .first()
                    .ok_or(unsupported(super::CLIP_DISTANCE_TYPE))?;
                let system = Some(SystemValue::ClipDistance);
                self.reflect_value(id, false, None, 0, member, 0, system);
                continue;
            }
            let member = io
                .ty
                .basic()
                .ok_or(unsupported(crate::refusal::OUTPUT_ARRAY))?;
            match (io.location, io.builtin) {
                (Some((loc, comp)), _) => {
                    let modes = super::output_modes(&self.variant.output_interpolation, loc);
                    self.reflect_value(id, false, Some(loc), comp, member, modes, None);
                }
                (None, Some(builtin::POSITION)) => {
                    self.reflect_value(id, false, None, 0, member, 0, Some(SystemValue::Position))
                }
                (None, Some(which @ (builtin::LAYER_INDEX | builtin::VIEWPORT_INDEX))) => {
                    let system = match which {
                        builtin::LAYER_INDEX => SystemValue::RenderTargetArrayIndex,
                        _ => SystemValue::ViewportArrayIndex,
                    };
                    self.reflect_value(id, false, None, 0, member, 0, Some(system))
                }
                _ => return Err(unsupported(crate::refusal::GEOMETRY_OUTPUT)),
            }
        }
        self.reflection.extras = StageExtras::Geometry {
            input_vertices: n as u8,
            max_output_vertices: max,
            output_vertices: match output {
                primitive::POINTS => 1,
                primitive::LINES => 2,
                _ => 3,
            },
            instances: gs_instances,
            streams: streams as u8,
        };
        self.module.entry_points.push(naga::EntryPoint {
            name: crate::ENTRY_POINT.into(),
            stage: naga::ShaderStage::Compute,
            early_depth_test: None,
            workgroup_size: [stage::WORKGROUP, 1, 1],
            workgroup_size_overrides: None,
            function: b.finish(),
            mesh_info: None,
            task_payload: None,
            incoming_ray_payload: None,
        });
        Ok(())
    }
}

/// Components `first..first + n` of a four-component value.
pub(super) fn swizzle(b: &mut Builder, v: Expr, first: u32, n: u8) -> Result<Expr> {
    if n == 1 {
        return Ok(b.field(v, first));
    }
    let mut pattern = [naga::SwizzleComponent::X; 4];
    for (i, p) in pattern.iter_mut().enumerate().take(n as usize) {
        *p = naga::SwizzleComponent::XYZW[(first as usize + i).min(3)];
    }
    Ok(b.expr(naga::Expression::Swizzle {
        size: super::vector_size(n)?,
        vector: v,
        pattern,
    }))
}

/// This invocation's number among the slots of `stream`: stream 0's slots come first, each later
/// stream's after every invocation's of the stream before.
fn stream_invocation(b: &mut Builder, state: State, stream: u32, gsi: u32) -> Expr {
    let inv = b.global(state.invocation);
    let inv = b.load(inv);
    if stream == 0 {
        return inv;
    }
    let values = b.global(state.values);
    let primitives = stage::value(b, values, stage::word::PRIMITIVES);
    let instances = stage::value(b, values, stage::word::INSTANCES);
    let per = b.u32(gsi * stream);
    let before = b.mul(primitives, instances);
    let before = b.mul(before, per);
    b.add(before, inv)
}

/// A pointer to the next index slot of this invocation in `stream`.
fn index_slot(b: &mut Builder, state: State, c: Counters, max: u32, stream: u32, gsi: u32) -> Expr {
    let inv = stream_invocation(b, state, stream, gsi);
    let slots = b.u32(2 * max);
    let first = b.mul(inv, slots);
    let values = b.global(state.values);
    let index_base = stage::value(b, values, stage::word::INDEX_BASE);
    let start = b.add(index_base, first);
    let w = b.global(c.written);
    let w = b.load(w);
    let at = b.add(start, w);
    let out = b.global(state.out);
    b.at(out, at)
}

fn increment(b: &mut Builder, var: Handle<naga::GlobalVariable>) {
    let p = b.global(var);
    let v = b.load(p);
    let one = b.u32(1);
    let next = b.add(v, one);
    b.store(p, next);
}
