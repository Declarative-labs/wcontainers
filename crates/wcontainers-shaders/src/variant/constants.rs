//! Constant buffers read through a texture (`VariantKey::constants_as_texture`): each load from the
//! buffer becomes a texel fetch of the 16-byte row it lies in, from the RGBA32 unsigned texture the
//! front copies the buffer into ([`CONSTANTS_TEXTURE_WIDTH`] rows to a texture row).

use alloc::vec;
use naga::BinaryOperator::{Add, And, Multiply, ShiftRight};
use naga::{ImageDimension, ScalarKind, SwizzleComponent};
use wcontainers_gpu::CONSTANTS_TEXTURE_WIDTH;

use super::*;

/// One step from a constant buffer's variable to a loaded value: a fixed byte offset, or an index
/// times a stride.
enum Step {
    Fixed(u32),
    Indexed(Handle<Expression>, u32),
}

pub(super) fn constants_as_texture(
    module: &mut Module,
    slots: u16,
    reflection: &mut Reflection,
    outlined: &mut Outlined,
) -> Result<()> {
    if slots == 0 {
        return Ok(());
    }
    let mut next = reflection
        .bindings
        .iter()
        .filter(|b| b.kind == SlotKind::Texture)
        .map(|b| b.binding + b.count)
        .max()
        .unwrap_or(SlotKind::Texture.first_binding());
    let texture_ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Image {
                dim: ImageDimension::D2,
                arrayed: false,
                class: ImageClass::Sampled {
                    kind: ScalarKind::Uint,
                    multi: false,
                },
            },
        },
        Span::UNDEFINED,
    );
    // Each moved buffer: its old variable, now private (nothing writes it, so the pointers into it
    // stay valid until compaction drops them), and its texture.
    let mut moved = Vec::new();
    for b in &mut reflection.bindings {
        if b.kind != SlotKind::Uniform || b.added.is_some() || slots & (1 << b.slot()) == 0 {
            continue;
        }
        if next >= SlotKind::Texture.first_binding() + SlotKind::Texture.slots() {
            return Err(ConvertError::Unsupported("BindingsBeyondLimit"));
        }
        let old = naga::ResourceBinding {
            group: b.set,
            binding: b.binding,
        };
        let (g, var) = module
            .global_variables
            .iter_mut()
            .find(|(_, g)| g.binding == Some(old))
            .ok_or(ConvertError::Malformed(
                "a constant buffer without its variable".into(),
            ))?;
        var.space = AddressSpace::Private;
        var.binding = None;
        let texture = module.global_variables.append(
            GlobalVariable {
                name: Some(format!("constants_{}", b.register)),
                space: AddressSpace::Handle,
                binding: Some(naga::ResourceBinding {
                    group: b.set,
                    binding: next,
                }),
                ty: texture_ty,
                init: None,
                memory_decorations: Default::default(),
            },
            Span::UNDEFINED,
        );
        moved.push((g, texture));
        b.added = Some(AddedBinding::ConstantsAsTexture(b.slot()));
        b.kind = SlotKind::Texture;
        b.binding = next;
        next += 1;
    }

    // Every load through a moved buffer: where it is, the texture, and the path to the value.
    let mut sites = Vec::new();
    for (owner, f) in owners(module) {
        for (h, e) in f.expressions.iter() {
            let Expression::Load { pointer } = *e else {
                continue;
            };
            let mut path = Vec::new();
            let mut at = pointer;
            let root = loop {
                match f.expressions[at] {
                    Expression::AccessIndex { base, index } => {
                        path.push((None, index));
                        at = base;
                    }
                    Expression::Access { base, index } => {
                        path.push((Some(index), 0));
                        at = base;
                    }
                    Expression::GlobalVariable(g) => break Some(g),
                    _ => break None,
                }
            };
            if let Some(&(_, texture)) = moved.iter().find(|(g, _)| Some(*g) == root) {
                path.reverse();
                sites.push((owner, h, root.unwrap(), texture, path));
            }
        }
    }
    for (owner, h, var, texture, path) in sites {
        // The byte offset, by the types along the path.
        let mut ty = module.global_variables[var].ty;
        let mut steps = Vec::new();
        for (index, constant) in path {
            let (step, next) = match (&module.types[ty].inner, index) {
                (TypeInner::Struct { members, .. }, None) => {
                    let m = &members[constant as usize];
                    (Step::Fixed(m.offset), m.ty)
                }
                (TypeInner::Array { base, stride, .. }, None) => {
                    (Step::Fixed(constant * stride), *base)
                }
                (TypeInner::Array { base, stride, .. }, Some(i)) => {
                    (Step::Indexed(i, *stride), *base)
                }
                (TypeInner::Vector { scalar, .. }, i) => {
                    let s = *scalar;
                    let step = match i {
                        Some(i) => Step::Indexed(i, 4),
                        None => Step::Fixed(constant * 4),
                    };
                    (step, super::scalar(module, s))
                }
                _ => return Err(ConvertError::Unsupported("ConstantsAsTextureLayout")),
            };
            steps.push(step);
            ty = next;
        }
        let indices: Vec<_> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Indexed(i, _) => Some(*i),
                Step::Fixed(_) => None,
            })
            .collect();
        let owner_function = super::function(module, owner).clone();
        let index_types = types_of(module, &owner_function, &indices)?;
        let callee = fetch(module, texture, ty, &steps, &index_types)?;
        outlined.add(owner, h, callee, indices);
    }
    Ok(())
}

/// The function that reads the value of type `ty` at `steps` from `texture`, taking each indexed
/// step's index as an argument.
fn fetch(
    module: &mut Module,
    texture: Handle<GlobalVariable>,
    ty: Handle<Type>,
    steps: &[Step],
    index_types: &[Handle<Type>],
) -> Result<Function> {
    let (n, out) = match module.types[ty].inner {
        TypeInner::Scalar(s) => (1, s),
        TypeInner::Vector { size, scalar } => (size as u32, scalar),
        _ => return Err(ConvertError::Unsupported("ConstantsAsTextureLayout")),
    };
    if out.width != 4 {
        return Err(ConvertError::Unsupported("ConstantsAsTextureLayout"));
    }
    let fixed: u32 = steps
        .iter()
        .map(|s| match s {
            Step::Fixed(b) => *b,
            Step::Indexed(..) => 0,
        })
        .sum();
    // The first component is known where every index moves by whole rows.
    let whole_rows = steps
        .iter()
        .all(|s| matches!(s, Step::Fixed(_) | Step::Indexed(_, 16)));
    if n > 1 && (!whole_rows || fixed % 16 / 4 + n > 4) {
        return Err(ConvertError::Unsupported("ConstantsAsTextureLayout"));
    }
    let uvec2 = vector(module, 2, Scalar::U32);
    let mut f = Function {
        name: Some("constants".into()),
        arguments: index_types
            .iter()
            .map(|&ty| FunctionArgument {
                name: None,
                ty,
                binding: None,
            })
            .collect(),
        result: Some(FunctionResult { ty, binding: None }),
        ..Default::default()
    };
    let mut b = Block::new();
    let (f_, b_) = (&mut f, &mut b);
    let u = |f: &mut Function, v: u32| {
        f.expressions
            .append(Expression::Literal(Literal::U32(v)), Span::UNDEFINED)
    };
    let mut bytes = u(f_, fixed);
    let mut argument = 0;
    for s in steps {
        let Step::Indexed(_, stride) = s else {
            continue;
        };
        let mut i = push(f_, b_, Expression::FunctionArgument(argument));
        if module.types[index_types[argument as usize]]
            .inner
            .scalar_kind()
            != Some(ScalarKind::Uint)
        {
            i = push(
                f_,
                b_,
                Expression::As {
                    expr: i,
                    kind: ScalarKind::Uint,
                    convert: Some(4),
                },
            );
        }
        let stride = u(f_, *stride);
        let moved = binary(f_, b_, Multiply, i, stride);
        bytes = binary(f_, b_, Add, bytes, moved);
        argument += 1;
    }
    let four = u(f_, 4);
    let row = binary(f_, b_, ShiftRight, bytes, four);
    let column_mask = u(f_, CONSTANTS_TEXTURE_WIDTH - 1);
    let x = binary(f_, b_, And, row, column_mask);
    let width_bits = u(f_, CONSTANTS_TEXTURE_WIDTH.trailing_zeros());
    let y = binary(f_, b_, ShiftRight, row, width_bits);
    let coordinate = push(
        f_,
        b_,
        Expression::Compose {
            ty: uvec2,
            components: vec![x, y],
        },
    );
    let image = push(f_, b_, Expression::GlobalVariable(texture));
    let level = u(f_, 0);
    let texel = push(
        f_,
        b_,
        Expression::ImageLoad {
            image,
            coordinate,
            array_index: None,
            sample: None,
            level: Some(level),
        },
    );
    let mut value = if n == 1 {
        let two = u(f_, 2);
        let word = binary(f_, b_, ShiftRight, bytes, two);
        let three = u(f_, 3);
        let component = binary(f_, b_, And, word, three);
        push(
            f_,
            b_,
            Expression::Access {
                base: texel,
                index: component,
            },
        )
    } else if n == 4 {
        texel
    } else {
        let first = fixed % 16 / 4;
        let c = [
            SwizzleComponent::X,
            SwizzleComponent::Y,
            SwizzleComponent::Z,
            SwizzleComponent::W,
        ];
        let mut pattern = [SwizzleComponent::X; 4];
        for (k, p) in pattern.iter_mut().enumerate().take(n as usize) {
            *p = c[first as usize + k];
        }
        push(
            f_,
            b_,
            Expression::Swizzle {
                size: if n == 2 {
                    VectorSize::Bi
                } else {
                    VectorSize::Tri
                },
                vector: texel,
                pattern,
            },
        )
    };
    if out.kind != ScalarKind::Uint {
        value = push(
            f_,
            b_,
            Expression::As {
                expr: value,
                kind: out.kind,
                convert: None,
            },
        );
    }
    b.push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
    f.body = b;
    Ok(f)
}
