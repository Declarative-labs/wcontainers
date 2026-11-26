//! One element of an SVE floating-point instruction, computed with the scalar pseudocode of the parent module and the
//! functions SVE adds to it, each linked to its page of Arm's
//! [shared pseudocode](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float).

use super::{
    ALTERNATIVE_HALF, Context, Destination, FloatingPointOperation, INEXACT, INVALID_OPERATION,
    Kind, Outcome, Precision, Real, Rounding, Source, Unpacked, add_reals, apply, compare,
    multiply, multiply_add, multiply_reals, process_nan, process_nans, round, round_sum,
    to_integral, to_real, to_rounding, unpack,
};
use crate::ConditionFlags;

/// What an SVE floating-point instruction computes for one element of its sources.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ElementOperation {
    /// The scalar family's operation on the element's sources in their order.
    Scalar(FloatingPointOperation),
    /// FSUBR and FDIVR: the scalar operation with its two sources swapped.
    Reversed(FloatingPointOperation),
    /// FABD: the difference, its sign bit cleared, a NaN's too.
    AbsoluteDifference,
    /// [FMULX](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.FPMulX.3):
    /// a product, but two of the product's sign for an infinity times a zero.
    MultiplyExtended,
    /// [FRECPS](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.FPRecipStepFused.3): two less the product, rounded once.
    ReciprocalStep,
    /// [FRSQRTS](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.FPRSqrtStepFused.3): three less the product, halved, rounded once.
    ReciprocalSquareRootStep,
    /// [FSCALE](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.FPScale.3): the first times two to the second, a signed integer of the element's size.
    Scale,
    /// [FRECPX](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.FPRecpX.2): the sign and the complement of the exponent.
    ReciprocalExponent,
}

/// What an SVE floating-point compare asks of two elements.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ElementComparison {
    Equal,
    NotEqual,
    GreaterOrEqual,
    Greater,
    /// Either is a NaN.
    Unordered,
    /// The magnitudes, greater or equal.
    AbsoluteGreaterOrEqual,
    AbsoluteGreater,
}

/// A conversion of one element.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Conversion {
    /// FCVT, which ignores FPCR.AHP.
    Precision { from: Precision, to: Precision },
    /// FCVTZS and FCVTZU: toward zero, saturating at an integer of `bits` bits.
    ToInteger {
        from: Precision,
        bits: u32,
        unsigned: bool,
    },
    /// SCVTF and UCVTF from an integer of `bits` bits.
    FromInteger {
        bits: u32,
        unsigned: bool,
        to: Precision,
    },
}

fn to_context(fpcr: u64) -> Context {
    Context {
        fpcr,
        flags: 0,
        conversion: false,
    }
}

/// The scalar operation's value on sources of the precision.
fn apply_scalar(
    operation: FloatingPointOperation,
    sources: &[u64],
    precision: Precision,
    context: &mut Context,
) -> u64 {
    let mut typed = [Source::Immediate(0); 3];
    for (place, bits) in sources.iter().enumerate() {
        typed[place] = Source::FloatingPoint {
            bits: *bits,
            precision,
        };
    }
    let destination = Destination::FloatingPoint {
        number: 0,
        precision,
    };
    match apply(
        operation,
        &typed[..sources.len()],
        Some(destination),
        0,
        0,
        ConditionFlags::empty(),
        context,
    ) {
        Outcome::Value(bits) => bits,
        Outcome::Flags(_) | Outcome::ValueAndFlags(..) => {
            unreachable!("an element operation gives only a value")
        }
    }
}

/// The element's result of the operation on its sources, adding the exception flags it raises.
pub fn compute_element(
    operation: ElementOperation,
    sources: &[u64],
    precision: Precision,
    fpcr: u64,
    flags: &mut u64,
) -> u64 {
    let mut context = to_context(fpcr);
    let rounding = to_rounding(fpcr);
    let result = match operation {
        ElementOperation::Scalar(scalar) => apply_scalar(scalar, sources, precision, &mut context),
        ElementOperation::Reversed(scalar) if crate::planted::is(2016) => {
            apply_scalar(scalar, sources, precision, &mut context)
        }
        ElementOperation::Reversed(scalar) => {
            apply_scalar(scalar, &[sources[1], sources[0]], precision, &mut context)
        }
        ElementOperation::AbsoluteDifference if crate::planted::is(2021) => {
            let difference = apply_scalar(
                FloatingPointOperation::Subtract,
                sources,
                precision,
                &mut context,
            );
            if super::is_nan_bits(difference, precision) {
                difference
            } else {
                difference & !precision.to_sign_bit()
            }
        }
        ElementOperation::AbsoluteDifference => {
            apply_scalar(
                FloatingPointOperation::Subtract,
                sources,
                precision,
                &mut context,
            ) & !precision.to_sign_bit()
        }
        ElementOperation::MultiplyExtended => {
            let first = unpack(sources[0], precision, &mut context);
            let second = unpack(sources[1], precision, &mut context);
            match process_nans(&[first, second], precision, &mut context) {
                Some(nan) => nan,
                None if is_infinity_times_zero(first, second) && !crate::planted::is(2017) => {
                    to_constant(1, first.sign != second.sign, precision)
                }
                None => multiply(first, second, precision, rounding, &mut context),
            }
        }
        ElementOperation::ReciprocalStep => step(sources, precision, rounding, false, &mut context),
        ElementOperation::ReciprocalSquareRootStep => {
            step(sources, precision, rounding, true, &mut context)
        }
        ElementOperation::Scale => {
            let operand = unpack(sources[0], precision, &mut context);
            let shift = precision.to_bits();
            let scale = if crate::planted::is(2020) {
                (sources[1] & (u64::MAX >> (u64::BITS - shift))) as i64
            } else {
                ((sources[1] << (u64::BITS - shift)) as i64) >> (u64::BITS - shift)
            };
            match operand.kind {
                Kind::QuietNaN | Kind::SignalingNaN => {
                    process_nan(operand, precision, &mut context)
                }
                Kind::Zero => precision.to_zero(operand.sign),
                Kind::Infinity => precision.to_infinity(operand.sign),
                Kind::Number => {
                    // Far past every format's range either way, so that the clamp changes no result.
                    const LIMIT: i64 = 1 << 16;
                    let mut real = Real::from_number(operand);
                    real.exponent += scale.clamp(-LIMIT, LIMIT) as i32;
                    round(real, precision, rounding, &mut context)
                }
            }
        }
        ElementOperation::ReciprocalExponent => {
            let operand = unpack(sources[0], precision, &mut context);
            let fraction_bits = precision.to_fraction_bits();
            let exponent = (sources[0] >> fraction_bits) & precision.to_exponent_ones();
            let sign = sources[0] & precision.to_sign_bit();
            match operand.kind {
                Kind::QuietNaN | Kind::SignalingNaN => {
                    process_nan(operand, precision, &mut context)
                }
                _ if exponent == 0 && !crate::planted::is(2030) => {
                    sign | ((precision.to_exponent_ones() - 1) << fraction_bits)
                }
                _ => sign | ((!exponent & precision.to_exponent_ones()) << fraction_bits),
            }
        }
    };
    *flags |= context.flags;
    result
}

fn is_infinity_times_zero(first: Unpacked, second: Unpacked) -> bool {
    (first.kind == Kind::Infinity && second.kind == Kind::Zero)
        || (first.kind == Kind::Zero && second.kind == Kind::Infinity)
}

/// The value two to `exponent` of the precision, with the sign: 0.5, 1.0, 2.0 for -1, 0 and 1.
pub fn to_constant(exponent: i32, sign: bool, precision: Precision) -> u64 {
    precision.to_zero(sign)
        | (((precision.to_bias() + exponent) as u64) << precision.to_fraction_bits())
}

/// FRECPS and FRSQRTS: the first source negated, then two (or three, halved) plus the product, rounded once.
fn step(
    sources: &[u64],
    precision: Precision,
    rounding: Rounding,
    square_root: bool,
    context: &mut Context,
) -> u64 {
    let negated = if crate::planted::is(2018) {
        0
    } else {
        precision.to_sign_bit()
    };
    let first = unpack(sources[0] ^ negated, precision, context);
    let second = unpack(sources[1], precision, context);
    if let Some(nan) = process_nans(&[first, second], precision, context) {
        return nan;
    }
    if is_infinity_times_zero(first, second) {
        // FPTwo or FPOnePointFive, positive.
        return if square_root {
            to_constant(0, false, precision) | 1 << (precision.to_fraction_bits() - 1)
        } else {
            to_constant(1, false, precision)
        };
    }
    if first.kind == Kind::Infinity || second.kind == Kind::Infinity {
        return precision.to_infinity(first.sign != second.sign);
    }
    let constant = if square_root { 3 } else { 2 };
    let product = if first.kind == Kind::Zero || second.kind == Kind::Zero {
        Real {
            sign: first.sign != second.sign,
            significand: 0,
            exponent: 0,
            sticky: false,
        }
    } else {
        multiply_reals(to_real(first), to_real(second))
    };
    let mut sum = add_reals(
        Real {
            sign: false,
            significand: constant,
            exponent: 0,
            sticky: false,
        },
        product,
    );
    if square_root && !crate::planted::is(2019) {
        sum.exponent -= 1;
    }
    round_sum(sum, precision, rounding, context)
}

/// Whether the compare holds for the two elements, adding the exception flags it raises: Invalid Operation for a
/// signaling NaN, or for any NaN where the compare is an ordering.
pub fn compare_elements(
    comparison: ElementComparison,
    first: u64,
    second: u64,
    precision: Precision,
    fpcr: u64,
    flags: &mut u64,
) -> bool {
    use ElementComparison as C;
    let mut context = to_context(fpcr);
    let magnitude = |bits: u64| match comparison {
        C::AbsoluteGreaterOrEqual | C::AbsoluteGreater => bits & !precision.to_sign_bit(),
        _ => bits,
    };
    let first = unpack(magnitude(first), precision, &mut context);
    let second = unpack(magnitude(second), precision, &mut context);
    let signaling = !matches!(comparison, C::Equal | C::NotEqual | C::Unordered)
        && !(crate::planted::is(2023) && comparison == C::GreaterOrEqual);
    let outcome = compare(first, second, signaling, &mut context);
    *flags |= context.flags;
    let unordered = outcome.contains(ConditionFlags::OVERFLOW);
    let equal = outcome.contains(ConditionFlags::ZERO);
    let greater = outcome == ConditionFlags::CARRY;
    match comparison {
        C::Equal => equal,
        C::NotEqual if crate::planted::is(2022) => !equal && !unordered,
        C::NotEqual => !equal,
        C::GreaterOrEqual | C::AbsoluteGreaterOrEqual => equal || greater,
        C::Greater | C::AbsoluteGreater => greater,
        C::Unordered => unordered,
    }
}

/// The converted element, adding the exception flags the conversion raises.
pub fn convert_element(conversion: Conversion, source: u64, fpcr: u64, flags: &mut u64) -> u64 {
    let mut context = to_context(fpcr);
    let rounding = to_rounding(fpcr);
    let result = match conversion {
        Conversion::Precision { from, to } => {
            if !crate::planted::is(2027) {
                context.fpcr &= !ALTERNATIVE_HALF;
            }
            context.conversion = true;
            let operand = unpack(source, from, &mut context);
            super::convert(operand, from, to, rounding, &mut context)
        }
        Conversion::ToInteger {
            from,
            bits,
            unsigned,
        } => {
            let operand = unpack(source, from, &mut context);
            to_integer(operand, bits, unsigned, &mut context)
        }
        Conversion::FromInteger { bits, unsigned, to } => {
            let mask = u64::MAX >> (u64::BITS - bits);
            let negative = !unsigned && (source >> (bits - 1)) & 1 != 0;
            let magnitude = if negative {
                source.wrapping_neg() & mask
            } else {
                source & mask
            };
            // The most negative integer's magnitude is one past the signed range, and still fits the bits.
            let magnitude = if negative && magnitude == 0 {
                1 << (bits - 1)
            } else {
                magnitude
            };
            if magnitude == 0 {
                to.to_zero(false)
            } else {
                round(
                    Real {
                        sign: negative,
                        significand: u128::from(magnitude),
                        exponent: 0,
                        sticky: false,
                    },
                    to,
                    rounding,
                    &mut context,
                )
            }
        }
    };
    *flags |= context.flags;
    result
}

/// [FPToFixed](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.FPToFixed.5)
/// toward zero, at an integer of `bits` bits: saturated with Invalid Operation, else Inexact where it rounded.
fn to_integer(operand: Unpacked, bits: u32, unsigned: bool, context: &mut Context) -> u64 {
    let mask = u64::MAX >> (u64::BITS - bits);
    let (sign, magnitude, inexact) = match operand.kind {
        Kind::QuietNaN | Kind::SignalingNaN => {
            context.raise(INVALID_OPERATION);
            return 0;
        }
        Kind::Zero => return 0,
        Kind::Infinity => (operand.sign, u128::MAX, false),
        Kind::Number => to_integral(operand, 0, Rounding::TowardZero),
    };
    let (minimum, maximum): (i128, i128) = if unsigned {
        (0, (1 << bits) - 1)
    } else {
        (-(1 << (bits - 1)), (1 << (bits - 1)) - 1)
    };
    let magnitude = i128::try_from(magnitude).unwrap_or(i128::MAX);
    let value = if sign { -magnitude } else { magnitude };
    if value < minimum || value > maximum {
        context.raise(INVALID_OPERATION);
        return value.clamp(minimum, maximum) as u64 & mask;
    }
    if inexact {
        context.raise(INEXACT);
    }
    value as u64 & mask
}

/// FMLA and its forms on one element: the addend, the last source, plus the product of the first two, each negated
/// where asked first.
pub fn multiply_add_element(
    [first, second, addend]: [u64; 3],
    negate_product: bool,
    negate_addend: bool,
    precision: Precision,
    fpcr: u64,
    flags: &mut u64,
) -> u64 {
    let mut context = to_context(fpcr);
    let negate = |bits: u64, negated: bool| {
        if negated {
            bits ^ precision.to_sign_bit()
        } else {
            bits
        }
    };
    let first = unpack(negate(first, negate_product), precision, &mut context);
    let second = unpack(second, precision, &mut context);
    let addend = unpack(negate(addend, negate_addend), precision, &mut context);
    let result = multiply_add(
        addend,
        first,
        second,
        precision,
        to_rounding(fpcr),
        &mut context,
    );
    *flags |= context.flags;
    result
}
