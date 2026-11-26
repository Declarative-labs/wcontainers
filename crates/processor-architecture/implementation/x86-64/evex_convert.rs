//! The EVEX conversions between floating point of each precision and signed and unsigned integers of 32 and 64
//! bits, as the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! volume 2 gives them, and the conversions to bfloat16.

use crate::float::{
    Class, DENORMAL, DENORMALS_ARE_ZEROS, Exact, INEXACT, INVALID, LaneOutcome, OVERFLOW, Operand, Precision, Rounding,
    read_rounding, round, round_to_integer,
};
use crate::planted::{Bug, is_active};

/// What a conversion reads or writes in an element.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Number {
    Float(Precision),
    /// A signed integer of this many bytes.
    Signed(usize),
    Unsigned(usize),
}

impl Number {
    pub const fn to_bytes(self) -> usize {
        match self {
            Self::Float(precision) => (precision.to_bits() / u8::BITS) as usize,
            Self::Signed(bytes) | Self::Unsigned(bytes) => bytes,
        }
    }
}

/// One element's conversion; `truncate` rounds toward zero whatever MXCSR or the embedded control says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Conversion {
    pub from: Number,
    pub to: Number,
    pub truncate: bool,
    /// The lowest element alone.
    pub scalar: bool,
}

impl Conversion {
    /// Converts one element under MXCSR: a NaN or a value out of the integer's range is invalid and gives the
    /// integer indefinite (the sign bit alone, or all ones for an unsigned integer); a conversion to an integer
    /// reports no denormal.
    pub(crate) fn convert(self, value: u64, mxcsr: u32) -> LaneOutcome {
        let daz = mxcsr & DENORMALS_ARE_ZEROS != 0;
        let rounding = if self.truncate { Rounding::TowardZero } else { read_rounding(mxcsr) };
        let done = |bits: u64, before: u32| LaneOutcome { bits, before, after: 0 };
        match (self.from, self.to) {
            (Number::Float(from), Number::Float(to)) => {
                let operand = Operand::read(value, from, daz);
                let sign = if operand.negative { to.to_sign_bit() } else { 0 };
                match operand.class {
                    Class::Zero => done(sign, 0),
                    Class::Infinity => done(sign | to.to_infinity(), 0),
                    Class::Nan { signaling } => {
                        let fraction = operand.bits & from.to_fraction_mask();
                        let moved = if from.to_bits() < to.to_bits() {
                            fraction << (to.to_significand_bits() - from.to_significand_bits())
                        } else {
                            fraction >> (from.to_significand_bits() - to.to_significand_bits())
                        };
                        done(sign | to.to_infinity() | moved | to.to_quiet_bit(), if signaling { INVALID } else { 0 })
                    }
                    Class::Finite { significand, exponent, denormal } => {
                        let exact = Exact { negative: operand.negative, significand, exponent, sticky: false };
                        let (bits, after) = round(exact, to, rounding, mxcsr);
                        LaneOutcome { bits, before: if denormal { DENORMAL } else { 0 }, after }
                    }
                }
            }
            (Number::Signed(bytes) | Number::Unsigned(bytes), Number::Float(to)) => {
                let unused = 64 - 8 * bytes as u32;
                let (negative, magnitude) = match self.from {
                    Number::Signed(_) => {
                        let signed = ((value << unused) as i64) >> unused;
                        (signed < 0, signed.unsigned_abs())
                    }
                    _ => (false, (value << unused) >> unused),
                };
                if magnitude == 0 {
                    return done(0, 0);
                }
                let exact = Exact { negative, significand: u128::from(magnitude), exponent: 0, sticky: false };
                let (bits, mut after) = round(exact, to, rounding, mxcsr);
                // An integer that overflows a half is reported inexact as well, as the chip shows.
                if to == Precision::Half && after & OVERFLOW != 0 && !is_active(Bug::IntegerToHalfOverflowExact) {
                    after |= INEXACT;
                }
                LaneOutcome { bits, before: 0, after }
            }
            (Number::Float(from), Number::Signed(bytes) | Number::Unsigned(bytes)) => {
                let bits = 8 * bytes as u32;
                let signed = matches!(self.to, Number::Signed(_));
                let indefinite = if signed { 1 << (bits - 1) } else if bits == 64 { u64::MAX } else { (1 << bits) - 1 };
                let operand = Operand::read(value, from, daz);
                let Class::Finite { significand, exponent, .. } = operand.class else {
                    return match operand.class {
                        Class::Zero => done(0, 0),
                        _ => done(indefinite, INVALID),
                    };
                };
                let (magnitude, inexact) = round_to_integer(significand, exponent, rounding, operand.negative);
                let limit: u128 = match (signed, operand.negative) {
                    (true, true) => 1 << (bits - 1),
                    (true, false) => (1 << (bits - 1)) - 1,
                    (false, true) if is_active(Bug::EvexUnsignedNegativeWraps) => u128::MAX,
                    (false, true) => 0,
                    (false, false) => (1 << bits) - 1,
                };
                if magnitude > limit {
                    return done(indefinite, INVALID);
                }
                let integer = if operand.negative { (magnitude as u64).wrapping_neg() } else { magnitude as u64 };
                let mask = if bits == 64 { u64::MAX } else { (1 << bits) - 1 };
                LaneOutcome { bits: integer & mask, before: 0, after: if inexact { INEXACT } else { 0 } }
            }
            _ => unreachable!("no conversion between integers"),
        }
    }
}

/// A single converted to bfloat16 as VCVTNEPS2BF16 does: to nearest even, a denormal read as a zero of its sign,
/// a NaN quieted, MXCSR not read and no flag reported.
pub fn to_bfloat16(single: u32) -> u16 {
    let exponent = (single >> 23) & 0xFF;
    let fraction = single & 0x7F_FFFF;
    if exponent == 0xFF && fraction != 0 {
        return ((single >> 16) | 0x40) as u16;
    }
    if exponent == 0 && !is_active(Bug::EvexBfloatKeepsDenormals) {
        return ((single >> 16) & 0x8000) as u16;
    }
    let lowest_kept = (single >> 16) & 1;
    (single.wrapping_add(0x7FFF + lowest_kept) >> 16) as u16
}
