//! Views' component mappings applied in the program (`VariantKey::swizzled`): each load or sample
//! of a mapped slot becomes a call of a function making the same read and composing its result
//! from the mapping; a gather of component c gathers the component c maps to. A depth texture
//! holds its first component alone in the common form, so its gathers of another component are
//! resolved here too, through the identity where the slot has no mapping.

use wcontainers_gpu::reflect::ViewComponent;

use crate::refusal::{DEPTH_GATHER_COMPONENT, SWIZZLED_DEPTH};

use super::*;

/// Why a texture read is malformed: its result is neither a scalar nor a vector.
const NEITHER_SCALAR_NOR_VECTOR: &str = "a texture read that is neither a scalar nor a vector";

/// The operands of an image expression: each handle in a fixed order.
fn operands(e: &mut Expression) -> Vec<&mut Handle<Expression>> {
    let mut out = Vec::new();
    match e {
        Expression::ImageSample {
            image,
            sampler,
            coordinate,
            array_index,
            offset,
            level,
            depth_ref,
            ..
        } => {
            out.extend([image, sampler, coordinate]);
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
            image,
            coordinate,
            array_index,
            sample,
            level,
        } => {
            out.extend([image, coordinate]);
            out.extend(array_index.as_mut());
            out.extend(sample.as_mut());
            out.extend(level.as_mut());
        }
        _ => {}
    }
    out
}

/// Whether `e` of `f` is known before the program runs (a global, or a constant built of
/// literals), so the callee copies it instead of taking it.
fn copied(f: &Function, e: Handle<Expression>) -> bool {
    match f.expressions[e] {
        Expression::Literal(_)
        | Expression::Constant(_)
        | Expression::ZeroValue(_)
        | Expression::GlobalVariable(_) => true,
        Expression::Compose { ref components, .. } => components.iter().all(|&c| copied(f, c)),
        Expression::Splat { value, .. } => copied(f, value),
        _ => false,
    }
}

fn copy(
    from: &Function,
    e: Handle<Expression>,
    to: &mut Function,
    body: &mut Block,
) -> Handle<Expression> {
    let made = match from.expressions[e] {
        Expression::Compose { ty, ref components } => Expression::Compose {
            ty,
            components: components
                .iter()
                .map(|&c| copy(from, c, to, body))
                .collect(),
        },
        Expression::Splat { size, value } => Expression::Splat {
            size,
            value: copy(from, value, to, body),
        },
        ref other => other.clone(),
    };
    push(to, body, made)
}

/// The function of `module` that `owner` numbers, as [`owners`] numbers them.
fn function_mut(module: &mut Module, owner: usize) -> &mut Function {
    let count = module.functions.len();
    match owner < count {
        true => {
            let handle = module.functions.iter().nth(owner).unwrap().0;
            &mut module.functions[handle]
        }
        false => &mut module.entry_points[owner - count].function,
    }
}

/// Maps the read `h` of a depth texture in `f`. A plain read is widened where it is read as
/// (depth, 0, 0, 1), as Direct3D reads a depth view; that widening takes the mapped components.
/// A gather of component c and a read used as its depth alone (c the first) give the mapped c-th
/// component of (depth, 0, 0, 1) for each texel: the read itself where that component is red,
/// otherwise the constant returned (zero for green, blue and zero, one for alpha and one), for the
/// caller to put in place of the read. A comparison through a mapping whose first component is
/// zero or one gives that constant whatever its function, on the RTX 3080 and on WARP alike.
/// Refused by name, where the devices and vkd3d's tests disagree: a comparison through green, blue
/// or alpha (the RTX 3080 gives their constant, WARP compares the depth), and a comparison gather
/// unless the mapping keeps red in its first and its c-th component (the RTX 3080 compares the
/// depth and maps the result by the first component; vkd3d's tests compare the mapped c-th)
/// (`refusal`: `SwizzledDepth` through a view's mapping, `DepthGatherComponent` through none).
fn depth_mapped(
    f: &mut Function,
    h: Handle<Expression>,
    map: &[ViewComponent],
    refusal: &'static str,
) -> Result<Option<ViewComponent>> {
    use ViewComponent as C;
    let (plain, gather, compared) = match f.expressions[h] {
        Expression::ImageSample {
            depth_ref, gather, ..
        } => (
            depth_ref.is_none() && gather.is_none(),
            gather,
            depth_ref.is_some(),
        ),
        _ => (true, None, false),
    };
    let widened = f.expressions.iter().find_map(|(w, e)| match e {
        Expression::Compose { components, .. }
            if components.len() == map.len() && components[0] == h =>
        {
            Some(w)
        }
        _ => None,
    });
    if let (Some(w), true) = (widened, plain) {
        let Expression::Compose { components, .. } = f.expressions.get_mut(w) else {
            unreachable!("found as a composition");
        };
        let read = components.clone();
        for (to, &from) in components.iter_mut().zip(map) {
            *to = match from {
                C::Zero => read[C::G as usize],
                C::One => read[C::A as usize],
                c => read[c as usize],
            };
        }
        return Ok(None);
    }
    let component = map[gather.map_or(0, |c| c as usize)];
    let agreed = match (gather, component) {
        (_, C::R) => map[0] == C::R,
        (None, C::Zero | C::One) => true,
        _ => false,
    };
    if compared && !agreed {
        return Err(ConvertError::Unsupported(refusal));
    }
    // The read stays a gather of the one component a depth texture has; a constant replaces its
    // result.
    if let Expression::ImageSample { gather, .. } = f.expressions.get_mut(h) {
        *gather = gather.map(|_| naga::SwizzleComponent::X);
    }
    Ok(match component {
        C::R => None,
        C::G | C::B | C::Zero => Some(C::Zero),
        C::A | C::One => Some(C::One),
    })
}

/// Whether global `g` of `module` is a depth texture.
fn is_depth(module: &Module, g: Handle<GlobalVariable>) -> bool {
    matches!(
        module.types[module.global_variables[g].ty].inner,
        TypeInner::Image {
            class: ImageClass::Depth { .. },
            ..
        }
    )
}

/// Whether `e` of `f` gathers a component past the first of a depth texture of `module`.
fn depth_gather_past_red(module: &Module, f: &Function, e: &Expression) -> bool {
    match *e {
        Expression::ImageSample {
            image,
            gather: Some(c),
            ..
        } => {
            c != naga::SwizzleComponent::X
                && global_of(f, image).is_some_and(|g| is_depth(module, g))
        }
        _ => false,
    }
}

pub(super) fn swizzled(
    module: &mut Module,
    mappings: &[(u32, [ViewComponent; 4])],
    reflection: &Reflection,
    outlined: &mut Outlined,
) -> Result<()> {
    use ViewComponent as C;
    const IDENTITY: [ViewComponent; 4] = [C::R, C::G, C::B, C::A];
    if mappings.is_empty()
        && !owners(module).iter().any(|(_, f)| {
            f.expressions
                .iter()
                .any(|(_, e)| depth_gather_past_red(module, f, e))
        })
    {
        return Ok(());
    }
    let mut sites = Vec::new();
    let mut gathers = Vec::new();
    let mut depths = Vec::new();
    for (owner, f) in owners(module) {
        for (h, e) in f.expressions.iter() {
            let (image, gather) = match *e {
                Expression::ImageSample { image, gather, .. } => (image, gather),
                Expression::ImageLoad { image, .. } => (image, None),
                _ => continue,
            };
            let resolved = depth_gather_past_red(module, f, e);
            let g = match global_of(f, image) {
                Some(g) => g,
                None if mappings.is_empty() => continue,
                None => return Err(ConvertError::Unsupported(crate::refusal::TEXTURE_ARGUMENT)),
            };
            let map = slot_of(module, reflection, g, SlotKind::Texture)
                .and_then(|slot| mappings.iter().find(|(s, _)| *s == slot))
                .map_or(IDENTITY, |&(_, map)| map);
            if map == IDENTITY && !resolved {
                continue;
            }
            match module.types[module.global_variables[g].ty].inner {
                TypeInner::Image {
                    class: ImageClass::Sampled { .. },
                    ..
                } => {}
                TypeInner::Image {
                    class: ImageClass::Depth { .. },
                    ..
                } => {
                    depths.push((owner, h, map));
                    continue;
                }
                _ => return Err(ConvertError::Unsupported(SWIZZLED_DEPTH)),
            }
            match gather {
                Some(c) => {
                    let to = map[c as usize];
                    match to {
                        C::Zero | C::One => sites.push((owner, h, [to; 4])),
                        _ => gathers.push((owner, h, to)),
                    }
                }
                None => sites.push((owner, h, map)),
            }
        }
    }
    for (owner, h, map) in depths {
        let refusal = match map == IDENTITY {
            true => DEPTH_GATHER_COMPONENT,
            false => SWIZZLED_DEPTH,
        };
        if let Some(constant) = depth_mapped(function_mut(module, owner), h, &map, refusal)? {
            sites.push((owner, h, map.map(|_| constant)));
        }
    }
    for (owner, h, to) in gathers {
        let component = match to {
            C::R => naga::SwizzleComponent::X,
            C::G => naga::SwizzleComponent::Y,
            C::B => naga::SwizzleComponent::Z,
            _ => naga::SwizzleComponent::W,
        };
        let f = function_mut(module, owner);
        if let Expression::ImageSample { gather, .. } = f.expressions.get_mut(h) {
            *gather = Some(component);
        }
    }
    for (owner, h, map) in sites {
        let f = function(module, owner).clone();
        let mut e = f.expressions[h].clone();
        let all: Vec<Handle<Expression>> = operands(&mut e).into_iter().map(|o| *o).collect();
        let passed: Vec<Handle<Expression>> =
            all.iter().copied().filter(|&o| !copied(&f, o)).collect();
        let mut wanted = passed.clone();
        wanted.push(h);
        let types = types_of(module, &f, &wanted)?;
        let result = *types.last().unwrap();
        let (scalar, vector) = match module.types[result].inner {
            TypeInner::Vector { scalar, .. } => (scalar, true),
            TypeInner::Scalar(scalar) => (scalar, false),
            _ => {
                return Err(ConvertError::Malformed(NEITHER_SCALAR_NOR_VECTOR.into()));
            }
        };
        let mut callee = Function {
            name: Some("swizzled".into()),
            result: Some(FunctionResult {
                ty: result,
                binding: None,
            }),
            ..Default::default()
        };
        let mut body = Block::new();
        for &ty in &types[..passed.len()] {
            callee.arguments.push(FunctionArgument {
                name: None,
                ty,
                binding: None,
            });
        }
        let args: Vec<_> = (0..passed.len() as u32)
            .map(|i| push(&mut callee, &mut body, Expression::FunctionArgument(i)))
            .collect();
        for o in operands(&mut e) {
            *o = match passed.iter().position(|p| p == o) {
                Some(i) => args[i],
                None => copy(&f, *o, &mut callee, &mut body),
            };
        }
        let value = push(&mut callee, &mut body, e);
        let constant = |callee: &mut Function, one: bool| {
            let literal = match (scalar.kind, one) {
                (naga::ScalarKind::Sint, one) => Literal::I32(one as i32),
                (naga::ScalarKind::Uint, one) => Literal::U32(one as u32),
                (_, one) => Literal::F32(one as u32 as f32),
            };
            callee
                .expressions
                .append(Expression::Literal(literal), Span::UNDEFINED)
        };
        // A scalar read is a depth texture's, mapped to a constant by `depth_mapped`.
        let composed = match vector {
            false => constant(&mut callee, map[0] == C::One),
            true => {
                let mut components = Vec::new();
                for c in map {
                    components.push(match c {
                        C::Zero => constant(&mut callee, false),
                        C::One => constant(&mut callee, true),
                        _ => push(
                            &mut callee,
                            &mut body,
                            Expression::AccessIndex {
                                base: value,
                                index: c as u32,
                            },
                        ),
                    });
                }
                push(
                    &mut callee,
                    &mut body,
                    Expression::Compose {
                        ty: result,
                        components,
                    },
                )
            }
        };
        body.push(
            Statement::Return {
                value: Some(composed),
            },
            Span::UNDEFINED,
        );
        callee.body = body;
        outlined.add(owner, h, callee, passed);
    }
    Ok(())
}
