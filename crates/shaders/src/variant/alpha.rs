//! Storage views of `A8Unorm` (`VariantKey::storage_formats`), which the back ends hold as
//! `rgba8unorm` with the value in alpha and the colour channels zero, so the program declares
//! `rgba8unorm`: each store writes its value's alpha alone, as Direct3D stores the fourth component
//! into the format's one channel, and keeps the colour channels zero, so each load reads
//! `(0, 0, 0, a)`, as Direct3D's does.

use alloc::vec::Vec;
use naga::{
    Expression, GlobalVariable, Handle, ImageClass, Literal, Module, Scalar, Span, Statement, Type,
    TypeInner, VectorSize,
};
use wcontainers_gpu::{
    format::Format, reflect::Reflection, reflect::SlotKind, reflect::VariantKey,
};

use super::{Result, each_block, each_function, global_of, push, slot_of};
use crate::{ConvertError, refusal};

/// Makes every store into an `A8Unorm` storage view write `(0, 0, 0, a)` of its value. Refuses by
/// name a store through an image passed as an argument (`TextureArgument`) where such views exist.
pub(super) fn alpha_only(
    module: &mut Module,
    variant: &VariantKey,
    reflection: &Reflection,
) -> Result<()> {
    let alpha_slot = |slot: u32| {
        variant
            .storage_formats
            .iter()
            .any(|&(s, f)| s == slot && f == Format::A8Unorm)
    };
    if !variant
        .storage_formats
        .iter()
        .any(|&(_, f)| f == Format::A8Unorm)
    {
        return Ok(());
    }
    let alpha: Vec<Handle<GlobalVariable>> = module
        .global_variables
        .iter()
        .filter(|&(g, v)| {
            let ty = match module.types[v.ty].inner {
                TypeInner::BindingArray { base, .. } => base,
                _ => v.ty,
            };
            matches!(
                module.types[ty].inner,
                TypeInner::Image {
                    class: ImageClass::Storage { .. },
                    ..
                }
            ) && slot_of(module, reflection, g, SlotKind::Storage).is_some_and(alpha_slot)
        })
        .map(|(g, _)| g)
        .collect();
    if alpha.is_empty() {
        return Ok(());
    }
    let vec4 = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: VectorSize::Quad,
                scalar: Scalar::F32,
            },
        },
        Span::UNDEFINED,
    );
    let mut result = Ok(());
    each_function(module, |f| {
        let mut body = core::mem::take(&mut f.body);
        each_block(&mut body, &mut |block| {
            for (s, span) in core::mem::take(block).span_into_iter() {
                let Statement::ImageStore {
                    image,
                    coordinate,
                    array_index,
                    value,
                } = s
                else {
                    block.push(s, span);
                    continue;
                };
                let value = match global_of(f, image) {
                    Some(g) if alpha.contains(&g) => {
                        let zero = push(f, block, Expression::Literal(Literal::F32(0.0)));
                        let a = push(
                            f,
                            block,
                            Expression::AccessIndex {
                                base: value,
                                index: crate::build::ALPHA,
                            },
                        );
                        push(
                            f,
                            block,
                            Expression::Compose {
                                ty: vec4,
                                components: alloc::vec![zero, zero, zero, a],
                            },
                        )
                    }
                    Some(_) => value,
                    None => {
                        result = Err(ConvertError::Unsupported(refusal::TEXTURE_ARGUMENT));
                        value
                    }
                };
                block.push(
                    Statement::ImageStore {
                        image,
                        coordinate,
                        array_index,
                        value,
                    },
                    span,
                );
            }
        });
        f.body = body;
    });
    result
}
