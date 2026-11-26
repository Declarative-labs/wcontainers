//! Arm's shared pseudocode that the integer instructions use, as specification functions: each is the function of
//! the [shared pseudocode](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer)
//! it names, read over integers, with a bit string of `n` bits read as its unsigned value. A bit string of at most 64
//! bits is also held in the low bits of a `u64`, where AND and NOT are Rust's `&` and `!`.

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::pow2;
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::condition_flags::Nzcv;
#[cfg(verus_keep_ghost)]
use crate::integer::{FlagSetting, IntegerOperation};

verus! {

/// [UInt](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.UInt.1)
/// of the low `n` bits of `x`.
pub open spec fn uint(x: int, n: nat) -> int {
    x % (pow2(n) as int)
}

/// [SInt](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.SInt.1)
/// of the low `n` bits of `x`: their unsigned value, less 2^n when the top one is set.
pub open spec fn sint(x: int, n: nat) -> int {
    if n > 0 && uint(x, n) >= pow2((n - 1) as nat) { uint(x, n) - pow2(n) } else { uint(x, n) }
}

/// [Ones](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-common#impl-shared.Ones.1)
/// of `n` bits, as an unsigned value.
pub open spec fn ones(n: nat) -> int {
    pow2(n) - 1
}

/// [IsZero](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-common#impl-shared.IsZero.1)
/// of the low `n` bits of `x`.
pub open spec fn is_zero(x: int, n: nat) -> bool {
    uint(x, n) == 0
}

/// [AddWithCarry](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.AddWithCarry.3)
/// of the low `n` bits of `x` and `y`: the result's unsigned value, then N, Z, C and V.
pub open spec fn add_with_carry(x: int, y: int, carry_in: bool, n: nat) -> (int, bool, bool, bool, bool) {
    let carry = if carry_in { 1int } else { 0int };
    let unsigned_sum = uint(x, n) + uint(y, n) + carry;
    let signed_sum = sint(x, n) + sint(y, n) + carry;
    let result = uint(unsigned_sum, n);
    // N is the result's top bit, which is set exactly when SInt of the result is negative.
    (result, sint(result, n) < 0, is_zero(result, n), uint(result, n) != unsigned_sum, sint(result, n) != signed_sum)
}

/// The low `n` bits of a logical instruction's result, then the N, Z, C and V it sets: `result<N-1>:IsZeroBit(result):'00'`.
pub open spec fn logical_result(x: int, n: nat) -> (int, bool, bool, bool, bool) {
    (uint(x, n), sint(x, n) < 0, is_zero(x, n), false, false)
}

/// The result written to the destination and, where the instruction sets flags, PSTATE.NZCV after it, from its result
/// and flags.
pub open spec fn written(outcome: (int, bool, bool, bool, bool), setting: FlagSetting) -> (int, Option<Nzcv>) {
    (
        outcome.0,
        match setting {
            FlagSetting::Sets => Some(Nzcv { negative: outcome.1, zero: outcome.2, carry: outcome.3, overflow: outcome.4 }),
            FlagSetting::Preserves => None,
        },
    )
}

/// The Operation of [ADD and ADDS](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADDS--shifted-register---Add-optionally-shifted-register--setting-flags-),
/// [SUB and SUBS](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SUBS--shifted-register---Subtract-optionally-shifted-register--setting-flags-),
/// [ADC and ADCS](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADCS--Add-with-carry--setting-flags-),
/// [SBC and SBCS](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SBCS--Subtract-with-carry--setting-flags-),
/// [AND and ANDS](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ANDS--shifted-register---Bitwise-AND--shifted-register---setting-flags-)
/// and [BIC and BICS](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BICS--shifted-register---Bitwise-bit-clear--shifted-register---setting-flags-)
/// on `n`-bit operands, after their decode, with PSTATE.NZCV before it.
pub open spec fn arithmetic_or_logical(
    operation: IntegerOperation,
    n: nat,
    operand1: u64,
    operand2: u64,
    nzcv: Nzcv,
) -> (int, Option<Nzcv>) {
    match operation {
        IntegerOperation::Add(setting) => written(add_with_carry(operand1 as int, operand2 as int, false, n), setting),
        IntegerOperation::Subtract(setting) => written(add_with_carry(operand1 as int, !operand2 as int, true, n), setting),
        IntegerOperation::AddWithCarry(setting) => written(add_with_carry(operand1 as int, operand2 as int, nzcv.carry, n), setting),
        IntegerOperation::SubtractWithCarry(setting) => written(add_with_carry(operand1 as int, !operand2 as int, nzcv.carry, n), setting),
        IntegerOperation::And(setting) => written(logical_result((operand1 & operand2) as int, n), setting),
        IntegerOperation::AndNot(setting) => written(logical_result((operand1 & !operand2) as int, n), setting),
        _ => (0, None),
    }
}

} // verus!
