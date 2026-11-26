//! The floating-point arithmetic of [`crate::avx`]'s forms that the legacy forms lack: VEX's 32 compare
//! predicates, the fused multiply-add, rounding to an integral value, conversions between quadwords and floats, and
//! half precision. Each lane is computed exactly and rounded once, as the legacy arithmetic of this module's parent
//! is, and its exceptions split the same way into those before the computation and those after it.

use alloc::vec::Vec;

use crate::planted::{Bug, is_active};

use super::{
    Class, DENORMAL, DENORMALS_ARE_ZEROS, DIVIDE_BY_ZERO, Exact, FLAGS, INEXACT, INVALID, LaneOutcome, MASK_SHIFT,
    Operand, OVERFLOW, Precision, ROUNDING_SHIFT, Rounding, UNDERFLOW, compare_less, is_equal, read_rounding, round,
    round_to_integer,
};

/// The flags of the exceptions detected before a lane's computation.
const BEFORE: u32 = INVALID | DENORMAL | DIVIDE_BY_ZERO;
/// Those detected after it.
const AFTER: u32 = OVERFLOW | UNDERFLOW | INEXACT;

/// The rule that picks the NaN a fused multiply-add returns when more than one operand is a NaN.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FusedNanOrder {
    /// The first NaN in Intel's operand order: the first operand, the second, the third.
    OperandOrder,
    /// The multiplicand, the multiplier, then the addend.
    FormulaOrder,
}

/// Whether a product of zero and infinity added to a quiet NaN signals invalid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InfiniteProductOfZero {
    Signals,
    Quiet,
}

/// The choices a fused multiply-add's lanes leave open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FusedChoices {
    pub nan_order: FusedNanOrder,
    pub product_of_zero: InfiniteProductOfZero,
}

/// Joins the outcomes of an instruction's halves, each the legacy arithmetic's result or #XM's flags, into the
/// instruction's: #XM with the flags before the computation if one of them is unmasked, else with every flag if one
/// after it is, else every half's bits and the flags to add.
pub(crate) fn join_halves(halves: &[Result<(u128, u32), u32>], mxcsr: u32) -> Result<(Vec<u128>, u32), u32> {
    let flags = |half: &Result<(u128, u32), u32>| match half {
        Ok((_, flags)) | Err(flags) => *flags,
    };
    let before = halves.iter().fold(0, |all, half| all | (flags(half) & BEFORE));
    if is_unmasked(before, mxcsr) {
        return Err(before);
    }
    let after = halves.iter().fold(0, |all, half| all | (flags(half) & AFTER));
    if is_unmasked(after, mxcsr) {
        return Err(before | after);
    }
    let bits = halves.iter().map(|half| half.map_or(0, |(bits, _)| bits)).collect();
    Ok((bits, before | after))
}

fn is_unmasked(flags: u32, mxcsr: u32) -> bool {
    flags & !(mxcsr >> MASK_SHIFT) & FLAGS != 0
}

/// Joins lane outcomes into the bits of `lanes` lanes of `bits` bits placed over `kept`, as [`join_halves`] joins
/// halves.
fn join_lanes(outcomes: &[LaneOutcome], bits: u32, kept: u128, mxcsr: u32) -> Result<(u128, u32), u32> {
    let before = outcomes.iter().fold(0, |all, outcome| all | outcome.before);
    if is_unmasked(before, mxcsr) {
        return Err(before);
    }
    let after = outcomes.iter().fold(0, |all, outcome| all | outcome.after);
    if is_unmasked(after, mxcsr) {
        return Err(before | after);
    }
    let mask = if bits == u128::BITS { u128::MAX } else { (1 << bits) - 1 };
    let result = outcomes.iter().enumerate().fold(kept, |all, (lane, outcome)| {
        let shift = lane as u32 * bits;
        (all & !(mask << shift)) | (u128::from(outcome.bits) << shift)
    });
    Ok((result, before | after))
}

/// The lanes of 128 bits of the precision, lowest first.
fn split_lanes(value: u128, precision: Precision) -> Vec<u64> {
    let bits = precision.to_bits();
    (0..u128::BITS / bits).map(|lane| (value >> (lane * bits)) as u64 & precision.to_mask()).collect()
}

/// One half of VCMPPS to VCMPSD: every lane (or the lowest, the first source's others kept) all ones where the
/// predicate holds.
pub(crate) fn compare_half(first: u128, second: u128, precision: Precision, scalar: bool, predicate: u8, mxcsr: u32) -> Result<(u128, u32), u32> {
    let lanes = if scalar { 1 } else { (u128::BITS / precision.to_bits()) as usize };
    let (a, b) = (split_lanes(first, precision), split_lanes(second, precision));
    let outcomes: Vec<LaneOutcome> = (0..lanes).map(|lane| compare_lane(a[lane], b[lane], precision, predicate, mxcsr)).collect();
    join_lanes(&outcomes, precision.to_bits(), first, mxcsr)
}

fn compare_lane(first: u64, second: u64, precision: Precision, predicate: u8, mxcsr: u32) -> LaneOutcome {
    const SIGNALING: [u8; 8] = [1, 2, 5, 6, 9, 0xA, 0xD, 0xE];
    let daz = mxcsr & DENORMALS_ARE_ZEROS != 0;
    let (a, b) = (Operand::read(first, precision, daz), Operand::read(second, precision, daz));
    let unordered = a.is_nan() || b.is_nan();
    let signaling_nan = [a, b].iter().any(|operand| matches!(operand.class, Class::Nan { signaling: true }));
    let less = !unordered && compare_less(a, b);
    let equal = !unordered && is_equal(a, b);
    let greater = !unordered && !less && !equal;
    let predicate = if is_active(Bug::ComparePredicateThreeBits) { predicate & 7 } else { predicate };
    let base = predicate & 0xF;
    let signals = SIGNALING.contains(&base) != (predicate & 0x10 != 0);
    let holds = match base {
        0 => equal,
        1 => less,
        2 => less || equal,
        3 => unordered,
        4 => !equal,
        5 => !less,
        6 => !(less || equal),
        7 => !unordered,
        8 => equal || unordered,
        9 => less || unordered,
        0xA => !greater,
        0xB => false,
        0xC => !equal && !unordered,
        0xD => greater || equal,
        0xE => greater,
        _ => true,
    };
    let before = if unordered {
        if signaling_nan || signals { INVALID } else { 0 }
    } else if a.is_denormal() || b.is_denormal() {
        DENORMAL
    } else {
        0
    };
    LaneOutcome { bits: if holds { precision.to_mask() } else { 0 }, before, after: 0 }
}

/// How an FMA form's lanes combine their operands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct FusedLanes {
    /// The operands' indices: the multiplicand, the multiplier and the addend.
    pub roles: [usize; 3],
    /// Whether the product is negated, as for VFNMADD and VFNMSUB.
    pub negate_product: bool,
    /// Whether the even lanes and the odd lanes subtract the addend.
    pub subtract: [bool; 2],
    pub precision: Precision,
    /// Whether only the lowest lane is computed, the first operand's others kept.
    pub scalar: bool,
}

/// One half of an FMA form: each lane the product of two operands plus or minus the third, rounded once.
pub(crate) fn fuse_half(operands: [u128; 3], lanes: FusedLanes, mxcsr: u32, choices: FusedChoices) -> Result<(u128, u32), u32> {
    let FusedLanes { roles, negate_product, subtract, precision, scalar } = lanes;
    let count = if scalar { 1 } else { (u128::BITS / precision.to_bits()) as usize };
    let split = operands.map(|operand| split_lanes(operand, precision));
    let outcomes: Vec<LaneOutcome> = (0..count)
        .map(|lane| {
            let values = [split[0][lane], split[1][lane], split[2][lane]];
            fuse_lane(values, roles, negate_product, subtract[lane % 2], precision, mxcsr, choices)
        })
        .collect();
    join_lanes(&outcomes, precision.to_bits(), operands[0], mxcsr)
}

fn fuse_lane(
    values: [u64; 3],
    roles: [usize; 3],
    negate_product: bool,
    subtract: bool,
    precision: Precision,
    mxcsr: u32,
    choices: FusedChoices,
) -> LaneOutcome {
    let daz = mxcsr & DENORMALS_ARE_ZEROS != 0;
    let operands = values.map(|value| Operand::read(value, precision, daz));
    let (multiplicand, multiplier, addend) = (operands[roles[0]], operands[roles[1]], operands[roles[2]]);
    let done = |bits: u64, before: u32| LaneOutcome { bits, before, after: 0 };
    let signaling = operands.iter().any(|operand| matches!(operand.class, Class::Nan { signaling: true }));
    let infinite_product_of_zero = matches!(
        (multiplicand.class, multiplier.class),
        (Class::Infinity, Class::Zero) | (Class::Zero, Class::Infinity)
    );
    let order = match choices.nan_order {
        FusedNanOrder::OperandOrder => [0, 1, 2],
        FusedNanOrder::FormulaOrder => roles,
    };
    if let Some(&index) = order.iter().find(|&&index| operands[index].is_nan()) {
        let quiet_addend_alone = infinite_product_of_zero && !signaling;
        let invalid = signaling || (quiet_addend_alone && choices.product_of_zero == InfiniteProductOfZero::Signals);
        return done(operands[index].bits | precision.to_quiet_bit(), if invalid { INVALID } else { 0 });
    }
    let denormal = if operands.iter().any(Operand::is_denormal) { DENORMAL } else { 0 };
    let default_nan = precision.to_default_nan();
    if infinite_product_of_zero {
        return done(default_nan, INVALID);
    }
    let rounding = read_rounding(mxcsr);
    let sign_of = |negative: bool| if negative { precision.to_sign_bit() } else { 0 };
    let infinity = |negative: bool| sign_of(negative) | precision.to_infinity();
    let product_negative = (multiplicand.negative != multiplier.negative) != negate_product;
    let addend_negative = addend.negative != subtract;
    let product_class = match (multiplicand.class, multiplier.class) {
        (Class::Infinity, _) | (_, Class::Infinity) => Class::Infinity,
        (Class::Zero, _) | (_, Class::Zero) => Class::Zero,
        (
            Class::Finite { significand: a, exponent: ea, .. },
            Class::Finite { significand: b, exponent: eb, .. },
        ) if is_active(Bug::FusedProductRounded) => {
            let exact = Exact { negative: false, significand: a * b, exponent: ea + eb, sticky: false };
            let (bits, _) = round(exact, precision, rounding, mxcsr | (0x3F << MASK_SHIFT));
            Operand::read(bits, precision, false).class
        }
        (
            Class::Finite { significand: a, exponent: ea, .. },
            Class::Finite { significand: b, exponent: eb, .. },
        ) => Class::Finite { significand: a * b, exponent: ea + eb, denormal: false },
        _ => unreachable!("NaNs are handled first"),
    };
    let exact = match (product_class, addend.class) {
        (Class::Infinity, Class::Infinity) if product_negative != addend_negative => return done(default_nan, INVALID),
        (Class::Infinity, _) => return done(infinity(product_negative), denormal),
        (_, Class::Infinity) => return done(infinity(addend_negative), denormal),
        (Class::Zero, Class::Zero) => {
            let negative = if product_negative == addend_negative { product_negative } else { rounding == Rounding::Down };
            return done(sign_of(negative), denormal);
        }
        (Class::Zero, Class::Finite { significand, exponent, .. }) => {
            Exact { negative: addend_negative, significand, exponent, sticky: false }
        }
        (Class::Finite { significand, exponent, .. }, Class::Zero) => {
            Exact { negative: product_negative, significand, exponent, sticky: false }
        }
        (
            Class::Finite { significand: a, exponent: ea, .. },
            Class::Finite { significand: b, exponent: eb, .. },
        ) => match add_exactly(product_negative, a, ea, addend_negative, b, eb) {
            Some(exact) => exact,
            None => return done(sign_of(rounding == Rounding::Down), denormal),
        },
        _ => unreachable!("NaNs are handled first"),
    };
    let (bits, after) = round(exact, precision, rounding, mxcsr);
    LaneOutcome { bits, before: denormal, after }
}

/// The exact sum of two finite nonzero values of any width, or `None` for an exact zero: both aligned below the
/// larger's top bit so that 125 bits hold the larger, the smaller's lost bits sticky.
fn add_exactly(first_negative: bool, first: u128, first_exponent: i32, second_negative: bool, second: u128, second_exponent: i32) -> Option<Exact> {
    const HELD: i32 = 124;
    let top = |significand: u128, exponent: i32| exponent + (u128::BITS - significand.leading_zeros()) as i32 - 1;
    let common = top(first, first_exponent).max(top(second, second_exponent)) - HELD;
    let align = |significand: u128, exponent: i32| -> (u128, bool) {
        let shift = exponent - common;
        if shift >= 0 {
            (significand << shift, false)
        } else if -shift >= u128::BITS as i32 {
            (0, true)
        } else {
            let shifted = significand >> -shift;
            (shifted, shifted << -shift != significand)
        }
    };
    let (a, a_sticky) = align(first, first_exponent);
    let (b, b_sticky) = align(second, second_exponent);
    if first_negative == second_negative {
        return Some(Exact { negative: first_negative, significand: a + b, exponent: common, sticky: a_sticky || b_sticky });
    }
    let (large, small, small_sticky, negative) = match a.cmp(&b) {
        core::cmp::Ordering::Equal if !a_sticky && !b_sticky => return None,
        core::cmp::Ordering::Less => (b, a, a_sticky, second_negative),
        core::cmp::Ordering::Equal if a_sticky => (b, a, a_sticky, second_negative),
        _ => (a, b, b_sticky, first_negative),
    };
    Some(Exact { negative, significand: large - small - u128::from(small_sticky), exponent: common, sticky: small_sticky })
}

/// One half of VDPPS or VDPPD: the selected products, their sums in pairs and the sum of those, each step under
/// MXCSR in the order they are computed, the first step with an unmasked exception raising #XM with the flags of
/// the steps so far; `flags` the flags of the halves before.
pub(crate) fn dot_half(first: u128, second: u128, precision: Precision, immediate: u8, mxcsr: u32, flags: u32) -> Result<(u128, u32), u32> {
    use super::FloatOperation::{Add, Multiply};
    let lanes = (u128::BITS / precision.to_bits()) as usize;
    let (a, b) = (split_lanes(first, precision), split_lanes(second, precision));
    let mut flags = flags;
    let mut step = |outcome: LaneOutcome| -> Result<u64, u32> {
        if is_unmasked(outcome.before, mxcsr) {
            return Err(flags | outcome.before);
        }
        flags |= outcome.before;
        if is_unmasked(outcome.after, mxcsr) {
            return Err(flags | outcome.after);
        }
        flags |= outcome.after;
        Ok(outcome.bits)
    };
    let mut products = Vec::new();
    for lane in 0..lanes {
        let selected = immediate >> (4 + lane) & 1 != 0 || is_active(Bug::DotProductEveryLane);
        products.push(if selected { step(Multiply.compute(a[lane], b[lane], precision, mxcsr, 0))? } else { 0 });
    }
    // The steps' exceptions are those of the pair sums and their sum; each lane's sum takes its operands in an order
    // of its own, which decides only the NaN it gives: for singles its partner's product before its own and its own
    // pair's sum before the other's, for doubles its own product first.
    let add = |first: u64, second: u64| Add.compute(first, second, precision, mxcsr, 0);
    if lanes == 4 {
        let low = step(add(products[0], products[1]))?;
        let high = step(add(products[2], products[3]))?;
        step(add(low, high))?;
    } else {
        step(add(products[0], products[1]))?;
    }
    let sum_of = |lane: usize| -> u64 {
        if lanes == 4 {
            let pair = |lane: usize| add(products[lane ^ 1], products[lane]).bits;
            add(pair(lane), pair(lane ^ 2)).bits
        } else {
            add(products[lane], products[lane ^ 1]).bits
        }
    };
    let bits = precision.to_bits();
    let result = (0..lanes).fold(0u128, |all, lane| if immediate >> lane & 1 != 0 { all | (u128::from(sum_of(lane)) << (lane as u32 * bits)) } else { all });
    Ok((result, flags))
}

/// One half of VROUNDPS to VROUNDSD: each lane (or the lowest, the first source's others kept) rounded to an
/// integral value in the mode the immediate's bits 1 to 0 name, or MXCSR's where its bit 2 is set; its bit 3
/// suppresses inexact. Only invalid (for a signaling NaN) and inexact are signaled.
pub(crate) fn round_half(kept: u128, source: u128, precision: Precision, scalar: bool, immediate: u8, mxcsr: u32) -> Result<(u128, u32), u32> {
    let lanes = if scalar { 1 } else { (u128::BITS / precision.to_bits()) as usize };
    let values = split_lanes(source, precision);
    let rounding = if immediate & 4 != 0 {
        read_rounding(mxcsr)
    } else {
        read_rounding(u32::from(immediate & 3) << ROUNDING_SHIFT)
    };
    let suppressed = immediate & 8 != 0 && !is_active(Bug::RoundInexactNeverSuppressed);
    let outcomes: Vec<LaneOutcome> = (0..lanes).map(|lane| round_lane(values[lane], precision, rounding, suppressed, mxcsr)).collect();
    join_lanes(&outcomes, precision.to_bits(), if scalar { kept } else { 0 }, mxcsr)
}

fn round_lane(value: u64, precision: Precision, rounding: Rounding, suppressed: bool, mxcsr: u32) -> LaneOutcome {
    let daz = mxcsr & DENORMALS_ARE_ZEROS != 0;
    let operand = Operand::read(value, precision, daz);
    let sign = if operand.negative { precision.to_sign_bit() } else { 0 };
    let done = |bits: u64, before: u32, after: u32| LaneOutcome { bits, before, after };
    match operand.class {
        Class::Nan { signaling } => done(operand.bits | precision.to_quiet_bit(), if signaling { INVALID } else { 0 }, 0),
        Class::Zero => done(sign, 0, 0),
        Class::Infinity => done(operand.bits, 0, 0),
        Class::Finite { exponent, .. } if exponent >= 0 => done(operand.bits, 0, 0),
        Class::Finite { significand, exponent, .. } => {
            let (magnitude, lost) = round_to_integer(significand, exponent, rounding, operand.negative);
            let bits = if magnitude == 0 {
                sign
            } else {
                round(Exact { negative: operand.negative, significand: magnitude, exponent: 0, sticky: false }, precision, Rounding::Nearest, mxcsr).0
            };
            done(bits, 0, if lost && !suppressed { INEXACT } else { 0 })
        }
    }
}

/// VCVTSI2SS, VCVTSI2SD: the signed integer rounded under MXCSR into the low lane of `kept`.
pub(crate) fn convert_integer(kept: u128, integer: i64, precision: Precision, mxcsr: u32) -> Result<(u128, u32), u32> {
    let outcome = if integer == 0 {
        LaneOutcome { bits: 0, before: 0, after: 0 }
    } else {
        let exact = Exact { negative: integer < 0, significand: u128::from(integer.unsigned_abs()), exponent: 0, sticky: false };
        let (bits, after) = round(exact, precision, read_rounding(mxcsr), mxcsr);
        LaneOutcome { bits, before: 0, after }
    };
    join_lanes(&[outcome], precision.to_bits(), kept, mxcsr)
}

/// VCVTSS2SI to VCVTTSD2SI: the low lane rounded (or truncated) to a signed integer of `bits` bits; a NaN, an
/// infinity or a value out of range is invalid and gives the integer indefinite, the lowest one.
pub(crate) fn convert_to_integer(value: u64, precision: Precision, bits: u32, truncate: bool, mxcsr: u32) -> Result<(u64, u32), u32> {
    let daz = mxcsr & DENORMALS_ARE_ZEROS != 0;
    let operand = Operand::read(value, precision, daz);
    let indefinite = if is_active(Bug::ConversionToQuadwordNarrowIndefinite) { 1u64 << 31 } else { 1u64 << (bits - 1) };
    let outcome = match operand.class {
        Class::Zero => LaneOutcome { bits: 0, before: 0, after: 0 },
        Class::Finite { significand, exponent, .. } => {
            let mode = if truncate { Rounding::TowardZero } else { read_rounding(mxcsr) };
            let (magnitude, lost) = round_to_integer(significand, exponent, mode, operand.negative);
            let limit = if operand.negative { 1u128 << (bits - 1) } else { (1u128 << (bits - 1)) - 1 };
            if magnitude > limit {
                LaneOutcome { bits: indefinite, before: INVALID, after: 0 }
            } else {
                let integer = if operand.negative { (magnitude as u64).wrapping_neg() } else { magnitude as u64 };
                let mask = if bits == u64::BITS { u64::MAX } else { (1 << bits) - 1 };
                LaneOutcome { bits: integer & mask, before: 0, after: if lost { INEXACT } else { 0 } }
            }
        }
        _ => LaneOutcome { bits: indefinite, before: INVALID, after: 0 },
    };
    join_lanes(&[outcome], u64::BITS, 0, mxcsr).map(|(bits, flags)| (bits as u64, flags))
}

/// Half precision: 1 sign bit, 5 exponent bits, 10 fraction bits.
const HALF_FRACTION_BITS: u32 = 10;
const HALF_EXPONENT_MASK: u64 = 0x1F;
const HALF_BIAS: i32 = 15;

/// VCVTPH2PS of 64 bits: four half-precision values, each exactly into a single; a signaling NaN is quieted and
/// signals invalid, and nothing else is signaled.
pub(crate) fn widen_halves(source: u128, mxcsr: u32) -> Result<(u128, u32), u32> {
    let outcomes: Vec<LaneOutcome> = (0..4).map(|lane| widen_half((source >> (16 * lane)) as u64 & 0xFFFF)).collect();
    join_lanes(&outcomes, u32::BITS, 0, mxcsr)
}

fn widen_half(half: u64) -> LaneOutcome {
    let negative = half >> 15 != 0;
    let biased = (half >> HALF_FRACTION_BITS) & HALF_EXPONENT_MASK;
    let fraction = half & ((1 << HALF_FRACTION_BITS) - 1);
    let single = Precision::Single;
    let sign = if negative { single.to_sign_bit() } else { 0 };
    let done = |bits: u64, before: u32| LaneOutcome { bits, before, after: 0 };
    match (biased, fraction) {
        (0, 0) => done(sign, 0),
        (0, _) if is_active(Bug::HalfSubnormalAsZero) => done(sign, 0),
        (HALF_EXPONENT_MASK, 0) => done(sign | single.to_infinity(), 0),
        (HALF_EXPONENT_MASK, _) => {
            let signaling = fraction & (1 << (HALF_FRACTION_BITS - 1)) == 0;
            let moved = fraction << (single.to_significand_bits() - 1 - HALF_FRACTION_BITS);
            done(sign | single.to_infinity() | moved | single.to_quiet_bit(), if signaling { INVALID } else { 0 })
        }
        _ => {
            let (significand, exponent) = if biased == 0 {
                (fraction, 1 - HALF_BIAS - HALF_FRACTION_BITS as i32)
            } else {
                (fraction | (1 << HALF_FRACTION_BITS), biased as i32 - HALF_BIAS - HALF_FRACTION_BITS as i32)
            };
            let exact = Exact { negative, significand: u128::from(significand), exponent, sticky: false };
            done(round(exact, single, Rounding::Nearest, 0).0, 0)
        }
    }
}

/// VCVTPS2PH of 128 bits: four singles, each rounded to half precision in the immediate's mode (MXCSR's where its
/// bit 2 is set); DAZ reads the singles, and FTZ does not apply to the halves.
pub(crate) fn narrow_to_halves(source: u128, immediate: u8, mxcsr: u32) -> Result<(u128, u32), u32> {
    let rounding = if is_active(Bug::HalfRoundsToNearest) {
        Rounding::Nearest
    } else if immediate & 4 != 0 {
        read_rounding(mxcsr)
    } else {
        read_rounding(u32::from(immediate & 3) << ROUNDING_SHIFT)
    };
    let values = split_lanes(source, Precision::Single);
    let outcomes: Vec<LaneOutcome> = values.iter().map(|&value| narrow_single(value, rounding, mxcsr)).collect();
    join_lanes(&outcomes, u16::BITS, 0, mxcsr)
}

pub(crate) fn narrow_single(value: u64, rounding: Rounding, mxcsr: u32) -> LaneOutcome {
    let daz = mxcsr & DENORMALS_ARE_ZEROS != 0;
    let operand = Operand::read(value, Precision::Single, daz);
    let sign = if operand.negative { 1 << 15 } else { 0 };
    let infinity = HALF_EXPONENT_MASK << HALF_FRACTION_BITS;
    let done = |bits: u64, before: u32| LaneOutcome { bits, before, after: 0 };
    match operand.class {
        Class::Zero => done(sign, 0),
        Class::Infinity => done(sign | infinity, 0),
        Class::Nan { signaling } => {
            let fraction = (operand.bits & Precision::Single.to_fraction_mask()) >> (Precision::Single.to_significand_bits() - 1 - HALF_FRACTION_BITS);
            done(sign | infinity | fraction | (1 << (HALF_FRACTION_BITS - 1)), if signaling { INVALID } else { 0 })
        }
        Class::Finite { significand, exponent, denormal } => {
            let exact = Exact { negative: operand.negative, significand, exponent, sticky: false };
            let (bits, after) = round_to_half(exact, rounding, mxcsr);
            LaneOutcome { bits: sign | bits, before: if denormal { DENORMAL } else { 0 }, after }
        }
    }
}

/// Rounds a finite nonzero value to half precision's magnitude bits, with overflow, underflow (tininess after
/// rounding) and inexact as MXCSR's masks ask, a tiny result kept whatever FTZ holds.
fn round_to_half(exact: Exact, rounding: Rounding, mxcsr: u32) -> (u64, u32) {
    const DIGITS: i32 = HALF_FRACTION_BITS as i32 + 1;
    const MINIMUM: i32 = 1 - HALF_BIAS;
    const MAXIMUM: i32 = HALF_BIAS;
    let Exact { negative, significand, exponent, sticky } = exact;
    let top = exponent + (u128::BITS - significand.leading_zeros()) as i32 - 1;
    let to_quantum = |quantum: i32| -> (u128, bool) {
        let shift = quantum - exponent;
        if shift <= 0 {
            return (significand << -shift, sticky);
        }
        let (kept, rest) = if shift >= u128::BITS as i32 { (0, significand) } else { (significand >> shift, significand & ((1 << shift) - 1)) };
        let half = if shift > u128::BITS as i32 { 0 } else { 1u128 << (shift - 1) };
        let lost = rest != 0 || sticky;
        let up = match rounding {
            Rounding::Nearest => rest > half || (rest == half && (sticky || kept & 1 == 1)),
            Rounding::TowardZero => false,
            Rounding::Up => lost && !negative,
            Rounding::Down => lost && negative,
        };
        (kept + u128::from(up), lost)
    };
    let masked = |flag: u32| mxcsr & (flag << MASK_SHIFT) != 0;
    let (unbounded, unbounded_lost) = to_quantum(top - (DIGITS - 1));
    let rounded_top = top + i32::from(unbounded >> DIGITS != 0);
    let tiny = rounded_top < MINIMUM;
    let mut quantum = (top - (DIGITS - 1)).max(MINIMUM - (DIGITS - 1));
    let (mut multiple, inexact) = to_quantum(quantum);
    if multiple >> DIGITS != 0 {
        multiple >>= 1;
        quantum += 1;
    }
    let unbounded_inexact = if unbounded_lost { INEXACT } else { 0 };
    if multiple != 0 && quantum + DIGITS - 1 > MAXIMUM {
        let toward_infinity = match rounding {
            Rounding::Nearest => true,
            Rounding::TowardZero => false,
            Rounding::Up => !negative,
            Rounding::Down => negative,
        };
        let infinity = HALF_EXPONENT_MASK << HALF_FRACTION_BITS;
        let magnitude = if toward_infinity { infinity } else { infinity - 1 };
        let flags = if masked(OVERFLOW) { OVERFLOW | INEXACT } else { OVERFLOW | unbounded_inexact };
        return (magnitude, flags);
    }
    let mut flags = if inexact { INEXACT } else { 0 };
    if tiny {
        if !masked(UNDERFLOW) {
            flags = UNDERFLOW | unbounded_inexact;
        } else if inexact {
            flags |= UNDERFLOW;
        }
    }
    let bits = if multiple >> (DIGITS - 1) == 0 {
        multiple as u64
    } else {
        let biased = (quantum + DIGITS - 1 + HALF_BIAS) as u64;
        (biased << HALF_FRACTION_BITS) | (multiple as u64 & ((1 << HALF_FRACTION_BITS) - 1))
    };
    (bits, flags)
}
