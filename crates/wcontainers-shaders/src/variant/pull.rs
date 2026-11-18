//! Vertex inputs fetched in the program (`VariantKey::pulled`): each from its own read-only storage
//! buffer of words, decoded from its format, with Direct3D's bounds (an element past the end of the
//! bound range reads zero in its components and the defaults in the ones its format lacks).

use alloc::vec;
use naga::BinaryOperator::{
    Add, And, Divide, Equal, GreaterEqual, LessEqual, LogicalAnd, Multiply, ShiftLeft, ShiftRight,
    Subtract,
};
use naga::{ArraySize, ScalarKind, SwizzleComponent};
use wcontainers_gpu::{ReflectedBinding, VertexFormat};

use super::*;

/// How a format's components are read and what they become.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Uint,
    Sint,
    Unorm,
    Snorm,
    Float,
}

/// One component: byte offset in the element, bit offset in the word read there, and width in bits.
pub(crate) type Component = (u32, u32, u32);

/// A format's kind, components in RGBA order and element size in bytes.
pub(crate) fn layout(format: VertexFormat) -> (Kind, Vec<Component>, u32) {
    use Kind::*;
    use VertexFormat as V;
    let packed = |kind, fields: &[(u32, u32)]| {
        let c = fields
            .iter()
            .map(|&(shift, bits)| (0, shift, bits))
            .collect();
        (kind, c, 4)
    };
    let (kind, bits) = match format {
        V::Bgra8Unorm => {
            return (Unorm, vec![(2, 0, 8), (1, 0, 8), (0, 0, 8), (3, 0, 8)], 4);
        }
        V::Unorm10_10_10_2 => return packed(Unorm, &[(0, 10), (10, 10), (20, 10), (30, 2)]),
        V::Uint10_10_10_2 => return packed(Uint, &[(0, 10), (10, 10), (20, 10), (30, 2)]),
        V::Float11_11_10 => return packed(Float, &[(0, 11), (11, 11), (22, 10)]),
        V::Uint8x1 | V::Uint8x2 | V::Uint8x3 | V::Uint8x4 => (Uint, 8),
        V::Sint8x1 | V::Sint8x2 | V::Sint8x3 | V::Sint8x4 => (Sint, 8),
        V::Unorm8x1 | V::Unorm8x2 | V::Unorm8x3 | V::Unorm8x4 => (Unorm, 8),
        V::Snorm8x1 | V::Snorm8x2 | V::Snorm8x3 | V::Snorm8x4 => (Snorm, 8),
        V::Uint16x1 | V::Uint16x2 | V::Uint16x3 | V::Uint16x4 => (Uint, 16),
        V::Sint16x1 | V::Sint16x2 | V::Sint16x3 | V::Sint16x4 => (Sint, 16),
        V::Unorm16x1 | V::Unorm16x2 | V::Unorm16x3 | V::Unorm16x4 => (Unorm, 16),
        V::Snorm16x1 | V::Snorm16x2 | V::Snorm16x3 | V::Snorm16x4 => (Snorm, 16),
        V::Float16x1 | V::Float16x2 | V::Float16x3 | V::Float16x4 => (Float, 16),
        V::Float32x1 | V::Float32x2 | V::Float32x3 | V::Float32x4 => (Float, 32),
        V::Uint32x1 | V::Uint32x2 | V::Uint32x3 | V::Uint32x4 => (Uint, 32),
        V::Sint32x1 | V::Sint32x2 | V::Sint32x3 | V::Sint32x4 => (Sint, 32),
    };
    let count = match format {
        V::Uint8x1 | V::Sint8x1 | V::Unorm8x1 | V::Snorm8x1 | V::Uint16x1 | V::Sint16x1 => 1,
        V::Unorm16x1 | V::Snorm16x1 | V::Float16x1 | V::Float32x1 | V::Uint32x1 | V::Sint32x1 => 1,
        V::Uint8x2 | V::Sint8x2 | V::Unorm8x2 | V::Snorm8x2 | V::Uint16x2 | V::Sint16x2 => 2,
        V::Unorm16x2 | V::Snorm16x2 | V::Float16x2 | V::Float32x2 | V::Uint32x2 | V::Sint32x2 => 2,
        V::Uint8x3 | V::Sint8x3 | V::Unorm8x3 | V::Snorm8x3 | V::Uint16x3 | V::Sint16x3 => 3,
        V::Unorm16x3 | V::Snorm16x3 | V::Float16x3 | V::Float32x3 | V::Uint32x3 | V::Sint32x3 => 3,
        _ => 4,
    };
    let bytes = bits / 8;
    let c = (0..count).map(|i| (i * bytes, 0, bits)).collect();
    (kind, c, count * bytes)
}

/// Replaces each pulled input of the vertex program `w` wraps by a call that fetches it, bound and
/// described as [`AddedBinding::PulledStream`] and [`HelperValue::PulledStream`] say.
pub(super) fn pulled(
    module: &mut Module,
    w: &mut Wrapped,
    pulled: &[(u32, VertexFormat)],
    reflection: &mut Reflection,
) -> Result<()> {
    if pulled.is_empty() {
        return Ok(());
    }
    let u32_ty = scalar(module, Scalar::U32);
    let vertex = w.input(
        module,
        |b| *b == Binding::BuiltIn(BuiltIn::VertexIndex),
        Binding::BuiltIn(BuiltIn::VertexIndex),
        u32_ty,
    );
    let instance = w.input(
        module,
        |b| *b == Binding::BuiltIn(BuiltIn::InstanceIndex),
        Binding::BuiltIn(BuiltIn::InstanceIndex),
        u32_ty,
    );
    let words = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Array {
                base: u32_ty,
                size: ArraySize::Dynamic,
                stride: 4,
            },
        },
        Span::UNDEFINED,
    );
    let vec4u = vector(module, 4, Scalar::U32);
    let mut next = reflection
        .bindings
        .iter()
        .filter(|b| b.kind == SlotKind::Storage)
        .map(|b| b.binding + b.count)
        .max()
        .unwrap_or(SlotKind::Storage.first_binding());
    for &(location, format) in pulled {
        let Some((argument, ty)) = w.input_at(module, location)? else {
            continue;
        };
        if next >= SlotKind::Storage.first_binding() + SlotKind::Storage.slots() {
            return Err(ConvertError::Unsupported("BindingsBeyondLimit"));
        }
        let buffer = module.global_variables.append(
            GlobalVariable {
                name: Some(format!("pulled_{location}")),
                space: AddressSpace::Storage {
                    access: StorageAccess::LOAD,
                },
                binding: Some(naga::ResourceBinding {
                    group: 0,
                    binding: next,
                }),
                ty: words,
                init: None,
                memory_decorations: Default::default(),
            },
            Span::UNDEFINED,
        );
        reflection.bindings.push(ReflectedBinding {
            kind: SlotKind::Storage,
            space: 0,
            register: 0,
            count: 1,
            set: 0,
            binding: next,
            added: Some(AddedBinding::PulledStream(location)),
        });
        next += 1;
        let stream = helper(
            module,
            reflection,
            ShaderStage::Vertex,
            HelperValue::PulledStream { input: location },
            vec4u,
            16,
        );
        let fetch = fetch(module, location, format, ty, buffer, stream)?;
        let fetch = module.functions.append(fetch, Span::UNDEFINED);
        let value =
            w.f.expressions
                .append(Expression::CallResult(fetch), Span::UNDEFINED);
        w.prologue.push(
            Statement::Call {
                function: fetch,
                arguments: vec![vertex, instance],
                result: Some(value),
            },
            Span::UNDEFINED,
        );
        w.replace(argument, value);
    }
    Ok(())
}

/// The function `pull_<location>(vertex index, instance index)` giving the input's value as type `ty`.
fn fetch(
    module: &mut Module,
    location: u32,
    format: VertexFormat,
    ty: Handle<Type>,
    buffer: Handle<GlobalVariable>,
    stream: HelperMember,
) -> Result<Function> {
    let (out, n) = match module.types[ty].inner {
        TypeInner::Scalar(s) => (s, 1),
        TypeInner::Vector { size, scalar } => (scalar, size as u32),
        _ => return Err(ConvertError::Unsupported("PulledInputType")),
    };
    if out.width != 4 {
        return Err(ConvertError::Unsupported("PulledInputWidth"));
    }
    let (kind, components, size) = layout(format);
    let natural = match kind {
        Kind::Uint => Scalar::U32,
        Kind::Sint => Scalar::I32,
        _ => Scalar::F32,
    };
    let natural_vec = vector(module, 4, natural);
    let u32_ty = scalar(module, Scalar::U32);
    let mut f = Function {
        name: Some(format!("pull_{location}")),
        arguments: ["vertex", "instance"]
            .map(|name| FunctionArgument {
                name: Some(name.into()),
                ty: u32_ty,
                binding: None,
            })
            .to_vec(),
        result: Some(FunctionResult { ty, binding: None }),
        ..Default::default()
    };
    let mut b = Block::new();
    let (f_, b_) = (&mut f, &mut b);
    let vertex = push(f_, b_, Expression::FunctionArgument(0));
    let instance = push(f_, b_, Expression::FunctionArgument(1));
    let s = load_helper(f_, b_, stream);
    let [offset, stride, divisor, bias] = [0, 1, 2, 3].map(|i| index(f_, b_, s, i));

    // The element: the vertex index, or the instance index stepped by the divisor.
    let one = u(f_, 1);
    let zero = u(f_, 0);
    let d = math(f_, b_, MathFunction::Max, [divisor, one]);
    let biased = binary(f_, b_, Add, instance, bias);
    let stepped = binary(f_, b_, Divide, biased, d);
    let per_vertex = binary(f_, b_, Equal, divisor, zero);
    let element = push(
        f_,
        b_,
        Expression::Select {
            condition: per_vertex,
            accept: vertex,
            reject: stepped,
        },
    );
    let moved = binary(f_, b_, Multiply, element, stride);
    let address = binary(f_, b_, Add, offset, moved);

    // Inside the bound range when all `size` bytes are.
    let base = push(f_, b_, Expression::GlobalVariable(buffer));
    let count = push(f_, b_, Expression::ArrayLength(base));
    let four = u(f_, 4);
    let length = binary(f_, b_, Multiply, count, four);
    let size = u(f_, size);
    let fits = binary(f_, b_, GreaterEqual, length, size);
    let last = binary(f_, b_, Subtract, length, size);
    let within = binary(f_, b_, LessEqual, address, last);
    let inside = binary(f_, b_, LogicalAnd, fits, within);

    let mut values = Vec::new();
    for &(byte, shift, bits) in &components {
        let at = u(f_, byte);
        let a = binary(f_, b_, Add, address, at);
        let two = u(f_, 2);
        let word_index = binary(f_, b_, ShiftRight, a, two);
        let pointer = push(
            f_,
            b_,
            Expression::Access {
                base,
                index: word_index,
            },
        );
        let word = push(f_, b_, Expression::Load { pointer });
        let mut raw = word;
        if bits < 32 {
            let three = u(f_, 3);
            let byte_in_word = binary(f_, b_, And, a, three);
            let bit_in_word = binary(f_, b_, ShiftLeft, byte_in_word, three);
            let field = u(f_, shift);
            let from = binary(f_, b_, Add, bit_in_word, field);
            let shifted = binary(f_, b_, ShiftRight, word, from);
            let mask = u(f_, (1 << bits) - 1);
            raw = binary(f_, b_, And, shifted, mask);
        }
        values.push(decode(f_, b_, kind, raw, bits));
    }
    // The defaults: zero, and one in w.
    let lit = |v: u32| match natural.kind {
        ScalarKind::Float => Literal::F32(v as f32),
        ScalarKind::Sint => Literal::I32(v as i32),
        _ => Literal::U32(v),
    };
    let mut zeroed = Vec::new();
    for i in 0..4 {
        let default = f_
            .expressions
            .append(Expression::Literal(lit((i == 3) as u32)), Span::UNDEFINED);
        if i >= values.len() {
            values.push(default);
            zeroed.push(default);
        } else {
            zeroed.push(
                f_.expressions
                    .append(Expression::Literal(lit(0)), Span::UNDEFINED),
            );
        }
    }
    let read = push(
        f_,
        b_,
        Expression::Compose {
            ty: natural_vec,
            components: values,
        },
    );
    let zeroed = push(
        f_,
        b_,
        Expression::Compose {
            ty: natural_vec,
            components: zeroed,
        },
    );
    let mut value = push(
        f_,
        b_,
        Expression::Select {
            condition: inside,
            accept: read,
            reject: zeroed,
        },
    );
    // As the program declares the input: its scalar kind (the bits kept, as a vertex fetch keeps
    // them) and component count.
    if out.kind != natural.kind {
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
    value = match n {
        1 => index(f_, b_, value, 0),
        2 | 3 => push(
            f_,
            b_,
            Expression::Swizzle {
                size: if n == 2 {
                    VectorSize::Bi
                } else {
                    VectorSize::Tri
                },
                vector: value,
                pattern: [
                    SwizzleComponent::X,
                    SwizzleComponent::Y,
                    SwizzleComponent::Z,
                    SwizzleComponent::W,
                ],
            },
        ),
        _ => value,
    };
    b.push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
    f.body = b;
    Ok(f)
}

/// One component's value from its `bits` raw bits, as Direct3D converts them: normalised values
/// correctly rounded, half floats with their NaN payloads.
pub(super) fn decode(
    f: &mut Function,
    body: &mut Block,
    kind: Kind,
    raw: Handle<Expression>,
    bits: u32,
) -> Handle<Expression> {
    let signed = |f: &mut Function, body: &mut Block| {
        if bits == 32 {
            return cast(f, body, raw, ScalarKind::Sint, None);
        }
        let by = u(f, 32 - bits);
        let up = binary(f, body, ShiftLeft, raw, by);
        let up = cast(f, body, up, ScalarKind::Sint, None);
        binary(f, body, ShiftRight, up, by)
    };
    match kind {
        Kind::Uint => raw,
        Kind::Sint => signed(f, body),
        Kind::Unorm => unorm(f, body, raw, bits),
        Kind::Snorm => {
            // |s| / (2^(bits-1) - 1), the most negative code clamped to -1 by `unorm`'s ceiling.
            let s = signed(f, body);
            let magnitude = math(f, body, MathFunction::Abs, [s, s]);
            let magnitude = cast(f, body, magnitude, ScalarKind::Uint, None);
            let v = unorm(f, body, magnitude, bits - 1);
            let negated = push(
                f,
                body,
                Expression::Unary {
                    op: naga::UnaryOperator::Negate,
                    expr: v,
                },
            );
            let zero = f
                .expressions
                .append(Expression::Literal(Literal::I32(0)), Span::UNDEFINED);
            let negative = binary(f, body, naga::BinaryOperator::Less, s, zero);
            select(f, body, negative, negated, v)
        }
        Kind::Float if bits == 32 => cast(f, body, raw, ScalarKind::Float, None),
        Kind::Float => {
            let float = small_float(f, body, raw, bits);
            cast(f, body, float, ScalarKind::Float, None)
        }
    }
}

pub(super) fn u(f: &mut Function, v: u32) -> Handle<Expression> {
    f.expressions
        .append(Expression::Literal(Literal::U32(v)), Span::UNDEFINED)
}

pub(super) fn cast(
    f: &mut Function,
    body: &mut Block,
    expr: Handle<Expression>,
    kind: ScalarKind,
    convert: Option<u8>,
) -> Handle<Expression> {
    push(
        f,
        body,
        Expression::As {
            expr,
            kind,
            convert,
        },
    )
}

pub(super) fn select(
    f: &mut Function,
    body: &mut Block,
    condition: Handle<Expression>,
    accept: Handle<Expression>,
    reject: Handle<Expression>,
) -> Handle<Expression> {
    push(
        f,
        body,
        Expression::Select {
            condition,
            accept,
            reject,
        },
    )
}

/// `c / (2^bits - 1)` correctly rounded to f32, codes past the largest giving one. Its binary
/// expansion is `c`'s bits repeated without end, so the mantissa is that pattern from its leading
/// one, and the rounding goes up exactly when the bit after the mantissa is set (the bits past it are
/// never all zero, so there is no tie).
fn unorm(
    f: &mut Function,
    body: &mut Block,
    c: Handle<Expression>,
    bits: u32,
) -> Handle<Expression> {
    let max = (1u32 << bits) - 1;
    // Leading zeros within the code's bits.
    let clz = math(f, body, MathFunction::CountLeadingZeros, [c, c]);
    let above = u(f, 32 - bits);
    let lz = binary(f, body, Subtract, clz, above);
    // The pattern from the leading one: the code rotated left by `lz` within its bits.
    let up = binary(f, body, ShiftLeft, c, lz);
    let width = u(f, bits);
    let rest = binary(f, body, Subtract, width, lz);
    let down = binary(f, body, ShiftRight, c, rest);
    let rotated = binary(f, body, naga::BinaryOperator::InclusiveOr, up, down);
    let mask = u(f, max);
    let rotated = binary(f, body, And, rotated, mask);
    // Repeated from bit 31 down.
    let mut window = None;
    let mut top = 32i32 - bits as i32;
    while top > -(bits as i32) {
        let part = if top >= 0 {
            let by = u(f, top as u32);
            binary(f, body, ShiftLeft, rotated, by)
        } else {
            let by = u(f, (-top) as u32);
            binary(f, body, ShiftRight, rotated, by)
        };
        window = Some(match window {
            None => part,
            Some(w) => binary(f, body, naga::BinaryOperator::InclusiveOr, w, part),
        });
        top -= bits as i32;
    }
    let window = window.unwrap();
    let exponent_top = u(f, 126);
    let exponent = binary(f, body, Subtract, exponent_top, lz);
    let at = u(f, 23);
    let exponent = binary(f, body, ShiftLeft, exponent, at);
    let eight = u(f, 8);
    let mantissa = binary(f, body, ShiftRight, window, eight);
    let fraction_mask = u(f, 0x7f_ffff);
    let mantissa = binary(f, body, And, mantissa, fraction_mask);
    let seven = u(f, 7);
    let round = binary(f, body, ShiftRight, window, seven);
    let one = u(f, 1);
    let round = binary(f, body, And, round, one);
    let value = binary(f, body, Add, exponent, mantissa);
    let value = binary(f, body, Add, value, round);
    let value = cast(f, body, value, ScalarKind::Float, None);
    let zero_code = u(f, 0);
    let is_zero = binary(f, body, Equal, c, zero_code);
    let zero = f
        .expressions
        .append(Expression::Literal(Literal::F32(0.0)), Span::UNDEFINED);
    let value = select(f, body, is_zero, zero, value);
    let is_full = binary(f, body, GreaterEqual, c, mask);
    let full = f
        .expressions
        .append(Expression::Literal(Literal::F32(1.0)), Span::UNDEFINED);
    select(f, body, is_full, full, value)
}

/// The f32 bits of a float of five exponent bits and `bits - 5` mantissa bits (11- and 10-bit floats,
/// unsigned), or of a half float (`bits` 16, signed), infinities and NaN payloads kept.
fn small_float(
    f: &mut Function,
    body: &mut Block,
    raw: Handle<Expression>,
    bits: u32,
) -> Handle<Expression> {
    use naga::BinaryOperator::InclusiveOr;
    let m = if bits == 16 { 10 } else { bits - 5 };
    let by = u(f, m);
    let exponent = binary(f, body, ShiftRight, raw, by);
    let five = u(f, 0x1f);
    let exponent = binary(f, body, And, exponent, five);
    let mask = u(f, (1 << m) - 1);
    let mantissa = binary(f, body, And, raw, mask);
    let to_f32 = u(f, 23 - m);
    let fraction = binary(f, body, ShiftLeft, mantissa, to_f32);
    // Normal: the exponent rebiased from 15 to 127.
    let rebias = u(f, 112);
    let e = binary(f, body, Add, exponent, rebias);
    let twenty_three = u(f, 23);
    let e = binary(f, body, ShiftLeft, e, twenty_three);
    let normal = binary(f, body, InclusiveOr, e, fraction);
    // Infinity or NaN.
    let all = u(f, 0x7f80_0000);
    let special = binary(f, body, InclusiveOr, all, fraction);
    // Denormal: the mantissa times 2^-(14 + m), exact in f32.
    let as_float = cast(f, body, mantissa, ScalarKind::Float, Some(4));
    let scale = f.expressions.append(
        Expression::Literal(Literal::F32(1.0 / (1u64 << (14 + m)) as f32)),
        Span::UNDEFINED,
    );
    let denormal = binary(f, body, Multiply, as_float, scale);
    let denormal = cast(f, body, denormal, ScalarKind::Uint, None);
    let top = u(f, 31);
    let is_special = binary(f, body, Equal, exponent, top);
    let zero = u(f, 0);
    let is_denormal = binary(f, body, Equal, exponent, zero);
    let big = select(f, body, is_special, special, normal);
    let value = select(f, body, is_denormal, denormal, big);
    if bits != 16 {
        return value;
    }
    let fifteen = u(f, 15);
    let sign = binary(f, body, ShiftRight, raw, fifteen);
    let one = u(f, 1);
    let sign = binary(f, body, And, sign, one);
    let thirty_one = u(f, 31);
    let sign = binary(f, body, ShiftLeft, sign, thirty_one);
    binary(f, body, InclusiveOr, value, sign)
}
