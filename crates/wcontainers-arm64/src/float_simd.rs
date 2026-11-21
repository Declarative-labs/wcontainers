//! The meaning of AdvSIMD floating point, on the paths of [`crate::float`].

use crate::Arm64;
use crate::exact::{
    ESTIMATE_FRACTION_BITS, Operation, SQRT_ESTIMATE_BIASES, WORKING_FRACTION_BITS,
};
use crate::float::{
    Op, Source, binary_fast, by_hook, convert, from_integer, lanes, low, op, paths, round,
    step_fast, to_integer_fast,
};
use crate::instruction::*;
use crate::meaning::{Emitter, Flag};
use crate::simd::{Input, map, replace, split_pairs, width};
use wasm_encoder::{BlockType, InstructionSink, ValType};

/// Lanes and bytes of a same-width shape.
/// Copy a v128 local into a fresh one, applying a float op.
fn applied<E: Emitter<Arm64> + ?Sized>(e: &mut E, x: u32, o: Op, size: u8) -> u32 {
    let y = e.local(ValType::V128);
    let mut s = e.sink();
    s.local_get(x);
    op(&mut s, o, size);
    s.local_set(y);
    y
}

/// How a comparison's NZCV answers a compare instruction.
#[derive(Clone, Copy)]
enum Holds {
    Equal,
    GreaterOrEqual,
    Greater,
    LessOrEqual,
    Less,
}

/// Push the mask of lanes where `a` compared with `b` holds.
fn compare<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    holds: Holds,
    size: u8,
    count: u8,
    a: u32,
    b: u32,
) {
    let signaling = !matches!(holds, Holds::Equal);
    let finish = move |e: &mut E| {
        let mut s = e.sink();
        let (shift, mask, want, equal) = match holds {
            Holds::Equal => (Flag::Z.bit(), 1, 1, true),
            Holds::GreaterOrEqual => (Flag::V.bit(), 3, 2, true),
            Holds::Greater => (Flag::V.bit(), 7, 2, true),
            Holds::LessOrEqual => (Flag::Z.bit(), 3, 0, false),
            Holds::Less => (Flag::N.bit(), 1, 1, true),
        };
        s.i64_const(i64::from(shift))
            .i64_shr_u()
            .i64_const(mask)
            .i64_and()
            .i64_const(want);
        if equal {
            s.i64_eq()
        } else {
            s.i64_ne()
        };
        s.i64_extend_i32_u().i64_const(-1).i64_mul();
    };
    let slow = |e: &mut E| {
        by_hook(
            e,
            Operation::Compare { signaling },
            size,
            size,
            count,
            &[Source::of(a, size), Source::of(b, size)],
            &finish,
        )
    };
    let o = match holds {
        Holds::Equal => Op::Eq,
        Holds::GreaterOrEqual => Op::Ge,
        Holds::Greater => Op::Gt,
        Holds::LessOrEqual => Op::Le,
        Holds::Less => Op::Lt,
    };
    paths(e, ValType::V128, slow, |e| {
        let mut s = e.sink();
        s.local_get(a).local_get(b);
        op(&mut s, o, size);
    });
}

/// The software path's name of a binary operation.
fn operation_of(op: SimdOp) -> Operation {
    use SimdOp as S;
    match op {
        S::FloatAdd | S::FloatAddPairwise | S::FloatAddAcross | S::FloatAbsDiff => Operation::Add,
        S::FloatSub => Operation::Sub,
        S::FloatMul => Operation::Mul,
        S::FloatDiv => Operation::Div,
        S::FloatMax | S::FloatMaxPairwise | S::FloatMaxAcross => Operation::Max,
        S::FloatMin | S::FloatMinPairwise | S::FloatMinAcross => Operation::Min,
        S::FloatMaxNumber | S::FloatMaxNumberPairwise | S::FloatMaxNumberAcross => {
            Operation::MaxNumber
        }
        S::FloatMinNumber | S::FloatMinNumberPairwise | S::FloatMinNumberAcross => {
            Operation::MinNumber
        }
        S::FloatMulExtended => Operation::MulExtended,
        _ => unreachable!("not binary: {op:?}"),
    }
}

/// Push the binary operation on `count` lanes of two locals.
fn binary<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    operation: Operation,
    size: u8,
    count: u8,
    a: u32,
    b: u32,
) {
    lanes(e, operation, size, count, &[a, b], true, &|e| {
        binary_fast(e, operation, size, a, b)
    });
}

/// Write the meaning of an AdvSIMD floating point operation, leaving the destination's new value.
pub(crate) fn emit<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    op_: SimdOp,
    shape: Shape,
    d: u32,
    n: u32,
    m: u32,
    shift: u8,
) {
    use SimdOp as S;
    let size = shape.element;
    let bytes = width(shape);
    let count = bytes / size;
    match op_ {
        S::FloatAdd
        | S::FloatSub
        | S::FloatMul
        | S::FloatDiv
        | S::FloatMax
        | S::FloatMin
        | S::FloatMaxNumber
        | S::FloatMinNumber
        | S::FloatMulExtended => binary(e, operation_of(op_), size, count, n, m),
        S::FloatAbsDiff => {
            binary(e, Operation::Sub, size, count, n, m);
            op(&mut e.sink(), Op::Abs, size);
        }
        S::FloatMulAdd | S::FloatMulSub => {
            let n = if op_ == S::FloatMulSub {
                applied(e, n, Op::Neg, size)
            } else {
                n
            };
            lanes(e, Operation::MulAdd, size, count, &[d, n, m], true, &|e| {
                let mut s = e.sink();
                s.local_get(n).local_get(m).local_get(d);
                if size == 8 {
                    s.f64x2_relaxed_madd();
                } else {
                    s.f32x4_relaxed_madd();
                }
            });
        }
        S::FloatReciprocalStep | S::FloatReciprocalSqrtStep => {
            let sqrt = op_ == S::FloatReciprocalSqrtStep;
            let operation = if sqrt {
                Operation::ReciprocalSqrtStep
            } else {
                Operation::ReciprocalStep
            };
            lanes(e, operation, size, count, &[n, m], true, &|e| {
                step_fast(e, size, n, m, sqrt)
            });
        }
        S::FloatEqual | S::FloatGreaterOrEqual | S::FloatGreater => {
            let holds = match op_ {
                S::FloatEqual => Holds::Equal,
                S::FloatGreaterOrEqual => Holds::GreaterOrEqual,
                _ => Holds::Greater,
            };
            compare(e, holds, size, count, n, m);
        }
        S::FloatAbsGreaterOrEqual | S::FloatAbsGreater => {
            let (a, b) = (applied(e, n, Op::Abs, size), applied(e, m, Op::Abs, size));
            let holds = if op_ == S::FloatAbsGreater {
                Holds::Greater
            } else {
                Holds::GreaterOrEqual
            };
            compare(e, holds, size, count, a, b);
        }
        S::FloatCompareZero(cond) => {
            let zero = e.local(ValType::V128);
            e.sink().v128_const(0).local_set(zero);
            let holds = match cond.0 {
                12 => Holds::Greater,
                10 => Holds::GreaterOrEqual,
                0 => Holds::Equal,
                13 => Holds::LessOrEqual,
                _ => Holds::Less,
            };
            compare(e, holds, size, count, n, zero);
        }
        S::FloatAddPairwise
        | S::FloatMaxPairwise
        | S::FloatMinPairwise
        | S::FloatMaxNumberPairwise
        | S::FloatMinNumberPairwise => {
            let (even, odd) = split_pairs(e, size, shape.q, n, m);
            binary(e, operation_of(op_), size, count, even, odd);
        }
        S::FloatAddAcross
        | S::FloatMaxAcross
        | S::FloatMinAcross
        | S::FloatMaxNumberAcross
        | S::FloatMinNumberAcross => {
            // Arm reduces in halves: the first two, the last two, then those.
            let operation = operation_of(op_);
            let mut v = n;
            let mut remaining = if shape.scalar { 2 } else { 16 / size };
            while remaining > 1 {
                let (even, odd) = split_pairs(e, size, true, v, v);
                binary(e, operation, size, remaining / 2, even, odd);
                v = e.local(ValType::V128);
                e.sink().local_set(v);
                remaining /= 2;
            }
            e.sink().local_get(v);
            low(e, size);
            return;
        }
        S::FloatAbs | S::FloatNegate => {
            let mut s = e.sink();
            s.local_get(n);
            op(
                &mut s,
                if op_ == S::FloatAbs { Op::Abs } else { Op::Neg },
                size,
            );
        }
        S::FloatSqrt => lanes(e, Operation::Sqrt, size, count, &[n], true, &|e| {
            let mut s = e.sink();
            s.local_get(n);
            op(&mut s, Op::Sqrt, size);
        }),
        S::FloatRound(r) => lanes(e, Operation::Round(r), size, count, &[n], true, &|e| {
            round(e, size, n, r)
        }),
        S::FloatToInteger { signed, rounding } => {
            let bits = 8 * size;
            let operation = Operation::ToInteger {
                signed,
                bits,
                rounding,
                fraction_bits: shift,
            };
            let slow = |e: &mut E| {
                by_hook(
                    e,
                    operation,
                    size,
                    size,
                    count,
                    &[Source::of(n, size)],
                    &|_| {},
                )
            };
            paths(e, ValType::V128, slow, |e| {
                e.sink().v128_const(0);
                for lane in 0..count {
                    to_integer_fast(e, n, lane, size, bits, signed, rounding, shift);
                    replace(&mut e.sink(), size, lane);
                }
            });
        }
        S::IntegerToFloat { signed } => from_integer(e, n, size, 8 * size, signed, shift, count),
        S::FloatNarrow { odd } => {
            let count = if shape.scalar { 1 } else { 8 / size };
            convert(e, n, 2 * size, size, count, false, odd);
            if shape.q && !shape.scalar {
                // Into the high half, keeping the low half.
                let converted = e.local(ValType::V128);
                e.sink()
                    .local_set(converted)
                    .local_get(d)
                    .local_get(converted)
                    .i8x16_shuffle(core::array::from_fn(|i| {
                        if i < 8 { i as u8 } else { 8 + i as u8 }
                    }));
                return;
            }
            low(e, if shape.scalar { size } else { 8 });
            return;
        }
        S::FloatWiden => {
            convert(e, n, size, 2 * size, 8 / size, shape.q, false);
            return;
        }
        S::FloatReciprocalEstimate
        | S::FloatReciprocalSqrtEstimate
        | S::FloatReciprocalExponent => {
            let operation = match op_ {
                S::FloatReciprocalEstimate => Operation::ReciprocalEstimate,
                S::FloatReciprocalSqrtEstimate => Operation::ReciprocalSqrtEstimate,
                _ => Operation::ReciprocalExponent,
            };
            let slow = |e: &mut E| {
                by_hook(
                    e,
                    operation,
                    size,
                    size,
                    count,
                    &[Source::of(n, size)],
                    &|_| {},
                )
            };
            paths(e, ValType::V128, slow, |e| {
                let input = Input {
                    local: n,
                    element: size,
                    first: 0,
                    signed: false,
                };
                map(e, size, count, 0, None, &[input], &|e, v, _| {
                    estimate(e, op_, size, v[0]);
                });
            });
        }
        _ => unreachable!("not AdvSIMD floating point: {op_:?}"),
    }
    low(e, bytes);
}

/// Push the fast estimate (frecpe, frsqrte, frecpx) of the float of `size` bytes in the i64 local `x`, by
/// Arm's `FPRecipEstimate`, `FPRSqrtEstimate` and `FPRecpX` with FPCR at its default.
fn estimate<E: Emitter<Arm64> + ?Sized>(e: &mut E, op_: SimdOp, size: u8, x: u32) {
    let (fraction_bits, exponent_bits): (i64, i64) = if size == 8 { (52, 11) } else { (23, 8) };
    let bits = 8 * i64::from(size);
    let sign = 1i64.wrapping_shl(bits as u32 - 1) & if size == 8 { -1 } else { 0xffff_ffff };
    let exponent_mask = ((1i64 << exponent_bits) - 1) << fraction_bits;
    let fraction_mask = (1i64 << fraction_bits) - 1;
    let quiet = 1i64 << (fraction_bits - 1);
    let infinity = exponent_mask;
    let (exponent, fraction, result, r) = (
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
    );
    let mut s = e.sink();
    s.local_get(x)
        .i64_const(fraction_bits)
        .i64_shr_u()
        .i64_const((1 << exponent_bits) - 1)
        .i64_and()
        .local_set(exponent);
    s.local_get(x)
        .i64_const(fraction_mask)
        .i64_and()
        .local_set(fraction);
    let magnitude = |s: &mut InstructionSink<'_>| {
        s.local_get(x)
            .i64_const(!sign & (if size == 8 { -1 } else { 0xffff_ffff }))
            .i64_and();
    };
    let sign_of = |s: &mut InstructionSink<'_>| {
        s.local_get(x).i64_const(sign).i64_and();
    };
    match op_ {
        SimdOp::FloatReciprocalExponent => {
            // Zero: the largest finite exponent; otherwise the exponent inverted, fraction cleared.
            sign_of(&mut s);
            s.i64_const(infinity - (1 << fraction_bits))
                .local_get(x)
                .i64_const(-1)
                .i64_xor()
                .i64_const(exponent_mask)
                .i64_and()
                .local_get(exponent)
                .i64_const(0)
                .i64_eq()
                .select()
                .i64_or()
                .local_set(result);
        }
        SimdOp::FloatReciprocalEstimate => {
            recip_table(e, x, size, exponent, fraction, r);
            let mut s = e.sink();
            s.local_get(r).local_set(result);
            // The sign.
            s.local_get(result)
                .local_get(x)
                .i64_const(sign)
                .i64_and()
                .i64_or()
                .local_set(result);
            // Tiny (below 2^-128 or 2^-1024): infinity, rounding to nearest.
            sign_of(&mut s);
            s.i64_const(infinity).i64_or().local_get(result);
            magnitude(&mut s);
            s.i64_const(if size == 8 { 1 << 50 } else { 1 << 21 })
                .i64_lt_u()
                .select()
                .local_set(result);
            // Zero: infinity with the sign; infinity: zero with the sign.
            sign_of(&mut s);
            s.i64_const(infinity).i64_or().local_get(result);
            magnitude(&mut s);
            s.i64_eqz().select().local_set(result);
            sign_of(&mut s);
            s.local_get(result);
            magnitude(&mut s);
            s.i64_const(infinity).i64_eq().select().local_set(result);
        }
        _ => {
            recip_sqrt_table(e, size, exponent, fraction, r);
            let mut s = e.sink();
            s.local_get(r).local_set(result);
            // Negative: the default not-a-number; infinity: zero; zero: infinity with the sign.
            s.i64_const(infinity | quiet)
                .local_get(result)
                .local_get(x)
                .i64_const(sign)
                .i64_and()
                .i64_const(0)
                .i64_ne()
                .select()
                .local_set(result);
            s.i64_const(0)
                .local_get(result)
                .local_get(x)
                .i64_const(infinity)
                .i64_eq()
                .select()
                .local_set(result);
            sign_of(&mut s);
            s.i64_const(infinity).i64_or().local_get(result);
            magnitude(&mut s);
            s.i64_eqz().select().local_set(result);
        }
    }
    // Not-a-number: quieted.
    let mut s = e.sink();
    s.local_get(x).i64_const(quiet).i64_or().local_get(result);
    magnitude(&mut s);
    s.i64_const(infinity).i64_gt_u().select();
}

/// The table part of `FPRecipEstimate` for a finite, non-zero, not tiny float.
fn recip_table<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    _x: u32,
    size: u8,
    exponent: u32,
    fraction: u32,
    r: u32,
) {
    let (fraction_bits, exponent_bits) = crate::float::parts(size);
    let (fraction_bits, exponent_bits) = (i64::from(fraction_bits), i64::from(exponent_bits));
    let bias = (1 << (exponent_bits - 1)) - 1;
    let w = i64::from(WORKING_FRACTION_BITS);
    let estimate_bits = i64::from(ESTIMATE_FRACTION_BITS);
    let (f52, ex, scaled, result_exponent, estimate) = (
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
    );
    let mut s = e.sink();
    s.local_get(fraction)
        .i64_const(w - fraction_bits)
        .i64_shl()
        .local_set(f52);
    s.local_get(exponent).local_set(ex);
    // A denormal: normalise by one or two places.
    s.local_get(exponent).i64_eqz().if_(BlockType::Empty);
    s.local_get(f52)
        .i64_const(w - 1)
        .i64_shr_u()
        .i64_eqz()
        .if_(BlockType::Empty)
        .i64_const(-1)
        .local_set(ex)
        .local_get(f52)
        .i64_const((1 << (w - 2)) - 1)
        .i64_and()
        .i64_const(2)
        .i64_shl()
        .local_set(f52)
        .else_()
        .local_get(f52)
        .i64_const((1 << (w - 1)) - 1)
        .i64_and()
        .i64_const(1)
        .i64_shl()
        .local_set(f52)
        .end();
    s.end();
    s.local_get(f52)
        .i64_const(w - estimate_bits)
        .i64_shr_u()
        .i64_const(1 << estimate_bits)
        .i64_or()
        .local_set(scaled);
    s.i64_const(2 * bias - 1)
        .local_get(ex)
        .i64_sub()
        .local_set(result_exponent);
    crate::simd::recip_estimate(e, scaled);
    let mut s = e.sink();
    s.i64_const((1 << estimate_bits) - 1)
        .i64_and()
        .i64_const(w - estimate_bits)
        .i64_shl()
        .local_set(estimate);
    // Result exponent 0 or -1: a denormal result.
    s.local_get(result_exponent).i64_eqz().if_(BlockType::Empty);
    s.local_get(estimate)
        .i64_const(1)
        .i64_shr_u()
        .i64_const(1 << (w - 1))
        .i64_or()
        .local_set(estimate);
    s.end();
    s.local_get(result_exponent)
        .i64_const(-1)
        .i64_eq()
        .if_(BlockType::Empty);
    s.local_get(estimate)
        .i64_const(2)
        .i64_shr_u()
        .i64_const(1 << (w - 2))
        .i64_or()
        .local_set(estimate)
        .i64_const(0)
        .local_set(result_exponent);
    s.end();
    s.local_get(result_exponent)
        .i64_const((1 << exponent_bits) - 1)
        .i64_and()
        .i64_const(fraction_bits)
        .i64_shl()
        .local_get(estimate)
        .i64_const(w - fraction_bits)
        .i64_shr_u()
        .i64_or()
        .local_set(r);
}

/// The table part of `FPRSqrtEstimate` for a positive, finite, non-zero float.
fn recip_sqrt_table<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    size: u8,
    exponent: u32,
    fraction: u32,
    r: u32,
) {
    let (fraction_bits, exponent_bits) = crate::float::parts(size);
    let (fraction_bits, exponent_bits) = (i64::from(fraction_bits), i64::from(exponent_bits));
    let bias = (1 << (exponent_bits - 1)) - 1;
    let w = i64::from(WORKING_FRACTION_BITS);
    let estimate_bits = i64::from(ESTIMATE_FRACTION_BITS);
    let (f52, ex, scaled, shift) = (
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
    );
    let mut s = e.sink();
    s.local_get(fraction)
        .i64_const(w - fraction_bits)
        .i64_shl()
        .local_set(f52);
    s.local_get(exponent).local_set(ex);
    // A denormal: shift until the leading one is out, counting down the exponent.
    s.local_get(exponent).i64_eqz().if_(BlockType::Empty);
    s.local_get(f52)
        .i64_clz()
        .i64_const(i64::from(u64::BITS) - w - 1)
        .i64_sub()
        .local_set(shift);
    s.local_get(f52)
        .local_get(shift)
        .i64_shl()
        .i64_const((1 << w) - 1)
        .i64_and()
        .local_set(f52);
    s.local_get(ex)
        .local_get(shift)
        .i64_sub()
        .i64_const(1)
        .i64_add()
        .local_set(ex);
    s.end();
    // Even exponent: 01:fraction<51:44>; odd: 001:fraction<51:45>.
    s.local_get(f52)
        .i64_const(w - estimate_bits)
        .i64_shr_u()
        .i64_const(1 << estimate_bits)
        .i64_or()
        .local_get(f52)
        .i64_const(w - estimate_bits + 1)
        .i64_shr_u()
        .i64_const(1 << (estimate_bits - 1))
        .i64_or()
        .local_get(ex)
        .i64_const(1)
        .i64_and()
        .i64_eqz()
        .select()
        .local_set(scaled);
    crate::simd::recip_sqrt_estimate(e, scaled);
    let mut s = e.sink();
    s.i64_const((1 << estimate_bits) - 1)
        .i64_and()
        .i64_const(fraction_bits - estimate_bits)
        .i64_shl()
        .i64_const(SQRT_ESTIMATE_BIASES * bias - 1)
        .local_get(ex)
        .i64_sub()
        .i64_const(1)
        .i64_shr_s()
        .i64_const((1 << exponent_bits) - 1)
        .i64_and()
        .i64_const(fraction_bits)
        .i64_shl()
        .i64_or()
        .local_set(r);
}
