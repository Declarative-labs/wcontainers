//! The fused multiply-add of the VFMADD family: the product and the addend summed exactly and rounded once, as
//! IEEE 754's fusedMultiplyAdd and the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! volume 2 give it, with MXCSR's denormals-are-zeros, flush-to-zero and masks as the legacy arithmetic reads them.

use crate::float::{
    Class, DENORMAL, DENORMALS_ARE_ZEROS, Exact, INVALID, LaneOutcome, Operand, Precision, Rounding, read_rounding,
    round,
};
use crate::planted::{Bug, is_active};

/// Bits the exact sum is aligned to, below u128's top two.
const SUM_BITS: i32 = 126;

/// The fused `±(a × b) ± c` of one element, NaNs taken in the order the operands are given.
#[expect(clippy::too_many_arguments, reason = "each is one input of the computation")]
pub fn multiply_add(
    a: u64,
    b: u64,
    c: u64,
    negate_product: bool,
    negate_addend: bool,
    precision: Precision,
    mxcsr: u32,
    nan_order: [usize; 3],
) -> LaneOutcome {
    let daz = mxcsr & DENORMALS_ARE_ZEROS != 0;
    let operands = [a, b, c].map(|bits| Operand::read(bits, precision, daz));
    let done = |bits: u64, before: u32| LaneOutcome { bits, before, after: 0 };
    let signaling = operands.iter().any(|operand| matches!(operand.class, Class::Nan { signaling: true }));
    if let Some(nan) = nan_order.iter().map(|&at| operands[at]).find(Operand::is_nan) {
        return done(nan.bits | precision.to_quiet_bit(), if signaling { INVALID } else { 0 });
    }
    let denormal = if operands.iter().any(Operand::is_denormal) { DENORMAL } else { 0 };
    let [a, b, c] = operands;
    let product_negative = (a.negative != b.negative) != negate_product;
    let addend_negative = c.negative != negate_addend;
    let sign_of = |negative: bool| if negative { precision.to_sign_bit() } else { 0 };
    let infinity = |negative: bool| sign_of(negative) | precision.to_infinity();
    let invalid = done(precision.to_default_nan(), INVALID);
    let rounding = read_rounding(mxcsr);
    let zero_sum = |first: bool, second: bool| {
        sign_of(if first == second { first } else { rounding == Rounding::Down })
    };
    let product_infinite = matches!(a.class, Class::Infinity) || matches!(b.class, Class::Infinity);
    let product_zero = matches!(a.class, Class::Zero) || matches!(b.class, Class::Zero);
    if product_infinite && product_zero {
        return invalid;
    }
    if product_infinite {
        if c.class == Class::Infinity && addend_negative != product_negative {
            return invalid;
        }
        return done(infinity(product_negative), denormal);
    }
    if c.class == Class::Infinity {
        return done(infinity(addend_negative), denormal);
    }
    let to_exact = |operand: Operand, negative: bool| match operand.class {
        Class::Finite { significand, exponent, .. } => Some(Exact { negative, significand, exponent, sticky: false }),
        _ => None,
    };
    let product = match (a.class, b.class) {
        (Class::Finite { significand: x, exponent: ex, .. }, Class::Finite { significand: y, exponent: ey, .. }) => {
            Some(Exact { negative: product_negative, significand: x * y, exponent: ex + ey, sticky: false })
        }
        _ => None,
    };
    let addend = to_exact(c, addend_negative);
    let exact = match (product, addend) {
        (None, None) => return done(zero_sum(product_negative, addend_negative), denormal),
        (Some(value), None) | (None, Some(value)) => value,
        (Some(product), Some(addend)) => match add_exactly(product, addend) {
            Some(sum) => sum,
            None => return done(sign_of(rounding == Rounding::Down), denormal),
        },
    };
    let (bits, after) = round(exact, precision, rounding, mxcsr);
    LaneOutcome { bits, before: denormal, after }
}

/// The exact sum of two finite nonzero values, aligned so that the larger's top bit is bit 126 and the smaller's
/// lost bits are sticky; `None` for an exact zero.
fn add_exactly(first: Exact, second: Exact) -> Option<Exact> {
    let top = |value: &Exact| value.exponent + (u128::BITS - value.significand.leading_zeros()) as i32;
    let common = top(&first).max(top(&second)) - SUM_BITS;
    let align = |value: &Exact| -> (u128, bool) {
        let shift = value.exponent - common;
        if shift >= 0 {
            (value.significand << shift, false)
        } else if -shift >= u128::BITS as i32 {
            (0, true)
        } else {
            let shifted = value.significand >> -shift;
            (shifted, shifted << -shift != value.significand)
        }
    };
    let ((a, a_sticky), (b, b_sticky)) = (align(&first), align(&second));
    if first.negative == second.negative {
        return Some(Exact { negative: first.negative, significand: a + b, exponent: common, sticky: a_sticky || b_sticky });
    }
    let (large, small, small_sticky, negative) = match a.cmp(&b) {
        core::cmp::Ordering::Equal if !a_sticky && !b_sticky => return None,
        core::cmp::Ordering::Less => (b, a, a_sticky, second.negative),
        core::cmp::Ordering::Equal if a_sticky => (b, a, a_sticky, second.negative),
        _ => (a, b, b_sticky, first.negative),
    };
    let lost = u128::from(small_sticky && !is_active(Bug::EvexFusedStickyIgnored));
    Some(Exact { negative, significand: large - small - lost, exponent: common, sticky: small_sticky })
}
