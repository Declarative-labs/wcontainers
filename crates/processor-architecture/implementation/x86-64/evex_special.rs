//! The AVX-512 operations on one floating-point number's parts: VGETEXP, VGETMANT, VFPCLASS, VRNDSCALE, VREDUCE,
//! VSCALEF and VRANGE, as the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! volume 2 describes them, for single, double and half precision.

use crate::float::{
    Class, DENORMAL, DENORMALS_ARE_ZEROS, DIVIDE_BY_ZERO, Exact, INEXACT, INVALID, LaneOutcome, MASK_SHIFT, Operand, Precision, Rounding,
    UNDERFLOW, compare_less, read_rounding, round, round_to_integer,
};
use crate::planted::{Bug, is_active};

/// What a form computes from an element (VSCALEF: from two) and the immediate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpecialOperation {
    /// The unbiased exponent, as a floating-point number: floor(log2(|x|)).
    GetExponent,
    /// The significand in the interval the immediate's bits 1:0 name, signed as its bits 3:2 say.
    GetMantissa,
    /// One where the element is of a class the immediate's bits select, as an opmask bit.
    Class,
    /// The element rounded to a multiple of 2^-M, M the immediate's bits 7:4.
    RoundScale,
    /// The element less its rounding to a multiple of 2^-M.
    Reduce,
    /// The first element times 2 to the floor of the second.
    Scale,
    /// The lesser or greater of the two elements, or of their magnitudes (the immediate's bits 1:0), signed as its
    /// bits 3:2 say.
    Range,
    /// VFIXUPIMM: the first element's class picks a 4-bit response from the second element, which fixes the result
    /// up (the destination kept, the element, a NaN, an infinity, a zero, one or a constant); the immediate's bits
    /// raise divide-by-zero or invalid for classes.
    Fixup,
}

impl SpecialOperation {
    /// Whether the operation reads two elements.
    pub const fn is_binary(self) -> bool {
        matches!(self, Self::Scale | Self::Range | Self::Fixup)
    }
}

/// The parts of a precision's encoding this module builds numbers from.
struct Layout {
    fraction_bits: u32,
    bias: i32,
    sign: u64,
}

impl Layout {
    const fn of(precision: Precision) -> Self {
        let digits = precision.to_significand_bits();
        Self {
            fraction_bits: digits - 1,
            bias: (1 << (precision.to_bits() - digits - 1)) - 1,
            sign: 1 << (precision.to_bits() - 1),
        }
    }

    /// The normal number 2^exponent times one and the fraction.
    const fn normal(&self, negative: bool, exponent: i32, fraction: u64) -> u64 {
        let sign = if negative { self.sign } else { 0 };
        sign | (((exponent + self.bias) as u64) << self.fraction_bits) | fraction
    }
}

/// Computes one element under MXCSR (its rounding, DAZ and masks; an embedded control already applied).
pub(crate) fn compute_special(
    operation: SpecialOperation,
    destination: u64,
    first: u64,
    second: u64,
    precision: Precision,
    mxcsr: u32,
    immediate: u8,
) -> LaneOutcome {
    let daz = mxcsr & DENORMALS_ARE_ZEROS != 0;
    let x = Operand::read(second, precision, daz);
    let layout = Layout::of(precision);
    let done = |bits: u64, before: u32| LaneOutcome { bits, before, after: 0 };
    let quiet = |operand: Operand| {
        let signaling = matches!(operand.class, Class::Nan { signaling: true });
        done(operand.bits | precision.to_quiet_bit(), if signaling { INVALID } else { 0 })
    };
    let denormal = if x.is_denormal() { DENORMAL } else { 0 };
    match operation {
        SpecialOperation::Class => done(u64::from(classify(x, immediate)), 0),
        SpecialOperation::GetExponent => match x.class {
            Class::Nan { .. } => quiet(x),
            Class::Infinity => done(precision.to_infinity(), 0),
            Class::Zero => done(layout.sign | precision.to_infinity(), 0),
            Class::Finite { significand, exponent, .. } => {
                let unbiased = exponent + (u128::BITS - significand.leading_zeros()) as i32 - 1;
                let bits = if unbiased == 0 {
                    0
                } else {
                    let exact = Exact { negative: unbiased < 0, significand: u128::from(unbiased.unsigned_abs()), exponent: 0, sticky: false };
                    round(exact, precision, Rounding::Nearest, mxcsr).0
                };
                done(bits, denormal)
            }
        },
        SpecialOperation::GetMantissa => {
            let (interval, sign_control) = (immediate & 3, (immediate >> 2) & 3);
            let signed_one = layout.normal(sign_control & 1 == 0, 0, 0);
            let one = layout.normal(false, 0, 0);
            let indefinite = done(precision.to_default_nan(), INVALID);
            match x.class {
                Class::Nan { .. } => quiet(x),
                Class::Zero | Class::Infinity if !x.negative => done(one, 0),
                Class::Zero => done(signed_one, 0),
                Class::Infinity if sign_control & 2 != 0 => indefinite,
                Class::Infinity => done(signed_one, 0),
                Class::Finite { .. } if x.negative && sign_control & 2 != 0 => indefinite,
                Class::Finite { significand, exponent, .. } => {
                    let width = u128::BITS - significand.leading_zeros();
                    let normalized = (significand << (layout.fraction_bits + 1 - width)) as u64;
                    let fraction = normalized & ((1 << layout.fraction_bits) - 1);
                    let unbiased = exponent + width as i32 - 1;
                    let top = fraction >> (layout.fraction_bits - 1) != 0;
                    let below_one = match interval {
                        0 => false,
                        1 => unbiased & 1 != 0,
                        2 => true,
                        _ => top != is_active(Bug::MantissaIntervalThreeInverted),
                    };
                    let negative = x.negative && sign_control & 1 == 0;
                    done(layout.normal(negative, -i32::from(below_one), fraction), denormal)
                }
            }
        }
        SpecialOperation::RoundScale | SpecialOperation::Reduce => {
            let scale = i32::from(immediate >> 4);
            let rounding = if immediate & 4 != 0 { read_rounding(mxcsr) } else { read_rounding(u32::from(immediate & 3) << 13) };
            let suppressed = immediate & 8 != 0 && !is_active(Bug::RoundScaleSuppressionIgnored);
            let reduce = operation == SpecialOperation::Reduce;
            let zero = |negative: bool| if negative { layout.sign } else { 0 };
            match x.class {
                Class::Nan { .. } => quiet(x),
                Class::Infinity if reduce => done(0, 0),
                Class::Infinity => done(x.bits, 0),
                // x less itself: +0, or -0 rounding down.
                Class::Zero if reduce => done(zero(rounding == Rounding::Down), 0),
                Class::Zero => done(zero(x.negative), 0),
                Class::Finite { exponent, .. } if exponent + scale >= 0 => {
                    done(if reduce { zero(rounding == Rounding::Down) } else { x.bits }, 0)
                }
                Class::Finite { significand, exponent, .. } => {
                    let (multiple, lost) = round_to_integer(significand, exponent + scale, rounding, x.negative);
                    let flag = if lost && !suppressed { INEXACT } else { 0 };
                    if !reduce {
                        if multiple == 0 {
                            return LaneOutcome { bits: zero(x.negative), before: 0, after: flag };
                        }
                        let exact = Exact { negative: x.negative, significand: multiple, exponent: -scale, sticky: false };
                        let bits = round(exact, precision, Rounding::Nearest, mxcsr).0;
                        // A nonzero result below the smallest normal number, with bits lost, underflows.
                        let tiny = bits & !layout.sign < 1 << layout.fraction_bits;
                        let underflow = if tiny && lost { UNDERFLOW } else { 0 };
                        return LaneOutcome { bits, before: 0, after: flag | underflow };
                    }
                    // x less the multiple, both at x's exponent, which lies below -M; where x is far below the
                    // multiple's unit, the multiple less a unit 64 bits below it, and x's bits only sticky.
                    let shift = (-scale - exponent) as u32;
                    let exact = if multiple == 0 {
                        Exact { negative: x.negative, significand, exponent, sticky: false }
                    } else if shift + (u128::BITS - multiple.leading_zeros()) < u128::BITS - 1 {
                        let subtracted = multiple << shift;
                        let (negative, magnitude) = if significand >= subtracted {
                            (x.negative, significand - subtracted)
                        } else {
                            (!x.negative, subtracted - significand)
                        };
                        if magnitude == 0 {
                            return LaneOutcome { bits: zero(rounding == Rounding::Down), before: 0, after: flag };
                        }
                        Exact { negative, significand: magnitude, exponent, sticky: false }
                    } else {
                        Exact { negative: !x.negative, significand: (multiple << 64) - 1, exponent: -scale - 64, sticky: true }
                    };
                    // The difference reports inexact alone, never underflow, as the chip shows.
                    let (bits, after) = round(exact, precision, rounding, mxcsr | (UNDERFLOW << MASK_SHIFT));
                    LaneOutcome { bits, before: 0, after: if suppressed { 0 } else { after & INEXACT } }
                }
            }
        }
        SpecialOperation::Scale => scale(Operand::read(first, precision, daz), x, precision, mxcsr),
        SpecialOperation::Range => range(Operand::read(first, precision, daz), x, precision, immediate),
        SpecialOperation::Fixup => fix_up(destination, Operand::read(first, precision, daz), second, precision, immediate),
    }
}

/// VFIXUPIMM of one element: the token of the first element's class (a denormal under DAZ is +0) picks the response
/// from the table's low 32 bits.
fn fix_up(destination: u64, value: Operand, table: u64, precision: Precision, immediate: u8) -> LaneOutcome {
    let layout = Layout::of(precision);
    // A denormal DAZ reads as zero becomes +0, its sign dropped.
    let flushed = value.class == Class::Zero && value.bits & !layout.sign != 0;
    let value_bits = if flushed { 0 } else { value.bits };
    let negative = value.negative && !flushed;
    let one = layout.normal(false, 0, 0);
    let token = match value.class {
        Class::Nan { signaling: false } => 0,
        Class::Nan { signaling: true } => 1,
        Class::Zero => 2,
        Class::Finite { .. } if value.bits == one || (is_active(Bug::FixupOneIgnoresSign) && value.bits == one | layout.sign) => 3,
        Class::Infinity if negative => 4,
        Class::Infinity => 5,
        _ if negative => 6,
        _ => 7,
    };
    let response = (table >> (4 * token)) & 0xF;
    let largest = precision.to_infinity() - 1;
    let constant = |single: u64, double: u64, half: u64| match precision {
        Precision::Single => single,
        Precision::Double => double,
        Precision::Half => half,
    };
    let bits = match response {
        0 => destination,
        1 => value_bits,
        // The element with every exponent bit and the quiet bit set.
        2 => value_bits | precision.to_infinity() | precision.to_quiet_bit(),
        3 => precision.to_default_nan(),
        4 => layout.sign | precision.to_infinity(),
        5 => precision.to_infinity(),
        6 if negative => layout.sign | precision.to_infinity(),
        6 => precision.to_infinity(),
        7 => layout.sign,
        8 => 0,
        9 => layout.normal(true, 0, 0),
        10 => one,
        11 => layout.normal(false, -1, 0),
        12 => constant(0x42B4_0000, 0x4056_8000_0000_0000, 0x55A0),
        13 => constant(0x3FC9_0FDB, 0x3FF9_21FB_5444_2D18, 0x3E48),
        14 => largest,
        _ => layout.sign | largest,
    };
    // The immediate's bits by class: zero (#ZE, #IE), one (#ZE, #IE), signaling NaN, -infinity, negative, +infinity.
    let (zero_divide, invalid) = match token {
        2 => (immediate & 1 != 0, immediate & 2 != 0),
        3 => (immediate & 4 != 0, immediate & 8 != 0),
        1 => (false, immediate & 0x10 != 0),
        4 => (false, immediate & 0x20 != 0),
        6 => (false, immediate & 0x40 != 0),
        5 => (false, immediate & 0x80 != 0),
        _ => (false, false),
    };
    let flags = if invalid { INVALID } else { 0 } | if zero_divide { DIVIDE_BY_ZERO } else { 0 };
    LaneOutcome { bits, before: flags, after: 0 }
}

/// VRANGE of one element pair: a signaling NaN quieted (the first's first) with invalid, else a quiet NaN as it is,
/// else the selected element (a denormal read as zero under DAZ) with the sign the immediate's bits 3:2 give.
fn range(first: Operand, second: Operand, precision: Precision, immediate: u8) -> LaneOutcome {
    let done = |bits: u64, before: u32| LaneOutcome { bits, before, after: 0 };
    let quiet = precision.to_quiet_bit();
    for operand in [first, second] {
        if matches!(operand.class, Class::Nan { signaling: true }) {
            return done(operand.bits | quiet, INVALID);
        }
    }
    // A quiet NaN gives way to the other element, which is reported denormal only when neither is a NaN.
    // Sign control 0 takes the first source's sign, a NaN's included.
    let first_sign = first.bits & (1u64 << (precision.to_bits() - 1));
    let (first, second, denormal) = match (first.is_nan(), second.is_nan()) {
        (true, true) => (first, first, 0),
        (true, false) => (second, second, 0),
        (false, true) => (first, first, 0),
        (false, false) => (first, second, if first.is_denormal() || second.is_denormal() { DENORMAL } else { 0 }),
    };
    let sign = 1u64 << (precision.to_bits() - 1);
    // An operand as DAZ reads it: a denormal is a zero of its sign.
    let as_read = |operand: Operand| if operand.class == Class::Zero { operand.bits & sign } else { operand.bits };
    let magnitude = |operand: Operand| Operand { negative: false, ..operand };
    let (absolute, greater) = (immediate & 2 != 0, immediate & 1 != 0);
    let (a, b) = if absolute { (magnitude(first), magnitude(second)) } else { (first, second) };
    let less = compare_less(a, b);
    let equal = !less && !compare_less(b, a);
    // Equal values of opposite signs: the lesser is the negative one.
    let first_lesser = if equal && first.negative != second.negative { first.negative } else { less || equal };
    let chosen = if first_lesser != greater { first } else { second };
    let selected = as_read(chosen);
    let bits = match (immediate >> 2) & 3 {
        0 if is_active(Bug::RangeSignFromSelected) => selected,
        0 => (selected & !sign) | first_sign,
        1 => selected,
        2 => selected & !sign,
        _ => selected | sign,
    };
    done(bits, denormal)
}

/// VFPCLASS's test: whether the element is of a class the immediate's set bits name, in their order quiet NaN, +0,
/// -0, +infinity, -infinity, denormal, negative finite, signaling NaN.
fn classify(x: Operand, immediate: u8) -> bool {
    let classes = [
        matches!(x.class, Class::Nan { signaling: false }),
        x.class == Class::Zero && !x.negative,
        x.class == Class::Zero && x.negative,
        x.class == Class::Infinity && !x.negative,
        x.class == Class::Infinity && x.negative,
        x.is_denormal(),
        x.negative && (matches!(x.class, Class::Finite { .. }) || (is_active(Bug::ClassNegativeFiniteIncludesZero) && x.class == Class::Zero)),
        matches!(x.class, Class::Nan { signaling: true }),
    ];
    classes.iter().enumerate().any(|(bit, &holds)| holds && immediate & (1 << bit) != 0)
}

/// VSCALEF of one element: the first times 2 to the floor of the second, rounded under MXCSR, with the manual's
/// special cases for NaNs, infinities and zeros.
fn scale(first: Operand, second: Operand, precision: Precision, mxcsr: u32) -> LaneOutcome {
    let layout = Layout::of(precision);
    let done = |bits: u64, before: u32| LaneOutcome { bits, before, after: 0 };
    let signaling = |operand: Operand| matches!(operand.class, Class::Nan { signaling: true });
    let invalid_nan = if signaling(first) || signaling(second) { INVALID } else { 0 };
    let sign = |negative: bool| if negative { layout.sign } else { 0 };
    let denormal = if first.is_denormal() { DENORMAL } else { 0 };
    if first.is_nan() {
        // A quiet NaN scaled by an infinity gives +infinity or +0.
        return match (first.class, second.class) {
            (Class::Nan { signaling: false }, Class::Infinity) if !second.negative => done(precision.to_infinity(), 0),
            (Class::Nan { signaling: false }, Class::Infinity) => done(0, 0),
            _ => done(first.bits | precision.to_quiet_bit(), invalid_nan),
        };
    }
    if second.is_nan() {
        return done(second.bits | precision.to_quiet_bit(), invalid_nan);
    }
    let indefinite = done(precision.to_default_nan(), INVALID);
    match (first.class, second.class) {
        (Class::Infinity, Class::Infinity) if second.negative => indefinite,
        (Class::Infinity, _) => done(first.bits, 0),
        (Class::Zero, Class::Infinity) if !second.negative => indefinite,
        (Class::Zero, _) => done(sign(first.negative), 0),
        (Class::Finite { .. }, Class::Infinity) if second.negative => done(sign(first.negative), denormal),
        (Class::Finite { .. }, Class::Infinity) => done(sign(first.negative) | precision.to_infinity(), denormal),
        (Class::Finite { significand, exponent, .. }, _) => {
            let power = floor_of(second);
            let exact = Exact { negative: first.negative, significand, exponent: exponent.saturating_add(power), sticky: false };
            let (bits, after) = round(exact, precision, read_rounding(mxcsr), mxcsr);
            LaneOutcome { bits, before: denormal, after }
        }
        _ => unreachable!("NaNs are handled first"),
    }
}

/// The floor of a zero or finite number, held far past any exponent's range.
fn floor_of(operand: Operand) -> i32 {
    const FAR: u128 = 1 << 20;
    let Class::Finite { significand, exponent, .. } = operand.class else { return 0 };
    let rounding = if is_active(Bug::ScaleTruncatesPower) { Rounding::TowardZero } else { Rounding::Down };
    let (magnitude, _) = round_to_integer(significand, exponent, rounding, operand.negative);
    let magnitude = magnitude.min(FAR) as i32;
    if operand.negative { -magnitude } else { magnitude }
}
