//! The scalar floating-point instructions: one skeleton reads the registers the list gives each instruction, and one
//! small operation per family computes its result, its exception flags and its condition flags.
//!
//! The arithmetic transcribes Arm's floating-point pseudocode ([FPUnpack](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.FPUnpack.3),
//! [FPRound](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.FPRoundBase.5),
//! [FPProcessNaNs](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.FPProcessNaNs.4) and the
//! operations built on them) with FPCR.AH clear, as a chip without FEAT_AFP has it. A real value is held exactly, as
//! a sign, an integer and a power of two, so that each operation rounds once.

use core::convert::Infallible;

use crate::condition_flags::{CONDITIONS, ConditionFlags};
use crate::integer::low_ones;
use crate::operand::{Operand, OperandAccess, RegisterThirtyOne, RegisterWidth};
use crate::registers::{read_register, write_register};
use crate::{Exception, Instruction, Processor};

pub mod lanes;

/// [FPCR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPCR--Floating-point-Control-Register)'s fields this module reads, as single bits or the rounding mode's lowest bit.
pub const ALTERNATIVE_HALF: u64 = 1 << 26;
pub const DEFAULT_NAN: u64 = 1 << 25;
pub const FLUSH_TO_ZERO: u64 = 1 << 24;
pub const ROUNDING_MODE_SHIFT: u32 = 22;
pub const ROUNDING_MODE_BITS: u32 = 2;
pub const FLUSH_HALF_TO_ZERO: u64 = 1 << 19;

/// [FPSR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPSR--Floating-point-Status-Register)'s cumulative exception flags.
pub const INVALID_OPERATION: u64 = 1 << 0;
pub const DIVIDE_BY_ZERO: u64 = 1 << 1;
pub const OVERFLOW: u64 = 1 << 2;
pub const UNDERFLOW: u64 = 1 << 3;
pub const INEXACT: u64 = 1 << 4;
pub const INPUT_DENORMAL: u64 = 1 << 7;

/// The bits of the fraction of a half-precision value, which Rust's stable types do not name.
const HALF_FRACTION_BITS: u32 = 10;
/// The bits a shift of a value from a fixed-point register leaves for its fraction: FBITS is 64 less the scale.
const FIXED_POINT_SCALE_LIMIT: u32 = u64::BITS;
/// The field of the scale of a fixed-point conversion.
const SCALE: crate::encoding::Field = crate::encoding::Field::new(10, 6);
/// How far a division shifts its normalized dividend up, which leaves a quotient of 63 or 64 bits: more than any
/// format keeps, so that one rounding of it with its sticky remainder is exact.
const QUOTIENT_EXTRA_BITS: u32 = u64::BITS - 1;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Precision {
    Half,
    Single,
    Double,
}

impl Precision {
    pub const fn to_bits(self) -> u32 {
        match self {
            Self::Half => u16::BITS,
            Self::Single => u32::BITS,
            Self::Double => u64::BITS,
        }
    }

    pub const fn to_fraction_bits(self) -> u32 {
        match self {
            Self::Half => HALF_FRACTION_BITS,
            Self::Single => f32::MANTISSA_DIGITS - 1,
            Self::Double => f64::MANTISSA_DIGITS - 1,
        }
    }

    pub const fn to_exponent_bits(self) -> u32 {
        self.to_bits() - 1 - self.to_fraction_bits()
    }

    const fn to_bias(self) -> i32 {
        (1 << (self.to_exponent_bits() - 1)) - 1
    }

    /// The exponent of the smallest normal value.
    const fn to_minimum_exponent(self) -> i32 {
        1 - self.to_bias()
    }

    const fn to_sign_bit(self) -> u64 {
        1 << (self.to_bits() - 1)
    }

    const fn to_exponent_ones(self) -> u64 {
        low_ones(self.to_exponent_bits())
    }

    /// The fraction's most significant bit, which marks a NaN quiet.
    const fn to_quiet_bit(self) -> u64 {
        1 << (self.to_fraction_bits() - 1)
    }

    pub const fn to_infinity(self, sign: bool) -> u64 {
        (if sign { self.to_sign_bit() } else { 0 })
            | (self.to_exponent_ones() << self.to_fraction_bits())
    }

    pub const fn to_default_nan(self) -> u64 {
        self.to_infinity(false) | self.to_quiet_bit()
    }

    const fn to_zero(self, sign: bool) -> u64 {
        if sign { self.to_sign_bit() } else { 0 }
    }

    /// The largest finite value, positive or negative.
    const fn to_maximum_normal(self, sign: bool) -> u64 {
        (self.to_infinity(sign) - (1 << self.to_fraction_bits()))
            | low_ones(self.to_fraction_bits())
    }
}

/// The rounding modes of the pseudocode, FPCR.RMode's four in their encoding's order and then ties away.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rounding {
    TiesToEven,
    TowardPlusInfinity,
    TowardMinusInfinity,
    TowardZero,
    TiesAway,
}

const RMODE_ROUNDINGS: [Rounding; 4] = [
    Rounding::TiesToEven,
    Rounding::TowardPlusInfinity,
    Rounding::TowardMinusInfinity,
    Rounding::TowardZero,
];

/// The rounding FPCR.RMode selects.
const fn to_rounding(fpcr: u64) -> Rounding {
    RMODE_ROUNDINGS[((fpcr >> ROUNDING_MODE_SHIFT) & low_ones(ROUNDING_MODE_BITS)) as usize]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Zero,
    Infinity,
    Number,
    QuietNaN,
    SignalingNaN,
}

/// A value as FPUnpack gives it: `mantissa` times two to `exponent` for a number.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Unpacked {
    kind: Kind,
    sign: bool,
    mantissa: u64,
    exponent: i32,
    bits: u64,
}

impl Unpacked {
    const fn is_nan(self) -> bool {
        matches!(self.kind, Kind::QuietNaN | Kind::SignalingNaN)
    }
}

/// An exact real value: `sign` and `significand` times two to `exponent`, with `sticky` set where the true value lies
/// strictly between this one and the next significand up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Real {
    sign: bool,
    significand: u128,
    exponent: i32,
    sticky: bool,
}

/// What an operation needs of FPCR, and the exception flags it raises.
struct Context {
    fpcr: u64,
    flags: u64,
    /// A conversion between formats, which reads FPCR.AHP and leaves out FPCR.FZ16.
    conversion: bool,
}

impl Context {
    fn raise(&mut self, flag: u64) {
        self.flags |= flag;
    }

    /// Whether denormal values of the precision flush to zero.
    fn flushes(&self, precision: Precision) -> bool {
        match precision {
            Precision::Half if crate::planted::is(489) => {
                !self.conversion && self.fpcr & (FLUSH_HALF_TO_ZERO | FLUSH_TO_ZERO) != 0
            }
            Precision::Half => {
                (!self.conversion || crate::planted::is(73)) && self.fpcr & FLUSH_HALF_TO_ZERO != 0
            }
            Precision::Single | Precision::Double => self.fpcr & FLUSH_TO_ZERO != 0,
        }
    }

    fn alternative_half(&self, precision: Precision) -> bool {
        !crate::planted::is(488)
            && matches!(precision, Precision::Half)
            && self.conversion
            && self.fpcr & ALTERNATIVE_HALF != 0
    }

    const fn default_nan(&self) -> bool {
        self.fpcr & DEFAULT_NAN != 0
    }
}

/// [FPUnpack](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPUnpack): the kind, sign and real value of the bits, flushing a subnormal to zero where FPCR asks.
fn unpack(bits: u64, precision: Precision, context: &mut Context) -> Unpacked {
    let fraction_bits = precision.to_fraction_bits();
    let sign = bits & precision.to_sign_bit() != 0;
    let exponent = (bits >> fraction_bits) & precision.to_exponent_ones();
    let fraction = bits & low_ones(fraction_bits);
    let number = |mantissa: u64, exponent: i32| Unpacked {
        kind: Kind::Number,
        sign,
        mantissa,
        exponent,
        bits,
    };
    let other = |kind| Unpacked {
        kind,
        sign,
        mantissa: 0,
        exponent: 0,
        bits,
    };
    if exponent == 0 {
        if fraction == 0 {
            return other(Kind::Zero);
        }
        if context.flushes(precision)
            && !crate::planted::is(318)
            && !crate::planted::is(347)
            && !(crate::planted::is(83) && precision == Precision::Half)
        {
            // A half-precision input flushed under FZ16 raises no Input Denormal: what a Neoverse V2 shows on
            // every generated state (the manual page could not be read to confirm it).
            if precision != Precision::Half {
                context.raise(INPUT_DENORMAL);
            }
            return other(Kind::Zero);
        }
        return number(
            fraction,
            precision.to_minimum_exponent() - fraction_bits as i32,
        );
    }
    let biased = exponent as i32;
    if exponent == precision.to_exponent_ones() && !context.alternative_half(precision) {
        return other(match (fraction, fraction & precision.to_quiet_bit()) {
            (0, _) => Kind::Infinity,
            (_, 0) => Kind::SignalingNaN,
            _ => Kind::QuietNaN,
        });
    }
    number(
        fraction | (1 << fraction_bits),
        biased - precision.to_bias() - fraction_bits as i32,
    )
}

/// [FPProcessNaN](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPProcessNaN): the NaN made quiet, raising Invalid Operation for a signaling one, or the default NaN.
fn process_nan(operand: Unpacked, precision: Precision, context: &mut Context) -> u64 {
    let mut bits = operand.bits;
    if operand.kind == Kind::SignalingNaN {
        context.raise(INVALID_OPERATION);
        if !crate::planted::any(&[485, 315]) {
            bits |= precision.to_quiet_bit();
        }
    }
    if context.default_nan()
        || crate::planted::is(314)
        || (crate::planted::is(484) && operand.kind == Kind::QuietNaN)
    {
        return precision.to_default_nan();
    }
    bits
}

/// [FPProcessNaNs](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPProcessNaNs) and [FPProcessNaNs3](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPProcessNaNs3): the first signaling NaN, else the first quiet one, of the operands in order.
fn process_nans(operands: &[Unpacked], precision: Precision, context: &mut Context) -> Option<u64> {
    if crate::planted::is(490) {
        let first = operands.iter().find(|operand| operand.is_nan())?;
        return Some(process_nan(*first, precision, context));
    }
    let first = operands
        .iter()
        .find(|operand| operand.kind == Kind::SignalingNaN)
        .or_else(|| {
            operands
                .iter()
                .find(|operand| operand.kind == Kind::QuietNaN)
        })?;
    Some(process_nan(*first, precision, context))
}

impl Real {
    /// The number with its significand's top bit at bit 62, so that a product of two leaves a bit spare for the
    /// carry of a sum with it, and a quotient keeps enough bits.
    fn from_number(operand: Unpacked) -> Self {
        let shift = operand.mantissa.leading_zeros() - 1;
        Self {
            sign: operand.sign,
            significand: u128::from(operand.mantissa << shift),
            exponent: operand.exponent - shift as i32,
            sticky: false,
        }
    }

    const fn is_zero(self) -> bool {
        self.significand == 0 && !self.sticky
    }
}

/// The exact sum of two reals, which hold no sticky bit.
fn add_reals(first: Real, second: Real) -> Real {
    if first.significand == 0 {
        return second;
    }
    if second.significand == 0 {
        return first;
    }
    let (larger, smaller) = if first.exponent >= second.exponent {
        (first, second)
    } else {
        (second, first)
    };
    let room = larger.significand.leading_zeros().saturating_sub(1);
    let difference = (larger.exponent - smaller.exponent) as u32;
    let (shifted, smaller_significand, sticky) = if difference <= room {
        (larger.significand << difference, smaller.significand, false)
    } else {
        // Keep as many of the larger's bits as fit, then the smaller's that reach them, and the rest as sticky.
        let larger_shift = room;
        let smaller_shift = difference - room;
        let kept = smaller.significand.checked_shr(smaller_shift).unwrap_or(0);
        let lost = smaller.significand != kept << smaller_shift.min(u128::BITS - 1);
        (larger.significand << larger_shift, kept, lost)
    };
    let exponent = larger.exponent - difference.min(room) as i32;
    if larger.sign == smaller.sign {
        return Real {
            sign: larger.sign,
            significand: shifted + smaller_significand,
            exponent,
            sticky,
        };
    }
    // A lost part of the smaller value takes one from the difference and leaves the rest as sticky.
    let borrowed = smaller_significand + u128::from(sticky);
    match shifted.cmp(&borrowed) {
        core::cmp::Ordering::Equal if !sticky => Real {
            sign: false,
            significand: 0,
            exponent,
            sticky: false,
        },
        core::cmp::Ordering::Less => Real {
            sign: smaller.sign,
            significand: borrowed - shifted - u128::from(sticky),
            exponent,
            sticky,
        },
        _ => Real {
            sign: larger.sign,
            significand: shifted - borrowed,
            exponent,
            sticky,
        },
    }
}

fn multiply_reals(first: Real, second: Real) -> Real {
    Real {
        sign: first.sign != second.sign,
        significand: first.significand * second.significand,
        exponent: first.exponent + second.exponent,
        sticky: false,
    }
}

fn divide_reals(dividend: Real, divisor: Real) -> Real {
    let numerator = dividend.significand << QUOTIENT_EXTRA_BITS;
    Real {
        sign: dividend.sign != divisor.sign,
        significand: numerator / divisor.significand,
        exponent: dividend.exponent - divisor.exponent - QUOTIENT_EXTRA_BITS as i32,
        sticky: !numerator.is_multiple_of(divisor.significand),
    }
}

fn square_root_real(operand: Real) -> Real {
    // Shifted up by an amount that leaves the exponent even, so that it halves exactly.
    let shift = QUOTIENT_EXTRA_BITS - 1
        + (operand.exponent - QUOTIENT_EXTRA_BITS as i32 + 1).rem_euclid(2) as u32;
    let scaled = operand.significand << shift;
    let root = scaled.isqrt();
    Real {
        sign: false,
        significand: root,
        exponent: (operand.exponent - shift as i32) / 2,
        sticky: root * root != scaled,
    }
}

/// Whether the rounding of a value whose discarded part compares with one half as given goes up in magnitude: the
/// `round_up` choice of each rounding mode in [FPRoundBase](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPRoundBase).
fn rounds_up(
    rounding: Rounding,
    sign: bool,
    lowest_kept_odd: bool,
    discarded: core::cmp::Ordering,
    inexact: bool,
) -> bool {
    use core::cmp::Ordering::{Equal, Greater};
    match rounding {
        Rounding::TiesToEven if crate::planted::any(&[481, 491, 496]) => {
            discarded != core::cmp::Ordering::Less
        }
        Rounding::TiesToEven => discarded == Greater || (discarded == Equal && lowest_kept_odd),
        Rounding::TiesAway if crate::planted::is(493) => {
            discarded == Greater || (discarded == Equal && lowest_kept_odd)
        }
        Rounding::TiesAway => discarded != core::cmp::Ordering::Less,
        Rounding::TowardMinusInfinity if crate::planted::is(494) => false,
        Rounding::TowardPlusInfinity => inexact && !sign,
        Rounding::TowardMinusInfinity => inexact && sign,
        Rounding::TowardZero => false,
    }
}

/// The significand shifted right by `shift`, how its discarded part compares with one half, and whether any part
/// was discarded.
fn shift_right_rounding(
    significand: u128,
    shift: u32,
    sticky: bool,
) -> (u128, core::cmp::Ordering, bool) {
    // Every real with a sticky part keeps more bits than its precision's, so only an exact one has nothing to shift.
    if shift == 0 {
        return (significand, core::cmp::Ordering::Less, sticky);
    }
    if shift > u128::BITS {
        return (0, core::cmp::Ordering::Less, significand != 0 || sticky);
    }
    let kept = significand.checked_shr(shift).unwrap_or(0);
    let remainder = significand & (u128::MAX >> (u128::BITS - shift));
    let half = 1_u128 << (shift - 1);
    let discarded = match remainder.cmp(&half) {
        core::cmp::Ordering::Equal if sticky => core::cmp::Ordering::Greater,
        ordering => ordering,
    };
    (kept, discarded, remainder != 0 || sticky)
}

/// [FPRoundBase](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPRoundBase): rounds a nonzero real to the precision, raising the flags it does.
fn round(value: Real, precision: Precision, rounding: Rounding, context: &mut Context) -> u64 {
    let rounding = if crate::planted::is(61) && rounding == Rounding::TiesToEven {
        Rounding::TiesAway
    } else {
        rounding
    };
    let fraction_bits = precision.to_fraction_bits() as i32;
    let alternative = context.alternative_half(precision);
    // The real's own exponent, of its most significant bit.
    let top = (u128::BITS - 1 - value.significand.leading_zeros()) as i32 + value.exponent;
    let minimum = precision.to_minimum_exponent();
    if context.flushes(precision) && top < minimum && !crate::planted::any(&[506, 319]) {
        context.raise(UNDERFLOW);
        return precision.to_zero(value.sign && !crate::planted::is(79));
    }
    let mut biased = (top - minimum + 1).max(0);
    let scale = if biased == 0 { minimum } else { top };
    // The bit of value two to `scale - fraction_bits` becomes the lowest kept bit.
    let shift = scale - fraction_bits - value.exponent;
    let (mut mantissa, discarded, inexact) = if shift <= 0 {
        (
            value.significand << (-shift) as u32,
            core::cmp::Ordering::Less,
            value.sticky,
        )
    } else {
        shift_right_rounding(value.significand, shift as u32, value.sticky)
    };
    if biased == 0 && (inexact || crate::planted::is(84)) && !crate::planted::any(&[483, 312]) {
        context.raise(UNDERFLOW);
    }
    let tie_rounding = if crate::planted::is(311) && rounding == Rounding::TiesToEven {
        Rounding::TiesAway
    } else {
        rounding
    };
    if rounds_up(
        tie_rounding,
        value.sign,
        mantissa & 1 != 0,
        discarded,
        inexact,
    ) {
        mantissa += 1;
        if mantissa == 1 << fraction_bits {
            biased = 1;
        }
        if mantissa == 1 << (fraction_bits + 1) {
            biased += 1;
            mantissa >>= 1;
        }
    }
    let sign = precision.to_zero(value.sign);
    let mut inexact = inexact;
    let result = if !alternative {
        if biased as u64 >= precision.to_exponent_ones() {
            if !crate::planted::is(328) {
                context.raise(OVERFLOW);
            }
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
                precision.to_maximum_normal(value.sign)
            }
        } else {
            sign | ((biased as u64) << fraction_bits)
                | (mantissa as u64 & low_ones(fraction_bits as u32))
        }
    } else if biased as u64 > precision.to_exponent_ones() {
        context.raise(INVALID_OPERATION);
        inexact = false;
        sign | low_ones(precision.to_bits() - 1)
    } else {
        sign | ((biased as u64) << fraction_bits)
            | (mantissa as u64 & low_ones(fraction_bits as u32))
    };
    if inexact && !crate::planted::any(&[482, 497, 313, 63]) {
        context.raise(INEXACT);
    }
    result
}

/// The rounded real, or for an exact zero the zero whose sign the rounding mode gives a sum: the exact-zero case of
/// [FPAdd](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPAdd)
/// and [FPMulAdd](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPMulAdd).
fn round_sum(value: Real, precision: Precision, rounding: Rounding, context: &mut Context) -> u64 {
    if value.is_zero() {
        return precision.to_zero(
            (rounding == Rounding::TowardMinusInfinity || crate::planted::is(62))
                && !crate::planted::is(352),
        );
    }
    round(value, precision, rounding, context)
}

/// What one family of floating-point instructions computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FloatingPointOperation {
    /// FMOV of a register's bits, between floating-point and general-purpose registers too.
    Move,
    /// FMOV of the 8-bit immediate [VFPExpandImm](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-vfpexpandimm) expands.
    MoveImmediate,
    Absolute,
    Negate,
    Add,
    Subtract,
    Multiply,
    /// FNMUL: the product, negated after rounding.
    MultiplyNegated,
    Divide,
    Maximum,
    Minimum,
    MaximumNumber,
    MinimumNumber,
    SquareRoot,
    /// FMADD and its forms: the first source negated, the addend negated, or both.
    MultiplyAdd {
        negate_product: bool,
        negate_addend: bool,
    },
    /// FCMP and FCMPE, with a register or zero; FCMPE raises Invalid Operation for a quiet NaN too.
    Compare {
        signaling: bool,
    },
    /// [FCCMP](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/FCCMP--Floating-point-conditional-quiet-compare--scalar--)
    /// and [FCCMPE](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/FCCMPE--Floating-point-conditional-signaling-compare--scalar--):
    /// the flags of the compare where the condition holds, the immediate's where it fails.
    ConditionalCompare {
        signaling: bool,
    },
    ConditionalSelect,
    /// FRINT: to an integral value in the rounding given, or FPCR's; FRINTX raises Inexact.
    RoundToIntegral {
        rounding: Option<Rounding>,
        exact: bool,
    },
    /// FRINT32 and FRINT64: as RoundToIntegral, inside the range of a signed integer of `bits` bits, and always
    /// raising Inexact.
    RoundToIntegralWithin {
        bits: u32,
        rounding: Option<Rounding>,
    },
    /// FCVT between precisions.
    ConvertPrecision,
    /// FCVT to a general-purpose register, with a scale for a fixed-point result.
    ConvertToInteger {
        rounding: Option<Rounding>,
        unsigned: bool,
        fixed_point: bool,
    },
    /// SCVTF and UCVTF from a general-purpose register, with a scale for a fixed-point source.
    ConvertFromInteger {
        unsigned: bool,
        fixed_point: bool,
    },
}

impl FloatingPointOperation {
    /// Whether the family reads FPCR.
    pub const fn reads_control(self) -> bool {
        !matches!(
            self,
            Self::Move
                | Self::MoveImmediate
                | Self::Absolute
                | Self::Negate
                | Self::ConditionalSelect
        )
    }

    pub const fn reads_condition_flags(self) -> bool {
        matches!(
            self,
            Self::ConditionalSelect | Self::ConditionalCompare { .. }
        )
    }
}

/// The family of each modeled scalar floating-point instruction, none for every other instruction.
pub const fn to_floating_point_operation(
    instruction: Instruction<Infallible>,
) -> Option<FloatingPointOperation> {
    use FloatingPointOperation as O;
    use Instruction as I;
    use Rounding::{TiesAway, TiesToEven, TowardMinusInfinity, TowardPlusInfinity, TowardZero};
    const fn to_integer(rounding: Rounding, unsigned: bool) -> FloatingPointOperation {
        FloatingPointOperation::ConvertToInteger {
            rounding: Some(rounding),
            unsigned,
            fixed_point: false,
        }
    }
    const fn round_to(rounding: Option<Rounding>, exact: bool) -> FloatingPointOperation {
        FloatingPointOperation::RoundToIntegral { rounding, exact }
    }
    Some(match instruction {
        I::FmovSingle
        | I::FmovDouble
        | I::FmovHalf
        | I::FmovFixedPointSingleTo32
        | I::Fmov32ToSingle
        | I::FmovHalfTo32
        | I::Fmov32ToHalf
        | I::FmovDoubleTo64
        | I::Fmov64ToDouble
        | I::FmovHalfTo64
        | I::Fmov64ToHalf => O::Move,
        I::FmovImmediateSingle | I::FmovImmediateDouble | I::FmovImmediateHalf => O::MoveImmediate,
        I::FabsSingle | I::FabsDouble | I::FabsHalf => O::Absolute,
        I::FnegSingle | I::FnegDouble | I::FnegHalf => O::Negate,
        I::FaddSingle | I::FaddDouble | I::FaddHalf => O::Add,
        I::FsubSingle | I::FsubDouble | I::FsubHalf => O::Subtract,
        I::FmulSingle | I::FmulDouble | I::FmulHalf => O::Multiply,
        I::FnmulSingle | I::FnmulDouble | I::FnmulHalf => O::MultiplyNegated,
        I::FdivSingle | I::FdivDouble | I::FdivHalf => O::Divide,
        I::FmaxSingle | I::FmaxDouble | I::FmaxHalf => O::Maximum,
        I::FminSingle | I::FminDouble | I::FminHalf => O::Minimum,
        I::FmaxnmSingle | I::FmaxnmDouble | I::FmaxnmHalf => O::MaximumNumber,
        I::FminnmSingle | I::FminnmDouble | I::FminnmHalf => O::MinimumNumber,
        I::FsqrtSingle | I::FsqrtDouble | I::FsqrtHalf => O::SquareRoot,
        I::FmaddSingle | I::FmaddDouble | I::FmaddHalf => O::MultiplyAdd {
            negate_product: false,
            negate_addend: false,
        },
        I::FmsubSingle | I::FmsubDouble | I::FmsubHalf => O::MultiplyAdd {
            negate_product: true,
            negate_addend: false,
        },
        I::FnmaddSingle | I::FnmaddDouble | I::FnmaddHalf => O::MultiplyAdd {
            negate_product: true,
            negate_addend: true,
        },
        I::FnmsubSingle | I::FnmsubDouble | I::FnmsubHalf => O::MultiplyAdd {
            negate_product: false,
            negate_addend: true,
        },
        I::FcmpSingle
        | I::FcmpDouble
        | I::FcmpHalf
        | I::FcmpImmediateSingle
        | I::FcmpImmediateDouble
        | I::FcmpImmediateHalf => O::Compare { signaling: false },
        I::FcmpeSingle
        | I::FcmpeDouble
        | I::FcmpeHalf
        | I::FcmpeImmediateSingle
        | I::FcmpeImmediateDouble
        | I::FcmpeImmediateHalf => O::Compare { signaling: true },
        I::FccmpSingle | I::FccmpDouble | I::FccmpHalf => {
            O::ConditionalCompare { signaling: false }
        }
        I::FccmpeSingle | I::FccmpeDouble | I::FccmpeHalf => {
            O::ConditionalCompare { signaling: true }
        }
        I::FcselSingle | I::FcselDouble | I::FcselHalf => O::ConditionalSelect,
        I::FrintnSingle | I::FrintnDouble | I::FrintnHalf => round_to(Some(TiesToEven), false),
        I::FrintpSingle | I::FrintpDouble | I::FrintpHalf => {
            round_to(Some(TowardPlusInfinity), false)
        }
        I::FrintmSingle | I::FrintmDouble | I::FrintmHalf => {
            round_to(Some(TowardMinusInfinity), false)
        }
        I::FrintzSingle | I::FrintzDouble | I::FrintzHalf => round_to(Some(TowardZero), false),
        I::FrintaSingle | I::FrintaDouble | I::FrintaHalf => round_to(Some(TiesAway), false),
        I::FrintxSingle | I::FrintxDouble | I::FrintxHalf => round_to(None, true),
        I::FrintiSingle | I::FrintiDouble | I::FrintiHalf => round_to(None, false),
        I::Frint32xSingle | I::Frint32xDouble => O::RoundToIntegralWithin {
            bits: u32::BITS,
            rounding: None,
        },
        I::Frint32zSingle | I::Frint32zDouble => O::RoundToIntegralWithin {
            bits: u32::BITS,
            rounding: Some(TowardZero),
        },
        I::Frint64xSingle | I::Frint64xDouble => O::RoundToIntegralWithin {
            bits: u64::BITS,
            rounding: None,
        },
        I::Frint64zSingle | I::Frint64zDouble => O::RoundToIntegralWithin {
            bits: u64::BITS,
            rounding: Some(TowardZero),
        },
        I::FcvtSingleToDouble
        | I::FcvtSingleToHalf
        | I::FcvtDoubleToSingle
        | I::FcvtDoubleToHalf
        | I::FcvtHalfToSingle
        | I::FcvtHalfToDouble => O::ConvertPrecision,
        I::FcvtasIntegerSingleTo32
        | I::FcvtasIntegerDoubleTo32
        | I::FcvtasIntegerHalfTo32
        | I::FcvtasIntegerSingleTo64
        | I::FcvtasIntegerDoubleTo64
        | I::FcvtasIntegerHalfTo64 => to_integer(TiesAway, false),
        I::FcvtauIntegerSingleTo32
        | I::FcvtauIntegerDoubleTo32
        | I::FcvtauIntegerHalfTo32
        | I::FcvtauIntegerSingleTo64
        | I::FcvtauIntegerDoubleTo64
        | I::FcvtauIntegerHalfTo64 => to_integer(TiesAway, true),
        I::FcvtmsIntegerSingleTo32
        | I::FcvtmsIntegerDoubleTo32
        | I::FcvtmsIntegerHalfTo32
        | I::FcvtmsIntegerSingleTo64
        | I::FcvtmsIntegerDoubleTo64
        | I::FcvtmsIntegerHalfTo64 => to_integer(TowardMinusInfinity, false),
        I::FcvtmuIntegerSingleTo32
        | I::FcvtmuIntegerDoubleTo32
        | I::FcvtmuIntegerHalfTo32
        | I::FcvtmuIntegerSingleTo64
        | I::FcvtmuIntegerDoubleTo64
        | I::FcvtmuIntegerHalfTo64 => to_integer(TowardMinusInfinity, true),
        I::FcvtnsIntegerSingleTo32
        | I::FcvtnsIntegerDoubleTo32
        | I::FcvtnsIntegerHalfTo32
        | I::FcvtnsIntegerSingleTo64
        | I::FcvtnsIntegerDoubleTo64
        | I::FcvtnsIntegerHalfTo64 => to_integer(TiesToEven, false),
        I::FcvtnuIntegerSingleTo32
        | I::FcvtnuIntegerDoubleTo32
        | I::FcvtnuIntegerHalfTo32
        | I::FcvtnuIntegerSingleTo64
        | I::FcvtnuIntegerDoubleTo64
        | I::FcvtnuIntegerHalfTo64 => to_integer(TiesToEven, true),
        I::FcvtpsIntegerSingleTo32
        | I::FcvtpsIntegerDoubleTo32
        | I::FcvtpsIntegerHalfTo32
        | I::FcvtpsIntegerSingleTo64
        | I::FcvtpsIntegerDoubleTo64
        | I::FcvtpsIntegerHalfTo64 => to_integer(TowardPlusInfinity, false),
        I::FcvtpuIntegerSingleTo32
        | I::FcvtpuIntegerDoubleTo32
        | I::FcvtpuIntegerHalfTo32
        | I::FcvtpuIntegerSingleTo64
        | I::FcvtpuIntegerDoubleTo64
        | I::FcvtpuIntegerHalfTo64 => to_integer(TowardPlusInfinity, true),
        I::FcvtzsIntegerSingleTo32
        | I::FcvtzsIntegerDoubleTo32
        | I::FcvtzsIntegerHalfTo32
        | I::FcvtzsIntegerSingleTo64
        | I::FcvtzsIntegerDoubleTo64
        | I::FcvtzsIntegerHalfTo64 => to_integer(TowardZero, false),
        I::FcvtzuIntegerSingleTo32
        | I::FcvtzuIntegerDoubleTo32
        | I::FcvtzuIntegerHalfTo32
        | I::FcvtzuIntegerSingleTo64
        | I::FcvtzuIntegerDoubleTo64
        | I::FcvtzuIntegerHalfTo64 => to_integer(TowardZero, true),
        I::FcvtzsFixedPointImmediateSingleTo32
        | I::FcvtzsFixedPointImmediateDoubleTo32
        | I::FcvtzsFixedPointImmediateHalfTo32
        | I::FcvtzsFixedPointImmediateSingleTo64
        | I::FcvtzsFixedPointImmediateDoubleTo64
        | I::FcvtzsFixedPointImmediateHalfTo64 => O::ConvertToInteger {
            rounding: Some(TowardZero),
            unsigned: false,
            fixed_point: true,
        },
        I::FcvtzuFixedPointImmediateSingleTo32
        | I::FcvtzuFixedPointImmediateDoubleTo32
        | I::FcvtzuFixedPointImmediateHalfTo32
        | I::FcvtzuFixedPointImmediateSingleTo64
        | I::FcvtzuFixedPointImmediateDoubleTo64
        | I::FcvtzuFixedPointImmediateHalfTo64 => O::ConvertToInteger {
            rounding: Some(TowardZero),
            unsigned: true,
            fixed_point: true,
        },
        I::ScvtfIntegerImmediate32ToSingle
        | I::ScvtfIntegerImmediate32ToDouble
        | I::ScvtfIntegerImmediate32ToHalf
        | I::ScvtfIntegerImmediate64ToSingle
        | I::ScvtfIntegerImmediate64ToDouble
        | I::ScvtfIntegerImmediate64ToHalf => O::ConvertFromInteger {
            unsigned: false,
            fixed_point: false,
        },
        I::UcvtfIntegerImmediate32ToSingle
        | I::UcvtfIntegerImmediate32ToDouble
        | I::UcvtfIntegerImmediate32ToHalf
        | I::UcvtfIntegerImmediate64ToSingle
        | I::UcvtfIntegerImmediate64ToDouble
        | I::UcvtfIntegerImmediate64ToHalf => O::ConvertFromInteger {
            unsigned: true,
            fixed_point: false,
        },
        I::ScvtfFixedPointImmediate32ToSingle
        | I::ScvtfFixedPointImmediate32ToDouble
        | I::ScvtfFixedPointImmediate32ToHalf
        | I::ScvtfFixedPointImmediate64ToSingle
        | I::ScvtfFixedPointImmediate64ToDouble
        | I::ScvtfFixedPointImmediate64ToHalf => O::ConvertFromInteger {
            unsigned: false,
            fixed_point: true,
        },
        I::UcvtfFixedPointImmediate32ToSingle
        | I::UcvtfFixedPointImmediate32ToDouble
        | I::UcvtfFixedPointImmediate32ToHalf
        | I::UcvtfFixedPointImmediate64ToSingle
        | I::UcvtfFixedPointImmediate64ToDouble
        | I::UcvtfFixedPointImmediate64ToHalf => O::ConvertFromInteger {
            unsigned: true,
            fixed_point: true,
        },
        _ => return None,
    })
}

/// One operand as the skeleton reads it.
#[derive(Clone, Copy)]
enum Source {
    FloatingPoint { bits: u64, precision: Precision },
    General { value: u64, width: RegisterWidth },
    Immediate(u64),
}

/// Where the result goes.
#[derive(Clone, Copy)]
enum Destination {
    FloatingPoint { number: u32, precision: Precision },
    General { number: u32, width: RegisterWidth },
}

/// The most source operands a scalar floating-point instruction reads: FMADD's three.
const MAXIMUM_SOURCES: usize = 3;

/// Reads the operands, applies the operation, and writes its result, FPSR's flags and the condition flags.
pub fn simulate_floating_point<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operands: &[Operand],
    operation: FloatingPointOperation,
) -> Result<(), Exception> {
    let mut sources = [Source::Immediate(0); MAXIMUM_SOURCES];
    let mut count = 0;
    let mut destination = None;
    let mut condition = 0;
    for operand in operands {
        match *operand {
            Operand::FloatingPointRegister {
                number,
                precision,
                access: OperandAccess::Written,
            } => {
                destination = Some(Destination::FloatingPoint {
                    number: number.read(word),
                    precision,
                });
            }
            Operand::FloatingPointRegister {
                number, precision, ..
            } => {
                let bits = processor.vector_registers[number.read(word) as usize] as u64
                    & low_ones(precision.to_bits());
                sources[count] = Source::FloatingPoint { bits, precision };
                count += 1;
            }
            Operand::GeneralPurposeRegister {
                number,
                width,
                access: OperandAccess::Written,
                ..
            } => {
                destination = Some(Destination::General {
                    number: number.read(word),
                    width,
                });
            }
            Operand::GeneralPurposeRegister { number, width, .. } => {
                let value = read_register(
                    processor,
                    number.read(word),
                    RegisterThirtyOne::ZeroRegister,
                ) & width.to_mask();
                sources[count] = Source::General { value, width };
                count += 1;
            }
            Operand::Condition(field) => condition = field.read(word),
            Operand::UnsignedImmediate(field) => {
                sources[count] = Source::Immediate(u64::from(field.read(word)));
                count += 1;
            }
            // The field a compare with zero leaves undecoded.
            Operand::Ignored(_) => {}
            _ => panic!("{operand:?} is no operand of a scalar floating-point instruction"),
        }
    }
    let sources = &sources[..count];
    let mut context = Context {
        fpcr: processor.floating_point_control,
        flags: 0,
        conversion: false,
    };
    let fbits = if crate::planted::is(87) {
        SCALE.read(word)
    } else {
        FIXED_POINT_SCALE_LIMIT - SCALE.read(word)
    };
    let result = apply(
        operation,
        sources,
        destination,
        condition,
        fbits,
        processor.condition_flags,
        &mut context,
    );
    match (result, destination) {
        (Outcome::Value(bits), Some(Destination::FloatingPoint { number, precision })) => {
            let kept = if crate::planted::any(&[502, 503, 336, 339, 82]) {
                processor.vector_registers[number as usize]
                    & !(u128::MAX >> (u128::BITS - precision.to_bits()))
            } else {
                0
            };
            processor.vector_registers[number as usize] =
                u128::from(bits & low_ones(precision.to_bits())) | kept;
        }
        (Outcome::Value(value), Some(Destination::General { number, width })) => {
            write_register(
                processor,
                number,
                value & width.to_mask(),
                RegisterThirtyOne::ZeroRegister,
            );
        }
        (Outcome::Flags(flags), _) => processor.condition_flags = flags,
        (Outcome::Value(_), None) => panic!("{operation:?} writes no register"),
    }
    if crate::planted::is(80) {
        const CUMULATIVE: u64 =
            INVALID_OPERATION | DIVIDE_BY_ZERO | OVERFLOW | UNDERFLOW | INEXACT | INPUT_DENORMAL;
        processor.floating_point_status &= !CUMULATIVE;
    }
    processor.floating_point_status |= context.flags;
    Ok(())
}

enum Outcome {
    Value(u64),
    Flags(ConditionFlags),
}

/// The precision of the first floating-point source, or of the destination.
fn first_precision(sources: &[Source], destination: Option<Destination>) -> Precision {
    sources
        .iter()
        .find_map(|source| match source {
            Source::FloatingPoint { precision, .. } => Some(*precision),
            _ => None,
        })
        .or(match destination {
            Some(Destination::FloatingPoint { precision, .. }) => Some(precision),
            _ => None,
        })
        .expect("a floating-point instruction names a floating-point register")
}

fn apply(
    operation: FloatingPointOperation,
    sources: &[Source],
    destination: Option<Destination>,
    condition: u32,
    fbits: u32,
    flags: ConditionFlags,
    context: &mut Context,
) -> Outcome {
    use FloatingPointOperation as O;
    let precision = first_precision(sources, destination);
    let rounding = to_rounding(context.fpcr);
    let bits_of = |index: usize| match sources[index] {
        Source::FloatingPoint { bits, .. }
        | Source::General { value: bits, .. }
        | Source::Immediate(bits) => bits,
    };
    Outcome::Value(match operation {
        O::Move if crate::planted::is(360) && is_nan_bits(bits_of(0), precision) => {
            bits_of(0) | precision.to_quiet_bit()
        }
        O::Move => bits_of(0),
        O::MoveImmediate => expand_immediate(bits_of(0), precision),
        O::Absolute
            if crate::planted::is(501)
                && bits_of(0) & !precision.to_sign_bit() > precision.to_infinity(false) =>
        {
            bits_of(0)
        }
        O::Absolute => bits_of(0) & !precision.to_sign_bit(),
        O::Negate if crate::planted::is(81) => {
            let operand = unpack(bits_of(0), precision, context);
            let bits = if operand.kind == Kind::SignalingNaN {
                context.raise(INVALID_OPERATION);
                bits_of(0) | precision.to_quiet_bit()
            } else {
                bits_of(0)
            };
            bits ^ precision.to_sign_bit()
        }
        O::Negate if crate::planted::is(337) && is_nan_bits(bits_of(0), precision) => bits_of(0),
        O::Negate => bits_of(0) ^ precision.to_sign_bit(),
        O::ConditionalSelect => {
            if CONDITIONS[condition as usize].holds(flags)
                && !(crate::planted::is(498) && condition == 15)
            {
                bits_of(0)
            } else {
                bits_of(1)
            }
        }
        O::Add | O::Subtract => {
            let first = unpack(bits_of(0), precision, context);
            let mut second = unpack(bits_of(1), precision, context);
            if let Some(nan) = process_nans(&[first, second], precision, context) {
                return Outcome::Value(nan);
            }
            if operation == O::Subtract {
                second.sign = !second.sign;
            }
            add(first, second, precision, rounding, context)
        }
        O::Multiply | O::MultiplyNegated => {
            let mut first = unpack(bits_of(0), precision, context);
            let second = unpack(bits_of(1), precision, context);
            let early = crate::planted::is(89)
                && operation == O::MultiplyNegated
                && !first.is_nan()
                && !second.is_nan();
            if early {
                first.sign = !first.sign;
            }
            let product = process_nans(&[first, second], precision, context)
                .unwrap_or_else(|| multiply(first, second, precision, rounding, context));
            let negated_zero_kept =
                crate::planted::is(317) && product & !precision.to_sign_bit() == 0;
            if operation == O::MultiplyNegated && !early && !negated_zero_kept {
                product ^ precision.to_sign_bit()
            } else {
                product
            }
        }
        O::Divide => {
            let first = unpack(bits_of(0), precision, context);
            let second = unpack(bits_of(1), precision, context);
            if let Some(nan) = process_nans(&[first, second], precision, context) {
                return Outcome::Value(nan);
            }
            divide(first, second, precision, rounding, context)
        }
        O::Maximum | O::Minimum | O::MaximumNumber | O::MinimumNumber => {
            let mut first = unpack(bits_of(0), precision, context);
            let mut second = unpack(bits_of(1), precision, context);
            let maximum = matches!(operation, O::Maximum | O::MaximumNumber);
            if matches!(operation, O::MaximumNumber | O::MinimumNumber) && !crate::planted::is(486)
            {
                // FPMaxNum and FPMinNum: a quiet NaN against a number loses, as the infinity on its side would.
                let infinity = unpack(precision.to_infinity(maximum), precision, context);
                let replaced = |kind: Kind| {
                    kind == Kind::QuietNaN || (crate::planted::is(85) && kind == Kind::SignalingNaN)
                };
                match (first.kind, second.kind) {
                    _ if crate::planted::is(68) => {}
                    (nan, kind) if crate::planted::is(85) && replaced(nan) && !replaced(kind) => {
                        first = infinity
                    }
                    (kind, nan) if crate::planted::is(85) && replaced(nan) && !replaced(kind) => {
                        second = infinity
                    }
                    (Kind::QuietNaN, kind)
                        if kind != Kind::QuietNaN && kind != Kind::SignalingNaN =>
                    {
                        first = infinity
                    }
                    (kind, Kind::QuietNaN)
                        if kind != Kind::QuietNaN && kind != Kind::SignalingNaN =>
                    {
                        second = infinity
                    }
                    _ => {}
                }
            }
            if let Some(nan) = process_nans(&[first, second], precision, context) {
                return Outcome::Value(nan);
            }
            maximum_or_minimum(first, second, maximum, precision)
        }
        // [FPSqrt](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPSqrt).
        O::SquareRoot => {
            let operand = unpack(bits_of(0), precision, context);
            match operand.kind {
                Kind::QuietNaN | Kind::SignalingNaN => process_nan(operand, precision, context),
                Kind::Zero => precision.to_zero(operand.sign && !crate::planted::is(65)),
                Kind::Infinity if !operand.sign => operand.bits,
                _ if operand.sign => {
                    context.raise(INVALID_OPERATION);
                    precision.to_default_nan()
                }
                _ => round(
                    square_root_real(Real::from_number(operand)),
                    precision,
                    rounding,
                    context,
                ),
            }
        }
        O::MultiplyAdd {
            negate_product,
            negate_addend,
        } => {
            // FMADD's sources are the multiplicands, then the addend; FMSUB and FNMADD negate the first, FNMADD
            // and FNMSUB the addend, before NaNs are processed.
            let negate = |bits: u64, negated: bool| {
                if negated {
                    bits ^ precision.to_sign_bit()
                } else {
                    bits
                }
            };
            let first = unpack(negate(bits_of(0), negate_product), precision, context);
            let second = unpack(bits_of(1), precision, context);
            let addend = unpack(negate(bits_of(2), negate_addend), precision, context);
            multiply_add(addend, first, second, precision, rounding, context)
        }
        O::Compare { signaling } => {
            let first = unpack(bits_of(0), precision, context);
            // FCMP with zero compares with +0.0.
            let second = if sources.len() > 1 {
                unpack(bits_of(1), precision, context)
            } else {
                unpack(0, precision, context)
            };
            return Outcome::Flags(compare(
                first,
                second,
                signaling && !crate::planted::is(499),
                context,
            ));
        }
        // The compare unpacks its sources only where the condition holds, so only there can it raise a flag.
        O::ConditionalCompare { signaling } => {
            let signaling = signaling && !crate::planted::is(343);
            if !CONDITIONS[condition as usize].holds(flags) != crate::planted::is(342) {
                if crate::planted::is(344) {
                    let mut scratch = Context {
                        fpcr: 0,
                        flags: 0,
                        conversion: false,
                    };
                    if [bits_of(0), bits_of(1)].iter().any(|bits| {
                        unpack(*bits, precision, &mut scratch).kind == Kind::SignalingNaN
                    }) {
                        context.raise(INVALID_OPERATION);
                    }
                }
                return Outcome::Flags(ConditionFlags::from_bits_truncate(
                    (bits_of(2) as u32) << ConditionFlags::OVERFLOW.bits().trailing_zeros(),
                ));
            }
            let first = unpack(bits_of(0), precision, context);
            let second = unpack(bits_of(1), precision, context);
            return Outcome::Flags(compare(first, second, signaling, context));
        }
        O::RoundToIntegral {
            rounding: chosen,
            exact,
        } => {
            let operand = unpack(bits_of(0), precision, context);
            let chosen = if crate::planted::is(331) && chosen == Some(Rounding::TiesToEven) {
                Some(Rounding::TiesAway)
            } else if crate::planted::is(333) && chosen.is_none() {
                Some(Rounding::TiesToEven)
            } else {
                chosen
            };
            round_to_integral(
                operand,
                chosen.unwrap_or(rounding),
                exact,
                None,
                precision,
                context,
            )
        }
        O::RoundToIntegralWithin {
            bits,
            rounding: chosen,
        } => {
            let operand = unpack(bits_of(0), precision, context);
            round_to_integral(
                operand,
                chosen.unwrap_or(rounding),
                !crate::planted::is(504),
                Some(bits),
                precision,
                context,
            )
        }
        O::ConvertPrecision => {
            context.conversion = true;
            let Some(Destination::FloatingPoint {
                precision: target, ..
            }) = destination
            else {
                panic!("FCVT writes a floating-point register");
            };
            let operand = unpack(bits_of(0), precision, context);
            convert(operand, precision, target, rounding, context)
        }
        O::ConvertToInteger {
            rounding: chosen,
            unsigned,
            fixed_point,
        } => {
            let Some(Destination::General { width, .. }) = destination else {
                panic!("a conversion to an integer writes a general-purpose register");
            };
            let operand = unpack(bits_of(0), precision, context);
            let fbits = if fixed_point { fbits } else { 0 };
            let chosen = if crate::planted::is(322) && chosen == Some(Rounding::TiesToEven) {
                Some(Rounding::TiesAway)
            } else if crate::planted::is(324) && chosen == Some(Rounding::TowardMinusInfinity) {
                Some(Rounding::TowardZero)
            } else {
                chosen
            };
            to_fixed(
                operand,
                fbits,
                unsigned,
                width,
                chosen.unwrap_or(rounding),
                context,
            )
        }
        O::ConvertFromInteger {
            unsigned,
            fixed_point,
        } => {
            let Source::General { value, width } = sources[0] else {
                panic!("a conversion from an integer reads a general-purpose register");
            };
            let fbits = if fixed_point { fbits } else { 0 };
            let fbits = if crate::planted::is(329) { 0 } else { fbits };
            let rounding = if crate::planted::is(327) {
                Rounding::TowardZero
            } else {
                rounding
            };
            from_fixed(value, width, fbits, unsigned, precision, rounding, context)
        }
    })
}

/// Whether the bits of the precision are a NaN, for the planted bugs.
fn is_nan_bits(bits: u64, precision: Precision) -> bool {
    let fraction_bits = precision.to_fraction_bits();
    (bits >> fraction_bits) & precision.to_exponent_ones() == precision.to_exponent_ones()
        && bits & low_ones(fraction_bits) != 0
}

/// [VFPExpandImm](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-vfpexpandimm): an 8-bit immediate as a value of the precision.
fn expand_immediate(immediate: u64, precision: Precision) -> u64 {
    const IMMEDIATE_FRACTION_BITS: u32 = 4;
    const IMMEDIATE_EXPONENT_LOW_BITS: u32 = 2;
    let exponent_bits = precision.to_exponent_bits();
    let fraction_bits = precision.to_fraction_bits();
    let sign = (immediate >> (u8::BITS - 1 - u32::from(crate::planted::is(340)))) & 1;
    let not_six = ((immediate >> (u8::BITS - 2)) & 1) ^ 1;
    let six = (immediate >> (u8::BITS - 2)) & 1;
    let exponent_low =
        (immediate >> IMMEDIATE_FRACTION_BITS) & low_ones(IMMEDIATE_EXPONENT_LOW_BITS);
    let exponent_low = if crate::planted::is(91) {
        ((exponent_low & 1) << 1) | (exponent_low >> 1)
    } else {
        exponent_low
    };
    let replicated = if six != 0 {
        low_ones(
            exponent_bits - IMMEDIATE_EXPONENT_LOW_BITS - 1 - u32::from(crate::planted::is(88)),
        )
    } else {
        0
    };
    let exponent = (not_six << (exponent_bits - 1))
        | (replicated << IMMEDIATE_EXPONENT_LOW_BITS)
        | exponent_low;
    let fraction = (immediate & low_ones(IMMEDIATE_FRACTION_BITS))
        << (fraction_bits - IMMEDIATE_FRACTION_BITS);
    (sign << (precision.to_bits() - 1)) | (exponent << fraction_bits) | fraction
}

/// [FPAdd](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPAdd) of two operands that are not NaNs, the second's sign already flipped for a subtraction.
fn add(
    first: Unpacked,
    second: Unpacked,
    precision: Precision,
    rounding: Rounding,
    context: &mut Context,
) -> u64 {
    match (first.kind, second.kind) {
        (Kind::Infinity, Kind::Infinity) if first.sign != second.sign => {
            context.raise(INVALID_OPERATION);
            precision.to_default_nan()
        }
        (Kind::Infinity, _) => precision.to_infinity(first.sign),
        (_, Kind::Infinity) => precision.to_infinity(second.sign),
        (Kind::Zero, Kind::Zero) if first.sign == second.sign => precision.to_zero(first.sign),
        _ => {
            let sum = add_reals(to_real(first), to_real(second));
            round_sum(sum, precision, rounding, context)
        }
    }
}

/// A number or zero as a real.
fn to_real(operand: Unpacked) -> Real {
    match operand.kind {
        Kind::Number => Real::from_number(operand),
        _ => Real {
            sign: operand.sign,
            significand: 0,
            exponent: 0,
            sticky: false,
        },
    }
}

/// [FPMul](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPMul) of operands that are not NaNs.
fn multiply(
    first: Unpacked,
    second: Unpacked,
    precision: Precision,
    rounding: Rounding,
    context: &mut Context,
) -> u64 {
    let sign = first.sign != second.sign;
    match (first.kind, second.kind) {
        (Kind::Infinity, Kind::Zero) | (Kind::Zero, Kind::Infinity) => {
            context.raise(INVALID_OPERATION);
            precision.to_default_nan()
        }
        (Kind::Infinity, _) | (_, Kind::Infinity) => precision.to_infinity(sign),
        (Kind::Zero, _) | (_, Kind::Zero) => precision.to_zero(sign),
        _ => round(
            multiply_reals(to_real(first), to_real(second)),
            precision,
            rounding,
            context,
        ),
    }
}

/// [FPDiv](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPDiv) of operands that are not NaNs.
fn divide(
    first: Unpacked,
    second: Unpacked,
    precision: Precision,
    rounding: Rounding,
    context: &mut Context,
) -> u64 {
    let sign = first.sign != second.sign;
    match (first.kind, second.kind) {
        (Kind::Infinity, Kind::Infinity) | (Kind::Zero, Kind::Zero) => {
            context.raise(INVALID_OPERATION);
            precision.to_default_nan()
        }
        (Kind::Infinity, _) => precision.to_infinity(sign),
        (_, Kind::Zero) => {
            context.raise(if crate::planted::is(64) {
                INVALID_OPERATION
            } else {
                DIVIDE_BY_ZERO
            });
            precision.to_infinity(sign)
        }
        (Kind::Zero, _) | (_, Kind::Infinity) => precision.to_zero(if crate::planted::is(487) {
            first.sign
        } else {
            sign
        }),
        _ => round(
            divide_reals(to_real(first), to_real(second)),
            precision,
            rounding,
            context,
        ),
    }
}

/// Orders two operands that are not NaNs by value.
fn compare_values(first: Unpacked, second: Unpacked) -> core::cmp::Ordering {
    let magnitude = |operand: Unpacked| -> (u8, i32, u128) {
        match operand.kind {
            Kind::Zero => (0, 0, 0),
            Kind::Infinity => (2, 0, 0),
            _ => {
                // Normalized so that the exponent orders numbers of either form.
                let shift = operand.mantissa.leading_zeros();
                (
                    1,
                    operand.exponent - shift as i32,
                    u128::from(operand.mantissa << shift),
                )
            }
        }
    };
    let (first_magnitude, second_magnitude) = (magnitude(first), magnitude(second));
    let first_negative = first.sign && first.kind != Kind::Zero;
    let second_negative = second.sign && second.kind != Kind::Zero;
    match (first_negative, second_negative) {
        (false, true) => core::cmp::Ordering::Greater,
        (true, false) => core::cmp::Ordering::Less,
        (false, false) => first_magnitude.cmp(&second_magnitude),
        (true, true) => second_magnitude.cmp(&first_magnitude),
    }
}

/// [FPMax](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPMax) and [FPMin](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPMin) of operands that are not NaNs: a pair of zeros keeps the sign of both for the maximum, of either
/// for the minimum.
fn maximum_or_minimum(
    first: Unpacked,
    second: Unpacked,
    maximum: bool,
    precision: Precision,
) -> u64 {
    if first.kind == Kind::Zero && second.kind == Kind::Zero {
        let sign = if crate::planted::is(67) {
            first.sign
        } else if maximum && crate::planted::is(505) {
            first.sign || second.sign
        } else if maximum || crate::planted::any(&[316, 346]) {
            first.sign && second.sign
        } else {
            first.sign || second.sign
        };
        return precision.to_zero(sign);
    }
    let first_larger = compare_values(first, second) == core::cmp::Ordering::Greater;
    let chosen = if first_larger == maximum {
        first
    } else {
        second
    };
    match chosen.kind {
        // A zero or infinity is rebuilt from its sign; a flushed denormal is a zero.
        Kind::Zero => precision.to_zero(chosen.sign),
        Kind::Infinity => precision.to_infinity(chosen.sign),
        _ => chosen.bits,
    }
}

/// [FPMulAdd](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPMulAdd): `addend` plus the product, which the caller has negated where the instruction asks.
fn multiply_add(
    addend: Unpacked,
    first: Unpacked,
    second: Unpacked,
    precision: Precision,
    rounding: Rounding,
    context: &mut Context,
) -> u64 {
    let infinite_times_zero = (first.kind == Kind::Infinity && second.kind == Kind::Zero)
        || (first.kind == Kind::Zero && second.kind == Kind::Infinity);
    let nan = if crate::planted::is(349) {
        process_nans(&[addend, second, first], precision, context)
    } else {
        process_nans(&[addend, first, second], precision, context)
    };
    if addend.kind == Kind::QuietNaN && infinite_times_zero && !crate::planted::is(350) {
        context.raise(INVALID_OPERATION);
        return precision.to_default_nan();
    }
    if let Some(nan) = nan {
        return nan;
    }
    let product_sign = first.sign != second.sign;
    let product_infinite = first.kind == Kind::Infinity || second.kind == Kind::Infinity;
    let product_zero = first.kind == Kind::Zero || second.kind == Kind::Zero;
    let addend_infinite = addend.kind == Kind::Infinity;
    if infinite_times_zero || (addend_infinite && product_infinite && addend.sign != product_sign) {
        context.raise(INVALID_OPERATION);
        return precision.to_default_nan();
    }
    if (addend_infinite && !addend.sign) || (product_infinite && !product_sign) {
        return precision.to_infinity(false);
    }
    if addend_infinite || product_infinite {
        return precision.to_infinity(true);
    }
    if addend.kind == Kind::Zero && product_zero && addend.sign == product_sign {
        return precision.to_zero(addend.sign);
    }
    let product = if product_zero {
        Real {
            sign: product_sign,
            significand: 0,
            exponent: 0,
            sticky: false,
        }
    } else if crate::planted::is(66) {
        let rounded = round(
            multiply_reals(to_real(first), to_real(second)),
            precision,
            rounding,
            context,
        );
        let unpacked = unpack(rounded, precision, context);
        match unpacked.kind {
            Kind::Number => Real::from_number(unpacked),
            Kind::Infinity => return rounded,
            _ => Real {
                sign: product_sign,
                significand: 0,
                exponent: 0,
                sticky: false,
            },
        }
    } else {
        multiply_reals(to_real(first), to_real(second))
    };
    let sum = add_reals(to_real(addend), product);
    round_sum(sum, precision, rounding, context)
}

/// [FPCompare](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPCompare): the condition flags, unordered for a NaN, which raises Invalid Operation if it signals or the compare
/// signals for every NaN.
fn compare(
    first: Unpacked,
    second: Unpacked,
    signaling: bool,
    context: &mut Context,
) -> ConditionFlags {
    if first.is_nan() || second.is_nan() {
        let signaling = (signaling && !crate::planted::is(70)) || crate::planted::is(69);
        if (signaling || first.kind == Kind::SignalingNaN || second.kind == Kind::SignalingNaN)
            && !crate::planted::is(500)
        {
            context.raise(INVALID_OPERATION);
        }
        if crate::planted::is(341) {
            return ConditionFlags::ZERO | ConditionFlags::CARRY;
        }
        return ConditionFlags::CARRY | ConditionFlags::OVERFLOW;
    }
    match compare_values(first, second) {
        core::cmp::Ordering::Equal => ConditionFlags::ZERO | ConditionFlags::CARRY,
        core::cmp::Ordering::Less => ConditionFlags::NEGATIVE,
        core::cmp::Ordering::Greater => ConditionFlags::CARRY,
    }
}

/// The integer part of a number times two to `fbits`, rounded as given, and whether rounding discarded anything: the
/// rounding step that [FPToFixed](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPToFixed)
/// and [FPRoundInt](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPRoundInt) share.
fn to_integral(operand: Unpacked, fbits: u32, rounding: Rounding) -> (bool, u128, bool) {
    let exponent = operand.exponent + fbits as i32;
    let significand = u128::from(operand.mantissa);
    let (mut magnitude, discarded, inexact) = if exponent >= 0 {
        // Saturates far beyond any integer's range, so the caller sees an overflow.
        let shifted = if exponent as u32 >= u128::BITS - u64::BITS {
            u128::MAX >> 1
        } else {
            significand << exponent as u32
        };
        (shifted, core::cmp::Ordering::Less, false)
    } else {
        shift_right_rounding(significand, exponent.unsigned_abs(), false)
    };
    if rounds_up(
        rounding,
        operand.sign,
        magnitude & 1 != 0,
        discarded,
        inexact,
    ) {
        magnitude += 1;
    }
    (operand.sign, magnitude, inexact)
}

/// [FPRoundInt](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPRoundInt) and [FPRoundIntN](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPRoundIntN): the operand rounded to an integral value, inside a signed integer's range if given.
fn round_to_integral(
    operand: Unpacked,
    rounding: Rounding,
    exact: bool,
    within_bits: Option<u32>,
    precision: Precision,
    context: &mut Context,
) -> u64 {
    let within_bits = if crate::planted::is(495) {
        None
    } else {
        within_bits
    };
    let out_of_range = |context: &mut Context, bits: u32| {
        if !crate::planted::is(86) {
            context.raise(INVALID_OPERATION);
        }
        // The most negative integer of the range, which every format holds exactly.
        let real = Real {
            sign: true,
            significand: 1,
            exponent: bits as i32 - 1,
            sticky: false,
        };
        round(real, precision, Rounding::TowardZero, context)
    };
    match (operand.kind, within_bits) {
        (Kind::QuietNaN | Kind::SignalingNaN | Kind::Infinity, Some(bits)) => {
            return out_of_range(context, bits);
        }
        (Kind::QuietNaN | Kind::SignalingNaN, None) => {
            return process_nan(operand, precision, context);
        }
        (Kind::Infinity, None) => return operand.bits,
        (Kind::Zero, _) => return precision.to_zero(operand.sign),
        _ => {}
    }
    // A number with no fraction bits is integral already; it fits a signed range only below its top power of two.
    if operand.exponent >= 0 {
        let top = (u64::BITS - 1 - operand.mantissa.leading_zeros()) as i32 + operand.exponent;
        let fits = within_bits.is_none_or(|bits| {
            top < bits as i32 - 1
                || (operand.sign && top == bits as i32 - 1 && operand.mantissa.is_power_of_two())
        });
        return match (fits, within_bits) {
            (true, _) | (false, None) => operand.bits,
            (false, Some(bits)) => out_of_range(context, bits),
        };
    }
    let (sign, magnitude, inexact) = to_integral(operand, 0, rounding);
    if let Some(bits) = within_bits {
        let limit = 1_u128 << (bits - 1);
        if (sign && magnitude > limit) || (!sign && magnitude >= limit) {
            return out_of_range(context, bits);
        }
    }
    let result = if magnitude == 0 {
        precision.to_zero(sign && !crate::planted::any(&[330, 78]))
    } else {
        round(
            Real {
                sign,
                significand: magnitude,
                exponent: 0,
                sticky: false,
            },
            precision,
            Rounding::TowardZero,
            context,
        )
    };
    if exact && inexact && !crate::planted::is(332) {
        context.raise(INEXACT);
    }
    result
}

/// [FPConvert](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPConvert): between precisions, a NaN keeping the top of its payload.
fn convert(
    operand: Unpacked,
    from: Precision,
    to: Precision,
    rounding: Rounding,
    context: &mut Context,
) -> u64 {
    let alternative = context.alternative_half(to);
    match operand.kind {
        Kind::QuietNaN | Kind::SignalingNaN => {
            if alternative {
                context.raise(INVALID_OPERATION);
                return to.to_zero(operand.sign);
            }
            if operand.kind == Kind::SignalingNaN {
                context.raise(INVALID_OPERATION);
            }
            if context.default_nan() || crate::planted::is(321) {
                return to.to_default_nan();
            }
            let payload = operand.bits & low_ones(from.to_fraction_bits());
            let payload = if crate::planted::is(71) { 0 } else { payload };
            if crate::planted::is(72) && operand.kind == Kind::SignalingNaN {
                let payload = payload >> (from.to_fraction_bits() - to.to_fraction_bits());
                return to.to_infinity(operand.sign) | (payload & !to.to_quiet_bit());
            }
            let payload = if to.to_fraction_bits() >= from.to_fraction_bits() {
                payload << (to.to_fraction_bits() - from.to_fraction_bits())
            } else {
                payload >> (from.to_fraction_bits() - to.to_fraction_bits())
            };
            let quiet = if crate::planted::is(320) && operand.kind == Kind::SignalingNaN {
                0
            } else {
                to.to_quiet_bit()
            };
            to.to_infinity(operand.sign) | quiet | payload
        }
        Kind::Infinity if alternative => {
            context.raise(INVALID_OPERATION);
            to.to_zero(operand.sign) | low_ones(to.to_bits() - 1)
        }
        Kind::Infinity => to.to_infinity(operand.sign),
        Kind::Zero => to.to_zero(operand.sign),
        Kind::Number => round(Real::from_number(operand), to, rounding, context),
    }
}

/// [FPToFixed](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPToFixed): the operand times two to `fbits` as an integer of the width, saturated with Invalid Operation.
fn to_fixed(
    operand: Unpacked,
    fbits: u32,
    unsigned: bool,
    width: RegisterWidth,
    rounding: Rounding,
    context: &mut Context,
) -> u64 {
    let bits = width.to_bits();
    let (sign, magnitude, inexact) = match operand.kind {
        Kind::QuietNaN | Kind::SignalingNaN => {
            context.raise(INVALID_OPERATION);
            if crate::planted::is(74) && !unsigned {
                return (1_u64 << (bits - 1)) & width.to_mask();
            }
            return 0;
        }
        Kind::Zero => return 0,
        Kind::Infinity => (operand.sign, u128::MAX, false),
        Kind::Number if crate::planted::is(326) => {
            let mut divided = operand;
            divided.exponent -= 2 * fbits as i32;
            to_integral(divided, fbits, rounding)
        }
        Kind::Number => to_integral(
            operand,
            fbits,
            if crate::planted::is(76) && rounding == Rounding::TiesAway {
                Rounding::TiesToEven
            } else {
                rounding
            },
        ),
    };
    let (minimum, maximum): (i128, i128) = if unsigned {
        (0, (1 << bits) - 1)
    } else {
        (-(1 << (bits - 1)), (1 << (bits - 1)) - 1)
    };
    let magnitude = i128::try_from(magnitude).unwrap_or(i128::MAX);
    let value = if sign { -magnitude } else { magnitude };
    if crate::planted::any(&[75, 355]) && unsigned && value < 0 {
        return 0;
    }
    if value < minimum || value > maximum {
        if !crate::planted::is(325) {
            context.raise(INVALID_OPERATION);
        }
        if crate::planted::is(475) && value > maximum {
            return (maximum + 1) as u64 & width.to_mask();
        }
        if crate::planted::is(476) && value < minimum {
            return width.to_mask();
        }
        return value.clamp(minimum, maximum) as u64 & width.to_mask();
    }
    if inexact && !crate::planted::any(&[492, 323]) {
        context.raise(INEXACT);
    }
    value as u64 & width.to_mask()
}

/// [FixedToFP](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FixedToFP): the integer of the width divided by two to `fbits`, rounded to the precision.
fn from_fixed(
    value: u64,
    width: RegisterWidth,
    fbits: u32,
    unsigned: bool,
    precision: Precision,
    rounding: Rounding,
    context: &mut Context,
) -> u64 {
    let bits = width.to_bits();
    let negative = !unsigned && (value >> (bits - 1)) & 1 != 0;
    let magnitude = if negative {
        (value.wrapping_neg()) & width.to_mask()
    } else {
        value
    };
    // The magnitude of the most negative value is one beyond the width's signed range, and still fits it unsigned.
    let magnitude = if negative && magnitude == 0 {
        1 << (bits - 1)
    } else {
        magnitude
    };
    if magnitude == 0 {
        return precision.to_zero(false);
    }
    let result = round(
        Real {
            sign: negative,
            significand: u128::from(magnitude),
            exponent: -(fbits as i32),
            sticky: false,
        },
        precision,
        rounding,
        context,
    );
    if crate::planted::is(77) {
        context.flags &= !INEXACT;
    }
    result
}
