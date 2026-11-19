//! Components of a constant buffer picked at run time. dxil-spirv reads them through a second view
//! of the buffer, an array of scalars (or pairs) packed 4 bytes apart; WebGPU's uniform arrays step
//! by 16 bytes, and WGSL takes one variable per binding. Each read of that view becomes a call of a
//! function reading the same bytes through the buffer's own view of 16-byte rows; the second view
//! goes.

use super::{Placed, functions_first, place_calls};
use crate::ConvertError;
use crate::build::{VEC4 as ROW_COMPONENTS, WORD};
use alloc::vec::Vec;
use naga::proc::Alignment;
use naga::{
    AddressSpace, BinaryOperator, Expression, Function, FunctionArgument, FunctionResult,
    GlobalVariable, Handle, Literal, Module, Range, Scalar, ScalarKind, Span, Statement, Type,
    TypeInner, VectorSize,
};

type Result<T> = core::result::Result<T, ConvertError>;

fn refused() -> ConvertError {
    ConvertError::Unsupported("ConstantComponentIndexing")
}

/// The array a constant buffer's view holds: its element type and stride.
fn array_of(module: &Module, v: &GlobalVariable) -> Option<(Handle<Type>, u32)> {
    let TypeInner::Struct { ref members, .. } = module.types[v.ty].inner else {
        return None;
    };
    match members[..] {
        [ref m] => match module.types[m.ty].inner {
            TypeInner::Array { base, stride, .. } => Some((base, stride)),
            _ => None,
        },
        _ => None,
    }
}

/// The element a read picks: at run time, or by a constant.
#[derive(Clone, Copy)]
enum Index {
    Run(Handle<Expression>),
    Constant(u32),
}

/// A view of words, and the view of rows at its binding.
struct View {
    words: Handle<GlobalVariable>,
    rows: Handle<GlobalVariable>,
    /// The element read: its scalar and its count of components.
    scalar: Scalar,
    components: u32,
    /// The scalar of the rows' components.
    row_scalar: Scalar,
    ty: Handle<Type>,
}

/// The function reading element `index` of `view` through its rows.
fn reader(module: &mut Module, view: &View) -> Function {
    let u32_ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(Scalar::U32),
        },
        Span::UNDEFINED,
    );
    let mut f = Function {
        arguments: alloc::vec![FunctionArgument {
            name: None,
            ty: u32_ty,
            binding: None,
        }],
        result: Some(FunctionResult {
            ty: view.ty,
            binding: None,
        }),
        ..Default::default()
    };
    let span = Span::UNDEFINED;
    let index = f.expressions.append(Expression::FunctionArgument(0), span);
    let literal = |f: &mut Function, n: u32| {
        f.expressions
            .append(Expression::Literal(Literal::U32(n)), span)
    };
    let words_per = literal(&mut f, view.components);
    let per_row = literal(&mut f, ROW_COMPONENTS);
    let rows = f
        .expressions
        .append(Expression::GlobalVariable(view.rows), span);
    let first = f.expressions.append(
        Expression::Binary {
            op: BinaryOperator::Multiply,
            left: index,
            right: words_per,
        },
        span,
    );
    let row = f.expressions.append(
        Expression::Binary {
            op: BinaryOperator::Divide,
            left: first,
            right: per_row,
        },
        span,
    );
    let column = f.expressions.append(
        Expression::Binary {
            op: BinaryOperator::Modulo,
            left: first,
            right: per_row,
        },
        span,
    );
    let array = f.expressions.append(
        Expression::AccessIndex {
            base: rows,
            index: 0,
        },
        span,
    );
    let pointer = f.expressions.append(
        Expression::Access {
            base: array,
            index: row,
        },
        span,
    );
    let value = f.expressions.append(Expression::Load { pointer }, span);
    let start = first;
    let mut components = Vec::new();
    for k in 0..view.components {
        let at = if k == 0 {
            column
        } else {
            let k = literal(&mut f, k);
            f.expressions.append(
                Expression::Binary {
                    op: BinaryOperator::Add,
                    left: column,
                    right: k,
                },
                span,
            )
        };
        let mut c = f.expressions.append(
            Expression::Access {
                base: value,
                index: at,
            },
            span,
        );
        if view.scalar.kind != view.row_scalar.kind {
            c = f.expressions.append(
                Expression::As {
                    expr: c,
                    kind: view.scalar.kind,
                    convert: None,
                },
                span,
            );
        }
        components.push(c);
    }
    let result = match components[..] {
        [one] => one,
        _ => f.expressions.append(
            Expression::Compose {
                ty: view.ty,
                components,
            },
            span,
        ),
    };
    // Literals are not emitted; every other expression from the first product on is.
    let emitted: Vec<Handle<Expression>> = f
        .expressions
        .iter()
        .filter(|(h, e)| *h >= start && !matches!(e, Expression::Literal(_)))
        .map(|(h, _)| h)
        .collect();
    for h in emitted {
        f.body
            .push(Statement::Emit(Range::new_from_bounds(h, h)), span);
    }
    f.body.push(
        Statement::Return {
            value: Some(result),
        },
        span,
    );
    f
}

/// Rewrites every read of a constant buffer's view of words (above). A view used otherwise than by
/// such reads, or of elements that straddle rows, stays refused.
pub(super) fn rows(module: &mut Module) -> Result<()> {
    let uniform = |(h, v): (Handle<GlobalVariable>, &GlobalVariable)| {
        (v.space == AddressSpace::Uniform).then_some(h)
    };
    let uniforms: Vec<_> = module.global_variables.iter().filter_map(uniform).collect();
    let mut views = Vec::new();
    for &words in &uniforms {
        let v = &module.global_variables[words];
        let Some((element, stride)) = array_of(module, v) else {
            continue;
        };
        if Alignment::MIN_UNIFORM.is_aligned(stride) {
            continue;
        }
        let (scalar, components) = match module.types[element].inner {
            TypeInner::Scalar(s) => (s, 1),
            TypeInner::Vector { size, scalar } => (scalar, size as u32),
            _ => return Err(refused()),
        };
        // An element must lie inside one row.
        if scalar.width as u32 != WORD
            || stride != components * WORD
            || !ROW_COMPONENTS.is_multiple_of(components)
        {
            return Err(refused());
        }
        let rows = uniforms.iter().copied().find(|&r| {
            let other = &module.global_variables[r];
            r != words
                && other.binding == v.binding
                && array_of(module, other).is_some_and(|(row, row_stride)| {
                    Alignment::MIN_UNIFORM.is_aligned(row_stride)
                        && matches!(module.types[row].inner,
                            TypeInner::Vector { size: VectorSize::Quad, scalar } if scalar.width as u32 == WORD)
                })
        });
        let Some(rows) = rows else {
            return Err(refused());
        };
        let row_scalar =
            match module.types[array_of(module, &module.global_variables[rows]).unwrap().0].inner {
                TypeInner::Vector { scalar, .. } => scalar,
                _ => unreachable!("checked above"),
            };
        views.push(View {
            words,
            rows,
            scalar,
            components,
            row_scalar,
            ty: element,
        });
    }
    if views.is_empty() {
        return Ok(());
    }

    // Each read: the function (index into functions, then entry points), the load, the element's
    // index (an expression, or a constant), and the view.
    let n = module.functions.len();
    let all: Vec<&Function> = module
        .functions
        .iter()
        .map(|(_, f)| f)
        .chain(module.entry_points.iter().map(|e| &e.function))
        .collect();
    let mut reads: Vec<(usize, Handle<Expression>, Index, usize)> = Vec::new();
    for (i, f) in all.iter().enumerate() {
        let view_of = |g: Handle<Expression>| match f.expressions[g] {
            Expression::GlobalVariable(v) => views.iter().position(|w| w.words == v),
            _ => None,
        };
        // Every use of a view must be a read of one element: member 0, an element, a load.
        let mut chain = 0;
        for (h, e) in f.expressions.iter() {
            match *e {
                Expression::AccessIndex { base, index } if view_of(base).is_some() => {
                    if index != 0 {
                        return Err(refused());
                    }
                    chain += 1;
                }
                Expression::Load { pointer } => {
                    let (base, index) = match f.expressions[pointer] {
                        Expression::Access { base, index } => (base, Index::Run(index)),
                        Expression::AccessIndex { base, index } => (base, Index::Constant(index)),
                        _ => continue,
                    };
                    let Expression::AccessIndex { base: g, .. } = f.expressions[base] else {
                        continue;
                    };
                    if let Some(v) = view_of(g) {
                        reads.push((i, h, index, v));
                    }
                }
                _ => {}
            }
        }
        let members = f
            .expressions
            .iter()
            .filter(|(_, e)| {
                matches!(**e, Expression::Access { base, .. } | Expression::AccessIndex { base, .. }
                    if matches!(f.expressions[base], Expression::AccessIndex { base: g, .. } if view_of(g).is_some()))
            })
            .count();
        let read_here = reads.iter().filter(|r| r.0 == i).count();
        if chain > 0 && members != read_here {
            return Err(refused());
        }
    }
    let built: Vec<Function> = (0..views.len())
        .map(|v| reader(module, &views[v]))
        .collect();
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
        let mut calls = Vec::new();
        for &(_, load, index, v) in reads.iter().filter(|r| r.0 == i) {
            let (argument, pre) = match index {
                Index::Run(index) => {
                    let a = f.expressions.append(
                        Expression::As {
                            expr: index,
                            kind: ScalarKind::Uint,
                            convert: None,
                        },
                        Span::UNDEFINED,
                    );
                    (a, Some(Range::new_from_bounds(a, a)))
                }
                Index::Constant(k) => (
                    f.expressions
                        .append(Expression::Literal(Literal::U32(k)), Span::UNDEFINED),
                    None,
                ),
            };
            // The view's pointer, no longer loaded, points at the first row instead.
            if let Expression::Load { pointer } = f.expressions[load]
                && let Expression::Access { base, .. } | Expression::AccessIndex { base, .. } =
                    f.expressions[pointer]
            {
                *f.expressions.get_mut(pointer) = Expression::AccessIndex { base, index: 0 };
            }
            calls.push(Placed {
                result: load,
                function: helpers[v],
                arguments: alloc::vec![argument],
                pre,
            });
        }
        // The views of words, read no more, become their rows'.
        for (_, e) in f.expressions.iter_mut() {
            if let Expression::GlobalVariable(g) = e
                && let Some(view) = views.iter().find(|w| w.words == *g)
            {
                *g = view.rows;
            }
        }
        place_calls(f, &calls);
    }
    naga::compact::compact(module, naga::compact::KeepUnused::No);
    Ok(())
}
