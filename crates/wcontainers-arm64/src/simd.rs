//! The meaning of AdvSIMD data processing: WebAssembly SIMD where it has the operation, else element by
//! element on i64 values.

use crate::Arm64;
use crate::float::{low, vector};
use crate::instruction::*;
use crate::meaning::{Control, Emitter};
use wasm_encoder::{BlockType, InstructionSink, ValType};

/// The bytes of a q register, a v128.
pub(crate) const Q_BYTES: u8 = size_of::<u128>() as u8;
/// The bytes of a d register, the low half of a q register.
pub(crate) const D_BYTES: u8 = size_of::<u64>() as u8;
/// The bytes of a word element.
const WORD_BYTES: u8 = size_of::<u32>() as u8;

/// The bytes of a vector register an operation uses: a whole q register, or its low d half.
fn used_bytes(whole: bool) -> u8 {
    if whole { Q_BYTES } else { D_BYTES }
}

/// Push lane `lane` of the v128 on the stack, of `element` bytes, as an i64, sign or zero extended.
pub(crate) fn extract(s: &mut InstructionSink<'_>, element: u8, lane: u8, signed: bool) {
    match (element, signed) {
        (1, false) => s.i8x16_extract_lane_u(lane).i64_extend_i32_u(),
        (1, true) => s.i8x16_extract_lane_s(lane).i64_extend_i32_s(),
        (2, false) => s.i16x8_extract_lane_u(lane).i64_extend_i32_u(),
        (2, true) => s.i16x8_extract_lane_s(lane).i64_extend_i32_s(),
        (WORD_BYTES, false) => s.i32x4_extract_lane(lane).i64_extend_i32_u(),
        (WORD_BYTES, true) => s.i32x4_extract_lane(lane).i64_extend_i32_s(),
        _ => s.i64x2_extract_lane(lane),
    };
}

/// Replace lane `lane` of `element` bytes of the v128 under the i64 on the stack with its low bits.
pub(crate) fn replace(s: &mut InstructionSink<'_>, element: u8, lane: u8) {
    match element {
        1 => s.i32_wrap_i64().i8x16_replace_lane(lane),
        2 => s.i32_wrap_i64().i16x8_replace_lane(lane),
        WORD_BYTES => s.i32_wrap_i64().i32x4_replace_lane(lane),
        _ => s.i64x2_replace_lane(lane),
    };
}

/// A v128 with every element of `element` bytes holding `value`.
fn splat(element: u8, value: u64) -> i128 {
    let mut v = 0u128;
    for i in 0..Q_BYTES / element {
        v |= u128::from(value) << (u8::BITS * u32::from(i) * u32::from(element));
    }
    v as i128
}

/// Whether WebAssembly has the operation on elements of this size.
fn native(op: SimdOp, element: u8) -> bool {
    int_binary(&mut InstructionSink::new(&mut Vec::new()), op, element)
}

/// One input of an element by element operation.
#[derive(Clone, Copy)]
pub(crate) struct Input {
    pub local: u32,
    pub element: u8,
    pub first: u8,
    pub signed: bool,
}

/// Push a v128 built element by element: lanes `out_first..out_first + count` of `out` bytes each get what
/// `f` pushes (an i64) from the inputs' lanes, each in an i64 local; the other lanes come from `base`, or
/// are zero. `f` also gets an i32 local to set when it saturated.
pub(crate) fn map<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    out: u8,
    count: u8,
    out_first: u8,
    base: Option<u32>,
    inputs: &[Input],
    f: &dyn Fn(&mut E, &[u32], u32),
) {
    let values: Vec<u32> = inputs.iter().map(|_| e.local(ValType::I64)).collect();
    // Locals may be reused from earlier instructions, so this one starts cleared.
    let saturated = e.local(ValType::I32);
    e.sink().i32_const(0).local_set(saturated);
    match base {
        Some(b) => e.sink().local_get(b),
        None => e.sink().v128_const(0),
    };
    for lane in 0..count {
        for (input, &value) in inputs.iter().zip(&values) {
            let mut s = e.sink();
            s.local_get(input.local);
            extract(&mut s, input.element, input.first + lane, input.signed);
            s.local_set(value);
        }
        f(e, &values, saturated);
        replace(&mut e.sink(), out, out_first + lane);
    }
    saturation(e, saturated);
}

/// FPSR.QC is set when the local is not zero.
fn saturation<E: Emitter<Arm64> + ?Sized>(e: &mut E, saturated: u32) {
    e.sink().local_get(saturated).if_(BlockType::Empty);
    e.get_control(Control::Fpsr);
    e.sink().i32_const(1 << 27).i32_or();
    e.set_control(Control::Fpsr);
    e.sink().end();
}

/// Push the i64 in `value` (exact as a signed number) clamped to the range of `bits` bits, signed or not,
/// noting saturation.
fn clamp<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    value: u32,
    bits: u32,
    signed: bool,
    saturated: u32,
) {
    clamp_from(e, value, bits, signed, true, saturated);
}

/// [`clamp`] of a value that is exact as a signed number, or (`from_signed` false) as an unsigned one.
fn clamp_from<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    value: u32,
    bits: u32,
    signed: bool,
    from_signed: bool,
    saturated: u32,
) {
    let (min, max) = if signed {
        (-(1i64 << (bits - 1)), (1i64 << (bits - 1)) - 1)
    } else {
        (0, ((1u64 << bits) - 1) as i64)
    };
    if !from_signed {
        // Unsigned: only too large is possible.
        let mut s = e.sink();
        s.local_get(value)
            .i64_const(max)
            .i64_gt_u()
            .local_get(saturated)
            .i32_or()
            .local_set(saturated)
            .i64_const(max)
            .local_get(value)
            .local_get(value)
            .i64_const(max)
            .i64_gt_u()
            .select();
        return;
    }
    let mut s = e.sink();
    s.local_get(value)
        .i64_const(min)
        .i64_lt_s()
        .local_get(value)
        .i64_const(max)
        .i64_gt_s()
        .i32_or()
        .local_get(saturated)
        .i32_or()
        .local_set(saturated);
    s.i64_const(min)
        .i64_const(max)
        .local_get(value)
        .local_get(value)
        .i64_const(max)
        .i64_gt_s()
        .select()
        .local_get(value)
        .i64_const(min)
        .i64_lt_s()
        .select();
}

/// Push the 64 bit saturating sum or difference of two i64 locals.
fn saturate64<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    a: u32,
    b: u32,
    subtract: bool,
    signed: bool,
    saturated: u32,
) {
    let r = e.local(ValType::I64);
    let overflow = e.local(ValType::I32);
    let mut s = e.sink();
    s.local_get(a).local_get(b);
    if subtract {
        s.i64_sub();
    } else {
        s.i64_add();
    }
    s.local_set(r);
    match (signed, subtract) {
        (true, false) => s
            .local_get(a)
            .local_get(r)
            .i64_xor()
            .local_get(b)
            .local_get(r)
            .i64_xor()
            .i64_and()
            .i64_const(0)
            .i64_lt_s(),
        (true, true) => s
            .local_get(a)
            .local_get(b)
            .i64_xor()
            .local_get(a)
            .local_get(r)
            .i64_xor()
            .i64_and()
            .i64_const(0)
            .i64_lt_s(),
        (false, false) => s.local_get(r).local_get(a).i64_lt_u(),
        (false, true) => s.local_get(a).local_get(b).i64_lt_u(),
    };
    s.local_tee(overflow)
        .local_get(saturated)
        .i32_or()
        .local_set(saturated);
    match (signed, subtract) {
        (true, _) => s
            .i64_const(i64::MAX)
            .i64_const(i64::MIN)
            .local_get(a)
            .i64_const(0)
            .i64_ge_s()
            .select(),
        (false, false) => s.i64_const(-1),
        (false, true) => s.i64_const(0),
    };
    s.local_get(r).local_get(overflow).select();
}

/// Push a shift of the i64 `value` (holding a `bits` bit element, extended) by the signed amount in
/// `amount`: left when positive, right (arithmetic when `signed`) when negative, rounding right shifts when
/// `rounding`. Amounts past the width give what the bits shifted out leave. Not saturating.
fn shift_by<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    value: u32,
    amount: u32,
    bits: u32,
    signed: bool,
    rounding: bool,
) {
    let right = e.local(ValType::I64);
    let mut s = e.sink();
    // Left: zero once the amount reaches the width.
    s.local_get(value)
        .local_get(amount)
        .i64_shl()
        .i64_const(0)
        .local_get(amount)
        .i64_const(i64::from(bits))
        .i64_lt_s()
        .select();
    // Right by -amount, clamped where shifting further changes nothing.
    s.i64_const(0).local_get(amount).i64_sub().local_set(right);
    let shifted = |s: &mut InstructionSink<'_>, by: i64| {
        // value >> (right - by), with amounts of 64 or more giving the sign fill or zero.
        s.local_get(value);
        s.local_get(right)
            .i64_const(by)
            .i64_sub()
            .i64_const(63)
            .local_get(right)
            .i64_const(by)
            .i64_sub()
            .i64_const(63)
            .i64_lt_u()
            .select();
        if signed {
            s.i64_shr_s();
        } else {
            s.i64_shr_u();
            // A logical shift of 64 or more gives zero.
            s.i64_const(0)
                .local_get(right)
                .i64_const(by)
                .i64_sub()
                .i64_const(64)
                .i64_lt_u()
                .select();
        }
    };
    shifted(&mut s, 0);
    if rounding {
        shifted(&mut s, 1);
        s.i64_const(1).i64_and().i64_add();
    }
    s.local_get(amount).i64_const(0).i64_ge_s().select();
}

/// Sign or zero extend the low `bits` of the i64 on the stack.
fn extend(s: &mut InstructionSink<'_>, bits: u32, signed: bool) {
    match (bits, signed) {
        (u64::BITS, _) => {}
        (u8::BITS, true) => {
            s.i64_extend8_s();
        }
        (u16::BITS, true) => {
            s.i64_extend16_s();
        }
        (u32::BITS, true) => {
            s.i64_extend32_s();
        }
        (_, false) => {
            s.i64_const(((1u128 << bits) - 1) as i64).i64_and();
        }
        _ => unreachable!(),
    }
}

/// Push a saturating left shift of the i64 `value` (a `bits` bit element, extended as `from_signed`) by
/// the non-negative amount in `amount`, saturating to `to_signed`.
fn saturating_shift_left<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    value: u32,
    amount: u32,
    bits: u32,
    from_signed: bool,
    to_signed: bool,
    saturated: u32,
) {
    let (r, bad) = (e.local(ValType::I64), e.local(ValType::I32));
    let mut s = e.sink();
    // The shifted value, cut to the element and extended back as the result's signedness.
    s.local_get(value)
        .local_get(amount)
        .i64_const(63)
        .local_get(amount)
        .i64_const(63)
        .i64_lt_u()
        .select()
        .i64_shl();
    extend(&mut s, bits, to_signed);
    s.local_set(r);
    // It saturated when shifting back does not give the value, or the amount reaches the width with a
    // value that is not zero, or a negative value goes to an unsigned result.
    s.local_get(r)
        .local_get(amount)
        .i64_const(63)
        .local_get(amount)
        .i64_const(63)
        .i64_lt_u()
        .select();
    if to_signed {
        s.i64_shr_s();
    } else {
        s.i64_shr_u();
    }
    s.local_get(value)
        .i64_ne()
        .local_get(amount)
        .i64_const(i64::from(bits))
        .i64_ge_u()
        .local_get(value)
        .i64_const(0)
        .i64_ne()
        .i32_and()
        .i32_or();
    if from_signed && !to_signed {
        s.local_get(value).i64_const(0).i64_lt_s().i32_or();
    }
    s.local_tee(bad)
        .local_get(saturated)
        .i32_or()
        .local_set(saturated);
    let (min, max) = match (to_signed, bits) {
        (true, u64::BITS) => (i64::MIN, i64::MAX),
        (true, b) => (-(1i64 << (b - 1)), (1i64 << (b - 1)) - 1),
        (false, u64::BITS) => (0, -1),
        (false, b) => (0, (1i64 << b) - 1),
    };
    // Negative values (only of a signed source) saturate down, the rest up.
    s.i64_const(min).i64_const(max);
    if from_signed {
        s.local_get(value).i64_const(0).i64_lt_s();
    } else {
        s.i32_const(0);
    }
    s.select().local_get(r).local_get(bad).select();
}

/// The shuffle that picks, from `a`:`b` (each 8 or 16 bytes used, by `whole`), the even (`odd` false) or
/// odd elements of `element` bytes, packed.
fn pairs(element: u8, whole: bool, odd: bool) -> [u8; Q_BYTES as usize] {
    let per = used_bytes(whole) / element;
    core::array::from_fn(|i| {
        let (lane, byte) = (i as u8 / element, i as u8 % element);
        let pick = 2 * lane + u8::from(odd);
        if lane >= per {
            0
        } else if pick < per {
            pick * element + byte
        } else {
            Q_BYTES + (pick - per) * element + byte
        }
    })
}

/// Push the even and odd elements of `rn`:`rm` into two fresh locals.
pub(crate) fn split_pairs<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    element: u8,
    whole: bool,
    a: u32,
    b: u32,
) -> (u32, u32) {
    let (even, odd) = (e.local(ValType::V128), e.local(ValType::V128));
    for (local, odd_side) in [(even, false), (odd, true)] {
        e.sink()
            .local_get(a)
            .local_get(b)
            .i8x16_shuffle(pairs(element, whole, odd_side))
            .local_set(local);
    }
    (even, odd)
}

/// The shuffle that makes every element a copy of element `index` of `element` bytes.
pub(crate) fn broadcast(element: u8, index: u8) -> [u8; Q_BYTES as usize] {
    core::array::from_fn(|i| index * element + i as u8 % element)
}

fn int_binary(s: &mut InstructionSink<'_>, op: SimdOp, element: u8) -> bool {
    use SimdOp as S;
    match (op, element) {
        (S::Add, 1) => s.i8x16_add(),
        (S::Add, 2) => s.i16x8_add(),
        (S::Add, WORD_BYTES) => s.i32x4_add(),
        (S::Add, _) => s.i64x2_add(),
        (S::Sub, 1) => s.i8x16_sub(),
        (S::Sub, 2) => s.i16x8_sub(),
        (S::Sub, WORD_BYTES) => s.i32x4_sub(),
        (S::Sub, _) => s.i64x2_sub(),
        (S::Mul, 2) => s.i16x8_mul(),
        (S::Mul, WORD_BYTES) => s.i32x4_mul(),
        (S::Equal, 1) => s.i8x16_eq(),
        (S::Equal, 2) => s.i16x8_eq(),
        (S::Equal, WORD_BYTES) => s.i32x4_eq(),
        (S::Equal, _) => s.i64x2_eq(),
        (S::Greater { signed: true }, 1) => s.i8x16_gt_s(),
        (S::Greater { signed: true }, 2) => s.i16x8_gt_s(),
        (S::Greater { signed: true }, WORD_BYTES) => s.i32x4_gt_s(),
        (S::Greater { signed: true }, _) => s.i64x2_gt_s(),
        (S::GreaterOrEqual { signed: true }, 1) => s.i8x16_ge_s(),
        (S::GreaterOrEqual { signed: true }, 2) => s.i16x8_ge_s(),
        (S::GreaterOrEqual { signed: true }, WORD_BYTES) => s.i32x4_ge_s(),
        (S::GreaterOrEqual { signed: true }, _) => s.i64x2_ge_s(),
        (S::Greater { signed: false }, 1) => s.i8x16_gt_u(),
        (S::Greater { signed: false }, 2) => s.i16x8_gt_u(),
        (S::Greater { signed: false }, WORD_BYTES) => s.i32x4_gt_u(),
        (S::GreaterOrEqual { signed: false }, 1) => s.i8x16_ge_u(),
        (S::GreaterOrEqual { signed: false }, 2) => s.i16x8_ge_u(),
        (S::GreaterOrEqual { signed: false }, WORD_BYTES) => s.i32x4_ge_u(),
        (S::Max { signed: true } | S::MaxPairwise { signed: true }, 1) => s.i8x16_max_s(),
        (S::Max { signed: true } | S::MaxPairwise { signed: true }, 2) => s.i16x8_max_s(),
        (S::Max { signed: true } | S::MaxPairwise { signed: true }, WORD_BYTES) => s.i32x4_max_s(),
        (S::Max { signed: false } | S::MaxPairwise { signed: false }, 1) => s.i8x16_max_u(),
        (S::Max { signed: false } | S::MaxPairwise { signed: false }, 2) => s.i16x8_max_u(),
        (S::Max { signed: false } | S::MaxPairwise { signed: false }, WORD_BYTES) => {
            s.i32x4_max_u()
        }
        (S::Min { signed: true } | S::MinPairwise { signed: true }, 1) => s.i8x16_min_s(),
        (S::Min { signed: true } | S::MinPairwise { signed: true }, 2) => s.i16x8_min_s(),
        (S::Min { signed: true } | S::MinPairwise { signed: true }, WORD_BYTES) => s.i32x4_min_s(),
        (S::Min { signed: false } | S::MinPairwise { signed: false }, 1) => s.i8x16_min_u(),
        (S::Min { signed: false } | S::MinPairwise { signed: false }, 2) => s.i16x8_min_u(),
        (S::Min { signed: false } | S::MinPairwise { signed: false }, WORD_BYTES) => {
            s.i32x4_min_u()
        }
        (S::AddPairwise, 1) => s.i8x16_add(),
        (S::AddPairwise, 2) => s.i16x8_add(),
        (S::AddPairwise, WORD_BYTES) => s.i32x4_add(),
        (S::AddPairwise, _) => s.i64x2_add(),
        (
            S::HalvingAdd {
                signed: false,
                rounding: true,
            },
            1,
        ) => s.i8x16_avgr_u(),
        (
            S::HalvingAdd {
                signed: false,
                rounding: true,
            },
            2,
        ) => s.i16x8_avgr_u(),
        _ => return false,
    };
    true
}

/// The scalar meaning of an integer operation on extended i64 lane values, pushing the result.
#[allow(clippy::too_many_arguments)]
fn int_lane<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    op: SimdOp,
    bits: u32,
    v: &[u32],
    saturated: u32,
) {
    use SimdOp as S;
    let (a, b) = (v[0], *v.get(1).unwrap_or(&v[0]));
    let d = *v.get(2).unwrap_or(&v[0]);
    let t = e.local(ValType::I64);
    match op {
        S::Mul => {
            e.sink().local_get(a).local_get(b).i64_mul();
        }
        S::MulAdd => {
            e.sink()
                .local_get(d)
                .local_get(a)
                .local_get(b)
                .i64_mul()
                .i64_add();
        }
        S::MulSub => {
            e.sink()
                .local_get(d)
                .local_get(a)
                .local_get(b)
                .i64_mul()
                .i64_sub();
        }
        S::PolyMul | S::PolyMulLong => {
            let mut s = e.sink();
            s.i64_const(0);
            for i in 0..bits {
                s.local_get(a)
                    .i64_const(i64::from(i))
                    .i64_shl()
                    .i64_const(0)
                    .local_get(b)
                    .i64_const(i64::from(i))
                    .i64_shr_u()
                    .i64_const(1)
                    .i64_and()
                    .i32_wrap_i64()
                    .select()
                    .i64_xor();
            }
        }
        S::Greater { signed: false } => {
            e.sink()
                .local_get(a)
                .local_get(b)
                .i64_gt_u()
                .i64_extend_i32_u()
                .i64_const(-1)
                .i64_mul();
        }
        S::GreaterOrEqual { signed: false } => {
            e.sink()
                .local_get(a)
                .local_get(b)
                .i64_ge_u()
                .i64_extend_i32_u()
                .i64_const(-1)
                .i64_mul();
        }
        S::HalvingAdd { signed, rounding } => {
            let mut s = e.sink();
            s.local_get(a).local_get(b).i64_add();
            if rounding {
                s.i64_const(1).i64_add();
            }
            s.i64_const(1);
            if signed {
                s.i64_shr_s()
            } else {
                s.i64_shr_u()
            };
        }
        S::HalvingSub { .. } => {
            e.sink()
                .local_get(a)
                .local_get(b)
                .i64_sub()
                .i64_const(1)
                .i64_shr_s();
        }
        S::SaturatingAdd { signed } | S::SaturatingSub { signed } => {
            let subtract = matches!(op, S::SaturatingSub { .. });
            if bits == u64::BITS {
                saturate64(e, a, b, subtract, signed, saturated);
            } else {
                let mut s = e.sink();
                s.local_get(a).local_get(b);
                if subtract {
                    s.i64_sub()
                } else {
                    s.i64_add()
                };
                s.local_set(t);
                clamp(e, t, bits, signed, saturated);
            }
        }
        S::ShiftByRegister {
            signed,
            rounding,
            saturating,
        } => {
            let amount = e.local(ValType::I64);
            e.sink().local_get(b).i64_extend8_s().local_set(amount);
            if saturating {
                // Left shifts saturate; right shifts are as without.
                let left = e.local(ValType::I64);
                e.sink()
                    .local_get(amount)
                    .i64_const(0)
                    .local_get(amount)
                    .i64_const(0)
                    .i64_gt_s()
                    .select()
                    .local_set(left);
                saturating_shift_left(e, a, left, bits, signed, signed, saturated);
                shift_by(e, a, amount, bits, signed, rounding);
                e.sink().local_get(amount).i64_const(0).i64_ge_s().select();
            } else {
                shift_by(e, a, amount, bits, signed, rounding);
            }
        }
        S::Max { signed }
        | S::Min { signed }
        | S::MaxPairwise { signed }
        | S::MinPairwise { signed } => {
            let max = matches!(op, S::Max { .. } | S::MaxPairwise { .. });
            let mut s = e.sink();
            s.local_get(a).local_get(b).local_get(a).local_get(b);
            match (signed, max) {
                (true, true) => s.i64_gt_s(),
                (true, false) => s.i64_lt_s(),
                (false, true) => s.i64_gt_u(),
                (false, false) => s.i64_lt_u(),
            };
            s.select();
        }
        S::AbsDiff { .. } | S::AbsDiffLong { .. } => {
            absolute_difference(e, a, b);
        }
        S::AbsDiffAccumulate { .. } | S::AbsDiffAccumulateLong { .. } => {
            e.sink().local_get(d);
            absolute_difference(e, a, b);
            e.sink().i64_add();
        }
        S::DoublingMulHigh { rounding } => {
            // (2ab + round) >> bits as (ab + round / 2) >> (bits - 1); only MIN * MIN saturates.
            let mut s = e.sink();
            s.local_get(a).local_get(b).i64_mul();
            if rounding {
                s.i64_const(1i64 << (bits - 2)).i64_add();
            }
            s.i64_const(i64::from(bits) - 1).i64_shr_s().local_set(t);
            clamp(e, t, bits, true, saturated);
        }
        S::SaturatingAccumulate { signed } => {
            // suqadd: signed destination plus unsigned rn; usqadd: unsigned destination plus signed rn.
            if bits == u64::BITS {
                suqadd64(e, d, a, signed, saturated);
            } else {
                e.sink().local_get(d).local_get(a).i64_add().local_set(t);
                clamp(e, t, bits, signed, saturated);
            }
        }
        S::CountLeadingSign => {
            e.sink()
                .local_get(a)
                .local_get(a)
                .i64_const(63)
                .i64_shr_s()
                .i64_xor()
                .i64_const(1)
                .i64_shl()
                .i64_const(1)
                .i64_or()
                .i64_clz()
                .i64_const(i64::from(u64::BITS - bits))
                .i64_sub();
        }
        S::CountLeadingZeros => {
            e.sink()
                .local_get(a)
                .i64_clz()
                .i64_const(i64::from(u64::BITS - bits))
                .i64_sub();
        }
        S::ReverseBits => {
            // A byte's bits reversed, by multiplication.
            e.sink()
                .local_get(a)
                .i64_const(0x0002_0202_0202)
                .i64_mul()
                .i64_const(0x0108_8442_2010)
                .i64_and()
                .i64_const(1023)
                .i64_rem_u();
        }
        S::SaturatingAbs | S::SaturatingNegate => {
            let negate_only = op == S::SaturatingNegate;
            let mut s = e.sink();
            s.i64_const(0).local_get(a).i64_sub();
            if !negate_only {
                s.local_get(a).local_get(a).i64_const(0).i64_lt_s().select();
            }
            s.local_set(t);
            if bits == u64::BITS {
                // Only the most negative value does not fit.
                let mut s = e.sink();
                s.local_get(a)
                    .i64_const(i64::MIN)
                    .i64_eq()
                    .local_get(saturated)
                    .i32_or()
                    .local_set(saturated)
                    .i64_const(i64::MAX)
                    .local_get(t)
                    .local_get(a)
                    .i64_const(i64::MIN)
                    .i64_eq()
                    .select();
            } else {
                clamp(e, t, bits, true, saturated);
            }
        }
        S::SaturatingNarrow {
            from_signed,
            to_signed,
        } => {
            clamp_from(e, a, bits, to_signed, from_signed, saturated);
        }
        S::ShiftLeftLongByWidth => {
            e.sink().local_get(a).i64_const(i64::from(bits)).i64_shl();
        }
        S::ReciprocalEstimateInteger | S::ReciprocalSqrtEstimateInteger => {
            let sqrt = op == S::ReciprocalSqrtEstimateInteger;
            let mut s = e.sink();
            s.i64_const(0xffff_ffff);
            s.local_get(a).i64_const(23).i64_shr_u().local_set(t);
            if sqrt {
                recip_sqrt_estimate(e, t);
            } else {
                recip_estimate(e, t);
            }
            let mut s = e.sink();
            s.i64_const(0x1ff).i64_and().i64_const(23).i64_shl();
            s.local_get(a)
                .i64_const(if sqrt { 1 << 30 } else { 1 << 31 })
                .i64_lt_u()
                .select();
        }
        S::AddLong { .. } | S::AddWide { .. } => {
            e.sink().local_get(a).local_get(b).i64_add();
        }
        S::SubLong { .. } | S::SubWide { .. } => {
            e.sink().local_get(a).local_get(b).i64_sub();
        }
        S::AddNarrowHigh { rounding } | S::SubNarrowHigh { rounding } => {
            let mut s = e.sink();
            s.local_get(a).local_get(b);
            if matches!(op, S::AddNarrowHigh { .. }) {
                s.i64_add();
            } else {
                s.i64_sub();
            }
            if rounding {
                s.i64_const(1i64 << (bits - 1)).i64_add();
            }
            s.i64_const(i64::from(bits)).i64_shr_u();
        }
        S::MulLong { .. } => {
            e.sink().local_get(a).local_get(b).i64_mul();
        }
        S::MulAddLong { .. } => {
            e.sink()
                .local_get(d)
                .local_get(a)
                .local_get(b)
                .i64_mul()
                .i64_add();
        }
        S::MulSubLong { .. } => {
            e.sink()
                .local_get(d)
                .local_get(a)
                .local_get(b)
                .i64_mul()
                .i64_sub();
        }
        S::DoublingMulLong | S::DoublingMulAddLong | S::DoublingMulSubLong => {
            // 2ab saturates only for MIN * MIN.
            let wide = 2 * bits;
            let mut s = e.sink();
            s.local_get(a).local_get(b).i64_mul().local_set(t);
            if wide == u64::BITS {
                s.local_get(t)
                    .i64_const(1 << 62)
                    .i64_eq()
                    .local_get(saturated)
                    .i32_or()
                    .local_set(saturated)
                    .i64_const(i64::MAX)
                    .local_get(t)
                    .i64_const(1)
                    .i64_shl()
                    .local_get(t)
                    .i64_const(1 << 62)
                    .i64_eq()
                    .select()
                    .local_set(t);
            } else {
                s.local_get(t).i64_const(1).i64_shl().local_set(t);
                clamp(e, t, wide, true, saturated);
                e.sink().local_set(t);
            }
            if op == S::DoublingMulLong {
                e.sink().local_get(t);
            } else {
                let subtract = op == S::DoublingMulSubLong;
                if wide == u64::BITS {
                    saturate64(e, d, t, subtract, true, saturated);
                } else {
                    let mut s = e.sink();
                    s.local_get(d).local_get(t);
                    if subtract {
                        s.i64_sub()
                    } else {
                        s.i64_add()
                    };
                    s.local_set(t);
                    clamp(e, t, wide, true, saturated);
                }
            }
        }
        _ => unreachable!("no element meaning for {op:?}"),
    }
}

fn absolute_difference<E: Emitter<Arm64> + ?Sized>(e: &mut E, a: u32, b: u32) {
    e.sink()
        .local_get(a)
        .local_get(b)
        .i64_sub()
        .local_get(b)
        .local_get(a)
        .i64_sub()
        .local_get(a)
        .local_get(b)
        .i64_gt_s()
        .select();
}

/// 64 bit suqadd (signed destination plus unsigned value) and usqadd (unsigned destination plus signed).
fn suqadd64<E: Emitter<Arm64> + ?Sized>(e: &mut E, d: u32, n: u32, signed: bool, saturated: u32) {
    let (r, bad) = (e.local(ValType::I64), e.local(ValType::I32));
    let mut s = e.sink();
    s.local_get(d).local_get(n).i64_add().local_set(r);
    if signed {
        // n below 2^63: overflow when the sum goes below d. Otherwise: too large when d >= 0 or the sum
        // is negative.
        s.local_get(r)
            .local_get(d)
            .i64_lt_s()
            .local_get(d)
            .i64_const(0)
            .i64_ge_s()
            .local_get(r)
            .i64_const(0)
            .i64_lt_s()
            .i32_or()
            .local_get(n)
            .i64_const(0)
            .i64_ge_s()
            .select()
            .local_tee(bad);
        s.local_get(saturated).i32_or().local_set(saturated);
        s.i64_const(i64::MAX).local_get(r).local_get(bad).select();
    } else {
        // n not negative: overflow when the sum wraps below d. Negative: underflow when it wraps above d.
        s.local_get(r)
            .local_get(d)
            .i64_lt_u()
            .local_get(r)
            .local_get(d)
            .i64_gt_u()
            .local_get(n)
            .i64_const(0)
            .i64_ge_s()
            .select()
            .local_tee(bad);
        s.local_get(saturated).i32_or().local_set(saturated);
        s.i64_const(-1)
            .i64_const(0)
            .local_get(n)
            .i64_const(0)
            .i64_ge_s()
            .select()
            .local_get(r)
            .local_get(bad)
            .select();
    }
}

/// Arm's `RecipEstimate` of the 9 bit value in `a`, pushed.
pub(crate) fn recip_estimate<E: Emitter<Arm64> + ?Sized>(e: &mut E, a: u32) {
    e.sink()
        .i64_const(1 << 19)
        .local_get(a)
        .i64_const(2)
        .i64_mul()
        .i64_const(1)
        .i64_add()
        .i64_div_u()
        .i64_const(1)
        .i64_add()
        .i64_const(1)
        .i64_shr_u();
}

/// Arm's `RecipSqrtEstimate` of the 9 bit value in `a` (128 to 511), pushed: the least b of at least 512
/// with a * (b + 1)^2 at least 2^28, found from a square root and corrected by one either way.
pub(crate) fn recip_sqrt_estimate<E: Emitter<Arm64> + ?Sized>(e: &mut E, a: u32) {
    let (scaled, c) = (e.local(ValType::I64), e.local(ValType::I64));
    let mut s = e.sink();
    s.local_get(a)
        .i64_const(1)
        .i64_shl()
        .i64_const(1)
        .i64_or()
        .local_get(a)
        .i64_const(!1)
        .i64_and()
        .i64_const(1)
        .i64_add()
        .i64_const(2)
        .i64_mul()
        .local_get(a)
        .i64_const(256)
        .i64_lt_u()
        .select()
        .local_set(scaled);
    // c = ceil(sqrt(2^28 / scaled)), the least c with scaled * c^2 >= 2^28.
    s.f64_const((268_435_456.0f64).into())
        .local_get(scaled)
        .f64_convert_i64_u()
        .f64_div()
        .f64_sqrt()
        .f64_ceil()
        .i64_trunc_sat_f64_u()
        .local_set(c);
    for _ in 0..2 {
        // Too small: up by one.
        s.local_get(c)
            .i64_const(1)
            .i64_add()
            .local_get(c)
            .local_get(scaled)
            .local_get(c)
            .i64_mul()
            .local_get(c)
            .i64_mul()
            .i64_const(1 << 28)
            .i64_lt_u()
            .select()
            .local_set(c);
        // One less also works: down by one.
        s.local_get(c)
            .i64_const(1)
            .i64_sub()
            .local_get(c)
            .local_get(scaled)
            .local_get(c)
            .i64_const(1)
            .i64_sub()
            .i64_mul()
            .local_get(c)
            .i64_const(1)
            .i64_sub()
            .i64_mul()
            .i64_const(1 << 28)
            .i64_ge_u()
            .select()
            .local_set(c);
    }
    // b = max(c - 1, 512); the estimate is (b + 1) / 2.
    s.local_get(c)
        .i64_const(1)
        .i64_sub()
        .i64_const(512)
        .local_get(c)
        .i64_const(1)
        .i64_sub()
        .i64_const(512)
        .i64_gt_u()
        .select()
        .i64_const(1)
        .i64_add()
        .i64_const(1)
        .i64_shr_u();
}

/// Write the meaning of an AdvSIMD data processing instruction.
pub(crate) fn emit<E: Emitter<Arm64> + ?Sized>(e: &mut E, instruction: &Instruction) {
    match *instruction {
        Instruction::Simd {
            op,
            shape,
            rd,
            rn,
            rm,
            index,
            shift,
        } => {
            simd(e, op, shape, rd, rn, rm, index, shift);
        }
        Instruction::Permute {
            op,
            element,
            whole,
            rd,
            rn,
            rm,
        } => {
            e.get_vector(rn);
            e.get_vector(rm);
            let lanes = used_bytes(whole) / element;
            let pick: [u8; Q_BYTES as usize] = core::array::from_fn(|i| {
                let (lane, byte) = (i as u8 / element, i as u8 % element);
                if lane >= lanes {
                    return 0;
                }
                // Element number in rn:rm, where rm's start at `lanes`.
                let source = match op {
                    Permute::Zip1 | Permute::Zip2 => {
                        let base = if op == Permute::Zip2 { lanes / 2 } else { 0 };
                        base + lane / 2 + if lane % 2 == 1 { lanes } else { 0 }
                    }
                    Permute::Unzip1 => 2 * lane,
                    Permute::Unzip2 => 2 * lane + 1,
                    Permute::Transpose1 | Permute::Transpose2 => {
                        let t = u8::from(op == Permute::Transpose2);
                        if lane % 2 == 0 {
                            lane + t
                        } else {
                            lanes + lane - 1 + t
                        }
                    }
                };
                let (register, at) = if source < lanes {
                    (0, source)
                } else {
                    (Q_BYTES, source - lanes)
                };
                register + at * element + byte
            });
            e.sink().i8x16_shuffle(pick);
            low(e, used_bytes(whole));
            e.set_vector(rd);
        }
        Instruction::Table {
            extend,
            registers,
            whole,
            rd,
            rn,
            rm,
        } => {
            // Each table register answers the indices in its own 16; out of range gives zero, or with
            // `extend` the destination's byte.
            let (index, result) = (e.local(ValType::V128), e.local(ValType::V128));
            e.get_vector(rm);
            e.sink().local_set(index).v128_const(0).local_set(result);
            for k in 0..registers {
                e.get_vector((rn + k) % 32);
                e.sink()
                    .local_get(index)
                    .v128_const(i128::from_le_bytes([Q_BYTES * k; Q_BYTES as usize]))
                    .i8x16_sub()
                    .i8x16_swizzle()
                    .local_get(result)
                    .v128_or()
                    .local_set(result);
            }
            if extend {
                // Keep the destination where the index is past every table register.
                e.get_vector(rd);
                e.sink()
                    .local_get(result)
                    .local_get(index)
                    .v128_const(i128::from_le_bytes(
                        [Q_BYTES * registers - 1; Q_BYTES as usize],
                    ))
                    .i8x16_gt_u()
                    .v128_bitselect();
            } else {
                e.sink().local_get(result);
            }
            low(e, used_bytes(whole));
            e.set_vector(rd);
        }
        Instruction::ScalarElement {
            bytes,
            index,
            rd,
            rn,
        } => {
            e.get_vector(rn);
            e.sink()
                .v128_const(0)
                .i8x16_shuffle(broadcast(bytes, index));
            low(e, bytes);
            e.set_vector(rd);
        }
        _ => unreachable!("not AdvSIMD data processing: {instruction:?}"),
    }
}

/// Lanes of a same-width operation and the bytes of its result.
fn width(shape: Shape) -> u8 {
    if shape.scalar {
        shape.element
    } else if shape.q {
        Q_BYTES
    } else {
        D_BYTES
    }
}

#[allow(clippy::too_many_arguments)]
fn simd<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    op: SimdOp,
    shape: Shape,
    rd: u8,
    rn: u8,
    rm: u8,
    index: Option<u8>,
    shift: u8,
) {
    use SimdOp as S;
    let element = shape.element;
    let n = vector(e, rn);
    let m = vector(e, rm);
    if let Some(i) = index {
        e.sink()
            .local_get(m)
            .local_get(m)
            .i8x16_shuffle(broadcast(element, i))
            .local_set(m);
    }
    let d = vector(e, rd);
    if op.is_float() {
        crate::float_simd::emit(e, op, shape, d, n, m, shift);
        e.set_vector(rd);
        return;
    }
    let bytes = width(shape);
    let lanes = bytes / element;
    let bits = u8::BITS * u32::from(element);
    let signed = signedness(op);
    let input = |local: u32, element: u8, first: u8, signed: bool| Input {
        local,
        element,
        first,
        signed,
    };
    // The half the long operations read, and the lanes of the wide side.
    let half = if shape.q && !shape.scalar {
        D_BYTES / element
    } else {
        0
    };
    match op {
        S::And | S::AndNot | S::Or | S::OrNot | S::Xor => {
            let mut s = e.sink();
            s.local_get(n).local_get(m);
            match op {
                S::And => s.v128_and(),
                S::AndNot => s.v128_andnot(),
                S::Or => s.v128_or(),
                S::OrNot => s.v128_not().v128_or(),
                _ => s.v128_xor(),
            };
            low(e, bytes);
        }
        S::Select | S::InsertIfTrue | S::InsertIfFalse => {
            let mut s = e.sink();
            match op {
                S::Select => s.local_get(n).local_get(m).local_get(d),
                S::InsertIfTrue => s.local_get(n).local_get(d).local_get(m),
                _ => s.local_get(d).local_get(n).local_get(m),
            };
            s.v128_bitselect();
            low(e, bytes);
        }
        S::Test => {
            let mut s = e.sink();
            s.local_get(n).local_get(m).v128_and().v128_const(0);
            int_binary(&mut s, S::Equal, element);
            s.v128_not();
            low(e, bytes);
        }
        S::Not => {
            e.sink().local_get(n).v128_not();
            low(e, bytes);
        }
        S::CountBits => {
            e.sink().local_get(n).i8x16_popcnt();
            low(e, bytes);
        }
        S::Abs | S::Negate => {
            let mut s = e.sink();
            s.local_get(n);
            match (op, element) {
                (S::Abs, 1) => s.i8x16_abs(),
                (S::Abs, 2) => s.i16x8_abs(),
                (S::Abs, WORD_BYTES) => s.i32x4_abs(),
                (S::Abs, _) => s.i64x2_abs(),
                (_, 1) => s.i8x16_neg(),
                (_, 2) => s.i16x8_neg(),
                (_, WORD_BYTES) => s.i32x4_neg(),
                _ => s.i64x2_neg(),
            };
            low(e, bytes);
        }
        S::CompareZero(cond) => {
            let mut s = e.sink();
            let (first, second, o) = match cond.0 {
                12 => (n, None, S::Greater { signed: true }),
                10 => (n, None, S::GreaterOrEqual { signed: true }),
                0 => (n, None, S::Equal),
                // le and lt: 0 >= n, 0 > n.
                13 => (n, Some(()), S::GreaterOrEqual { signed: true }),
                _ => (n, Some(()), S::Greater { signed: true }),
            };
            if second.is_some() {
                s.v128_const(0).local_get(first);
            } else {
                s.local_get(first).v128_const(0);
            }
            int_binary(&mut s, o, element);
            low(e, bytes);
        }
        S::Reverse { group } => {
            let pick: [u8; Q_BYTES as usize] = core::array::from_fn(|i| {
                let i = i as u8;
                let (start, within) = (i - i % group, i % group);
                let (lane, byte) = (within / element, within % element);
                start + (group / element - 1 - lane) * element + byte
            });
            e.sink().local_get(n).local_get(n).i8x16_shuffle(pick);
            low(e, bytes);
        }
        S::ShiftLeft => {
            let mut s = e.sink();
            s.local_get(n).i32_const(shift.into());
            match element {
                1 => s.i8x16_shl(),
                2 => s.i16x8_shl(),
                4 => s.i32x4_shl(),
                _ => s.i64x2_shl(),
            };
            low(e, bytes);
        }
        S::ShiftRight {
            signed,
            rounding,
            accumulate,
        } => {
            let shr = |s: &mut InstructionSink<'_>, signed: bool| {
                match (element, signed) {
                    (1, true) => s.i8x16_shr_s(),
                    (1, false) => s.i8x16_shr_u(),
                    (2, true) => s.i16x8_shr_s(),
                    (2, false) => s.i16x8_shr_u(),
                    (4, true) => s.i32x4_shr_s(),
                    (4, false) => s.i32x4_shr_u(),
                    (_, true) => s.i64x2_shr_s(),
                    (_, false) => s.i64x2_shr_u(),
                };
            };
            let full = u32::from(shift) == bits;
            let mut s = e.sink();
            if full && !signed {
                s.v128_const(0);
            } else {
                s.local_get(n)
                    .i32_const(i32::from(shift).min(bits as i32 - 1));
                shr(&mut s, signed);
            }
            if rounding {
                // Add the last bit shifted out.
                s.local_get(n).i32_const(i32::from(shift) - 1);
                shr(&mut s, false);
                s.v128_const(splat(element, 1));
                s.v128_and();
                int_binary(&mut s, S::Add, element);
            }
            if accumulate {
                s.local_get(d);
                int_binary(&mut s, S::Add, element);
            }
            low(e, bytes);
        }
        S::Max { .. }
        | S::Min { .. }
        | S::Greater { .. }
        | S::GreaterOrEqual { .. }
        | S::Equal
        | S::Add
        | S::Sub
            if native(op, element) =>
        {
            let mut s = e.sink();
            s.local_get(n).local_get(m);
            int_binary(&mut s, op, element);
            low(e, bytes);
        }
        S::Mul | S::MulAdd | S::MulSub if element > 1 => {
            let mut s = e.sink();
            if op != S::Mul {
                s.local_get(d);
            }
            s.local_get(n).local_get(m);
            int_binary(&mut s, S::Mul, element);
            if op == S::MulAdd {
                int_binary(&mut s, S::Add, element);
            } else if op == S::MulSub {
                int_binary(&mut s, S::Sub, element);
            }
            low(e, bytes);
        }
        S::AddPairwise | S::MaxPairwise { .. } | S::MinPairwise { .. } => {
            let (even, odd) = split_pairs(e, element, shape.q, n, m);
            if native(op, element) {
                let mut s = e.sink();
                s.local_get(even).local_get(odd);
                int_binary(&mut s, op, element);
            } else {
                map(
                    e,
                    element,
                    lanes,
                    0,
                    None,
                    &[
                        input(even, element, 0, signed),
                        input(odd, element, 0, signed),
                    ],
                    &|e, v, q| int_lane(e, op, bits, v, q),
                );
            }
            low(e, bytes);
        }
        S::AddAcross | S::AddLongAcross { .. } | S::MaxAcross { .. } | S::MinAcross { .. } => {
            let long = matches!(op, S::AddLongAcross { .. });
            let out = if long { 2 * element } else { element };
            let combine = match op {
                S::MaxAcross { signed } => S::Max { signed },
                S::MinAcross { signed } => S::Min { signed },
                _ => S::AddLong { signed },
            };
            let lanes = if shape.scalar {
                Q_BYTES / element
            } else {
                lanes
            };
            // The sum (or the extreme) of every lane, element by element.
            let acc = e.local(ValType::I64);
            let value = e.local(ValType::I64);
            // Adding, or taking the extreme, never saturates.
            let unused = e.local(ValType::I32);
            for lane in 0..lanes {
                let mut s = e.sink();
                s.local_get(n);
                extract(&mut s, element, lane, signed);
                if lane == 0 {
                    s.local_set(acc);
                    continue;
                }
                s.local_set(value);
                int_lane(e, combine, bits, &[acc, value], unused);
                e.sink().local_set(acc);
            }
            e.sink().v128_const(0).local_get(acc);
            replace(&mut e.sink(), out, 0);
            low(e, out);
        }
        _ if op.is_long() => {
            // Result elements twice as wide; sources from the low or high half (`rn` whole for wide).
            let wide = 2 * element;
            let count = if shape.scalar { 1 } else { D_BYTES / element };
            let first = if shape.scalar { 0 } else { half };
            let mut inputs = vec![];
            match op {
                S::AddWide { .. } | S::SubWide { .. } => {
                    inputs.push(input(n, wide, 0, signed));
                    inputs.push(input(m, element, first, signed));
                }
                S::AddLongPairwise { .. } => {
                    let (even, odd) = split_pairs(e, element, shape.q, n, n);
                    let count = width(shape) / wide;
                    let mut i = vec![
                        input(even, element, 0, signed),
                        input(odd, element, 0, signed),
                    ];
                    if let S::AddLongPairwise {
                        accumulate: true, ..
                    } = op
                    {
                        i.push(input(d, wide, 0, false));
                    }
                    let accumulate = i.len() == 3;
                    map(e, wide, count, 0, None, &i, &|e, v, _| {
                        e.sink().local_get(v[0]).local_get(v[1]).i64_add();
                        if accumulate {
                            e.sink().local_get(v[2]).i64_add();
                        }
                    });
                    low(e, width(shape));
                    e.set_vector(rd);
                    return;
                }
                S::ShiftLeftLong { .. } => {
                    let shift = i64::from(shift);
                    map(
                        e,
                        wide,
                        count,
                        0,
                        None,
                        &[input(n, element, first, signed)],
                        &|e, v, _| {
                            e.sink().local_get(v[0]).i64_const(shift).i64_shl();
                        },
                    );
                    e.set_vector(rd);
                    return;
                }
                S::ShiftLeftLongByWidth => {
                    inputs.push(input(n, element, first, false));
                }
                _ => {
                    inputs.push(input(n, element, first, signed));
                    inputs.push(input(m, element, first, signed));
                }
            }
            if matches!(
                op,
                S::AbsDiffAccumulateLong { .. }
                    | S::MulAddLong { .. }
                    | S::MulSubLong { .. }
                    | S::DoublingMulAddLong
                    | S::DoublingMulSubLong
            ) {
                inputs.push(input(d, wide, 0, true));
            }
            map(e, wide, count, 0, None, &inputs, &|e, v, q| {
                int_lane(e, op, bits, v, q)
            });
            if shape.scalar {
                low(e, wide);
            }
        }
        _ if op.is_narrow() => {
            // Sources twice as wide; results to the low half, or the high half keeping the low.
            let wide = 2 * element;
            let count = if shape.scalar { 1 } else { D_BYTES / element };
            let (base, first) = if shape.q && !shape.scalar {
                (Some(d), count)
            } else {
                (None, 0)
            };
            let wide_bits = 2 * bits;
            match op {
                S::Narrow => {
                    map(
                        e,
                        element,
                        count,
                        first,
                        base,
                        &[input(n, wide, 0, false)],
                        &|e, v, _| {
                            e.sink().local_get(v[0]);
                        },
                    );
                }
                S::SaturatingNarrow { from_signed, .. } => {
                    map(
                        e,
                        element,
                        count,
                        first,
                        base,
                        &[input(n, wide, 0, from_signed)],
                        &|e, v, q| int_lane(e, op, bits, v, q),
                    );
                }
                S::AddNarrowHigh { .. } | S::SubNarrowHigh { .. } => {
                    map(
                        e,
                        element,
                        count,
                        first,
                        base,
                        &[input(n, wide, 0, false), input(m, wide, 0, false)],
                        &|e, v, q| int_lane(e, op, bits, v, q),
                    );
                }
                S::ShiftRightNarrow { rounding }
                | S::SaturatingShiftRightNarrow { rounding, .. } => {
                    let from_signed = match op {
                        S::SaturatingShiftRightNarrow { from_signed, .. } => from_signed,
                        _ => false,
                    };
                    let saturate = match op {
                        S::SaturatingShiftRightNarrow { to_signed, .. } => Some(to_signed),
                        _ => None,
                    };
                    let shift = i64::from(shift);
                    map(
                        e,
                        element,
                        count,
                        first,
                        base,
                        &[input(n, wide, 0, from_signed)],
                        &|e, v, q| {
                            let (amount, t) = (e.local(ValType::I64), e.local(ValType::I64));
                            e.sink().i64_const(-shift).local_set(amount);
                            shift_by(e, v[0], amount, wide_bits, from_signed, rounding);
                            e.sink().local_set(t);
                            match saturate {
                                Some(to_signed) => {
                                    clamp_from(e, t, bits, to_signed, from_signed, q)
                                }
                                None => {
                                    e.sink().local_get(t);
                                }
                            }
                        },
                    );
                }
                _ => unreachable!(),
            }
            if shape.scalar {
                low(e, element);
            }
        }
        S::SaturatingShiftLeft {
            from_signed,
            to_signed,
        } => {
            let shift = i64::from(shift);
            map(
                e,
                element,
                lanes,
                0,
                None,
                &[input(n, element, 0, from_signed)],
                &|e, v, q| {
                    let amount = e.local(ValType::I64);
                    e.sink().i64_const(shift).local_set(amount);
                    saturating_shift_left(e, v[0], amount, bits, from_signed, to_signed, q);
                },
            );
            low(e, bytes);
        }
        S::ShiftRightInsert | S::ShiftLeftInsert => {
            let right = op == S::ShiftRightInsert;
            let ones = if bits == u64::BITS {
                u64::MAX
            } else {
                (1u64 << bits) - 1
            };
            let mask = if right {
                ones.checked_shr(shift.into()).unwrap_or(0)
            } else {
                (ones << shift) & ones
            };
            map(
                e,
                element,
                lanes,
                0,
                None,
                &[input(n, element, 0, false), input(d, element, 0, false)],
                &|e, v, _| {
                    let mut s = e.sink();
                    s.local_get(v[1]).i64_const(!mask as i64).i64_and();
                    if right && u32::from(shift) == bits {
                        s.i64_const(0);
                    } else {
                        s.local_get(v[0]).i64_const(shift.into());
                        if right {
                            s.i64_shr_u()
                        } else {
                            s.i64_shl()
                        };
                    }
                    s.i64_const(mask as i64).i64_and().i64_or();
                },
            );
            low(e, bytes);
        }
        _ => {
            // Everything else element by element on the same width.
            let mut inputs = vec![input(n, element, 0, signed), input(m, element, 0, signed)];
            if matches!(
                op,
                S::MulAdd
                    | S::MulSub
                    | S::AbsDiffAccumulate { .. }
                    | S::SaturatingAccumulate { .. }
            ) {
                inputs.push(input(
                    d,
                    element,
                    0,
                    !matches!(op, S::SaturatingAccumulate { signed: false }),
                ));
            }
            if let S::SaturatingAccumulate { signed } = op {
                // rn has the other signedness.
                inputs[0].signed = !signed;
            }
            map(e, element, lanes, 0, None, &inputs, &|e, v, q| {
                int_lane(e, op, bits, v, q)
            });
            low(e, bytes);
        }
    }
    e.set_vector(rd);
}

/// Whether an operation reads its elements as signed.
fn signedness(op: SimdOp) -> bool {
    use SimdOp as S;
    match op {
        S::Greater { signed }
        | S::GreaterOrEqual { signed }
        | S::HalvingAdd { signed, .. }
        | S::HalvingSub { signed }
        | S::SaturatingAdd { signed }
        | S::SaturatingSub { signed }
        | S::ShiftByRegister { signed, .. }
        | S::Max { signed }
        | S::Min { signed }
        | S::AbsDiff { signed }
        | S::AbsDiffAccumulate { signed }
        | S::MaxPairwise { signed }
        | S::MinPairwise { signed }
        | S::AddLongPairwise { signed, .. }
        | S::ShiftRight { signed, .. }
        | S::ShiftLeftLong { signed }
        | S::AddLong { signed }
        | S::SubLong { signed }
        | S::AddWide { signed }
        | S::SubWide { signed }
        | S::AbsDiffLong { signed }
        | S::AbsDiffAccumulateLong { signed }
        | S::MulLong { signed }
        | S::MulAddLong { signed }
        | S::MulSubLong { signed }
        | S::AddLongAcross { signed }
        | S::MaxAcross { signed }
        | S::MinAcross { signed } => signed,
        S::DoublingMulHigh { .. }
        | S::DoublingMulLong
        | S::DoublingMulAddLong
        | S::DoublingMulSubLong
        | S::SaturatingAbs
        | S::SaturatingNegate
        | S::CountLeadingSign
        | S::Abs
        | S::Negate => true,
        _ => false,
    }
}
