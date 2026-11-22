//! Arrays of textures, storage textures and typed buffers (`Texture2D t[8] : register(t0)`): WebGPU
//! has no binding arrays, so each element becomes a resource of its own at its own register, as
//! Direct3D 12 numbers a range; an array declared without a length takes the rest of the root
//! signature's range holding its first register (`VariantKey::register_ranges`). An element picked by a constant is that texture; one read after a pick at run time is a
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
use wcontainers_gpu::VariantKey;

type Result<T> = core::result::Result<T, ConvertError>;

pub(super) fn refused() -> ConvertError {
    ConvertError::Unsupported(crate::refusal::RESOURCE_ARRAY)
}

/// One use of an element picked at run time: the image expression using it, rewritten as a call.
struct Dynamic {
    /// The function (index into functions, then entry points) and the using expression.
    function: usize,
    user: Handle<Expression>,
    /// The arrays its image and its sampler are picked from, at one index.
    image: Option<usize>,
    sampler: Option<usize>,
    /// Whether the index is signed (the switch's cases are of its type).
    signed: bool,
    /// Operands passed to the helper, with their types; the rest are copied into it.
    passed: Vec<(Handle<Expression>, Handle<Type>)>,
    result: Handle<Type>,
}

/// The operands of an image expression other than its image, in a fixed order.
pub(crate) fn operands(e: &mut Expression) -> Result<Vec<&mut Handle<Expression>>> {
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

pub(crate) fn image_of(e: &mut Expression) -> Option<&mut Handle<Expression>> {
    match e {
        Expression::ImageSample { image, .. }
        | Expression::ImageLoad { image, .. }
        | Expression::ImageQuery { image, .. } => Some(image),
        _ => None,
    }
}

fn sampler_of(e: &mut Expression) -> Option<&mut Handle<Expression>> {
    match e {
        Expression::ImageSample { sampler, .. } => Some(sampler),
        _ => None,
    }
}

/// Whether `a` and `b` of `f` give the same value wherever both are used: the same expression, or
/// the same operations on the same values with nothing read from memory.
fn same_value(f: &Function, a: Handle<Expression>, b: Handle<Expression>) -> bool {
    use Expression as E;
    if a == b {
        return true;
    }
    match (&f.expressions[a], &f.expressions[b]) {
        (E::Literal(x), E::Literal(y)) => x == y,
        (E::Constant(x), E::Constant(y)) => x == y,
        (E::FunctionArgument(x), E::FunctionArgument(y)) => x == y,
        (E::GlobalVariable(x), E::GlobalVariable(y)) => x == y,
        (
            E::As {
                expr: x,
                kind: k,
                convert: c,
            },
            E::As {
                expr: y,
                kind: l,
                convert: d,
            },
        ) => k == l && c == d && same_value(f, *x, *y),
        (E::Unary { op, expr: x }, E::Unary { op: other, expr: y }) => {
            op == other && same_value(f, *x, *y)
        }
        (
            E::Binary {
                op,
                left: x,
                right: r,
            },
            E::Binary {
                op: other,
                left: y,
                right: s,
            },
        ) => op == other && same_value(f, *x, *y) && same_value(f, *r, *s),
        (E::AccessIndex { base: x, index: i }, E::AccessIndex { base: y, index: j }) => {
            i == j && same_value(f, *x, *y)
        }
        _ => false,
    }
}

/// Whether `e` of `f` is a value known before the program runs, which the helper copies.
pub(crate) fn constant(f: &Function, e: Handle<Expression>) -> bool {
    match f.expressions[e] {
        Expression::Literal(_) | Expression::Constant(_) | Expression::ZeroValue(_) => true,
        Expression::Compose { ref components, .. } => components.iter().all(|&c| constant(f, c)),
        Expression::Splat { value, .. } => constant(f, value),
        _ => false,
    }
}

/// Copies the constant `e` of `from` into `to`, emitting what is made at run time.
pub(crate) fn copy_constant(
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
pub(super) fn constant_index(module: &Module, f: &Function, e: Handle<Expression>) -> Option<u32> {
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

/// Splits every array of textures, storage textures and typed buffers into its elements, numbered
/// after the resources `resources` lists, and rewrites the uses. Runs right after reading: it moves
/// the module's functions and compacts it. Arrays of anything else stay refused.
pub(super) fn split(
    module: &mut Module,
    resources: &mut Resources,
    declared: &mut Vec<super::images::Declared>,
    variant: &VariantKey,
) -> Result<()> {
    let vec4 = super::depth::vec4(module);
    super::buffer_arrays::split(module, resources, variant, |module, f| {
        super::depth::as_widened(module, f, vec4)
    })?;
    split_with(
        module,
        |module, binding, base, size| {
            // Typed buffers were read as 1D images (`images::prepare`).
            let buffer = super::images::typed_buffer_at(declared, binding);
            match module.types[base].inner {
                TypeInner::Image {
                    dim,
                    class:
                        ImageClass::Sampled { .. }
                        | ImageClass::Depth { .. }
                        | ImageClass::Storage { .. },
                    ..
                } if dim != ImageDimension::D1 || buffer => {}
                _ => return Err(refused()),
            }
            let &[class, space, register] =
                resources
                    .get(binding.binding as usize)
                    .ok_or(ConvertError::Malformed(
                        "a binding past the resource list".into(),
                    ))?;
            let n = match size {
                ArraySize::Constant(n) => n.get(),
                ArraySize::Dynamic => unbounded_length(variant, class, space, register)?,
                _ => return Err(refused()),
            };
            let first = resources.len() as u32;
            super::images::declare_elements(declared, binding, first..first + n);
            resources.extend((0..n).map(|i| [class, space, register + i]));
            Ok((first..first + n)
                .map(|binding_of_element| ResourceBinding {
                    group: binding.group,
                    binding: binding_of_element,
                })
                .collect())
        },
        |module, f| super::depth::as_widened(module, f, vec4),
    )
}

/// Splits every binding array into its elements, each a resource of its own at the binding
/// `elements` gives it (from the array's binding, element type and size), and rewrites the uses: an
/// element picked by a constant is that resource; an image read with its image, its sampler or both
/// picked at run time (at one index) becomes a call of a function switching on the index; a store to
/// an image picked at run time a switch in place. `typed` gives the copy of a function its
/// expressions are typed in. Moves the module's functions and compacts it.
pub(crate) fn split_with(
    module: &mut Module,
    mut elements: impl FnMut(
        &Module,
        ResourceBinding,
        Handle<Type>,
        ArraySize,
    ) -> Result<Vec<ResourceBinding>>,
    typed: impl Fn(&Module, &Function) -> Result<Function>,
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
        let binding = binding.ok_or(refused())?;
        let bindings = elements(module, binding, base, size)?;
        let made = bindings
            .into_iter()
            .map(|binding| {
                module.global_variables.append(
                    GlobalVariable {
                        name: module.global_variables[g].name.clone(),
                        space: naga::AddressSpace::Handle,
                        binding: Some(binding),
                        ty: base,
                        init: None,
                        memory_decorations: module.global_variables[g].memory_decorations,
                    },
                    Span::UNDEFINED,
                )
            })
            .collect();
        arrays.push((g, made));
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
        // Run-time picks used as images and samplers; one passed to a function is refused, as is an
        // array passed whole.
        for (h, e) in f.expressions.iter() {
            let mut e = e.clone();
            let pick_of = |o: Option<&mut Handle<Expression>>| {
                o.and_then(|o| picked.iter().find(|p| p.0 == *o).copied())
            };
            let image = pick_of(image_of(&mut e));
            let sampler = pick_of(sampler_of(&mut e));
            let index = match (image, sampler) {
                (None, None) => continue,
                (Some((_, a, index)), Some((_, b, other))) => {
                    if !same_value(f, index, other) || arrays[a].1.len() != arrays[b].1.len() {
                        return Err(ConvertError::Unsupported(
                            crate::refusal::RESOURCE_ARRAY_PAIR,
                        ));
                    }
                    index
                }
                (Some((_, _, index)), None) | (None, Some((_, _, index))) => index,
            };
            dynamic.push((i, h, image.map(|p| p.1), sampler.map(|p| (p.0, p.1)), index));
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
    // Elements picked by constants are their resources from here, so a helper copies them.
    for &(i, h, element) in &fixed {
        let f = match i < n {
            true => module.functions.iter_mut().nth(i).expect("a function").1,
            false => &mut module.entry_points[i - n].function,
        };
        *f.expressions.get_mut(h) = Expression::GlobalVariable(element);
    }
    // The types of what the helpers take and give, in the copies `typed` gives.
    let functions = module.functions.iter().map(|(_, f)| f);
    let all: Vec<&Function> = functions
        .chain(module.entry_points.iter().map(|ep| &ep.function))
        .collect();
    let mut plans = Vec::new();
    for &(i, h, image, sampler, index) in &dynamic {
        let f = &typing_copy(module, all[i], &typed)?;
        let mut typifier = naga::front::Typifier::new();
        let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
        let mut e = f.expressions[h].clone();
        let mut passed = Vec::new();
        let mut wanted = alloc::vec![index];
        for o in operands(&mut e)? {
            let o = *o;
            let picked_sampler = sampler.is_some_and(|(s, _)| s == o);
            if !constant(f, o)
                && !picked_sampler
                && !matches!(f.expressions[o], Expression::GlobalVariable(_))
            {
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
        plans.push((i, h, image, sampler.map(|s| s.1), passed, result));
    }
    let mut made = Vec::new();
    for (i, h, image, sampler, passed, result) in plans {
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
            image,
            sampler,
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
        let elements_of = |a: Option<usize>| a.map(|a| arrays[a].1.as_slice());
        built.push(helper(f, d, elements_of(d.image), elements_of(d.sampler))?);
    }
    let (helpers, moved) = functions_first(module, |module| -> Vec<Handle<Function>> {
        built
            .into_iter()
            .map(|h| module.functions.append(h, Span::UNDEFINED))
            .collect()
    });
    for (i, picks) in stored.iter().enumerate() {
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

/// The copy of `f` its expressions are typed in: `typed`'s, with each texel pointer (an access into
/// an image, which atomic operations use, and which has no type) standing for its image.
pub(super) fn typing_copy(
    module: &Module,
    f: &Function,
    typed: &impl Fn(&Module, &Function) -> Result<Function>,
) -> Result<Function> {
    let mut f = typed(module, f)?;
    let global_is = |e: &Expression, is: fn(&TypeInner) -> bool| match *e {
        Expression::GlobalVariable(g) => is(&module.types[module.global_variables[g].ty].inner),
        _ => false,
    };
    let handles: Vec<_> = f.expressions.iter().map(|(h, _)| h).collect();
    for h in handles {
        let Expression::Access { base, .. } = f.expressions[h] else {
            continue;
        };
        let image = match f.expressions[base] {
            ref e @ Expression::GlobalVariable(_) => {
                global_is(e, |t| matches!(t, TypeInner::Image { .. }))
            }
            Expression::Access { base, .. } | Expression::AccessIndex { base, .. } => {
                global_is(&f.expressions[base], |t| {
                    matches!(t, TypeInner::BindingArray { .. })
                })
            }
            _ => false,
        };
        if image {
            *f.expressions.get_mut(h) = f.expressions[base].clone();
        }
    }
    Ok(f)
}

/// An element of an array picked at run time: the access, the array and the index.
struct Pick {
    access: Handle<Expression>,
    elements: usize,
    index: Handle<Expression>,
}

/// The length of an array declared without one: the rest of the root signature's range of its kind
/// and space holding its first register. No more elements than the kind's slots hold.
pub(super) fn unbounded_length(
    variant: &VariantKey,
    class: u32,
    space: u32,
    register: u32,
) -> Result<u32> {
    let kind =
        super::slot_kind(class).ok_or_else(|| ConvertError::Malformed(super::NO_CLASS.into()))?;
    let length = variant
        .register_ranges
        .iter()
        .find(|(k, s, range)| *k == kind && *s == space && range.contains(&register))
        .map(|(_, _, range)| range.end - register)
        .ok_or(ConvertError::Unsupported(
            crate::refusal::UNBOUNDED_RESOURCE_ARRAY,
        ))?;
    if length > kind.slots() {
        return Err(ConvertError::Unsupported(
            crate::refusal::BINDINGS_BEYOND_LIMIT,
        ));
    }
    Ok(length)
}

pub(super) fn is_signed(inner: &TypeInner) -> bool {
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
pub(super) fn case_value(k: usize, count: usize, signed: bool) -> SwitchValue {
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
/// switch on the index runs the user on the image and sampler elements it names (the last for any
/// index past the array, which Direct3D leaves undefined).
fn helper(
    f: &Function,
    d: &Dynamic,
    images: Option<&[Handle<GlobalVariable>]>,
    samplers: Option<&[Handle<GlobalVariable>]>,
) -> Result<Function> {
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
    let picked_sampler = samplers.and(sampler_of(&mut user).map(|s| *s));
    for o in operands(&mut user)? {
        if Some(*o) == picked_sampler {
            continue;
        }
        *o = match d.passed.iter().position(|p| p.0 == *o) {
            Some(i) => args[i],
            None => copy_constant(f, &mut h, *o),
        };
    }
    if images.is_none() {
        let image = image_of(&mut user).expect("an image expression");
        if !matches!(f.expressions[*image], Expression::GlobalVariable(_)) {
            return Err(refused());
        }
        *image = copy_constant(f, &mut h, *image);
    }
    let elements = images.or(samplers).expect("a pick").len();
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
    for k in 0..elements {
        let mut body = Block::new();
        let mut e = user.clone();
        if let Some(images) = images {
            *image_of(&mut e).expect("an image expression") = h
                .expressions
                .append(Expression::GlobalVariable(images[k]), Span::UNDEFINED);
        }
        if let Some(samplers) = samplers {
            *sampler_of(&mut e).expect("a sample") = h
                .expressions
                .append(Expression::GlobalVariable(samplers[k]), Span::UNDEFINED);
        }
        let value = h.expressions.append(e, Span::UNDEFINED);
        body.push(
            Statement::Emit(Range::new_from_bounds(value, value)),
            Span::UNDEFINED,
        );
        body.push(Statement::Store { pointer, value }, Span::UNDEFINED);
        cases.push(SwitchCase {
            value: case_value(k, elements, d.signed),
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
pub(super) fn unemit(block: &mut Block, dead: &[Handle<Expression>]) {
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
