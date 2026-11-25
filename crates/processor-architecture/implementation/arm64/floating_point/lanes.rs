//! One lane of an Advanced SIMD floating-point instruction: the scalar formulas of the parent module, and the
//! operations only the vector forms have, each following Arm's pseudocode for it
//! ([shared floating-point functions](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float)).

use super::{
    Context, DIVIDE_BY_ZERO, Destination, FloatingPointOperation, INEXACT, INVALID_OPERATION, Kind,
    OVERFLOW, Outcome, Precision, Real, Rounding, Source, UNDERFLOW, apply, compare, convert,
    low_ones, process_nan, process_nans, round, round_sum, to_integral, to_real, to_rounding,
    unpack,
};
use crate::condition_flags::ConditionFlags;

/// How a lane compares two values.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneComparison {
    Equal,
    GreaterOrEqual,
    Greater,
    /// The absolute values, as FACGE and FACGT compare them.
    AbsoluteGreaterOrEqual,
    AbsoluteGreater,
}

/// What a lane computes from its sources, all of one precision unless the operation says otherwise.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneOperation {
    /// A scalar instruction's formula, the result of the same precision.
    Scalar(FloatingPointOperation),
    /// [FPMulX](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPMulX): an infinity times a zero is two.
    MultiplyExtended,
    /// The absolute value of the difference, a NaN's sign cleared too.
    AbsoluteDifference,
    /// [FPRecipStepFused](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPRecipStepFused): two less the product.
    ReciprocalStep,
    /// [FPRSqrtStepFused](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPRSqrtStepFused): three less the product, halved.
    ReciprocalSquareRootStep,
    /// All ones where the comparison holds, the second source zero where the instruction names none.
    Compare(LaneComparison),
    /// The zero first: FCMLE and FCMLT compare zero with the source.
    CompareZeroWith(LaneComparison),
    /// [FPRecipEstimate](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPRecipEstimate).
    ReciprocalEstimate,
    /// [FPRSqrtEstimate](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPRSqrtEstimate).
    ReciprocalSquareRootEstimate,
    /// [FPRecpX](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPRecpX).
    ReciprocalExponent,
    /// [FPToFixed](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPToFixed) to an integer as wide as the lane.
    ToInteger {
        rounding: Option<Rounding>,
        unsigned: bool,
        fraction_bits: u32,
    },
    /// [FixedToFP](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FixedToFP) from an integer as wide as the lane.
    FromInteger { unsigned: bool, fraction_bits: u32 },
    /// FCVTL and FCVTN: to another precision.
    ConvertTo(Precision),
    /// FCVTXN: a double to a single, rounding to odd.
    ConvertToOdd,
}

/// The lane's result, adding the exception flags it raises to `flags`.
pub fn compute_lane(
    operation: LaneOperation,
    sources: &[u64],
    precision: Precision,
    fpcr: u64,
    flags: &mut u64,
) -> u64 {
    let mut context = Context {
        fpcr,
        flags: 0,
        conversion: matches!(
            operation,
            LaneOperation::ConvertTo(_) | LaneOperation::ConvertToOdd
        ),
    };
    let result = compute(operation, sources, precision, &mut context);
    *flags |= context.flags;
    result & low_ones(result_bits(operation, precision))
}

/// The width of the lane's result.
const fn result_bits(operation: LaneOperation, precision: Precision) -> u32 {
    match operation {
        LaneOperation::ConvertTo(target) => target.to_bits(),
        LaneOperation::ConvertToOdd => u32::BITS,
        _ => precision.to_bits(),
    }
}

fn compute(
    operation: LaneOperation,
    sources: &[u64],
    precision: Precision,
    context: &mut Context,
) -> u64 {
    let rounding = to_rounding(context.fpcr);
    let source = |index: usize| sources.get(index).copied().unwrap_or(0);
    match operation {
        LaneOperation::Scalar(scalar) => {
            let operands: [Source; 3] = core::array::from_fn(|index| Source::FloatingPoint {
                bits: source(index),
                precision,
            });
            let destination = Some(Destination::FloatingPoint {
                number: 0,
                precision,
            });
            match apply(
                scalar,
                &operands[..sources.len()],
                destination,
                0,
                0,
                ConditionFlags::empty(),
                context,
            ) {
                Outcome::Value(bits) => bits,
                Outcome::Flags(_) => unreachable!("a lane's formula gives a value"),
            }
        }
        LaneOperation::MultiplyExtended => {
            let first = unpack(source(0), precision, context);
            let second = unpack(source(1), precision, context);
            if let Some(nan) = process_nans(&[first, second], precision, context) {
                return nan;
            }
            let sign = first.sign != second.sign;
            match (first.kind, second.kind) {
                (Kind::Infinity, Kind::Zero) | (Kind::Zero, Kind::Infinity)
                    if crate::planted::is(1031) =>
                {
                    precision.to_default_nan()
                }
                (Kind::Infinity, Kind::Zero) | (Kind::Zero, Kind::Infinity) => {
                    to_two(sign, precision)
                }
                (Kind::Infinity, _) | (_, Kind::Infinity) => precision.to_infinity(sign),
                (Kind::Zero, _) | (_, Kind::Zero) => precision.to_zero(sign),
                _ => round(
                    super::multiply_reals(to_real(first), to_real(second)),
                    precision,
                    rounding,
                    context,
                ),
            }
        }
        LaneOperation::AbsoluteDifference => {
            let difference = compute(
                LaneOperation::Scalar(FloatingPointOperation::Subtract),
                sources,
                precision,
                context,
            );
            if crate::planted::is(1065) {
                let exponent_ones = precision.to_exponent_ones() << precision.to_fraction_bits();
                if difference & exponent_ones == exponent_ones
                    && difference & low_ones(precision.to_fraction_bits()) != 0
                {
                    return difference;
                }
            }
            difference & !precision.to_sign_bit()
        }
        LaneOperation::ReciprocalStep | LaneOperation::ReciprocalSquareRootStep => {
            let square_root = operation == LaneOperation::ReciprocalSquareRootStep;
            let negated = if crate::planted::is(1032) {
                0
            } else {
                precision.to_sign_bit()
            };
            let first = unpack(source(0) ^ negated, precision, context);
            let second = unpack(source(1), precision, context);
            if let Some(nan) = process_nans(&[first, second], precision, context) {
                return nan;
            }
            let sign = first.sign != second.sign;
            match (first.kind, second.kind) {
                (Kind::Infinity, Kind::Zero) | (Kind::Zero, Kind::Infinity) => {
                    if square_root {
                        to_one_point_five(precision)
                    } else {
                        to_two(false, precision)
                    }
                }
                (Kind::Infinity, _) | (_, Kind::Infinity) => precision.to_infinity(sign),
                _ => {
                    // Two, or three, plus the exact product, halved for the square root's step.
                    let constant = Real {
                        sign: false,
                        significand: if square_root { 3 } else { 2 },
                        exponent: 0,
                        sticky: false,
                    };
                    let product = super::multiply_reals(to_real(first), to_real(second));
                    let mut sum = super::add_reals(constant, product);
                    if square_root {
                        sum.exponent -= 1;
                    }
                    round_sum(sum, precision, rounding, context)
                }
            }
        }
        LaneOperation::Compare(comparison) => {
            let holds = compare_lane(comparison, source(0), source(1), precision, context);
            if holds {
                low_ones(precision.to_bits())
            } else {
                0
            }
        }
        LaneOperation::CompareZeroWith(comparison) => {
            let holds = compare_lane(comparison, 0, source(0), precision, context);
            if holds {
                low_ones(precision.to_bits())
            } else {
                0
            }
        }
        LaneOperation::ReciprocalEstimate => estimate_reciprocal(source(0), precision, context),
        LaneOperation::ReciprocalSquareRootEstimate => {
            estimate_reciprocal_square_root(source(0), precision, context)
        }
        LaneOperation::ReciprocalExponent => {
            let operand = unpack(source(0), precision, context);
            if operand.is_nan() {
                return process_nan(operand, precision, context);
            }
            let fraction_bits = precision.to_fraction_bits();
            let exponent = (source(0) >> fraction_bits) & precision.to_exponent_ones();
            let sign = source(0) & precision.to_sign_bit();
            let exponent = if exponent == 0 {
                precision.to_exponent_ones() - 1
            } else {
                !exponent & precision.to_exponent_ones()
            };
            sign | (exponent << fraction_bits)
        }
        LaneOperation::ToInteger {
            rounding: chosen,
            unsigned,
            fraction_bits,
        } => {
            let operand = unpack(source(0), precision, context);
            to_lane_integer(
                operand,
                fraction_bits,
                unsigned,
                precision.to_bits(),
                chosen.unwrap_or(rounding),
                context,
            )
        }
        LaneOperation::FromInteger {
            unsigned,
            fraction_bits,
        } => from_lane_integer(
            source(0),
            precision.to_bits(),
            fraction_bits,
            unsigned,
            precision,
            rounding,
            context,
        ),
        LaneOperation::ConvertTo(target) => {
            let operand = unpack(source(0), precision, context);
            convert(operand, precision, target, rounding, context)
        }
        LaneOperation::ConvertToOdd => {
            let operand = unpack(source(0), precision, context);
            if operand.kind != Kind::Number {
                return convert(
                    operand,
                    precision,
                    Precision::Single,
                    Rounding::TowardZero,
                    context,
                );
            }
            // FPRound with FPRounding_ODD: toward zero, then the lowest bit set where anything was discarded.
            let before = context.flags;
            context.flags &= !INEXACT;
            let rounded = round(
                Real::from_number(operand),
                Precision::Single,
                Rounding::TowardZero,
                context,
            );
            let inexact = context.flags & INEXACT != 0;
            context.flags |= before;
            if crate::planted::is(1040) {
                context.flags &= !INEXACT;
                let nearest = round(
                    Real::from_number(operand),
                    Precision::Single,
                    Rounding::TiesToEven,
                    context,
                );
                context.flags |= before;
                return nearest;
            }
            if inexact { rounded | 1 } else { rounded }
        }
    }
}

/// Two with the sign, as FPTwo gives it.
fn to_two(sign: bool, precision: Precision) -> u64 {
    precision.to_zero(sign) | ((precision.to_bias() as u64 + 1) << precision.to_fraction_bits())
}

/// One and a half, as FPOnePointFive gives it.
fn to_one_point_five(precision: Precision) -> u64 {
    ((precision.to_bias() as u64) << precision.to_fraction_bits()) | precision.to_quiet_bit()
}

/// FPCompareEQ, FPCompareGE and FPCompareGT: a NaN compares false, raising Invalid Operation where it signals or the
/// comparison is an ordered one.
fn compare_lane(
    comparison: LaneComparison,
    first: u64,
    second: u64,
    precision: Precision,
    context: &mut Context,
) -> bool {
    let absolute = matches!(
        comparison,
        LaneComparison::AbsoluteGreaterOrEqual | LaneComparison::AbsoluteGreater
    ) && !(crate::planted::is(1034)
        && comparison == LaneComparison::AbsoluteGreater);
    let strip = |bits: u64| {
        if absolute {
            bits & !precision.to_sign_bit()
        } else {
            bits
        }
    };
    let first = unpack(strip(first), precision, context);
    let second = unpack(strip(second), precision, context);
    let signaling = comparison != LaneComparison::Equal
        && !(crate::planted::is(1033) && comparison == LaneComparison::GreaterOrEqual);
    let flags = compare(first, second, signaling, context);
    let equal = ConditionFlags::ZERO | ConditionFlags::CARRY;
    match comparison {
        LaneComparison::Equal => flags == equal,
        LaneComparison::GreaterOrEqual | LaneComparison::AbsoluteGreaterOrEqual => {
            flags == equal || flags == ConditionFlags::CARRY
        }
        LaneComparison::Greater | LaneComparison::AbsoluteGreater => flags == ConditionFlags::CARRY,
    }
}

/// The exponent field's width, and the bias of a double, which the estimates scale every precision to.
const DOUBLE_FRACTION_BITS: u32 = 52;

/// The fraction of the operand as the 52 bits of a double's, and its biased exponent.
fn to_double_fraction(bits: u64, precision: Precision) -> (u64, i64) {
    let fraction_bits = precision.to_fraction_bits();
    let fraction = (bits & low_ones(fraction_bits)) << (DOUBLE_FRACTION_BITS - fraction_bits);
    let exponent = ((bits >> fraction_bits) & precision.to_exponent_ones()) as i64;
    (fraction, exponent)
}

/// [RecipEstimate](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=RecipEstimate): of `a` in 256 to 511, standing for 0.5 to 1.0, an estimate in 256 to 511.
pub fn estimate_reciprocal_fraction(a: u64) -> u64 {
    let a = a * 2 + 1;
    let b = (1 << 19) / a;
    if crate::planted::is(1035) {
        b / 2
    } else {
        b.div_ceil(2)
    }
}

/// [RecipSqrtEstimate](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=RecipSqrtEstimate): of `a` in 128 to 511, standing for 0.25 to 1.0, an estimate in 256 to 511.
pub fn estimate_reciprocal_square_root_fraction(a: u64) -> u64 {
    let a = if a < 256 {
        a * 2 + 1
    } else {
        ((a >> 1) << 1) * 2 + 2
    };
    let mut b = 512;
    while a * (b + 1) * (b + 1) < 1 << 28 {
        b += 1;
    }
    b.div_ceil(2)
}

fn estimate_reciprocal(bits: u64, precision: Precision, context: &mut Context) -> u64 {
    let operand = unpack(bits, precision, context);
    let sign = operand.sign;
    let rounding = to_rounding(context.fpcr);
    match operand.kind {
        Kind::QuietNaN | Kind::SignalingNaN => return process_nan(operand, precision, context),
        Kind::Infinity => return precision.to_zero(sign),
        Kind::Zero => {
            context.raise(DIVIDE_BY_ZERO);
            return precision.to_infinity(sign);
        }
        Kind::Number => {}
    }
    // The value's own exponent, of its most significant bit.
    let top = (u64::BITS - 1 - operand.mantissa.leading_zeros()) as i32 + operand.exponent;
    let bias = precision.to_bias();
    // Below two to the -(bias + 1), whose reciprocal overflows.
    if top < -(bias + 1) {
        let to_infinity = match rounding {
            Rounding::TiesToEven | Rounding::TiesAway => true,
            Rounding::TowardPlusInfinity => !sign,
            Rounding::TowardMinusInfinity => sign,
            Rounding::TowardZero => false,
        };
        context.raise(OVERFLOW);
        context.raise(INEXACT);
        return if to_infinity {
            precision.to_infinity(sign)
        } else {
            precision.to_maximum_normal(sign)
        };
    }
    // At or above two to (bias - 1), whose reciprocal is below the normal range, flushed where FPCR asks.
    if context.flushes(precision) && top >= bias - 1 {
        context.raise(UNDERFLOW);
        return precision.to_zero(sign);
    }
    let (mut fraction, mut exponent) = to_double_fraction(bits, precision);
    if exponent == 0 {
        if fraction >> (DOUBLE_FRACTION_BITS - 1) == 0 {
            exponent = -1;
            fraction = (fraction << 2) & low_ones(DOUBLE_FRACTION_BITS);
        } else {
            fraction = (fraction << 1) & low_ones(DOUBLE_FRACTION_BITS);
        }
    }
    let scaled = (1 << 8) | (fraction >> (DOUBLE_FRACTION_BITS - 8));
    let mut result_exponent = 2 * i64::from(bias) - 1 - exponent;
    let estimate = estimate_reciprocal_fraction(scaled);
    let mut fraction = (estimate & 0xFF) << (DOUBLE_FRACTION_BITS - 8);
    if result_exponent == 0 {
        fraction = (1 << (DOUBLE_FRACTION_BITS - 1)) | (fraction >> 1);
    } else if result_exponent == -1 {
        fraction = (1 << (DOUBLE_FRACTION_BITS - 2)) | (fraction >> 2);
        result_exponent = 0;
    }
    let fraction_bits = precision.to_fraction_bits();
    precision.to_zero(sign)
        | ((result_exponent as u64 & precision.to_exponent_ones()) << fraction_bits)
        | (fraction >> (DOUBLE_FRACTION_BITS - fraction_bits))
}

fn estimate_reciprocal_square_root(bits: u64, precision: Precision, context: &mut Context) -> u64 {
    let operand = unpack(bits, precision, context);
    match operand.kind {
        Kind::QuietNaN | Kind::SignalingNaN => return process_nan(operand, precision, context),
        Kind::Zero => {
            context.raise(DIVIDE_BY_ZERO);
            return precision.to_infinity(operand.sign);
        }
        _ if operand.sign => {
            context.raise(INVALID_OPERATION);
            return precision.to_default_nan();
        }
        Kind::Infinity => return 0,
        Kind::Number => {}
    }
    let (mut fraction, mut exponent) = to_double_fraction(bits, precision);
    if exponent == 0 {
        while fraction >> (DOUBLE_FRACTION_BITS - 1) == 0 {
            fraction = (fraction << 1) & low_ones(DOUBLE_FRACTION_BITS);
            exponent -= 1;
        }
        fraction = (fraction << 1) & low_ones(DOUBLE_FRACTION_BITS);
    }
    let scaled = if (exponent & 1 == 1) != crate::planted::is(1036) {
        (1 << 7) | (fraction >> (DOUBLE_FRACTION_BITS - 7))
    } else {
        (1 << 8) | (fraction >> (DOUBLE_FRACTION_BITS - 8))
    };
    let bias = i64::from(precision.to_bias());
    let result_exponent = (3 * bias - 1 - exponent).div_euclid(2);
    let estimate = estimate_reciprocal_square_root_fraction(scaled);
    let fraction_bits = precision.to_fraction_bits();
    ((result_exponent as u64 & precision.to_exponent_ones()) << fraction_bits)
        | ((estimate & 0xFF) << (fraction_bits - 8))
}

/// FPToFixed to an integer of `bits` bits, saturated with Invalid Operation.
fn to_lane_integer(
    operand: super::Unpacked,
    fraction_bits: u32,
    unsigned: bool,
    bits: u32,
    rounding: Rounding,
    context: &mut Context,
) -> u64 {
    let (sign, magnitude, inexact) = match operand.kind {
        Kind::QuietNaN | Kind::SignalingNaN => {
            context.raise(INVALID_OPERATION);
            return 0;
        }
        Kind::Zero => return 0,
        Kind::Infinity => (operand.sign, u128::MAX, false),
        Kind::Number => to_integral(operand, fraction_bits, rounding),
    };
    let (minimum, maximum): (i128, i128) = if unsigned {
        (0, (1 << bits) - 1)
    } else {
        (-(1 << (bits - 1)), (1 << (bits - 1)) - 1)
    };
    let magnitude = i128::try_from(magnitude).unwrap_or(i128::MAX);
    let value = if sign { -magnitude } else { magnitude };
    if value < minimum || value > maximum {
        if !crate::planted::is(1037) {
            context.raise(INVALID_OPERATION);
        }
        return value.clamp(minimum, maximum) as u64 & low_ones(bits);
    }
    if inexact {
        context.raise(INEXACT);
    }
    value as u64 & low_ones(bits)
}

/// FixedToFP from an integer of `bits` bits divided by two to `fraction_bits`.
fn from_lane_integer(
    value: u64,
    bits: u32,
    fraction_bits: u32,
    unsigned: bool,
    precision: Precision,
    rounding: Rounding,
    context: &mut Context,
) -> u64 {
    let value = value & low_ones(bits);
    let negative = !unsigned && !crate::planted::is(1038) && (value >> (bits - 1)) & 1 != 0;
    let magnitude = if negative {
        value.wrapping_neg() & low_ones(bits)
    } else {
        value
    };
    let magnitude = if negative && magnitude == 0 {
        1 << (bits - 1)
    } else {
        magnitude
    };
    if magnitude == 0 {
        return precision.to_zero(false);
    }
    round(
        Real {
            sign: negative,
            significand: u128::from(magnitude),
            exponent: -(fraction_bits as i32),
            sticky: false,
        },
        precision,
        rounding,
        context,
    )
}

/// [FPMulAddH](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPMulAddH): a single-precision addend plus the exact product of two
/// half-precision values, rounded once; a NaN of a half-precision source is widened as FPConvertNaN does.
pub fn multiply_add_halves(
    addend: u64,
    first: u64,
    second: u64,
    fpcr: u64,
    flags: &mut u64,
) -> u64 {
    let mut context = Context {
        fpcr,
        flags: 0,
        conversion: false,
    };
    let rounding = to_rounding(fpcr);
    let addend = unpack(addend, Precision::Single, &mut context);
    let first = unpack(first, Precision::Half, &mut context);
    let second = unpack(second, Precision::Half, &mut context);
    let infinite_times_zero = (first.kind == Kind::Infinity && second.kind == Kind::Zero)
        || (first.kind == Kind::Zero && second.kind == Kind::Infinity);
    let operands = [
        (addend, Precision::Single),
        (first, Precision::Half),
        (second, Precision::Half),
    ];
    let nan = operands
        .iter()
        .find(|(operand, _)| operand.kind == Kind::SignalingNaN)
        .or_else(|| {
            operands
                .iter()
                .find(|(operand, _)| operand.kind == Kind::QuietNaN)
        })
        .copied();
    let result = if let Some((nan, precision)) = nan {
        let converted = convert(nan, precision, Precision::Single, rounding, &mut context);
        if addend.kind == Kind::QuietNaN && infinite_times_zero {
            context.raise(INVALID_OPERATION);
            Precision::Single.to_default_nan()
        } else {
            converted
        }
    } else {
        super::multiply_add(
            addend,
            first,
            second,
            Precision::Single,
            rounding,
            &mut context,
        )
    };
    *flags |= context.flags;
    result & low_ones(u32::BITS)
}

/// The fraction bits of a BFloat16 value, which has a single's exponent.
const BRAIN_FLOAT_FRACTION_BITS: u32 = 7;
/// The bits a BFloat16 value lies above in the single it widens to.
const BRAIN_FLOAT_SHIFT: u32 = u16::BITS;

/// [FPConvertBF](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPConvertBF): a single rounded to BFloat16 as FPCR asks, as FPRoundBase does with
/// a single's exponent range and seven fraction bits.
pub fn convert_to_brain_float(bits: u64, fpcr: u64, flags: &mut u64) -> u64 {
    let mut context = Context {
        fpcr,
        flags: 0,
        conversion: false,
    };
    let operand = unpack(bits, Precision::Single, &mut context);
    let single = match operand.kind {
        Kind::QuietNaN | Kind::SignalingNaN => {
            if operand.kind == Kind::SignalingNaN {
                context.raise(INVALID_OPERATION);
            }
            if context.default_nan() {
                Precision::Single.to_default_nan()
            } else {
                bits | Precision::Single.to_quiet_bit()
            }
        }
        Kind::Infinity => Precision::Single.to_infinity(operand.sign),
        Kind::Zero => Precision::Single.to_zero(operand.sign),
        Kind::Number => {
            // Rounding the real to a single whose fraction ends seven bits down: the single's own rounding, with the
            // value scaled so that its lowest kept bit is the BFloat16's.
            let value = Real::from_number(operand);
            let rounding = if crate::planted::is(1061) {
                Rounding::TowardZero
            } else {
                to_rounding(fpcr)
            };
            round_to_fraction_bits(value, rounding, &mut context)
        }
    };
    *flags |= context.flags;
    (single >> BRAIN_FLOAT_SHIFT) & low_ones(u16::BITS)
}

/// FPRoundBase of a nonzero real to a single's exponent range and BFloat16's fraction, the result as a single's bits.
fn round_to_fraction_bits(value: Real, rounding: Rounding, context: &mut Context) -> u64 {
    let precision = Precision::Single;
    let dropped = precision.to_fraction_bits() - BRAIN_FLOAT_FRACTION_BITS;
    let top = (u128::BITS - 1 - value.significand.leading_zeros()) as i32 + value.exponent;
    let minimum = precision.to_minimum_exponent();
    if context.flushes(precision) && top < minimum {
        context.raise(UNDERFLOW);
        return precision.to_zero(value.sign);
    }
    let biased = (top - minimum + 1).max(0);
    let scale = if biased == 0 { minimum } else { top };
    let shift = scale - BRAIN_FLOAT_FRACTION_BITS as i32 - value.exponent;
    let (mut mantissa, discarded, inexact) = if shift <= 0 {
        (
            value.significand << (-shift) as u32,
            core::cmp::Ordering::Less,
            value.sticky,
        )
    } else {
        super::shift_right_rounding(value.significand, shift as u32, value.sticky)
    };
    if biased == 0 && inexact {
        context.raise(UNDERFLOW);
    }
    let mut biased = biased;
    if super::rounds_up(rounding, value.sign, mantissa & 1 != 0, discarded, inexact) {
        mantissa += 1;
        if mantissa == 1 << BRAIN_FLOAT_FRACTION_BITS {
            biased = 1;
        }
        if mantissa == 1 << (BRAIN_FLOAT_FRACTION_BITS + 1) {
            biased += 1;
            mantissa >>= 1;
        }
    }
    let mut inexact = inexact;
    let result = if biased as u64 >= precision.to_exponent_ones() {
        context.raise(OVERFLOW);
        inexact = true;
        let to_infinity = match rounding {
            Rounding::TiesToEven | Rounding::TiesAway => true,
            Rounding::TowardPlusInfinity => !value.sign,
            Rounding::TowardMinusInfinity => value.sign,
            Rounding::TowardZero => false,
        };
        if to_infinity {
            precision.to_infinity(value.sign)
        } else {
            precision.to_maximum_normal(value.sign) & !low_ones(dropped)
        }
    } else {
        precision.to_zero(value.sign)
            | ((biased as u64) << precision.to_fraction_bits())
            | ((mantissa as u64 & low_ones(BRAIN_FLOAT_FRACTION_BITS)) << dropped)
    };
    if inexact {
        context.raise(INEXACT);
    }
    result
}

/// [BFRound](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=BFRound): a nonzero real to a single, rounding to odd, a value below the normal range
/// flushed to zero and one past the largest infinite, raising no flag.
fn round_brain_odd(value: Real) -> u64 {
    let mut context = Context {
        fpcr: super::FLUSH_TO_ZERO,
        flags: 0,
        conversion: false,
    };
    let rounded = round(value, Precision::Single, Rounding::TowardZero, &mut context);
    if context.flags & OVERFLOW != 0 {
        return Precision::Single.to_infinity(value.sign);
    }
    if context.flags & INEXACT != 0 && context.flags & UNDERFLOW == 0 && !crate::planted::is(1060) {
        rounded | 1
    } else {
        rounded
    }
}

/// BFUnpack: a single's kind and value, a subnormal as zero.
fn unpack_brain(bits: u64) -> super::Unpacked {
    let mut context = Context {
        fpcr: super::FLUSH_TO_ZERO,
        flags: 0,
        conversion: false,
    };
    unpack(bits, Precision::Single, &mut context)
}

/// BFMul: the product of two BFloat16 values as a single, every NaN the default NaN.
fn multiply_brain(first: u64, second: u64) -> u64 {
    let precision = Precision::Single;
    let first = unpack_brain(first << BRAIN_FLOAT_SHIFT);
    let second = unpack_brain(second << BRAIN_FLOAT_SHIFT);
    let sign = first.sign != second.sign;
    match (first.kind, second.kind) {
        _ if first.is_nan() || second.is_nan() => precision.to_default_nan(),
        (Kind::Infinity, Kind::Zero) | (Kind::Zero, Kind::Infinity) => precision.to_default_nan(),
        (Kind::Infinity, _) | (_, Kind::Infinity) => precision.to_infinity(sign),
        (Kind::Zero, _) | (_, Kind::Zero) => precision.to_zero(sign),
        _ => round_brain_odd(super::multiply_reals(to_real(first), to_real(second))),
    }
}

/// BFAdd: the sum of two singles, every NaN the default NaN, an exact zero positive.
fn add_brain(first: u64, second: u64) -> u64 {
    let precision = Precision::Single;
    let first = unpack_brain(first);
    let second = unpack_brain(second);
    match (first.kind, second.kind) {
        _ if first.is_nan() || second.is_nan() => precision.to_default_nan(),
        (Kind::Infinity, Kind::Infinity) if first.sign != second.sign => precision.to_default_nan(),
        (Kind::Infinity, _) => precision.to_infinity(first.sign),
        (_, Kind::Infinity) => precision.to_infinity(second.sign),
        (Kind::Zero, Kind::Zero) if first.sign == second.sign => precision.to_zero(first.sign),
        _ => {
            let sum = super::add_reals(to_real(first), to_real(second));
            if sum.is_zero() {
                precision.to_zero(false)
            } else {
                round_brain_odd(sum)
            }
        }
    }
}

/// [BFDotAdd](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=BFDotAdd) without FEAT_EBF16: the addend plus the sum of two products of BFloat16 pairs.
pub fn dot_add_brain(addend: u64, first: [u64; 2], second: [u64; 2]) -> u64 {
    let product = add_brain(
        multiply_brain(first[0], second[0]),
        multiply_brain(first[1], second[1]),
    );
    add_brain(addend, product) & low_ones(u32::BITS)
}
