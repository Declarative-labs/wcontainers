//! The x87 transcendental forms: F2XM1, FSIN, FCOS, FPTAN, FSINCOS, FPATAN, FYL2X and FYL2XP1, as the
//! [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (vol. 1, 8.3.10 and
//! 8.3.11; vol. 2, each instruction's page) gives them.
//!
//! Chip makers round these results differently in their last bits; the manual bounds the error at under one unit in
//! the last place when rounding to nearest and one and a half otherwise. So a behavior leaves each register such a
//! result goes to, and C1 (which says how it was rounded), out of what it compares, and compares the rest: the
//! special values the manual's tables give, the stack's movement, C2, the exception flags and which register is
//! written. The value the model writes is its own, within the bound: worked out in fixed point to about 120 bits and
//! rounded once to 64 bits by FCW's rounding control. FCW's precision control does not apply to these forms.
//!
//! The trigonometric forms reduce their argument by multiples of π/2 with the 66-bit π the manual names, so their
//! results are those of the reduced argument; an argument of magnitude 2 to the 63 or more is out of range: C2 set
//! and nothing else changed.
//!
//! A translator that replays a program cannot know a chip's last bits; it computes these results in software, within
//! the same bound, and the replay takes them as the chip's.

use crate::planted::{Bug, is_active};
use crate::x87::{BUSY, Class, INDEFINITE, SUMMARY, classify};
use crate::x87_arithmetic::{
    DENORMAL, INVALID, OVERFLOW, Outcome, PRECISION, UNDERFLOW, ZERO_DIVIDE, is_negative, propagate, quiet,
    round_approximation,
};

/// What a transcendental form computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Function {
    /// F2XM1: 2 to ST(0), less one.
    TwoToXMinusOne,
    /// FSIN.
    Sine,
    /// FCOS.
    Cosine,
    /// FPTAN: the tangent, then 1.0 pushed.
    Tangent,
    /// FSINCOS: the sine, then the cosine pushed.
    SineCosine,
    /// FPATAN: the arctangent of ST(1) over ST(0), into ST(1), then a pop.
    ArcTangent,
    /// FYL2X: ST(1) times the base-2 logarithm of ST(0), into ST(1), then a pop.
    YLogX,
    /// FYL2XP1: ST(1) times the base-2 logarithm of ST(0) plus one, into ST(1), then a pop.
    YLogXPlusOne,
}

impl Function {
    /// Whether the form reduces its argument and reports an argument out of range in C2.
    pub const fn is_trigonometric(self) -> bool {
        matches!(self, Self::Sine | Self::Cosine | Self::Tangent | Self::SineCosine)
    }

    /// Whether the form reads ST(1) and pops.
    pub const fn is_binary(self) -> bool {
        matches!(self, Self::ArcTangent | Self::YLogX | Self::YLogXPlusOne)
    }

    /// Whether the form pushes a second result.
    pub const fn pushes(self) -> bool {
        matches!(self, Self::Tangent | Self::SineCosine)
    }
}

/// One result of a transcendental form: its value, whether that value is the model's approximation, which the
/// behaviors do not compare, and whether its magnitude was rounded up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Result {
    pub value: u128,
    pub approximate: bool,
    pub up: bool,
}

/// What a transcendental form gives on operands it can read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Computed {
    /// An unmasked exception found before the computation: only the flags change.
    Stopped { flags: u16 },
    /// The argument of a trigonometric form is out of range: C2 set, nothing else changed.
    OutOfRange,
    /// The result, and for FPTAN and FSINCOS the value pushed after it; `undecided` holds the flags (UE or OE) the
    /// chip's own last bit decides, where the result lies within the error bound of the threshold.
    Done { flags: u16, first: Result, second: Option<Result>, undecided: u16 },
}

const SIGN: u128 = 1 << 79;
const INFINITY: u128 = 0x7FFF_8000_0000_0000_0000;
const ONE: u128 = 0x3FFF_8000_0000_0000_0000;
const BIAS: i32 = 16383;
/// The fractional bits of the fixed-point numbers the series work in.
const FRACTION: u32 = 124;
const FIXED_ONE: u128 = 1 << FRACTION;
/// π in the fixed point, its bits past the 124th dropped.
const PI: u128 = 0x3243_F6A8_885A_308D_3131_98A2_E037_0734;
/// The natural logarithm of 2 in the fixed point, its bits past the 124th dropped.
const LN_2: u128 = 0x0B17_217F_7D1C_F79A_BC9E_3B39_803F_2F6A;
/// The 66-bit π the trigonometric forms reduce by, times 2 to the 64.
const REDUCTION_PI: u128 = 0x3_243F_6A88_85A3_08D3;
/// Arguments of this magnitude or more are out of range for the trigonometric forms: 2 to the 63.
const TRIGONOMETRIC_LIMIT_BIASED: i32 = BIAS + 63;

/// The limit of the trigonometric forms' range, as a biased exponent.
fn find_trigonometric_limit() -> i32 {
    if is_active(Bug::TrigonometricRangeTo64) { TRIGONOMETRIC_LIMIT_BIASED + 1 } else { TRIGONOMETRIC_LIMIT_BIASED }
}

/// FPTAN's pushed 1.0.
fn find_tangent_push() -> u128 {
    if is_active(Bug::TangentPushesZero) { 0 } else { ONE }
}

/// Whether the operand class raises DE.
fn is_denormal(class: Class) -> bool {
    class == Class::Denormal && !is_active(Bug::TranscendentalDenormalIgnored)
}

/// A finite nonzero number: `significand` times two to `exponent` less 127, the significand's top bit set; `sticky`
/// where the value is an approximation or has lost bits below the significand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Real {
    negative: bool,
    exponent: i32,
    significand: u128,
    sticky: bool,
}

impl Real {
    /// A finite nonzero extended value, a denormal made normal.
    fn from_extended(value: u128) -> Self {
        let biased = ((value >> 64) & 0x7FFF) as i32;
        let significand = value as u64;
        let shift = significand.leading_zeros();
        Self {
            negative: is_negative(value),
            exponent: biased.max(1) - BIAS - shift as i32,
            significand: u128::from(significand << shift) << 64,
            sticky: false,
        }
    }

    /// A positive fixed-point value, none for zero.
    fn from_fixed(value: u128, sticky: bool) -> Option<Self> {
        (value != 0).then(|| {
            let shift = value.leading_zeros();
            Self { negative: false, exponent: 127 - shift as i32 - FRACTION as i32, significand: value << shift, sticky }
        })
    }

    /// The magnitude in the fixed point, the bits below it dropped.
    fn to_fixed(self) -> u128 {
        let shift = self.exponent + FRACTION as i32 - 127;
        assert!(shift < 0, "a fixed-point value below 8");
        if shift > -128 {
            self.significand >> -shift
        } else {
            0
        }
    }

    fn negate(self) -> Self {
        Self { negative: !self.negative, ..self }
    }

    fn multiply(self, other: Self) -> Self {
        let (high, low) = multiply_wide(self.significand, other.significand);
        let (significand, exponent, lost) = if high >> 127 != 0 {
            (high, self.exponent + other.exponent + 1, low != 0)
        } else {
            (high << 1 | low >> 127, self.exponent + other.exponent, low << 1 != 0)
        };
        Self { negative: self.negative != other.negative, exponent, significand, sticky: self.sticky || other.sticky || lost }
    }

    /// The product with a positive fixed-point value, none for zero.
    fn scale(self, factor: u128, sticky: bool) -> Option<Self> {
        Self::from_fixed(factor, sticky).map(|factor| self.multiply(factor))
    }

    fn divide(self, divisor: Self) -> Self {
        let (mut remainder, mut exponent) = (self.significand, self.exponent - divisor.exponent);
        // The quotient's first bit is set: the dividend is doubled where it is the smaller.
        let mut carry = false;
        if remainder < divisor.significand {
            carry = remainder >> 127 != 0;
            remainder <<= 1;
            exponent -= 1;
        }
        let mut quotient = 0u128;
        for _ in 0..128 {
            quotient <<= 1;
            if carry || remainder >= divisor.significand {
                remainder = remainder.wrapping_sub(divisor.significand);
                quotient |= 1;
            }
            carry = remainder >> 127 != 0;
            remainder <<= 1;
        }
        Self {
            negative: self.negative != divisor.negative,
            exponent,
            significand: quotient,
            sticky: self.sticky || divisor.sticky || remainder != 0 || carry,
        }
    }

    /// The value rounded to 64 bits under FCW's rounding, as inexact whatever its bits: the chip raises PE for every
    /// result it computes, an exact one too.
    fn round(self, control: u16) -> Outcome {
        round_approximation(self.negative, self.exponent - 127, self.significand, true, control)
    }

    /// The flags that depend on the chip's own last bit: UE where the value lies within two units in the last place of
    /// the smallest normal, OE where it does of the largest finite value; ES and B too where such a flag is unmasked.
    fn find_undecided_flags(self, control: u16) -> u16 {
        const NEAR: u128 = 1 << 66;
        let smallest = 1 - BIAS;
        let near_smallest = self.exponent == smallest && self.significand - (1 << 127) <= NEAR
            || self.exponent == smallest - 1 && self.significand.wrapping_neg() <= NEAR;
        let largest = BIAS;
        let near_largest = self.exponent == largest && self.significand.wrapping_neg() <= NEAR
            || self.exponent == largest + 1 && self.significand - (1 << 127) <= NEAR;
        let flags = if near_smallest { UNDERFLOW } else if near_largest { OVERFLOW } else { 0 };
        if flags & !control != 0 { flags | SUMMARY | BUSY } else { flags }
    }

    /// The result the chip delivers for this value: its flags, and those its own last bit decides.
    fn deliver(self, control: u16) -> (Result, u16, u16) {
        let rounded = self.round(control);
        (approximate(rounded), rounded.flags, self.find_undecided_flags(control))
    }
}

/// The 256-bit product of two 128-bit values, high half first.
fn multiply_wide(first: u128, second: u128) -> (u128, u128) {
    let mask = u128::from(u64::MAX);
    let (a, b) = (first >> 64, first & mask);
    let (c, d) = (second >> 64, second & mask);
    let (ac, ad, bc, bd) = (a * c, a * d, b * c, b * d);
    let middle = (bd >> 64) + (ad & mask) + (bc & mask);
    let low = (bd & mask) | (middle << 64);
    let high = ac + (ad >> 64) + (bc >> 64) + (middle >> 64);
    (high, low)
}

/// The product of two fixed-point values.
fn multiply_fixed(first: u128, second: u128) -> u128 {
    let (high, low) = multiply_wide(first, second);
    high << (128 - FRACTION) | low >> FRACTION
}

/// The quotient of two fixed-point values below 8.
fn divide_fixed(dividend: u128, divisor: u128) -> u128 {
    let (mut quotient, mut remainder) = (dividend / divisor, dividend % divisor);
    for _ in 0..FRACTION {
        let carry = remainder >> 127 != 0;
        remainder <<= 1;
        quotient <<= 1;
        if carry || remainder >= divisor {
            remainder = remainder.wrapping_sub(divisor);
            quotient |= 1;
        }
    }
    quotient
}

/// The sum of a series of terms that falls to zero, each made from the one before and the term's index; the terms
/// alternate in sign where `alternating`. `lost` says a nonzero square fell below the fixed point, so the sum moves
/// one unit off 1 the way the next term would.
fn sum_series(first: u128, alternating: bool, lost: bool, next: impl Fn(u128, u32) -> u128) -> u128 {
    let (mut sum, mut term, mut index) = (first as i128, first, 1);
    while term != 0 {
        term = next(term, index);
        let signed = term as i128;
        sum += if alternating && index % 2 == 1 { -signed } else { signed };
        index += 1;
    }
    if lost {
        sum += if alternating { -1 } else { 1 };
    }
    sum as u128
}

/// sin(r) / r of r², in the fixed point.
fn sine_ratio(square: u128, lost: bool) -> u128 {
    sum_series(FIXED_ONE, true, lost, |term, k| multiply_fixed(term, square) / u128::from(2 * k * (2 * k + 1)))
}

/// cos(r) of r², in the fixed point.
fn cosine(square: u128, lost: bool) -> u128 {
    sum_series(FIXED_ONE, true, lost, |term, k| multiply_fixed(term, square) / u128::from((2 * k - 1) * (2 * k)))
}

/// arctan(z) / z of z² (`alternating`), or artanh(z) / z, in the fixed point.
fn arc_ratio(square: u128, alternating: bool, lost: bool) -> u128 {
    // Each term is z to the 2k over 2k + 1: the power is kept apart from the divided term.
    let mut sum = FIXED_ONE as i128;
    let mut power = FIXED_ONE;
    let mut index = 1u32;
    loop {
        power = multiply_fixed(power, square);
        let term = (power / u128::from(2 * index + 1)) as i128;
        if term == 0 {
            break;
        }
        sum += if alternating && index % 2 == 1 { -term } else { term };
        index += 1;
    }
    if lost {
        sum += if alternating { -1 } else { 1 };
    }
    sum as u128
}

/// (e to the z less one) / z, z signed in the fixed point.
fn exponential_ratio(z: i128) -> u128 {
    let mut sum = FIXED_ONE as i128;
    let mut term = FIXED_ONE as i128;
    let mut index = 2i128;
    loop {
        let magnitude = multiply_fixed(term.unsigned_abs(), z.unsigned_abs()) as i128;
        term = if (term < 0) != (z < 0) { -magnitude } else { magnitude } / index;
        if term == 0 {
            break;
        }
        sum += term;
        index += 1;
    }
    sum as u128
}

/// The square of a value's magnitude in the fixed point, and whether a nonzero square fell below it.
fn square_of(value: Real) -> (u128, bool) {
    let fixed = value.to_fixed();
    let square = multiply_fixed(fixed, fixed);
    (square, square == 0)
}

/// arctan of a magnitude at most 1, in the fixed point.
fn arc_tangent_fixed(z: u128) -> u128 {
    let half = FIXED_ONE / 2;
    let series = |z: u128| multiply_fixed(z, arc_ratio(multiply_fixed(z, z), true, false));
    if z <= half {
        return series(z);
    }
    // arctan(z) = arctan(1/2) + arctan((z - 1/2) / (1 + z / 2)).
    let reduced = divide_fixed(z - half, FIXED_ONE + z / 2);
    series(half) + series(reduced)
}

/// arctan of a positive value at most 1.
fn arc_tangent(z: Real) -> Real {
    if z.exponent < -8 {
        let (square, lost) = square_of(z);
        return z.scale(arc_ratio(square, true, lost), true).expect("a ratio near 1");
    }
    Real::from_fixed(arc_tangent_fixed(z.to_fixed()), true).expect("a positive arctangent")
}

/// The base-2 logarithm of a finite positive value, none for 1.
fn log_2(value: Real) -> Option<Real> {
    let one = 1u128 << 127;
    // value = m times 2 to e, m in [1, 2), made [√2/2, √2) so that e is 0 about 1.
    let mut exponent = value.exponent;
    let above_root = value.significand > 0xB504_F333_F9DE_6484_597D_89B3_754A_BE9F;
    let (numerator, denominator) = if above_root {
        exponent += 1;
        // m / 2 - 1 is negative: 1 - m / 2.
        let difference = Real::from_fixed((one - value.significand / 2) >> (127 - FRACTION), false);
        (difference.map(Real::negate), FIXED_ONE + (value.significand >> (127 - FRACTION + 1)))
    } else {
        let difference = Real::from_fixed((value.significand - one) >> (127 - FRACTION), value.significand & 0x7 != 0);
        (difference, FIXED_ONE + (value.significand >> (127 - FRACTION)))
    };
    let fraction = numerator.map(|numerator| {
        // ln m = 2 artanh(s), s = (m - 1) / (m + 1); log2 m = ln m / ln 2.
        let s = numerator.divide(Real::from_fixed(denominator, false).expect("a positive sum"));
        let (square, lost) = square_of(s);
        let ln = s.scale(2 * arc_ratio(square, false, lost), true).expect("a ratio near 1");
        ln.divide(Real::from_fixed(LN_2, true).expect("ln 2"))
    });
    match (exponent, fraction) {
        (0, fraction) => fraction,
        (exponent, None) => Some(from_integer(exponent)),
        (exponent, Some(fraction)) => {
            // e plus the fraction, in a fixed point of 110 bits that holds every exponent.
            const WIDE_FRACTION: u32 = 110;
            let whole = i128::from(exponent) << WIDE_FRACTION;
            let part = (fraction.to_fixed() >> (FRACTION - WIDE_FRACTION)) as i128;
            let sum = whole + if fraction.negative { -part } else { part };
            let magnitude = sum.unsigned_abs();
            let shift = magnitude.leading_zeros();
            Some(Real {
                negative: sum < 0,
                exponent: 127 - shift as i32 - WIDE_FRACTION as i32,
                significand: magnitude << shift,
                sticky: true,
            })
        }
    }
}

/// An exact nonzero integer.
fn from_integer(value: i32) -> Real {
    let magnitude = u128::from(value.unsigned_abs());
    let shift = magnitude.leading_zeros();
    Real { negative: value < 0, exponent: 127 - shift as i32, significand: magnitude << shift, sticky: false }
}

/// The argument reduced by the nearest multiple of the 66-bit π over 2: the remainder exactly, and the multiple's
/// last two bits.
fn reduce(argument: Real) -> (Real, u32) {
    // |argument| below a half needs no reduction; above, it is A times 2 to -65 for an integer A below 2 to 128.
    if argument.exponent < -1 {
        return (argument, 0);
    }
    let significand = argument.significand >> 64;
    let shift = argument.exponent + 2;
    let scaled = significand << shift;
    let (mut multiple, remainder) = (scaled / REDUCTION_PI, scaled % REDUCTION_PI);
    if remainder * 2 >= REDUCTION_PI {
        multiple += 1;
    }
    let reduced = scaled.wrapping_sub(multiple.wrapping_mul(REDUCTION_PI)) as i128;
    let magnitude = reduced.unsigned_abs();
    let leading = magnitude.leading_zeros();
    let real = Real {
        negative: argument.negative != (reduced < 0),
        exponent: 127 - leading as i32 - 65,
        significand: magnitude << leading,
        sticky: false,
    };
    let quadrant = (multiple & 3) as u32;
    (real, if argument.negative { (4 - quadrant) % 4 } else { quadrant })
}

/// The sine and cosine of a finite nonzero argument in range.
fn sine_and_cosine(argument: Real) -> (Real, Real) {
    let (reduced, quadrant) = reduce(argument);
    let (square, lost) = square_of(reduced);
    let sine = reduced.scale(sine_ratio(square, lost), true).expect("a ratio near 1");
    let cosine = Real::from_fixed(cosine(square, lost), true).expect("a cosine near 1");
    match quadrant {
        0 => (sine, cosine),
        1 => (cosine, sine.negate()),
        2 => (sine.negate(), cosine.negate()),
        _ => (cosine.negate(), sine),
    }
}

/// An exact result.
const fn exact(value: u128) -> Result {
    Result { value, approximate: false, up: false }
}

fn approximate(outcome: Outcome) -> Result {
    Result { value: outcome.value.expect("a rounded value"), approximate: true, up: outcome.up }
}

/// The result of a form on its operands: ST(0), and ST(1) for the binary forms, neither empty.
pub fn compute(function: Function, top: u128, next: u128, control: u16) -> Computed {
    let masked = |flag: u16| control & flag != 0;
    let stop = |flag: u16, value: u128, second: Option<u128>| {
        if masked(flag) {
            Computed::Done { flags: flag, first: exact(value), second: second.map(exact), undecided: 0 }
        } else {
            Computed::Stopped { flags: flag }
        }
    };
    let classes = if function.is_binary() { [classify(top), classify(next)] } else { [classify(top); 2] };
    let pushed = |value: u128| function.pushes().then_some(value);
    if classes.contains(&Class::Unsupported) {
        return stop(INVALID, INDEFINITE, pushed(INDEFINITE));
    }
    if classes.contains(&Class::SignalingNaN) {
        let nan = if function.is_binary() { propagate(top, next) } else { quiet(top) };
        return stop(INVALID, nan, pushed(nan));
    }
    if classes.contains(&Class::QuietNaN) {
        let nan = if function.is_binary() { propagate(top, next) } else { top };
        return Computed::Done { flags: 0, first: exact(nan), second: pushed(nan).map(exact), undecided: 0 };
    }
    if function.is_binary() {
        compute_binary(function, next, top, control)
    } else {
        compute_unary(function, top, control)
    }
}

fn compute_unary(function: Function, value: u128, control: u16) -> Computed {
    let masked = |flag: u16| control & flag != 0;
    let class = classify(value);
    let negative = is_negative(value);
    let signed_one = ONE | if negative { SIGN } else { 0 };
    if class == Class::Infinity {
        let result = match function {
            // 2 to -∞ less one is -1; 2 to +∞ less one is +∞.
            Function::TwoToXMinusOne if negative => signed_one,
            Function::TwoToXMinusOne => value,
            _ => {
                let pushed = function.pushes().then_some(exact(INDEFINITE));
                return if masked(INVALID) {
                    Computed::Done { flags: INVALID, first: exact(INDEFINITE), second: pushed, undecided: 0 }
                } else {
                    Computed::Stopped { flags: INVALID }
                };
            }
        };
        return Computed::Done { flags: 0, first: exact(result), second: None, undecided: 0 };
    }
    if function.is_trigonometric() && ((value >> 64) & 0x7FFF) as i32 >= find_trigonometric_limit() {
        return Computed::OutOfRange;
    }
    let mut flags = 0;
    if is_denormal(class) {
        if !masked(DENORMAL) {
            return Computed::Stopped { flags: DENORMAL };
        }
        flags |= DENORMAL;
    }
    if class == Class::Zero {
        let (first, second) = match function {
            Function::Cosine => (ONE, None),
            Function::Tangent => (value, Some(find_tangent_push())),
            Function::SineCosine if is_active(Bug::SineCosineOrderSwapped) => (ONE, Some(value)),
            Function::SineCosine => (value, Some(ONE)),
            _ => (value, None),
        };
        return Computed::Done { flags, first: exact(first), second: second.map(exact), undecided: 0 };
    }
    let argument = Real::from_extended(value);
    let finish = |flags: u16, first: Real, second: Option<Result>| {
        let (first, rounded, undecided) = first.deliver(control);
        Computed::Done { flags: flags | rounded, first, second, undecided }
    };
    match function {
        Function::TwoToXMinusOne => {
            // Beyond ±1, where the manual leaves the result undefined, the chip leaves the operand and raises PE.
            if argument.exponent >= 0 && value & !SIGN != ONE {
                let first = Result { value, approximate: true, up: false };
                let precision = if is_active(Bug::TwoToXOutOfRangeExact) { 0 } else { PRECISION };
                return Computed::Done { flags: flags | precision, first, second: None, undecided: 0 };
            }
            finish(flags, two_to_x_minus_one(argument), None)
        }
        Function::Sine => finish(flags, sine_and_cosine(argument).0, None),
        Function::Cosine => finish(flags, sine_and_cosine(argument).1, None),
        Function::Tangent => {
            let (sine, cosine) = sine_and_cosine(argument);
            finish(flags, sine.divide(cosine), Some(exact(find_tangent_push())))
        }
        Function::SineCosine => {
            let (sine, cosine) = sine_and_cosine(argument);
            let (sine, sine_flags, sine_undecided) = sine.deliver(control);
            let (cosine, cosine_flags, cosine_undecided) = cosine.deliver(control);
            let (first, second) = if is_active(Bug::SineCosineOrderSwapped) { (cosine, sine) } else { (sine, cosine) };
            Computed::Done {
                flags: flags | sine_flags | cosine_flags,
                first,
                second: Some(second),
                undecided: sine_undecided | cosine_undecided,
            }
        }
        Function::ArcTangent | Function::YLogX | Function::YLogXPlusOne => unreachable!("a binary form"),
    }
}

/// The binary forms of y, ST(1), and x, ST(0).
fn compute_binary(function: Function, y: u128, x: u128, control: u16) -> Computed {
    let masked = |flag: u16| control & flag != 0;
    let (y_class, x_class) = (classify(y), classify(x));
    let (y_negative, x_negative) = (is_negative(y), is_negative(x));
    let signed = |negative: bool, value: u128| value | if negative { SIGN } else { 0 };
    let zero = |negative: bool| signed(negative, 0);
    let infinity = |negative: bool| signed(negative, INFINITY);
    let invalid = || if masked(INVALID) {
        Computed::Done { flags: INVALID, first: exact(INDEFINITE), second: None, undecided: 0 }
    } else {
        Computed::Stopped { flags: INVALID }
    };
    let done = |flags: u16, value: u128| Computed::Done { flags, first: exact(value), second: None, undecided: 0 };
    let deliver = |flags: u16, value: Real| {
        let (first, rounded, undecided) = value.deliver(control);
        Computed::Done { flags: flags | rounded, first, second: None, undecided }
    };
    let infinite = |class: Class| class == Class::Infinity;
    let is_zero = |class: Class| class == Class::Zero;
    // The special cases of the manual's tables, before the denormal operand.
    match function {
        Function::ArcTangent => {
            let pi_fraction = |numerator: u128, denominator: u128| {
                let fixed = PI * numerator / denominator;
                let real = Real::from_fixed(fixed, true).expect("a positive multiple of π");
                Real { negative: y_negative, ..real }
            };
            // A zero y gives ±0 against a positive x (+0 among them) and ±π against a negative one; an infinite y
            // gives ±π/4 or ±3π/4 against an infinite x and ±π/2 otherwise, as a zero x does; an infinite x gives ±0
            // or ±π by its sign.
            let special = if is_zero(y_class) {
                let to_pi = x_negative && !(is_zero(x_class) && is_active(Bug::ArcTangentZeroOverNegativeZero));
                Some(if to_pi { Ok(pi_fraction(1, 1)) } else { Err(zero(y_negative)) })
            } else if infinite(y_class) && infinite(x_class) {
                Some(Ok(pi_fraction(if x_negative { 3 } else { 1 }, 4)))
            } else if infinite(y_class) || is_zero(x_class) {
                Some(Ok(pi_fraction(1, 2)))
            } else if infinite(x_class) {
                Some(if x_negative { Ok(pi_fraction(1, 1)) } else { Err(zero(y_negative)) })
            } else {
                None
            };
            let denormal = is_denormal(y_class) || is_denormal(x_class);
            let mut flags = 0;
            if denormal {
                if !masked(DENORMAL) {
                    return Computed::Stopped { flags: DENORMAL };
                }
                flags |= DENORMAL;
            }
            let result = match special {
                Some(Err(value)) => return done(flags, value),
                Some(Ok(real)) => real,
                None => arc_tangent_of(Real::from_extended(y), Real::from_extended(x)),
            };
            deliver(flags, result)
        }
        Function::YLogX => {
            // log2 of a negative value (not -0) is invalid; of ±0 it is -∞, with ZE where y is finite and nonzero.
            if x_negative && !is_zero(x_class) {
                return invalid();
            }
            if is_zero(x_class) {
                return match y_class {
                    Class::Zero => invalid(),
                    Class::Infinity => done(0, infinity(!y_negative)),
                    // ZE comes before a denormal y, which raises no DE.
                    _ if is_active(Bug::LogOfZeroNoDivideByZero) => done(0, infinity(!y_negative)),
                    _ if masked(ZERO_DIVIDE) => done(ZERO_DIVIDE, infinity(!y_negative)),
                    _ => Computed::Stopped { flags: ZERO_DIVIDE },
                };
            }
            if infinite(x_class) {
                return match y_class {
                    Class::Zero => invalid(),
                    _ if is_denormal(y_class) && !masked(DENORMAL) => Computed::Stopped { flags: DENORMAL },
                    Class::Denormal => done(DENORMAL, infinity(y_negative)),
                    _ => done(0, infinity(y_negative)),
                };
            }
            let x_real = Real::from_extended(x);
            let logarithm = log_2(x_real);
            if infinite(y_class) {
                return match logarithm {
                    None => invalid(),
                    Some(logarithm) => {
                        if is_denormal(x_class) && !masked(DENORMAL) {
                            return Computed::Stopped { flags: DENORMAL };
                        }
                        let flags = if is_denormal(x_class) { DENORMAL } else { 0 };
                        done(flags, infinity(y_negative != logarithm.negative))
                    }
                };
            }
            let mut flags = 0;
            if is_denormal(x_class) || is_denormal(y_class) {
                if !masked(DENORMAL) {
                    return Computed::Stopped { flags: DENORMAL };
                }
                flags |= DENORMAL;
            }
            let Some(logarithm) = logarithm else {
                return done(flags, zero(y_negative));
            };
            if is_zero(y_class) {
                return done(flags, zero(y_negative != logarithm.negative));
            }
            deliver(flags, Real::from_extended(y).multiply(logarithm))
        }
        Function::YLogXPlusOne => {
            if infinite(x_class) {
                return match y_class {
                    _ if x_negative || is_zero(y_class) => invalid(),
                    Class::Denormal if !masked(DENORMAL) => Computed::Stopped { flags: DENORMAL },
                    Class::Denormal => done(DENORMAL, infinity(y_negative)),
                    _ => done(0, infinity(y_negative)),
                };
            }
            if is_zero(x_class) {
                if infinite(y_class) {
                    return invalid();
                }
                if is_denormal(y_class) && !masked(DENORMAL) {
                    return Computed::Stopped { flags: DENORMAL };
                }
                let flags = if is_denormal(y_class) { DENORMAL } else { 0 };
                return done(flags, zero(y_negative != x_negative));
            }
            let x_real = Real::from_extended(x);
            // At -1 and below, outside the range the manual defines, the chip leaves x as the result and raises PE.
            let at_most_minus_one = x_negative && x_real.exponent >= 0;
            let mut flags = 0;
            if is_denormal(x_class) || is_denormal(y_class) {
                if !masked(DENORMAL) {
                    return Computed::Stopped { flags: DENORMAL };
                }
                flags |= DENORMAL;
            }
            if infinite(y_class) {
                return done(flags, infinity(y_negative != x_negative));
            }
            if is_zero(y_class) {
                return done(flags, zero(y_negative != x_negative));
            }
            if at_most_minus_one && is_active(Bug::LogOnePlusBelowMinusOneInvalid) {
                return invalid();
            }
            if at_most_minus_one {
                let first = Result { value: x, approximate: true, up: false };
                return Computed::Done { flags: flags | PRECISION, first, second: None, undecided: 0 };
            }
            let logarithm = log_2_of_one_plus(x_real);
            deliver(flags, Real::from_extended(y).multiply(logarithm))
        }
        _ => unreachable!("a unary form"),
    }
}

/// 2 to a magnitude below 1, less one: e to the z less one for z = x ln 2.
fn two_to_fraction_minus_one(argument: Real) -> Real {
    let z = argument.multiply(Real::from_fixed(LN_2, true).expect("ln 2"));
    let fixed = z.to_fixed() as i128;
    z.scale(exponential_ratio(if z.negative { -fixed } else { fixed }), true).expect("a positive ratio")
}

/// 2 to a finite nonzero x, less one. The manual defines it for x from -1 to 1; beyond, the model goes on with the
/// same function, as 2 to the integer part times 2 to the rest.
fn two_to_x_minus_one(argument: Real) -> Real {
    /// Beyond 2 to this, 2 to x overflows every format, or 2 to -x less one is -1 within the last bit.
    const LARGEST_EXPONENT: i32 = 14;
    if argument.exponent < 0 {
        return two_to_fraction_minus_one(argument);
    }
    if argument.exponent >= LARGEST_EXPONENT {
        return if argument.negative {
            Real { negative: true, exponent: -1, significand: u128::MAX, sticky: true }
        } else {
            Real { negative: false, exponent: 1 << 20, significand: 1 << 127, sticky: true }
        };
    }
    let shift = 127 - argument.exponent as u32;
    let whole = (argument.significand >> shift) as i32;
    let fraction_bits = argument.significand & ((1 << shift) - 1);
    let power = if argument.negative { -whole } else { whole };
    // 2 to the power, less one, exactly: 2^n - 1, or -(1 - 2^-n).
    let one = Real { negative: false, exponent: 0, significand: 1 << 127, sticky: false };
    let scaled_one = Real { exponent: power, ..one };
    let power_less_one = add(scaled_one, one.negate());
    // The fraction's bits lie at the argument's scale: read as the fixed point, they are 2 to the exponent too small.
    let Some(fraction) = Real::from_fixed(fraction_bits >> (127 - FRACTION), fraction_bits & 0x7 != 0)
        .map(|fraction| Real { negative: argument.negative, exponent: fraction.exponent + argument.exponent, ..fraction })
    else {
        return power_less_one.expect("an integer power other than 0");
    };
    // 2^x - 1 = 2^n (2^f - 1) + (2^n - 1).
    let part = two_to_fraction_minus_one(fraction);
    let part = Real { exponent: part.exponent + power, ..part };
    match power_less_one {
        Some(power_less_one) => add(part, power_less_one).expect("a nonzero sum"),
        None => part,
    }
}

/// The base-2 logarithm of 1 + x for a finite nonzero x above -1.
fn log_2_of_one_plus(x: Real) -> Real {
    if x.exponent >= -2 {
        // Far from zero, 1 + x is formed as it is.
        let one = Real { negative: false, exponent: 0, significand: 1 << 127, sticky: false };
        let sum = add(one, x).expect("1 + x is positive for x above -1");
        return log_2(sum).expect("1 + x is not 1 for x far from zero");
    }
    let fixed = x.to_fixed();
    let denominator = if x.negative { 2 * FIXED_ONE - fixed } else { 2 * FIXED_ONE + fixed };
    let s = x.divide(Real::from_fixed(denominator, true).expect("a positive sum"));
    let (square, lost) = square_of(s);
    let ln = s.scale(2 * arc_ratio(square, false, lost), true).expect("a ratio near 1");
    ln.divide(Real::from_fixed(LN_2, true).expect("ln 2"))
}

/// The sum of two values, none where it is zero.
fn add(first: Real, second: Real) -> Option<Real> {
    let (big, small) = if (first.exponent, first.significand) >= (second.exponent, second.significand) {
        (first, second)
    } else {
        (second, first)
    };
    // Two bits of headroom: each significand a quarter, the smaller's moved to the larger's exponent.
    let distance = (big.exponent - small.exponent) as u32;
    let quarter = |value: u128| (value >> 2, value & 3 != 0);
    let (large, large_lost) = quarter(big.significand);
    let (small_quarter, small_lost) = quarter(small.significand);
    let (moved, moved_lost) = if distance >= 128 {
        (0, small_quarter != 0)
    } else {
        (small_quarter >> distance, small_quarter & ((1 << distance) - 1) != 0)
    };
    let lost = large_lost || small_lost || moved_lost;
    let sum = if big.negative == small.negative { large + moved } else { large - moved - u128::from(lost) };
    (sum != 0).then(|| {
        let shift = sum.leading_zeros();
        Real {
            negative: big.negative,
            exponent: big.exponent + 2 - shift as i32,
            significand: sum << shift,
            sticky: big.sticky || small.sticky || lost,
        }
    })
}

/// The arctangent of y over x, both finite and nonzero, in the quadrant their signs give.
fn arc_tangent_of(y: Real, x: Real) -> Real {
    let (y_magnitude, x_magnitude) = (Real { negative: false, ..y }, Real { negative: false, ..x });
    let y_smaller = (y.exponent, y.significand) <= (x.exponent, x.significand);
    let angle = if y_smaller {
        let z = y_magnitude.divide(x_magnitude);
        if !x.negative {
            arc_tangent(z)
        } else {
            let fixed = PI - arc_tangent(z).to_fixed();
            Real::from_fixed(fixed, true).expect("an angle above π/2")
        }
    } else {
        let z = x_magnitude.divide(y_magnitude);
        let right = PI / 2;
        let fixed = if x.negative { right + arc_tangent(z).to_fixed() } else { right - arc_tangent(z).to_fixed() };
        Real::from_fixed(fixed, true).expect("an angle above π/4")
    };
    Real { negative: y.negative, ..angle }
}

/// The high-precision value the model rounds, as a significand of 128 bits, its exponent and its sign, for the test
/// that bounds a chip's error: none where the result is exact or special.
pub fn find_unrounded(function: Function, top: u128, next: u128) -> Option<(bool, i32, u128)> {
    let finite = |value: u128| matches!(classify(value), Class::Normal | Class::Denormal);
    match function {
        _ if !finite(top) || function.is_binary() && !finite(next) => None,
        Function::Sine | Function::Cosine | Function::Tangent | Function::SineCosine => {
            if ((top >> 64) & 0x7FFF) as i32 >= TRIGONOMETRIC_LIMIT_BIASED {
                return None;
            }
            let (sine, cosine) = sine_and_cosine(Real::from_extended(top));
            let value = match function {
                Function::Sine | Function::SineCosine => sine,
                Function::Cosine => cosine,
                _ => sine.divide(cosine),
            };
            Some((value.negative, value.exponent - 127, value.significand))
        }
        Function::TwoToXMinusOne => {
            let argument = Real::from_extended(top);
            if argument.exponent >= 0 {
                return None;
            }
            let value = two_to_fraction_minus_one(argument);
            Some((value.negative, value.exponent - 127, value.significand))
        }
        Function::ArcTangent => {
            let value = arc_tangent_of(Real::from_extended(next), Real::from_extended(top));
            Some((value.negative, value.exponent - 127, value.significand))
        }
        Function::YLogX => {
            if is_negative(top) {
                return None;
            }
            let logarithm = log_2(Real::from_extended(top))?;
            let value = Real::from_extended(next).multiply(logarithm);
            Some((value.negative, value.exponent - 127, value.significand))
        }
        Function::YLogXPlusOne => {
            // The range the manual defines: |x| below 1 - √2/2.
            if top & !SIGN >= 0x3FFD_95F6_1998_0C43_36F8 {
                return None;
            }
            let x = Real::from_extended(top);
            let value = Real::from_extended(next).multiply(log_2_of_one_plus(x));
            Some((value.negative, value.exponent - 127, value.significand))
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    /// arctan(1/k) by its series, in the fixed point.
    fn arc_tangent_of_reciprocal(k: u128) -> u128 {
        let z = FIXED_ONE / k;
        multiply_fixed(z, arc_ratio(multiply_fixed(z, z), true, false))
    }

    #[test]
    fn constants_agree_with_their_series() {
        // Machin: π = 16 arctan(1/5) - 4 arctan(1/239).
        let pi = 16 * arc_tangent_of_reciprocal(5) - 4 * arc_tangent_of_reciprocal(239);
        assert!(pi.abs_diff(PI) < 1 << 8, "{pi:#x} against {PI:#x}");
        // ln 2 = 2 artanh(1/3).
        let third = FIXED_ONE / 3;
        let ln_2 = 2 * multiply_fixed(third, arc_ratio(multiply_fixed(third, third), false, false));
        assert!(ln_2.abs_diff(LN_2) < 1 << 8, "{ln_2:#x} against {LN_2:#x}");
        assert_eq!(REDUCTION_PI, PI >> 60);
    }

    fn to_f64(value: u128) -> f64 {
        let negative = is_negative(value);
        let exponent = ((value >> 64) & 0x7FFF) as i32 - BIAS;
        let magnitude = (value as u64) as f64 / (1u64 << 63) as f64 * 2f64.powi(exponent);
        if negative { -magnitude } else { magnitude }
    }

    fn from_f64(value: f64) -> u128 {
        let bits = value.to_bits();
        crate::x87::widen(bits, 52, 11).0
    }

    #[test]
    fn results_are_near_the_double_library() {
        let cases: [(Function, f64, f64, fn(f64, f64) -> f64); 8] = [
            (Function::Sine, 0.7, 0.0, |x, _| x.sin()),
            (Function::Cosine, 2.5, 0.0, |x, _| x.cos()),
            (Function::Tangent, -10.0, 0.0, |x, _| x.tan()),
            (Function::SineCosine, 1000.0, 0.0, |x, _| x.sin()),
            (Function::TwoToXMinusOne, -0.3, 0.0, |x, _| x.exp2() - 1.0),
            (Function::ArcTangent, -3.0, 2.0, |x, y| y.atan2(x)),
            (Function::YLogX, 10.0, -3.0, |x, y| y * x.log2()),
            (Function::YLogXPlusOne, 0.01, 5.0, |x, y| y * x.ln_1p() / core::f64::consts::LN_2),
        ];
        for (function, x, y, reference) in cases {
            let Computed::Done { first, .. } = compute(function, from_f64(x), from_f64(y), 0x037F) else {
                panic!("{function:?} computes");
            };
            let got = to_f64(first.value);
            let want = reference(x, y);
            assert!((got - want).abs() <= want.abs() * 1e-14, "{function:?}({x}, {y}): {got} against {want}");
        }
    }
}
