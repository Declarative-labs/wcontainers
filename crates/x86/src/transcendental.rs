//! The exact values of the x87 unit's transcendental instructions (plan 5.8), worked out with astro-float
//! well past the wide semantics' precision and rounded to odd there, for the unit to round once more as the
//! rounding control says. Each takes finite, non-zero operands inside the instruction's range.

use crate::root::{GUESS_BITS, WIDE_BITS, big, of_extended, top_bits};
use crate::x87::{Extended, Wide};
use astro_float_num::{BigFloat, Consts, RoundingMode, Sign, Word};
use rustc_apfloat::Float;
use std::cell::RefCell;

/// The bits of π the trigonometric instructions reduce their argument by (Intel SDM Vol. 1 8.3.10).
const REDUCTION_PI_BITS: u32 = 66;
/// The working precision of a first evaluation: past twice the wide precision, so that the few units of error
/// of a chain of correctly rounded steps never reach the wide semantics' last bit.
const FIRST_BITS: usize = 192;
/// The working precision past which an inexact value whose bits below the wide precision are still all zero or
/// all one is taken as its nearest wide value: twice the bits from the largest 80-bit value to the smallest
/// denormal, which a value as near one of its operands as `sin(x)` is to `x` needs.
const LAST_BITS: usize = 1 << 16;
/// Enough bits to hold `x - k * π / 2` exactly for every reduced `x` below 2^63.
const REDUCTION_BITS: usize = 2 * GUESS_BITS;
const ROUND: RoundingMode = RoundingMode::ToEven;

thread_local! {
    static CONSTANTS: RefCell<Consts> = RefCell::new(Consts::new().expect("the constants cache"));
}

/// The value `evaluate` works out at a working precision, rounded to odd in the wide semantics. An exact value
/// is taken as it is; for another the working precision doubles until the bits below the wide precision, but
/// for the last few that may be in error, tell the truncation apart (they are neither all zero nor all one).
fn settled(evaluate: impl Fn(usize, &mut Consts) -> BigFloat) -> Wide {
    CONSTANTS.with_borrow_mut(|cc| {
        let drop = u128::BITS - WIDE_BITS;
        let mut bits = FIRST_BITS;
        loop {
            let v = evaluate(bits, cc);
            if v.is_zero() {
                return Wide::ZERO;
            }
            let last = bits >= LAST_BITS;
            let exact = !v.inexact();
            let digits = v.mantissa_digits().expect("a finite value");
            if exact || last || told_apart(digits) {
                let (top, exponent, lower) = top_bits(&v);
                let below = (1u128 << drop) - 1;
                let mut odd = if exact {
                    top >> drop | u128::from(top & below != 0 || lower)
                } else if last {
                    (top >> drop) + (top >> (drop - 1) & 1)
                } else {
                    top >> drop | 1
                };
                let mut scale = exponent - u128::BITS as i32 + drop as i32;
                if odd >> WIDE_BITS != 0 {
                    odd >>= 1;
                    scale += 1;
                }
                let wide = Wide::from_u128(odd).value.scalbn(scale);
                return if v.is_negative() { -wide } else { wide };
            }
            bits *= 2;
        }
    })
}

/// The lowest bits of a working value that may be in error: a chain of a few correctly rounded steps is off by
/// a few units.
const GUARD_BITS: u32 = 8;

/// Whether the bits of a mantissa (its words least significant first, its top bit set) below the wide
/// precision and above the guard bits are neither all zero nor all one.
fn told_apart(digits: &[Word]) -> bool {
    let total = digits.len() as u32 * Word::BITS;
    let (mut zero, mut one) = (true, true);
    for bit in GUARD_BITS..total - WIDE_BITS {
        let set = digits[(bit / Word::BITS) as usize] >> (bit % Word::BITS) & 1 != 0;
        zero &= !set;
        one &= set;
        if !zero && !one {
            return true;
        }
    }
    false
}

fn signed(mut b: BigFloat, negative: bool) -> BigFloat {
    if negative {
        b.set_sign(if b.is_negative() {
            Sign::Pos
        } else {
            Sign::Neg
        });
    }
    b
}

/// π rounded to `REDUCTION_PI_BITS`, halved.
fn reduction_half_pi(cc: &mut Consts) -> BigFloat {
    let (top, exponent, _) = top_bits(&cc.pi(GUESS_BITS, ROUND));
    let drop = u128::BITS - REDUCTION_PI_BITS;
    let rounded = (top >> drop) + (top >> (drop - 1) & 1);
    big(rounded, exponent - REDUCTION_PI_BITS as i32 - 1)
}

/// The integer nearest a finite value (no value given here lies half way).
fn nearest_integer(b: &BigFloat) -> i128 {
    if b.is_zero() {
        return 0;
    }
    let (top, exponent, _) = top_bits(b);
    if exponent < 0 {
        return 0;
    }
    let twice = top >> (u128::BITS as i32 - exponent - 1);
    let n = ((twice + 1) >> 1) as i128;
    if b.is_negative() { -n } else { n }
}

/// `x` reduced to `r = x - k * π66 / 2` exactly, with the quadrant `k mod 4`.
fn reduced(x: Extended, cc: &mut Consts) -> (BigFloat, u32) {
    let x = of_extended(x);
    let half = reduction_half_pi(cc);
    let k = nearest_integer(&x.div(&half, FIRST_BITS, ROUND));
    let multiple = signed(big(k.unsigned_abs(), 0), k < 0).mul(&half, REDUCTION_BITS, ROUND);
    let r = x.sub(&multiple, REDUCTION_BITS, ROUND);
    (r, k.rem_euclid(QUADRANTS) as u32)
}

const QUADRANTS: i128 = 4;

/// `fsin`'s value.
pub(crate) fn sine(x: Extended) -> Wide {
    settled(|p, cc| {
        let (r, quadrant) = reduced(x, cc);
        match quadrant {
            0 => r.sin(p, ROUND, cc),
            1 => r.cos(p, ROUND, cc),
            2 => r.sin(p, ROUND, cc).neg(),
            _ => r.cos(p, ROUND, cc).neg(),
        }
    })
}

/// `fcos`'s value.
pub(crate) fn cosine(x: Extended) -> Wide {
    settled(|p, cc| {
        let (r, quadrant) = reduced(x, cc);
        match quadrant {
            0 => r.cos(p, ROUND, cc),
            1 => r.sin(p, ROUND, cc).neg(),
            2 => r.cos(p, ROUND, cc).neg(),
            _ => r.sin(p, ROUND, cc),
        }
    })
}

/// `fptan`'s value.
pub(crate) fn tangent(x: Extended) -> Wide {
    settled(|p, cc| {
        let (r, quadrant) = reduced(x, cc);
        let t = r.tan(p, ROUND, cc);
        if quadrant % 2 == 0 {
            t
        } else {
            t.reciprocal(p, ROUND).neg()
        }
    })
}

/// The bits a value of exponent `e` below zero needs past the working precision so that subtracting it from,
/// or adding it to, one keeps that precision.
fn cancelled(x: Extended) -> usize {
    (crate::x87::BIAS - i32::from(x.exponent.max(1))).max(0) as usize
}

/// `f2xm1`'s value, `2^x - 1`, for `|x| < 1`.
pub(crate) fn exponent_minus_one(x: Extended) -> Wide {
    settled(|p, cc| {
        let p = p + cancelled(x);
        let t = of_extended(x).mul(&cc.ln_2(p, ROUND), p, ROUND);
        t.exp(p, ROUND, cc)
            .sub(&BigFloat::from_word(1, p), p, ROUND)
    })
}

/// `fyl2x`'s value, `y * log2(x)`, for positive `x`.
pub(crate) fn logarithm(y: Extended, x: Extended) -> Wide {
    settled(|p, cc| {
        of_extended(x)
            .log2(p, ROUND, cc)
            .mul(&of_extended(y), p, ROUND)
    })
}

/// `fyl2xp1`'s value, `y * log2(x + 1)`, for `x` above -1.
pub(crate) fn logarithm_plus_one(y: Extended, x: Extended) -> Wide {
    logarithm_plus_one_by(
        y,
        x,
        i32::from(x.exponent) - crate::x87::BIAS < SERIES_BELOW,
    )
}

/// `fyl2xp1`'s value, by the series or by the exact sum `1 + x`.
pub(crate) fn logarithm_plus_one_by(y: Extended, x: Extended, series: bool) -> Wide {
    settled(|p, cc| {
        let logarithm = if series {
            natural_logarithm_plus_one_small(&of_extended(x), p).div(&cc.ln_2(p, ROUND), p, ROUND)
        } else {
            let exact = GUESS_BITS + cancelled(x);
            let sum = of_extended(x).add(&BigFloat::from_word(1, exact), exact, ROUND);
            sum.log2(p, ROUND, cc)
        };
        logarithm.mul(&of_extended(y), p, ROUND)
    })
}

/// Below 2^-64 `ln(1 + x)` is summed from its series, each term 64 bits or more below the one before, which
/// spares the sum `1 + x` the thousands of exact bits a tiny `x` gives it.
const SERIES_BELOW: i32 = -(u64::BITS as i32);

/// `ln(1 + x)` for `|x|` below 2^-64, from `x - x^2/2 + x^3/3 - ...` to the working precision and the guard
/// bits below it.
fn natural_logarithm_plus_one_small(x: &BigFloat, p: usize) -> BigFloat {
    let floor = x.exponent().expect("a non-zero operand") - p as i32 - GUARD_BITS as i32;
    let (mut sum, mut power) = (x.clone(), x.clone());
    let mut k: Word = 1;
    loop {
        k += 1;
        power = power.mul(x, p, ROUND);
        let term = power.div(&BigFloat::from_word(k, p), p, ROUND);
        if term.exponent().is_none_or(|e| e < floor) {
            return sum;
        }
        sum = if k.is_multiple_of(2) {
            sum.sub(&term, p, ROUND)
        } else {
            sum.add(&term, p, ROUND)
        };
    }
}

/// `fpatan`'s value, the angle of the point (`x`, `y`) from -π to π.
pub(crate) fn arc_tangent(y: Extended, x: Extended) -> Wide {
    settled(|p, cc| {
        let (y, x) = (of_extended(y), of_extended(x));
        let a = y.div(&x, p, ROUND).atan(p, ROUND, cc);
        if !x.is_negative() {
            return a;
        }
        let pi = signed(cc.pi(p, ROUND), y.is_negative());
        a.add(&pi, p, ROUND)
    })
}

/// The bits of ln 2, truncated, the reference multiplies an `f2xm1` operand below 2^-68 by (seen on it).
const TINY_LN_2_BITS: u32 = 66;
/// The bits of the quotient, truncated, the reference takes as `fpatan`'s value below 2^-40 (seen on it).
const TINY_QUOTIENT_BITS: u32 = 67;

/// `b`'s top `bits` bits, truncated, exactly.
fn truncated(b: &BigFloat, bits: u32) -> BigFloat {
    let (top, exponent, _) = top_bits(b);
    signed(
        big(top >> (u128::BITS - bits), exponent - bits as i32),
        b.is_negative(),
    )
}

/// The reference's `f2xm1` of an operand below 2^-68: the operand times ln 2 truncated to 66 bits.
pub(crate) fn tiny_exponent_minus_one(x: Extended) -> Wide {
    settled(|p, cc| {
        let ln_2 = truncated(&cc.ln_2(GUESS_BITS, ROUND), TINY_LN_2_BITS);
        of_extended(x).mul(&ln_2, p, ROUND)
    })
}

/// The reference's `fpatan` of a positive `x` and a quotient below 2^-40: the quotient truncated to 67 bits.
pub(crate) fn tiny_arc_tangent(y: Extended, x: Extended) -> Wide {
    settled(|_, _| {
        let q = of_extended(y).div(&of_extended(x), FIRST_BITS, RoundingMode::ToZero);
        truncated(&q, TINY_QUOTIENT_BITS)
    })
}

/// Whether `|y / x|` is below `2^exponent`.
pub(crate) fn quotient_below(y: Extended, x: Extended, exponent: i32) -> bool {
    let q = of_extended(y).div(&of_extended(x), FIRST_BITS, ROUND);
    // `0.1… * 2^e` is below `2^exponent` for `e` up to `exponent`.
    q.exponent().is_some_and(|e| e <= exponent)
}

/// `n * π / 4`, as `fpatan` gives for its infinite and zero operands.
pub(crate) fn quarter_pi(n: i32) -> Wide {
    settled(|p, cc| {
        let pi = cc.pi(p, ROUND);
        let mut q = pi.mul(&signed(big(n.unsigned_abs().into(), 0), n < 0), p, ROUND);
        if let Some(e) = q.exponent() {
            q.set_exponent(e - QUARTER_SHIFT);
        }
        q
    })
}

/// A quarter is a shift of two.
const QUARTER_SHIFT: i32 = 2;
