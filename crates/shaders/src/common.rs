//! The rewrites of plan 15.4 on a program another route brought into the common form: arrays of
//! textures, storage textures and samplers split into their elements, gathers whose offset is known
//! only at run time, and storage textures given the bound view's format.

use crate::ConvertError;
use crate::dxil::arrays::{constant, copy_constant, image_of, operands, split_with};
use crate::dxil::images::{Declared, finish};
use crate::dxil::{Read, outline};
use alloc::vec::Vec;
use naga::{
    ArraySize, BinaryOperator, Expression, Function, FunctionArgument, FunctionResult,
    GlobalVariable, Handle, ImageClass, ImageDimension, ImageQuery, Literal, Module, Range,
    ResourceBinding, Scalar, ScalarKind, Span, Statement, TypeInner, VectorSize,
};
use wcontainers_gpu::{
    reflect::Reflection, reflect::SLOT_KINDS, reflect::SlotKind, reflect::VariantKey,
    shader::ShaderStage,
};

type Result<T> = core::result::Result<T, ConvertError>;

/// How many low bits of each component of a gather's offset Direct3D takes, signed; Vulkan leaves
/// offsets outside its smaller range undefined, so the same bits serve both.
pub(crate) const GATHER_OFFSET_BITS: u32 = 6;

/// Splits each binding array of textures, storage textures or samplers into its elements, element
/// `k` bound at the array's binding plus `k` (the consecutive slots its reflected `count` names).
/// Returns whether any array was split.
pub(crate) fn split_arrays(module: &mut Module) -> Result<bool> {
    let any = module
        .global_variables
        .iter()
        .any(|(_, g)| matches!(module.types[g.ty].inner, TypeInner::BindingArray { .. }));
    if !any {
        return Ok(false);
    }
    split_with(
        module,
        |module, binding, base, size| {
            let kind = match module.types[base].inner {
                TypeInner::Image {
                    class: ImageClass::Storage { .. },
                    ..
                } => SlotKind::Storage,
                TypeInner::Image { .. } => SlotKind::Texture,
                TypeInner::Sampler { .. } => SlotKind::Sampler,
                _ => return Err(ConvertError::Unsupported(crate::refusal::RESOURCE_ARRAY)),
            };
            let ArraySize::Constant(n) = size else {
                return Err(ConvertError::Unsupported(
                    crate::refusal::UNBOUNDED_RESOURCE_ARRAY,
                ));
            };
            let end = binding
                .binding
                .checked_sub(SLOT_KINDS[kind as usize].first_binding)
                .and_then(|slot| slot.checked_add(n.get()))
                .filter(|&end| end <= SLOT_KINDS[kind as usize].slots)
                .ok_or(ConvertError::Unsupported(
                    crate::refusal::BINDINGS_BEYOND_LIMIT,
                ))?
                + SLOT_KINDS[kind as usize].first_binding;
            Ok((binding.binding..end)
                .map(|b| ResourceBinding {
                    group: binding.group,
                    binding: b,
                })
                .collect())
        },
        |_, f| Ok(f.clone()),
    )?;
    Ok(true)
}

/// The (texture slot, sampler slot) pairs the program samples with, in the order its code first uses
/// them, each image and sampler a global of its own.
pub(crate) fn sampled_pairs(module: &Module) -> Result<Vec<(u32, u32)>> {
    let slot = |f: &Function, e: Handle<Expression>, kind: SlotKind| match f.expressions[e] {
        Expression::GlobalVariable(g) => module.global_variables[g]
            .binding
            .map(|b| b.binding - SLOT_KINDS[kind as usize].first_binding),
        _ => None,
    };
    let functions = module.functions.iter().map(|(_, f)| f);
    let mut pairs = Vec::new();
    for f in functions.chain(module.entry_points.iter().map(|ep| &ep.function)) {
        for (_, e) in f.expressions.iter() {
            let Expression::ImageSample { image, sampler, .. } = *e else {
                continue;
            };
            let texture = slot(f, image, SlotKind::Texture);
            let sampler = slot(f, sampler, SlotKind::Sampler);
            let (Some(texture), Some(sampler)) = (texture, sampler) else {
                return Err(ConvertError::Unsupported(
                    crate::refusal::SAMPLED_THROUGH_ARGUMENT,
                ));
            };
            if !pairs.contains(&(texture, sampler)) {
                pairs.push((texture, sampler));
            }
        }
    }
    Ok(pairs)
}

/// Makes each gather whose offset is not a constant a call of a helper that moves the coordinate by
/// the offset in texels of level 0, which picks the same four texels, and gathers there.
pub(crate) fn gather_offsets(module: &mut Module) -> Result<()> {
    let functions = module.functions.iter().map(|(_, f)| f);
    let mut found = Vec::new();
    for (i, f) in functions
        .chain(module.entry_points.iter().map(|ep| &ep.function))
        .enumerate()
    {
        for (h, e) in f.expressions.iter() {
            if let Expression::ImageSample {
                gather: Some(_),
                offset: Some(o),
                ..
            } = *e
                && !constant(f, o)
            {
                found.push((i, h));
            }
        }
    }
    if found.is_empty() {
        return Ok(());
    }
    outline(module, &found, |_, _| {}, moved_gather)
}

/// The helper for one gather of [`gather_offsets`]: it takes the gather's run-time operands.
fn moved_gather(module: &mut Module, f: &Function, r: &Read) -> Result<Function> {
    let mut gather = f.expressions[r.user].clone();
    let Expression::ImageSample { image, .. } = gather else {
        unreachable!("a gather");
    };
    let Expression::GlobalVariable(var) = f.expressions[image] else {
        return Err(ConvertError::Unsupported(
            crate::refusal::SAMPLED_THROUGH_ARGUMENT,
        ));
    };
    let scalar = match module.types[module.global_variables[var].ty].inner {
        TypeInner::Image {
            dim: ImageDimension::D2,
            class: ImageClass::Sampled { kind, .. },
            ..
        } => Scalar {
            kind,
            width: Scalar::U32.width,
        },
        TypeInner::Image {
            dim: ImageDimension::D2,
            class: ImageClass::Depth { .. },
            ..
        } => Scalar::F32,
        _ => return Err(ConvertError::Unsupported(crate::refusal::DYNAMIC_OFFSET)),
    };
    let result = crate::variant::vector(module, VectorSize::Quad as u8, scalar);
    let mut h = Function {
        arguments: r
            .passed
            .iter()
            .map(|&(_, ty)| FunctionArgument {
                name: None,
                ty,
                binding: None,
            })
            .collect(),
        result: Some(FunctionResult {
            ty: result,
            binding: None,
        }),
        ..Default::default()
    };
    let args: Vec<_> = (0..r.passed.len() as u32)
        .map(|i| {
            h.expressions
                .append(Expression::FunctionArgument(i), Span::UNDEFINED)
        })
        .collect();
    for o in operands(&mut gather)? {
        *o = match r.passed.iter().position(|p| p.0 == *o) {
            Some(i) => args[i],
            None => copy_constant(f, &mut h, *o),
        };
    }
    let image_here = copy_constant(f, &mut h, image);
    *image_of(&mut gather).expect("a gather") = image_here;
    let Expression::ImageSample {
        coordinate,
        offset: Some(offset),
        ..
    } = gather
    else {
        unreachable!("a gather with an offset");
    };
    let mut add = |e: Expression| h.expressions.append(e, Span::UNDEFINED);
    let shift = add(Expression::Literal(Literal::U32(
        u32::BITS - GATHER_OFFSET_BITS,
    )));
    let level = add(Expression::Literal(Literal::U32(0)));
    let first = add(Expression::Splat {
        size: VectorSize::Bi,
        value: shift,
    });
    let signed = add(Expression::As {
        expr: offset,
        kind: ScalarKind::Sint,
        convert: None,
    });
    let up = add(Expression::Binary {
        op: BinaryOperator::ShiftLeft,
        left: signed,
        right: first,
    });
    let low = add(Expression::Binary {
        op: BinaryOperator::ShiftRight,
        left: up,
        right: first,
    });
    let texels = add(Expression::As {
        expr: low,
        kind: ScalarKind::Float,
        convert: Some(Scalar::F32.width),
    });
    let size = add(Expression::ImageQuery {
        image: image_here,
        query: ImageQuery::Size { level: Some(level) },
    });
    let size = add(Expression::As {
        expr: size,
        kind: ScalarKind::Float,
        convert: Some(Scalar::F32.width),
    });
    let moved = add(Expression::Binary {
        op: BinaryOperator::Divide,
        left: texels,
        right: size,
    });
    let moved = add(Expression::Binary {
        op: BinaryOperator::Add,
        left: coordinate,
        right: moved,
    });
    if let Expression::ImageSample {
        coordinate, offset, ..
    } = &mut gather
    {
        *coordinate = moved;
        *offset = None;
    }
    let value = add(gather);
    h.body.push(
        Statement::Emit(Range::new_from_bounds(first, value)),
        Span::UNDEFINED,
    );
    h.body
        .push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
    Ok(h)
}

/// Gives each storage texture whose slot `storage_formats` or `texels_through_buffer` names the
/// bound view's format, or its route through a buffer, as the Direct3D routes do; one it does not
/// name keeps the module's format.
pub(crate) fn storage_textures(
    module: &mut Module,
    variant: &VariantKey,
    stage: ShaderStage,
    reflection: &mut Reflection,
) -> Result<()> {
    let named = |slot: u32| {
        variant.storage_formats.iter().any(|(s, _)| *s == slot)
            || u64::checked_shr(variant.texels_through_buffer, slot).is_some_and(|b| b & 1 != 0)
    };
    let globals: Vec<(Handle<GlobalVariable>, Declared)> = module
        .global_variables
        .iter()
        .filter_map(|(h, g)| {
            let binding = g.binding?;
            let storage = matches!(
                module.types[g.ty].inner,
                TypeInner::Image {
                    class: ImageClass::Storage { .. },
                    ..
                }
            );
            let slot = binding
                .binding
                .checked_sub(SLOT_KINDS[SlotKind::Storage as usize].first_binding)?;
            (storage && named(slot)).then(|| (h, Declared::storage_image(binding)))
        })
        .collect();
    finish(module, variant, &globals, &mut [], stage, reflection)
}
