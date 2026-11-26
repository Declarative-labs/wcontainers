//! The EVEX floating-point compares into an opmask register, with the 32 predicates of the
//! [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) volume 2
//! (CMPPS): the low four bits name the relation, bit 4 swaps whether a quiet NaN signals.

use crate::float::{DENORMAL, DENORMALS_ARE_ZEROS, INVALID, LaneOutcome, Operand, Precision, compare_less, is_equal};
use crate::planted::{Bug, is_active};

/// One element's predicate: bit 0 of the outcome's bits set where it holds; invalid for a signaling NaN, or for any
/// NaN under a signaling predicate, else denormal for a denormal operand.
pub fn compare(first: u64, second: u64, precision: Precision, mxcsr: u32, predicate: u8) -> LaneOutcome {
    let daz = mxcsr & DENORMALS_ARE_ZEROS != 0;
    let (first, second) = (Operand::read(first, precision, daz), Operand::read(second, precision, daz));
    let unordered = first.is_nan() || second.is_nan();
    let less = !unordered && compare_less(first, second);
    let equal = !unordered && is_equal(first, second);
    let greater = !unordered && !less && !equal;
    let relation = predicate & 0xF;
    let holds = match relation {
        0 => equal,
        1 => less,
        2 => less || equal,
        3 => unordered,
        4 => !equal,
        5 => !less,
        6 => !(less || equal),
        7 => !unordered,
        8 => equal || unordered,
        9 => less || unordered,
        10 => !greater,
        11 => false,
        12 => !equal && !unordered,
        13 => greater || equal,
        14 => greater,
        _ => true,
    };
    let quiet_signals = matches!(relation, 1 | 2 | 5 | 6 | 9 | 10 | 13 | 14) != (predicate & 0x10 != 0 && !is_active(Bug::EvexCompareSignalingBitIgnored));
    let signaling_nan = [first, second].iter().any(|operand| matches!(operand.class, crate::float::Class::Nan { signaling: true }));
    let before = if unordered {
        if signaling_nan || quiet_signals { INVALID } else { 0 }
    } else if first.is_denormal() || second.is_denormal() {
        DENORMAL
    } else {
        0
    };
    LaneOutcome { bits: u64::from(holds), before, after: 0 }
}
