//! Textures bound through a view of another component kind (float, unsigned, signed) than the program
//! declares, the view's format in `VariantKey::texel_formats`: the slot's texture is declared in the
//! view's kind and each load reads the view's value by its bits as the declared kind, as Direct3D
//! does. Samples and gathers of such a slot are refused by name.

use alloc::vec::Vec;
use naga::{
    Expression, GlobalVariable, Handle, ImageClass, Module, ScalarKind, Span, Type, TypeInner,
};
use wcontainers_gpu::{Format, FormatKind, Reflection, SlotKind, VariantKey};

use super::{Result, global_of, owners, slot_of};
use crate::{ConvertError, refusal};

/// A texture slot read in its view's kind: the slot, its variable, and the view's kind.
#[derive(Clone, Copy)]
pub(super) struct Viewed {
    pub(super) slot: u32,
    pub(super) global: Handle<GlobalVariable>,
    pub(super) kind: ScalarKind,
}

/// The scalar kind a texture view of `format` reads as.
fn view_kind(format: Format) -> ScalarKind {
    match format.kind() {
        FormatKind::Uint => ScalarKind::Uint,
        FormatKind::Sint => ScalarKind::Sint,
        _ => ScalarKind::Float,
    }
}

/// The texture slots whose bound view's kind differs from the program's declaration (typed buffers,
/// whose routes declare the view's kind themselves, and slots in `decoded` are not among them).
/// Refuses by name a sample or gather of one.
pub(super) fn viewed(
    module: &Module,
    variant: &VariantKey,
    reflection: &Reflection,
) -> Result<Vec<Viewed>> {
    let mut out = Vec::new();
    if variant.texel_formats.is_empty() {
        return Ok(out);
    }
    for (global, g) in module.global_variables.iter() {
        let TypeInner::Image {
            class: ImageClass::Sampled { kind: declared, .. },
            ..
        } = module.types[g.ty].inner
        else {
            continue;
        };
        let Some(slot) = slot_of(module, reflection, global, SlotKind::Texture) else {
            continue;
        };
        if variant.decoded.iter().any(|(s, _)| *s == slot) {
            continue;
        }
        let Some(&(_, format)) = variant.texel_formats.iter().find(|(s, _)| *s == slot) else {
            continue;
        };
        let kind = view_kind(format);
        if kind != declared {
            out.push(Viewed { slot, global, kind });
        }
    }
    for (_, f) in owners(module) {
        for (_, e) in f.expressions.iter() {
            if let Expression::ImageSample { image, .. } = *e
                && let Some(g) = global_of(f, image)
                && out.iter().any(|v| v.global == g)
            {
                return Err(ConvertError::Unsupported(refusal::SAMPLED_VIEW_KIND));
            }
        }
    }
    Ok(out)
}

/// The view's kind of `global`, where it is read in its view's kind.
pub(super) fn kind_of(viewed: &[Viewed], global: Handle<GlobalVariable>) -> Option<ScalarKind> {
    viewed.iter().find(|v| v.global == global).map(|v| v.kind)
}

/// Declares each slot's texture in its view's kind, once every read of it is rewritten.
pub(super) fn retype(module: &mut Module, viewed: &[Viewed]) {
    for v in viewed {
        let TypeInner::Image {
            dim,
            arrayed,
            class: ImageClass::Sampled { multi, .. },
        } = module.types[module.global_variables[v.global].ty].inner
        else {
            continue;
        };
        module.global_variables[v.global].ty = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Image {
                    dim,
                    arrayed,
                    class: ImageClass::Sampled {
                        kind: v.kind,
                        multi,
                    },
                },
            },
            Span::UNDEFINED,
        );
    }
}
