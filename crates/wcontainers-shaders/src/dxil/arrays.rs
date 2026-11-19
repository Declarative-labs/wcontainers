//! Arrays of textures and storage textures (`Texture2D t[8] : register(t0)`): WebGPU has no binding
//! arrays, so each element becomes a texture of its own at its own register, as Direct3D 12 numbers
//! a range. An element picked by a constant is that texture; one read after a pick at run time is a
//! call of a function that switches on the index and uses the element it names, and a store to one
//! is a switch in place.

use super::{Placed, Resources, functions_first, place_calls};
use crate::ConvertError;
use alloc::vec::Vec;
use naga::{
    ArraySize, Block, Expression, Function, FunctionArgument, FunctionResult, GlobalVariable,
    Handle, ImageClass, ImageDimension, ImageQuery, Literal, LocalVariable, Module, Range,
    ResourceBinding, SampleLevel, Span, Statement, SwitchCase, SwitchValue, Type, TypeInner,
};

type Result<T> = core::result::Result<T, ConvertError>;

fn refused() -> ConvertError {
    ConvertError::Unsupported(crate::refusal::RESOURCE_ARRAY)
}

/// One use of an element picked at run time: the image expression using it, rewritten as a call.
struct Dynamic {
    /// The function (index into functions, then entry points) and the using expression.
    function: usize,
    user: Handle<Expression>,
    elements: usize,
    /// Whether the index is signed (the switch's cases are of its type).
    signed: bool,
    /// Operands passed to the helper, with their types; the rest are copied into it.
    passed: Vec<(Handle<Expression>, Handle<Type>)>,
    result: Handle<Type>,
}

/// The operands of an image expression other than its image, in a fixed order.
pub(super) fn operands(e: &mut Expression) -> Result<Vec<&mut Handle<Expression>>> {
    let mut out = Vec::new();
    match e {
        Expression::ImageSample {
            sampler,
            coordinate,
            array_index,
            offset,
            level,
            depth_ref,
            ..
        } => {
            out.extend([sampler, coordinate]);
            out.extend(array_index.as_mut());
            out.extend(offset.as_mut());
            match level {
                SampleLevel::Exact(h) | SampleLevel::Bias(h) => out.push(h),
                SampleLevel::Gradient { x, y } => out.extend([x, y]),
                SampleLevel::Auto | SampleLevel::Zero => {}
            }
            out.extend(depth_ref.as_mut());
        }
        Expression::ImageLoad {
            coordinate,
            array_index,
            sample,
            level,
            ..
        } => {
            out.push(coordinate);
            out.extend(array_index.as_mut());
            out.extend(sample.as_mut());
            out.extend(level.as_mut());
        }
        Expression::ImageQuery { query, .. } => {
            if let ImageQuery::Size { level: Some(l) } = query {
                out.push(l);
            }
        }
        _ => return Err(refused()),
    }
    Ok(out)
}

fn image_of(e: &mut Expression) -> Option<&mut Handle<Expression>> {
    match e {
        Expression::ImageSample { image, .. }
        | Expression::ImageLoad { image, .. }
        | Expression::ImageQuery { image, .. } => Some(image),
        _ => None,
    }
}

/// Whether `e` of `f` is a value known before the program runs, which the helper copies.
pub(super) fn constant(f: &Function, e: Handle<Expression>) -> bool {
    match f.expressions[e] {
        Expression::Literal(_) | Expression::Constant(_) | Expression::ZeroValue(_) => true,
        Expression::Compose { ref components, .. } => components.iter().all(|&c| constant(f, c)),
        Expression::Splat { value, .. } => constant(f, value),
        _ => false,
    }
}

/// Copies the constant `e` of `from` into `to`, emitting what is made at run time.
pub(super) fn copy_constant(
    from: &Function,
    to: &mut Function,
    e: Handle<Expression>,
) -> Handle<Expression> {
    let made = match from.expressions[e] {
        Expression::Compose { ty, ref components } => Expression::Compose {
            ty,
            components: components
                .iter()
                .map(|&c| copy_constant(from, to, c))
                .collect(),
        },
        Expression::Splat { size, value } => Expression::Splat {
            size,
            value: copy_constant(from, to, value),
        },
        ref other => return to.expressions.append(other.clone(), Span::UNDEFINED),
    };
    let h = to.expressions.append(made, Span::UNDEFINED);
    to.body.push(
        Statement::Emit(Range::new_from_bounds(h, h)),
        Span::UNDEFINED,
    );
    h
}

/// A constant index's value.
fn constant_index(module: &Module, f: &Function, e: Handle<Expression>) -> Option<u32> {
    let literal = |e: &Expression| match *e {
        Expression::Literal(Literal::U32(v)) => Some(v),
        Expression::Literal(Literal::I32(v)) => u32::try_from(v).ok(),
        _ => None,
    };
    match f.expressions[e] {
        Expression::Constant(c) => literal(&module.global_expressions[module.constants[c].init]),
        ref other => literal(other),
    }
}

/// Splits every array of textures and storage textures into its elements, numbered after the
/// resources `resources` lists, and rewrites the uses. Runs right after reading: it moves the
/// module's functions and compacts it. Arrays of anything else stay refused.
pub(super) fn split(
    module: &mut Module,
    resources: &mut Resources,
    declared: &mut Vec<super::images::Declared>,
) -> Result<()> {
    let mut arrays: Vec<(Handle<GlobalVariable>, Vec<Handle<GlobalVariable>>)> = Vec::new();
    let found: Vec<_> = module
        .global_variables
        .iter()
        .filter_map(|(g, v)| match module.types[v.ty].inner {
            TypeInner::BindingArray { base, size } => Some((g, base, size, v.binding)),
            _ => None,
        })
        .collect();
    if found.is_empty() {
        return Ok(());
    }
    for (g, base, size, binding) in found {
        let ArraySize::Constant(n) = size else {
            return Err(refused());
        };
        // Typed buffers were read as 1D images (`images::prepare`); they stay refused.
        match module.types[base].inner {
            TypeInner::Image {
                dim,
                class:
                    ImageClass::Sampled { .. } | ImageClass::Depth { .. } | ImageClass::Storage { .. },
                ..
            } if dim != ImageDimension::D1 => {}
            _ => return Err(refused()),
        }
        let binding = binding.ok_or(refused())?;
        let &[class, space, register] =
            resources
                .get(binding.binding as usize)
                .ok_or(ConvertError::Malformed(
                    "a binding past the resource list".into(),
                ))?;
        let mut elements = Vec::new();
        let first = resources.len() as u32;
        super::images::declare_elements(declared, binding, first..first + n.get());
        for i in 0..n.get() {
            resources.push([class, space, register + i]);
            elements.push(module.global_variables.append(
                GlobalVariable {
                    name: module.global_variables[g].name.clone(),
                    space: naga::AddressSpace::Handle,
                    binding: Some(ResourceBinding {
                        group: binding.group,
                        binding: resources.len() as u32 - 1,
                    }),
                    ty: base,
                    init: None,
                    memory_decorations: module.global_variables[g].memory_decorations,
                },
                Span::UNDEFINED,
            ));
        }
        arrays.push((g, elements));
    }
    let array_of = |e: &Expression| match *e {
        Expression::GlobalVariable(g) => arrays.iter().position(|(a, _)| *a == g),
        _ => None,
    };
    // Per function: accesses picked by constants (expression, array, element) and run-time uses.
    let n = module.functions.len();
    let mut fixed = Vec::new();
    let mut dynamic = Vec::new();
    // Run-time picks per function, for stores.
    let mut stored = Vec::new();
    // Every access to an array, to make dead once its users are rewritten.
    let mut accesses = Vec::new();
    let functions = module.functions.iter().map(|(_, f)| f);
    let all: Vec<&Function> = functions
        .chain(module.entry_points.iter().map(|ep| &ep.function))
        .collect();
    for (i, f) in all.iter().enumerate() {
        let mut picked = Vec::new();
        for (h, e) in f.expressions.iter() {
            let (base, k) = match *e {
                Expression::Access { base, index } => (base, constant_index(module, f, index)),
                Expression::AccessIndex { base, index } => (base, Some(index)),
                _ => continue,
            };
            let Some(a) = array_of(&f.expressions[base]) else {
                continue;
            };
            accesses.push((i, h, a));
            match k {
                Some(k) if (k as usize) < arrays[a].1.len() => {
                    fixed.push((i, h, arrays[a].1[k as usize]))
                }
                Some(_) => return Err(refused()),
                None => {
                    let Expression::Access { index, .. } = *e else {
                        unreachable!("AccessIndex has a constant index");
                    };
                    picked.push((h, a, index));
                }
            }
        }
        // Run-time picks used as images; one passed to a function is refused (dxil-spirv's
        // functions take no images), as is an array passed whole.
        for (h, e) in f.expressions.iter() {
            let mut e = e.clone();
            if let Some(image) = image_of(&mut e) {
                let image = *image;
                if let Some(&(_, a, index)) = picked.iter().find(|p| p.0 == image) {
                    dynamic.push((i, h, a, index));
                }
            }
        }
        let is_pick = |o| {
            picked
                .iter()
                .any(|p: &(Handle<Expression>, usize, _)| p.0 == o)
        };
        // An atomic operation on a texel of an element picked at run time stays refused.
        let atomic_on_pick = |s: &Statement| match *s {
            Statement::Atomic { pointer, .. } => matches!(
                f.expressions[pointer],
                Expression::Access { base, .. } | Expression::AccessIndex { base, .. } if is_pick(base)
            ),
            Statement::ImageAtomic { image, .. } => is_pick(image),
            _ => false,
        };
        let passes_pick = |s: &Statement| match s {
            Statement::Call { arguments, .. } => arguments
                .iter()
                .any(|&a| is_pick(a) || array_of(&f.expressions[a]).is_some()),
            _ => false,
        };
        if any_statement(&f.body, &|s| passes_pick(s) || atomic_on_pick(s)) {
            return Err(refused());
        }
        stored.push(
            picked
                .into_iter()
                .map(|(access, elements, index)| Pick {
                    access,
                    elements,
                    index,
                })
                .collect::<Vec<_>>(),
        );
    }
    // The types of what the helpers take and give, with plain reads of depth textures typed as
    // the four components `depth::widen` makes them give.
    let vec4 = (!dynamic.is_empty()).then(|| super::depth::vec4(module));
    let functions = module.functions.iter().map(|(_, f)| f);
    let all: Vec<&Function> = functions
        .chain(module.entry_points.iter().map(|ep| &ep.function))
        .collect();
    let mut plans = Vec::new();
    for &(i, h, a, index) in &dynamic {
        let vec4 = vec4.expect("a run-time pick");
        let mut f = super::depth::as_widened(module, all[i], vec4)?;
        // Texel pointers (an access into an image, which atomic operations use) have no type;
        // for typing, each stands for its image.
        let handles: Vec<_> = f.expressions.iter().map(|(h, _)| h).collect();
        for h in handles {
            let Expression::Access { base, .. } = f.expressions[h] else {
                continue;
            };
            let image = match f.expressions[base] {
                Expression::GlobalVariable(g) => matches!(
                    module.types[module.global_variables[g].ty].inner,
                    TypeInner::Image { .. }
                ),
                Expression::Access { base, .. } | Expression::AccessIndex { base, .. } => {
                    array_of(&f.expressions[base]).is_some()
                }
                _ => false,
            };
            if image {
                *f.expressions.get_mut(h) = f.expressions[base].clone();
            }
        }
        let f = &f;
        let mut typifier = naga::front::Typifier::new();
        let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
        let mut e = f.expressions[h].clone();
        let mut passed = Vec::new();
        let mut wanted = alloc::vec![index];
        for o in operands(&mut e)? {
            let o = *o;
            if !constant(f, o) && !matches!(f.expressions[o], Expression::GlobalVariable(_)) {
                wanted.push(o);
            }
        }
        wanted.push(h);
        let mut inners = Vec::new();
        for &w in &wanted {
            typifier
                .grow(w, &f.expressions, &ctx)
                .map_err(|e| ConvertError::Malformed(alloc::format!("{e}")))?;
            inners.push(typifier.get(w, &module.types).clone());
        }
        let result = inners.pop().expect("the user's");
        passed.extend(wanted[..wanted.len() - 1].iter().copied().zip(inners));
        plans.push((i, h, a, passed, result));
    }
    let mut made = Vec::new();
    for (i, h, a, passed, result) in plans {
        let mut ty = |inner| {
            module
                .types
                .insert(Type { name: None, inner }, Span::UNDEFINED)
        };
        let passed: Vec<_> = passed
            .into_iter()
            .map(|(e, inner)| (e, ty(inner)))
            .collect();
        let result = ty(result);
        let signed = is_signed(&module.types[passed[0].1].inner);
        made.push(Dynamic {
            function: i,
            user: h,
            elements: a,
            signed,
            passed,
            result,
        });
    }
    let mut built = Vec::new();
    for d in &made {
        let f = match d.function < n {
            true => {
                module
                    .functions
                    .iter()
                    .nth(d.function)
                    .expect("a function")
                    .1
            }
            false => &module.entry_points[d.function - n].function,
        };
        built.push(helper(f, d, &arrays[d.elements].1)?);
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
        let calls: Vec<Placed> = made
            .iter()
            .zip(&helpers)
            .filter(|(d, _)| d.function == i)
            .map(|(d, &helper)| Placed {
                result: d.user,
                function: helper,
                arguments: d.passed.iter().map(|p| p.0).collect(),
                pre: None,
            })
            .collect();
        place_calls(f, &calls);
        let picks = &stored[i];
        if !picks.is_empty() {
            let mut expressions = core::mem::take(&mut f.expressions);
            super::each_block(&mut f.body, &mut |block| {
                switch_stores(block, &mut expressions, picks, &arrays)
            });
            f.expressions = expressions;
        }
        let mut dead = Vec::new();
        for &(j, h, element) in &fixed {
            if j == i {
                *f.expressions.get_mut(h) = Expression::GlobalVariable(element);
                dead.push(h);
            }
        }
        for &(j, h, a) in &accesses {
            if j == i && !dead.contains(&h) {
                *f.expressions.get_mut(h) = Expression::GlobalVariable(arrays[a].1[0]);
                dead.push(h);
            }
        }
        unemit(&mut f.body, &dead);
    }
    // Drops the arrays, now unused.
    naga::compact::compact(module, naga::compact::KeepUnused::No);
    Ok(())
}

/// An element of an array picked at run time: the access, the array and the index.
struct Pick {
    access: Handle<Expression>,
    elements: usize,
    index: Handle<Expression>,
}

fn is_signed(inner: &TypeInner) -> bool {
    matches!(
        *inner,
        TypeInner::Scalar(naga::Scalar {
            kind: naga::ScalarKind::Sint,
            ..
        })
    )
}

/// The case of a switch on an index that runs for element `k` of `count` (the last for any index
/// past the array, which Direct3D leaves undefined).
fn case_value(k: usize, count: usize, signed: bool) -> SwitchValue {
    match k + 1 == count {
        true => SwitchValue::Default,
        false if signed => SwitchValue::I32(k as i32),
        false => SwitchValue::U32(k as u32),
    }
}

/// Replaces each store in `block` to an element picked at run time with a switch on the index,
/// read as unsigned, storing to the element named.
fn switch_stores(
    block: &mut Block,
    expressions: &mut naga::Arena<Expression>,
    picks: &[Pick],
    arrays: &[(Handle<GlobalVariable>, Vec<Handle<GlobalVariable>>)],
) {
    for (s, span) in core::mem::take(block).span_into_iter() {
        let image = match s {
            Statement::ImageStore { image, .. } => image,
            s => {
                block.push(s, span);
                continue;
            }
        };
        let Some(pick) = picks.iter().find(|p| p.access == image) else {
            block.push(s, span);
            continue;
        };
        let elements = &arrays[pick.elements].1;
        let selector = expressions.append(
            Expression::As {
                expr: pick.index,
                kind: naga::ScalarKind::Uint,
                convert: None,
            },
            span,
        );
        block.push(
            Statement::Emit(Range::new_from_bounds(selector, selector)),
            span,
        );
        let cases = elements
            .iter()
            .enumerate()
            .map(|(k, &element)| {
                let mut s = s.clone();
                let Statement::ImageStore { ref mut image, .. } = s else {
                    unreachable!("matched above");
                };
                *image = expressions.append(Expression::GlobalVariable(element), span);
                let mut body = Block::new();
                body.push(s, span);
                SwitchCase {
                    value: case_value(k, elements.len(), false),
                    body,
                    fall_through: false,
                }
            })
            .collect();
        block.push(Statement::Switch { selector, cases }, span);
    }
}

/// The helper for one run-time pick: its arguments are the index and the passed operands; a
/// switch on the index runs the user on the element it names (the last for any index past the
/// array, which Direct3D leaves undefined).
fn helper(f: &Function, d: &Dynamic, elements: &[Handle<GlobalVariable>]) -> Result<Function> {
    let mut h = Function {
        arguments: d
            .passed
            .iter()
            .map(|&(_, ty)| FunctionArgument {
                name: None,
                ty,
                binding: None,
            })
            .collect(),
        result: Some(FunctionResult {
            ty: d.result,
            binding: None,
        }),
        ..Default::default()
    };
    let args: Vec<_> = (0..d.passed.len() as u32)
        .map(|i| {
            h.expressions
                .append(Expression::FunctionArgument(i), Span::UNDEFINED)
        })
        .collect();
    let mut user = f.expressions[d.user].clone();
    for o in operands(&mut user)? {
        *o = match d.passed.iter().position(|p| p.0 == *o) {
            Some(i) => args[i],
            None => copy_constant(f, &mut h, *o),
        };
    }
    let var = h.local_variables.append(
        LocalVariable {
            name: None,
            ty: d.result,
            init: None,
        },
        Span::UNDEFINED,
    );
    let pointer = h
        .expressions
        .append(Expression::LocalVariable(var), Span::UNDEFINED);
    let mut cases = Vec::new();
    for (k, &element) in elements.iter().enumerate() {
        let mut body = Block::new();
        let image = h
            .expressions
            .append(Expression::GlobalVariable(element), Span::UNDEFINED);
        let mut e = user.clone();
        *image_of(&mut e).expect("an image expression") = image;
        let value = h.expressions.append(e, Span::UNDEFINED);
        body.push(
            Statement::Emit(Range::new_from_bounds(value, value)),
            Span::UNDEFINED,
        );
        body.push(Statement::Store { pointer, value }, Span::UNDEFINED);
        cases.push(SwitchCase {
            value: case_value(k, elements.len(), d.signed),
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
    let load = h
        .expressions
        .append(Expression::Load { pointer }, Span::UNDEFINED);
    h.body.push(
        Statement::Emit(Range::new_from_bounds(load, load)),
        Span::UNDEFINED,
    );
    h.body
        .push(Statement::Return { value: Some(load) }, Span::UNDEFINED);
    Ok(h)
}

/// Takes `dead` out of every emitted range in `block` and the blocks in it.
fn unemit(block: &mut Block, dead: &[Handle<Expression>]) {
    if dead.is_empty() {
        return;
    }
    super::each_block(block, &mut |block| {
        for (s, span) in core::mem::take(block).span_into_iter() {
            let Statement::Emit(range) = s else {
                block.push(s, span);
                continue;
            };
            let mut run: Option<(Handle<Expression>, Handle<Expression>)> = None;
            for h in range {
                if dead.contains(&h) {
                    if let Some((first, last)) = run.take() {
                        block.push(Statement::Emit(Range::new_from_bounds(first, last)), span);
                    }
                    continue;
                }
                run = Some((run.map_or(h, |r| r.0), h));
            }
            if let Some((first, last)) = run {
                block.push(Statement::Emit(Range::new_from_bounds(first, last)), span);
            }
        }
    });
}

/// Whether a statement in `block` or the blocks in it is one `is` picks.
fn any_statement(block: &Block, is: &impl Fn(&Statement) -> bool) -> bool {
    block.iter().any(|s| {
        is(s)
            || match s {
                Statement::Block(b) => any_statement(b, is),
                Statement::If { accept, reject, .. } => {
                    any_statement(accept, is) || any_statement(reject, is)
                }
                Statement::Loop {
                    body, continuing, ..
                } => any_statement(body, is) || any_statement(continuing, is),
                Statement::Switch { cases, .. } => cases.iter().any(|c| any_statement(&c.body, is)),
                _ => false,
            }
    })
}
