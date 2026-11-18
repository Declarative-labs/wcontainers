//! Infinite and NaN constants. naga's reader makes them literals, which WGSL cannot write and Tint
//! refuses to evaluate; each becomes the result of a call, at the start of the function using it,
//! of a function reading the float's bits from a local variable (as the DXBC route reads them).
//! A private table starting at such values (a `static const` array) is stored when the entry point
//! starts.

use super::functions_first;
use crate::ConvertError;
use alloc::vec::Vec;
use naga::{
    AddressSpace, Arena, Constant, Expression, Function, FunctionResult, GlobalVariable, Handle,
    Literal, LocalVariable, Module, Range, Scalar, ScalarKind, Span, Statement, Type, TypeInner,
};

type Result<T> = core::result::Result<T, ConvertError>;

/// Whether global expression `g` is or holds an infinite or NaN float.
fn non_finite(
    exprs: &Arena<Expression>,
    constants: &Arena<Constant>,
    g: Handle<Expression>,
) -> bool {
    match exprs[g] {
        Expression::Literal(Literal::F32(x)) => !x.is_finite(),
        Expression::Literal(Literal::F64(x)) => !x.is_finite(),
        Expression::Constant(c) => non_finite(exprs, constants, constants[c].init),
        Expression::Compose { ref components, .. } => {
            components.iter().any(|&c| non_finite(exprs, constants, c))
        }
        Expression::Splat { value, .. } => non_finite(exprs, constants, value),
        _ => false,
    }
}

/// What a function expression holding a non-finite value stands for: a global expression tree and
/// its type.
fn value_of(
    module: &Module,
    f32_ty: Handle<Type>,
    e: &Expression,
) -> Option<(Value, Handle<Type>)> {
    match *e {
        Expression::Literal(Literal::F32(x)) if !x.is_finite() => {
            Some((Value::Bits(x.to_bits()), f32_ty))
        }
        Expression::Constant(c)
            if non_finite(
                &module.global_expressions,
                &module.constants,
                module.constants[c].init,
            ) =>
        {
            Some((
                Value::Global(module.constants[c].init),
                module.constants[c].ty,
            ))
        }
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Value {
    /// A lone f32 literal's bits.
    Bits(u32),
    Global(Handle<Expression>),
}

/// Copies global expression `g` into `f`, each non-finite float read from a local variable of its
/// bits; every expression made at run time is emitted in order.
fn copy(
    module: &Module,
    f: &mut Function,
    u32_ty: Handle<Type>,
    value: Value,
) -> Result<Handle<Expression>> {
    let mut emit = |f: &mut Function, e: Expression| {
        let h = f.expressions.append(e, Span::UNDEFINED);
        f.body.push(
            Statement::Emit(Range::new_from_bounds(h, h)),
            Span::UNDEFINED,
        );
        h
    };
    let g = match value {
        Value::Bits(bits) => return Ok(bits_of(f, u32_ty, bits, &mut emit)),
        Value::Global(g) => g,
    };
    Ok(match module.global_expressions[g] {
        Expression::Literal(Literal::F32(x)) if !x.is_finite() => {
            bits_of(f, u32_ty, x.to_bits(), &mut emit)
        }
        Expression::Literal(Literal::F64(_)) => {
            return Err(ConvertError::Unsupported("NonFiniteConstant"));
        }
        ref e @ (Expression::Literal(_) | Expression::ZeroValue(_)) => {
            f.expressions.append(e.clone(), Span::UNDEFINED)
        }
        Expression::Constant(c) => {
            copy(module, f, u32_ty, Value::Global(module.constants[c].init))?
        }
        Expression::Compose { ty, ref components } => {
            let components = components
                .iter()
                .map(|&c| copy(module, f, u32_ty, Value::Global(c)))
                .collect::<Result<Vec<_>>>()?;
            emit(f, Expression::Compose { ty, components })
        }
        Expression::Splat { size, value } => {
            let value = copy(module, f, u32_ty, Value::Global(value))?;
            emit(f, Expression::Splat { size, value })
        }
        _ => return Err(ConvertError::Unsupported("NonFiniteConstant")),
    })
}

/// The float of `bits`, loaded from a local variable so that no writer folds it into a literal.
fn bits_of(
    f: &mut Function,
    u32_ty: Handle<Type>,
    bits: u32,
    emit: &mut impl FnMut(&mut Function, Expression) -> Handle<Expression>,
) -> Handle<Expression> {
    let init = f
        .expressions
        .append(Expression::Literal(Literal::U32(bits)), Span::UNDEFINED);
    let var = f.local_variables.append(
        LocalVariable {
            name: None,
            ty: u32_ty,
            init: Some(init),
        },
        Span::UNDEFINED,
    );
    let pointer = f
        .expressions
        .append(Expression::LocalVariable(var), Span::UNDEFINED);
    let load = emit(f, Expression::Load { pointer });
    emit(
        f,
        Expression::As {
            expr: load,
            kind: ScalarKind::Float,
            convert: None,
        },
    )
}

/// Where a non-finite value is used: a function's expression, or the start value of a private
/// global variable, stored at the start of each entry point instead.
#[derive(Clone, Copy)]
enum Use {
    Expression(Handle<Expression>),
    Global(Handle<GlobalVariable>),
}

/// Replaces every non-finite constant the module's functions use by a call (above). A local
/// variable or private global variable starting at such a value is stored after the calls instead.
/// Runs right after reading: it moves the module's functions and compacts it.
pub(super) fn calls(module: &mut Module) -> Result<()> {
    let scalar = |scalar| Type {
        name: None,
        inner: TypeInner::Scalar(scalar),
    };
    let f32_ty = module.types.insert(scalar(Scalar::F32), Span::UNDEFINED);
    let globals: Vec<_> = module
        .global_variables
        .iter()
        .filter_map(|(g, v)| {
            let init = v.init?;
            non_finite(&module.global_expressions, &module.constants, init).then_some((
                Use::Global(g),
                Value::Global(init),
                v.ty,
            ))
        })
        .collect();
    if globals
        .iter()
        .any(|&(u, ..)| matches!(u, Use::Global(g) if module.global_variables[g].space != AddressSpace::Private))
    {
        return Err(ConvertError::Unsupported("NonFiniteConstant"));
    }
    // (function index, or entry point index + functions, and its (use, value, type))
    let mut found = Vec::new();
    let n = module.functions.len();
    let functions = module.functions.iter().map(|(_, f)| f);
    let all = functions.chain(module.entry_points.iter().map(|ep| &ep.function));
    for (i, f) in all.enumerate() {
        let mut values: Vec<_> = f
            .expressions
            .iter()
            .filter_map(|(h, e)| {
                value_of(module, f32_ty, e).map(|(v, ty)| (Use::Expression(h), v, ty))
            })
            .collect();
        if i >= n {
            values.extend(globals.iter().copied());
        }
        if !values.is_empty() {
            found.push((i, values));
        }
    }
    if found.is_empty() {
        return Ok(());
    }
    for (g, ..) in &globals {
        if let Use::Global(g) = *g {
            module.global_variables.get_mut(g).init = None;
        }
    }
    let u32_ty = module.types.insert(scalar(Scalar::U32), Span::UNDEFINED);
    let (helpers, moved) = functions_first(module, |module| -> Result<_> {
        let mut helpers: Vec<(Value, Handle<Function>)> = Vec::new();
        for (_, values) in &found {
            for &(_, value, ty) in values {
                if helpers.iter().any(|(v, _)| *v == value) {
                    continue;
                }
                let mut f = Function {
                    result: Some(FunctionResult { ty, binding: None }),
                    ..Default::default()
                };
                let v = copy(module, &mut f, u32_ty, value)?;
                f.body
                    .push(Statement::Return { value: Some(v) }, Span::UNDEFINED);
                helpers.push((value, module.functions.append(f, Span::UNDEFINED)));
            }
        }
        Ok(helpers)
    });
    let helpers = helpers?;
    for (i, values) in found {
        let f = match moved.get(i) {
            Some(&h) => &mut module.functions[h],
            None => &mut module.entry_points[i - n].function,
        };
        let mut start = naga::Block::new();
        let mut made = Vec::new();
        for &(u, value, _) in &values {
            let function = helpers
                .iter()
                .find(|(v, _)| *v == value)
                .expect("made above")
                .1;
            let result = match u {
                Use::Expression(h) => {
                    *f.expressions.get_mut(h) = Expression::CallResult(function);
                    made.push(h);
                    h
                }
                Use::Global(_) => f
                    .expressions
                    .append(Expression::CallResult(function), Span::UNDEFINED),
            };
            start.push(
                Statement::Call {
                    function,
                    arguments: Vec::new(),
                    result: Some(result),
                },
                Span::UNDEFINED,
            );
            if let Use::Global(g) = u {
                let pointer = f
                    .expressions
                    .append(Expression::GlobalVariable(g), Span::UNDEFINED);
                start.push(
                    Statement::Store {
                        pointer,
                        value: result,
                    },
                    Span::UNDEFINED,
                );
            }
        }
        let starting: Vec<_> = f
            .local_variables
            .iter()
            .filter(|(_, v)| v.init.is_some_and(|i| made.contains(&i)))
            .map(|(var, v)| (var, v.init.expect("filtered")))
            .collect();
        for (var, value) in starting {
            f.local_variables.get_mut(var).init = None;
            let pointer = f
                .expressions
                .append(Expression::LocalVariable(var), Span::UNDEFINED);
            start.push(Statement::Store { pointer, value }, Span::UNDEFINED);
        }
        let body = core::mem::take(&mut f.body);
        start.extend_block(body);
        f.body = start;
    }
    // Drops the constants no function reads now.
    naga::compact::compact(module, naga::compact::KeepUnused::No);
    Ok(())
}
