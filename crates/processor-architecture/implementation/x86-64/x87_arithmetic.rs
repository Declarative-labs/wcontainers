//! The x87 arithmetic on 80-bit values, as the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! (vol. 1, chapters 4 and 8) gives it: each result exact, then rounded to FCW's precision (24, 53 or 64 bits of
//! significand, the exponent always extended) by FCW's rounding.
//!
//! The exceptions come in the manual's order of priority (vol. 1, 4.9.2): IE for a signaling NaN or a format no
//! operation takes, a quiet NaN's propagation, the other invalid operations and ZE, DE for a denormal operand, then
//! OE or UE and PE from the rounding. Masked, IE gives a NaN or the indefinite QNaN, ZE an infinity, OE an infinity
//! or the largest finite value by the rounding, UE a denormal. Unmasked, IE, ZE and DE leave the destination as it
//! was; OE and UE deliver the result with its exponent moved by 24,576 into range. A result is tiny where, rounded
//! to the precision with an unbounded exponent, it lies below the smallest normal, as the chip has it. C1 tells
//! whether the result's magnitude was rounded up.

use crate::planted::{Bug, is_active};
use crate::x87::{Class, INDEFINITE, Rounding, classify};

pub const INVALID: u16 = 1 << 0;
pub const DENORMAL: u16 = 1 << 1;
pub const ZERO_DIVIDE: u16 = 1 << 2;
pub const OVERFLOW: u16 = 1 << 3;
pub const UNDERFLOW: u16 = 1 << 4;
pub const PRECISION: u16 = 1 << 5;

const BIAS: i32 = 16383;
const MAXIMUM_BIASED: i32 = 0x7FFF;
/// How far an unmasked overflow or underflow moves the exponent.
const EXPONENT_ADJUSTMENT: i32 = 24576;
const SIGN: u128 = 1 << 79;
const INTEGER_BIT: u64 = 1 << 63;
const QUIET: u64 = 1 << 62;
const INFINITY: u128 = 0x7FFF_8000_0000_0000_0000;

/// The four operations of FADD to FDIVR.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arithmetic {
    Add,
    Subtract,
    Multiply,
    Divide,
}

/// What an operation gives: the value to write, none where an unmasked exception before the computation stops it;
/// the exception flags; whether the magnitude was rounded up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    pub value: Option<u128>,
    pub flags: u16,
    pub up: bool,
}

/// A finite value taken apart: its significand times two to `exponent` less 63.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Parts {
    pub negative: bool,
    pub exponent: i32,
    pub significand: u64,
}

/// A value exactly or to a sticky bit: `significand` plus less than one times two to `exponent` less 127, the
/// significand's top bit set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Exact {
    negative: bool,
    exponent: i32,
    significand: u128,
    sticky: bool,
}

/// The bits of significand FCW's precision control keeps: 24, 53, or 64 (and 64 for the reserved setting).
pub const fn to_precision(control: u16) -> u32 {
    match control >> 8 & 3 {
        0 => 24,
        2 => 53,
        _ => 64,
    }
}

pub const fn is_negative(value: u128) -> bool {
    value & SIGN != 0
}

/// A finite value's parts; a denormal, pseudo-denormals among them, has the smallest exponent.
pub fn to_parts(value: u128) -> Parts {
    let biased = ((value >> 64) & 0x7FFF) as i32;
    Parts { negative: is_negative(value), exponent: biased.max(1) - BIAS, significand: value as u64 }
}

/// The value of parts already in range.
pub fn from_parts(negative: bool, biased: i32, significand: u64) -> u128 {
    (if negative { SIGN } else { 0 }) | ((biased as u128) << 64) | u128::from(significand)
}

pub fn quiet(value: u128) -> u128 {
    value | u128::from(QUIET)
}

/// The NaN an operation on two operands gives, at least one a NaN: a quiet NaN over a signaling one, else the larger
/// significand, the first where they are equal; a signaling NaN quieted.
pub fn propagate(first: u128, second: u128) -> u128 {
    let (first_class, second_class) = (classify(first), classify(second));
    let nan = |class| matches!(class, Class::QuietNaN | Class::SignalingNaN);
    match (nan(first_class), nan(second_class)) {
        (true, false) => quiet(first),
        (false, true) => quiet(second),
        _ if first_class == Class::QuietNaN && second_class == Class::SignalingNaN => first,
        _ if first_class == Class::SignalingNaN && second_class == Class::QuietNaN => second,
        _ if (second as u64 & !QUIET) > (first as u64 & !QUIET) && !is_active(Bug::X87NanSmallerSignificandWins) => quiet(second),
        _ => quiet(first),
    }
}

/// The parts made normal, the significand's top bit set; zero stays zero.
fn normalize(parts: Parts) -> Parts {
    if parts.significand == 0 {
        return parts;
    }
    let shift = parts.significand.leading_zeros();
    Parts { exponent: parts.exponent - shift as i32, significand: parts.significand << shift, ..parts }
}

/// The exact value `bits` times two to `scale`, its top bit moved to bit 127; none for zero.
fn to_exact(negative: bool, scale: i32, bits: u128, sticky: bool) -> Option<Exact> {
    if bits == 0 {
        return None;
    }
    let shift = bits.leading_zeros();
    Some(Exact { negative, exponent: scale + 127 - shift as i32, significand: bits << shift, sticky })
}

/// The bits kept after dropping `drop` of them, rounded: the kept bits, whether anything was dropped, and whether
/// the magnitude was rounded up.
fn round_bits(bits: u128, sticky: bool, drop: u32, negative: bool, rounding: Rounding) -> (u128, bool, bool) {
    let (kept, remainder_above_half, remainder_half, remainder_nonzero) = if drop == 0 {
        (bits, false, false, false)
    } else if drop > 128 {
        (0, false, false, bits != 0)
    } else if drop == 128 {
        (0, bits > 1 << 127, bits == 1 << 127, bits != 0)
    } else {
        let remainder = bits & ((1 << drop) - 1);
        let half = 1 << (drop - 1);
        (bits >> drop, remainder > half, remainder == half, remainder != 0)
    };
    let inexact = remainder_nonzero || sticky;
    let up = match rounding {
        Rounding::Nearest => remainder_above_half || (remainder_half && (sticky || kept & 1 == 1)),
        Rounding::Down => negative && inexact,
        Rounding::Up => !negative && inexact,
        Rounding::TowardZero => false,
    };
    (kept + u128::from(up), inexact, up)
}

/// The largest finite value of the precision.
fn largest(negative: bool, precision: u32) -> u128 {
    let significand = u64::MAX << (64 - precision);
    from_parts(negative, MAXIMUM_BIASED - 1, significand)
}

/// Rounds an exact nonzero value to the precision with an extended exponent.
fn round_extended(exact: Exact, precision: u32, rounding: Rounding, control: u16) -> Outcome {
    let negative = exact.negative;
    let underflow_unmasked = control & UNDERFLOW == 0;
    let overflow_unmasked = control & OVERFLOW == 0;
    let mut biased = exact.exponent + BIAS;
    let mut flags = 0;
    if biased == 0 && !is_tiny(exact, precision, rounding) {
        // Rounded with an unbounded exponent it reaches the smallest normal, which is what it rounds to.
        let (kept, _, up) = round_bits(exact.significand, exact.sticky, 128 - precision, negative, rounding);
        let significand = ((kept >> 1) << (64 - precision)) as u64;
        return Outcome { value: Some(from_parts(negative, 1, significand)), flags: PRECISION, up };
    }
    if biased < 1 && underflow_unmasked {
        biased += EXPONENT_ADJUSTMENT;
        flags |= UNDERFLOW;
    }
    if biased < 1 {
        // Masked underflow: the result denormalized at the precision's fixed point below the smallest normal.
        let shift = (1 - biased) as u32;
        let (kept, inexact, up) = round_bits(exact.significand, exact.sticky, 128 - precision + shift, negative, rounding);
        let field = if kept >> (precision - 1) != 0 { 1 } else { 0 };
        if inexact {
            flags |= UNDERFLOW | PRECISION;
        }
        let significand = (kept << (64 - precision)) as u64;
        return Outcome { value: Some(from_parts(negative, field, significand)), flags, up };
    }
    let (mut kept, inexact, up) = round_bits(exact.significand, exact.sticky, 128 - precision, negative, rounding);
    if kept >> precision != 0 {
        kept >>= 1;
        biased += 1;
    }
    if inexact {
        flags |= PRECISION;
    }
    if biased >= MAXIMUM_BIASED {
        if overflow_unmasked {
            biased -= EXPONENT_ADJUSTMENT;
            flags |= OVERFLOW;
        } else {
            let infinite = match rounding {
                Rounding::Nearest => true,
                Rounding::TowardZero => false,
                Rounding::Down => negative,
                Rounding::Up => !negative,
            };
            let value = if infinite { INFINITY | if negative { SIGN } else { 0 } } else { largest(negative, precision) };
            return Outcome { value: Some(value), flags: flags | OVERFLOW | PRECISION, up: infinite };
        }
    }
    Outcome { value: Some(from_parts(negative, biased, (kept << (64 - precision)) as u64)), flags, up }
}

/// Whether a value below the smallest normal stays below it once rounded to the precision with an unbounded
/// exponent.
fn is_tiny(exact: Exact, precision: u32, rounding: Rounding) -> bool {
    if exact.exponent + BIAS < 0 || is_active(Bug::X87TinyBeforeRounding) {
        return true;
    }
    let (kept, _, _) = round_bits(exact.significand, exact.sticky, 128 - precision, exact.negative, rounding);
    kept >> precision == 0
}

/// The exact sum of two finite values, or `None` with the sign for an exact zero.
fn add_exact(first: Parts, second: Parts) -> Result<Exact, bool> {
    // Two bits of headroom above each significand, 62 below it.
    let (mut larger, mut smaller) = (first, second);
    if smaller.exponent > larger.exponent {
        (larger, smaller) = (smaller, larger);
    }
    let scale = larger.exponent - 63 - 62;
    let big = u128::from(larger.significand) << 62;
    let distance = (larger.exponent - smaller.exponent) as u32;
    let small_bits = u128::from(smaller.significand) << 62;
    let (small, lost) = if distance >= 128 {
        (0, small_bits != 0)
    } else {
        (small_bits >> distance, small_bits & ((1 << distance) - 1) != 0)
    };
    if larger.negative == smaller.negative {
        return to_exact(larger.negative, scale, big + small, lost).ok_or(larger.negative);
    }
    if distance > 0 {
        // The larger exponent's magnitude is the larger, so the lost bits borrow from the difference.
        return to_exact(larger.negative, scale, big - small - u128::from(lost), lost).ok_or(larger.negative);
    }
    match big.cmp(&small) {
        core::cmp::Ordering::Greater => to_exact(larger.negative, scale, big - small, false).ok_or(false),
        core::cmp::Ordering::Less => to_exact(smaller.negative, scale, small - big, false).ok_or(false),
        core::cmp::Ordering::Equal => Err(false),
    }
}

fn multiply_exact(first: Parts, second: Parts) -> Option<Exact> {
    let product = u128::from(first.significand) * u128::from(second.significand);
    to_exact(first.negative != second.negative, first.exponent - 63 + second.exponent - 63, product, false)
}

fn divide_exact(first: Parts, second: Parts) -> Option<Exact> {
    let (dividend, divisor) = (normalize(first), normalize(second));
    let numerator = u128::from(dividend.significand) << 64;
    let divisor_bits = u128::from(divisor.significand);
    let (quotient, remainder) = (numerator / divisor_bits, numerator % divisor_bits);
    let doubled = remainder << 1;
    let guard = doubled >= divisor_bits;
    let rest = if guard { doubled - divisor_bits } else { doubled };
    let bits = quotient << 1 | u128::from(guard);
    to_exact(first.negative != second.negative, dividend.exponent - divisor.exponent - 65, bits, rest != 0)
}

/// The square root of a positive finite value.
fn square_root_exact(value: Parts) -> Option<Exact> {
    const EXTRA_PAIRS: u32 = 36;
    let normal = normalize(value);
    let mut scale = normal.exponent - 63;
    let mut bits = u128::from(normal.significand);
    if scale % 2 != 0 {
        bits <<= 1;
        scale -= 1;
    }
    let pairs = (u128::BITS - bits.leading_zeros()).div_ceil(2);
    let (mut remainder, mut root) = (0u128, 0u128);
    for index in (0..pairs + EXTRA_PAIRS).rev() {
        let pair = if index >= EXTRA_PAIRS { (bits >> (2 * (index - EXTRA_PAIRS))) & 3 } else { 0 };
        remainder = (remainder << 2) | pair;
        let trial = (root << 2) | 1;
        if remainder >= trial {
            remainder -= trial;
            root = (root << 1) | 1;
        } else {
            root <<= 1;
        }
    }
    to_exact(false, scale / 2 - EXTRA_PAIRS as i32, root, remainder != 0)
}

/// The zero an exact zero result takes: negative where both operands' signs make it so, or under rounding down.
fn to_zero(negative: bool) -> u128 {
    if negative { SIGN } else { 0 }
}

/// FADD to FDIVR on two 80-bit values, the first the minuend or dividend, under FCW.
pub fn compute(operation: Arithmetic, first: u128, second: u128, control: u16) -> Outcome {
    compute_with_source(operation, first, second, control, false)
}

/// FADD to FDIVR, where `denormal_source` says the memory operand was a denormal before it was widened.
pub fn compute_with_source(operation: Arithmetic, first: u128, second: u128, control: u16, denormal_source: bool) -> Outcome {
    let rounding = Rounding::from_control(control);
    let precision = if is_active(Bug::X87PrecisionControlIgnored) { 64 } else { to_precision(control) };
    let (first_class, second_class) = (classify(first), classify(second));
    let classes = [first_class, second_class];
    let masked = |flag: u16| control & flag != 0;
    let stop = |flag: u16, value: u128| Outcome { value: masked(flag).then_some(value), flags: flag, up: false };
    if classes.contains(&Class::Unsupported) {
        return stop(INVALID, INDEFINITE);
    }
    if classes.contains(&Class::SignalingNaN) {
        return stop(INVALID, propagate(first, second));
    }
    if classes.contains(&Class::QuietNaN) {
        return Outcome { value: Some(propagate(first, second)), flags: 0, up: false };
    }
    let second = if operation == Arithmetic::Subtract { second ^ SIGN } else { second };
    let negative = is_negative(first) != is_negative(second);
    let infinite = |class| class == Class::Infinity;
    let zero = |class| class == Class::Zero;
    let invalid = match operation {
        Arithmetic::Add | Arithmetic::Subtract => infinite(first_class) && infinite(second_class) && negative,
        Arithmetic::Multiply => (zero(first_class) && infinite(second_class)) || (infinite(first_class) && zero(second_class)),
        Arithmetic::Divide => (zero(first_class) && zero(second_class)) || (infinite(first_class) && infinite(second_class)),
    };
    if invalid {
        return stop(INVALID, INDEFINITE);
    }
    if operation == Arithmetic::Divide && zero(second_class) && !infinite(first_class) {
        return stop(ZERO_DIVIDE, INFINITY | to_zero(negative));
    }
    let denormal = (classes.contains(&Class::Denormal) || denormal_source) && !is_active(Bug::X87DenormalOperandIgnored);
    if denormal && !masked(DENORMAL) {
        return Outcome { value: None, flags: DENORMAL, up: false };
    }
    let flags = if denormal { DENORMAL } else { 0 };
    let done = |value: u128| Outcome { value: Some(value), flags, up: false };
    let exact = match operation {
        Arithmetic::Add | Arithmetic::Subtract => {
            if infinite(first_class) {
                return done(first);
            }
            if infinite(second_class) {
                return done(second);
            }
            match add_exact(to_parts(first), to_parts(second)) {
                Ok(exact) => exact,
                // An exact zero: the operands' sign where both have it, else positive but under rounding down.
                Err(_) => {
                    let both = is_negative(first) && is_negative(second);
                    let either = is_negative(first) != is_negative(second);
                    let negative = both || (either && rounding == Rounding::Down);
                    return done(to_zero(negative));
                }
            }
        }
        Arithmetic::Multiply => {
            if infinite(first_class) || infinite(second_class) {
                return done(INFINITY | to_zero(negative));
            }
            match multiply_exact(to_parts(first), to_parts(second)) {
                Some(exact) => exact,
                None => return done(to_zero(negative)),
            }
        }
        Arithmetic::Divide => {
            if infinite(first_class) {
                return done(INFINITY | to_zero(negative));
            }
            if infinite(second_class) || zero(first_class) {
                return done(to_zero(negative));
            }
            divide_exact(to_parts(first), to_parts(second)).expect("a nonzero quotient")
        }
    };
    let rounded = round_extended(exact, precision, rounding, control);
    Outcome { flags: rounded.flags | flags, ..rounded }
}

/// FSQRT of an 80-bit value under FCW.
pub fn square_root(value: u128, control: u16) -> Outcome {
    let masked = |flag: u16| control & flag != 0;
    let stop = |flag: u16, result: u128| Outcome { value: masked(flag).then_some(result), flags: flag, up: false };
    match classify(value) {
        Class::Unsupported => return stop(INVALID, INDEFINITE),
        Class::SignalingNaN => return stop(INVALID, quiet(value)),
        Class::QuietNaN => return Outcome { value: Some(value), flags: 0, up: false },
        Class::Zero => return Outcome { value: Some(value), flags: 0, up: false },
        _ if is_negative(value) => return stop(INVALID, INDEFINITE),
        Class::Infinity => return Outcome { value: Some(value), flags: 0, up: false },
        _ => {}
    }
    let mut flags = 0;
    if classify(value) == Class::Denormal {
        if !masked(DENORMAL) {
            return Outcome { value: None, flags: DENORMAL, up: false };
        }
        flags |= DENORMAL;
    }
    let exact = square_root_exact(to_parts(value)).expect("a nonzero root");
    let rounded = round_extended(exact, to_precision(control), Rounding::from_control(control), control);
    Outcome { flags: rounded.flags | flags, ..rounded }
}

/// A finite 80-bit value rounded to a binary format of `fraction_bits` and `exponent_bits` under FCW's rounding, as
/// FST to memory gives it: the bits, the flags, and whether the magnitude was rounded up; none where an unmasked
/// overflow or underflow keeps the store from happening.
pub fn round_to_format(value: u128, fraction_bits: u32, exponent_bits: u32, control: u16) -> (Option<u64>, u16, bool) {
    let rounding = Rounding::from_control(control);
    let parts = normalize(to_parts(value));
    let negative = parts.negative;
    let sign = u64::from(negative) << (fraction_bits + exponent_bits);
    if parts.significand == 0 {
        return (Some(sign), 0, false);
    }
    let bias = (1i32 << (exponent_bits - 1)) - 1;
    let maximum = (1i32 << exponent_bits) - 1;
    let precision = fraction_bits + 1;
    let mut biased = parts.exponent + bias;
    let bits = u128::from(parts.significand) << 64;
    let mut flags = 0;
    let rounded_to_normal = biased == 0 && {
        let (kept, _, _) = round_bits(bits, false, 128 - precision, negative, rounding);
        kept >> precision != 0
    };
    if biased < 1 && !(rounded_to_normal && !is_active(Bug::X87TinyBeforeRounding)) {
        if control & UNDERFLOW == 0 {
            return (None, UNDERFLOW, false);
        }
        let shift = (1 - biased) as u32;
        let (kept, inexact, up) = round_bits(bits, false, 128 - precision + shift, negative, rounding);
        if inexact {
            flags |= UNDERFLOW | PRECISION;
        }
        // A result rounded up to the smallest normal carries into the exponent field by itself.
        return (Some(sign | kept as u64), flags, up);
    }
    let (mut kept, inexact, up) = round_bits(bits, false, 128 - precision, negative, rounding);
    if kept >> precision != 0 {
        kept >>= 1;
        biased += 1;
    }
    if inexact {
        flags |= PRECISION;
    }
    if biased >= maximum {
        // Unmasked, the store does not happen and only OE is raised, as the chip has it.
        if control & OVERFLOW == 0 {
            return (None, OVERFLOW, false);
        }
        let infinite = match rounding {
            Rounding::Nearest => true,
            Rounding::TowardZero => false,
            Rounding::Down => negative,
            Rounding::Up => !negative,
        };
        let magnitude = if infinite { (maximum as u64) << fraction_bits } else { ((maximum as u64 - 1) << fraction_bits) | ((1 << fraction_bits) - 1) };
        return (Some(sign | magnitude), flags | OVERFLOW | PRECISION, infinite);
    }
    let fraction = kept as u64 & ((1 << fraction_bits) - 1);
    (Some(sign | ((biased as u64) << fraction_bits) | fraction), flags, up)
}

/// An 80-bit value rounded to an integer by FCW's rounding, as FRNDINT gives it: the value, whether it was inexact,
/// and whether its magnitude was rounded up.
pub fn round_to_integral(value: u128, rounding: Rounding) -> (u128, bool, bool) {
    let parts = to_parts(value);
    let negative = parts.negative;
    // At an exponent of 63 or more every bit is above the binary point.
    if parts.exponent >= 63 || parts.significand == 0 {
        return (value, false, false);
    }
    let drop = (63 - parts.exponent) as u32;
    let (kept, inexact, up) = round_bits(u128::from(parts.significand), false, drop, negative, rounding);
    if kept == 0 {
        return (to_zero(negative), inexact, up);
    }
    let normal = normalize(Parts { negative, exponent: 63, significand: kept as u64 });
    (from_parts(negative, normal.exponent + BIAS, normal.significand), inexact, up)
}

/// Whether the significand's integer bit is set.
pub const fn has_integer_bit(value: u128) -> bool {
    value as u64 & INTEGER_BIT != 0
}

/// FST to memory of an 80-bit value in a binary format of `fraction_bits` and `exponent_bits`: the bits stored, none
/// where an unmasked exception keeps the store from happening; the flags; whether the magnitude was rounded up. A
/// NaN keeps its fraction's top bits, quieted, with IE for a signaling one; a format no operation takes stores the
/// format's indefinite QNaN with IE; a denormal raises no DE, as the chip has it.
pub fn store_float(value: u128, fraction_bits: u32, exponent_bits: u32, control: u16) -> (Option<u64>, u16, bool) {
    let sign = u64::from(is_negative(value)) << (fraction_bits + exponent_bits);
    let exponent_ones = ((1u64 << exponent_bits) - 1) << fraction_bits;
    let quiet_bit = 1u64 << (fraction_bits - 1);
    let masked = |flag: u16| control & flag != 0;
    let nan = |value: u128| sign | exponent_ones | ((value as u64 & !INTEGER_BIT) >> (63 - fraction_bits));
    match classify(value) {
        Class::Unsupported => (masked(INVALID).then_some(sign | exponent_ones | quiet_bit | 1 << (fraction_bits + exponent_bits)), INVALID, false),
        Class::SignalingNaN => (masked(INVALID).then_some(nan(value) | quiet_bit), INVALID, false),
        Class::QuietNaN => (Some(nan(value)), 0, false),
        Class::Infinity => (Some(sign | exponent_ones), 0, false),
        Class::Zero => (Some(sign), 0, false),
        _ => round_to_format(value, fraction_bits, exponent_bits, control),
    }
}

/// FRNDINT of an 80-bit value under FCW's rounding: a NaN, infinity or zero as it is, with IE for a signaling NaN or
/// a format no operation takes, DE for a denormal, PE for an inexact result.
pub fn round_integral(value: u128, control: u16) -> Outcome {
    let masked = |flag: u16| control & flag != 0;
    let stop = |flag: u16, result: u128| Outcome { value: masked(flag).then_some(result), flags: flag, up: false };
    let mut flags = 0;
    match classify(value) {
        Class::Unsupported => return stop(INVALID, INDEFINITE),
        Class::SignalingNaN => return stop(INVALID, quiet(value)),
        Class::QuietNaN | Class::Infinity | Class::Zero => return Outcome { value: Some(value), flags: 0, up: false },
        Class::Denormal => {
            if !masked(DENORMAL) {
                return Outcome { value: None, flags: DENORMAL, up: false };
            }
            flags |= DENORMAL;
        }
        Class::Normal => {}
    }
    let (result, inexact, up) = round_to_integral(value, Rounding::from_control(control));
    if inexact {
        flags |= PRECISION;
    }
    Outcome { value: Some(result), flags, up }
}
