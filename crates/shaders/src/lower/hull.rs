//! Hull shaders as compute programs and domain shaders as vertex programs (plan section 7). A hull
//! invocation runs one patch: its control point function once per output control point, then its patch
//! constant function, and writes the patch's record in `crate::stage::tess`'s layout (and, for patch
//! 0, the header naming each location's slot). The domain program draws the tessellator's points: its
//! vertex number picks a point, which names its patch, whose record gives its inputs. Both routes use
//! these passes: each describes its program's stage values ([`TessellationValue`]) and functions.

use super::geometry::{INPUT_WITHOUT_TYPE, Reflected, Side};
use super::{Lowering, Result, malformed, unsupported};
use crate::build::{self, Builder, Expr};
use crate::ir::{self, Basic, Ty, builtin, code, primitive};
use crate::stage::{self, tess};
use alloc::vec::Vec;
use naga::{
    AddressSpace, BinaryOperator, Binding, BuiltIn, FunctionArgument, Handle, Module,
    ResourceBinding, Scalar, ScalarKind, Span, StorageAccess, Type, TypeInner, VectorSize,
};
use wcontainers_gpu::{reflect::HELPER_SET, reflect::StageExtras, reflect::SystemValue};

/// What a hull or domain shader declares.
#[derive(Default, Clone, Copy)]
pub(crate) struct Tessellation {
    /// `StageExtras::Hull`'s numbering.
    pub domain: u8,
    pub partitioning: u8,
    pub output: u8,
    pub points_in: u32,
    pub points_out: u32,
    /// The patch constant function, by id (the DXBC route's).
    pub constants: u32,
}

/// The most control points a patch has in Direct3D 11 and 12 (`D3D11_IA_PATCH_MAX_CONTROL_POINT_COUNT`).
pub(crate) const PATCH_MAX_POINTS: u32 = 32;

/// `StageExtras::Hull`'s and `StageExtras::Domain`'s domain codes.
pub(crate) const DOMAIN_TRIANGLE: u8 = 0;
pub(crate) const DOMAIN_QUAD: u8 = 1;
pub(crate) const DOMAIN_ISOLINE: u8 = 2;
/// `StageExtras::Hull`'s partitioning codes.
pub(crate) const PARTITIONING_INTEGER: u8 = 0;
pub(crate) const PARTITIONING_FRACTIONAL_ODD: u8 = 2;
pub(crate) const PARTITIONING_FRACTIONAL_EVEN: u8 = 3;
/// `StageExtras::Hull`'s triangle output codes (points and lines are `tess::OUTPUT_POINT` and
/// `tess::OUTPUT_LINE`).
pub(crate) const OUTPUT_CLOCKWISE: u8 = 2;
pub(crate) const OUTPUT_COUNTER_CLOCKWISE: u8 = 3;

/// dxbc-spirv's `ir::TessWindingOrder::eCw` and `ir::TessPartitioning` codes.
const IR_WINDING_CW: u64 = 1;
const IR_PARTITIONING_INTEGER: u64 = 0;
const IR_PARTITIONING_FRACTIONAL_ODD: u64 = 1;
const IR_PARTITIONING_FRACTIONAL_EVEN: u64 = 2;

/// The tessellator's point records' words: the domain location's u and v, the patch, the primitive.
const POINT_U: u32 = 0;
const POINT_V: u32 = 1;
const POINT_PATCH: u32 = 2;
pub(super) const POINT_PRIMITIVE: u32 = 3;
/// The tessellation factors' words in a record: outer 0 to 3, then inner 0 and 1.
const OUTER_FACTORS: u32 = build::VEC4;
const INNER_BASE: u32 = OUTER_FACTORS;
const INNER_FACTORS: u32 = 2;

/// The patch records, written by the hull program and read by the domain program.
const PATCHES_VARIABLE: &str = "stage_patches";
/// The tessellator's points the domain program reads, and a geometry program its primitives' numbers.
pub(super) const POINTS_VARIABLE: &str = "stage_points";
/// The function filling a domain program's inputs, and its argument: the vertex number.
pub(crate) const DOMAIN_INPUTS_FUNCTION: &str = "domain_inputs";
pub(crate) const DOMAIN_VERTEX_ARGUMENT: &str = "domain_vertex";

/// Why a hull or domain shader is malformed.
const OUTSIDE_TESSELLATION: &str = "a tessellation declaration outside hull and domain";
const NO_PATCH_CONSTANTS: &str = "a hull shader without its patch constant function";
const POINT_COUNTS: &str = "a hull shader's control point counts";
const OUTPUT_WITHOUT_TYPE: &str = "an output without a type";
const NO_DOMAIN_ENTRY: &str = "a domain program without its entry point";

/// One stage value of a hull or domain shader: the private variable the program reads or writes, and
/// where Direct3D places it (a register and component, or a built-in of `ir::builtin`).
pub(crate) struct TessellationValue {
    pub var: Handle<naga::GlobalVariable>,
    pub side: Side,
    pub location: Option<(u32, u32)>,
    pub builtin: Option<u64>,
    /// Its type: within one control point or the patch, a scalar or vector, or an array of them one
    /// register a row; arrayed by control point (the outermost count) where `per_point`.
    pub ty: Ty,
    pub per_point: bool,
}

impl TessellationValue {
    /// The control points it holds, when it holds one a control point.
    fn points(&self) -> Option<u32> {
        if self.per_point {
            self.ty.dims.last().copied()
        } else {
            None
        }
    }

    /// Its counts within one control point or the patch, innermost first.
    fn within(&self) -> &[u32] {
        let dims = &self.ty.dims[..];
        match self.per_point {
            true => dims.split_last().map_or(dims, |(_, rest)| rest),
            false => dims,
        }
    }

    /// Whether it is an array of rows.
    fn rowed(&self) -> bool {
        !self.within().is_empty()
    }

    /// The registers it takes.
    fn rows(&self) -> u32 {
        self.within().first().copied().unwrap_or(1)
    }

    fn member(&self, why: &str) -> Result<Basic> {
        self.ty
            .members
            .first()
            .copied()
            .ok_or_else(|| malformed(why.into()))
    }
}

/// The pass that makes a hull shader's compute entry point.
pub(crate) struct HullPass<'a> {
    pub module: &'a mut Module,
    pub tessellation: Tessellation,
    pub values: &'a [TessellationValue],
    /// The values the entry point reflects, in order.
    pub reflected: Vec<Reflected>,
}

/// The pass that makes a domain shader's input function.
pub(crate) struct DomainPass<'a> {
    pub module: &'a mut Module,
    pub tessellation: Tessellation,
    pub values: &'a [TessellationValue],
}

fn sorted(mut v: Vec<u32>) -> Vec<u32> {
    v.sort_unstable();
    v.dedup();
    v
}

/// A scalar or vector type of `n` components.
fn vector(module: &mut Module, n: u8, s: Scalar) -> Result<Handle<Type>> {
    let inner = match n {
        1 => TypeInner::Scalar(s),
        n => TypeInner::Vector {
            size: super::vector_size(n)?,
            scalar: s,
        },
    };
    Ok(module
        .types
        .insert(Type { name: None, inner }, Span::UNDEFINED))
}

/// `value` (u32) as `to`'s kind.
fn from_u32(b: &mut Builder, value: Expr, to: Scalar) -> Expr {
    if to == Scalar::U32 {
        value
    } else {
        b.bitcast(value, to.kind)
    }
}

/// The first and count of a tessellation factor built-in's words in a record.
fn factor_words(which: u64) -> Option<(u32, u32)> {
    match which {
        builtin::TESS_FACTOR_OUTER => Some((0, OUTER_FACTORS)),
        builtin::TESS_FACTOR_INNER => Some((INNER_BASE, INNER_FACTORS)),
        _ => None,
    }
}

impl Lowering<'_> {
    pub(super) fn tessellation_declaration(&mut self, op: &ir::Op) -> Result<()> {
        let t = self
            .tessellation
            .as_mut()
            .ok_or_else(|| malformed(OUTSIDE_TESSELLATION.into()))?;
        let lit = |i: usize| op.lits.get(i).copied().unwrap_or(0);
        match op.code {
            code::SET_TESS_DOMAIN => {
                t.domain = match lit(0) {
                    primitive::TRIANGLES => DOMAIN_TRIANGLE,
                    primitive::QUADS => DOMAIN_QUAD,
                    primitive::LINES => DOMAIN_ISOLINE,
                    _ => return Err(malformed("tessellation domain".into())),
                }
            }
            code::SET_TESS_PRIMITIVE => {
                t.output = match (lit(0), lit(1)) {
                    (primitive::POINTS, _) => tess::OUTPUT_POINT,
                    (primitive::LINES, _) => tess::OUTPUT_LINE,
                    (primitive::TRIANGLES, IR_WINDING_CW) => OUTPUT_CLOCKWISE,
                    (primitive::TRIANGLES, _) => OUTPUT_COUNTER_CLOCKWISE,
                    _ => return Err(malformed("tessellator output".into())),
                };
                t.partitioning = match lit(2) {
                    IR_PARTITIONING_INTEGER => PARTITIONING_INTEGER,
                    IR_PARTITIONING_FRACTIONAL_ODD => PARTITIONING_FRACTIONAL_ODD,
                    IR_PARTITIONING_FRACTIONAL_EVEN => PARTITIONING_FRACTIONAL_EVEN,
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

    /// The stage values as the hull and domain passes take them, with each one's declaring id.
    fn tessellation_values(&self) -> Result<(Vec<TessellationValue>, Vec<u32>)> {
        let mut values = Vec::new();
        let mut ids = Vec::new();
        for (id, io) in &self.io {
            let input = matches!(
                self.ir.op(*id)?.code,
                code::DCL_INPUT | code::DCL_INPUT_BUILT_IN
            );
            values.push(TessellationValue {
                var: io.var,
                side: if input { Side::Input } else { Side::Output },
                location: io.location,
                builtin: io.builtin,
                per_point: !io.ty.dims.is_empty(),
                ty: io.ty.clone(),
            });
            ids.push(*id);
        }
        Ok((values, ids))
    }

    /// The compute entry point of a hull shader, and its inputs' reflection.
    pub(super) fn hull_entry_point(&mut self) -> Result<()> {
        let (values, ids) = self.tessellation_values()?;
        let tessellation = *self.tessellation.as_ref().unwrap();
        let main = self.functions[&self.main];
        let constants = self
            .functions
            .get(&tessellation.constants)
            .copied()
            .ok_or_else(|| malformed(NO_PATCH_CONSTANTS.into()))?;
        let mut pass = HullPass {
            module: &mut self.module,
            tessellation,
            values: &values,
            reflected: Vec::new(),
        };
        let done = pass.entry_point(main, Some(constants));
        let reflected = core::mem::take(&mut pass.reflected);
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

    /// The function a domain program's entry point calls first with its vertex number.
    pub(super) fn domain_inputs(&mut self) -> Result<Handle<naga::Function>> {
        let (values, _) = self.tessellation_values()?;
        let mut pass = DomainPass {
            module: &mut self.module,
            tessellation: *self.tessellation.as_ref().unwrap(),
            values: &values,
        };
        let (function, extras) = pass.inputs()?;
        self.reflection.extras = extras;
        Ok(function)
    }
}

impl HullPass<'_> {
    /// The four words of slot `loc` from the outputs declared there (indices into `values`, with the
    /// row where they are arrays of rows), of control point `element` where they hold one a control
    /// point; zero where none is declared.
    fn slot_words(
        &self,
        b: &mut Builder,
        decls: &[(usize, Option<u32>)],
        element: Option<u32>,
    ) -> Result<Vec<Expr>> {
        let mut words = Vec::new();
        for c in 0..build::VEC4 {
            let found = decls.iter().find(|&&(i, _)| {
                let v = &self.values[i];
                let (_, comp) = v.location.unwrap();
                let size = v.ty.members.first().map_or(1, |m| m.size as u32);
                (comp..comp + size).contains(&c)
            });
            let w = match found {
                Some(&(i, row)) => {
                    let v = &self.values[i];
                    let (_, comp) = v.location.unwrap();
                    let m = v.member(OUTPUT_WITHOUT_TYPE)?;
                    let p = b.global(v.var);
                    let p = match element {
                        Some(k) => b.field(p, k),
                        None => p,
                    };
                    let p = match row {
                        Some(r) => b.field(p, r),
                        None => p,
                    };
                    let value = b.load(p);
                    let value = if m.size > 1 {
                        b.field(value, c - comp)
                    } else {
                        value
                    };
                    let from = Lowering::scalar(m.scalar)?;
                    if from == Scalar::BOOL {
                        let one = b.u32(1);
                        let zero = b.u32(0);
                        b.select(value, one, zero)
                    } else if from.kind == ScalarKind::Uint {
                        value
                    } else {
                        b.bitcast(value, ScalarKind::Uint)
                    }
                }
                None => b.u32(0),
            };
            words.push(w);
        }
        Ok(words)
    }

    /// The outputs declared at slot `loc` holding one value a control point (`per_point`) or one for
    /// the patch, by index into `values`, with the row of `loc` where they are arrays of rows.
    fn output_decls(&self, loc: u32, per_point: bool) -> Vec<(usize, Option<u32>)> {
        self.values
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let (first, _) = v.location?;
                (v.side == Side::Output
                    && v.per_point == per_point
                    && (first..first + v.rows()).contains(&loc))
                .then(|| (i, v.rowed().then_some(loc - first)))
            })
            .collect()
    }

    /// The compute entry point: one invocation per patch (x) and API instance (y). `main` is the control
    /// point function, run once per output control point in order, then `constants`, the patch constant
    /// function, where the program has one of its own.
    pub(crate) fn entry_point(
        &mut self,
        main: Handle<naga::Function>,
        constants: Option<Handle<naga::Function>>,
    ) -> Result<StageExtras> {
        let t = self.tessellation;
        let (points_in, points_out) = (t.points_in, t.points_out);
        if points_in == 0 || points_in > PATCH_MAX_POINTS || points_out > PATCH_MAX_POINTS {
            return Err(malformed(POINT_COUNTS.into()));
        }
        if points_out == 0 {
            // No control point is made, so the control point function never runs; its outputs are
            // arrays of none, which its body would index.
            let f = &mut self.module.functions[main];
            *f = naga::Function {
                name: f.name.take(),
                ..Default::default()
            };
        }
        // Inputs: the vertex records' slots, in order of location.
        let mut inputs = Vec::new();
        let mut point_outputs = Vec::new();
        let mut constant_outputs = Vec::new();
        for v in self.values {
            let rows = |loc: u32| loc..loc + v.rows();
            match (v.side, v.location, v.per_point) {
                (Side::Input, Some((loc, _)), _) => inputs.extend(rows(loc)),
                (Side::Output, Some((loc, _)), true) => point_outputs.extend(rows(loc)),
                (Side::Output, Some((loc, _)), false) => constant_outputs.extend(rows(loc)),
                (Side::Output, None, true) if v.builtin == Some(builtin::POSITION) => {
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
            return Err(unsupported(crate::refusal::HULL_OUTPUT_LOCATION));
        }
        let in_slots = stage::record_words(inputs.len()) / build::VEC4;
        let point_words = build::VEC4 * point_outputs.len() as u32;
        let constant_base = points_out * point_words;
        let factor_base = constant_base + build::VEC4 * constant_outputs.len() as u32;
        let patch_words = factor_base + tess::FACTOR_WORDS;

        let m = &mut *self.module;
        let values = stage::values(m);
        let uvec4 = build::vec4(m, Scalar::U32);
        let u32_ty = build::scalar(m, Scalar::U32);
        let records = build::runtime_array(m, uvec4, stage::RECORD_SLOT_BYTES);
        let vertices = build::global(
            m,
            stage::VERTICES_VARIABLE,
            AddressSpace::Storage {
                access: StorageAccess::LOAD,
            },
            Some(stage::binding(stage::VERTICES_BINDING)),
            records,
        );
        let words = build::runtime_array(m, u32_ty, build::WORD);
        let out = build::global(
            m,
            PATCHES_VARIABLE,
            AddressSpace::Storage {
                access: StorageAccess::LOAD | StorageAccess::STORE,
            },
            Some(stage::binding(stage::PATCHES_BINDING)),
            words,
        );
        let uvec3 = vector(m, VectorSize::Tri as u8, Scalar::U32)?;
        let mut b = Builder::new(crate::ENTRY_POINT);
        b.f.arguments.push(FunctionArgument {
            name: Some(stage::ID_ARGUMENT.into()),
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
        for (i, value) in self.values.iter().enumerate() {
            if value.side != Side::Input {
                continue;
            }
            let (var, location, bi) = (value.var, value.location, value.builtin);
            let member = value.member(INPUT_WITHOUT_TYPE)?;
            let to = Lowering::scalar(member.scalar)?;
            let reflect = |location, component, system| Reflected {
                value: i,
                side: Side::Input,
                location,
                component,
                member,
                modes: 0,
                system,
            };
            match (location, bi) {
                (Some((loc, comp)), _) => {
                    let short = value.points().is_none_or(|n| n < points_in);
                    if short || value.within().len() > 1 || to == Scalar::BOOL {
                        return Err(unsupported(crate::refusal::HULL_INPUT_SHAPE));
                    }
                    let rows = value.rows();
                    for k in 0..points_in {
                        for r in 0..rows {
                            let slot =
                                1 + inputs.iter().position(|&l| l == loc + r).unwrap() as u32;
                            let kk = b.u32(k);
                            let record = b.add(first_record, kk);
                            let w = b.u32(in_slots);
                            let record = b.mul(record, w);
                            let s = b.u32(slot);
                            let at = b.add(record, s);
                            let p = b.global(vertices);
                            let p = b.at(p, at);
                            let raw = b.load(p);
                            let read = super::geometry::swizzle(&mut b, raw, comp, member.size)?;
                            let read = from_u32(&mut b, read, to);
                            let g = b.global(var);
                            let p = b.field(g, k);
                            let p = if value.rowed() { b.field(p, r) } else { p };
                            b.store(p, read);
                        }
                    }
                    for r in 0..rows {
                        self.reflected.push(reflect(Some(loc + r), comp, None));
                    }
                }
                (None, Some(builtin::POSITION)) => {
                    let short = value.points().is_none_or(|n| n < points_in);
                    if short || value.rowed() || u32::from(member.size) != build::VEC4 {
                        return Err(unsupported(crate::refusal::HULL_INPUT_SHAPE));
                    }
                    for k in 0..points_in {
                        let kk = b.u32(k);
                        let record = b.add(first_record, kk);
                        let w = b.u32(in_slots);
                        let at = b.mul(record, w);
                        let p = b.global(vertices);
                        let p = b.at(p, at);
                        let raw = b.load(p);
                        let read = b.bitcast(raw, ScalarKind::Float);
                        let g = b.global(var);
                        let p = b.field(g, k);
                        b.store(p, read);
                    }
                    self.reflected
                        .push(reflect(None, 0, Some(SystemValue::Position)));
                }
                (None, Some(which)) => {
                    if !value.ty.dims.is_empty() || member.size != 1 || to == Scalar::BOOL {
                        return Err(unsupported(crate::refusal::HULL_INPUT_SHAPE));
                    }
                    let read = match which {
                        builtin::PRIMITIVE_ID => x,
                        builtin::TESS_CONTROL_POINT_COUNT_IN => b.u32(points_in),
                        builtin::TESS_CONTROL_POINT_ID => {
                            point_id = Some((var, to));
                            continue;
                        }
                        _ => return Err(unsupported(crate::refusal::HULL_INPUT)),
                    };
                    let read = from_u32(&mut b, read, to);
                    let p = b.global(var);
                    b.store(p, read);
                    if which == builtin::PRIMITIVE_ID {
                        self.reflected
                            .push(reflect(None, 0, Some(SystemValue::PrimitiveId)));
                    }
                }
                _ => return Err(unsupported(crate::refusal::HULL_INPUT)),
            }
        }

        // The control point function per output control point, then the patch constant function.
        let zero = b.u32(0);
        let end = b.u32(points_out);
        b.each(self.module, zero, end, |b, k| {
            if let Some((var, to)) = point_id {
                let value = from_u32(b, k, to);
                let p = b.global(var);
                b.store(p, value);
            }
            b.call(main, Vec::new(), false);
        });
        if let Some(constants) = constants {
            b.call(constants, Vec::new(), false);
        }

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
        let position = self.values.iter().find_map(|v| {
            (v.side == Side::Output && v.builtin == Some(builtin::POSITION)).then_some(v.var)
        });
        for (slot, &loc) in point_outputs.iter().enumerate() {
            let decls = self.output_decls(loc, true);
            for k in 0..points_out {
                let words = match (loc == tess::LOCATIONS, position) {
                    (true, Some(var)) => {
                        let p = b.global(var);
                        let p = b.field(p, k);
                        let v = b.load(p);
                        let v = b.bitcast(v, ScalarKind::Uint);
                        (0..build::VEC4).map(|c| b.field(v, c)).collect()
                    }
                    _ => self.slot_words(&mut b, &decls, Some(k))?,
                };
                for (c, w) in words.into_iter().enumerate() {
                    store(
                        &mut b,
                        k * point_words + slot as u32 * build::VEC4 + c as u32,
                        w,
                    );
                }
            }
        }
        for (slot, &loc) in constant_outputs.iter().enumerate() {
            let decls = self.output_decls(loc, false);
            let words = self.slot_words(&mut b, &decls, None)?;
            for (c, w) in words.into_iter().enumerate() {
                store(
                    &mut b,
                    constant_base + slot as u32 * build::VEC4 + c as u32,
                    w,
                );
            }
        }
        for v in self.values {
            if v.side == Side::Input || v.location.is_some() || v.builtin == Some(builtin::POSITION)
            {
                continue;
            }
            let (first, count) = v
                .builtin
                .and_then(factor_words)
                .ok_or(unsupported(crate::refusal::HULL_OUTPUT))?;
            let shape = v.ty.dims.first().copied().unwrap_or(1);
            for f in 0..count.min(shape) {
                let p = b.global(v.var);
                let p = if v.ty.dims.is_empty() {
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
        Ok(StageExtras::Hull {
            input_control_points: points_in as u8,
            output_control_points: points_out as u8,
            domain: t.domain,
            partitioning: t.partitioning,
            output_primitive: t.output,
            max_tess_factor: tess::MAX_FACTOR as u32,
            point_locations: point_outputs.len() as u8,
            constant_locations: constant_outputs.len() as u8,
        })
    }
}

/// Makes a domain program's one entry point take its vertex number and call `inputs`, the function
/// [`DomainPass::inputs`] made, with it before anything else.
pub(crate) fn call_domain_inputs(
    module: &mut Module,
    inputs: Handle<naga::Function>,
) -> Result<()> {
    let u32_ty = build::scalar(module, Scalar::U32);
    let ep = module
        .entry_points
        .first_mut()
        .ok_or_else(|| malformed(NO_DOMAIN_ENTRY.into()))?;
    let f = &mut ep.function;
    f.arguments.push(FunctionArgument {
        name: Some(DOMAIN_VERTEX_ARGUMENT.into()),
        ty: u32_ty,
        binding: Some(Binding::BuiltIn(BuiltIn::VertexIndex)),
    });
    let vertex = f.expressions.append(
        naga::Expression::FunctionArgument(f.arguments.len() as u32 - 1),
        Span::UNDEFINED,
    );
    let call = naga::Statement::Call {
        function: inputs,
        arguments: alloc::vec![vertex],
        result: None,
    };
    f.body.splice(..0, naga::Block::from_vec(alloc::vec![call]));
    Ok(())
}

impl DomainPass<'_> {
    /// The function a domain program's entry point calls first with its vertex number: it fills every
    /// input from the point it names and that point's patch record. Also the stage's extras.
    pub(crate) fn inputs(&mut self) -> Result<(Handle<naga::Function>, StageExtras)> {
        let t = self.tessellation;
        let m = &mut *self.module;
        let u32_ty = build::scalar(m, Scalar::U32);
        let uvec4 = build::vec4(m, Scalar::U32);
        let read = |m: &mut Module, name: &str, binding: u32, ty| {
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
        let words = build::runtime_array(m, u32_ty, build::WORD);
        let patches = read(
            m,
            PATCHES_VARIABLE,
            wcontainers_gpu::reflect::DOMAIN_PATCHES_BINDING,
            words,
        );
        let point_array = build::runtime_array(m, uvec4, stage::RECORD_SLOT_BYTES);
        let points = read(
            m,
            POINTS_VARIABLE,
            wcontainers_gpu::reflect::DOMAIN_POINTS_BINDING,
            point_array,
        );

        let mut b = Builder::new(DOMAIN_INPUTS_FUNCTION);
        b.f.arguments.push(FunctionArgument {
            name: Some(DOMAIN_VERTEX_ARGUMENT.into()),
            ty: u32_ty,
            binding: None,
        });
        let vertex = b.argument(0);
        let p = b.global(points);
        let p = b.at(p, vertex);
        let point = b.load(p);
        let patch = b.field(point, POINT_PATCH);
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
        let point_count = self.values.iter().find_map(|v| {
            let factor = v.builtin.and_then(factor_words).is_some();
            (v.side == Side::Input && !factor)
                .then(|| v.points())
                .flatten()
        });
        for v in self.values {
            if v.side != Side::Input {
                continue;
            }
            let var = v.var;
            let dims = &v.ty.dims;
            // `SV_Position` as control point data reads as a location of its own.
            let (location, bi) = match v.builtin {
                Some(builtin::POSITION) => (Some((tess::LOCATIONS, 0)), None),
                b => (v.location, b),
            };
            let member = v.member(INPUT_WITHOUT_TYPE)?;
            let to = Lowering::scalar(member.scalar)?;
            if to == Scalar::BOOL {
                return Err(unsupported(crate::refusal::DOMAIN_INPUT_SHAPE));
            }
            match (location, bi) {
                (Some((loc, comp)), _) => {
                    // Control point data or a patch constant: its slot from the header, zero where the
                    // hull shader writes none.
                    let rows = v.rows();
                    let (table, elements) = match (v.points(), v.within(), loc) {
                        (Some(n), [], tess::LOCATIONS) => (tess::POSITION_SLOT, Some(n)),
                        (_, _, l) if l + rows > tess::LOCATIONS => {
                            return Err(unsupported(crate::refusal::DOMAIN_INPUT_LOCATION));
                        }
                        (Some(n), [] | [_], _) => (tess::SLOTS + loc, Some(n)),
                        (None, [] | [_], _) => (tess::SLOTS + tess::LOCATIONS + loc, None),
                        _ => return Err(unsupported(crate::refusal::DOMAIN_INPUT_SHAPE)),
                    };
                    for r in 0..rows {
                        let slot = header_word(&mut b, table + r);
                        let none = b.u32(tess::NO_SLOT);
                        let absent = b.binary(BinaryOperator::Equal, slot, none);
                        let slot_words = b.u32(build::VEC4);
                        let slot_word = b.mul(slot, slot_words);
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
                                let ty = vector(self.module, member.size, Scalar::U32)?;
                                b.compose(ty, comps)
                            };
                            let value = from_u32(&mut b, value, to);
                            let g = b.global(var);
                            let p = if elements.is_some() { b.field(g, k) } else { g };
                            let p = if v.rowed() { b.field(p, r) } else { p };
                            b.store(p, value);
                        }
                    }
                }
                (None, Some(builtin::TESS_COORD)) => {
                    let coordinates = VectorSize::Tri as u8;
                    if member.size != coordinates || to != Scalar::F32 || !dims.is_empty() {
                        return Err(unsupported(crate::refusal::DOMAIN_INPUT_SHAPE));
                    }
                    let u = b.field(point, POINT_U);
                    let u = b.bitcast(u, ScalarKind::Float);
                    let v = b.field(point, POINT_V);
                    let v = b.bitcast(v, ScalarKind::Float);
                    let w = if t.domain == DOMAIN_TRIANGLE {
                        let one = b.expr(naga::Expression::Literal(naga::Literal::F32(1.0)));
                        let w = b.binary(BinaryOperator::Subtract, one, u);
                        b.binary(BinaryOperator::Subtract, w, v)
                    } else {
                        b.expr(naga::Expression::Literal(naga::Literal::F32(0.0)))
                    };
                    let ty = vector(self.module, coordinates, Scalar::F32)?;
                    let value = b.compose(ty, alloc::vec![u, v, w]);
                    let g = b.global(var);
                    b.store(g, value);
                }
                (None, Some(builtin::TESS_CONTROL_POINT_COUNT_IN)) => {
                    let n = point_count.ok_or(unsupported(crate::refusal::DOMAIN_POINT_COUNT))?;
                    let value = b.u32(n);
                    let value = from_u32(&mut b, value, to);
                    let g = b.global(var);
                    b.store(g, value);
                }
                (None, Some(builtin::PRIMITIVE_ID)) => {
                    let value = b.field(point, POINT_PRIMITIVE);
                    let value = from_u32(&mut b, value, to);
                    let g = b.global(var);
                    b.store(g, value);
                }
                (None, Some(which)) if factor_words(which).is_some() => {
                    let (first, count) = factor_words(which).unwrap();
                    let factor_base = header_word(&mut b, tess::FACTOR_BASE);
                    let at = b.add(base, factor_base);
                    for f in 0..dims.first().copied().unwrap_or(1).min(count) {
                        let o = b.u32(first + f);
                        let at = b.add(at, o);
                        let w = word(&mut b, at);
                        let value = b.bitcast(w, ScalarKind::Float);
                        let g = b.global(var);
                        let p = if dims.is_empty() { g } else { b.field(g, f) };
                        b.store(p, value);
                    }
                }
                _ => return Err(unsupported(crate::refusal::DOMAIN_INPUT)),
            }
        }
        b.ret(None);
        let extras = StageExtras::Domain {
            domain: t.domain,
            input_control_points: t.points_in as u8,
        };
        Ok((
            self.module.functions.append(b.finish(), Span::UNDEFINED),
            extras,
        ))
    }
}
