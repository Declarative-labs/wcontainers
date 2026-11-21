//! `fsqrt`'s root of a finite positive 80-bit value, rounded to odd in the wide semantics (plan 5.8): a first
//! guess from astro-float's correctly rounded root, then the exact square of its top bits decides them.

use crate::x87::{BIAS, Extended, Wide};
use astro_float_num::{BigFloat, Exponent, RoundingMode, Word};
use rustc_apfloat::Float;

/// Bits of the first guess, well past the wide semantics' 66, and of an exact product of two guesses.
const GUESS_BITS: usize = u128::BITS as usize;
const SQUARE_BITS: usize = 2 * GUESS_BITS;
/// The wide semantics' precision.
const WIDE_BITS: u32 = <crate::x87::WideS as rustc_apfloat::ieee::Semantics>::PRECISION as u32;

/// `m * 2^e` exactly.
fn big(m: u128, e: i32) -> BigFloat {
    let mut b = BigFloat::from_u128(m, GUESS_BITS);
    if let Some(at) = b.exponent() {
        b.set_exponent(at + e as Exponent);
    }
    b
}

/// The integer `n` with `0.n * 2^exponent` the value, its top bit at bit 127; the rest of the digits are
/// folded into whether any lower bit is set.
fn top_bits(b: &BigFloat) -> (u128, i32, bool) {
    let digits = b.mantissa_digits().expect("a finite root");
    let mut top = 0u128;
    let mut taken = 0;
    let mut lower = false;
    for &d in digits.iter().rev() {
        if taken < u128::BITS {
            top = top << Word::BITS | u128::from(d);
            taken += Word::BITS;
        } else {
            lower |= d != 0;
        }
    }
    let exponent = b.exponent().expect("a finite root");
    (top, exponent, lower)
}

/// The square root of `x`, positive and finite, truncated to the wide semantics' precision with its lowest
/// bit set when it is not exact.
pub(crate) fn odd_root(x: Extended) -> Wide {
    let integer_shift = u64::BITS as i32 - 1;
    let e = i32::from(x.exponent.max(1)) - BIAS - integer_shift;
    let value = big(x.significand.into(), e);
    let guess = value.sqrt(GUESS_BITS, RoundingMode::ToEven);
    let (top, exponent, _) = top_bits(&guess);
    let drop = u128::BITS - WIDE_BITS;
    let mut root = top >> drop;
    let mut scale = exponent - u128::BITS as i32 + drop as i32;
    let square = |r: u128, scale: i32| {
        let b = big(r, scale);
        b.mul(&b, SQUARE_BITS, RoundingMode::ToEven)
            .cmp(&value)
            .expect("finite")
    };
    // The guess may sit one unit of the wide precision off the truncated root either way.
    if square(root, scale) > 0 {
        root -= 1;
    } else if square(root + 1, scale) <= 0 {
        root += 1;
    }
    if root >> WIDE_BITS != 0 {
        root >>= 1;
        scale += 1;
    }
    let exact = square(root, scale) == 0;
    let odd = root | u128::from(!exact);
    Wide::from_u128(odd).value.scalbn(scale)
}
