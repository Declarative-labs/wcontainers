//! Level-of-detail queries (`CalculateLevelOfDetail` and its unclamped form) and samples with a
//! per-sample clamp. The reader reads `OpImageQueryLod` as a call of an `IMAGE_QUERY_LOD` function and
//! a sample's `MinLod` as gradients from a call of an `IMAGE_SAMPLE_CLAMP` function, one per
//! signature; each such function becomes the level-of-detail library's (in its place, before every
//! caller), and each call one of it as the DXBC route makes it (`variant::lod`).

use super::each_block;
use crate::ConvertError;
use crate::variant::lod::{self, Query};
use alloc::vec::Vec;
use naga::front::spv::{IMAGE_QUERY_LOD, IMAGE_SAMPLE_CLAMP};
use naga::{
    Block, Expression, Function, GlobalVariable, Handle, ImageClass, ImageDimension, Module,
    Statement, TypeInner, VectorSize,
};
use wcontainers_gpu::{Reflection, ShaderStage, VariantKey};

type Result<T> = core::result::Result<T, ConvertError>;

/// What a malformed query names.
const COORDINATE: &str = "a query's coordinate";
const CALL: &str = "a call";
const IMAGE: &str = "an image";

/// The arguments every stand-in's call begins with: the image, the sampler and the coordinate.
const LEADING: usize = 3;

/// A reader's stand-in: its function, its coordinate's size, and whether it is a sample's clamp.
#[derive(Clone, Copy)]
struct StandIn {
    function: Handle<Function>,
    coordinate_size: u8,
    clamp: bool,
}

/// Replaces every query and clamped sample of `module`, a program of `stage` whose resources' slots
/// are `slots`.
pub(super) fn queries(
    module: &mut Module,
    stage: ShaderStage,
    variant: &VariantKey,
    slots: &[(Handle<GlobalVariable>, u32)],
    reflection: &mut Reflection,
) -> Result<()> {
    let mut stand_ins = Vec::new();
    for (h, f) in module.functions.iter() {
        let clamp = match f.name.as_deref() {
            Some(IMAGE_QUERY_LOD) => false,
            Some(IMAGE_SAMPLE_CLAMP) => true,
            _ => continue,
        };
        let coordinate_size = match f.arguments.get(2).map(|a| &module.types[a.ty].inner) {
            Some(TypeInner::Scalar(_)) => 1,
            Some(&TypeInner::Vector { size, .. }) => size as u8,
            _ => return Err(malformed(COORDINATE)),
        };
        stand_ins.push(StandIn {
            function: h,
            coordinate_size,
            clamp,
        });
    }
    // Derivatives exist in pixel programs only.
    if let Some(s) = stand_ins.first()
        && stage != ShaderStage::Fragment
    {
        return Err(ConvertError::Unsupported(refusal(s.clamp)));
    }
    for s in &stand_ins {
        module.functions[s.function] = match s.clamp {
            false => lod::imported(module)?,
            true => lod::imported_gradients(module)?,
        };
    }
    let callers: Vec<_> = module
        .functions
        .iter()
        .map(|(h, _)| h)
        .filter(|h| !stand_ins.iter().any(|s| s.function == *h))
        .collect();
    for h in callers {
        let mut f = core::mem::take(&mut module.functions[h]);
        let done = replace(module, &mut f, &stand_ins, variant, slots, reflection);
        module.functions[h] = f;
        done?;
    }
    for i in 0..module.entry_points.len() {
        let mut f = core::mem::take(&mut module.entry_points[i].function);
        let done = replace(module, &mut f, &stand_ins, variant, slots, reflection);
        module.entry_points[i].function = f;
        done?;
    }
    Ok(())
}

/// The name a stand-in's refusal gives.
fn refusal(clamp: bool) -> &'static str {
    match clamp {
        false => crate::refusal::CALCULATE_LEVEL_OF_DETAIL,
        true => crate::refusal::SAMPLE_CLAMP,
    }
}

fn malformed(what: &str) -> ConvertError {
    ConvertError::Malformed(alloc::format!(
        "{what} the level-of-detail query cannot take"
    ))
}

/// Replaces the calls of `stand_ins` in `f`, a function taken out of `module`.
fn replace(
    module: &mut Module,
    f: &mut Function,
    stand_ins: &[StandIn],
    variant: &VariantKey,
    slots: &[(Handle<GlobalVariable>, u32)],
    reflection: &mut Reflection,
) -> Result<()> {
    let mut body = core::mem::take(&mut f.body);
    let mut failed = None;
    each_block(&mut body, &mut |block: &mut Block| {
        let mut i = 0;
        while i < block.len() && failed.is_none() {
            let Statement::Call {
                function,
                ref arguments,
                result,
            } = block[i]
            else {
                i += 1;
                continue;
            };
            let Some(&s) = stand_ins.iter().find(|s| s.function == function) else {
                i += 1;
                continue;
            };
            let arguments = arguments.clone();
            let mut made = Block::new();
            match query(module, f, &arguments, s, slots).and_then(|(q, clamp)| match clamp {
                None => lod::call(
                    module,
                    reflection,
                    variant,
                    |_| Ok(function),
                    f,
                    &mut made,
                    &q,
                    result,
                ),
                Some((shift, clamp)) => lod::clamped(
                    module,
                    reflection,
                    |_| Ok(function),
                    f,
                    &mut made,
                    &q,
                    shift,
                    clamp,
                    result,
                ),
            }) {
                Ok(_) => {
                    let n = made.len();
                    block.splice(i..i + 1, made);
                    i += n;
                }
                Err(e) => failed = Some(e),
            }
        }
    });
    f.body = body;
    failed.map_or(Ok(()), Err)
}

/// A clamped sample's bias, where it has one, and clamp.
type Clamp = Option<(Option<Handle<Expression>>, Handle<Expression>)>;

/// What a call's arguments (the image, the sampler, the coordinate; of a clamped sample then its bias
/// or two gradients if it has them, and the clamp) query, with a clamped sample's bias and clamp.
fn query(
    module: &Module,
    f: &Function,
    arguments: &[Handle<Expression>],
    s: StandIn,
    slots: &[(Handle<GlobalVariable>, u32)],
) -> Result<(Query, Clamp)> {
    let Some((&[image, sampler, coordinate], rest)) = arguments.split_at_checked(LEADING) else {
        return Err(malformed(CALL));
    };
    let (gradients, clamp) = match (s.clamp, rest) {
        (false, []) => (None, None),
        (true, &[clamp]) => (None, Some((None, clamp))),
        (true, &[bias, clamp]) => (None, Some((Some(bias), clamp))),
        (true, &[x, y, clamp]) => (Some([x, y]), Some((None, clamp))),
        _ => return Err(malformed(CALL)),
    };
    let slot = |e: Handle<Expression>, refusal: &'static str| match f.expressions[e] {
        Expression::GlobalVariable(g) => slots
            .iter()
            .find(|(h, _)| *h == g)
            .map(|&(_, s)| (g, s))
            .ok_or(ConvertError::Unsupported(refusal)),
        _ => Err(ConvertError::Unsupported(refusal)),
    };
    let (g, texture) = slot(image, crate::refusal::TEXTURE_ARGUMENT)?;
    let (_, sampler) = slot(sampler, crate::refusal::SAMPLER_ARGUMENT)?;
    let dimensions = match module.types[module.global_variables[g].ty].inner {
        // The clamp is taken through gradients, which a depth texture does not take.
        TypeInner::Image {
            class: ImageClass::Depth { .. },
            ..
        } if s.clamp => return Err(ConvertError::Unsupported(crate::refusal::SAMPLE_CLAMP)),
        TypeInner::Image { dim, .. } => match dim {
            ImageDimension::D1 => 1,
            ImageDimension::D2 => VectorSize::Bi as u8,
            ImageDimension::D3 => VectorSize::Tri as u8,
            ImageDimension::Cube => {
                return Err(ConvertError::Unsupported(match s.clamp {
                    false => crate::refusal::CALCULATE_LEVEL_OF_DETAIL_CUBE,
                    true => crate::refusal::SAMPLE_CLAMP,
                }));
            }
        },
        _ => return Err(malformed(IMAGE)),
    };
    let q = Query {
        image,
        dimensions,
        texture,
        sampler,
        coordinate,
        coordinate_size: s.coordinate_size,
        gradients,
    };
    Ok((q, clamp))
}
