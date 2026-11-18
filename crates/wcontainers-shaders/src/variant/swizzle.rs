//! Views' component mappings applied in the program (`VariantKey::swizzled`): each load or sample
//! of a mapped slot becomes a call of a function making the same read and composing its result
//! from the mapping; a gather of component c gathers the component c maps to.

use wcontainers_gpu::ViewComponent;

use super::*;

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

pub(super) fn swizzled(
    module: &mut Module,
    mappings: &[(u32, [ViewComponent; 4])],
    reflection: &Reflection,
    outlined: &mut Outlined,
) -> Result<()> {
    use ViewComponent as C;
    if mappings.is_empty() {
        return Ok(());
    }
    let mut sites = Vec::new();
    let mut gathers = Vec::new();
    for (owner, f) in owners(module) {
        for (h, e) in f.expressions.iter() {
            let (image, gather) = match *e {
                Expression::ImageSample { image, gather, .. } => (image, gather),
                Expression::ImageLoad { image, .. } => (image, None),
                _ => continue,
            };
            let g = global_of(f, image).ok_or(ConvertError::Unsupported("TextureArgument"))?;
            let Some(slot) = slot_of(module, reflection, g, SlotKind::Texture) else {
                continue;
            };
            let Some(&(_, map)) = mappings.iter().find(|(s, _)| *s == slot) else {
                continue;
            };
            if map == [C::R, C::G, C::B, C::A] {
                continue;
            }
            match module.types[module.global_variables[g].ty].inner {
                TypeInner::Image {
                    class: ImageClass::Sampled { .. },
                    ..
                } => {}
                _ => return Err(ConvertError::Unsupported("SwizzledDepth")),
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
    for (owner, h, to) in gathers {
        let component = match to {
            C::R => naga::SwizzleComponent::X,
            C::G => naga::SwizzleComponent::Y,
            C::B => naga::SwizzleComponent::Z,
            _ => naga::SwizzleComponent::W,
        };
        let count = module.functions.len();
        let f = match owner < count {
            true => {
                let handle = module.functions.iter().nth(owner).unwrap().0;
                &mut module.functions[handle]
            }
            false => &mut module.entry_points[owner - count].function,
        };
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
        let TypeInner::Vector { scalar, .. } = module.types[result].inner else {
            return Err(ConvertError::Malformed(
                "a texture read that is not a vector".into(),
            ));
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
        let composed = push(
            &mut callee,
            &mut body,
            Expression::Compose {
                ty: result,
                components,
            },
        );
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
