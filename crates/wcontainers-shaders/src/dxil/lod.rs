//! Level-of-detail queries (`CalculateLevelOfDetail` and its unclamped form). The reader reads
//! `OpImageQueryLod` as a call of an `IMAGE_QUERY_LOD` function, one per signature; each such function
//! becomes the level-of-detail library's (in its place, before every caller), and each call one of it
//! as the DXBC route makes it (`variant::lod`).

use super::each_block;
use crate::ConvertError;
use crate::variant::lod::{self, Query};
use alloc::vec::Vec;
use naga::front::spv::IMAGE_QUERY_LOD;
use naga::{
    Block, Expression, Function, GlobalVariable, Handle, ImageDimension, Module, Statement,
    TypeInner, VectorSize,
};
use wcontainers_gpu::{Reflection, ShaderStage, VariantKey};

type Result<T> = core::result::Result<T, ConvertError>;

/// What a malformed query names.
const COORDINATE: &str = "a query's coordinate";
const CALL: &str = "a call";
const IMAGE: &str = "an image";

/// Replaces every query of `module`, a program of `stage` whose resources' slots are `slots`.
pub(super) fn queries(
    module: &mut Module,
    stage: ShaderStage,
    variant: &VariantKey,
    slots: &[(Handle<GlobalVariable>, u32)],
    reflection: &mut Reflection,
) -> Result<()> {
    // Each reader's function with its coordinate's size.
    let mut stand_ins = Vec::new();
    for (h, f) in module.functions.iter() {
        if f.name.as_deref() != Some(IMAGE_QUERY_LOD) {
            continue;
        }
        let size = match f.arguments.get(2).map(|a| &module.types[a.ty].inner) {
            Some(TypeInner::Scalar(_)) => 1,
            Some(&TypeInner::Vector { size, .. }) => size as u8,
            _ => return Err(malformed(COORDINATE)),
        };
        stand_ins.push((h, size));
    }
    if stand_ins.is_empty() {
        return Ok(());
    }
    if stage != ShaderStage::Fragment {
        return Err(ConvertError::Unsupported(
            crate::refusal::CALCULATE_LEVEL_OF_DETAIL,
        ));
    }
    for &(h, _) in &stand_ins {
        module.functions[h] = lod::imported(module)?;
    }
    let callers: Vec<_> = module
        .functions
        .iter()
        .map(|(h, _)| h)
        .filter(|h| !stand_ins.iter().any(|(s, _)| s == h))
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

fn malformed(what: &str) -> ConvertError {
    ConvertError::Malformed(alloc::format!(
        "{what} the level-of-detail query cannot take"
    ))
}

/// Replaces the calls of `stand_ins` in `f`, a function taken out of `module`.
fn replace(
    module: &mut Module,
    f: &mut Function,
    stand_ins: &[(Handle<Function>, u8)],
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
            let Some(&(_, coordinate_size)) = stand_ins.iter().find(|(s, _)| *s == function) else {
                i += 1;
                continue;
            };
            let arguments = arguments.clone();
            let mut made = Block::new();
            match query(module, f, &arguments, coordinate_size, slots).and_then(|q| {
                lod::call(
                    module,
                    reflection,
                    variant,
                    |_| Ok(function),
                    f,
                    &mut made,
                    &q,
                    result,
                )
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

/// What a call's arguments (the image, the sampler, the coordinate) query.
fn query(
    module: &Module,
    f: &Function,
    arguments: &[Handle<Expression>],
    coordinate_size: u8,
    slots: &[(Handle<GlobalVariable>, u32)],
) -> Result<Query> {
    let &[image, sampler, coordinate] = arguments else {
        return Err(malformed(CALL));
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
        TypeInner::Image { dim, .. } => match dim {
            ImageDimension::D1 => 1,
            ImageDimension::D2 => VectorSize::Bi as u8,
            ImageDimension::D3 => VectorSize::Tri as u8,
            ImageDimension::Cube => {
                return Err(ConvertError::Unsupported(
                    crate::refusal::CALCULATE_LEVEL_OF_DETAIL_CUBE,
                ));
            }
        },
        _ => return Err(malformed(IMAGE)),
    };
    Ok(Query {
        image,
        dimensions,
        texture,
        sampler,
        coordinate,
        coordinate_size,
    })
}
