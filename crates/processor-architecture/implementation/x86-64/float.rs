//! The legacy SSE floating-point forms that share one skeleton: ADD, SUB, MUL, DIV, MIN, MAX and SQRT of single and
//! double precision, packed or scalar, with the source a register or memory, under MXCSR.
//!
//! Each lane is computed exactly and rounded once, in MXCSR's rounding mode, as IEEE 754 and Intel's manual give it:
//! DAZ reads a denormal operand as a zero of its sign; FTZ writes a zero for a tiny result while underflow is
//! masked; tininess is detected after rounding. The exceptions of every lane before the computation (invalid,
//! denormal operand, divide by zero) come first: if any is unmasked, #XM is raised with their flags set and no
//! result written. Then those after it (overflow, underflow, inexact): if any is unmasked, #XM with every flag set
//! and no result written. Otherwise every lane is written and the flags are added to MXCSR.

use iced_x86::Code;
use crate::forms::Instruction;

use crate::integer::OperandPlace;
use crate::planted::{Bug, is_active};

/// MXCSR's exception flags, in the order of its bits; each mask is the flag's bit shifted by [`MASK_SHIFT`].
pub const INVALID: u32 = 1 << 0;
pub const DENORMAL: u32 = 1 << 1;
pub const DIVIDE_BY_ZERO: u32 = 1 << 2;
pub const OVERFLOW: u32 = 1 << 3;
pub const UNDERFLOW: u32 = 1 << 4;
pub const INEXACT: u32 = 1 << 5;
pub const DENORMALS_ARE_ZEROS: u32 = 1 << 6;
pub const MASK_SHIFT: u32 = 7;
pub const ROUNDING_SHIFT: u32 = 13;
pub const FLUSH_TO_ZERO: u32 = 1 << 15;
const FLAGS: u32 = INVALID | DENORMAL | DIVIDE_BY_ZERO | OVERFLOW | UNDERFLOW | INEXACT;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FloatOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    /// MIN: the source wherever either is a NaN or both are zeros.
    Minimum,
    /// MAX: the source wherever either is a NaN or both are zeros.
    Maximum,
    /// SQRT of the source.
    SquareRoot,
    /// CMPccPS to CMPccSD: all ones where the immediate's low three bits' predicate holds, else zero; LT, LE,
    /// NLT and NLE raise invalid for a quiet NaN too.
    Compare,
    /// COMISS, COMISD (`signaling`, invalid for any NaN) and UCOMISS, UCOMISD (invalid for a signaling NaN): ZF, PF
    /// and CF from the order, all three set when unordered, OF, SF and AF cleared; no lane written.
    OrderedCompare { signaling: bool },
    /// ADDSUBPS, ADDSUBPD: the even lanes subtracted, the odd ones added.
    AddSubtract,
    /// HADDPS, HADDPD: the sums of adjacent lanes, the destination's then the source's.
    HorizontalAdd,
    /// HSUBPS, HSUBPD: each even lane less the odd lane above it, the destination's then the source's.
    HorizontalSubtract,
    /// The CVT forms between precisions and between precisions and signed doublewords.
    Convert(Conversion),
}

/// What a conversion turns its source's lanes into; the form's precision is that of its floating-point side, the
/// source's but for [`Self::IntegerToFloat`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Conversion {
    /// CVTSS2SD, CVTPS2PD: exact, a NaN quieted.
    SingleToDouble,
    /// CVTSD2SS, CVTPD2PS: rounded under MXCSR, a NaN quieted with the fraction's top bits.
    DoubleToSingle,
    /// CVTDQ2PS, CVTDQ2PD: signed doublewords rounded under MXCSR.
    IntegerToFloat,
    /// CVTPS2DQ, CVTPD2DQ, and with `truncate` CVTTPS2DQ, CVTTPD2DQ: signed doublewords, a NaN or a value out of
    /// their range invalid and written as 0x8000_0000; no denormal flag.
    FloatToInteger { truncate: bool },
}

/// The integer a conversion writes for a NaN or a value out of range.
const INTEGER_INDEFINITE: u64 = 1 << 31;

impl Conversion {
    /// The bits of a source lane and of a destination lane.
    pub const fn to_widths(self, precision: Precision) -> (u32, u32) {
        match self {
            Self::SingleToDouble => (u32::BITS, u64::BITS),
            Self::DoubleToSingle => (u64::BITS, u32::BITS),
            Self::IntegerToFloat => (u32::BITS, precision.to_bits()),
            Self::FloatToInteger { .. } => (precision.to_bits(), u32::BITS),
        }
    }
}

/// RFLAGS' ZF, PF and CF, as an ordered compare's lane result holds them, in their places.
pub const ORDER_FLAGS: u64 = (1 << 6) | (1 << 2) | 1;

impl FloatOperation {
    /// Whether the operation writes RFLAGS rather than a lane.
    pub const fn writes_flags(self) -> bool {
        matches!(self, Self::OrderedCompare { .. })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Precision {
    Single,
    Double,
}

/// Whether the operation works on every lane or on the lowest alone, leaving the others as they were.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arrangement {
    Packed,
    Scalar,
}

/// One legacy SSE floating-point form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FloatForm {
    pub instruction: Instruction,
    pub operation: FloatOperation,
    pub precision: Precision,
    pub arrangement: Arrangement,
    pub place: OperandPlace,
    pub code: Code,
}

impl Precision {
    pub const fn to_bits(self) -> u32 {
        match self {
            Self::Single => u32::BITS,
            Self::Double => u64::BITS,
        }
    }

    /// The significand's bits, the leading one included.
    pub const fn to_significand_bits(self) -> u32 {
        match self {
            Self::Single => f32::MANTISSA_DIGITS,
            Self::Double => f64::MANTISSA_DIGITS,
        }
    }

    const fn to_exponent_bits(self) -> u32 {
        self.to_bits() - self.to_significand_bits()
    }

    /// The exponent of the smallest normal number, unbiased.
    const fn to_minimum_exponent(self) -> i32 {
        match self {
            Self::Single => f32::MIN_EXP - 1,
            Self::Double => f64::MIN_EXP - 1,
        }
    }

    const fn to_maximum_exponent(self) -> i32 {
        match self {
            Self::Single => f32::MAX_EXP - 1,
            Self::Double => f64::MAX_EXP - 1,
        }
    }

    const fn to_mask(self) -> u64 {
        if self.to_bits() == u64::BITS { u64::MAX } else { (1 << self.to_bits()) - 1 }
    }

    const fn to_fraction_mask(self) -> u64 {
        (1 << (self.to_significand_bits() - 1)) - 1
    }

    const fn to_sign_bit(self) -> u64 {
        1 << (self.to_bits() - 1)
    }

    /// The bit that makes a NaN quiet: the fraction's highest.
    pub const fn to_quiet_bit(self) -> u64 {
        1 << (self.to_significand_bits() - 2)
    }

    const fn to_infinity(self) -> u64 {
        self.to_mask() & !self.to_sign_bit() & !self.to_fraction_mask()
    }

    /// The QNaN indefinite that an invalid operation writes: the sign set, the quiet bit alone in the fraction.
    pub const fn to_default_nan(self) -> u64 {
        self.to_sign_bit() | self.to_infinity() | self.to_quiet_bit()
    }

    const fn to_bias(self) -> i32 {
        (1 << (self.to_exponent_bits() - 1)) - 1
    }
}

/// MXCSR's rounding control.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Rounding {
    Nearest,
    Down,
    Up,
    TowardZero,
}

/// What an operand is, its value read as `significand × 2^exponent` when finite.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Zero,
    Finite { significand: u128, exponent: i32, denormal: bool },
    Infinity,
    Nan { signaling: bool },
}

#[derive(Clone, Copy, Debug)]
struct Operand {
    bits: u64,
    negative: bool,
    class: Class,
}

/// A lane's result, or the exceptions that stop the instruction before it is computed.
struct LaneOutcome {
    bits: u64,
    /// The flags of the exceptions detected before the computation.
    before: u32,
    /// The flags of those detected after it.
    after: u32,
}

impl Operand {
    fn read(bits: u64, precision: Precision, denormals_are_zeros: bool) -> Self {
        let bits = bits & precision.to_mask();
        let negative = bits & precision.to_sign_bit() != 0;
        let fraction = bits & precision.to_fraction_mask();
        let biased = ((bits & !precision.to_sign_bit()) >> (precision.to_significand_bits() - 1)) as i32;
        let fraction_bits = precision.to_significand_bits() as i32 - 1;
        let all_ones = (1 << precision.to_exponent_bits()) - 1;
        let class = match (biased, fraction) {
            (0, 0) => Class::Zero,
            (0, _) if denormals_are_zeros => Class::Zero,
            (0, _) => Class::Finite {
                significand: u128::from(fraction),
                exponent: 1 - precision.to_bias() - fraction_bits,
                denormal: true,
            },
            (biased, 0) if biased == all_ones => Class::Infinity,
            (biased, _) if biased == all_ones => Class::Nan {
                signaling: fraction & precision.to_quiet_bit() == 0,
            },
            (biased, _) => Class::Finite {
                significand: u128::from(fraction | (1 << fraction_bits)),
                exponent: biased - precision.to_bias() - fraction_bits,
                denormal: false,
            },
        };
        Self { bits, negative, class }
    }

    const fn is_denormal(&self) -> bool {
        matches!(self.class, Class::Finite { denormal: true, .. })
    }

    const fn is_nan(&self) -> bool {
        matches!(self.class, Class::Nan { .. })
    }
}

impl FloatOperation {
    /// Computes one lane from the destination's operand and the source's, under MXCSR.
    fn compute(self, first: u64, second: u64, precision: Precision, mxcsr: u32, immediate: u8) -> LaneOutcome {
        let single = precision == Precision::Single;
        let daz = mxcsr & DENORMALS_ARE_ZEROS != 0
            && !is_active(Bug::DenormalsAreZerosIgnored)
            && !(is_active(Bug::SquareRootDoubleIgnoresDenormalsAreZeros) && self == Self::SquareRoot && !single);
        let (first, second) = (Operand::read(first, precision, daz), Operand::read(second, precision, daz));
        let operands: &[Operand] = if self == Self::SquareRoot { &[second] } else { &[first, second] };
        let quiet = |operand: Operand| operand.bits | precision.to_quiet_bit();
        let signaling = operands.iter().any(|operand| matches!(operand.class, Class::Nan { signaling: true }));
        let unflagged = is_active(Bug::MaximumSingleNoDenormalFlag) && self == Self::Maximum && single;
        let denormal = if operands.iter().any(Operand::is_denormal) && !unflagged { DENORMAL } else { 0 };
        let done = |bits: u64, before: u32| LaneOutcome { bits, before, after: 0 };
        if matches!(self, Self::Minimum | Self::Maximum) {
            // The source as DAZ reads it: a denormal written as a zero of its sign.
            let as_read = |operand: Operand| {
                if operand.class == Class::Zero { operand.bits & precision.to_sign_bit() } else { operand.bits }
            };
            if first.is_nan() || second.is_nan() {
                if is_active(Bug::MinimumDoubleNanGivesDestination) && self == Self::Minimum && !single {
                    return done(as_read(first), INVALID);
                }
                if is_active(Bug::MaximumDoubleQuietNanNoInvalid) && self == Self::Maximum && !single && !signaling {
                    return done(as_read(second), 0);
                }
                return done(as_read(second), INVALID);
            }
            let (zero_first, zero_second) = (first.class == Class::Zero, second.class == Class::Zero);
            let zeros_give_first = (is_active(Bug::MinimumSingleZerosGiveDestination) && single)
                || is_active(Bug::MinimumOfZerosGivesDestination);
            let written = if zero_first && zero_second {
                if zeros_give_first && self == Self::Minimum { first } else { second }
            } else {
                let less = compare_less(first, second);
                match (self, less) {
                    (Self::Minimum, true) | (Self::Maximum, false) if !is_equal(first, second) => first,
                    _ => second,
                }
            };
            return done(as_read(written), denormal);
        }
        if let Self::Compare | Self::OrderedCompare { .. } = self {
            return compare(self, first, second, precision, immediate, signaling, denormal);
        }
        // A NaN operand: the first NaN, quieted.
        let last_nan = is_active(Bug::AddDoubleTwoNansGiveSource) && self == Self::Add && !single;
        let mut nans = operands.iter().filter(|operand| operand.is_nan());
        if let Some(nan) = if last_nan { nans.next_back() } else { nans.next() } {
            return done(quiet(*nan), if signaling { INVALID } else { 0 });
        }
        let default_nan = if is_active(Bug::DefaultNanPositive) {
            precision.to_default_nan() & !precision.to_sign_bit()
        } else {
            precision.to_default_nan()
        };
        let invalid = done(default_nan, INVALID);
        let sign_of = |negative: bool| if negative { precision.to_sign_bit() } else { 0 };
        let infinity = |negative: bool| sign_of(negative) | precision.to_infinity();
        let swapped = is_active(Bug::AddSingleRoundingSwapped) && self == Self::Add && single;
        let rounding = match read_rounding(mxcsr) {
            Rounding::Down if swapped => Rounding::Up,
            Rounding::Up if swapped => Rounding::Down,
            rounding => rounding,
        };
        let exact_zero_sign = |negative_first: bool, negative_second: bool| {
            if negative_first == negative_second { negative_first } else { rounding == Rounding::Down }
        };
        let (exact, before) = match self {
            Self::Add | Self::Subtract => {
                let second_negative = second.negative != (self == Self::Subtract);
                match (first.class, second.class) {
                    (Class::Infinity, Class::Infinity) if first.negative != second_negative => return invalid,
                    (Class::Infinity, _) => return done(infinity(first.negative), denormal),
                    (_, Class::Infinity) => return done(infinity(second_negative), denormal),
                    (Class::Zero, Class::Zero) => {
                        return done(sign_of(exact_zero_sign(first.negative, second_negative)), denormal);
                    }
                    (Class::Zero, Class::Finite { significand, exponent, .. }) => {
                        (Exact { negative: second_negative, significand, exponent, sticky: false }, denormal)
                    }
                    (Class::Finite { significand, exponent, .. }, Class::Zero) => {
                        (Exact { negative: first.negative, significand, exponent, sticky: false }, denormal)
                    }
                    (
                        Class::Finite { significand: a, exponent: ea, .. },
                        Class::Finite { significand: b, exponent: eb, .. },
                    ) => match add(first.negative, a, ea, second_negative, b, eb, precision) {
                        Some(exact) => (exact, denormal),
                        None if is_active(Bug::SubtractDoubleCancellationPositiveZero) && self == Self::Subtract && !single => {
                            return done(0, denormal);
                        }
                        None => return done(sign_of(rounding == Rounding::Down), denormal),
                    },
                    _ => unreachable!("NaNs are handled first"),
                }
            }
            Self::Multiply => {
                let negative = first.negative != second.negative;
                match (first.class, second.class) {
                    (Class::Infinity, Class::Zero) | (Class::Zero, Class::Infinity) => return invalid,
                    (Class::Infinity, _) | (_, Class::Infinity) => return done(infinity(negative), denormal),
                    (Class::Zero, _) | (_, Class::Zero) => return done(sign_of(negative), denormal),
                    (
                        Class::Finite { significand: a, exponent: ea, .. },
                        Class::Finite { significand: b, exponent: eb, .. },
                    ) => (Exact { negative, significand: a * b, exponent: ea + eb, sticky: false }, denormal),
                    _ => unreachable!("NaNs are handled first"),
                }
            }
            Self::Divide => {
                let negative = first.negative != second.negative;
                match (first.class, second.class) {
                    (Class::Zero, Class::Zero) if is_active(Bug::DivideDoubleZeroByZeroReportsDivideByZero) && !single => {
                        return done(default_nan, DIVIDE_BY_ZERO);
                    }
                    (Class::Infinity, Class::Infinity) | (Class::Zero, Class::Zero) => return invalid,
                    (Class::Infinity, _) => return done(infinity(negative), denormal),
                    (_, Class::Infinity) => return done(sign_of(negative), denormal),
                    // Division by zero is reported alone, a denormal dividend's flag with it not.
                    (Class::Finite { .. }, Class::Zero) if is_active(Bug::DivideSingleDenormalByZeroReportsDenormal) && single => {
                        return done(infinity(negative), DIVIDE_BY_ZERO | denormal);
                    }
                    (Class::Finite { .. }, Class::Zero) => return done(infinity(negative), DIVIDE_BY_ZERO),
                    (Class::Zero, _) => return done(sign_of(negative), denormal),
                    (
                        Class::Finite { significand: a, exponent: ea, .. },
                        Class::Finite { significand: b, exponent: eb, .. },
                    ) => {
                        let shift = u128::BITS - 1 - (u128::BITS - a.leading_zeros());
                        let numerator = a << shift;
                        let exact = Exact {
                            negative,
                            significand: numerator / b,
                            exponent: ea - eb - shift as i32,
                            sticky: numerator % b != 0,
                        };
                        (exact, denormal)
                    }
                    _ => unreachable!("NaNs are handled first"),
                }
            }
            Self::SquareRoot => match second.class {
                Class::Zero if is_active(Bug::SquareRootSingleNegativeZeroPositive) && single => return done(0, 0),
                Class::Zero => return done(second.bits & precision.to_sign_bit(), 0),
                _ if second.negative && is_active(Bug::SquareRootSingleNegativeNanPositive) && single => {
                    return done(precision.to_default_nan() & !precision.to_sign_bit(), INVALID);
                }
                _ if second.negative => return invalid,
                Class::Infinity => return done(second.bits, 0),
                Class::Finite { significand, exponent, .. } => {
                    // An even exponent, and the significand shifted left by an even count to as many bits as fit.
                    let (mut significand, mut exponent) = (significand, exponent);
                    if exponent % 2 != 0 {
                        significand <<= 1;
                        exponent -= 1;
                    }
                    let shift = (significand.leading_zeros() - 2) & !1;
                    let widened = significand << shift;
                    let root = square_root(widened);
                    let exact = Exact {
                        negative: false,
                        significand: root,
                        exponent: (exponent - shift as i32) / 2,
                        sticky: root * root != widened,
                    };
                    (exact, denormal)
                }
                Class::Nan { .. } => unreachable!("NaNs are handled first"),
            },
            Self::Minimum | Self::Maximum | Self::Compare | Self::OrderedCompare { .. } => unreachable!("handled above"),
            Self::AddSubtract | Self::HorizontalAdd | Self::HorizontalSubtract | Self::Convert(_) => {
                unreachable!("each lane is computed as an addition or a subtraction")
            }
        };
        let (bits, mut after) = round(exact, precision, rounding, mxcsr);
        if is_active(Bug::SquareRootDoubleNeverInexact) && self == Self::SquareRoot && !single {
            after &= !INEXACT;
        }
        LaneOutcome { bits, before, after }
    }
}

/// MXCSR's rounding control.
const fn read_rounding(mxcsr: u32) -> Rounding {
    match (mxcsr >> ROUNDING_SHIFT) & 3 {
        0 => Rounding::Nearest,
        1 => Rounding::Down,
        2 => Rounding::Up,
        _ => Rounding::TowardZero,
    }
}

/// One lane of a conversion, from the source lane's bits.
fn convert(conversion: Conversion, value: u64, precision: Precision, mxcsr: u32) -> LaneOutcome {
    let daz = mxcsr & DENORMALS_ARE_ZEROS != 0;
    let rounding = read_rounding(mxcsr);
    let done = |bits: u64, before: u32| LaneOutcome { bits, before, after: 0 };
    match conversion {
        Conversion::IntegerToFloat => {
            let integer = i64::from(value as u32 as i32);
            if integer == 0 {
                return done(0, 0);
            }
            let exact = Exact { negative: integer < 0, significand: u128::from(integer.unsigned_abs()), exponent: 0, sticky: false };
            let nearest = is_active(Bug::IntegerToSingleRoundsToNearest) && precision == Precision::Single;
            let (bits, after) = round(exact, precision, if nearest { Rounding::Nearest } else { rounding }, mxcsr);
            LaneOutcome { bits, before: 0, after }
        }
        Conversion::SingleToDouble | Conversion::DoubleToSingle => {
            let (from, to) = match conversion {
                Conversion::SingleToDouble => (Precision::Single, Precision::Double),
                _ => (Precision::Double, Precision::Single),
            };
            let operand = Operand::read(value, from, daz);
            let sign = if operand.negative { to.to_sign_bit() } else { 0 };
            match operand.class {
                Class::Zero => done(sign, 0),
                Class::Infinity => done(sign | to.to_infinity(), 0),
                Class::Nan { signaling } => {
                    let fraction = operand.bits & from.to_fraction_mask();
                    let moved = if from == Precision::Single {
                        fraction << (to.to_significand_bits() - from.to_significand_bits())
                    } else {
                        fraction >> (from.to_significand_bits() - to.to_significand_bits())
                    };
                    let kept = is_active(Bug::SingleToDoubleKeepsSignaling) && from == Precision::Single;
                    let quiet = if kept { 0 } else { to.to_quiet_bit() };
                    done(sign | to.to_infinity() | moved | quiet, if signaling { INVALID } else { 0 })
                }
                Class::Finite { significand, exponent, denormal } => {
                    let exact = Exact { negative: operand.negative, significand, exponent, sticky: false };
                    let nearest = is_active(Bug::DoubleToSingleRoundsToNearest) && from == Precision::Double;
                    let (bits, after) = round(exact, to, if nearest { Rounding::Nearest } else { rounding }, mxcsr);
                    let unflagged = is_active(Bug::SingleToDoubleDenormalUnflagged) && from == Precision::Single;
                    LaneOutcome { bits, before: if denormal && !unflagged { DENORMAL } else { 0 }, after }
                }
            }
        }
        Conversion::FloatToInteger { truncate } => {
            let operand = Operand::read(value, precision, daz);
            let Class::Finite { significand, exponent, .. } = operand.class else {
                return match operand.class {
                    Class::Zero => done(0, 0),
                    _ => done(INTEGER_INDEFINITE, INVALID),
                };
            };
            let mode = if (truncate && !is_active(Bug::TruncatingConversionRounds))
                || (is_active(Bug::ConversionToIntegerTruncates) && precision == Precision::Single)
            {
                Rounding::TowardZero
            } else {
                rounding
            };
            let (magnitude, inexact) = round_to_integer(significand, exponent, mode, operand.negative);
            let limit = if operand.negative { 1u128 << 31 } else { (1 << 31) - 1 };
            if magnitude > limit {
                return done(INTEGER_INDEFINITE, INVALID);
            }
            let integer = if operand.negative { (magnitude as i64).wrapping_neg() } else { magnitude as i64 };
            LaneOutcome { bits: u64::from(integer as u32), before: 0, after: if inexact { INEXACT } else { 0 } }
        }
    }
}

/// `significand × 2^exponent` rounded to an integer in the mode, its magnitude (saturated far past any integer
/// range) and whether bits were lost.
fn round_to_integer(significand: u128, exponent: i32, rounding: Rounding, negative: bool) -> (u128, bool) {
    if exponent >= 0 {
        return if exponent >= 64 { (u128::MAX, false) } else { (significand << exponent, false) };
    }
    let shift = -exponent;
    let (kept, rest, half) = if shift >= 127 {
        (0, significand, u128::MAX)
    } else {
        (significand >> shift, significand & ((1 << shift) - 1), 1 << (shift - 1))
    };
    let up = match rounding {
        Rounding::Nearest => rest > half || (rest == half && kept & 1 == 1),
        Rounding::TowardZero => false,
        Rounding::Up => rest != 0 && !negative,
        Rounding::Down => rest != 0 && negative,
    };
    (kept + u128::from(up), rest != 0)
}

/// A compare's lane: CMPcc's mask or the ordered compare's ZF, PF and CF, and invalid for a signaling NaN, or for
/// any NaN under a signaling predicate, else denormal for a denormal operand.
fn compare(
    operation: FloatOperation,
    first: Operand,
    second: Operand,
    precision: Precision,
    immediate: u8,
    signaling: bool,
    denormal: u32,
) -> LaneOutcome {
    const SIGNALING_PREDICATES: [u8; 4] = [1, 2, 5, 6];
    let unordered = first.is_nan() || second.is_nan();
    let (less, equal) = (!unordered && compare_less(first, second), !unordered && is_equal(first, second));
    let predicate = immediate & if is_active(Bug::ComparePredicateFourBits) { 0xF } else { 7 };
    let signals = match operation {
        FloatOperation::OrderedCompare { signaling: true } => true,
        FloatOperation::Compare if is_active(Bug::CompareLessQuiet) && predicate == 1 => false,
        FloatOperation::Compare => SIGNALING_PREDICATES.contains(&predicate),
        _ => is_active(Bug::UnorderedCompareSignalsQuiet),
    };
    let unflagged = is_active(Bug::UnorderedCompareDoubleSignalingNoInvalid)
        && operation == FloatOperation::OrderedCompare { signaling: false }
        && precision == Precision::Double;
    let before = if unordered {
        if (signaling && !unflagged) || signals { INVALID } else { 0 }
    } else {
        denormal
    };
    let bits = match operation {
        FloatOperation::OrderedCompare { .. } => match (unordered, less, equal) {
            (true, _, _) if is_active(Bug::OrderedCompareUnorderedWithoutParity) => ORDER_FLAGS & !(1 << 2),
            (true, _, _) if is_active(Bug::OrderedCompareDoubleNanClearsZero) && precision == Precision::Double => ORDER_FLAGS & !(1 << 6),
            (true, _, _) => ORDER_FLAGS,
            (_, true, _) => 1,
            (_, _, true) => 1 << 6,
            _ => 0,
        },
        _ => {
            let holds = match predicate {
                0 => equal,
                1 => less,
                2 if is_active(Bug::CompareLessOrEqualExcludesEqual) && precision == Precision::Double => less,
                2 => less || equal,
                3 => unordered,
                4 => !equal,
                5 => !less,
                6 => !(less || equal),
                _ => !unordered,
            };
            if holds { precision.to_mask() } else { 0 }
        }
    };
    LaneOutcome { bits, before, after: 0 }
}

/// A finite nonzero value, `significand × 2^exponent`, with `sticky` set where nonzero bits lie below it; when set,
/// the significand holds at least three bits more than the precision.
#[derive(Clone, Copy, Debug)]
struct Exact {
    negative: bool,
    significand: u128,
    exponent: i32,
    sticky: bool,
}

/// The exact sum of two finite nonzero values, or `None` when it is an exact zero.
fn add(
    first_negative: bool,
    first: u128,
    first_exponent: i32,
    second_negative: bool,
    second: u128,
    second_exponent: i32,
    precision: Precision,
) -> Option<Exact> {
    // Both at an exponent far enough below the larger operand's that the smaller one's lost bits are only sticky.
    let guard = precision.to_significand_bits() as i32 + 4;
    let common = first_exponent.max(second_exponent) - guard;
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
        return Some(Exact {
            negative: first_negative,
            significand: a + b,
            exponent: common,
            sticky: a_sticky || b_sticky,
        });
    }
    // The larger magnitude less the smaller; a smaller operand with lost bits is a little larger than its part.
    let (large, small, small_sticky, negative) = match a.cmp(&b) {
        core::cmp::Ordering::Equal if !a_sticky && !b_sticky => return None,
        core::cmp::Ordering::Less => (b, a, a_sticky, second_negative),
        core::cmp::Ordering::Equal if a_sticky => (b, a, a_sticky, second_negative),
        _ => (a, b, b_sticky, first_negative),
    };
    let difference = large - small - u128::from(small_sticky);
    Some(Exact {
        negative,
        significand: difference,
        exponent: common,
        sticky: small_sticky,
    })
}

/// Whether shifting the significand right by `shift` loses a set bit.
fn significand_lost(shift: i32, significand: u128) -> bool {
    match shift {
        ..=0 => false,
        shift if shift >= u128::BITS as i32 => significand != 0,
        shift => significand & ((1 << shift) - 1) != 0,
    }
}

/// The integer square root, rounded down.
fn square_root(value: u128) -> u128 {
    let mut root = 0u128;
    let mut bit = 1u128 << (u128::BITS - 2);
    let mut remainder = value;
    while bit > value {
        bit >>= 2;
    }
    while bit != 0 {
        if remainder >= root + bit {
            remainder -= root + bit;
            root = (root >> 1) + bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    root
}

/// Whether `first` is less than `second`, both numbers or infinities.
fn compare_less(first: Operand, second: Operand) -> bool {
    let key = |operand: Operand| -> (i8, i32, u128) {
        match operand.class {
            Class::Zero => (0, i32::MIN, 0),
            Class::Finite { significand, exponent, .. } => {
                let width = (u128::BITS - significand.leading_zeros()) as i32;
                (1, exponent + width, significand << (u128::BITS as i32 - width))
            }
            _ => (2, 0, 0),
        }
    };
    let (first_key, second_key) = (key(first), key(second));
    match (first.negative && first.class != Class::Zero, second.negative && second.class != Class::Zero) {
        (true, false) => true,
        (false, true) => false,
        (false, false) => first_key < second_key,
        (true, true) => first_key > second_key,
    }
}

fn is_equal(first: Operand, second: Operand) -> bool {
    !compare_less(first, second) && !compare_less(second, first)
}

/// Rounds the exact value in the mode to the precision, and gives its bits and the flags after the computation:
/// overflow, underflow and inexact, as their masks in MXCSR ask.
fn round(exact: Exact, precision: Precision, rounding: Rounding, mxcsr: u32) -> (u64, u32) {
    let double = precision == Precision::Double;
    let ties_away = is_active(Bug::NearestTiesAwayFromZero) || (is_active(Bug::DoubleTiesAwayFromZero) && double);
    let digits = precision.to_significand_bits() as i32;
    let minimum = precision.to_minimum_exponent();
    let Exact { negative, significand, exponent, sticky } = exact;
    let sign = if negative { precision.to_sign_bit() } else { 0 };
    let top = exponent + (u128::BITS - significand.leading_zeros()) as i32 - 1;
    // Rounds to the quantum 2^quantum, giving the multiple of it and whether bits were lost.
    let to_quantum = |quantum: i32| -> (u128, bool) {
        let shift = quantum - exponent;
        // The bits below the quantum, against half of it.
        let (kept, lost) = if shift <= 0 {
            (significand << -shift, core::cmp::Ordering::Less)
        } else if shift > u128::BITS as i32 {
            (0, core::cmp::Ordering::Less)
        } else {
            let rest = if shift == u128::BITS as i32 { significand } else { significand & ((1 << shift) - 1) };
            let half = 1 << (shift - 1);
            let kept = if shift == u128::BITS as i32 { 0 } else { significand >> shift };
            (kept, rest.cmp(&half).then(if sticky { core::cmp::Ordering::Greater } else { core::cmp::Ordering::Equal }))
        };
        let below_quantum = significand_lost(shift, significand) || sticky;
        let up = match rounding {
            Rounding::Nearest if ties_away => lost != core::cmp::Ordering::Less,
            Rounding::Nearest => lost == core::cmp::Ordering::Greater || (lost == core::cmp::Ordering::Equal && kept & 1 == 1),
            Rounding::TowardZero => false,
            Rounding::Up => below_quantum && !negative,
            Rounding::Down => below_quantum && negative,
        };
        (kept + u128::from(up), below_quantum)
    };
    // Tininess after rounding: the value rounded to the precision with no bound on the exponent lies below 2^minimum.
    let (unbounded, inexact_unbounded) = to_quantum(top - (digits - 1));
    // An unmasked overflow or underflow reports inexact where the result with no bound on the exponent is inexact.
    let unbounded_inexact = if inexact_unbounded { INEXACT } else { 0 };
    let rounded_top = top + i32::from(unbounded >> digits != 0);
    let tiny = if is_active(Bug::DoubleTininessBeforeRounding) && double { top < minimum } else { rounded_top < minimum };
    let quantum = (top - (digits - 1)).max(minimum - (digits - 1));
    let (mut multiple, inexact) = to_quantum(quantum);
    let mut quantum = quantum;
    if multiple >> digits != 0 {
        multiple >>= 1;
        quantum += 1;
    }
    let masked = |flag: u32| mxcsr & (flag << MASK_SHIFT) != 0;
    if multiple != 0 && quantum + digits - 1 > precision.to_maximum_exponent() {
        let largest = (precision.to_infinity() - 1) & precision.to_mask();
        let toward_infinity = match rounding {
            Rounding::Nearest => true,
            Rounding::TowardZero => is_active(Bug::DoubleOverflowTowardZeroInfinity) && double,
            Rounding::Up => !negative,
            Rounding::Down => negative,
        };
        let magnitude = if toward_infinity { precision.to_infinity() } else { largest };
        let unmasked_inexact = if is_active(Bug::SingleUnmaskedOverflowAlwaysInexact) && !double {
            INEXACT
        } else if is_active(Bug::UnmaskedOverflowNeverInexact) {
            0
        } else {
            unbounded_inexact
        };
        let flags = if masked(OVERFLOW) { OVERFLOW | INEXACT } else { OVERFLOW | unmasked_inexact };
        return (sign | magnitude, flags);
    }
    let mut flags = if inexact { INEXACT } else { 0 };
    if tiny {
        if !masked(UNDERFLOW) {
            flags = UNDERFLOW | unbounded_inexact;
        } else if mxcsr & FLUSH_TO_ZERO != 0 && !is_active(Bug::FlushToZeroIgnored) {
            return (sign, UNDERFLOW | INEXACT);
        } else if inexact || (is_active(Bug::MaskedUnderflowOnExactTiny) && double) {
            flags |= UNDERFLOW;
        }
    }
    let bits = if multiple >> (digits - 1) == 0 {
        // A subnormal, or zero.
        multiple as u64
    } else {
        let biased = (quantum + digits - 1 + precision.to_bias()) as u64;
        (biased << (digits - 1)) | (multiple as u64 & precision.to_fraction_mask())
    };
    (sign | bits, flags)
}

impl FloatForm {
    /// Applies the form to the destination's 128 bits and the source's, under MXCSR, with the form's immediate (0
    /// for a form without one): the destination's new bits (an ordered compare's ZF, PF and CF in the low lane) and
    /// the flags to add, or the flags to add and #XM.
    pub fn apply(&self, destination: u128, source: u128, mxcsr: u32, immediate: u8) -> Result<(u128, u32), u32> {
        if let FloatOperation::Convert(conversion) = self.operation {
            return self.apply_conversion(conversion, destination, source, mxcsr);
        }
        let precision = self.precision;
        let bits = precision.to_bits();
        let lanes = match self.arrangement {
            Arrangement::Packed => u128::BITS / bits,
            Arrangement::Scalar => 1,
        };
        let mask = u128::from(precision.to_mask());
        let read = |value: u128, lane: u32| (value >> (lane * bits)) as u64;
        let outcomes: alloc::vec::Vec<LaneOutcome> = (0..lanes)
            .map(|lane| {
                let (first, second, operation) = match self.operation {
                    FloatOperation::AddSubtract if (lane % 2 == 0) != is_active(Bug::AddSubtractReversed) => {
                        (read(destination, lane), read(source, lane), Subtract)
                    }
                    FloatOperation::AddSubtract => (read(destination, lane), read(source, lane), Add),
                    FloatOperation::HorizontalAdd | FloatOperation::HorizontalSubtract => {
                        let half = lanes / 2;
                        let low = (lane < half) != is_active(Bug::HorizontalAddSourceFirst);
                        let pair = lane % half;
                        let value = if low { destination } else { source };
                        let operation = if self.operation == FloatOperation::HorizontalAdd { Add } else { Subtract };
                        let reversed = is_active(Bug::HorizontalSubtractDoubleReversed)
                            && operation == Subtract
                            && precision == Precision::Double;
                        if reversed {
                            (read(value, 2 * pair + 1), read(value, 2 * pair), operation)
                        } else {
                            (read(value, 2 * pair), read(value, 2 * pair + 1), operation)
                        }
                    }
                    operation => (read(destination, lane), read(source, lane), operation),
                };
                operation.compute(first, second, precision, mxcsr, immediate)
            })
            .collect();
        let unmasked = |flags: u32| flags & !(mxcsr >> MASK_SHIFT) & FLAGS != 0;
        let before = outcomes.iter().fold(0, |all, outcome| all | outcome.before);
        if unmasked(before) {
            return Err(before);
        }
        let after = outcomes.iter().fold(0, |all, outcome| all | outcome.after);
        if unmasked(after) {
            return Err(before | after);
        }
        let result = outcomes.iter().enumerate().fold(destination, |all, (lane, outcome)| {
            let shift = lane as u32 * bits;
            (all & !(mask << shift)) | (u128::from(outcome.bits) << shift)
        });
        Ok((result, before | after))
    }
}

impl FloatForm {
    /// A conversion: each source lane converted into the destination's lane of the same index; a scalar form
    /// keeps the destination's bits above its lane, a packed one zeroes those above its lanes.
    fn apply_conversion(&self, conversion: Conversion, destination: u128, source: u128, mxcsr: u32) -> Result<(u128, u32), u32> {
        let (from, to) = conversion.to_widths(self.precision);
        let lanes = match self.arrangement {
            Arrangement::Scalar => 1,
            Arrangement::Packed => u128::BITS / from.max(to),
        };
        let lane_mask = |bits: u32| if bits == u128::BITS { u128::MAX } else { (1u128 << bits) - 1 };
        let outcomes: alloc::vec::Vec<LaneOutcome> = (0..lanes)
            .map(|lane| convert(conversion, ((source >> (lane * from)) & lane_mask(from)) as u64, self.precision, mxcsr))
            .collect();
        let unmasked = |flags: u32| flags & !(mxcsr >> MASK_SHIFT) & FLAGS != 0;
        let before = outcomes.iter().fold(0, |all, outcome| all | outcome.before);
        if unmasked(before) {
            return Err(before);
        }
        let after = outcomes.iter().fold(0, |all, outcome| all | outcome.after);
        if unmasked(after) {
            return Err(before | after);
        }
        let kept = match self.arrangement {
            Arrangement::Scalar => destination & !lane_mask(to),
            Arrangement::Packed if is_active(Bug::PackedConversionKeepsUpperHalf) => destination & !lane_mask(lanes * to),
            Arrangement::Packed => 0,
        };
        let result = outcomes.iter().enumerate().fold(kept, |all, (lane, outcome)| all | (u128::from(outcome.bits) << (lane as u32 * to)));
        Ok((result, before | after))
    }

    /// The bytes a memory source reaches: 16 for a packed form but one that widens two lanes from 8, one lane's for
    /// a scalar one.
    pub const fn memory_bytes(&self) -> u64 {
        match (self.arrangement, self.operation) {
            (Arrangement::Packed, FloatOperation::Convert(conversion)) => {
                let (from, to) = conversion.to_widths(self.precision);
                if from < to { 8 } else { 16 }
            }
            (Arrangement::Packed, _) => 16,
            (Arrangement::Scalar, FloatOperation::Convert(conversion)) => (conversion.to_widths(self.precision).0 / u8::BITS) as u64,
            (Arrangement::Scalar, _) => (self.precision.to_bits() / u8::BITS) as u64,
        }
    }
}

/// The row of a legacy SSE floating-point form, or `None` for any other form.
pub fn find_float_form(instruction: Instruction) -> Option<&'static FloatForm> {
    FLOAT_FORMS.iter().find(|form| form.instruction == instruction)
}

const fn form(
    instruction: Instruction,
    operation: FloatOperation,
    precision: Precision,
    arrangement: Arrangement,
    place: OperandPlace,
    code: Code,
) -> FloatForm {
    FloatForm {
        instruction,
        operation,
        precision,
        arrangement,
        place,
        code,
    }
}

use Arrangement::{Packed, Scalar};
use FloatOperation::{
    Add, AddSubtract, Compare, Convert, Divide, HorizontalAdd, HorizontalSubtract, Maximum, Minimum, Multiply,
    OrderedCompare, SquareRoot, Subtract,
};
use Conversion::{DoubleToSingle, FloatToInteger, IntegerToFloat, SingleToDouble};
use OperandPlace::{Memory, Register};
use Precision::{Double, Single};

/// The register and the memory form of each operation in each precision and arrangement.
macro_rules! forms {
    ($($register:ident, $memory:ident, $operation:expr, $precision:ident, $arrangement:ident, $code:ident;)*) => {
        &[$(
            form(Instruction::$register, $operation, $precision, $arrangement, Register, Code::$code),
            form(Instruction::$memory, $operation, $precision, $arrangement, Memory, Code::$code),
        )*]
    };
}

/// Every legacy SSE floating-point form modeled so far.
pub static FLOAT_FORMS: &[FloatForm] = forms! {
    AddpsXmmpsXmmps, AddpsXmmpsMemps, Add, Single, Packed, Addps_xmm_xmmm128;
    AddssXmmssXmmss, AddssXmmssMemss, Add, Single, Scalar, Addss_xmm_xmmm32;
    AddpdXmmpdXmmpd, AddpdXmmpdMempd, Add, Double, Packed, Addpd_xmm_xmmm128;
    AddsdXmmsdXmmsd, AddsdXmmsdMemsd, Add, Double, Scalar, Addsd_xmm_xmmm64;
    SubpsXmmpsXmmps, SubpsXmmpsMemps, Subtract, Single, Packed, Subps_xmm_xmmm128;
    SubssXmmssXmmss, SubssXmmssMemss, Subtract, Single, Scalar, Subss_xmm_xmmm32;
    SubpdXmmpdXmmpd, SubpdXmmpdMempd, Subtract, Double, Packed, Subpd_xmm_xmmm128;
    SubsdXmmsdXmmsd, SubsdXmmsdMemsd, Subtract, Double, Scalar, Subsd_xmm_xmmm64;
    MulpsXmmpsXmmps, MulpsXmmpsMemps, Multiply, Single, Packed, Mulps_xmm_xmmm128;
    MulssXmmssXmmss, MulssXmmssMemss, Multiply, Single, Scalar, Mulss_xmm_xmmm32;
    MulpdXmmpdXmmpd, MulpdXmmpdMempd, Multiply, Double, Packed, Mulpd_xmm_xmmm128;
    MulsdXmmsdXmmsd, MulsdXmmsdMemsd, Multiply, Double, Scalar, Mulsd_xmm_xmmm64;
    DivpsXmmpsXmmps, DivpsXmmpsMemps, Divide, Single, Packed, Divps_xmm_xmmm128;
    DivssXmmssXmmss, DivssXmmssMemss, Divide, Single, Scalar, Divss_xmm_xmmm32;
    DivpdXmmpdXmmpd, DivpdXmmpdMempd, Divide, Double, Packed, Divpd_xmm_xmmm128;
    DivsdXmmsdXmmsd, DivsdXmmsdMemsd, Divide, Double, Scalar, Divsd_xmm_xmmm64;
    MinpsXmmpsXmmps, MinpsXmmpsMemps, Minimum, Single, Packed, Minps_xmm_xmmm128;
    MinssXmmssXmmss, MinssXmmssMemss, Minimum, Single, Scalar, Minss_xmm_xmmm32;
    MinpdXmmpdXmmpd, MinpdXmmpdMempd, Minimum, Double, Packed, Minpd_xmm_xmmm128;
    MinsdXmmsdXmmsd, MinsdXmmsdMemsd, Minimum, Double, Scalar, Minsd_xmm_xmmm64;
    MaxpsXmmpsXmmps, MaxpsXmmpsMemps, Maximum, Single, Packed, Maxps_xmm_xmmm128;
    MaxssXmmssXmmss, MaxssXmmssMemss, Maximum, Single, Scalar, Maxss_xmm_xmmm32;
    MaxpdXmmpdXmmpd, MaxpdXmmpdMempd, Maximum, Double, Packed, Maxpd_xmm_xmmm128;
    MaxsdXmmsdXmmsd, MaxsdXmmsdMemsd, Maximum, Double, Scalar, Maxsd_xmm_xmmm64;
    SqrtpsXmmpsXmmps, SqrtpsXmmpsMemps, SquareRoot, Single, Packed, Sqrtps_xmm_xmmm128;
    SqrtssXmmssXmmss, SqrtssXmmssMemss, SquareRoot, Single, Scalar, Sqrtss_xmm_xmmm32;
    SqrtpdXmmpdXmmpd, SqrtpdXmmpdMempd, SquareRoot, Double, Packed, Sqrtpd_xmm_xmmm128;
    SqrtsdXmmsdXmmsd, SqrtsdXmmsdMemsd, SquareRoot, Double, Scalar, Sqrtsd_xmm_xmmm64;
    CmppsXmmpsXmmpsImmb, CmppsXmmpsMempsImmb, Compare, Single, Packed, Cmpps_xmm_xmmm128_imm8;
    CmpssXmmssXmmssImmb, CmpssXmmssMemssImmb, Compare, Single, Scalar, Cmpss_xmm_xmmm32_imm8;
    CmppdXmmpdXmmpdImmb, CmppdXmmpdMempdImmb, Compare, Double, Packed, Cmppd_xmm_xmmm128_imm8;
    CmpsdXmmXmmsdXmmsdImmb, CmpsdXmmXmmsdMemsdImmb, Compare, Double, Scalar, Cmpsd_xmm_xmmm64_imm8;
    ComissXmmssXmmss, ComissXmmssMemss, OrderedCompare { signaling: true }, Single, Scalar, Comiss_xmm_xmmm32;
    UcomissXmmssXmmss, UcomissXmmssMemss, OrderedCompare { signaling: false }, Single, Scalar, Ucomiss_xmm_xmmm32;
    ComisdXmmsdXmmsd, ComisdXmmsdMemsd, OrderedCompare { signaling: true }, Double, Scalar, Comisd_xmm_xmmm64;
    UcomisdXmmsdXmmsd, UcomisdXmmsdMemsd, OrderedCompare { signaling: false }, Double, Scalar, Ucomisd_xmm_xmmm64;
    AddsubpsXmmpsXmmps, AddsubpsXmmpsMemps, AddSubtract, Single, Packed, Addsubps_xmm_xmmm128;
    AddsubpdXmmpdXmmpd, AddsubpdXmmpdMempd, AddSubtract, Double, Packed, Addsubpd_xmm_xmmm128;
    HaddpsXmmpsXmmps, HaddpsXmmpsMemps, HorizontalAdd, Single, Packed, Haddps_xmm_xmmm128;
    HaddpdXmmpdXmmpd, HaddpdXmmpdMempd, HorizontalAdd, Double, Packed, Haddpd_xmm_xmmm128;
    HsubpsXmmpsXmmps, HsubpsXmmpsMemps, HorizontalSubtract, Single, Packed, Hsubps_xmm_xmmm128;
    HsubpdXmmpdXmmpd, HsubpdXmmpdMempd, HorizontalSubtract, Double, Packed, Hsubpd_xmm_xmmm128;
    Cvtss2sdXmmsdXmmss, Cvtss2sdXmmsdMemss, Convert(SingleToDouble), Single, Scalar, Cvtss2sd_xmm_xmmm32;
    Cvtps2pdXmmpdXmmq, Cvtps2pdXmmpdMemq, Convert(SingleToDouble), Single, Packed, Cvtps2pd_xmm_xmmm64;
    Cvtsd2ssXmmssXmmsd, Cvtsd2ssXmmssMemsd, Convert(DoubleToSingle), Double, Scalar, Cvtsd2ss_xmm_xmmm64;
    Cvtpd2psXmmpsXmmpd, Cvtpd2psXmmpsMempd, Convert(DoubleToSingle), Double, Packed, Cvtpd2ps_xmm_xmmm128;
    Cvtdq2psXmmpsXmmdq, Cvtdq2psXmmpsMemdq, Convert(IntegerToFloat), Single, Packed, Cvtdq2ps_xmm_xmmm128;
    Cvtdq2pdXmmpdXmmq, Cvtdq2pdXmmpdMemq, Convert(IntegerToFloat), Double, Packed, Cvtdq2pd_xmm_xmmm64;
    Cvtps2dqXmmdqXmmps, Cvtps2dqXmmdqMemps, Convert(FloatToInteger { truncate: false }), Single, Packed, Cvtps2dq_xmm_xmmm128;
    Cvttps2dqXmmdqXmmps, Cvttps2dqXmmdqMemps, Convert(FloatToInteger { truncate: true }), Single, Packed, Cvttps2dq_xmm_xmmm128;
    Cvtpd2dqXmmdqXmmpd, Cvtpd2dqXmmdqMempd, Convert(FloatToInteger { truncate: false }), Double, Packed, Cvtpd2dq_xmm_xmmm128;
    Cvttpd2dqXmmdqXmmpd, Cvttpd2dqXmmdqMempd, Convert(FloatToInteger { truncate: true }), Double, Packed, Cvttpd2dq_xmm_xmmm128;
};
