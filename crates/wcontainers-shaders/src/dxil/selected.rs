//! A DXIL pixel shader reading the layer or viewport its primitive was sent to, which naga's reader
//! lacks as inputs (and WGSL too): each is made a private variable before the reader takes the
//! SPIR-V, and the entry point fills it as the DXBC route reads them: the layer a geometry stage's
//! pass-through passes (`VariantFlags::LAYER_FROM_STAGE`), else the draw's run's layer or viewport
//! from the helper constants.

use super::staged::{self, Ids};
use super::words;
use crate::ConvertError;
use crate::container::Element;
use crate::ir::{Basic, interp, scalar};
use crate::lower::{self, Placed};
use alloc::vec::Vec;
use naga::{
    Binding, Expression, FunctionArgument, Interpolation, Module, Scalar, ScalarKind, Span,
    Statement,
};
use spirv::{BuiltIn, ExecutionModel, Op, StorageClass};
use wcontainers_gpu::{
    HelperValue, Reflection, ShaderStage, SystemValue, VariantFlags, VariantKey,
};

/// The prefix of the names given to the private variables.
const VALUE_NAME: &str = "ps_selected";
/// `OpTypePointer`'s pointee, after its result and storage class.
const POINTER_POINTEE: usize = 3;
/// The argument taking the layer a geometry stage's pass-through passes.
const LAYER_ARGUMENT: &str = "named_layer";
/// `OpVariable`'s result type.
const VARIABLE_TYPE: usize = 1;

/// Why a pixel shader's selection input is malformed.
const NO_POINTER: &str = "a stage variable without its pointer type";

/// A layer or viewport input the SPIR-V had.
pub(crate) struct Selected {
    system: SystemValue,
    helper: HelperValue,
}

fn selection(builtin: u32) -> Option<Selected> {
    Some(match BuiltIn::from_u32(builtin)? {
        BuiltIn::Layer => Selected {
            system: SystemValue::RenderTargetArrayIndex,
            helper: HelperValue::Layer,
        },
        BuiltIn::ViewportIndex => Selected {
            system: SystemValue::ViewportArrayIndex,
            helper: HelperValue::Viewport,
        },
        _ => return None,
    })
}

/// Makes a Fragment entry point's layer and viewport inputs private and named, and returns them in
/// that order.
pub(crate) fn prepare(spirv: &mut Vec<u32>) -> Result<Vec<Selected>, ConvertError> {
    let Some(entry) = staged::entry(spirv, ExecutionModel::Fragment) else {
        return Ok(Vec::new());
    };
    let inputs: Vec<(u32, Selected)> = entry
        .values
        .iter()
        .filter(|v| v.side == crate::lower::geometry::Side::Input)
        .filter_map(|v| Some((v.id, selection(v.builtin?)?)))
        .collect();
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let staged = |id: u32| inputs.iter().any(|i| i.0 == id);
    // Each variable's pointer type, and a private pointer to the same type for it.
    let mut ids = Ids::of(spirv);
    let mut types = Vec::new();
    let mut at = words::HEADER;
    while let Some(i) = staged::instruction(spirv, at) {
        if Op::from_u32(staged::opcode(i)) == Some(Op::Variable)
            && staged(i[words::RESULT_AFTER_TYPE])
        {
            types.push((i[words::RESULT_AFTER_TYPE], i[VARIABLE_TYPE]));
        }
        at += i.len();
    }
    let mut out = Vec::from(&spirv[..words::HEADER]);
    let mut privates: Vec<(u32, u32)> = Vec::new();
    let mut named = false;
    let mut at = words::HEADER;
    while let Some(i) = staged::instruction(spirv, at) {
        at += i.len();
        let op = Op::from_u32(staged::opcode(i));
        if !named && !matches!(op, Some(o) if staged::preamble(o)) {
            named = true;
            for (k, (id, _)) in inputs.iter().enumerate() {
                staged::name(&mut out, *id, &staged::value_name(VALUE_NAME, k));
            }
        }
        match op {
            Some(Op::EntryPoint) => out.extend(staged::without_interface(i, &staged)),
            Some(Op::Name) if staged(i[words::RESULT]) => {}
            Some(Op::Decorate) if staged(i[words::decorate::TARGET]) => {}
            Some(Op::TypePointer) => {
                out.extend_from_slice(i);
                let pointer = i[words::RESULT];
                if types.iter().any(|t| t.1 == pointer) {
                    let private = ids.next();
                    out.extend([
                        staged::header(Op::TypePointer, i.len()),
                        private,
                        StorageClass::Private as u32,
                        i[POINTER_POINTEE],
                    ]);
                    privates.push((pointer, private));
                }
            }
            Some(Op::Variable) if staged(i[words::RESULT_AFTER_TYPE]) => {
                let mut i = Vec::from(i);
                i[VARIABLE_TYPE] = privates
                    .iter()
                    .find(|p| p.0 == i[VARIABLE_TYPE])
                    .ok_or_else(|| ConvertError::Malformed(NO_POINTER.into()))?
                    .1;
                i[words::VARIABLE_STORAGE_CLASS] = StorageClass::Private as u32;
                out.extend(i);
            }
            _ => out.extend_from_slice(i),
        }
    }
    out[words::BOUND] = ids.bound();
    *spirv = out;
    Ok(inputs.into_iter().map(|(_, s)| s).collect())
}

/// Fills the private variables [`prepare`] made at the start of the entry point, and reflects them
/// as the inputs they were, named by `signature`.
pub(crate) fn finish(
    module: &mut Module,
    selected: &[Selected],
    variant: &VariantKey,
    reflection: &mut Reflection,
    signature: &[Element],
) -> Result<(), ConvertError> {
    let u32_ty = crate::build::scalar(module, Scalar::U32);
    let mut values = Vec::new();
    for (k, s) in selected.iter().enumerate() {
        let var = staged::value_variable(module, VALUE_NAME, k)?;
        let from_stage = s.system == SystemValue::RenderTargetArrayIndex
            && variant.flags.contains(VariantFlags::LAYER_FROM_STAGE);
        let helper = (!from_stage).then(|| {
            crate::variant::helper_once(module, reflection, ShaderStage::Fragment, s.helper, u32_ty)
        });
        let kind = match module.types[module.global_variables[var].ty].inner {
            naga::TypeInner::Scalar(s) => s.kind,
            _ => ScalarKind::Uint,
        };
        values.push((var, helper, kind));
        let place = Placed {
            location: None,
            component: 0,
            member: Basic {
                scalar: scalar::U32,
                size: 1,
            },
            modes: interp::FLAT,
            system: Some(s.system),
            stream: 0,
        };
        reflection
            .inputs
            .push(lower::stage_value(Default::default(), 0, &place, false));
    }
    crate::container::place_system_values(&mut reflection.inputs, signature);
    let ep = module
        .entry_points
        .first_mut()
        .ok_or(ConvertError::Malformed(staged::NO_MAIN.into()))?;
    let f = &mut ep.function;
    let mut body = Vec::new();
    for (var, helper, kind) in values {
        let (value, first) = match helper {
            None => {
                f.arguments.push(FunctionArgument {
                    name: Some(LAYER_ARGUMENT.into()),
                    ty: u32_ty,
                    binding: Some(Binding::Location {
                        location: crate::stage::named_layer_location(variant),
                        interpolation: Some(Interpolation::Flat),
                        sampling: None,
                        blend_src: None,
                        per_primitive: false,
                    }),
                });
                let index = f.arguments.len() as u32 - 1;
                let value = f
                    .expressions
                    .append(Expression::FunctionArgument(index), Span::UNDEFINED);
                (value, f.expressions.len())
            }
            Some((global, member)) => {
                let base = f
                    .expressions
                    .append(Expression::GlobalVariable(global), Span::UNDEFINED);
                let first = f.expressions.len();
                let pointer = f.expressions.append(
                    Expression::AccessIndex {
                        base,
                        index: member,
                    },
                    Span::UNDEFINED,
                );
                let value = f
                    .expressions
                    .append(Expression::Load { pointer }, Span::UNDEFINED);
                (value, first)
            }
        };
        let value = match kind {
            ScalarKind::Uint => value,
            kind => f.expressions.append(
                Expression::As {
                    expr: value,
                    kind,
                    convert: None,
                },
                Span::UNDEFINED,
            ),
        };
        if f.expressions.len() > first {
            body.push(Statement::Emit(f.expressions.range_from(first)));
        }
        let pointer = f
            .expressions
            .append(Expression::GlobalVariable(var), Span::UNDEFINED);
        body.push(Statement::Store { pointer, value });
    }
    f.body.splice(..0, naga::Block::from_vec(body));
    Ok(())
}
