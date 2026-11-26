//! AVX-VNNI's dot products, as the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! volume 2 describes VPDPBUSD, VPDPBUSDS, VPDPWSSD and VPDPWSSDS: each product is exact, their sum with the
//! destination's doubleword is exact, and only then wrapped or saturated to 32 bits.

use crate::planted::{Bug, is_active};
use crate::vector::{Lane, join, split, to_signed};

/// Each doubleword of `accumulator` plus the dot product of the sources' elements in it.
pub(crate) fn accumulate_dots(accumulator: u128, first: u128, second: u128, words: bool, saturate: bool) -> u128 {
    let (lane, per_doubleword) = if words { (Lane::Word, 2) } else { (Lane::Byte, 4) };
    let (first, second) = (split(lane, first), split(lane, second));
    let sums = split(Lane::Doubleword, accumulator).into_iter().enumerate().map(|(index, value)| {
        let products: i64 = (index * per_doubleword..(index + 1) * per_doubleword)
            .map(|element| to_factor(first[element], lane, !words) * to_factor(second[element], lane, false))
            .sum();
        let products = if is_active(Bug::DotProductsWrappedFirst) { i64::from(products as i32) } else { products };
        let sum = i64::from(value as u32 as i32) + products;
        let saturated = saturate && !is_active(Bug::DotSaturationWraps);
        let result = if saturated { sum.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) } else { sum };
        u64::from(result as u32)
    });
    join(Lane::Doubleword, sums)
}

/// An element as a factor: zero-extended where `unsigned`, else sign-extended.
fn to_factor(element: u64, lane: Lane, unsigned: bool) -> i64 {
    if unsigned && !is_active(Bug::DotUnsignedBytesSigned) { element as i64 } else { to_signed(element, lane.to_bits()) }
}
