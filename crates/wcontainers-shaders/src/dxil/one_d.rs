//! One-dimensional textures as 2D textures one texel high, as the fronts make them (plan 4.2) and as
//! the DXBC route reads them: WebGPU has no 1D array textures and binds no 2D view where a program
//! declares 1D. Each read takes its coordinate as (x, the row's centre), each load and store as (x,
//! 0), each offset and gradient with a zero second component, and each size its first component.

use super::Read;
use super::arrays::{copy_constant, operands};
use super::images::Declared;
use crate::ConvertError;
use alloc::vec::Vec;
use naga::{
    Expression, Function, FunctionResult, GlobalVariable, Handle, ImageDimension, ImageQuery,
    Literal, Module, Range, SampleLevel, Scalar, Span, Statement, Type, TypeInner, VectorSize,
};

type Result<T> = core::result::Result<T, ConvertError>;

/// Where a 1D read's coordinate falls in the one row: its centre.
const ROW_CENTRE: f32 = 0.5;

/// The 1D textures of `module` (typed buffers, which the reader also holds as 1D images, left out).
fn one_d(module: &Module, declared: &[Declared]) -> Vec<Handle<GlobalVariable>> {
    let buffers: Vec<_> = super::images::globals(module, declared)
        .into_iter()
        .filter(|(_, d)| d.buffer())
        .map(|(g, _)| g)
        .collect();
    module
        .global_variables
        .iter()
        .filter(|&(g, v)| {
            !buffers.contains(&g)
                && matches!(
                    module.types[v.ty].inner,
                    TypeInner::Image {
                        dim: ImageDimension::D1,
                        ..
                    }
                )
        })
        .map(|(g, _)| g)
        .collect()
}

/// The global `e` of `f` names, if it is one of `textures`.
fn texture(
    f: &Function,
    e: Handle<Expression>,
    textures: &[Handle<GlobalVariable>],
) -> Option<Handle<GlobalVariable>> {
    match f.expressions[e] {
        Expression::GlobalVariable(g) if textures.contains(&g) => Some(g),
        _ => None,
    }
}

/// The reads and size queries of `textures` in `f`; refuses other uses but stores.
fn reads(f: &Function, textures: &[Handle<GlobalVariable>]) -> Result<Vec<Handle<Expression>>> {
    let mut found = Vec::new();
    for (h, e) in f.expressions.iter() {
        match *e {
            Expression::ImageSample { image, .. }
            | Expression::ImageLoad { image, .. }
            | Expression::ImageQuery {
                image,
                query: ImageQuery::Size { .. },
            } if texture(f, image, textures).is_some() => found.push(h),
            // The reader's atomic operations index the image by the texel's coordinate.
            Expression::Access { base, .. } | Expression::AccessIndex { base, .. }
                if texture(f, base, textures).is_some() =>
            {
                return Err(ConvertError::Unsupported(crate::refusal::TEXTURE_1D_ATOMIC));
            }
            _ => {}
        }
    }
    Ok(found)
}

/// The type `e` of `f` has, in `module`.
fn type_of(module: &mut Module, f: &Function, e: Handle<Expression>) -> Result<Handle<Type>> {
    let mut typifier = naga::front::Typifier::new();
    let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
    typifier
        .grow(e, &f.expressions, &ctx)
        .map_err(|e| ConvertError::Malformed(alloc::format!("{e}")))?;
    let inner = typifier.get(e, &module.types).clone();
    Ok(module
        .types
        .insert(Type { name: None, inner }, Span::UNDEFINED))
}

/// The scalar of a scalar type.
fn scalar_of(module: &Module, ty: Handle<Type>) -> Result<Scalar> {
    match module.types[ty].inner {
        TypeInner::Scalar(s) => Ok(s),
        _ => Err(ConvertError::Malformed(
            "a 1D texture's coordinate that is not a scalar".into(),
        )),
    }
}

/// `(value, second)` as a two-component vector of `scalar`, appended to `f` and emitted into `body`.
fn pair(
    module: &mut Module,
    f: &mut Function,
    body: &mut Vec<(Statement, Span)>,
    value: Handle<Expression>,
    second: Literal,
) -> Handle<Expression> {
    let ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: VectorSize::Bi,
                scalar: second.scalar(),
            },
        },
        Span::UNDEFINED,
    );
    let second = f
        .expressions
        .append(Expression::Literal(second), Span::UNDEFINED);
    let h = f.expressions.append(
        Expression::Compose {
            ty,
            components: alloc::vec![value, second],
        },
        Span::UNDEFINED,
    );
    body.push((
        Statement::Emit(Range::new_from_bounds(h, h)),
        Span::UNDEFINED,
    ));
    h
}

/// The zero of `scalar`, or the row's centre for a float coordinate.
fn second(scalar: Scalar, centre: bool) -> Literal {
    match (scalar, centre) {
        (Scalar::F32, true) => Literal::F32(ROW_CENTRE),
        (s, _) => Literal::zero(s).expect("a coordinate's scalar"),
    }
}

/// The helper for one read or size query: it takes the read's run-time operands and reads the 2D
/// texture at (x, row).
fn helper(module: &mut Module, f: &Function, r: &Read) -> Result<Function> {
    let result = type_of(module, f, r.user)?;
    let mut operand_types = Vec::new();
    let mut read = f.expressions[r.user].clone();
    for o in operands(&mut read)? {
        operand_types.push((*o, type_of(module, f, *o)?));
    }
    let mut h = Function {
        arguments: r
            .passed
            .iter()
            .map(|&(_, ty)| naga::FunctionArgument {
                name: None,
                ty,
                binding: None,
            })
            .collect(),
        result: Some(FunctionResult {
            ty: result,
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
    for o in operands(&mut read)? {
        *o = match r.passed.iter().position(|p| p.0 == *o) {
            Some(i) => args[i],
            None => copy_constant(f, &mut h, *o),
        };
    }
    let scalar = |module: &Module, e: Handle<Expression>| -> Result<Scalar> {
        let ty = operand_types
            .iter()
            .find(|t| t.0 == e)
            .map(|t| t.1)
            .ok_or(ConvertError::Malformed("an operand without a type".into()))?;
        scalar_of(module, ty)
    };
    let original = f.expressions[r.user].clone();
    let mut body = Vec::new();
    match (&mut read, original) {
        (
            Expression::ImageSample {
                image,
                coordinate,
                offset,
                level,
                ..
            },
            Expression::ImageSample {
                coordinate: c0,
                offset: o0,
                level: l0,
                ..
            },
        ) => {
            *image = copy_constant(f, &mut h, *image);
            let s = scalar(module, c0)?;
            *coordinate = pair(module, &mut h, &mut body, *coordinate, second(s, true));
            if let (Some(o), Some(o0)) = (offset.as_mut(), o0) {
                let s = scalar(module, o0)?;
                *o = pair(module, &mut h, &mut body, *o, second(s, false));
            }
            if let (SampleLevel::Gradient { x, y }, SampleLevel::Gradient { x: x0, .. }) =
                (level, l0)
            {
                let s = scalar(module, x0)?;
                *x = pair(module, &mut h, &mut body, *x, second(s, false));
                *y = pair(module, &mut h, &mut body, *y, second(s, false));
            }
        }
        (
            Expression::ImageLoad {
                image, coordinate, ..
            },
            Expression::ImageLoad { coordinate: c0, .. },
        ) => {
            *image = copy_constant(f, &mut h, *image);
            let s = scalar(module, c0)?;
            *coordinate = pair(module, &mut h, &mut body, *coordinate, second(s, false));
        }
        (Expression::ImageQuery { image, .. }, _) => *image = copy_constant(f, &mut h, *image),
        _ => unreachable!("only reads and size queries are outlined"),
    }
    let size = matches!(read, Expression::ImageQuery { .. });
    let mut value = h.expressions.append(read, Span::UNDEFINED);
    body.push((
        Statement::Emit(Range::new_from_bounds(value, value)),
        Span::UNDEFINED,
    ));
    if size {
        // The 2D size's width.
        value = h.expressions.append(
            Expression::AccessIndex {
                base: value,
                index: 0,
            },
            Span::UNDEFINED,
        );
        body.push((
            Statement::Emit(Range::new_from_bounds(value, value)),
            Span::UNDEFINED,
        ));
    }
    for (s, span) in body {
        h.body.push(s, span);
    }
    h.body
        .push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
    Ok(h)
}

/// Makes every 1D texture of `module` a 2D texture one texel high, with its reads, size queries and
/// stores taking the second coordinate. Atomic operations on one are refused by name.
pub(super) fn as_two_d(module: &mut Module, declared: &[Declared]) -> Result<()> {
    let textures = one_d(module, declared);
    if textures.is_empty() {
        return Ok(());
    }
    let mut found = Vec::new();
    for (i, f) in super::all_functions(module).iter().enumerate() {
        found.extend(reads(f, &textures)?.into_iter().map(|h| (i, h)));
    }
    // Each store's coordinate type, by function, before anything changes.
    let mut stores = Vec::new();
    for f in super::all_functions(module) {
        let mut here = Vec::new();
        let mut coordinates = Vec::new();
        super::each_block(&mut f.body.clone(), &mut |block| {
            for s in block.iter() {
                if let Statement::ImageStore {
                    image, coordinate, ..
                } = *s
                    && texture(&f, image, &textures).is_some()
                {
                    coordinates.push(coordinate);
                }
            }
        });
        for c in coordinates {
            let ty = type_of(module, &f, c)?;
            here.push((c, scalar_of(module, ty)?));
        }
        stores.push(here);
    }
    super::outline(module, &found, |_, _| {}, helper)?;
    for &g in &textures {
        let ty = module.global_variables[g].ty;
        let TypeInner::Image { arrayed, class, .. } = module.types[ty].inner else {
            unreachable!("a 1D texture is an image")
        };
        module.global_variables[g].ty = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Image {
                    dim: ImageDimension::D2,
                    arrayed,
                    class,
                },
            },
            Span::UNDEFINED,
        );
    }
    // `outline` put its helpers first; the module's own functions follow in their order.
    let first = module.functions.len() - (stores.len() - module.entry_points.len());
    let handles: Vec<_> = module
        .functions
        .iter()
        .skip(first)
        .map(|(h, _)| h)
        .collect();
    for (i, here) in stores.into_iter().enumerate() {
        if here.is_empty() {
            continue;
        }
        let f = match handles.get(i) {
            Some(&h) => &mut module.functions[h],
            None => &mut module.entry_points[i - handles.len()].function,
        };
        let types = &mut module.types;
        let expressions = &mut f.expressions;
        super::each_block(&mut f.body, &mut |block| {
            for (mut s, span) in core::mem::take(block).span_into_iter() {
                if let Statement::ImageStore { coordinate, .. } = &mut s
                    && let Some(&(_, scalar)) = here.iter().find(|c| c.0 == *coordinate)
                {
                    let ty = types.insert(
                        Type {
                            name: None,
                            inner: TypeInner::Vector {
                                size: VectorSize::Bi,
                                scalar,
                            },
                        },
                        Span::UNDEFINED,
                    );
                    let zero = expressions
                        .append(Expression::Literal(second(scalar, false)), Span::UNDEFINED);
                    let h = expressions.append(
                        Expression::Compose {
                            ty,
                            components: alloc::vec![*coordinate, zero],
                        },
                        Span::UNDEFINED,
                    );
                    block.push(Statement::Emit(Range::new_from_bounds(h, h)), span);
                    *coordinate = h;
                }
                block.push(s, span);
            }
        });
    }
    Ok(())
}
