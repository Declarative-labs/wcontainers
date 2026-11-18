//! Storage buffers read from an offset into their binding (`VariantFlags::STORAGE_OFFSET`): WebGPU binds
//! storage at multiples of 256 bytes, Direct3D's raw and structured views start at any element, so the
//! front binds from below the view and the program adds the rest, in whole elements, to every index.

use super::{HelperMember, Result, each_block, helper, scalar};
use crate::ConvertError;
use alloc::vec::Vec;
use naga::{
    AddressSpace, ArraySize, BinaryOperator, Expression, Function, Handle, Literal, Module, Range,
    Scalar, ScalarKind, Span, Statement, TypeInner,
};
use wcontainers_gpu::{HelperValue, Reflection, ShaderStage, SlotKind};

/// One of the program's storage buffers: its variable, the struct member holding its runtime array
/// (none where the variable is the array), the array's stride and its offset's helper member.
struct Buffer {
    var: Handle<naga::GlobalVariable>,
    member: Option<u32>,
    stride: u32,
    offset: HelperMember,
}

/// The runtime array of a storage variable of type `ty`: the struct member holding it, if any, and
/// its stride. `None` for a variable that is not a runtime array (a counter).
fn runtime_array(module: &Module, ty: Handle<naga::Type>) -> Result<Option<(Option<u32>, u32)>> {
    let array = |ty: Handle<naga::Type>| match module.types[ty].inner {
        TypeInner::Array {
            size: ArraySize::Dynamic,
            stride,
            ..
        } => Some(stride),
        _ => None,
    };
    if let Some(stride) = array(ty) {
        return Ok(Some((None, stride)));
    }
    match &module.types[ty].inner {
        TypeInner::Struct { members, .. } => match (members.len(), members.last()) {
            (1, Some(m)) => Ok(array(m.ty).map(|stride| (Some(0), stride))),
            // A block with sized members before its array: an offset cannot move them alone.
            (_, Some(m)) if array(m.ty).is_some() => {
                Err(ConvertError::Unsupported("StorageOffsetOfBlock"))
            }
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

/// Gives each storage binding of `reflection` its element's stride, which a storage offset comes in.
pub(crate) fn strides(module: &Module, reflection: &mut Reflection) {
    for (_, v) in module.global_variables.iter() {
        let (Some(rb), AddressSpace::Storage { .. }) = (v.binding, v.space) else {
            continue;
        };
        let Ok(Some((_, stride))) = runtime_array(module, v.ty) else {
            continue;
        };
        for b in reflection.bindings.iter_mut() {
            if b.set == rb.group && b.binding == rb.binding && b.kind == SlotKind::Storage {
                b.stride = stride;
            }
        }
    }
}

/// Makes every storage buffer of the program's own slots read from `HelperValue::StorageOffset` bytes
/// into its binding.
pub(crate) fn storage_offsets(
    module: &mut Module,
    stage: ShaderStage,
    reflection: &mut Reflection,
) -> Result<()> {
    let mut buffers = Vec::new();
    let globals: Vec<_> = module
        .global_variables
        .iter()
        .map(|(g, v)| (g, v.binding, v.space, v.ty))
        .collect();
    for (var, binding, space, ty) in globals {
        let (Some(rb), AddressSpace::Storage { .. }) = (binding, space) else {
            continue;
        };
        let own = reflection.bindings.iter().any(|b| {
            b.set == rb.group
                && b.binding == rb.binding
                && b.kind == SlotKind::Storage
                && b.added.is_none()
        });
        // Typed views carry their own place ([`HelperValue::TexelBufferView`], `TexelLayout`).
        let typed = reflection.helpers.iter().any(|h| match h.value {
            HelperValue::TexelBufferView { set, binding } => {
                set == rb.group && binding == rb.binding
            }
            HelperValue::TexelLayout { slot } => {
                slot == rb.binding - SlotKind::Storage.first_binding()
            }
            _ => false,
        });
        if !own || typed {
            continue;
        }
        let Some((member, stride)) = runtime_array(module, ty)? else {
            continue;
        };
        let u32_ty = scalar(module, Scalar::U32);
        let value = HelperValue::StorageOffset {
            set: rb.group,
            binding: rb.binding,
        };
        let offset = helper(module, reflection, stage, value, u32_ty, 4);
        buffers.push(Buffer {
            var,
            member,
            stride,
            offset,
        });
    }
    if buffers.is_empty() {
        return Ok(());
    }
    let handles: Vec<_> = module.functions.iter().map(|(h, _)| h).collect();
    for h in handles {
        let mut f = core::mem::take(&mut module.functions[h]);
        offset_function(module, &mut f, &buffers);
        module.functions[h] = f;
    }
    for i in 0..module.entry_points.len() {
        let mut f = core::mem::take(&mut module.entry_points[i].function);
        offset_function(module, &mut f, &buffers);
        module.entry_points[i].function = f;
    }
    Ok(())
}

/// Adds each buffer's offset, in elements, to every index into its array and takes it from every
/// length of it, each computed just before the expression it changes.
fn offset_function(module: &Module, f: &mut Function, buffers: &[Buffer]) {
    // The buffer whose runtime array `pointer` points at.
    let array_of = |f: &Function, pointer: Handle<Expression>| {
        buffers.iter().find(|b| match (f.expressions[pointer].clone(), b.member) {
            (Expression::GlobalVariable(g), None) => g == b.var,
            (Expression::AccessIndex { base, index }, Some(m)) => {
                index == m && matches!(f.expressions[base], Expression::GlobalVariable(g) if g == b.var)
            }
            _ => false,
        })
    };
    let mut sites = Vec::new();
    for (h, e) in f.expressions.iter() {
        let site = match *e {
            Expression::Access { base, .. } | Expression::AccessIndex { base, .. } => {
                array_of(f, base)
            }
            Expression::ArrayLength(pointer) => array_of(f, pointer).or_else(|| {
                // A length taken of the block holding the array.
                match f.expressions[pointer] {
                    Expression::GlobalVariable(g) => {
                        buffers.iter().find(|b| b.var == g && b.member.is_some())
                    }
                    _ => None,
                }
            }),
            _ => None,
        };
        if let Some(b) = site {
            sites.push((h, b));
        }
    }
    if sites.is_empty() {
        return;
    }
    let original = f.expressions.len();
    let mut placed = Vec::new();
    // Each site and the expressions made for it, which go just before it in the arena.
    let mut made = Vec::new();
    for (h, b) in sites {
        let from = f.expressions.len();
        let append = |f: &mut Function, e| f.expressions.append(e, Span::UNDEFINED);
        // Never emitted: read where they are used.
        let block = append(f, Expression::GlobalVariable(b.offset.0));
        let stride = append(f, Expression::Literal(Literal::U32(b.stride)));
        let constant = match f.expressions[h] {
            Expression::AccessIndex { index, .. } => {
                Some(append(f, Expression::Literal(Literal::U32(index))))
            }
            _ => None,
        };
        let first = append(
            f,
            Expression::AccessIndex {
                base: block,
                index: b.offset.1,
            },
        );
        let bytes = append(f, Expression::Load { pointer: first });
        let elements = append(
            f,
            Expression::Binary {
                op: BinaryOperator::Divide,
                left: bytes,
                right: stride,
            },
        );
        let replaced = match f.expressions[h].clone() {
            Expression::Access { base, index } => {
                let index = append(
                    f,
                    Expression::As {
                        expr: index,
                        kind: ScalarKind::Uint,
                        convert: None,
                    },
                );
                let index = append(
                    f,
                    Expression::Binary {
                        op: BinaryOperator::Add,
                        left: index,
                        right: elements,
                    },
                );
                Expression::Access { base, index }
            }
            Expression::AccessIndex { base, .. } => {
                let index = append(
                    f,
                    Expression::Binary {
                        op: BinaryOperator::Add,
                        left: constant.expect("made above"),
                        right: elements,
                    },
                );
                Expression::Access { base, index }
            }
            length => {
                let whole = append(f, length);
                Expression::Binary {
                    op: BinaryOperator::Subtract,
                    left: whole,
                    right: elements,
                }
            }
        };
        let last = f.expressions.len();
        let pre = f.expressions.range_from(first.index());
        debug_assert_eq!(pre.clone().count(), last - first.index());
        *f.expressions.get_mut(h) = replaced;
        placed.push((h, pre));
        made.push((h, from..last));
    }
    emit_before(f, &placed);
    let all: Vec<_> = f.expressions.iter().map(|(h, _)| h).collect();
    let mut order = Vec::with_capacity(all.len());
    let mut made = made.into_iter().peekable();
    for &h in &all[..original] {
        if let Some((_, range)) = made.next_if(|m| m.0 == h) {
            order.extend_from_slice(&all[range]);
        }
        order.push(h);
    }
    naga::compact::reorder_expressions(module, f, &order);
}

/// Emits each `(h, pre)`'s expressions `pre` just before `h` is emitted.
fn emit_before(f: &mut Function, placed: &[(Handle<Expression>, Range<Expression>)]) {
    if placed.is_empty() {
        return;
    }
    each_block(&mut f.body, &mut |block| {
        let touched = |s: &Statement| matches!(s, Statement::Emit(r) if r.clone().any(|h| placed.iter().any(|p| p.0 == h)));
        if !block.iter().any(touched) {
            return;
        }
        for (s, span) in core::mem::take(block).span_into_iter() {
            let Statement::Emit(range) = s else {
                block.push(s, span);
                continue;
            };
            let mut run: Option<(Handle<Expression>, Handle<Expression>)> = None;
            for h in range {
                if let Some((_, pre)) = placed.iter().find(|p| p.0 == h) {
                    if let Some((first, last)) = run.take() {
                        block.push(Statement::Emit(Range::new_from_bounds(first, last)), span);
                    }
                    block.push(Statement::Emit(pre.clone()), span);
                }
                run = Some((run.map_or(h, |r| r.0), h));
            }
            if let Some((first, last)) = run {
                block.push(Statement::Emit(Range::new_from_bounds(first, last)), span);
            }
        }
    });
}
