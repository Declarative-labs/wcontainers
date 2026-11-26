//! Textures a front stores as the integer format of the same bits (`VariantKey::decoded`,
//! `SlowPath::NormAsInteger`): the slot's texture is declared as that integer format, each load
//! decodes the texel as Direct3D converts the guest's format, and samples and gathers filter in the
//! program from decoded loads (`filter.rs`).

use alloc::vec::Vec;
use naga::{
    Block, Expression, Function, ImageClass, Module, ScalarKind, Span, Statement, Type, TypeInner,
};
use wcontainers_gpu::{
    format::Format, reflect::Reflection, reflect::SlotKind, state::VertexFormat,
};

use super::pull::{Kind, decode, layout};
use super::texel::vertex_format;
use super::{Result, f32_lit, index, push, slot_of};
use crate::build::{self, Expr};
use crate::{ConvertError, refusal};

/// The guest format's kind (unorm or snorm) and each component's bits, red first.
pub(super) fn decoding(format: Format) -> Result<(Kind, Vec<u32>)> {
    let layout = match vertex_format(format) {
        // No integer format holds BGRA's bytes in red's place.
        Some(VertexFormat::Bgra8Unorm | VertexFormat::Bgrx8Unorm) | None => None,
        Some(v) => Some(layout(v)),
    };
    match layout {
        Some((kind @ (Kind::Unorm | Kind::Snorm), components, _)) => {
            Ok((kind, components.iter().map(|&(_, _, bits)| bits).collect()))
        }
        _ => Err(ConvertError::Unsupported(refusal::DECODED_FORMAT)),
    }
}

/// The scalar kind of the integer format a front stores `format` as: signed for signed normalised.
pub(super) fn stored_kind(format: Format) -> Result<ScalarKind> {
    Ok(match decoding(format)?.0 {
        Kind::Snorm => ScalarKind::Sint,
        _ => ScalarKind::Uint,
    })
}

/// The texel `raw`, a four-component load of the integer format, as four f32 components decoded
/// from the guest's `format`, the components it lacks their defaults (zero, and one in alpha).
pub(super) fn texel(
    f: &mut Function,
    body: &mut Block,
    raw: Expr,
    format: Format,
    vec4f: naga::Handle<Type>,
) -> Result<Expr> {
    let (kind, bits) = decoding(format)?;
    let raw = match kind {
        Kind::Snorm => push(
            f,
            body,
            Expression::As {
                expr: raw,
                kind: ScalarKind::Uint,
                convert: None,
            },
        ),
        _ => raw,
    };
    let mut components = Vec::new();
    for i in 0..build::VEC4 {
        components.push(match bits.get(i as usize) {
            Some(&b) => {
                let c = index(f, body, raw, i);
                decode(f, body, kind, c, b)
            }
            None => f32_lit(f, if i == build::ALPHA { 1.0 } else { 0.0 }),
        });
    }
    Ok(push(
        f,
        body,
        Expression::Compose {
            ty: vec4f,
            components,
        },
    ))
}

/// A function of one argument, a four-component load of the integer format, returning
/// [`texel`]'s value of type `vec4f`: the body given to the filter library's decoding function.
pub(super) fn texel_function(
    stub: &Function,
    vec4f: naga::Handle<Type>,
    format: Format,
) -> Result<Function> {
    let mut f = Function {
        name: stub.name.clone(),
        arguments: stub.arguments.clone(),
        result: stub.result.clone(),
        ..Default::default()
    };
    let mut body = Block::new();
    let raw = f
        .expressions
        .append(Expression::FunctionArgument(0), Span::UNDEFINED);
    let value = texel(&mut f, &mut body, raw, format, vec4f)?;
    body.push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
    f.body = body;
    Ok(f)
}

/// Declares each decoded slot's texture as its integer format, once every read of it is rewritten.
/// Only single-sampled float textures are taken: a front stores no other this way.
pub(super) fn retype(
    module: &mut Module,
    decoded: &[(u32, Format)],
    reflection: &Reflection,
) -> Result<()> {
    let globals: Vec<_> = module.global_variables.iter().map(|(h, _)| h).collect();
    for g in globals {
        let Some(slot) = slot_of(module, reflection, g, SlotKind::Texture) else {
            continue;
        };
        let Some(&(_, format)) = decoded.iter().find(|(s, _)| *s == slot) else {
            continue;
        };
        let kind = stored_kind(format)?;
        let TypeInner::Image {
            dim,
            arrayed,
            class:
                ImageClass::Sampled {
                    kind: ScalarKind::Float,
                    multi: false,
                },
        } = module.types[module.global_variables[g].ty].inner
        else {
            return Err(ConvertError::Unsupported(refusal::DECODED_TEXTURE));
        };
        module.global_variables[g].ty = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Image {
                    dim,
                    arrayed,
                    class: ImageClass::Sampled { kind, multi: false },
                },
            },
            Span::UNDEFINED,
        );
    }
    Ok(())
}

/// The decoded slot `texture` names, with the guest's format.
pub(super) fn format_of(decoded: &[(u32, Format)], texture: Option<u32>) -> Option<Format> {
    let texture = texture?;
    decoded.iter().find(|(s, _)| *s == texture).map(|&(_, f)| f)
}
