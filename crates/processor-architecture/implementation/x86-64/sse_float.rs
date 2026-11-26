//! The scalar conversions between general registers and floating point, and ROUND's lanes, on the exact rounding of
//! [`super`]: CVTSI2SS and CVTSI2SD round under MXCSR; CVTSS2SI to CVTTSD2SI give the integer indefinite for a NaN,
//! an infinity or a value out of range; ROUND rounds to an integral value in the mode its immediate or MXCSR names.

use super::{Class, DENORMALS_ARE_ZEROS, Exact, FLAGS, INEXACT, INVALID, MASK_SHIFT, Operand, Precision, ROUNDING_SHIFT, Rounding};
use super::{FloatOperation, LaneOutcome, read_rounding, round, round_to_integer};
use crate::planted::{Bug, is_active};

/// A lane's bits, and the flags of the exceptions found before the computation and after it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConvertedLane {
    pub bits: u64,
    pub before: u32,
    pub after: u32,
}

/// ROUND's immediate bits: the rounding mode, the bit that takes MXCSR's mode instead, and the bit that suppresses
/// inexact.
const ROUNDING_MODE_BITS: u8 = 3;
const USE_MXCSR_ROUNDING: u8 = 1 << 2;
const SUPPRESS_INEXACT: u8 = 1 << 3;

/// The flags to add to MXCSR, or, where a flag found is unmasked, the flags to add with #XM: those before the
/// computation alone if one of them is unmasked.
pub fn resolve_flags(lanes: &[ConvertedLane], mxcsr: u32) -> Result<u32, u32> {
    let unmasked = |flags: u32| flags & !(mxcsr >> MASK_SHIFT) & FLAGS != 0;
    let before = lanes.iter().fold(0, |all, lane| all | lane.before);
    if unmasked(before) {
        return Err(before);
    }
    let after = lanes.iter().fold(0, |all, lane| all | lane.after);
    if unmasked(after) {
        return Err(before | after);
    }
    Ok(before | after)
}

/// A signed integer rounded to the precision under MXCSR.
pub fn convert_integer(value: i64, precision: Precision, mxcsr: u32) -> ConvertedLane {
    if value == 0 {
        return ConvertedLane { bits: 0, before: 0, after: 0 };
    }
    let exact = Exact { negative: value < 0, significand: u128::from(value.unsigned_abs()), exponent: 0, sticky: false };
    let rounding = if is_active(Bug::IntegerToFloatRoundsToNearest) { Rounding::Nearest } else { read_rounding(mxcsr) };
    let (bits, after) = round(exact, precision, rounding, mxcsr);
    ConvertedLane { bits, before: 0, after }
}

/// A floating-point value rounded under MXCSR, or truncated, to a signed integer of `integer_bits`.
pub fn convert_to_integer(bits: u64, precision: Precision, integer_bits: u32, truncate: bool, mxcsr: u32) -> ConvertedLane {
    let indefinite_bits = if is_active(Bug::FloatToIntegerIndefiniteThirtyTwo) { u32::BITS } else { integer_bits };
    let indefinite = ConvertedLane { bits: 1 << (indefinite_bits - 1), before: INVALID, after: 0 };
    let operand = Operand::read(bits, precision, mxcsr & DENORMALS_ARE_ZEROS != 0);
    let Class::Finite { significand, exponent, .. } = operand.class else {
        return match operand.class {
            Class::Zero => ConvertedLane { bits: 0, before: 0, after: 0 },
            _ => indefinite,
        };
    };
    let rounding = if truncate { Rounding::TowardZero } else { read_rounding(mxcsr) };
    let (magnitude, inexact) = round_to_integer(significand, exponent, rounding, operand.negative);
    let limit = if operand.negative { 1u128 << (integer_bits - 1) } else { (1 << (integer_bits - 1)) - 1 };
    if magnitude > limit {
        return indefinite;
    }
    let integer = if operand.negative { (magnitude as i64).wrapping_neg() } else { magnitude as i64 };
    let mask = if integer_bits == u64::BITS { u64::MAX } else { (1 << integer_bits) - 1 };
    ConvertedLane { bits: integer as u64 & mask, before: 0, after: if inexact { INEXACT } else { 0 } }
}

/// A lane rounded to an integral value in the immediate's mode, or MXCSR's where its bit 2 is set; a signaling NaN is
/// quieted and invalid, and inexact is not reported where bit 3 is set.
pub fn round_integral(bits: u64, precision: Precision, control: u8, mxcsr: u32) -> ConvertedLane {
    let operand = Operand::read(bits, precision, mxcsr & DENORMALS_ARE_ZEROS != 0);
    let sign = if operand.negative { 1 << (precision.to_bits() - 1) } else { 0 };
    let rounding = if control & USE_MXCSR_ROUNDING != 0 && !is_active(Bug::RoundIgnoresMxcsrSelect) {
        read_rounding(mxcsr)
    } else {
        read_rounding(u32::from(control & ROUNDING_MODE_BITS) << ROUNDING_SHIFT)
    };
    let reports_inexact = control & SUPPRESS_INEXACT == 0 || is_active(Bug::RoundReportsSuppressedInexact);
    match operand.class {
        Class::Zero => ConvertedLane { bits: sign, before: 0, after: 0 },
        Class::Infinity => ConvertedLane { bits: operand.bits, before: 0, after: 0 },
        Class::Nan { signaling } => ConvertedLane {
            bits: operand.bits | precision.to_quiet_bit(),
            before: if signaling { INVALID } else { 0 },
            after: 0,
        },
        Class::Finite { exponent, .. } if exponent >= 0 => ConvertedLane { bits: operand.bits, before: 0, after: 0 },
        Class::Finite { significand, exponent, .. } => {
            let (magnitude, inexact) = round_to_integer(significand, exponent, rounding, operand.negative);
            let after = if inexact && reports_inexact { INEXACT } else { 0 };
            if magnitude == 0 {
                return ConvertedLane { bits: sign, before: 0, after };
            }
            let exact = Exact { negative: operand.negative, significand: magnitude, exponent: 0, sticky: false };
            let (bits, _) = round(exact, precision, Rounding::Nearest, mxcsr);
            ConvertedLane { bits, before: 0, after }
        }
    }
}

/// The immediate's bit that selects lane 0's product; the bits above it select the lanes above.
const FIRST_PRODUCT: u32 = 4;

/// DPPS and DPPD: the selected lanes' products, each other +0.0, then in each lane its value and its neighbor's
/// summed, the lane one away, then for DPPS two away, each step rounded and checked for exceptions in turn as the SDM
/// orders them; each lane the low bits select gets its own sum, and each other +0.0. Or the flags found up to the
/// first step with an unmasked exception, with #XM. The first operand of each sum, which decides the NaN of two, is
/// the lane's own value but in DPPS's first step, where it is the neighbor's, as the chip shows.
pub fn compute_dot_product(destination: u128, source: u128, precision: Precision, immediate: u8, mxcsr: u32) -> Result<(u128, u32), u32> {
    let bits = precision.to_bits();
    let lanes = u128::BITS / bits;
    let read = |value: u128, lane: u32| (value >> (lane * bits)) as u64 & lane_mask(precision);
    let unmasked = |flags: u32| flags & !(mxcsr >> MASK_SHIFT) & FLAGS != 0;
    let mut found = 0;
    let mut step = |outcomes: &[LaneOutcome]| -> Result<alloc::vec::Vec<u64>, u32> {
        let before = outcomes.iter().fold(0, |all, outcome| all | outcome.before);
        if unmasked(before) {
            return Err(found | before);
        }
        let after = outcomes.iter().fold(0, |all, outcome| all | outcome.after);
        if unmasked(after) {
            return Err(found | before | after);
        }
        found |= before | after;
        Ok(outcomes.iter().map(|outcome| outcome.bits).collect())
    };
    let products: alloc::vec::Vec<LaneOutcome> = (0..lanes)
        .map(|lane| match u32::from(immediate) >> (FIRST_PRODUCT + lane) & 1 {
            0 => LaneOutcome { bits: 0, before: 0, after: 0 },
            _ => FloatOperation::Multiply.compute(read(destination, lane), read(source, lane), precision, mxcsr, 0),
        })
        .collect();
    let mut sums = step(&products)?;
    let add = |first: u64, second: u64| FloatOperation::Add.compute(first, second, precision, mxcsr, 0);
    if is_active(Bug::DotProductSumsInOrder) {
        while sums.len() > 1 {
            let first = step(&[add(sums[0], sums[1])])?;
            sums = first.into_iter().chain(sums[2..].iter().copied()).collect();
        }
        sums = alloc::vec![sums[0]; lanes as usize];
    }
    let mut distance = 1;
    while distance < sums.len() {
        let neighbor_first = precision == Precision::Single && distance == 1 && !is_active(Bug::DotProductOwnLaneFirst);
        let pairs: alloc::vec::Vec<LaneOutcome> = (0..sums.len())
            .map(|lane| {
                let (own, neighbor) = (sums[lane], sums[lane ^ distance]);
                if neighbor_first { add(neighbor, own) } else { add(own, neighbor) }
            })
            .collect();
        sums = step(&pairs)?;
        distance *= 2;
    }
    let result = (0..lanes)
        .filter(|lane| immediate >> lane & 1 != 0)
        .fold(0u128, |all, lane| all | (u128::from(sums[lane as usize]) << (lane * bits)));
    Ok((result, found))
}

const fn lane_mask(precision: Precision) -> u64 {
    if precision.to_bits() == u64::BITS { u64::MAX } else { (1 << precision.to_bits()) - 1 }
}
