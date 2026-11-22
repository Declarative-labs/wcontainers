//! A geometry shader as a compute program (plan section 7): one invocation per input primitive and
//! geometry instance, reading its input vertices from the vertex stage's records and writing the
//! vertices it emits, with the strip indices that join them, at slots of its own, so the output keeps
//! Direct3D's order without counting across invocations. The layout is `crate::stage`'s. Both routes
//! use it: each describes its program's stage values ([`GeometryValue`]) and its main function, which
//! calls the emit and cut functions this pass fills in.

use super::hull::PATCH_MAX_POINTS;
use super::{Lowering, Result, malformed, unsupported};
use crate::build::{self, Builder, Expr};
use crate::ir::{self, Basic, Ty, builtin, code, primitive, scalar};
use crate::stage;
use alloc::vec::Vec;
use naga::{
    AddressSpace, BinaryOperator, Binding, BuiltIn, FunctionArgument, Handle, Module, Scalar,
    ScalarKind, Span, StorageAccess,
};
use wcontainers_gpu::{StageExtras, SystemValue, VariantFlags, VariantKey};

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

impl Geometry {
    /// A geometry shader's declarations: its input and output primitives (`ir::primitive`), most
    /// vertices emitted, instances, and the mask of streams it writes.
    pub(crate) fn declared(
        input: u64,
        output: u64,
        max_vertices: u32,
        instances: u32,
        stream_mask: u64,
    ) -> Result<Geometry> {
        let mut g = Geometry {
            input,
            max_vertices,
            instances,
            ..Geometry::default()
        };
        g.output_declared(output, stream_mask)?;
        Ok(g)
    }

    fn output_declared(&mut self, output: u64, stream_mask: u64) -> Result<()> {
        self.output = output;
        self.streams = (u64::BITS - stream_mask.leading_zeros()).max(1);
        if self.streams as usize > stage::STREAMS {
            return Err(malformed(STREAM_PAST_DIRECT3D.into()));
        }
        // Direct3D takes several streams of points only.
        if self.streams > 1 && output != primitive::POINTS {
            return Err(unsupported(crate::refusal::GEOMETRY_STREAMS));
        }
        Ok(())
    }

    /// Streams written, at least one.
    pub(crate) fn streams(&self) -> u32 {
        self.streams.max(1)
    }
}

/// Which side of the stage a [`GeometryValue`] is on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Input,
    Output,
}

/// One stage value of a geometry shader: the private variable the program reads or writes, and
/// where Direct3D places it (a register and component, or a built-in of `ir::builtin`).
pub(crate) struct GeometryValue {
    pub var: Handle<naga::GlobalVariable>,
    pub side: Side,
    pub location: Option<(u32, u32)>,
    pub builtin: Option<u64>,
    pub stream: u32,
    /// Its type: a scalar or vector, or an array of them one register a row; an input's outermost
    /// count is its vertices.
    pub ty: Ty,
}

impl GeometryValue {
    /// Its counts within one vertex, innermost first.
    fn within(&self) -> &[u32] {
        let dims = &self.ty.dims[..];
        match self.side {
            Side::Input => dims.split_last().map_or(dims, |(_, rest)| rest),
            Side::Output => dims,
        }
    }

    /// The registers it takes.
    fn rows(&self) -> u32 {
        self.within().first().copied().unwrap_or(1)
    }

    /// The element of one register, or the refusal of an array of arrays.
    fn row(&self) -> Result<Basic> {
        match (self.within().len(), &self.ty.members[..]) {
            (0 | 1, &[member]) => Ok(member),
            _ => Err(unsupported(crate::refusal::OUTPUT_ARRAY)),
        }
    }

    /// The row of this value register `loc` is, if it is one of its registers.
    fn row_at(&self, loc: u32) -> Option<u32> {
        let (first, _) = self.location?;
        loc.checked_sub(first).filter(|&r| r < self.rows())
    }
}

/// A stage value the reflection takes, by its index among the [`GeometryValue`]s.
pub(crate) struct Reflected {
    pub value: usize,
    pub side: Side,
    pub location: Option<u32>,
    pub component: u32,
    pub member: Basic,
    pub modes: u64,
    pub system: Option<SystemValue>,
}

/// The pass that makes a geometry shader's emit and cut functions and its entry point.
pub(crate) struct GeometryPass<'a> {
    pub module: &'a mut Module,
    pub variant: &'a VariantKey,
    pub geometry: &'a mut Geometry,
    pub values: &'a [GeometryValue],
    /// The values the entry point reflects, in order.
    pub reflected: Vec<Reflected>,
}

/// The private and bound variables the emit functions and the entry point share.
#[derive(Clone, Copy)]
struct State {
    values: Handle<naga::GlobalVariable>,
    /// The vertex records: a storage buffer, or in the replay form a texture of one slot a texel.
    vertices: Handle<naga::GlobalVariable>,
    form: Form,
    /// This invocation's number in the draw, which picks its output slots.
    invocation: Handle<naga::GlobalVariable>,
}

/// What the compute form and the replay form (`VariantFlags::GEOMETRY_REPLAY`) have of their own.
#[derive(Clone, Copy)]
enum Form {
    Compute {
        /// The listed primitives after a tessellation stage (`stage::word::LISTED`).
        list: Handle<naga::GlobalVariable>,
        out: Handle<naga::GlobalVariable>,
        /// The tessellator's points (`stage::POINTS_BINDING`), whose fourth word is a listed
        /// primitive's number, where the shader reads that number.
        points: Option<Handle<naga::GlobalVariable>>,
    },
    Replay(Replay),
}

/// The replay form's private variables. Its invocation draws one corner of one output primitive slot:
/// the slot's primitive is the strip's vertices from the slot's number on, whole when no cut falls
/// inside it, and the corner keeps one of them.
#[derive(Clone, Copy)]
struct Replay {
    /// The record slots of the vertex kept.
    kept: Handle<naga::GlobalVariable>,
    slot: Handle<naga::GlobalVariable>,
    corner: Handle<naga::GlobalVariable>,
    /// The emitted vertex the corner keeps, known once the slot's first vertex is emitted.
    wanted: Handle<naga::GlobalVariable>,
    /// The first vertex of the strip being made, and of the strip holding the slot's first vertex.
    strip_start: Handle<naga::GlobalVariable>,
    slot_start: Handle<naga::GlobalVariable>,
    whole: Handle<naga::GlobalVariable>,
    /// A texel's coordinate in the records' texture.
    coordinate: Handle<naga::Type>,
}

impl State {
    fn list(&self) -> Handle<naga::GlobalVariable> {
        match self.form {
            Form::Compute { list, .. } => list,
            Form::Replay(_) => unreachable!("the replay form lists nothing"),
        }
    }

    fn out(&self) -> Handle<naga::GlobalVariable> {
        match self.form {
            Form::Compute { out, .. } => out,
            Form::Replay(_) => unreachable!("the replay form writes no output buffer"),
        }
    }
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
pub(crate) const EMIT: &str = "gs_emit";
/// The cut function's name.
pub(crate) const CUT: &str = "gs_cut_strip";
/// Why a geometry shader is malformed: it declares no vertex count.
const NO_VERTEX_COUNT: &str = "a geometry shader without its vertex count";
/// The compute form's output.
const OUT_VARIABLE: &str = "stage_geometry";
/// The invocation's number, and each stream's counters in `Counters`' order (a later stream's
/// names end with its number).
const INVOCATION_VARIABLE: &str = "gs_invocation";
const COUNTER_VARIABLES: [&str; 3] = ["gs_emitted", "gs_written", "gs_strip"];
/// The record slot holding the position.
const POSITION_SLOT: u32 = 0;
/// The replay form's outputs.
const REPLAY_OUTPUTS_TYPE: &str = "ReplayOutputs";
/// The replay form's output saying whether its slot's primitive is whole (1) or cut (0).
const REPLAY_WHOLE_MEMBER: &str = "primitive_whole";
/// The replay form's private variables, in `Replay`'s order.
const REPLAY_VARIABLES: [&str; 7] = [
    "gs_kept",
    "gs_slot",
    "gs_corner",
    "gs_wanted",
    "gs_strip_start",
    "gs_slot_start",
    "gs_whole",
];
/// The replay form's arguments.
const VERTEX_ARGUMENT: &str = "replay_vertex";
const INSTANCE_ARGUMENT: &str = "replay_instance";
pub(super) const INPUT_WITHOUT_TYPE: &str = "an input without a type";

/// A geometry shader's counts its entry points step by.
#[derive(Clone, Copy)]
struct Shape {
    /// Vertices per input primitive.
    n: u32,
    max: u32,
    gs_instances: u32,
    /// Slots per vertex record.
    in_slots: u32,
}
/// Why a geometry shader is malformed: its emit or cut functions were not made for its streams.
const NO_CUT: &str = "a geometry shader without an emit function per stream and a cut function";
/// A stream past `stage::STREAMS`, declared or emitted into.
pub(crate) const STREAM_PAST_DIRECT3D: &str = "a geometry stream past Direct3D's";

/// Vertices per input primitive.
fn input_vertices(input: u64) -> Result<u32> {
    Ok(match input {
        primitive::POINTS => 1,
        primitive::LINES => 2,
        primitive::LINES_ADJ => 4,
        primitive::TRIANGLES => 3,
        primitive::TRIANGLES_ADJ => 6,
        // A patch drawn without tessellation: its control points are the primitive's vertices.
        p if p > primitive::PATCH && p - primitive::PATCH <= u64::from(PATCH_MAX_POINTS) => {
            (p - primitive::PATCH) as u32
        }
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
            _ => g.output_declared(value, op.lits.get(1).copied().unwrap_or(1))?,
        }
        Ok(())
    }

    /// The stage values as the geometry pass takes them, with each one's declaring id.
    fn geometry_values(&self) -> Result<(Vec<GeometryValue>, Vec<u32>)> {
        let mut values = Vec::new();
        let mut ids = Vec::new();
        for (id, io) in &self.io {
            let side = match self.ir.op(*id)?.code {
                code::DCL_INPUT | code::DCL_INPUT_BUILT_IN => Side::Input,
                code::DCL_OUTPUT | code::DCL_OUTPUT_BUILT_IN => Side::Output,
                _ => continue,
            };
            values.push(GeometryValue {
                var: io.var,
                side,
                location: io.location,
                builtin: io.builtin,
                stream: io.stream,
                ty: io.ty.clone(),
            });
            ids.push(*id);
        }
        Ok((values, ids))
    }

    /// The emit and cut functions, ahead of the shader's own.
    pub(super) fn geometry_functions(&mut self) -> Result<()> {
        let (values, _) = self.geometry_values()?;
        let mut g = self.geometry.take().unwrap();
        let placeholders = |m: &mut Module, n: u32| -> Vec<Handle<naga::Function>> {
            (0..n)
                .map(|_| {
                    m.functions
                        .append(naga::Function::default(), Span::UNDEFINED)
                })
                .collect()
        };
        g.emit = placeholders(&mut self.module, g.streams());
        g.cut = placeholders(&mut self.module, 1).first().copied();
        let mut pass = GeometryPass {
            module: &mut self.module,
            variant: self.variant,
            geometry: &mut g,
            values: &values,
            reflected: Vec::new(),
        };
        let done = pass.functions();
        self.geometry = Some(g);
        done
    }

    /// The entry point, and the stage values' reflection.
    pub(super) fn geometry_entry_point(&mut self) -> Result<()> {
        let (values, ids) = self.geometry_values()?;
        let mut g = self.geometry.take().unwrap();
        let main = self.functions[&self.main];
        let mut pass = GeometryPass {
            module: &mut self.module,
            variant: self.variant,
            geometry: &mut g,
            values: &values,
            reflected: Vec::new(),
        };
        let done = pass.entry_point(main);
        let reflected = core::mem::take(&mut pass.reflected);
        self.geometry = Some(g);
        let extras = done?;
        for r in reflected {
            self.reflect_value(
                ids[r.value],
                r.side == Side::Input,
                r.location,
                r.component,
                r.member,
                r.modes,
                r.system,
            );
        }
        self.reflection.extras = extras;
        Ok(())
    }
}

impl GeometryPass<'_> {
    /// The values of `side` with a built-in.
    fn built_ins(&self, side: Side) -> impl Iterator<Item = &GeometryValue> {
        self.values
            .iter()
            .filter(move |v| v.side == side && v.builtin.is_some())
    }

    /// The sorted user locations of the inputs (`input`) or outputs.
    fn geometry_locations(&self, input: bool) -> Result<Vec<u32>> {
        let mut locations = Vec::new();
        for io in self.values {
            let is_input = io.side == Side::Input;
            if let (true, Some((first, _))) = (is_input == input, io.location) {
                for loc in first..first + io.rows() {
                    if !locations.contains(&loc) {
                        locations.push(loc);
                    }
                }
            }
        }
        locations.sort_unstable();
        Ok(locations)
    }

    /// The layer and viewport outputs' variables, if declared: each vertex record then ends with a
    /// slot holding them (`stage::SELECTION_SLOT`).
    fn selection(&self) -> Result<Option<[Option<Handle<naga::GlobalVariable>>; 2]>> {
        let mut found = [None, None];
        for io in self.built_ins(Side::Output) {
            if io.stream != 0 {
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

    /// The clip (`builtin::CLIP_DISTANCE`) or cull (`builtin::CULL_DISTANCE`) distance outputs'
    /// variables and how many distances each holds, in declaration order: each vertex record ends
    /// with the clip distances' slots, then the cull distances', four distances a slot
    /// ([`stage::distance_slots`]).
    fn distance_outputs(&self, which: u64) -> Result<Vec<(Handle<naga::GlobalVariable>, u32)>> {
        let mut found = Vec::new();
        for io in self.built_ins(Side::Output) {
            if io.builtin != Some(which) || io.stream != 0 {
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

    /// The slots after the output locations: the layer and viewport slot, then the clip distances',
    /// then the cull distances'.
    fn extra_slots(&self) -> Result<usize> {
        let mut slots = usize::from(self.selection()?.is_some());
        for which in [builtin::CLIP_DISTANCE, builtin::CULL_DISTANCE] {
            let distances: u32 = self.distance_outputs(which)?.iter().map(|c| c.1).sum();
            slots += stage::distance_slots(distances) as usize;
        }
        Ok(slots)
    }

    /// Vertices per output primitive.
    fn output_vertices(&self) -> u32 {
        match self.geometry.output {
            primitive::POINTS => 1,
            primitive::LINES => 2,
            _ => stage::TRIANGLE_VERTICES,
        }
    }

    fn strips(&self) -> bool {
        self.geometry.output != primitive::POINTS
    }

    /// Fills the emit function of each stream and the cut function (`Geometry::emit` and
    /// `Geometry::cut`, made empty by the caller ahead of the shader's own functions).
    pub(crate) fn functions(&mut self) -> Result<()> {
        let g = &*self.geometry;
        if g.max_vertices == 0 {
            return Err(malformed(NO_VERTEX_COUNT.into()));
        }
        input_vertices(g.input)?;
        let (max, streams, gsi) = (g.max_vertices, g.streams.max(1), g.instances.max(1));
        let replay = self.variant.flags.contains(VariantFlags::GEOMETRY_REPLAY);
        let outputs = self.geometry_locations(false)?;
        let record_slots = 1 + outputs.len() as u32 + self.extra_slots()? as u32;
        let (emit, cut) = (g.emit.clone(), g.cut.ok_or(malformed(NO_CUT.into()))?);
        if emit.len() != streams as usize {
            return Err(malformed(NO_CUT.into()));
        }
        let m = &mut *self.module;
        let u32_ty = build::scalar(m, Scalar::U32);
        let values = stage::values(m);
        let uvec4 = build::vec4(m, Scalar::U32);
        let vertices = if replay {
            let texture = m.types.insert(
                naga::Type {
                    name: None,
                    inner: naga::TypeInner::Image {
                        dim: naga::ImageDimension::D2,
                        arrayed: false,
                        class: naga::ImageClass::Sampled {
                            kind: ScalarKind::Uint,
                            multi: false,
                        },
                    },
                },
                Span::UNDEFINED,
            );
            build::global(
                m,
                stage::VERTICES_VARIABLE,
                AddressSpace::Handle,
                Some(stage::binding(stage::VERTICES_BINDING)),
                texture,
            )
        } else {
            let vertex_array = build::runtime_array(m, uvec4, stage::RECORD_SLOT_BYTES);
            build::global(
                m,
                stage::VERTICES_VARIABLE,
                AddressSpace::Storage {
                    access: StorageAccess::LOAD,
                },
                Some(stage::binding(stage::VERTICES_BINDING)),
                vertex_array,
            )
        };
        let private = |m: &mut naga::Module, name: &str, ty| {
            build::global(m, name, AddressSpace::Private, None, ty)
        };
        let form = if replay {
            let kept_ty = m.types.insert(
                naga::Type {
                    name: None,
                    inner: naga::TypeInner::Array {
                        base: uvec4,
                        size: naga::ArraySize::Constant(
                            core::num::NonZeroU32::new(record_slots).expect("a position slot"),
                        ),
                        stride: stage::RECORD_SLOT_BYTES,
                    },
                },
                Span::UNDEFINED,
            );
            let bool_ty = build::scalar(m, Scalar::BOOL);
            let coordinate = m.types.insert(
                naga::Type {
                    name: None,
                    inner: naga::TypeInner::Vector {
                        size: naga::VectorSize::Bi,
                        scalar: Scalar::U32,
                    },
                },
                Span::UNDEFINED,
            );
            let [kept, slot, corner, wanted, strip_start, slot_start, whole] = REPLAY_VARIABLES;
            Form::Replay(Replay {
                coordinate,
                kept: private(m, kept, kept_ty),
                slot: private(m, slot, u32_ty),
                corner: private(m, corner, u32_ty),
                wanted: private(m, wanted, u32_ty),
                strip_start: private(m, strip_start, u32_ty),
                slot_start: private(m, slot_start, u32_ty),
                whole: private(m, whole, bool_ty),
            })
        } else {
            let word_array = build::runtime_array(m, u32_ty, build::WORD);
            let list = build::global(
                m,
                stage::LIST_VARIABLE,
                AddressSpace::Storage {
                    access: StorageAccess::LOAD,
                },
                Some(stage::binding(stage::INDICES_BINDING)),
                word_array,
            );
            let out = build::global(
                m,
                OUT_VARIABLE,
                AddressSpace::Storage {
                    access: StorageAccess::LOAD | StorageAccess::STORE,
                },
                Some(stage::binding(stage::GEOMETRY_BINDING)),
                word_array,
            );
            let numbered = self
                .values
                .iter()
                .any(|io| io.side == Side::Input && io.builtin == Some(builtin::PRIMITIVE_ID));
            let points = numbered.then(|| {
                let point_array = build::runtime_array(m, uvec4, stage::RECORD_SLOT_BYTES);
                build::global(
                    m,
                    super::hull::POINTS_VARIABLE,
                    AddressSpace::Storage {
                        access: StorageAccess::LOAD,
                    },
                    Some(stage::binding(stage::POINTS_BINDING)),
                    point_array,
                )
            });
            Form::Compute { list, out, points }
        };
        let invocation = private(m, INVOCATION_VARIABLE, u32_ty);
        let counters: Vec<Counters> = (0..streams)
            .map(|s| {
                let [emitted, written, strip] = COUNTER_VARIABLES.map(|name| {
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
            form,
            invocation,
        };
        let strips = self.strips();
        let selection = self.selection()?;
        let clips = self.distance_outputs(builtin::CLIP_DISTANCE)?;
        let culls = self.distance_outputs(builtin::CULL_DISTANCE)?;
        // The replay form's slots hold what the pass-through draws, which has no cull distances.
        if replay && !culls.is_empty() {
            return Err(unsupported(crate::refusal::GEOMETRY_CULL_DISTANCE_REPLAY));
        }
        let record = stage::record_words(outputs.len() + self.extra_slots()?);
        let output_vertices = self.output_vertices();

        // emit: the stream's outputs' current values into its next slot, and its index into the strip;
        // in the replay form, into the kept slots when the vertex is the one the corner keeps.
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
            // Of several streams, the replay keeps the vertices of the one `word::STREAM` names.
            let keeps = match state.form {
                Form::Replay(r) => {
                    let replayed = (streams > 1).then(|| {
                        let values = b.global(state.values);
                        let named = stage::value(&mut b, values, stage::word::STREAM);
                        let this = b.u32(stream as u32);
                        b.binary(BinaryOperator::Equal, named, this)
                    });
                    Some(replay_emit(&mut b, r, n, output_vertices, replayed))
                }
                Form::Compute { .. } => None,
            };
            let place = match keeps {
                Some(_) => {
                    // The slots are made only for the vertex kept.
                    b.open();
                    None
                }
                None => {
                    let inv = stream_invocation(&mut b, state, stream as u32, gsi);
                    let maxe = b.u32(max);
                    let first = b.mul(inv, maxe);
                    let vertex = b.add(first, n);
                    let words = b.u32(record);
                    Some((vertex, b.mul(vertex, words)))
                }
            };
            let mut slots: Vec<(Expr, ScalarKind)> = Vec::new();
            let position = self
                .built_ins(Side::Output)
                .find(|io| io.builtin == Some(builtin::POSITION) && io.stream == stream as u32)
                .map(|io| (io.var, io.ty.basic().map(|m| m.scalar)));
            if let Some((pos_var, pos_scalar)) = position {
                let pos_scalar = Lowering::scalar(
                    pos_scalar.ok_or(unsupported(crate::refusal::BUILT_IN_ARRAY))?,
                )?;
                let p = b.global(pos_var);
                let p = b.load(p);
                slots.push((p, pos_scalar.kind));
            } else {
                // A shader feeding only stream output writes no position: the record carries (0, 0, 0,
                // -1), outside every clip volume, as a vertex program without one does.
                let vec4 = build::vec4(&mut *self.module, Scalar::F32);
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
                let uvec4 = build::vec4(&mut *self.module, Scalar::U32);
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
            let vec4 = build::vec4(&mut *self.module, Scalar::F32);
            for outputs in [&clips, &culls] {
                let mut distances = Vec::new();
                for &(var, count) in outputs {
                    if stream != 0 {
                        for _ in 0..count {
                            distances
                                .push(b.expr(naga::Expression::Literal(naga::Literal::F32(0.0))));
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
                for chunk in distances.chunks(build::VEC4 as usize) {
                    let mut comps = chunk.to_vec();
                    while comps.len() < build::VEC4 as usize {
                        comps.push(b.expr(naga::Expression::Literal(naga::Literal::F32(0.0))));
                    }
                    slots.push((b.compose(vec4, comps), ScalarKind::Float));
                }
            }
            let slots: Vec<Expr> = slots
                .into_iter()
                .map(|(v, kind)| match kind {
                    ScalarKind::Uint => v,
                    _ => b.bitcast(v, ScalarKind::Uint),
                })
                .collect();
            match (keeps, place, state.form) {
                (Some(keeps), _, Form::Replay(r)) => {
                    for (i, v) in slots.into_iter().enumerate() {
                        let kept = b.global(r.kept);
                        let at = b.field(kept, i as u32);
                        b.store(at, v);
                    }
                    let accept = b.close();
                    b.stmt(naga::Statement::If {
                        condition: keeps,
                        accept,
                        reject: naga::Block::new(),
                    });
                }
                (None, Some((vertex, base)), _) => {
                    for (i, v) in slots.into_iter().enumerate() {
                        for c in 0..build::VEC4 {
                            let offset = b.u32(i as u32 * build::VEC4 + c);
                            let word = b.add(base, offset);
                            let out_ptr = b.global(state.out());
                            let at = b.at(out_ptr, word);
                            let value = b.field(v, c);
                            b.store(at, value);
                        }
                    }
                    let at = index_slot(&mut b, state, c, max, stream as u32, gsi);
                    b.store(at, vertex);
                    increment(&mut b, written);
                    increment(&mut b, strip);
                }
                _ => unreachable!("a kept vertex in the replay form only"),
            }
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
            self.module.functions[emit[stream]] = b.finish();
        }

        // cut: ends the strip being made, if it has a vertex (strips have one stream).
        let mut b = Builder::new(CUT);
        let Counters {
            written,
            strip,
            emitted,
        } = counters[0];
        if let Form::Replay(r) = state.form {
            // The next vertex starts a strip.
            let n = b.global(emitted);
            let n = b.load(n);
            let p = b.global(r.strip_start);
            b.store(p, n);
        } else if strips {
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
        self.module.functions[cut] = b.finish();
        let g = &mut *self.geometry;
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
        let decls: Vec<usize> = (0..self.values.len())
            .filter(|&i| {
                let v = &self.values[i];
                v.side == Side::Output
                    && v.builtin.is_none()
                    && v.row_at(loc).is_some()
                    && v.stream == stream
            })
            .collect();
        let Some(&first) = decls.first() else {
            let zero = b.u32(0);
            let uvec4 = build::vec4(&mut *self.module, Scalar::U32);
            let comps = alloc::vec![zero; build::VEC4 as usize];
            return Ok((b.compose(uvec4, comps), ScalarKind::Uint));
        };
        let first = self.values[first].row()?;
        let s = Lowering::scalar(first.scalar)?;
        let s = if s == Scalar::BOOL { Scalar::U32 } else { s };
        let mut comps = Vec::new();
        for c in 0..build::VEC4 {
            let mut found = None;
            for &i in &decls {
                let values = self.values;
                let io = &values[i];
                let (_, comp) = io.location.unwrap();
                let m = io.row()?;
                if (comp..comp + m.size as u32).contains(&c) {
                    found = Some((io, comp, m));
                    break;
                }
            }
            let v = match found {
                Some((io, comp, m)) => {
                    let p = b.global(io.var);
                    let v = b.load(p);
                    let v = match (io.ty.dims.is_empty(), io.row_at(loc)) {
                        (false, Some(row)) => b.field(v, row),
                        _ => v,
                    };
                    let v = if m.size > 1 { b.field(v, c - comp) } else { v };
                    let from = Lowering::scalar(m.scalar)?;
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
        let ty = build::vec4(self.module, s);
        Ok((b.compose(ty, comps), s.kind))
    }

    /// The compute entry point: one invocation per input primitive and geometry instance (x) and API
    /// instance (y). It fills the inputs from the vertex records, runs the shader, and closes its
    /// slots: unused indices restart, unused point vertices lie outside the clip volume.
    /// The entry point: the compute form's or the replay form's, its outputs reflected as a vertex
    /// stage's for linking the pixel stage. Returns the stage's extras.
    pub(crate) fn entry_point(&mut self, main: Handle<naga::Function>) -> Result<StageExtras> {
        let g = &*self.geometry;
        let (input, max, gs_instances) = (g.input, g.max_vertices, g.instances.max(1));
        let state = g.state.unwrap();
        let counters = g.counters.clone();
        let streams = counters.len();
        let n = input_vertices(input)?;
        let inputs = self.geometry_locations(true)?;
        let in_slots = stage::record_words(inputs.len()) / build::VEC4;
        let shape = Shape {
            n,
            max,
            gs_instances,
            in_slots,
        };
        let (function, entry_stage, workgroup_size) = match state.form {
            Form::Compute { .. } => (
                self.geometry_compute_entry(state, &counters, shape, &inputs, main)?,
                naga::ShaderStage::Compute,
                [stage::WORKGROUP, 1, 1],
            ),
            Form::Replay(r) => (
                self.geometry_replay_entry(state, r, &counters, shape, &inputs, main)?,
                naga::ShaderStage::Vertex,
                Default::default(),
            ),
        };

        // Outputs as a vertex stage's, for linking the pixel stage.
        for i in 0..self.values.len() {
            let values = self.values;
            let io = &values[i];
            if io.side != Side::Output {
                continue;
            }
            // The layer, viewport and clip and cull distances are the drawn stream's alone.
            if io.stream != 0 && io.location.is_none() && io.builtin != Some(builtin::POSITION) {
                continue;
            }
            let distances = match io.builtin {
                Some(builtin::CLIP_DISTANCE) => Some(SystemValue::ClipDistance),
                Some(builtin::CULL_DISTANCE) => Some(SystemValue::CullDistance),
                _ => None,
            };
            if let Some(system) = distances {
                let member = *io
                    .ty
                    .members
                    .first()
                    .ok_or(unsupported(super::CLIP_DISTANCE_TYPE))?;
                self.reflect(i, Side::Output, None, 0, member, 0, Some(system));
                continue;
            }
            let member = io.row()?;
            if io.location.is_none() && !io.ty.dims.is_empty() {
                return Err(unsupported(crate::refusal::BUILT_IN_ARRAY));
            }
            match (io.location, io.builtin) {
                (Some((first, comp)), _) => {
                    for loc in first..first + io.rows() {
                        let modes = super::output_modes(&self.variant.output_interpolation, loc);
                        self.reflect(i, Side::Output, Some(loc), comp, member, modes, None);
                    }
                }
                (None, Some(builtin::POSITION)) => {
                    let system = Some(SystemValue::Position);
                    self.reflect(i, Side::Output, None, 0, member, 0, system)
                }
                (None, Some(which @ (builtin::LAYER_INDEX | builtin::VIEWPORT_INDEX))) => {
                    let system = match which {
                        builtin::LAYER_INDEX => SystemValue::RenderTargetArrayIndex,
                        _ => SystemValue::ViewportArrayIndex,
                    };
                    self.reflect(i, Side::Output, None, 0, member, 0, Some(system))
                }
                _ => return Err(unsupported(crate::refusal::GEOMETRY_OUTPUT)),
            }
        }
        let extras = StageExtras::Geometry {
            input_vertices: n as u8,
            max_output_vertices: max,
            output_vertices: self.output_vertices() as u8,
            instances: gs_instances,
            streams: streams as u8,
        };
        self.module.entry_points.push(naga::EntryPoint {
            name: crate::ENTRY_POINT.into(),
            stage: entry_stage,
            early_depth_test: None,
            workgroup_size,
            workgroup_size_overrides: None,
            function,
            mesh_info: None,
            task_payload: None,
            incoming_ray_payload: None,
        });
        Ok(extras)
    }

    #[allow(clippy::too_many_arguments)]
    fn reflect(
        &mut self,
        value: usize,
        side: Side,
        location: Option<u32>,
        component: u32,
        member: Basic,
        modes: u64,
        system: Option<SystemValue>,
    ) {
        self.reflected.push(Reflected {
            value,
            side,
            location,
            component,
            member,
            modes,
            system,
        });
    }

    /// The replay form's entry point: a vertex program whose vertex `v` draws corner `v % c` of output
    /// primitive slot `v / c % u` of geometry invocation `v / (c * u)`, `c` being the output
    /// primitive's vertices and `u` the slots of an invocation (`max - c + 1`), in the API instance
    /// its instance numbers. It writes the record slots of the vertex it keeps as flat values from
    /// location 0, with the position outside every clip volume where the slot's primitive is not
    /// whole, then whether it is whole as one flat word (for stream output). Primitive `p`'s input vertices are records `(instance * primitives + p) * n + k`: the
    /// vertex stage is captured in list order.
    fn geometry_replay_entry(
        &mut self,
        state: State,
        r: Replay,
        counters: &[Counters],
        shape: Shape,
        inputs: &[u32],
        main: Handle<naga::Function>,
    ) -> Result<naga::Function> {
        let Shape {
            n,
            max,
            gs_instances,
            in_slots,
        } = shape;
        let c = self.output_vertices();
        let units = stage::replay_slots(max, c);
        let u32_ty = build::scalar(&mut *self.module, Scalar::U32);
        let uvec4 = build::vec4(&mut *self.module, Scalar::U32);
        let vec4 = build::vec4(&mut *self.module, Scalar::F32);
        let kept_ty = self.module.global_variables[r.kept].ty;
        let slots = match self.module.types[kept_ty].inner {
            naga::TypeInner::Array {
                size: naga::ArraySize::Constant(count),
                ..
            } => count.get(),
            _ => unreachable!("the kept slots are an array"),
        };
        let mut b = Builder::new(crate::ENTRY_POINT);
        for (name, built) in [
            (VERTEX_ARGUMENT, BuiltIn::VertexIndex),
            (INSTANCE_ARGUMENT, BuiltIn::InstanceIndex),
        ] {
            b.f.arguments.push(FunctionArgument {
                name: Some(name.into()),
                ty: u32_ty,
                binding: Some(Binding::BuiltIn(built)),
            });
        }
        let v = b.argument(0);
        let api_instance = b.argument(1);
        let per = b.u32(units * c);
        let x = b.binary(BinaryOperator::Divide, v, per);
        let within = b.binary(BinaryOperator::Modulo, v, per);
        let corners = b.u32(c);
        let slot = b.binary(BinaryOperator::Divide, within, corners);
        let corner = b.binary(BinaryOperator::Modulo, within, corners);
        let gsi = b.u32(gs_instances);
        let prim = b.binary(BinaryOperator::Divide, x, gsi);
        let gi = b.binary(BinaryOperator::Modulo, x, gsi);
        let zero = b.u32(0);
        for c in counters {
            for var in [c.emitted, c.written, c.strip] {
                let p = b.global(var);
                b.store(p, zero);
            }
        }
        let never = b.u32(u32::MAX);
        let f = b.expr(naga::Expression::Literal(naga::Literal::Bool(false)));
        for (var, value) in [
            (r.slot, slot),
            (r.corner, corner),
            (r.wanted, never),
            (r.strip_start, zero),
            (r.slot_start, zero),
            (r.whole, f),
            (state.invocation, x),
        ] {
            let p = b.global(var);
            b.store(p, value);
        }
        let empty = b.expr(naga::Expression::ZeroValue(kept_ty));
        let p = b.global(r.kept);
        b.store(p, empty);
        let lit =
            |b: &mut Builder, x: f32| b.expr(naga::Expression::Literal(naga::Literal::F32(x)));
        let origin = lit(&mut b, 0.0);
        let below = lit(&mut b, stage::OUTSIDE_W);
        let outside = b.compose(vec4, alloc::vec![origin, origin, origin, below]);
        let outside = b.bitcast(outside, ScalarKind::Uint);
        let kept = b.global(r.kept);
        let position = b.field(kept, POSITION_SLOT);
        b.store(position, outside);

        let values = b.global(state.values);
        let primitives = stage::value(&mut b, values, stage::word::PRIMITIVES);
        let before = b.mul(api_instance, primitives);
        let primitive = b.add(before, prim);
        let nn = b.u32(n);
        let first = b.mul(primitive, nn);
        let mut records = Vec::new();
        for k in 0..n {
            let kk = b.u32(k);
            let record = b.add(first, kk);
            let per_record = b.u32(in_slots);
            records.push(b.mul(record, per_record));
        }
        self.geometry_inputs(&mut b, state, &records, (prim, gi), inputs)?;
        b.call(main, Vec::new(), false);
        let whole = b.global(r.whole);
        let whole = b.load(whole);
        let broken = b.expr(naga::Expression::Unary {
            op: naga::UnaryOperator::LogicalNot,
            expr: whole,
        });
        b.when(broken, |b| {
            let kept = b.global(r.kept);
            let position = b.field(kept, POSITION_SLOT);
            b.store(position, outside);
        });

        let mut members = Vec::new();
        let mut components = Vec::new();
        let kept = b.global(r.kept);
        let kept = b.load(kept);
        for i in 0..slots {
            members.push(naga::StructMember {
                name: Some(format!("slot{i}")),
                ty: uvec4,
                binding: Some(Binding::Location {
                    location: i,
                    interpolation: Some(naga::Interpolation::Flat),
                    sampling: None,
                    blend_src: None,
                    per_primitive: false,
                }),
                offset: stage::RECORD_SLOT_BYTES * i,
            });
            components.push(b.field(kept, i));
        }
        members.push(naga::StructMember {
            name: Some(REPLAY_WHOLE_MEMBER.into()),
            ty: u32_ty,
            binding: Some(Binding::Location {
                location: slots,
                interpolation: Some(naga::Interpolation::Flat),
                sampling: None,
                blend_src: None,
                per_primitive: false,
            }),
            offset: stage::RECORD_SLOT_BYTES * slots,
        });
        let one = b.u32(1);
        components.push(b.select(whole, one, zero));
        members.push(naga::StructMember {
            name: Some(stage::POSITION_MEMBER.into()),
            ty: vec4,
            binding: Some(Binding::BuiltIn(BuiltIn::Position { invariant: false })),
            offset: stage::RECORD_SLOT_BYTES * (slots + 1),
        });
        let position = b.field(kept, POSITION_SLOT);
        components.push(b.bitcast(position, ScalarKind::Float));
        let ty = self.module.types.insert(
            naga::Type {
                name: Some(REPLAY_OUTPUTS_TYPE.into()),
                inner: naga::TypeInner::Struct {
                    members,
                    span: stage::RECORD_SLOT_BYTES * (slots + 2),
                },
            },
            Span::UNDEFINED,
        );
        let out = b.compose(ty, components);
        b.ret(Some(out));
        let mut f = b.finish();
        f.result = Some(naga::FunctionResult { ty, binding: None });
        Ok(f)
    }

    /// The compute form's entry point.
    fn geometry_compute_entry(
        &mut self,
        state: State,
        counters: &[Counters],
        shape: Shape,
        inputs: &[u32],
        main: Handle<naga::Function>,
    ) -> Result<naga::Function> {
        let Shape {
            n,
            max,
            gs_instances,
            in_slots: in_words,
        } = shape;
        let strips = self.strips();
        let outputs = self.geometry_locations(false)?;
        let out_words = stage::record_words(outputs.len() + self.extra_slots()?);

        let uvec3 = self.module.types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Vector {
                    size: naga::VectorSize::Tri,
                    scalar: Scalar::U32,
                },
            },
            Span::UNDEFINED,
        );
        let mut b = Builder::new(crate::ENTRY_POINT);
        b.f.arguments.push(FunctionArgument {
            name: Some(stage::ID_ARGUMENT.into()),
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
        // Listed primitives, or an indexed strip made a list: the list's first word is its index
        // total, which only the GPU knows, so invocations past it (the dispatch covers the most there
        // can be) run no shader and close their slots.
        let listed = stage::value(&mut b, values, stage::word::LISTED);
        let zero = b.u32(0);
        let is_listed = b.binary(BinaryOperator::NotEqual, listed, zero);
        let listed_total = stage::value(&mut b, values, stage::word::LISTED_TOTAL);
        let total_only = b.binary(BinaryOperator::NotEqual, listed_total, zero);
        let capped = b.binary(BinaryOperator::LogicalOr, is_listed, total_only);
        let list_total = {
            let l = b.global(state.list());
            let first = b.u32(0);
            let p = b.at(l, first);
            let words = b.load(p);
            let nn = b.u32(n);
            b.binary(BinaryOperator::Divide, words, nn)
        };
        let unlisted = b.binary(BinaryOperator::GreaterEqual, prim, list_total);
        let idle = b.binary(BinaryOperator::LogicalAnd, capped, unlisted);
        let runs = b.expr(naga::Expression::Unary {
            op: naga::UnaryOperator::LogicalNot,
            expr: idle,
        });
        let gi = b.binary(BinaryOperator::Modulo, x, gsi);
        let before = b.mul(api_instance, total);
        let invocation = b.add(before, x);
        let p = b.global(state.invocation);
        b.store(p, invocation);
        for c in counters {
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
        let mut first_listed = None;
        for k in 0..n {
            let nn = b.u32(n);
            let list_first = b.mul(prim, nn);
            let kk = b.u32(k);
            let list = b.add(list_first, kk);
            let even_k = b.add(prim, kk);
            let along = if n == stage::TRIANGLE_VERTICES && k > 0 {
                let other = b.u32(stage::TRIANGLE_VERTICES - k);
                let odd_k = b.add(prim, other);
                b.select(is_odd, odd_k, even_k)
            } else {
                even_k
            };
            let v = b.select(is_strip, along, list);
            let r = b.add(instance_first, v);
            let header = b.u32(stage::tess::INDEX_HEADER);
            let at = b.add(header, list);
            let l = b.global(state.list());
            let p = b.at(l, at);
            let listed_r = b.load(p);
            first_listed.get_or_insert(listed_r);
            let r = b.select(is_listed, listed_r, r);
            let words = b.u32(in_words);
            records.push(b.mul(r, words));
        }
        // A listed primitive's number is its patch's, held by the point of its first vertex.
        let number = match (state.form, first_listed) {
            (
                Form::Compute {
                    points: Some(points),
                    ..
                },
                Some(first),
            ) => {
                let points = b.global(points);
                let p = b.at(points, first);
                let point = b.load(p);
                let patch = b.field(point, super::hull::POINT_PRIMITIVE);
                b.select(is_listed, patch, prim)
            }
            _ => prim,
        };

        self.geometry_inputs(&mut b, state, &records, (number, gi), inputs)?;

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
            b.each(&mut *self.module, from, to, |b, i| {
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
                let out = b.global(state.out());
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
            b.each(&mut *self.module, from, to, |b, i| {
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
                    let out = b.global(state.out());
                    let p = b.at(out, at);
                    let v = b.u32(value.to_bits());
                    b.store(p, v);
                }
            });
        }
        b.ret(None);
        Ok(b.finish())
    }

    /// Fills the inputs from the vertex records: `records` holds the slot number of each input
    /// vertex's record.
    fn geometry_inputs(
        &mut self,
        b: &mut Builder,
        state: State,
        records: &[Expr],
        (prim, gi): (Expr, Expr),
        inputs: &[u32],
    ) -> Result<()> {
        let n = records.len() as u32;
        for i in 0..self.values.len() {
            let values = self.values;
            let io = &values[i];
            if io.side != Side::Input {
                continue;
            }
            let (var, location, bi) = (io.var, io.location, io.builtin);
            let member = *io
                .ty
                .members
                .first()
                .ok_or_else(|| malformed(INPUT_WITHOUT_TYPE.into()))?;
            let to = Lowering::scalar(member.scalar)?;
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
                        self.reflect(i, Side::Input, None, 0, member, 0, system);
                    }
                    continue;
                }
                _ => return Err(unsupported(crate::refusal::GEOMETRY_INPUT)),
            };
            // A value per input vertex (a row per register where it is an array of rows), from its
            // record's slots.
            let rowed = io.within().len() == 1 && location.is_some();
            if io.ty.dims.last() != Some(&n)
                || io.within().len() > usize::from(rowed)
                || to == Scalar::BOOL
            {
                return Err(unsupported(crate::refusal::GEOMETRY_INPUT_SHAPE));
            }
            let (first, comp) = location.unwrap_or((0, 0));
            for row in 0..io.rows() {
                let slot = match location {
                    Some(_) => {
                        let i = inputs.iter().position(|&l| l == first + row).unwrap();
                        1 + i as u32
                    }
                    None => POSITION_SLOT,
                };
                for (k, &record) in records.iter().enumerate() {
                    let s = b.u32(slot);
                    let at = b.add(record, s);
                    let raw = record_slot(b, state, at);
                    let v = swizzle(b, raw, comp, member.size)?;
                    let v = if to == Scalar::U32 {
                        v
                    } else {
                        b.bitcast(v, to.kind)
                    };
                    let g = b.global(var);
                    let p = b.field(g, k as u32);
                    let p = if rowed { b.field(p, row) } else { p };
                    b.store(p, v);
                }
                let at = location.map(|_| first + row);
                self.reflect(i, Side::Input, at, comp, member, 0, system);
            }
        }
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

/// Slot `at` of the vertex records: an element of the storage buffer, or in the replay form the texel
/// at that place of the texture, read row after row.
fn record_slot(b: &mut Builder, state: State, at: Expr) -> Expr {
    let vertices = b.global(state.vertices);
    let r = match state.form {
        Form::Compute { .. } => {
            let p = b.at(vertices, at);
            return b.load(p);
        }
        Form::Replay(r) => r,
    };
    let level = b.u32(0);
    let size = b.expr(naga::Expression::ImageQuery {
        image: vertices,
        query: naga::ImageQuery::Size { level: Some(level) },
    });
    let width = b.field(size, 0);
    let x = b.binary(BinaryOperator::Modulo, at, width);
    let y = b.binary(BinaryOperator::Divide, at, width);
    let coordinate = b.expr(naga::Expression::Compose {
        ty: r.coordinate,
        components: alloc::vec![x, y],
    });
    b.expr(naga::Expression::ImageLoad {
        image: vertices,
        coordinate,
        array_index: None,
        sample: None,
        level: Some(level),
    })
}

/// The replay form's bookkeeping as vertex `n` of a stream is emitted, and whether it is the vertex
/// the corner keeps; with `replayed`, only where it is true (the stream is the one replayed). Odd
/// triangles of a strip take their last two vertices swapped, as the strip draws them.
fn replay_emit(
    b: &mut Builder,
    r: Replay,
    n: Expr,
    output_vertices: u32,
    replayed: Option<Expr>,
) -> Expr {
    let only = |b: &mut Builder, condition| match replayed {
        Some(replayed) => b.binary(BinaryOperator::LogicalAnd, condition, replayed),
        None => condition,
    };
    let slot = b.global(r.slot);
    let slot = b.load(slot);
    let first = b.binary(BinaryOperator::Equal, n, slot);
    let first = only(b, first);
    b.when(first, |b| {
        let start = b.global(r.strip_start);
        let start = b.load(start);
        let p = b.global(r.slot_start);
        b.store(p, start);
        let corner = b.global(r.corner);
        let corner = b.load(corner);
        let offset = if output_vertices == stage::TRIANGLE_VERTICES {
            let along = b.binary(BinaryOperator::Subtract, n, start);
            let one = b.u32(1);
            let parity = b.binary(BinaryOperator::And, along, one);
            let zero = b.u32(0);
            let odd = b.binary(BinaryOperator::NotEqual, parity, zero);
            let three = b.u32(stage::TRIANGLE_VERTICES);
            let back = b.binary(BinaryOperator::Subtract, three, corner);
            let swapped = b.binary(BinaryOperator::Modulo, back, three);
            b.select(odd, swapped, corner)
        } else {
            corner
        };
        let wanted = b.add(n, offset);
        let p = b.global(r.wanted);
        b.store(p, wanted);
    });
    let last = b.u32(output_vertices - 1);
    let last = b.add(slot, last);
    let closes = b.binary(BinaryOperator::Equal, n, last);
    let closes = only(b, closes);
    b.when(closes, |b| {
        let start = b.global(r.strip_start);
        let start = b.load(start);
        let slot_start = b.global(r.slot_start);
        let slot_start = b.load(slot_start);
        let whole = b.binary(BinaryOperator::Equal, start, slot_start);
        let p = b.global(r.whole);
        b.store(p, whole);
    });
    let wanted = b.global(r.wanted);
    let wanted = b.load(wanted);
    let keeps = b.binary(BinaryOperator::Equal, n, wanted);
    only(b, keeps)
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
    let out = b.global(state.out());
    b.at(out, at)
}

fn increment(b: &mut Builder, var: Handle<naga::GlobalVariable>) {
    let p = b.global(var);
    let v = b.load(p);
    let one = b.u32(1);
    let next = b.add(v, one);
    b.store(p, next);
}
