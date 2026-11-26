//! Arm's shared pseudocode that the integer instructions use, as specification functions: each is the function of
//! the [shared pseudocode](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer)
//! it names, read over integers, with a bit string of `n` bits read as its unsigned value. A bit string of at most 64
//! bits is also held in the low bits of a `u64`, where AND and NOT are Rust's `&` and `!`.

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::pow2;
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::condition_flags::{
    IMMEDIATE_CARRY, IMMEDIATE_NEGATIVE, IMMEDIATE_OVERFLOW, IMMEDIATE_ZERO, Nzcv,
};
#[cfg(verus_keep_ghost)]
use crate::integer::{Comparison, FlagSetting, IntegerOperation};
#[cfg(verus_keep_ghost)]
use crate::operand::Rotation;

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

/// [LSL](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.LSL.2)
/// of the low `n` bits of `x` by `shift`.
pub open spec fn lsl(x: int, shift: nat, n: nat) -> int {
    uint(x * pow2(shift), n)
}

/// [LSR](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.LSR.2)
/// of the low `n` bits of `x` by `shift`.
pub open spec fn lsr(x: int, shift: nat, n: nat) -> int {
    uint(x, n) / (pow2(shift) as int)
}

/// [ASR](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.ASR.2)
/// of the low `n` bits of `x` by `shift`: SInt divided by 2^shift, rounded down.
pub open spec fn asr(x: int, shift: nat, n: nat) -> int {
    uint(sint(x, n) / (pow2(shift) as int), n)
}

/// [ROR](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.ROR.2)
/// of the low `n` bits of `x` by `shift`: LSR(x, m) OR LSL(x, n - m) for m = shift MOD n, whose bits do not overlap,
/// so that their OR is their sum.
pub open spec fn ror(x: int, shift: nat, n: nat) -> int {
    let m = shift % n;
    if m == 0 { uint(x, n) } else { lsr(x, m, n) + lsl(x, (n - m) as nat, n) }
}

/// [ShiftReg](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-shiftreg#impl-aarch64.ShiftReg.4)
/// of the low `n` bits of a register, the shift types in the order of their encodings: LSL, LSR, ASR, ROR.
pub open spec fn shift_reg(x: int, shift_type: nat, amount: nat, n: nat) -> int {
    if shift_type == 0 {
        lsl(x, amount, n)
    } else if shift_type == 1 {
        lsr(x, amount, n)
    } else if shift_type == 2 {
        asr(x, amount, n)
    } else {
        ror(x, amount, n)
    }
}

/// The second operand of a shifted register form, as its decode gives it: UNDEFINED (none) for an amount of `n` bits
/// or more, and for ROR where the instruction reserves it, as [ADD (shifted register)](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--shifted-register---Add-optionally-shifted-register-) does.
pub open spec fn shifted_register(x: int, shift_type: nat, amount: nat, n: nat, rotation: Rotation) -> Option<int> {
    if amount >= n || (shift_type == 3 && rotation == Rotation::Reserved) {
        None
    } else {
        Some(shift_reg(uint(x, n), shift_type, amount, n))
    }
}

/// [ConditionHolds](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-system#impl-shared.ConditionHolds.1)
/// of a condition's 4-bit encoding.
pub open spec fn condition_holds(cond: nat, nzcv: Nzcv) -> bool {
    // cond<3:1> picks the test, and cond<0> inverts it except for 1111.
    let tested = cond / 2;
    let result = if tested == 0 {
        nzcv.zero
    } else if tested == 1 {
        nzcv.carry
    } else if tested == 2 {
        nzcv.negative
    } else if tested == 3 {
        nzcv.overflow
    } else if tested == 4 {
        nzcv.carry && !nzcv.zero
    } else if tested == 5 {
        nzcv.negative == nzcv.overflow
    } else if tested == 6 {
        nzcv.negative == nzcv.overflow && !nzcv.zero
    } else {
        true
    };
    if cond % 2 == 1 && cond != 15 { !result } else { result }
}

/// The immediate of [ADD (immediate)](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--immediate---Add-immediate-value-)
/// and SUB, ADDS and SUBS (immediate), as their decode gives it: imm12, or imm12:Zeros(12) where sh is set.
pub open spec fn arithmetic_immediate_value(imm12: nat, sh: nat) -> int {
    if sh == 0 { imm12 as int } else { (imm12 * pow2(12)) as int }
}

/// The NZCV an immediate gives, its bits N, Z, C and V from the most significant.
pub open spec fn nzcv_immediate(nzcv: u64) -> Nzcv {
    Nzcv {
        negative: nzcv & IMMEDIATE_NEGATIVE != 0,
        zero: nzcv & IMMEDIATE_ZERO != 0,
        carry: nzcv & IMMEDIATE_CARRY != 0,
        overflow: nzcv & IMMEDIATE_OVERFLOW != 0,
    }
}

/// The Operation of [CCMN](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CCMN--register---Conditional-compare-negative--register--)
/// and [CCMP](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CCMP--register---Conditional-compare--register--),
/// register and immediate: the flags of the sum or difference where the condition holds, the immediate's otherwise.
pub open spec fn conditional_compare(comparison: Comparison, n: nat, operand1: u64, operand2: u64, nzcv: u64, cond: nat, flags: Nzcv) -> Nzcv {
    if condition_holds(cond, flags) {
        let sum = match comparison {
            Comparison::Sum => add_with_carry(operand1 as int, operand2 as int, false, n),
            Comparison::Difference => add_with_carry(operand1 as int, !operand2 as int, true, n),
        };
        Nzcv { negative: sum.1, zero: sum.2, carry: sum.3, overflow: sum.4 }
    } else {
        nzcv_immediate(nzcv)
    }
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
