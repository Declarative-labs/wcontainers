//! Stage values laid out as Direct3D lays them out. dxc packs small values into the components of one
//! register and gives arrays and matrices consecutive registers; WGSL takes one value of a scalar or
//! vector type per location. Between stages each register becomes one value: a value alone in it
//! keeps its type, values sharing it become four components; a vertex program's inputs keep their own
//! types and a pixel program's targets take the register's four components (as the target's format may
//! have more than the value, which WebGPU refuses), their arrays split per register.

use alloc::vec::Vec;
use naga::{
    ArraySize, Binding, Expression, FunctionArgument, Handle, Literal, Module, Scalar, ScalarKind,
    ShaderStage, Type, TypeInner, VectorSize,
};

use crate::ConvertError;
use crate::variant::{Output, Wrapped, push, vector};

/// The components of one register.
pub(crate) const COMPONENTS: u32 = VectorSize::Quad as u32;

/// Gives every stage variable of `spirv` the location `register * COMPONENTS + component`, so that
/// naga's reader, which drops SPIR-V's component, keeps each packed value apart and the component known.
pub(crate) fn encode(spirv: &mut rspirv::dr::Module) {
    use super::staged::{DECORATION_LITERAL, DECORATION_TARGET, decorates, id, literal};
    use rspirv::dr::Operand;
    use spirv::Decoration;
    let components: Vec<(u32, u32)> = spirv
        .annotations
        .iter()
        .filter(|i| decorates(i, Decoration::Component))
        .filter_map(|i| Some((id(i, DECORATION_TARGET)?, literal(i, DECORATION_LITERAL)?)))
        .collect();
    for i in spirv.annotations.iter_mut() {
        if !decorates(i, Decoration::Location) {
            continue;
        }
        let target = id(i, DECORATION_TARGET);
        let component = components
            .iter()
            .find(|(t, _)| Some(*t) == target)
            .map_or(0, |c| c.1);
        if let Some(Operand::LiteralBit32(location)) = i.operands.get_mut(DECORATION_LITERAL) {
            *location = *location * COMPONENTS + component;
        }
    }
}

/// The register and first component of an encoded location.
pub(super) fn decode(location: u32) -> (u32, u32) {
    (location / COMPONENTS, location % COMPONENTS)
}

/// One value the program reads or writes at a register, for the reflection: its register, its first
/// component, and its type (a scalar or vector; an array gives one per register).
pub(crate) struct Placed {
    pub(crate) output: bool,
    pub(crate) register: u32,
    pub(crate) component: u32,
    pub(crate) ty: Handle<Type>,
}

/// A value at a location before the rewrite.
struct Value {
    register: u32,
    component: u32,
    /// Its element type and the registers it takes (more than one for an array).
    element: Handle<Type>,
    rows: u32,
    binding: Binding,
    /// The value an output takes (none for an input).
    expression: Option<Handle<Expression>>,
}

fn element_of(module: &Module, ty: Handle<Type>) -> Result<(Handle<Type>, u32), ConvertError> {
    match module.types[ty].inner {
        TypeInner::Array {
            base,
            size: ArraySize::Constant(n),
            ..
        } => Ok((base, n.get())),
        TypeInner::Scalar(_) | TypeInner::Vector { .. } => Ok((ty, 1)),
        _ => Err(ConvertError::Unsupported(crate::refusal::STAGE_VALUE_TYPE)),
    }
}

/// A scalar or vector type's scalar and width.
fn shape(module: &Module, ty: Handle<Type>) -> (Scalar, u32) {
    match module.types[ty].inner {
        TypeInner::Vector { size, scalar } => (scalar, size as u32),
        TypeInner::Scalar(scalar) => (scalar, 1),
        _ => unreachable!("element_of gives scalars and vectors"),
    }
}

fn with_location(binding: &Binding, register: u32) -> Binding {
    let mut binding = binding.clone();
    if let Binding::Location { location, .. } = &mut binding {
        *location = register;
    }
    binding
}

/// The binding of a register shared by `values`: the first one's interpolation (the reader leaves an
/// array's unset: WGSL's default then), its sampling (the reader leaves a non-perspective one's unset:
/// the centre then, as WGSL's default, which WebGPU matches between stages only when written), flat for
/// integers.
fn register_binding(values: &[&Value], register: u32, scalar: Scalar) -> Binding {
    let mut binding = with_location(&values[0].binding, register);
    if let Binding::Location {
        interpolation,
        sampling,
        ..
    } = &mut binding
    {
        if scalar.kind != ScalarKind::Float {
            *interpolation = Some(naga::Interpolation::Flat);
            *sampling = None;
        } else if *interpolation.get_or_insert(naga::Interpolation::Perspective)
            != naga::Interpolation::Flat
        {
            sampling.get_or_insert(naga::Sampling::Center);
        }
    }
    binding
}

/// The type of a register's value between stages: a value alone at its first component keeps its
/// own (as the other stage reads it alone too), else four components of [`register_scalar`].
/// Whether the value is kept whole is the third.
fn register_type(module: &mut Module, values: &[&Value]) -> (Handle<Type>, Scalar, bool) {
    if let [v] = values
        && v.component == 0
    {
        return (v.element, shape(module, v.element).0, true);
    }
    let scalar = register_scalar(module, values);
    (vector(module, COMPONENTS as u8, scalar), scalar, false)
}

/// The scalar one register holds: its values' own where they agree, else 32-bit words read as each.
fn register_scalar(module: &Module, values: &[&Value]) -> Scalar {
    let first = shape(module, values[0].element).0;
    if values.iter().all(|v| shape(module, v.element).0 == first) {
        first
    } else {
        Scalar::U32
    }
}

/// The values of `values` that take `register`, with the row each takes it at.
fn at_register(values: &[Value], register: u32) -> Vec<(&Value, u32)> {
    values
        .iter()
        .filter(|v| (v.register..v.register + v.rows).contains(&register))
        .map(|v| (v, register - v.register))
        .collect()
}

fn registers(values: &[Value]) -> Vec<u32> {
    let mut all: Vec<u32> = values
        .iter()
        .flat_map(|v| v.register..v.register + v.rows)
        .collect();
    all.sort_unstable();
    all.dedup();
    all
}

/// Rewrites the entry point's stage values as Direct3D's registers; returns each value as placed.
pub(crate) fn lay_out(module: &mut Module) -> Result<Vec<Placed>, ConvertError> {
    let stage = module.entry_points[0].stage;
    let mut w = Wrapped::new(module);
    let mut placed = Vec::new();

    // Inputs.
    let mut inputs = Vec::new();
    for (i, a) in w.f.arguments.clone().iter().enumerate() {
        let Some(binding @ Binding::Location { location, .. }) = &a.binding else {
            continue;
        };
        let (register, component) = decode(*location);
        let (element, rows) = element_of(module, a.ty)?;
        for row in 0..rows {
            placed.push(Placed {
                output: false,
                register: register + row,
                component,
                ty: element,
            });
        }
        inputs.push((
            i as u32,
            a.ty,
            Value {
                register,
                component,
                element,
                rows,
                binding: binding.clone(),
                expression: None,
            },
        ));
    }
    if stage == ShaderStage::Fragment {
        // One argument per register.
        let values: Vec<Value> = inputs.iter().map(|(_, _, v)| clone(v)).collect();
        let mut read = Vec::new();
        for register in registers(&values) {
            let sharing: Vec<&Value> = at_register(&values, register)
                .into_iter()
                .map(|(v, _)| v)
                .collect();
            let (ty, scalar, whole) = register_type(module, &sharing);
            w.f.arguments.push(FunctionArgument {
                name: None,
                ty,
                binding: Some(register_binding(&sharing, register, scalar)),
            });
            let n = w.f.arguments.len() as u32 - 1;
            let e = push(&mut w.f, &mut w.prologue, Expression::FunctionArgument(n));
            read.push((register, scalar, e, whole));
        }
        for (argument, ty, v) in &inputs {
            let mut rows = Vec::new();
            for row in 0..v.rows {
                let &(_, scalar, e, whole) = read
                    .iter()
                    .find(|r| r.0 == v.register + row)
                    .expect("every register read is an argument");
                rows.push(match whole {
                    true => e,
                    false => components_of(module, &mut w, e, scalar, v),
                });
            }
            let value = if v.rows > 1 {
                push(
                    &mut w.f,
                    &mut w.prologue,
                    Expression::Compose {
                        ty: *ty,
                        components: rows,
                    },
                )
            } else {
                rows[0]
            };
            w.replace(*argument, value);
        }
    } else {
        // Vertex inputs keep their types; an array takes one argument per register.
        for (argument, ty, v) in &inputs {
            if v.component != 0 {
                return Err(ConvertError::Unsupported(
                    crate::refusal::PACKED_VERTEX_INPUT,
                ));
            }
            if v.rows == 1 {
                w.f.arguments[*argument as usize].binding =
                    Some(with_location(&v.binding, v.register));
                continue;
            }
            let mut rows = Vec::new();
            for row in 0..v.rows {
                w.f.arguments.push(FunctionArgument {
                    name: None,
                    ty: v.element,
                    binding: Some(with_location(&v.binding, v.register + row)),
                });
                let n = w.f.arguments.len() as u32 - 1;
                rows.push(push(
                    &mut w.f,
                    &mut w.prologue,
                    Expression::FunctionArgument(n),
                ));
            }
            let value = push(
                &mut w.f,
                &mut w.prologue,
                Expression::Compose {
                    ty: *ty,
                    components: rows,
                },
            );
            w.replace(*argument, value);
        }
    }

    // Outputs.
    let mut kept = Vec::new();
    let mut values = Vec::new();
    for o in core::mem::take(&mut w.outputs) {
        let Binding::Location { location, .. } = o.binding else {
            kept.push(o);
            continue;
        };
        let (register, component) = decode(location);
        let (element, rows) = element_of(module, o.ty)?;
        for row in 0..rows {
            placed.push(Placed {
                output: true,
                register: register + row,
                component,
                ty: element,
            });
        }
        values.push(Value {
            register,
            component,
            element,
            rows,
            binding: o.binding,
            expression: Some(o.value),
        });
    }
    w.changed |= !values.is_empty();
    if stage == ShaderStage::Vertex {
        for register in registers(&values) {
            let sharing = at_register(&values, register);
            let all: Vec<&Value> = sharing.iter().map(|(v, _)| *v).collect();
            let (ty, scalar, whole) = register_type(module, &all);
            if whole {
                let (v, row) = sharing[0];
                let mut value = v.expression.expect("outputs have values");
                if v.rows > 1 {
                    value = index(&mut w, value, row);
                }
                kept.push(Output {
                    name: None,
                    ty,
                    binding: register_binding(&all, register, scalar),
                    value,
                });
                continue;
            }
            let mut components = Vec::new();
            for c in 0..COMPONENTS {
                let covering = sharing.iter().find(|(v, _)| {
                    let width = shape(module, v.element).1;
                    (v.component..v.component + width).contains(&c)
                });
                let value = match covering {
                    Some(&(v, row)) => {
                        let (own, width) = shape(module, v.element);
                        let mut e = v.expression.expect("outputs have values");
                        if v.rows > 1 {
                            e = index(&mut w, e, row);
                        }
                        if width > 1 {
                            e = index(&mut w, e, c - v.component);
                        }
                        bitcast(&mut w, e, own, scalar)
                    }
                    None => {
                        w.f.expressions
                            .append(Expression::Literal(zero(scalar)), naga::Span::UNDEFINED)
                    }
                };
                components.push(value);
            }
            let ty = vector(module, COMPONENTS as u8, scalar);
            let value = push(
                &mut w.f,
                &mut w.body,
                Expression::Compose { ty, components },
            );
            kept.push(Output {
                name: None,
                ty,
                binding: register_binding(&all, register, scalar),
                value,
            });
        }
    } else {
        // Targets take four components of their own scalar, zero past the value's; an array gives one
        // output per register.
        for v in &values {
            if v.component != 0 {
                return Err(ConvertError::Malformed(
                    "a target not at a register's first component".into(),
                ));
            }
            for row in 0..v.rows {
                let whole = v.expression.expect("outputs have values");
                let value = if v.rows > 1 {
                    index(&mut w, whole, row)
                } else {
                    whole
                };
                let (scalar, width) = shape(module, v.element);
                let ty = vector(module, COMPONENTS as u8, scalar);
                let value = if width == COMPONENTS {
                    value
                } else {
                    let components = (0..COMPONENTS)
                        .map(|c| match (c < width, width) {
                            (true, 1) => value,
                            (true, _) => index(&mut w, value, c),
                            (false, _) => w
                                .f
                                .expressions
                                .append(Expression::Literal(zero(scalar)), naga::Span::UNDEFINED),
                        })
                        .collect();
                    push(
                        &mut w.f,
                        &mut w.body,
                        Expression::Compose { ty, components },
                    )
                };
                kept.push(Output {
                    name: None,
                    ty,
                    binding: with_location(&v.binding, v.register + row),
                    value,
                });
            }
        }
    }
    kept.sort_by_key(|o| order(&o.binding));
    w.outputs = kept;
    w.finish(module);
    order_arguments(&mut module.entry_points[0].function);
    Ok(placed)
}

/// Where a value goes among a stage's values: the position, then the registers in order, then the
/// other built-ins. Direct3D numbers a signature's registers in the order its values are declared, so
/// a vertex program's outputs and a pixel program's inputs written in this order link.
fn order(binding: &Binding) -> (u8, u32) {
    match binding {
        Binding::BuiltIn(naga::BuiltIn::Position { .. }) => (0, 0),
        Binding::Location { location, .. } => (1, *location),
        Binding::BuiltIn(_) => (2, 0),
    }
}

/// Puts the entry point's arguments in [`order`].
fn order_arguments(f: &mut naga::Function) {
    let mut sorted: Vec<usize> = (0..f.arguments.len()).collect();
    sorted.sort_by_key(|&i| f.arguments[i].binding.as_ref().map(order));
    let mut place = alloc::vec![0; sorted.len()];
    for (new, &old) in sorted.iter().enumerate() {
        place[old] = new as u32;
    }
    f.arguments = sorted.iter().map(|&i| f.arguments[i].clone()).collect();
    for (_, e) in f.expressions.iter_mut() {
        if let Expression::FunctionArgument(i) = e {
            *i = place[*i as usize];
        }
    }
}

fn clone(v: &Value) -> Value {
    Value {
        binding: v.binding.clone(),
        ..*v
    }
}

/// The components of register value `register` (of `scalar`) that `v` takes, as `v`'s element type.
fn components_of(
    module: &Module,
    w: &mut Wrapped,
    register: Handle<Expression>,
    scalar: Scalar,
    v: &Value,
) -> Handle<Expression> {
    let (own, width) = shape(module, v.element);
    let e = if width == 1 {
        push(
            &mut w.f,
            &mut w.prologue,
            Expression::AccessIndex {
                base: register,
                index: v.component,
            },
        )
    } else {
        let mut pattern = [naga::SwizzleComponent::X; COMPONENTS as usize];
        for (k, p) in pattern.iter_mut().enumerate().take(width as usize) {
            *p = naga::SwizzleComponent::XYZW[v.component as usize + k];
        }
        push(
            &mut w.f,
            &mut w.prologue,
            Expression::Swizzle {
                size: [VectorSize::Bi, VectorSize::Tri, VectorSize::Quad]
                    .into_iter()
                    .find(|s| *s as u32 == width)
                    .expect("a vector's width"),
                vector: register,
                pattern,
            },
        )
    };
    if own == scalar {
        return e;
    }
    push(
        &mut w.f,
        &mut w.prologue,
        Expression::As {
            expr: e,
            kind: own.kind,
            convert: None,
        },
    )
}

fn index(w: &mut Wrapped, base: Handle<Expression>, index: u32) -> Handle<Expression> {
    push(
        &mut w.f,
        &mut w.body,
        Expression::AccessIndex { base, index },
    )
}

fn bitcast(w: &mut Wrapped, e: Handle<Expression>, from: Scalar, to: Scalar) -> Handle<Expression> {
    if from == to {
        return e;
    }
    push(
        &mut w.f,
        &mut w.body,
        Expression::As {
            expr: e,
            kind: to.kind,
            convert: None,
        },
    )
}

fn zero(s: Scalar) -> Literal {
    match s.kind {
        ScalarKind::Float => Literal::F32(0.0),
        ScalarKind::Sint => Literal::I32(0),
        _ => Literal::U32(0),
    }
}
