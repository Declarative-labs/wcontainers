//! Direct3D's level-of-detail query (`CalculateLevelOfDetail`, `lod`) and per-sample clamp (`Sample`
//! with a clamp, `sample_cl`), which WGSL and GLSL ES lack: library functions written in WGSL
//! (`lod.wgsl`) compute the level and the gradients that make a sample take the clamp, from the
//! coordinate's derivatives and the sampler's state in the helper block.

use alloc::format;
use alloc::vec::Vec;
use naga::{
    Block, DerivativeAxis, DerivativeControl, Expression, Function, Handle, ImageQuery, Literal,
    Module, Scalar, ScalarKind, Span, Statement, SwizzleComponent, VectorSize,
};
use wcontainers_gpu::{HelperValue, Reflection, ShaderStage, VariantKey};

use super::{Result, push};
use crate::ConvertError;

const LIBRARY: &str = include_str!("lod.wgsl");
const LEVEL_OF_DETAIL: &str = "level_of_detail";
const CLAMPED_GRADIENTS: &str = "clamped_gradients";

/// The library's axes: a value of fewer components is padded with zeros.
const AXES: u8 = VectorSize::Tri as u8;

/// The library's function `name`, its types added to `module` (no library function calls another).
fn import(module: &mut Module, name: &str) -> Result<Function> {
    let parsed = naga::front::wgsl::parse_str(LIBRARY).map_err(|e| {
        ConvertError::Malformed(format!("the level-of-detail library does not parse: {e}"))
    })?;
    let f = parsed
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(name))
        .map(|(h, _)| h)
        .ok_or_else(|| ConvertError::Malformed(format!("the library has no {name}")))?;
    let mut library = super::filter::Library::default();
    library.import(module, &parsed, f)?;
    let mut made = library.ahead.into_inner().into_iter();
    match (made.next(), made.next()) {
        (Some(g), None) => Ok(g),
        _ => Err(ConvertError::Malformed(format!(
            "{name} was not imported alone"
        ))),
    }
}

/// The library's `level_of_detail`, its types added to `module`.
pub(crate) fn imported(module: &mut Module) -> Result<Function> {
    import(module, LEVEL_OF_DETAIL)
}

/// The library's `clamped_gradients`, its types added to `module`.
pub(crate) fn imported_gradients(module: &mut Module) -> Result<Function> {
    import(module, CLAMPED_GRADIENTS)
}

/// The library's function `name` in `module`, imported on first use (it calls no other function, so
/// it may go before any caller made after it).
fn appended(module: &mut Module, name: &str) -> Result<Handle<Function>> {
    if let Some((h, _)) = module
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(name))
    {
        return Ok(h);
    }
    let g = import(module, name)?;
    Ok(module.functions.append(g, Span::UNDEFINED))
}

/// The library's `level_of_detail` in `module`, as `appended`.
pub(crate) fn function(module: &mut Module) -> Result<Handle<Function>> {
    appended(module, LEVEL_OF_DETAIL)
}

/// The library's `clamped_gradients` in `module`, as `appended`.
pub(crate) fn gradients_function(module: &mut Module) -> Result<Handle<Function>> {
    appended(module, CLAMPED_GRADIENTS)
}

/// One query or clamped sample in a pixel program: image `image` (an expression of `f` naming the
/// texture's global, of `dimensions` axes, 2 or 3) of texture slot `texture`, sampler slot `sampler`, at
/// `coordinate` (a float or a vector of `coordinate_size` floats, the layer left out), with the sample's
/// own gradients in place of the coordinate's derivatives where it has them.
pub(crate) struct Query {
    pub(crate) image: Handle<Expression>,
    pub(crate) dimensions: u8,
    pub(crate) texture: u32,
    pub(crate) sampler: u32,
    pub(crate) coordinate: Handle<Expression>,
    pub(crate) coordinate_size: u8,
    pub(crate) gradients: Option<[Handle<Expression>; 2]>,
}

/// What both library functions take first: the texture's size and the two gradients, padded to the
/// library's axes, and the sampler's state rows and bias from the helper block.
struct Inputs {
    size: Handle<Expression>,
    gradients: [Handle<Expression>; 2],
    state: Vec<Handle<Expression>>,
    bias: Handle<Expression>,
}

fn inputs(
    module: &mut Module,
    reflection: &mut Reflection,
    f: &mut Function,
    body: &mut Block,
    q: &Query,
) -> Inputs {
    let stage = ShaderStage::Fragment;
    let three = super::vector(module, AXES, Scalar::F32);
    let size = push(
        f,
        body,
        Expression::ImageQuery {
            image: q.image,
            query: ImageQuery::Size { level: None },
        },
    );
    let size = push(
        f,
        body,
        Expression::As {
            expr: size,
            kind: ScalarKind::Float,
            convert: Some(Scalar::F32.width),
        },
    );
    let gradients = q.gradients.unwrap_or_else(|| {
        [DerivativeAxis::X, DerivativeAxis::Y].map(|axis| {
            push(
                f,
                body,
                Expression::Derivative {
                    axis,
                    ctrl: DerivativeControl::None,
                    expr: q.coordinate,
                },
            )
        })
    });
    let mut pad = |v, n: u8| {
        if n >= AXES {
            return v;
        }
        let mut components = alloc::vec![v];
        for _ in n..AXES {
            components.push(push(f, body, Expression::Literal(Literal::F32(0.0))));
        }
        push(
            f,
            body,
            Expression::Compose {
                ty: three,
                components,
            },
        )
    };
    let size = pad(size, q.dimensions);
    let gradients = gradients.map(|g| pad(g, q.coordinate_size));
    let state = super::sampler_state(module, reflection, stage, q.sampler);
    let f32_ty = super::scalar(module, Scalar::F32);
    let bias = super::helper_once(
        module,
        reflection,
        stage,
        HelperValue::LodBias { sampler: q.sampler },
        f32_ty,
    );
    let state = load(f, body, state);
    let state = (0..super::SAMPLER_STATE_ROWS)
        .map(|index| push(f, body, Expression::AccessIndex { base: state, index }))
        .collect();
    let bias = load(f, body, bias);
    Inputs {
        size,
        gradients,
        state,
        bias,
    }
}

/// The value of a helper block's member.
fn load(
    f: &mut Function,
    body: &mut Block,
    (var, index): (Handle<naga::GlobalVariable>, u32),
) -> Handle<Expression> {
    let block = push(f, body, Expression::GlobalVariable(var));
    let pointer = push(f, body, Expression::AccessIndex { base: block, index });
    push(f, body, Expression::Load { pointer })
}

/// Pushes into `body` `function`'s call with `arguments`, its result `result`, made where `None`.
fn called(
    f: &mut Function,
    body: &mut Block,
    function: Handle<Function>,
    arguments: Vec<Handle<Expression>>,
    result: Option<Handle<Expression>>,
) -> Handle<Expression> {
    let result = match result {
        Some(r) => {
            f.expressions[r] = Expression::CallResult(function);
            r
        }
        None => push(f, body, Expression::CallResult(function)),
    };
    body.push(
        Statement::Call {
            function,
            arguments,
            result: Some(result),
        },
        Span::UNDEFINED,
    );
    result
}

/// Pushes into `body` the call of the library's `level_of_detail` (which `function` gives) giving
/// `q`'s (clamped, unclamped) pair, from the sampler's state and bias and, for a texture in
/// `min_lod_clamped`, its clamp in the helper block; the call's result is `result`, made where `None`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn call(
    module: &mut Module,
    reflection: &mut Reflection,
    variant: &VariantKey,
    function: impl FnOnce(&mut Module) -> Result<Handle<Function>>,
    f: &mut Function,
    body: &mut Block,
    q: &Query,
    result: Option<Handle<Expression>>,
) -> Result<Handle<Expression>> {
    let stage = ShaderStage::Fragment;
    let levels = push(
        f,
        body,
        Expression::ImageQuery {
            image: q.image,
            query: ImageQuery::NumLevels,
        },
    );
    let i = inputs(module, reflection, f, body, q);
    let clamped = q.texture < u128::BITS && variant.min_lod_clamped & (1 << q.texture) != 0;
    let min_lod = match clamped {
        true => {
            let pair = super::vector(module, VectorSize::Bi as u8, Scalar::U32);
            let at = super::helper_once(
                module,
                reflection,
                stage,
                HelperValue::MinLod { texture: q.texture },
                pair,
            );
            let pair = load(f, body, at);
            let bits = push(
                f,
                body,
                Expression::AccessIndex {
                    base: pair,
                    index: 0,
                },
            );
            push(
                f,
                body,
                Expression::As {
                    expr: bits,
                    kind: ScalarKind::Float,
                    convert: None,
                },
            )
        }
        false => push(f, body, Expression::Literal(Literal::F32(0.0))),
    };
    let function = function(module)?;
    let [dx, dy] = i.gradients;
    let mut arguments = alloc::vec![i.size, dx, dy, levels];
    arguments.extend(i.state);
    arguments.extend([i.bias, min_lod]);
    Ok(called(f, body, function, arguments, result))
}

/// Pushes into `body` the call of the library's `clamped_gradients` (which `function` gives) for a
/// sample of `q` with Direct3D's per-sample clamp `clamp` and its own bias `shift` (none: zero),
/// giving the gradients the sample takes as two columns of the library's axes; the call's result is
/// `result`, made where `None`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn clamped(
    module: &mut Module,
    reflection: &mut Reflection,
    function: impl FnOnce(&mut Module) -> Result<Handle<Function>>,
    f: &mut Function,
    body: &mut Block,
    q: &Query,
    shift: Option<Handle<Expression>>,
    clamp: Handle<Expression>,
    result: Option<Handle<Expression>>,
) -> Result<Handle<Expression>> {
    let i = inputs(module, reflection, f, body, q);
    let shift = shift.unwrap_or_else(|| push(f, body, Expression::Literal(Literal::F32(0.0))));
    let function = function(module)?;
    let [dx, dy] = i.gradients;
    let arguments = alloc::vec![i.size, dx, dy, i.state[1], i.bias, shift, clamp];
    Ok(called(f, body, function, arguments, result))
}

/// The two columns of `clamped`'s result `gradients`, each cut to `size` components (a float for one).
pub(crate) fn columns(
    f: &mut Function,
    body: &mut Block,
    gradients: Handle<Expression>,
    size: u8,
) -> [Handle<Expression>; 2] {
    [0, 1].map(|index| {
        let column = push(
            f,
            body,
            Expression::AccessIndex {
                base: gradients,
                index,
            },
        );
        if size >= AXES {
            column
        } else if size == VectorSize::Bi as u8 {
            push(
                f,
                body,
                Expression::Swizzle {
                    size: VectorSize::Bi,
                    vector: column,
                    pattern: SwizzleComponent::XYZW,
                },
            )
        } else {
            push(
                f,
                body,
                Expression::AccessIndex {
                    base: column,
                    index: 0,
                },
            )
        }
    })
}
