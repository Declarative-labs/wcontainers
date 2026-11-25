//! Multisampled array textures (`VariantKey::texture_layers`): WGSL has none, and the front holds
//! one as a texture per layer (plan 4.2). The array's variable becomes layer 0, at the slot's own
//! binding; the other layers are added bindings. A load becomes a call of a function switching on
//! the layer (zero past the count, as Direct3D's out-of-range loads); the layer count is the key's.

use naga::{SwitchCase, SwitchValue};

use super::*;

pub(super) fn split(
    module: &mut Module,
    layers: &[(u32, u32)],
    reflection: &mut Reflection,
    outlined: &mut Outlined,
) -> Result<()> {
    let arrays: Vec<Handle<GlobalVariable>> = module
        .global_variables
        .iter()
        .filter(|(_, v)| {
            matches!(
                module.types[v.ty].inner,
                TypeInner::Image {
                    arrayed: true,
                    class: ImageClass::Sampled { multi: true, .. }
                        | ImageClass::Depth { multi: true },
                    ..
                }
            )
        })
        .map(|(g, _)| g)
        .collect();
    if arrays.is_empty() {
        return Ok(());
    }
    let mut next = reflection
        .bindings
        .iter()
        .filter(|b| b.kind == SlotKind::Texture)
        .map(|b| b.binding + b.count)
        .max()
        .unwrap_or(wcontainers_gpu::SLOT_KINDS[SlotKind::Texture as usize].first_binding);
    // Each array with its layers' variables, layer 0 first.
    let mut split = Vec::new();
    for g in arrays {
        let refused = ConvertError::Unsupported(crate::refusal::MULTISAMPLED_ARRAY_TEXTURE);
        let slot = slot_of(module, reflection, g, SlotKind::Texture).ok_or(refused.clone())?;
        let count = layers
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|&(_, n)| n)
            .filter(|&n| n > 0)
            .ok_or(refused)?;
        let var = &module.global_variables[g];
        let TypeInner::Image { dim, class, .. } = module.types[var.ty].inner else {
            unreachable!("an image")
        };
        let ty = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Image {
                    dim,
                    arrayed: false,
                    class,
                },
            },
            Span::UNDEFINED,
        );
        let (set, name) = (
            var.binding.expect("a bound texture").group,
            var.name.clone(),
        );
        module.global_variables.get_mut(g).ty = ty;
        let mut vars = alloc::vec![g];
        for layer in 1..count {
            if next
                >= wcontainers_gpu::SLOT_KINDS[SlotKind::Texture as usize].first_binding
                    + wcontainers_gpu::SLOT_KINDS[SlotKind::Texture as usize].slots
            {
                return Err(ConvertError::Unsupported(
                    crate::refusal::BINDINGS_BEYOND_LIMIT,
                ));
            }
            vars.push(module.global_variables.append(
                GlobalVariable {
                    name: name.as_ref().map(|n| format!("{n}_layer{layer}")),
                    space: AddressSpace::Handle,
                    binding: Some(naga::ResourceBinding {
                        group: set,
                        binding: next,
                    }),
                    ty,
                    init: None,
                    memory_decorations: Default::default(),
                },
                Span::UNDEFINED,
            ));
            reflection.bindings.push(wcontainers_gpu::ReflectedBinding {
                kind: SlotKind::Texture,
                space: 0,
                register: 0,
                count: 1,
                set,
                binding: next,
                added: Some(AddedBinding::TextureLayer(slot, layer)),
                stride: 0,
            });
            next += 1;
        }
        split.push(vars);
    }

    // Loads and layer counts of each array; the other queries read layer 0 as they are.
    let mut sites = Vec::new();
    for (owner, f) in owners(module) {
        for (h, e) in f.expressions.iter() {
            let (image, load) = match *e {
                Expression::ImageLoad {
                    image,
                    coordinate,
                    array_index: Some(layer),
                    sample: Some(sample),
                    level: None,
                } => (image, Some([coordinate, layer, sample])),
                Expression::ImageQuery {
                    image,
                    query: ImageQuery::NumLayers,
                } => (image, None),
                Expression::ImageLoad { image, .. } => {
                    if global_of(f, image).is_some_and(|g| split.iter().any(|v| v[0] == g)) {
                        return Err(ConvertError::Malformed(
                            "a multisampled array load without a layer and sample".into(),
                        ));
                    }
                    continue;
                }
                _ => continue,
            };
            let Some(g) = global_of(f, image) else {
                continue;
            };
            let Some(vars) = split.iter().find(|v| v[0] == g) else {
                continue;
            };
            sites.push((owner, h, vars.clone(), load));
        }
    }
    let u32_ty = scalar(module, Scalar::U32);
    for (owner, h, vars, load) in sites {
        let Some(operands) = load else {
            let mut callee = Function {
                name: Some("texture_layers".into()),
                result: Some(FunctionResult {
                    ty: u32_ty,
                    binding: None,
                }),
                ..Default::default()
            };
            let n = callee.expressions.append(
                Expression::Literal(Literal::U32(vars.len() as u32)),
                Span::UNDEFINED,
            );
            callee
                .body
                .push(Statement::Return { value: Some(n) }, Span::UNDEFINED);
            outlined.add(owner, h, callee, Vec::new());
            continue;
        };
        let mut wanted = operands.to_vec();
        wanted.push(h);
        let f = function(module, owner).clone();
        let types = types_of(module, &f, &wanted)?;
        let signed = matches!(
            module.types[types[1]].inner,
            TypeInner::Scalar(Scalar {
                kind: naga::ScalarKind::Sint,
                ..
            })
        );
        let callee = layer_load(&vars, &types, signed);
        outlined.add(owner, h, callee, operands.to_vec());
    }
    Ok(())
}

/// The function loading a texel of one layer: arguments coordinate, layer and sample, of `types`
/// (the last is the result's); `signed` says whether the layer is an `i32`.
fn layer_load(vars: &[Handle<GlobalVariable>], types: &[Handle<Type>], signed: bool) -> Function {
    let mut f = Function {
        name: Some("layer_load".into()),
        arguments: types[..3]
            .iter()
            .map(|&ty| FunctionArgument {
                name: None,
                ty,
                binding: None,
            })
            .collect(),
        result: Some(FunctionResult {
            ty: types[3],
            binding: None,
        }),
        ..Default::default()
    };
    let args: Vec<_> = (0..3)
        .map(|i| {
            f.expressions
                .append(Expression::FunctionArgument(i), Span::UNDEFINED)
        })
        .collect();
    let var = f.local_variables.append(
        naga::LocalVariable {
            name: None,
            ty: types[3],
            init: None,
        },
        Span::UNDEFINED,
    );
    let pointer = f
        .expressions
        .append(Expression::LocalVariable(var), Span::UNDEFINED);
    let mut cases = Vec::new();
    for (k, &layer) in vars.iter().enumerate() {
        let image = f
            .expressions
            .append(Expression::GlobalVariable(layer), Span::UNDEFINED);
        let value = f.expressions.append(
            Expression::ImageLoad {
                image,
                coordinate: args[0],
                array_index: None,
                sample: Some(args[2]),
                level: None,
            },
            Span::UNDEFINED,
        );
        let mut body = Block::new();
        body.push(
            Statement::Emit(Range::new_from_bounds(value, value)),
            Span::UNDEFINED,
        );
        body.push(Statement::Store { pointer, value }, Span::UNDEFINED);
        cases.push(SwitchCase {
            value: match signed {
                true => SwitchValue::I32(k as i32),
                false => SwitchValue::U32(k as u32),
            },
            body,
            fall_through: false,
        });
    }
    let zero = f
        .expressions
        .append(Expression::ZeroValue(types[3]), Span::UNDEFINED);
    let mut default = Block::new();
    default.push(
        Statement::Store {
            pointer,
            value: zero,
        },
        Span::UNDEFINED,
    );
    cases.push(SwitchCase {
        value: SwitchValue::Default,
        body: default,
        fall_through: false,
    });
    f.body.push(
        Statement::Switch {
            selector: args[1],
            cases,
        },
        Span::UNDEFINED,
    );
    let load = f
        .expressions
        .append(Expression::Load { pointer }, Span::UNDEFINED);
    f.body.push(
        Statement::Emit(Range::new_from_bounds(load, load)),
        Span::UNDEFINED,
    );
    f.body
        .push(Statement::Return { value: Some(load) }, Span::UNDEFINED);
    f
}
