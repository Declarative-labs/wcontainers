//! Arrays of constant and structured buffers (`ConstantBuffer<T> b[4]`, `StructuredBuffer<T> s[]`),
//! which the reader takes as one buffer holding an array of blocks: WebGPU has no binding arrays, so
//! each element becomes a buffer of its own at its own register, as `arrays` does for textures. A
//! pointer into an element picked by a constant points into that buffer; each load, store, atomic
//! operation and length taken through an element picked at run time is a call of a function that
//! switches on the index and uses the element it names.

use super::arrays::{
    array_length, case_value, constant, constant_index, copy_constant, is_signed, refused,
    typing_copy, unemit,
};
use super::{Placed, Resources, each_block, functions_first, place_calls};
use crate::ConvertError;
use alloc::vec::Vec;
use naga::{
    AddressSpace, AtomicFunction, Block, Expression, Function, FunctionArgument, FunctionResult,
    GlobalVariable, Handle, LocalVariable, Module, Range, ResourceBinding, Scalar, Span, Statement,
    SwitchCase, Type, TypeInner,
};
use wcontainers_gpu::VariantKey;

type Result<T> = core::result::Result<T, ConvertError>;

/// Why a module is malformed: an atomic operation's result is another expression.
const NOT_AN_ATOMIC_RESULT: &str = "an atomic operation's result of another kind";

/// One step from a block's pointer towards the pointer used: a member or element by a constant, or
/// an element at an index expression.
#[derive(Clone, Copy)]
enum Step {
    Constant(u32),
    Index(Handle<Expression>),
}

/// A pointer through an element picked at run time: the array, the index and the steps after it.
#[derive(Clone)]
struct Chain {
    array: usize,
    index: Handle<Expression>,
    steps: Vec<Step>,
}

/// What is done through such a pointer.
#[derive(Clone, Copy)]
enum Operation {
    Load,
    Length,
    Store {
        value: Handle<Expression>,
    },
    Atomic {
        fun: AtomicFunction,
        value: Handle<Expression>,
        result: Option<Handle<Expression>>,
    },
}

/// One use through an element picked at run time, made a call of a helper.
struct Use {
    /// The function (index into functions, then entry points).
    function: usize,
    /// The load or length the call's result stands for; none for a statement.
    user: Option<Handle<Expression>>,
    chain: Chain,
    operation: Operation,
    /// The operands passed (the index first), with their types; the rest are constants copied in.
    passed: Vec<(Handle<Expression>, Handle<Type>)>,
    result: Option<Handle<Type>>,
    signed: bool,
}

/// The pointer a statement stores or operates through.
fn pointer_of(s: &Statement) -> Option<Handle<Expression>> {
    match *s {
        Statement::Store { pointer, .. } | Statement::Atomic { pointer, .. } => Some(pointer),
        _ => None,
    }
}

/// Splits every array of constant or structured buffers into its elements, numbered after the
/// resources `resources` lists (an array declared without a length sized by
/// `VariantKey::register_ranges`), and rewrites the uses. `typed` gives the copy of a function its
/// expressions are typed in. Moves the module's functions and compacts it.
pub(super) fn split(
    module: &mut Module,
    resources: &mut Resources,
    variant: &VariantKey,
    typed: impl Fn(&Module, &Function) -> Result<Function>,
) -> Result<()> {
    let found: Vec<_> = module
        .global_variables
        .iter()
        .filter_map(|(g, v)| {
            let binding = v.binding?;
            if !matches!(
                v.space,
                AddressSpace::Uniform | AddressSpace::Storage { .. }
            ) {
                return None;
            }
            match module.types[v.ty].inner {
                TypeInner::Array { base, size, .. }
                    if matches!(module.types[base].inner, TypeInner::Struct { .. }) =>
                {
                    Some((g, binding, base, size))
                }
                _ => None,
            }
        })
        .collect();
    if found.is_empty() {
        return Ok(());
    }
    let mut arrays: Vec<(Handle<GlobalVariable>, Vec<Handle<GlobalVariable>>)> = Vec::new();
    for (g, binding, base, size) in found {
        let &[class, space, register] = resources
            .get(binding.binding as usize)
            .ok_or(ConvertError::Malformed(super::PAST_THE_LIST.into()))?;
        let n = array_length(variant, class, space, register, size)?;
        let first = resources.len() as u32;
        resources.extend((0..n).map(|i| [class, space, register + i]));
        let array = module.global_variables[g].clone();
        let elements = (first..first + n)
            .map(|b| {
                module.global_variables.append(
                    GlobalVariable {
                        binding: Some(ResourceBinding {
                            group: binding.group,
                            binding: b,
                        }),
                        ty: base,
                        ..array.clone()
                    },
                    Span::UNDEFINED,
                )
            })
            .collect();
        arrays.push((g, elements));
    }
    let n = module.functions.len();
    let all = super::all_functions(module);
    // Accesses picked by constants (function, expression, element), run-time picks (function,
    // expression), and every pointer through a run-time pick, per function.
    let mut fixed = Vec::new();
    let mut picks = Vec::new();
    let mut through: Vec<Vec<Option<Chain>>> = Vec::new();
    let mut uses = Vec::new();
    for (i, f) in all.iter().enumerate() {
        let array_of = |e: Handle<Expression>| match f.expressions[e] {
            Expression::GlobalVariable(g) => arrays.iter().position(|a| a.0 == g),
            _ => None,
        };
        let mut chains: Vec<Option<Chain>> = alloc::vec![None; f.expressions.len()];
        for (h, e) in f.expressions.iter() {
            let (base, step) = match *e {
                Expression::Access { base, index } => (
                    base,
                    constant_index(module, f, index).map_or(Step::Index(index), Step::Constant),
                ),
                Expression::AccessIndex { base, index } => (base, Step::Constant(index)),
                _ => continue,
            };
            if let Some(a) = array_of(base) {
                match step {
                    Step::Constant(k) => {
                        let element = *arrays[a].1.get(k as usize).ok_or(refused())?;
                        fixed.push((i, h, element));
                    }
                    Step::Index(index) => {
                        picks.push((i, h, a));
                        chains[h.index()] = Some(Chain {
                            array: a,
                            index,
                            steps: Vec::new(),
                        });
                    }
                }
            } else if let Some(c) = &chains[base.index()] {
                let mut c = c.clone();
                c.steps.push(step);
                chains[h.index()] = Some(c);
            }
        }
        // Loads and lengths, then stores and atomic operations in the order `each_block` meets them;
        // a pointer passed to a function is refused.
        let mut here = Vec::new();
        for (h, e) in f.expressions.iter() {
            let (pointer, operation) = match *e {
                Expression::Load { pointer } => (pointer, Operation::Load),
                Expression::ArrayLength(pointer) => (pointer, Operation::Length),
                _ => continue,
            };
            if let Some(c) = &chains[pointer.index()] {
                here.push((Some(h), c.clone(), operation));
            }
        }
        let mut body = f.body.clone();
        let mut passed_pointer = false;
        each_block(&mut body, &mut |block| {
            for s in block.iter() {
                if let Statement::Call { arguments, .. } = s {
                    passed_pointer |= arguments.iter().any(|a| chains[a.index()].is_some());
                }
                let Some(c) = pointer_of(s).and_then(|p| chains[p.index()].as_ref()) else {
                    continue;
                };
                let operation = match *s {
                    Statement::Store { value, .. } => Operation::Store { value },
                    Statement::Atomic {
                        fun, value, result, ..
                    } => Operation::Atomic { fun, value, result },
                    _ => unreachable!("pointer_of names stores and atomic operations"),
                };
                here.push((None, c.clone(), operation));
            }
        });
        if passed_pointer {
            return Err(refused());
        }
        if !here.is_empty() {
            uses.extend(typed_uses(module, f, i, here, &typed)?);
        }
        through.push(chains);
    }
    let mut built = Vec::new();
    for u in &uses {
        built.push(helper(&all[u.function], u, &arrays[u.chain.array].1));
    }
    let (helpers, moved) = functions_first(module, |module| -> Vec<Handle<Function>> {
        built
            .into_iter()
            .map(|h| module.functions.append(h, Span::UNDEFINED))
            .collect()
    });
    for (i, chains) in through.iter().enumerate() {
        let f = match moved.get(i) {
            Some(&h) => &mut module.functions[h],
            None => &mut module.entry_points[i - n].function,
        };
        let mut dead = Vec::new();
        for &(j, h, element) in &fixed {
            if j == i {
                *f.expressions.get_mut(h) = Expression::GlobalVariable(element);
                dead.push(h);
            }
        }
        for &(j, h, a) in &picks {
            if j == i {
                *f.expressions.get_mut(h) = Expression::GlobalVariable(arrays[a].1[0]);
                dead.push(h);
            }
        }
        let here: Vec<_> = uses
            .iter()
            .zip(&helpers)
            .filter(|(u, _)| u.function == i)
            .collect();
        // Statements, in the order they were found.
        let mut statements = here.iter().filter(|(u, _)| u.user.is_none());
        let mut results = Vec::new();
        each_block(&mut f.body, &mut |block| {
            for s in block.iter_mut() {
                if !pointer_of(s).is_some_and(|p| chains[p.index()].is_some()) {
                    continue;
                }
                let &(u, &function) = statements.next().expect("one use a statement");
                let result = match u.operation {
                    Operation::Atomic { result, .. } => result,
                    _ => None,
                };
                results.extend(result.map(|r| (r, function)));
                *s = Statement::Call {
                    function,
                    arguments: u.passed.iter().map(|p| p.0).collect(),
                    result,
                };
            }
        });
        for (r, function) in results {
            *f.expressions.get_mut(r) = Expression::CallResult(function);
        }
        let calls: Vec<Placed> = here
            .iter()
            .filter_map(|&(u, &function)| {
                Some(Placed {
                    result: u.user?,
                    function,
                    arguments: u.passed.iter().map(|p| p.0).collect(),
                    pre: None,
                })
            })
            .collect();
        place_calls(f, &calls);
        unemit(&mut f.body, &dead);
    }
    // Drops the arrays, now unused.
    naga::compact::compact(module, naga::compact::KeepUnused::No);
    Ok(())
}

/// The uses `here` (the load or length a call stands for, the chain, the operation) of function `i`
/// with the types of their passed operands and results.
fn typed_uses(
    module: &mut Module,
    f: &Function,
    i: usize,
    here: Vec<(Option<Handle<Expression>>, Chain, Operation)>,
    typed: &impl Fn(&Module, &Function) -> Result<Function>,
) -> Result<Vec<Use>> {
    let typing = typing_copy(module, f, typed)?;
    let mut typifier = naga::front::Typifier::new();
    let ctx =
        naga::proc::ResolveContext::with_locals(module, &typing.local_variables, &typing.arguments);
    let mut inner = |e: Handle<Expression>| -> Result<TypeInner> {
        typifier
            .grow(e, &typing.expressions, &ctx)
            .map_err(|e| ConvertError::Malformed(alloc::format!("{e}")))?;
        Ok(typifier.get(e, &module.types).clone())
    };
    let mut plans = Vec::new();
    for (user, chain, operation) in here {
        let mut wanted = alloc::vec![chain.index];
        wanted.extend(chain.steps.iter().filter_map(|s| match *s {
            Step::Index(e) if !constant(f, e) => Some(e),
            _ => None,
        }));
        match operation {
            Operation::Store { value } | Operation::Atomic { value, .. } => wanted.push(value),
            Operation::Load | Operation::Length => {}
        }
        if let Operation::Atomic {
            fun: AtomicFunction::Exchange {
                compare: Some(compare),
            },
            ..
        } = operation
        {
            wanted.push(compare);
        }
        wanted.retain(|&e| e == chain.index || !constant(f, e));
        let passed = wanted
            .iter()
            .map(|&e| Ok((e, inner(e)?)))
            .collect::<Result<Vec<_>>>()?;
        let result = match operation {
            Operation::Load => Some(inner(user.expect("a load"))?),
            Operation::Length => Some(TypeInner::Scalar(Scalar::U32)),
            Operation::Atomic {
                result: Some(r), ..
            } => match f.expressions[r] {
                Expression::AtomicResult { ty, .. } => Some(module.types[ty].inner.clone()),
                _ => {
                    return Err(ConvertError::Malformed(NOT_AN_ATOMIC_RESULT.into()));
                }
            },
            Operation::Atomic { .. } | Operation::Store { .. } => None,
        };
        plans.push((user, chain, operation, passed, result));
    }
    let mut ty = |inner| {
        module
            .types
            .insert(Type { name: None, inner }, Span::UNDEFINED)
    };
    Ok(plans
        .into_iter()
        .map(|(user, chain, operation, passed, result)| {
            let signed = is_signed(&passed[0].1);
            Use {
                function: i,
                user,
                chain,
                operation,
                passed: passed.into_iter().map(|(e, t)| (e, ty(t))).collect(),
                result: result.map(&mut ty),
                signed,
            }
        })
        .collect())
}

/// The helper for one use: its arguments are the index and the passed operands; a switch on the
/// index runs the operation through the element it names (the last for any index past the array,
/// which Direct3D leaves undefined).
fn helper(f: &Function, u: &Use, elements: &[Handle<GlobalVariable>]) -> Function {
    let mut h = Function {
        arguments: u
            .passed
            .iter()
            .map(|&(_, ty)| FunctionArgument {
                name: None,
                ty,
                binding: None,
            })
            .collect(),
        result: u.result.map(|ty| FunctionResult { ty, binding: None }),
        ..Default::default()
    };
    let args: Vec<_> = (0..u.passed.len() as u32)
        .map(|i| {
            h.expressions
                .append(Expression::FunctionArgument(i), Span::UNDEFINED)
        })
        .collect();
    let operand =
        |h: &mut Function, e: Handle<Expression>| match u.passed.iter().position(|p| p.0 == e) {
            Some(i) => args[i],
            None => copy_constant(f, h, e),
        };
    let steps: Vec<_> = u
        .chain
        .steps
        .iter()
        .map(|s| match *s {
            Step::Constant(k) => Step::Constant(k),
            Step::Index(e) => Step::Index(operand(&mut h, e)),
        })
        .collect();
    let operation = match u.operation {
        Operation::Store { value } => Operation::Store {
            value: operand(&mut h, value),
        },
        Operation::Atomic { fun, value, result } => Operation::Atomic {
            fun: match fun {
                AtomicFunction::Exchange {
                    compare: Some(compare),
                } => AtomicFunction::Exchange {
                    compare: Some(operand(&mut h, compare)),
                },
                fun => fun,
            },
            value: operand(&mut h, value),
            result,
        },
        other => other,
    };
    let local = u.result.map(|ty| {
        let var = h.local_variables.append(
            LocalVariable {
                name: None,
                ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        h.expressions
            .append(Expression::LocalVariable(var), Span::UNDEFINED)
    });
    let mut cases = Vec::new();
    for (k, &element) in elements.iter().enumerate() {
        let mut body = Block::new();
        let mut pointer = h
            .expressions
            .append(Expression::GlobalVariable(element), Span::UNDEFINED);
        let mut emitted = Vec::new();
        let mut emit = |h: &mut Function, e| {
            let e = h.expressions.append(e, Span::UNDEFINED);
            emitted.push(e);
            e
        };
        for &step in &steps {
            pointer = emit(
                &mut h,
                match step {
                    Step::Constant(index) => Expression::AccessIndex {
                        base: pointer,
                        index,
                    },
                    Step::Index(index) => Expression::Access {
                        base: pointer,
                        index,
                    },
                },
            );
        }
        let value = match operation {
            Operation::Load => Some(emit(&mut h, Expression::Load { pointer })),
            Operation::Length => Some(emit(&mut h, Expression::ArrayLength(pointer))),
            Operation::Store { .. } | Operation::Atomic { .. } => None,
        };
        if let (Some(&first), Some(&last)) = (emitted.first(), emitted.last()) {
            body.push(
                Statement::Emit(Range::new_from_bounds(first, last)),
                Span::UNDEFINED,
            );
        }
        let value = match operation {
            Operation::Store { value } => {
                body.push(Statement::Store { pointer, value }, Span::UNDEFINED);
                None
            }
            Operation::Atomic { fun, value, result } => {
                let result = result.map(|r| {
                    h.expressions
                        .append(f.expressions[r].clone(), Span::UNDEFINED)
                });
                body.push(
                    Statement::Atomic {
                        pointer,
                        fun,
                        value,
                        result,
                    },
                    Span::UNDEFINED,
                );
                result
            }
            Operation::Load | Operation::Length => value,
        };
        if let (Some(pointer), Some(value)) = (local, value) {
            body.push(Statement::Store { pointer, value }, Span::UNDEFINED);
        }
        cases.push(SwitchCase {
            value: case_value(k, elements.len(), u.signed),
            body,
            fall_through: false,
        });
    }
    h.body.push(
        Statement::Switch {
            selector: args[0],
            cases,
        },
        Span::UNDEFINED,
    );
    if let Some(pointer) = local {
        let load = h
            .expressions
            .append(Expression::Load { pointer }, Span::UNDEFINED);
        h.body.push(
            Statement::Emit(Range::new_from_bounds(load, load)),
            Span::UNDEFINED,
        );
        h.body
            .push(Statement::Return { value: Some(load) }, Span::UNDEFINED);
    }
    h
}
