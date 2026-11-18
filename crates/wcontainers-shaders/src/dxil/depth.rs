//! Depth textures read plainly. WebGPU compares only against depth textures, so the reader makes
//! every texture a program compares against one (a shadow map is often also sampled or loaded
//! plainly). A plain read of a depth texture gives one value where the program expects four, so
//! each becomes a call of a function giving (depth, 0, 0, 1), as Direct3D reads a depth view.

use super::arrays::{constant, copy_constant, operands};
use super::{Placed, functions_first, place_calls};
use crate::ConvertError;
use alloc::vec::Vec;
use naga::{
    Expression, Function, FunctionArgument, FunctionResult, Handle, ImageClass, Literal, Module,
    Range, SampleLevel, Scalar, Span, Statement, SwizzleComponent, Type, TypeInner, VectorSize,
};

type Result<T> = core::result::Result<T, ConvertError>;

/// One plain read: the function (index into functions, then entry points), the reading
/// expression, and the operands passed to its helper with their types.
struct Read {
    function: usize,
    user: Handle<Expression>,
    passed: Vec<(Handle<Expression>, Handle<Type>)>,
}

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

/// Makes every plain sample and load of a depth texture give four components.
pub(super) fn widen(module: &mut Module) -> Result<()> {
    let n = module.functions.len();
    let all = |module: &Module| -> Vec<Function> {
        module
            .functions
            .iter()
            .map(|(_, f)| f.clone())
            .chain(module.entry_points.iter().map(|ep| ep.function.clone()))
            .collect()
    };
    // The plain reads, by function.
    let mut found = Vec::new();
    for (i, f) in all(module).iter().enumerate() {
        for (h, e) in f.expressions.iter() {
            match *e {
                Expression::ImageSample {
                    image,
                    depth_ref: None,
                    gather,
                    level,
                    ..
                } if depth(module, f, image) => match (gather, level) {
                    (Some(SwizzleComponent::X), _) => continue,
                    (Some(_), _) => {
                        return Err(ConvertError::Unsupported("DepthGatherComponent"));
                    }
                    (None, SampleLevel::Bias(_)) => {
                        return Err(ConvertError::Unsupported("DepthSampleBias"));
                    }
                    (None, SampleLevel::Gradient { .. }) => {
                        return Err(ConvertError::Unsupported("DepthSampleGradient"));
                    }
                    (None, SampleLevel::Exact(l)) if whole_level(module, f, l).is_none() => {
                        return Err(ConvertError::Unsupported("DepthSampleLevel"));
                    }
                    _ => {}
                },
                Expression::ImageLoad { image, .. } if depth(module, f, image) => {}
                _ => continue,
            }
            found.push((i, h));
        }
    }
    if found.is_empty() {
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
    // The operands' types, each function typed with its plain reads standing as the four
    // components they will give.
    let mut reads = Vec::new();
    for (i, mut f) in all(module).into_iter().enumerate() {
        let here: Vec<_> = found.iter().filter(|r| r.0 == i).map(|r| r.1).collect();
        if here.is_empty() {
            continue;
        }
        let original = f.expressions.clone();
        for &h in &here {
            *f.expressions.get_mut(h) = Expression::ZeroValue(vec4);
        }
        let mut typifier = naga::front::Typifier::new();
        let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
        let mut types = Vec::new();
        for &h in &here {
            let mut user = original[h].clone();
            let mut passed = Vec::new();
            for o in operands(&mut user)? {
                let o = *o;
                if constant(&f, o) || matches!(f.expressions[o], Expression::GlobalVariable(_)) {
                    continue;
                }
                typifier
                    .grow(o, &f.expressions, &ctx)
                    .map_err(|e| ConvertError::Malformed(alloc::format!("{e}")))?;
                passed.push((o, typifier.get(o, &module.types).clone()));
            }
            types.push((h, passed));
        }
        reads.push((i, types));
    }
    let mut ty = |inner| {
        module
            .types
            .insert(Type { name: None, inner }, Span::UNDEFINED)
    };
    let reads: Vec<Read> = reads
        .into_iter()
        .flat_map(|(function, types)| {
            types
                .into_iter()
                .map(move |(user, passed)| (function, user, passed))
        })
        .map(|(function, user, passed)| Read {
            function,
            user,
            passed: passed.into_iter().map(|(e, t)| (e, ty(t))).collect(),
        })
        .collect();
    let mut built = Vec::new();
    for r in &reads {
        let f = match r.function < n {
            true => {
                module
                    .functions
                    .iter()
                    .nth(r.function)
                    .expect("a function")
                    .1
            }
            false => &module.entry_points[r.function - n].function,
        };
        built.push(helper(module, f, r, vec4)?);
    }
    let (helpers, moved) = functions_first(module, |module| -> Vec<Handle<Function>> {
        built
            .into_iter()
            .map(|h| module.functions.append(h, Span::UNDEFINED))
            .collect()
    });
    for i in 0..n + module.entry_points.len() {
        let f = match moved.get(i) {
            Some(&h) => &mut module.functions[h],
            None => &mut module.entry_points[i - n].function,
        };
        let calls: Vec<Placed> = reads
            .iter()
            .zip(&helpers)
            .filter(|(r, _)| r.function == i)
            .map(|(r, &helper)| Placed {
                result: r.user,
                function: helper,
                arguments: r.passed.iter().map(|p| p.0).collect(),
                pre: None,
            })
            .collect();
        place_calls(f, &calls);
    }
    Ok(())
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
