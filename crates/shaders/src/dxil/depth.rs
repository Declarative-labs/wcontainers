//! Depth textures read plainly. WebGPU compares only against depth textures, so the reader makes
//! every texture a program compares against one (a shadow map is often also sampled or loaded
//! plainly). A plain read of a depth texture gives one value where the program expects four, so
//! each becomes a call of a function giving (depth, 0, 0, 1), as Direct3D reads a depth view.

use super::Read;
use super::arrays::{copy_constant, operands};
use crate::ConvertError;
use alloc::vec::Vec;
use naga::{
    Expression, Function, FunctionArgument, FunctionResult, Handle, ImageClass, Literal, Module,
    Range, SampleLevel, Scalar, Span, Statement, SwizzleComponent, Type, TypeInner, VectorSize,
};

type Result<T> = core::result::Result<T, ConvertError>;

/// Whether `image` of `f` names a depth texture of `module`.
fn depth(module: &Module, f: &Function, image: Handle<Expression>) -> bool {
    match f.expressions[image] {
        Expression::GlobalVariable(g) => matches!(
            module.types[module.global_variables[g].ty].inner,
            TypeInner::Image {
                class: ImageClass::Depth { .. },
                ..
            }
        ),
        _ => false,
    }
}

/// A constant whole sample level of `f`, as WGSL takes it for a depth texture.
fn whole_level(module: &Module, f: &Function, e: Handle<Expression>) -> Option<i32> {
    let value = match f.expressions[e] {
        Expression::Constant(c) => &module.global_expressions[module.constants[c].init],
        ref other => other,
    };
    let Expression::Literal(Literal::F32(v)) = *value else {
        return None;
    };
    // Exact for every level a texture can have.
    let whole = v as i32;
    (whole as f32 == v && whole >= 0).then_some(whole)
}

/// The plain samples and loads of depth textures in `f`; refuses those WGSL cannot make.
fn plain_reads(module: &Module, f: &Function) -> Result<Vec<Handle<Expression>>> {
    let mut found = Vec::new();
    for (h, e) in f.expressions.iter() {
        match *e {
            Expression::ImageSample {
                image,
                depth_ref: None,
                gather,
                level,
                ..
            } if depth(module, f, image) => match (gather, level) {
                // Every component's gather is resolved by `variant::swizzle`.
                (Some(_), _) => continue,
                (None, SampleLevel::Bias(_)) => {
                    return Err(ConvertError::Unsupported(crate::refusal::DEPTH_SAMPLE_BIAS));
                }
                (None, SampleLevel::Gradient { .. }) => {
                    return Err(ConvertError::Unsupported(
                        crate::refusal::DEPTH_SAMPLE_GRADIENT,
                    ));
                }
                (None, SampleLevel::Exact(l)) if whole_level(module, f, l).is_none() => {
                    return Err(ConvertError::Unsupported(
                        crate::refusal::DEPTH_SAMPLE_LEVEL,
                    ));
                }
                _ => {}
            },
            Expression::ImageLoad { image, .. } if depth(module, f, image) => {}
            _ => continue,
        }
        found.push(h);
    }
    Ok(found)
}

/// The module's `vec4<f32>`, which a widened read gives.
pub(super) fn vec4(module: &mut Module) -> Handle<Type> {
    module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: VectorSize::Quad,
                scalar: Scalar::F32,
            },
        },
        Span::UNDEFINED,
    )
}

/// `f` with each plain read of a depth texture standing as the four components [`widen`] makes it
/// give (`vec4` from [`vec4`]), for typing `f`'s expressions before `widen` runs.
pub(super) fn as_widened(module: &Module, f: &Function, vec4: Handle<Type>) -> Result<Function> {
    let mut typed = f.clone();
    for h in plain_reads(module, f)? {
        *typed.expressions.get_mut(h) = Expression::ZeroValue(vec4);
    }
    Ok(typed)
}

/// Makes every plain sample and load of a depth texture give four components.
pub(super) fn widen(module: &mut Module) -> Result<()> {
    let mut found = Vec::new();
    for (i, f) in super::all_functions(module).iter().enumerate() {
        found.extend(plain_reads(module, f)?.into_iter().map(|h| (i, h)));
    }
    if found.is_empty() {
        return Ok(());
    }
    let vec4 = vec4(module);
    // Each function's operands are typed with its plain reads standing as the four components they
    // will give.
    super::outline(
        module,
        &found,
        |f, here| {
            for &h in here {
                *f.expressions.get_mut(h) = Expression::ZeroValue(vec4);
            }
        },
        |module, f, r| helper(module, f, r, vec4),
    )
}

/// The helper for one plain read: it takes the read's run-time operands, reads, and gives
/// (value, 0, 0, 1).
fn helper(module: &Module, f: &Function, r: &Read, vec4: Handle<Type>) -> Result<Function> {
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
            ty: vec4,
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
    let mut read = f.expressions[r.user].clone();
    let level = match read {
        Expression::ImageSample {
            level: SampleLevel::Exact(l),
            ..
        } => whole_level(module, f, l),
        _ => None,
    };
    for o in operands(&mut read)? {
        *o = match r.passed.iter().position(|p| p.0 == *o) {
            Some(i) => args[i],
            None => copy_constant(f, &mut h, *o),
        };
    }
    match &mut read {
        Expression::ImageSample {
            image, level: l, ..
        } => {
            *image = copy_constant(f, &mut h, *image);
            if let Some(whole) = level {
                *l = SampleLevel::Exact(
                    h.expressions
                        .append(Expression::Literal(Literal::I32(whole)), Span::UNDEFINED),
                );
            }
        }
        Expression::ImageLoad { image, .. } => *image = copy_constant(f, &mut h, *image),
        _ => unreachable!("only reads are widened"),
    }
    let value = h.expressions.append(read, Span::UNDEFINED);
    h.body.push(
        Statement::Emit(Range::new_from_bounds(value, value)),
        Span::UNDEFINED,
    );
    let zero = h
        .expressions
        .append(Expression::Literal(Literal::F32(0.0)), Span::UNDEFINED);
    let one = h
        .expressions
        .append(Expression::Literal(Literal::F32(1.0)), Span::UNDEFINED);
    let wide = h.expressions.append(
        Expression::Compose {
            ty: vec4,
            components: alloc::vec![value, zero, zero, one],
        },
        Span::UNDEFINED,
    );
    h.body.push(
        Statement::Emit(Range::new_from_bounds(wide, wide)),
        Span::UNDEFINED,
    );
    h.body
        .push(Statement::Return { value: Some(wide) }, Span::UNDEFINED);
    Ok(h)
}

/// Gives each comparison gather the channel the DXIL names, which dxil-spirv drops (its SPIR-V
/// compares the first): `channels` are those of the program's `TextureGatherCmp` calls, as
/// `cpp/dxil.cpp` reads them. Calls naming different channels, or one not constant, cannot be told
/// apart in the SPIR-V and are refused by name; `variant::swizzle` resolves the channel given.
pub(super) fn compared_gathers(module: &mut Module, channels: &[u32]) -> Result<()> {
    let Some(&channel) = channels.first() else {
        return Ok(());
    };
    let component = SwizzleComponent::XYZW
        .get(channel as usize)
        .filter(|_| channels.iter().all(|&c| c == channel))
        .copied()
        .ok_or(ConvertError::Unsupported(
            crate::refusal::DEPTH_GATHER_COMPONENT,
        ))?;
    let functions = module.functions.iter_mut().map(|(_, f)| f);
    for f in functions.chain(module.entry_points.iter_mut().map(|e| &mut e.function)) {
        for (_, e) in f.expressions.iter_mut() {
            if let Expression::ImageSample {
                gather: Some(gather),
                depth_ref: Some(_),
                ..
            } = e
            {
                *gather = component;
            }
        }
    }
    Ok(())
}
