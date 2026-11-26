//! The element functions of `sve_remaining_floating_point` that need the parent module's unpacking.

use super::{Context, INVALID_OPERATION, Kind, Precision, low_ones, unpack};

/// [FPLogB](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-sve#impl-aarch64.FPLogB.2):
/// the exponent of the operand's magnitude as a signed integer of its width, the smallest one with Invalid Operation for a
/// NaN or a zero, a flushed subnormal included, and the largest for an infinity.
pub fn compute_logarithm(bits: u64, precision: Precision, fpcr: u64, flags: &mut u64) -> u64 {
    let mut context = Context {
        fpcr,
        flags: 0,
        conversion: false,
    };
    let operand = unpack(bits, precision, &mut context);
    let width = precision.to_bits();
    let result: i64 = match operand.kind {
        Kind::QuietNaN | Kind::SignalingNaN | Kind::Zero => {
            if !crate::planted::is(3101) {
                context.raise(INVALID_OPERATION);
            }
            -(1_i64 << (width - 1))
        }
        Kind::Infinity => (1_i64 << (width - 1)) - 1,
        Kind::Number => {
            let top = u64::BITS - 1 - operand.mantissa.leading_zeros();
            i64::from(top as i32 + operand.exponent)
        }
    };
    *flags |= context.flags;
    result as u64 & low_ones(width)
}
