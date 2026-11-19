//! Color targets written in the program (`VariantKey::targets_in_shader`): an output of another type
//! than its target's format is written as the format's type with its bits kept, as Direct3D writes it,
//! and widened with zeros to the format's components, which WebGPU and GLSL ES require of an output.

use naga::ScalarKind;
use wcontainers_gpu::{FormatKind, ScalarKind as ValueKind, SystemValue, TargetInShader};

use super::pull::{Kind, layout};
use super::texel::vertex_format;
use super::*;

/// The scalar target `t` takes from the program. Normalised formats are refused by name, as the program
/// encodes into them only where the front stores them as integers (`NormAsInteger`), and so is
/// combining with the target's old value (`BlendInShader`).
fn target_scalar(t: &TargetInShader) -> Result<Scalar> {
    if t.combine {
        return Err(ConvertError::Unsupported(crate::refusal::BLEND_IN_SHADER));
    }
    let format = t.format;
    match format.kind() {
        FormatKind::Uint => Ok(Scalar::U32),
        FormatKind::Sint => Ok(Scalar::I32),
        FormatKind::Float => match vertex_format(format).map(layout) {
            Some((Kind::Float, _, _)) => Ok(Scalar::F32),
            _ => Err(ConvertError::Unsupported(crate::refusal::NORM_AS_INTEGER)),
        },
        _ => Err(ConvertError::Unsupported(crate::refusal::TARGETS_IN_SHADER)),
    }
}

/// What target `t`'s output of type `ty` becomes: its scalar, its component count and the output's own
/// count; `None` where it stays as it is.
fn retyped(
    module: &Module,
    ty: Handle<Type>,
    t: &TargetInShader,
) -> Result<Option<(Scalar, u32, u32)>> {
    let to = target_scalar(t)?;
    let (own_count, own_scalar) = match module.types[ty].inner {
        TypeInner::Scalar(s) => (1, s),
        TypeInner::Vector { size, scalar } => (size as u32, scalar),
        _ => return Err(ConvertError::Unsupported(crate::refusal::TARGETS_IN_SHADER)),
    };
    let format_count = u32::BITS - t.format.components().leading_zeros();
    let count = own_count.max(format_count);
    Ok((own_scalar != to || own_count != count).then_some((to, count, own_count)))
}

fn at_target(b: &Binding, target: u8) -> bool {
    matches!(b, Binding::Location { location, .. } if *location == u32::from(target))
}

/// Whether any of `targets` changes the entry point's outputs (and so needs it wrapped).
pub(super) fn needed(module: &Module, targets: &[TargetInShader]) -> Result<bool> {
    let Some(result) = &module.entry_points[0].function.result else {
        return Ok(false);
    };
    let outputs: Vec<(Binding, Handle<Type>)> =
        match (&result.binding, &module.types[result.ty].inner) {
            (Some(b), _) => alloc::vec![(b.clone(), result.ty)],
            (None, TypeInner::Struct { members, .. }) => members
                .iter()
                .filter_map(|m| Some((m.binding.clone()?, m.ty)))
                .collect(),
            _ => Vec::new(),
        };
    let mut needed = false;
    for t in targets {
        // Refused by name even where the program writes no such target.
        target_scalar(t)?;
        for (b, ty) in &outputs {
            needed |= at_target(b, t.target) && retyped(module, *ty, t)?.is_some();
        }
    }
    Ok(needed)
}

pub(super) fn targets_in_shader(
    module: &mut Module,
    w: &mut Wrapped,
    targets: &[TargetInShader],
    reflection: &mut Reflection,
) -> Result<()> {
    for t in targets {
        let Some(at) = w.output(|b| at_target(b, t.target)) else {
            continue;
        };
        let Some((to, count, own_count)) = retyped(module, w.outputs[at].ty, t)? else {
            continue;
        };
        let (f, body) = (&mut w.f, &mut w.body);
        let mut value = w.outputs[at].value;
        if module.types[w.outputs[at].ty].inner.scalar() != Some(to) {
            value = push(
                f,
                body,
                Expression::As {
                    expr: value,
                    kind: to.kind,
                    convert: None,
                },
            );
        }
        let ty = if count == 1 {
            scalar(module, to)
        } else {
            vector(module, count as u8, to)
        };
        if count != own_count {
            let mut components: Vec<_> = match own_count {
                1 => alloc::vec![value],
                n => (0..n).map(|i| index(f, body, value, i)).collect(),
            };
            let zero_ty = scalar(module, to);
            let zero = f
                .expressions
                .append(Expression::ZeroValue(zero_ty), Span::UNDEFINED);
            components.resize(count as usize, zero);
            value = push(f, body, Expression::Compose { ty, components });
        }
        w.outputs[at].ty = ty;
        w.outputs[at].value = value;
        w.changed = true;
        let kind = match to.kind {
            ScalarKind::Uint => ValueKind::Uint,
            ScalarKind::Sint => ValueKind::Sint,
            _ => ValueKind::Float,
        };
        for o in reflection
            .outputs
            .iter_mut()
            .filter(|o| o.system == Some(SystemValue::Target(t.target)))
        {
            o.scalar = kind;
        }
    }
    Ok(())
}
