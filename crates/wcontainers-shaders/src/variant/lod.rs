//! Direct3D's level-of-detail query (`CalculateLevelOfDetail`, `lod`), which WGSL and GLSL ES lack: a
//! library function written in WGSL (`lod.wgsl`) computes it from the coordinate's derivatives and the
//! sampler's state in the helper block.

use alloc::format;
use alloc::vec::Vec;
use naga::{
    Block, DerivativeAxis, DerivativeControl, Expression, Function, Handle, ImageQuery, Literal,
    Module, Scalar, ScalarKind, Span, Statement, VectorSize,
};
use wcontainers_gpu::{HelperValue, Reflection, ShaderStage, VariantKey};

use super::{Result, push};
use crate::ConvertError;

const LIBRARY: &str = include_str!("lod.wgsl");
const FUNCTION: &str = "level_of_detail";

/// The library's axes: a value of fewer components is padded with zeros.
const AXES: u8 = VectorSize::Tri as u8;

/// The library's `level_of_detail`, its types added to `module` (it calls no other function).
pub(crate) fn imported(module: &mut Module) -> Result<Function> {
    let parsed = naga::front::wgsl::parse_str(LIBRARY).map_err(|e| {
        ConvertError::Malformed(format!("the level-of-detail library does not parse: {e}"))
    })?;
    let f = parsed
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(FUNCTION))
        .map(|(h, _)| h)
        .ok_or_else(|| ConvertError::Malformed(format!("the library has no {FUNCTION}")))?;
    let mut library = super::filter::Library::default();
    library.import(module, &parsed, f)?;
    let mut made = library.ahead.into_inner().into_iter();
    match (made.next(), made.next()) {
        (Some(g), None) => Ok(g),
        _ => Err(ConvertError::Malformed(format!(
            "{FUNCTION} was not imported alone"
        ))),
    }
}

/// The library's `level_of_detail` in `module`, imported on first use (it calls no other function, so
/// it may go before any caller made after it).
pub(crate) fn function(module: &mut Module) -> Result<Handle<Function>> {
    if let Some((h, _)) = module
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(FUNCTION))
    {
        return Ok(h);
    }
    let g = imported(module)?;
    Ok(module.functions.append(g, Span::UNDEFINED))
}

/// One query in a pixel program: image `image` (an expression of `f` naming the texture's global, of
/// `dimensions` axes, 2 or 3) of texture slot `texture`, sampler slot `sampler`, at `coordinate` (a
/// float or a vector of `coordinate_size` floats, the layer left out).
pub(crate) struct Query {
    pub(crate) image: Handle<Expression>,
    pub(crate) dimensions: u8,
    pub(crate) texture: u32,
    pub(crate) sampler: u32,
    pub(crate) coordinate: Handle<Expression>,
    pub(crate) coordinate_size: u8,
}

/// Pushes into `body` the call of the library's function (which `function` gives) giving `q`'s
/// (clamped, unclamped) pair, from the sampler's state and bias and, for a texture in
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
    let levels = push(
        f,
        body,
        Expression::ImageQuery {
            image: q.image,
            query: ImageQuery::NumLevels,
        },
    );
    let mut axes = Vec::new();
    for axis in [DerivativeAxis::X, DerivativeAxis::Y] {
        axes.push(push(
            f,
            body,
            Expression::Derivative {
                axis,
                ctrl: DerivativeControl::None,
                expr: q.coordinate,
            },
        ));
    }
    let mut padded = Vec::new();
    for (v, n) in [
        (size, q.dimensions),
        (axes[0], q.coordinate_size),
        (axes[1], q.coordinate_size),
    ] {
        padded.push(if n >= AXES {
            v
        } else {
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
        });
    }
    let state = super::sampler_state(module, reflection, stage, q.sampler);
    let f32_ty = super::scalar(module, Scalar::F32);
    let bias = super::helper_once(
        module,
        reflection,
        stage,
        HelperValue::LodBias { sampler: q.sampler },
        f32_ty,
    );
    let clamped = q.texture < u128::BITS && variant.min_lod_clamped & (1 << q.texture) != 0;
    let min_lod = clamped.then(|| {
        let pair = super::vector(module, VectorSize::Bi as u8, Scalar::U32);
        super::helper_once(
            module,
            reflection,
            stage,
            HelperValue::MinLod { texture: q.texture },
            pair,
        )
    });
    let load = |f: &mut Function, body: &mut Block, (var, index): (_, u32)| {
        let block = push(f, body, Expression::GlobalVariable(var));
        let pointer = push(f, body, Expression::AccessIndex { base: block, index });
        push(f, body, Expression::Load { pointer })
    };
    let state = load(f, body, state);
    let mut rows = Vec::new();
    for index in 0..super::SAMPLER_STATE_ROWS {
        rows.push(push(
            f,
            body,
            Expression::AccessIndex { base: state, index },
        ));
    }
    let bias = load(f, body, bias);
    let min_lod = match min_lod {
        Some(at) => {
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
        None => push(f, body, Expression::Literal(Literal::F32(0.0))),
    };
    let function = function(module)?;
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
            arguments: alloc::vec![
                padded[0], padded[1], padded[2], levels, rows[0], rows[1], rows[2], bias, min_lod,
            ],
            result: Some(result),
        },
        Span::UNDEFINED,
    );
    Ok(result)
}
