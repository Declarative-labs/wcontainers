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
use crate::build;

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

/// Bits of the words the program reads.
const WORD_BITS: u32 = u32::BITS;
/// Field widths of the packed formats: red, green and blue of 10-10-10-2 and its alpha; red and
/// green of 11-11-10 and its blue.
const RGB10_BITS: u32 = 10;
const A2_BITS: u32 = 2;
const RG11_BITS: u32 = 11;
const B10_BITS: u32 = 10;
/// The bytes of red, green, blue and alpha in a BGRA element.
const BGRA_BYTES: [u32; 4] = [2, 1, 0, 3];
/// The generated fetch function's name before its location, and its arguments.
const FETCH_FUNCTION: &str = "pull_";
const FETCH_ARGUMENTS: [&str; 2] = ["vertex", "instance"];
/// The name of a pulled input's buffer before its location.
const PULLED_BUFFER: &str = "pulled_";

/// A format's kind, components in RGBA order and element size in bytes.
pub(crate) fn layout(format: VertexFormat) -> (Kind, Vec<Component>, u32) {
    use Kind::*;
    use VertexFormat as V;
    // A format packed into one word, its fields from bit 0 up.
    let packed = |kind, widths: &[u32]| {
        let mut shift = 0;
        let c = widths
            .iter()
            .map(|&bits| {
                shift += bits;
                (0, shift - bits, bits)
            })
            .collect();
        (kind, c, build::WORD)
    };
    let (kind, bits) = match format {
        V::Bgra8Unorm => {
            let c = BGRA_BYTES.iter().map(|&b| (b, 0, u8::BITS)).collect();
            return (Unorm, c, build::WORD);
        }
        V::Unorm10_10_10_2 => {
            return packed(Unorm, &[RGB10_BITS, RGB10_BITS, RGB10_BITS, A2_BITS]);
        }
        V::Uint10_10_10_2 => return packed(Uint, &[RGB10_BITS, RGB10_BITS, RGB10_BITS, A2_BITS]),
        V::Float11_11_10 => return packed(Float, &[RG11_BITS, RG11_BITS, B10_BITS]),
        V::Uint8x1 | V::Uint8x2 | V::Uint8x3 | V::Uint8x4 => (Uint, u8::BITS),
        V::Sint8x1 | V::Sint8x2 | V::Sint8x3 | V::Sint8x4 => (Sint, u8::BITS),
        V::Unorm8x1 | V::Unorm8x2 | V::Unorm8x3 | V::Unorm8x4 => (Unorm, u8::BITS),
        V::Snorm8x1 | V::Snorm8x2 | V::Snorm8x3 | V::Snorm8x4 => (Snorm, u8::BITS),
        V::Uint16x1 | V::Uint16x2 | V::Uint16x3 | V::Uint16x4 => (Uint, u16::BITS),
        V::Sint16x1 | V::Sint16x2 | V::Sint16x3 | V::Sint16x4 => (Sint, u16::BITS),
        V::Unorm16x1 | V::Unorm16x2 | V::Unorm16x3 | V::Unorm16x4 => (Unorm, u16::BITS),
        V::Snorm16x1 | V::Snorm16x2 | V::Snorm16x3 | V::Snorm16x4 => (Snorm, u16::BITS),
        V::Float16x1 | V::Float16x2 | V::Float16x3 | V::Float16x4 => (Float, u16::BITS),
        V::Float32x1 | V::Float32x2 | V::Float32x3 | V::Float32x4 => (Float, u32::BITS),
        V::Uint32x1 | V::Uint32x2 | V::Uint32x3 | V::Uint32x4 => (Uint, u32::BITS),
        V::Sint32x1 | V::Sint32x2 | V::Sint32x3 | V::Sint32x4 => (Sint, u32::BITS),
    };
    let count = match format {
        V::Uint8x1 | V::Sint8x1 | V::Unorm8x1 | V::Snorm8x1 | V::Uint16x1 | V::Sint16x1 => 1,
        V::Unorm16x1 | V::Snorm16x1 | V::Float16x1 | V::Float32x1 | V::Uint32x1 | V::Sint32x1 => 1,
        V::Uint8x2 | V::Sint8x2 | V::Unorm8x2 | V::Snorm8x2 | V::Uint16x2 | V::Sint16x2 => 2,
        V::Unorm16x2 | V::Snorm16x2 | V::Float16x2 | V::Float32x2 | V::Uint32x2 | V::Sint32x2 => 2,
        V::Uint8x3 | V::Sint8x3 | V::Unorm8x3 | V::Snorm8x3 | V::Uint16x3 | V::Sint16x3 => 3,
        V::Unorm16x3 | V::Snorm16x3 | V::Float16x3 | V::Float32x3 | V::Uint32x3 | V::Sint32x3 => 3,
        _ => VectorSize::Quad as u32,
    };
    let bytes = bits / u8::BITS;
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
                stride: build::WORD,
            },
        },
        Span::UNDEFINED,
    );
    let vec4u = vector(module, VectorSize::Quad as u8, Scalar::U32);
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
            return Err(ConvertError::Unsupported(
                crate::refusal::BINDINGS_BEYOND_LIMIT,
            ));
        }
        let buffer = module.global_variables.append(
            GlobalVariable {
                name: Some(format!("{PULLED_BUFFER}{location}")),
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
            stride: 0,
        });
        next += 1;
        let stream = helper(
            module,
            reflection,
            ShaderStage::Vertex,
            HelperValue::PulledStream { input: location },
            vec4u,
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
        TypeInner::Scalar(s) => (s, None),
        TypeInner::Vector { size, scalar } => (scalar, Some(size)),
        _ => return Err(ConvertError::Unsupported(crate::refusal::PULLED_INPUT_TYPE)),
    };
    if u32::from(out.width) != build::WORD {
        return Err(ConvertError::Unsupported(
            crate::refusal::PULLED_INPUT_WIDTH,
        ));
    }
    let (kind, components, size) = layout(format);
    let natural = match kind {
        Kind::Uint => Scalar::U32,
        Kind::Sint => Scalar::I32,
        _ => Scalar::F32,
    };
    let natural_vec = vector(module, VectorSize::Quad as u8, natural);
    let u32_ty = scalar(module, Scalar::U32);
    let mut f = Function {
        name: Some(format!("{FETCH_FUNCTION}{location}")),
        arguments: FETCH_ARGUMENTS
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
    let [offset, stride, divisor, bias]: [_; VectorSize::Quad as usize] =
        core::array::from_fn(|i| index(f_, b_, s, i as u32));

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
    let word = u(f_, build::WORD);
    let length = binary(f_, b_, Multiply, count, word);
    let size = u(f_, size);
    let fits = binary(f_, b_, GreaterEqual, length, size);
    let last = binary(f_, b_, Subtract, length, size);
    let within = binary(f_, b_, LessEqual, address, last);
    let inside = binary(f_, b_, LogicalAnd, fits, within);

    let mut values = Vec::new();
    for &(byte, shift, bits) in &components {
        let at = u(f_, byte);
        let a = binary(f_, b_, Add, address, at);
        let word_shift = u(f_, build::WORD_SHIFT);
        let word_index = binary(f_, b_, ShiftRight, a, word_shift);
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
        if bits < WORD_BITS {
            let in_word = u(f_, build::WORD - 1);
            let byte_in_word = binary(f_, b_, And, a, in_word);
            let byte_shift = u(f_, build::BYTE_SHIFT);
            let bit_in_word = binary(f_, b_, ShiftLeft, byte_in_word, byte_shift);
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
    let quad = VectorSize::Quad as usize;
    for i in 0..quad {
        let default = f_.expressions.append(
            Expression::Literal(lit((i == quad - 1) as u32)),
            Span::UNDEFINED,
        );
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
        None => index(f_, b_, value, 0),
        Some(size @ (VectorSize::Bi | VectorSize::Tri)) => push(
            f_,
            b_,
            Expression::Swizzle {
                size,
                vector: value,
                pattern: [
                    SwizzleComponent::X,
                    SwizzleComponent::Y,
                    SwizzleComponent::Z,
                    SwizzleComponent::W,
                ],
            },
        ),
        Some(VectorSize::Quad) => value,
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
        if bits == WORD_BITS {
            return cast(f, body, raw, ScalarKind::Sint, None);
        }
        let by = u(f, WORD_BITS - bits);
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
        Kind::Float if bits == WORD_BITS => cast(f, body, raw, ScalarKind::Float, None),
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
    let above = u(f, WORD_BITS - bits);
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
    let mut top = WORD_BITS as i32 - bits as i32;
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
    // The leading one at bit 31 is 2^-1 before the leading zeros.
    let exponent_top = u(f, build::F32_EXPONENT_BIAS - 1);
    let exponent = binary(f, body, Subtract, exponent_top, lz);
    let at = u(f, build::F32_FRACTION_BITS);
    let exponent = binary(f, body, ShiftLeft, exponent, at);
    let below = WORD_BITS - build::F32_SIGNIFICAND_BITS;
    let past = u(f, below);
    let mantissa = binary(f, body, ShiftRight, window, past);
    let fraction_mask = u(f, build::F32_FRACTION_MASK);
    let mantissa = binary(f, body, And, mantissa, fraction_mask);
    let next = u(f, below - 1);
    let round = binary(f, body, ShiftRight, window, next);
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
    let signed = bits == build::HALF_BITS;
    let m = bits - build::SMALL_FLOAT_EXPONENT_BITS - u32::from(signed);
    let by = u(f, m);
    let exponent = binary(f, body, ShiftRight, raw, by);
    let exponent_mask = u(f, build::SMALL_FLOAT_EXPONENT_MASK);
    let exponent = binary(f, body, And, exponent, exponent_mask);
    let mask = u(f, (1 << m) - 1);
    let mantissa = binary(f, body, And, raw, mask);
    let to_f32 = u(f, build::F32_FRACTION_BITS - m);
    let fraction = binary(f, body, ShiftLeft, mantissa, to_f32);
    // Normal: the exponent rebiased to f32's.
    let rebias = u(
        f,
        build::F32_EXPONENT_BIAS - build::SMALL_FLOAT_EXPONENT_BIAS,
    );
    let e = binary(f, body, Add, exponent, rebias);
    let fraction_bits = u(f, build::F32_FRACTION_BITS);
    let e = binary(f, body, ShiftLeft, e, fraction_bits);
    let normal = binary(f, body, InclusiveOr, e, fraction);
    // Infinity or NaN.
    let all = u(f, build::F32_EXPONENT_MASK);
    let special = binary(f, body, InclusiveOr, all, fraction);
    // Denormal: the mantissa times 2^-(bias - 1 + m), exact in f32.
    let as_float = cast(f, body, mantissa, ScalarKind::Float, Some(build::F32_WIDTH));
    let scale = f.expressions.append(
        Expression::Literal(Literal::F32(
            1.0 / (1u64 << (build::SMALL_FLOAT_EXPONENT_BIAS - 1 + m)) as f32,
        )),
        Span::UNDEFINED,
    );
    let denormal = binary(f, body, Multiply, as_float, scale);
    let denormal = cast(f, body, denormal, ScalarKind::Uint, None);
    let top = u(f, build::SMALL_FLOAT_EXPONENT_MASK);
    let is_special = binary(f, body, Equal, exponent, top);
    let zero = u(f, 0);
    let is_denormal = binary(f, body, Equal, exponent, zero);
    let big = select(f, body, is_special, special, normal);
    let value = select(f, body, is_denormal, denormal, big);
    if !signed {
        return value;
    }
    let sign_at = u(f, build::HALF_BITS - 1);
    let sign = binary(f, body, ShiftRight, raw, sign_at);
    let one = u(f, 1);
    let sign = binary(f, body, And, sign, one);
    let f32_sign_at = u(f, WORD_BITS - 1);
    let sign = binary(f, body, ShiftLeft, sign, f32_sign_at);
    binary(f, body, InclusiveOr, value, sign)
}
