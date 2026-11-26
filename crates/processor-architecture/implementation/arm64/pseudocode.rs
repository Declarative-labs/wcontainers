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
use crate::integer::{BitfieldFill, Comparison, FlagSetting, IntegerOperation, SelectAlternative};
#[cfg(verus_keep_ghost)]
use crate::operand::{Rotation, Signedness};

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

/// The product of [SMADDL, UMADDL, SMSUBL and UMSUBL](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SMADDL--Signed-multiply-add-long-):
/// `Int(operand1, unsigned) * Int(operand2, unsigned)` of the low 32 bits of each operand, as 64 bits.
pub open spec fn multiply_long_value(operand1: u64, operand2: u64, signedness: Signedness) -> int {
    match signedness {
        Signedness::Unsigned => uint(uint(operand1 as int, 32) * uint(operand2 as int, 32), 64),
        Signedness::Signed => uint(sint(operand1 as int, 32) * sint(operand2 as int, 32), 64),
    }
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

/// [ExtendReg](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-extendreg#impl-aarch64.ExtendReg.4)
/// of the low `n` bits of a register: its low `len` bits, at most `n - shift` of them, with `shift` zero bits appended,
/// zero- or sign-extended to `n` bits.
pub open spec fn extend_reg(x: int, unsigned: bool, len: nat, shift: nat, n: nat) -> int {
    let len = if len <= n - shift { len } else { (n - shift) as nat };
    let field = uint(x, len) * pow2(shift);
    if unsigned { field } else { uint(sint(field, len + shift), n) }
}

/// The second operand of an extended register form, as its decode gives it: UNDEFINED (none) for a shift above 4, and
/// the extend types in the order of their encodings, UXTB, UXTH, UXTW, UXTX, SXTB, SXTH, SXTW and SXTX.
pub open spec fn extended_register(x: int, option: nat, shift: nat, n: nat) -> Option<int> {
    if shift > 4 {
        None
    } else {
        Some(extend_reg(x, option < 4, pow2((3 + option % 4) as nat), shift, n))
    }
}

/// [HighestSetBit](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.HighestSetBit.1)
/// of a value: the position of its most significant one, or -1 where it has none.
pub open spec fn highest_set_bit(x: nat) -> int
    decreases x,
{
    if x == 0 { -1 } else { 1 + highest_set_bit(x / 2) }
}

/// [Replicate](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-vector#impl-shared.Replicate.2)
/// of the low `esize` bits of `x`, `count` times: `x:Replicate(x, count - 1)`, the first copy the most significant.
pub open spec fn replicate(x: int, esize: nat, count: nat) -> int
    decreases count,
{
    if count == 0 { 0 } else { uint(x, esize) * pow2(((count - 1) * esize) as nat) + replicate(x, esize, (count - 1) as nat) }
}

/// [DecodeBitMasks](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-bitmasks#impl-aarch64.DecodeBitMasks.5)
/// of immN, the 6-bit imms and immr, whether it decodes an immediate, and `m` bits: wmask and tmask, or none
/// (UNDEFINED). Arm asserts that `m` holds an element, which the decodes of its callers ensure.
pub open spec fn decode_bit_masks(imm_n: bool, imms: nat, immr: nat, immediate: bool, m: nat) -> Option<(int, int)> {
    // len = HighestSetBit(immN:NOT(imms)).
    let len = highest_set_bit(((if imm_n { pow2(6) } else { 0 }) + ones(6) - uint(imms as int, 6)) as nat);
    if len < 1 {
        None
    } else {
        // levels = ZeroExtend(Ones(len), 6), so `imms AND levels` is the low len bits of imms.
        let levels = ones(len as nat);
        let s = uint(imms as int, len as nat);
        let r = uint(immr as int, len as nat);
        if immediate && s == levels {
            None
        } else {
            let esize = pow2(len as nat);
            // diff = s - r, a 6-bit subtraction with borrow, of which d takes the low len bits.
            let d = uint(s - r, len as nat);
            let welem = ones((s + 1) as nat);
            let telem = ones((d + 1) as nat);
            Some((replicate(ror(welem, r as nat, esize), esize, m / esize), replicate(telem, esize, m / esize)))
        }
    }
}

/// The immediate of [AND (immediate)](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/AND--immediate---Bitwise-AND--immediate--),
/// ORR, EOR and ANDS (immediate), as their decode gives it from the field N:immr:imms: the wmask of
/// DecodeBitMasks(N, imms, immr, TRUE, datasize), or none (UNDEFINED).
pub open spec fn bitmask_immediate_value(field: nat, n: nat) -> Option<int> {
    let imms = field % 64;
    let immr = (field / 64) % 64;
    let imm_n = (field / pow2(12)) % 2 == 1;
    match decode_bit_masks(imm_n, imms, immr, true, n) {
        Some((wmask, _)) => Some(wmask),
        None => None,
    }
}

/// The position of a move wide's immediate, as [MOVZ](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MOVZ--Move-wide-with-zero-)'s
/// decode gives it from hw: `UInt(hw:'0000')`, or none (UNDEFINED) where it is not below the width, which is hw<1> set
/// in the 32-bit forms.
pub open spec fn wide_immediate_position(hw: nat, n: nat) -> Option<nat> {
    if hw * 16 >= n { None } else { Some(hw * 16) }
}

/// The Operation of [MOVN](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MOVN--Move-wide-with-NOT-),
/// [MOVZ](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MOVZ--Move-wide-with-zero-) and
/// [MOVK](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MOVK--Move-wide-with-keep-) on
/// `n` bits: imm16 at bits `pos + 15:pos` of zeros, of zeros then inverted, or of the destination, whose other bits
/// it keeps.
pub open spec fn move_wide(operation: IntegerOperation, n: nat, kept: u64, imm16: int, pos: nat) -> int {
    let placed = imm16 * pow2(pos);
    match operation {
        IntegerOperation::MoveWideZero => placed,
        IntegerOperation::MoveWideNot => ones(n) - placed,
        IntegerOperation::MoveWideKeep => uint(kept as int, n) - uint(kept as int / (pow2(pos) as int), 16) * pow2(pos) + placed,
        _ => 0,
    }
}

/// The Operation of [SBFM](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SBFM--Signed-bitfield-move-),
/// [BFM](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BFM--Bitfield-move-) and
/// [UBFM](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/UBFM--Unsigned-bitfield-move-) on
/// `n` bits after their decode, `(wmask, tmask) = DecodeBitMasks(N, imms, immr, FALSE, datasize)` with N set for 64
/// bits: none (UNDEFINED) where it refuses, else `(top AND NOT(tmask)) OR (bot AND tmask)`.
pub open spec fn bitfield_move(fill: BitfieldFill, n: nat, kept: u64, source: u64, immr: nat, imms: nat) -> Option<int> {
    match decode_bit_masks(n == 64, imms, immr, false, n) {
        None => None,
        Some((wmask, tmask)) => {
            let (wmask, tmask) = (wmask as u64, tmask as u64);
            // dst is Zeros() for SBFM and UBFM, X[d] for BFM.
            let dst = match fill {
                BitfieldFill::DestinationBits => kept,
                _ => 0u64,
            };
            let bot = (dst & !wmask) | ((ror(uint(source as int, n), immr, n) as u64) & wmask);
            // top is Replicate(src<S>) for SBFM, dst for BFM and UBFM.
            let top = match fill {
                BitfieldFill::SignBits => if uint(source as int / (pow2(imms) as int), 1) == 1 { ones(n) as u64 } else { 0u64 },
                _ => dst,
            };
            Some(uint(((top & !tmask) | (bot & tmask)) as int, n))
        },
    }
}

/// The Operation of [EXTR](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/EXTR--Extract-register-)
/// on `n`-bit registers: `concat = operand1:operand2; result = concat<lsb + datasize - 1:lsb>`.
pub open spec fn extract(n: nat, operand1: u64, operand2: u64, lsb: nat) -> int {
    let concat = uint(operand1 as int, n) * pow2(n) + uint(operand2 as int, n);
    uint(concat / (pow2(lsb) as int), n)
}

/// The Operation of [CSEL](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CSEL--Conditional-select-),
/// [CSINC](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CSINC--Conditional-select-increment-),
/// [CSINV](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CSINV--Conditional-select-invert-)
/// and [CSNEG](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CSNEG--Conditional-select-negation-)
/// on `n` bits: operand1 where the condition holds, else operand2, inverted for CSINV and CSNEG, plus one for CSINC
/// and CSNEG.
pub open spec fn conditional_select(alternative: SelectAlternative, n: nat, operand1: u64, operand2: u64, cond: nat, nzcv: Nzcv) -> int {
    if condition_holds(cond, nzcv) {
        uint(operand1 as int, n)
    } else {
        match alternative {
            SelectAlternative::Select => uint(operand2 as int, n),
            SelectAlternative::Increment => uint(operand2 as int + 1, n),
            SelectAlternative::Invert => uint(!operand2 as int, n),
            SelectAlternative::Negate => uint(!operand2 as int + 1, n),
        }
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
/// [BIC and BICS](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BICS--shifted-register---Bitwise-bit-clear--shifted-register---setting-flags-),
/// [ORR](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ORR--shifted-register---Bitwise-OR--shifted-register--),
/// [ORN](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ORN--shifted-register---Bitwise-OR-NOT--shifted-register--),
/// [EOR](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/EOR--shifted-register---Bitwise-exclusive-OR--shifted-register--)
/// and [EON](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/EON--Bitwise-exclusive-OR-NOT--shifted-register--)
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
        IntegerOperation::Or => (uint((operand1 | operand2) as int, n), None),
        IntegerOperation::OrNot => (uint((operand1 | !operand2) as int, n), None),
        IntegerOperation::ExclusiveOr => (uint((operand1 ^ operand2) as int, n), None),
        IntegerOperation::ExclusiveOrNot => (uint((operand1 ^ !operand2) as int, n), None),
        _ => (0, None),
    }
}

} // verus!
