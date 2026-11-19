//! Hull shaders as compute programs and domain shaders as vertex programs (plan section 7). A hull
//! invocation runs one patch: its control point function once per output control point, then its patch
//! constant function, and writes the patch's record in `crate::stage::tess`'s layout (and, for patch
//! 0, the header naming each location's slot). The domain program draws the tessellator's points: its
//! vertex number picks a point, which names its patch, whose record gives its inputs.

use super::{Lowering, Result, malformed, unsupported};
use crate::build::{self, Builder, Expr};
use crate::ir::{self, builtin, code, primitive};
use crate::stage::{self, tess};
use alloc::vec::Vec;
use naga::{
    AddressSpace, BinaryOperator, Binding, BuiltIn, FunctionArgument, Handle, ResourceBinding,
    Scalar, ScalarKind, Span, StorageAccess,
};
use wcontainers_gpu::{HELPER_SET, StageExtras, SystemValue};

/// What a hull or domain shader declares.
#[derive(Default)]
pub(crate) struct Tessellation {
    /// `StageExtras::Hull`'s numbering.
    pub domain: u8,
    pub partitioning: u8,
    pub output: u8,
    pub points_in: u32,
    pub points_out: u32,
    /// The patch constant function, by id.
    pub constants: u32,
}

/// The output locations of a hull shader: control point data (arrays) and patch constants.
/// The most control points a patch has in Direct3D 11 and 12 (`D3D11_IA_PATCH_MAX_CONTROL_POINT_COUNT`).
pub(crate) const PATCH_MAX_POINTS: u32 = 32;

fn sorted(mut v: Vec<u32>) -> Vec<u32> {
    v.sort_unstable();
    v.dedup();
    v
}

impl Lowering<'_> {
    pub(super) fn tessellation_declaration(&mut self, op: &ir::Op) -> Result<()> {
        let t = self.tessellation.as_mut().ok_or_else(|| {
            malformed("a tessellation declaration outside hull and domain".into())
        })?;
        let lit = |i: usize| op.lits.get(i).copied().unwrap_or(0);
        match op.code {
            code::SET_TESS_DOMAIN => {
                t.domain = match lit(0) {
                    primitive::TRIANGLES => 0,
                    primitive::QUADS => 1,
                    primitive::LINES => 2,
                    _ => return Err(malformed("tessellation domain".into())),
                }
            }
            code::SET_TESS_PRIMITIVE => {
                t.output = match (lit(0), lit(1)) {
                    (primitive::POINTS, _) => 0,
                    (primitive::LINES, _) => 1,
                    (primitive::TRIANGLES, 1) => 2,
                    (primitive::TRIANGLES, _) => 3,
                    _ => return Err(malformed("tessellator output".into())),
                };
                t.partitioning = match lit(2) {
                    0 => 0,
                    1 => 2,
                    2 => 3,
                    _ => return Err(malformed("tessellator partitioning".into())),
                };
            }
            _ => {
                t.points_in = lit(0) as u32;
                t.points_out = lit(1) as u32;
            }
        }
        Ok(())
    }

    fn is_input(&self, id: u32) -> bool {
        self.ir
            .op(id)
            .is_ok_and(|o| matches!(o.code, code::DCL_INPUT | code::DCL_INPUT_BUILT_IN))
    }

    /// The four words of slot `loc` from the declarations at that location (indices into `self.io`),
    /// of element `element` where they are arrays; zero where none is declared.
    fn slot_words(
        &mut self,
        b: &mut Builder,
        decls: &[usize],
        element: Option<u32>,
    ) -> Result<[Expr; 4]> {
        let mut words = Vec::new();
        for c in 0..4u32 {
            let found = decls.iter().find(|&&i| {
                let io = &self.io[i].1;
                let (_, comp) = io.location.unwrap();
                let size = io.ty.members.first().map_or(1, |m| m.size as u32);
                (comp..comp + size).contains(&c)
            });
            let w = match found {
                Some(&i) => {
                    let io = &self.io[i].1;
                    let (_, comp) = io.location.unwrap();
                    let m = *io
                        .ty
                        .members
                        .first()
                        .ok_or_else(|| malformed("an output without a type".into()))?;
                    let p = b.global(io.var);
                    let p = match element {
                        Some(k) => b.field(p, k),
                        None => p,
                    };
                    let v = b.load(p);
                    let v = if m.size > 1 { b.field(v, c - comp) } else { v };
                    let from = Self::scalar(m.scalar)?;
                    if from == Scalar::BOOL {
                        let one = b.u32(1);
                        let zero = b.u32(0);
                        b.select(v, one, zero)
                    } else if from.kind == ScalarKind::Uint {
                        v
                    } else {
                        b.bitcast(v, ScalarKind::Uint)
                    }
                }
                None => b.u32(0),
            };
            words.push(w);
        }
        Ok([words[0], words[1], words[2], words[3]])
    }

    /// The compute entry point of a hull shader: one invocation per patch (x) and API instance (y).
    pub(super) fn hull_entry_point(&mut self) -> Result<()> {
        let t = self.tessellation.as_ref().unwrap();
        let (points_in, points_out) = (t.points_in, t.points_out);
        if points_out == 0 {
            // A patch handing the domain shader no control points: not built.
            return Err(unsupported("HullWithoutControlPoints"));
        }
        if points_in == 0 || points_in > PATCH_MAX_POINTS || points_out > PATCH_MAX_POINTS {
            return Err(malformed("a hull shader's control point counts".into()));
        }
        // Inputs: the vertex records' slots, in order of location.
        let mut inputs = Vec::new();
        let mut point_outputs = Vec::new();
        let mut constant_outputs = Vec::new();
        for (id, io) in &self.io {
            let input = self.is_input(*id);
            match (input, io.location, io.ty.dims.is_empty()) {
                (true, Some((loc, _)), _) => inputs.push(loc),
                (false, Some((loc, _)), false) => point_outputs.push(loc),
                (false, Some((loc, _)), true) => constant_outputs.push(loc),
                (false, None, false) if io.builtin == Some(builtin::POSITION) => {
                    point_outputs.push(tess::LOCATIONS)
                }
                _ => {}
            }
        }
        let (inputs, point_outputs, constant_outputs) = (
            sorted(inputs),
            sorted(point_outputs),
            sorted(constant_outputs),
        );
        if point_outputs
            .iter()
            .filter(|&&l| l != tess::LOCATIONS)
            .chain(&constant_outputs)
            .any(|&l| l >= tess::LOCATIONS)
        {
            return Err(unsupported("HullOutputLocation"));
        }
        let in_words = stage::record_words(inputs.len()) / 4;
        let point_words = 4 * point_outputs.len() as u32;
        let constant_base = points_out * point_words;
        let factor_base = constant_base + 4 * constant_outputs.len() as u32;
        let patch_words = factor_base + 8;

        let m = &mut self.module;
        let values = stage::values(m);
        let uvec4 = build::vec4(m, Scalar::U32);
        let u32_ty = build::scalar(m, Scalar::U32);
        let records = build::runtime_array(m, uvec4, 16);
        let vertices = build::global(
            m,
            "stage_vertices",
            AddressSpace::Storage {
                access: StorageAccess::LOAD,
            },
            Some(stage::binding(stage::VERTICES_BINDING)),
            records,
        );
        let words = build::runtime_array(m, u32_ty, 4);
        let out = build::global(
            m,
            "stage_patches",
            AddressSpace::Storage {
                access: StorageAccess::LOAD | StorageAccess::STORE,
            },
            Some(stage::binding(stage::PATCHES_BINDING)),
            words,
        );
        let uvec3 = self.vec_ty(3, Scalar::U32)?;
        let mut b = Builder::new("main");
        b.f.arguments.push(FunctionArgument {
            name: Some("id".into()),
            ty: uvec3,
            binding: Some(Binding::BuiltIn(BuiltIn::GlobalInvocationId)),
        });
        let id = b.argument(0);
        let x = b.field(id, 0);
        let instance = b.field(id, 1);
        let v = b.global(values);
        let primitives = stage::value(&mut b, v, stage::word::PRIMITIVES);
        let past = b.binary(BinaryOperator::GreaterEqual, x, primitives);
        b.when(past, |b| b.ret(None));
        let before = b.mul(instance, primitives);
        let patch = b.add(before, x);
        let count = stage::value(&mut b, v, stage::word::COUNT);
        let instance_first = b.mul(instance, count);
        let n_in = stage::value(&mut b, v, stage::word::PATCH_POINTS);
        let first_point = b.mul(x, n_in);
        let first_record = b.add(instance_first, first_point);

        let mut point_id = None;
        for i in 0..self.io.len() {
            let (id, ref io) = self.io[i];
            if !self.is_input(id) {
                continue;
            }
            let (var, location, bi) = (io.var, io.location, io.builtin);
            let member = *io
                .ty
                .members
                .first()
                .ok_or_else(|| malformed("an input without a type".into()))?;
            let to = Self::scalar(member.scalar)?;
            match (location, bi) {
                (Some((loc, comp)), _) => {
                    if io.ty.dims.len() != 1 || io.ty.dims[0] < points_in || to == Scalar::BOOL {
                        return Err(unsupported("HullInputShape"));
                    }
                    let slot = 1 + inputs.iter().position(|&l| l == loc).unwrap() as u32;
                    for k in 0..points_in {
                        let kk = b.u32(k);
                        let r = b.add(first_record, kk);
                        let w = b.u32(in_words);
                        let r = b.mul(r, w);
                        let s = b.u32(slot);
                        let at = b.add(r, s);
                        let p = b.global(vertices);
                        let p = b.at(p, at);
                        let raw = b.load(p);
                        let value = super::geometry::swizzle(&mut b, raw, comp, member.size)?;
                        let value = if to == Scalar::U32 {
                            value
                        } else {
                            b.bitcast(value, to.kind)
                        };
                        let g = b.global(var);
                        let p = b.field(g, k);
                        b.store(p, value);
                    }
                    self.reflect_value(id, true, Some(loc), comp, member, 0, None);
                }
                (None, Some(builtin::POSITION)) => {
                    if io.ty.dims.len() != 1 || io.ty.dims[0] < points_in || member.size != 4 {
                        return Err(unsupported("HullInputShape"));
                    }
                    for k in 0..points_in {
                        let kk = b.u32(k);
                        let r = b.add(first_record, kk);
                        let w = b.u32(in_words);
                        let at = b.mul(r, w);
                        let p = b.global(vertices);
                        let p = b.at(p, at);
                        let raw = b.load(p);
                        let value = b.bitcast(raw, ScalarKind::Float);
                        let g = b.global(var);
                        let p = b.field(g, k);
                        b.store(p, value);
                    }
                    let system = Some(SystemValue::Position);
                    self.reflect_value(id, true, None, 0, member, 0, system);
                }
                (None, Some(which)) => {
                    if !io.ty.dims.is_empty() || member.size != 1 || to == Scalar::BOOL {
                        return Err(unsupported("HullInputShape"));
                    }
                    let value = match which {
                        builtin::PRIMITIVE_ID => x,
                        builtin::TESS_CONTROL_POINT_COUNT_IN => b.u32(points_in),
                        builtin::TESS_CONTROL_POINT_ID => {
                            point_id = Some((var, to));
                            continue;
                        }
                        _ => return Err(unsupported("HullInput")),
                    };
                    let value = if to == Scalar::U32 {
                        value
                    } else {
                        b.bitcast(value, to.kind)
                    };
                    let p = b.global(var);
                    b.store(p, value);
                    if which == builtin::PRIMITIVE_ID {
                        let system = Some(SystemValue::PrimitiveId);
                        self.reflect_value(id, true, None, 0, member, 0, system);
                    }
                }
                _ => return Err(unsupported("HullInput")),
            }
        }

        // The control point function per output control point, then the patch constant function.
        let main = self.functions[&self.main];
        let constants = self.tessellation.as_ref().unwrap().constants;
        let constants = *self
            .functions
            .get(&constants)
            .ok_or_else(|| malformed("a hull shader without its patch constant function".into()))?;
        let zero = b.u32(0);
        let end = b.u32(points_out);
        b.each(&mut self.module, zero, end, |b, k| {
            if let Some((var, to)) = point_id {
                let value = if to == Scalar::U32 {
                    k
                } else {
                    b.bitcast(k, to.kind)
                };
                let p = b.global(var);
                b.store(p, value);
            }
            b.call(main, Vec::new(), false);
        });
        b.call(constants, Vec::new(), false);

        // The record.
        let header = b.u32(tess::HEADER_WORDS);
        let pw = b.u32(patch_words);
        let along = b.mul(patch, pw);
        let base = b.add(header, along);
        let store = |b: &mut Builder, at: u32, value: Expr| {
            let o = b.u32(at);
            let at = b.add(base, o);
            let p = b.global(out);
            let p = b.at(p, at);
            b.store(p, value);
        };
        let output_decls = |l: &Self, loc: u32, array: bool| -> Vec<usize> {
            (0..l.io.len())
                .filter(|&i| {
                    let io = &l.io[i].1;
                    !l.is_input(l.io[i].0)
                        && io.location.is_some_and(|x| x.0 == loc)
                        && io.ty.dims.is_empty() != array
                })
                .collect()
        };
        let position = self.io.iter().find_map(|(id, io)| {
            (!self.is_input(*id) && io.builtin == Some(builtin::POSITION)).then_some(io.var)
        });
        for (slot, &loc) in point_outputs.iter().enumerate() {
            let decls = output_decls(self, loc, true);
            for k in 0..points_out {
                let words = match (loc == tess::LOCATIONS, position) {
                    (true, Some(var)) => {
                        let p = b.global(var);
                        let p = b.field(p, k);
                        let v = b.load(p);
                        let v = b.bitcast(v, ScalarKind::Uint);
                        [0, 1, 2, 3].map(|c| b.field(v, c))
                    }
                    _ => self.slot_words(&mut b, &decls, Some(k))?,
                };
                for (c, w) in words.into_iter().enumerate() {
                    store(&mut b, k * point_words + slot as u32 * 4 + c as u32, w);
                }
            }
        }
        for (slot, &loc) in constant_outputs.iter().enumerate() {
            let decls = output_decls(self, loc, false);
            let words = self.slot_words(&mut b, &decls, None)?;
            for (c, w) in words.into_iter().enumerate() {
                store(&mut b, constant_base + slot as u32 * 4 + c as u32, w);
            }
        }
        for i in 0..self.io.len() {
            let (id, ref io) = self.io[i];
            if self.is_input(id) || io.location.is_some() || io.builtin == Some(builtin::POSITION) {
                continue;
            }
            let (first, count) = match io.builtin {
                Some(builtin::TESS_FACTOR_OUTER) => (0, 4),
                Some(builtin::TESS_FACTOR_INNER) => (4, 2),
                _ => return Err(unsupported("HullOutput")),
            };
            let var = io.var;
            let shape = io.ty.dims.first().copied().unwrap_or(1);
            for f in 0..count.min(shape) {
                let p = b.global(var);
                let p = if io.ty.dims.is_empty() {
                    p
                } else {
                    b.field(p, f)
                };
                let value = b.load(p);
                let value = b.bitcast(value, ScalarKind::Uint);
                store(&mut b, factor_base + first + f, value);
            }
        }
        // The header, written by patch 0's invocation.
        let zero = b.u32(0);
        let first = b.binary(BinaryOperator::Equal, patch, zero);
        b.when(first, |b| {
            let mut header = alloc::vec![tess::NO_SLOT; tess::HEADER_WORDS as usize];
            header[tess::PATCH_WORDS as usize] = patch_words;
            header[tess::POINT_WORDS as usize] = point_words;
            header[tess::CONSTANT_BASE as usize] = constant_base;
            header[tess::FACTOR_BASE as usize] = factor_base;
            for (slot, &loc) in point_outputs.iter().enumerate() {
                let at = match loc {
                    tess::LOCATIONS => tess::POSITION_SLOT,
                    _ => tess::SLOTS + loc,
                };
                header[at as usize] = slot as u32;
            }
            for (slot, &loc) in constant_outputs.iter().enumerate() {
                header[(tess::SLOTS + tess::LOCATIONS + loc) as usize] = slot as u32;
            }
            for (w, value) in header.into_iter().enumerate() {
                let at = b.u32(w as u32);
                let p = b.global(out);
                let p = b.at(p, at);
                let value = b.u32(value);
                b.store(p, value);
            }
        });
        b.ret(None);

        let t = self.tessellation.as_ref().unwrap();
        self.reflection.extras = StageExtras::Hull {
            input_control_points: points_in as u8,
            output_control_points: points_out as u8,
            domain: t.domain,
            partitioning: t.partitioning,
            output_primitive: t.output,
            max_tess_factor: 64,
            point_locations: point_outputs.len() as u8,
            constant_locations: constant_outputs.len() as u8,
        };
        self.module.entry_points.push(naga::EntryPoint {
            name: "main".into(),
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

    /// The function a domain program's entry point calls first with its vertex number: it fills every
    /// input from the point it names and that point's patch record.
    pub(super) fn domain_inputs(&mut self) -> Result<Handle<naga::Function>> {
        let domain = self.tessellation.as_ref().unwrap().domain;
        let m = &mut self.module;
        let u32_ty = build::scalar(m, Scalar::U32);
        let uvec4 = build::vec4(m, Scalar::U32);
        let read = |m: &mut naga::Module, name: &str, binding: u32, ty| {
            build::global(
                m,
                name,
                AddressSpace::Storage {
                    access: StorageAccess::LOAD,
                },
                Some(ResourceBinding {
                    group: HELPER_SET,
                    binding,
                }),
                ty,
            )
        };
        let words = build::runtime_array(m, u32_ty, 4);
        let patches = read(
            m,
            "stage_patches",
            wcontainers_gpu::DOMAIN_PATCHES_BINDING,
            words,
        );
        let point_array = build::runtime_array(m, uvec4, 16);
        let points = read(
            m,
            "stage_points",
            wcontainers_gpu::DOMAIN_POINTS_BINDING,
            point_array,
        );

        let mut b = Builder::new("domain_inputs");
        b.f.arguments.push(FunctionArgument {
            name: Some("vertex".into()),
            ty: u32_ty,
            binding: None,
        });
        let vertex = b.argument(0);
        let p = b.global(points);
        let p = b.at(p, vertex);
        let point = b.load(p);
        let patch = b.field(point, 2);
        let word = |b: &mut Builder, at: Expr| {
            let p = b.global(patches);
            let p = b.at(p, at);
            b.load(p)
        };
        let header_word = |b: &mut Builder, w: u32| {
            let at = b.u32(w);
            word(b, at)
        };
        let patch_words = header_word(&mut b, tess::PATCH_WORDS);
        let header = b.u32(tess::HEADER_WORDS);
        let along = b.mul(patch, patch_words);
        let base = b.add(header, along);

        // The patch's control points, as the arrays of control point data declare them.
        let point_count = self.io.iter().find_map(|(id, io)| {
            let factor = matches!(
                io.builtin,
                Some(builtin::TESS_FACTOR_OUTER | builtin::TESS_FACTOR_INNER)
            );
            (self.is_input(*id) && io.ty.dims.len() == 1 && !factor).then(|| io.ty.dims[0])
        });
        for i in 0..self.io.len() {
            let (id, ref io) = self.io[i];
            if !self.is_input(id) {
                continue;
            }
            let (var, location, bi) = (io.var, io.location, io.builtin);
            let dims = io.ty.dims.clone();
            // `SV_Position` as control point data reads as a location of its own.
            let (location, bi) = match bi {
                Some(builtin::POSITION) => (Some((tess::LOCATIONS, 0)), None),
                _ => (location, bi),
            };
            let member = *io
                .ty
                .members
                .first()
                .ok_or_else(|| malformed("an input without a type".into()))?;
            let to = Self::scalar(member.scalar)?;
            if to == Scalar::BOOL {
                return Err(unsupported("DomainInputShape"));
            }
            match (location, bi) {
                (Some((loc, comp)), _) => {
                    // Control point data (arrays) or a patch constant: its slot from the header, zero
                    // where the hull shader writes none.
                    let (table, elements) = match (&dims[..], loc) {
                        ([n], tess::LOCATIONS) => (tess::POSITION_SLOT, Some(*n)),
                        (_, l) if l >= tess::LOCATIONS => {
                            return Err(unsupported("DomainInputLocation"));
                        }
                        ([n], _) => (tess::SLOTS + loc, Some(*n)),
                        ([], _) => (tess::SLOTS + tess::LOCATIONS + loc, None),
                        _ => return Err(unsupported("DomainInputShape")),
                    };
                    let slot = header_word(&mut b, table);
                    let none = b.u32(tess::NO_SLOT);
                    let absent = b.binary(BinaryOperator::Equal, slot, none);
                    let four = b.u32(4);
                    let slot_word = b.mul(slot, four);
                    let start = match elements {
                        Some(_) => base,
                        None => {
                            let cb = header_word(&mut b, tess::CONSTANT_BASE);
                            b.add(base, cb)
                        }
                    };
                    let point_words = header_word(&mut b, tess::POINT_WORDS);
                    for k in 0..elements.unwrap_or(1) {
                        let kk = b.u32(k);
                        let skip = b.mul(kk, point_words);
                        let at = b.add(start, skip);
                        let at = b.add(at, slot_word);
                        let mut comps = Vec::new();
                        for c in 0..member.size as u32 {
                            let o = b.u32(comp + c);
                            let at = b.add(at, o);
                            let w = word(&mut b, at);
                            let zero = b.u32(0);
                            comps.push(b.select(absent, zero, w));
                        }
                        let value = if comps.len() == 1 {
                            comps[0]
                        } else {
                            let ty = self.vec_ty(member.size, Scalar::U32)?;
                            b.compose(ty, comps)
                        };
                        let value = if to == Scalar::U32 {
                            value
                        } else {
                            b.bitcast(value, to.kind)
                        };
                        let g = b.global(var);
                        let p = if elements.is_some() { b.field(g, k) } else { g };
                        b.store(p, value);
                    }
                }
                (None, Some(builtin::TESS_COORD)) => {
                    if member.size != 3 || to != Scalar::F32 || !dims.is_empty() {
                        return Err(unsupported("DomainInputShape"));
                    }
                    let u = b.field(point, 0);
                    let u = b.bitcast(u, ScalarKind::Float);
                    let v = b.field(point, 1);
                    let v = b.bitcast(v, ScalarKind::Float);
                    let w = if domain == 0 {
                        let one = b.expr(naga::Expression::Literal(naga::Literal::F32(1.0)));
                        let w = b.binary(BinaryOperator::Subtract, one, u);
                        b.binary(BinaryOperator::Subtract, w, v)
                    } else {
                        b.expr(naga::Expression::Literal(naga::Literal::F32(0.0)))
                    };
                    let ty = self.vec_ty(3, Scalar::F32)?;
                    let value = b.compose(ty, alloc::vec![u, v, w]);
                    let g = b.global(var);
                    b.store(g, value);
                }
                (None, Some(builtin::TESS_CONTROL_POINT_COUNT_IN)) => {
                    let n = point_count.ok_or(unsupported("DomainPointCount"))?;
                    let value = b.u32(n);
                    let value = if to == Scalar::U32 {
                        value
                    } else {
                        b.bitcast(value, to.kind)
                    };
                    let g = b.global(var);
                    b.store(g, value);
                }
                (None, Some(builtin::PRIMITIVE_ID)) => {
                    let value = b.field(point, 3);
                    let value = if to == Scalar::U32 {
                        value
                    } else {
                        b.bitcast(value, to.kind)
                    };
                    let g = b.global(var);
                    b.store(g, value);
                }
                (None, Some(which @ (builtin::TESS_FACTOR_OUTER | builtin::TESS_FACTOR_INNER))) => {
                    let first = if which == builtin::TESS_FACTOR_OUTER {
                        0
                    } else {
                        4
                    };
                    let factor_base = header_word(&mut b, tess::FACTOR_BASE);
                    let at = b.add(base, factor_base);
                    for f in 0..dims.first().copied().unwrap_or(1).min(4) {
                        let o = b.u32(first + f);
                        let at = b.add(at, o);
                        let w = word(&mut b, at);
                        let value = b.bitcast(w, ScalarKind::Float);
                        let g = b.global(var);
                        let p = if dims.is_empty() { g } else { b.field(g, f) };
                        b.store(p, value);
                    }
                }
                _ => return Err(unsupported("DomainInput")),
            }
        }
        b.ret(None);
        let t = self.tessellation.as_ref().unwrap();
        self.reflection.extras = StageExtras::Domain {
            domain: t.domain,
            input_control_points: t.points_in as u8,
        };
        Ok(self.module.functions.append(b.finish(), Span::UNDEFINED))
    }
}
