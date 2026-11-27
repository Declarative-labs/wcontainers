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
use crate::integer::{
    BitfieldFill, Comparison, CrcPolynomial, FlagSetting, IntegerOperation, SelectAlternative,
};
#[cfg(verus_keep_ghost)]
use crate::operand::{Indexing, Rotation, Signedness};

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

/// The assignment `x<lowest + width - 1:lowest> = value` to a bit string held in a `u64`, `mask` being Ones(width):
/// the bits of the slice replaced by the low bits of `value`, every other bit kept.
pub open spec fn set_bits(x: u64, lowest: u32, mask: u64, value: u64) -> u64 {
    (x & !(mask << lowest)) | ((value & mask) << lowest)
}

/// The Operation of [RBIT](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/RBIT--Reverse-bits-)
/// on `n` bits after its first `steps` steps, on a result whose bits are all zeros before the first:
/// `for i = 0 to datasize-1: result<datasize-1-i> = operand<i>`.
pub open spec fn reverse_bits_steps(operand: u64, n: nat, steps: nat) -> u64
    decreases steps,
{
    if steps == 0 {
        0
    } else {
        let i = (steps - 1) as nat;
        set_bits(reverse_bits_steps(operand, n, i), (n - 1 - i) as u32, 1, operand >> (i as u32))
    }
}

/// The result of [RBIT](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/RBIT--Reverse-bits-)
/// on `n` bits: every step of its loop.
pub open spec fn reverse_bits_value(operand: u64, n: nat) -> u64 {
    reverse_bits_steps(operand, n, n)
}

/// The byte `operand<index+7:index>` goes to in the Operation of [REV16](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/REV16--Reverse-bytes-in-16-bit-halfwords-),
/// [REV32](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/REV32--Reverse-bytes-in-32-bit-words-)
/// and [REV](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/REV--Reverse-bytes-), for the
/// byte `byte = index DIV 8`: its loop sets `rev_index = index + ((elements_per_container - 1) * 8)` at the start of
/// each container and lowers it by 8 for each byte after, so the byte lands at the same container's other end.
pub open spec fn reversed_byte(byte: nat, elements_per_container: nat) -> nat {
    (byte / elements_per_container) * elements_per_container + (elements_per_container - 1 - byte % elements_per_container) as nat
}

/// The Operation of REV16, REV32 and REV after the bytes before `byte` in the order of its two loops (containers, then
/// the bytes of each), on a result whose bits are all zeros before the first:
/// `result<rev_index+7:rev_index> = operand<index+7:index>`.
pub open spec fn reverse_bytes_steps(operand: u64, elements_per_container: nat, byte: nat) -> u64
    decreases byte,
{
    if byte == 0 {
        0
    } else {
        let index = (byte - 1) as nat;
        set_bits(
            reverse_bytes_steps(operand, elements_per_container, index),
            (8 * reversed_byte(index, elements_per_container)) as u32,
            0xff,
            operand >> ((8 * index) as u32),
        )
    }
}

/// The result of REV16, REV32 and REV on `n` bits, with containers of `container_size` bits: every step of their
/// loops, `containers = datasize DIV container_size` containers of `elements_per_container = container_size DIV 8`
/// bytes.
pub open spec fn reverse_bytes_value(operand: u64, n: nat, container_size: nat) -> u64 {
    reverse_bytes_steps(operand, container_size / 8, (n / container_size) * (container_size / 8))
}

/// [CountLeadingZeroBits](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.CountLeadingZeroBits.1)
/// of an `n`-bit value: `N - (HighestSetBit(x) + 1)`, the Operation of [CLZ](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CLZ--Count-leading-zeros-).
pub open spec fn count_leading_zero_bits(x: nat, n: nat) -> int {
    n - (highest_set_bit(x) + 1)
}

/// [CountLeadingSignBits](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.CountLeadingSignBits.1)
/// of the low `n` bits of `x`, the Operation of [CLS](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CLS--Count-leading-sign-bits-):
/// `CountLeadingZeroBits(x<N-1:1> EOR x<N-2:0>)`, both slices of `n - 1` bits.
pub open spec fn count_leading_sign_bits(x: u64, n: nat) -> int {
    count_leading_zero_bits((((x >> 1u32) ^ x) & (ones((n - 1) as nat) as u64)) as nat, (n - 1) as nat)
}

/// The shift type of [LSLV](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LSLV--Logical-shift-left-variable-),
/// [LSRV](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LSRV--Logical-shift-right-variable-),
/// [ASRV](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ASRV--Arithmetic-shift-right-variable-)
/// and [RORV](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/RORV--Rotate-right-variable-),
/// as their decode gives it from op2: `DecodeShift(op2)`, in the order of the shift types' encodings.
pub open spec fn variable_shift_type(operation: IntegerOperation) -> nat {
    match operation {
        IntegerOperation::ShiftLeftVariable => 0,
        IntegerOperation::ShiftRightLogicalVariable => 1,
        IntegerOperation::ShiftRightArithmeticVariable => 2,
        _ => 3,
    }
}

/// The Operation of LSLV, LSRV, ASRV and RORV on `n` bits:
/// `ShiftReg(n, shift_type, UInt(operand2) MOD datasize, datasize)`.
pub open spec fn variable_shift(operation: IntegerOperation, operand1: u64, operand2: u64, n: nat) -> int {
    shift_reg(uint(operand1 as int, n), variable_shift_type(operation), (uint(operand2 as int, n) % (n as int)) as nat, n)
}

/// [RoundTowardsZero](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.RoundTowardsZero.1)
/// of the quotient of two integers, the second not zero: the quotient of their magnitudes, negated where their signs
/// differ.
pub open spec fn round_towards_zero(dividend: int, divisor: int) -> int {
    let magnitude = (if dividend < 0 { -dividend } else { dividend }) / (if divisor < 0 { -divisor } else { divisor });
    if (dividend < 0) == (divisor < 0) { magnitude } else { -magnitude }
}

/// The Operation of [UDIV](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/UDIV--Unsigned-divide-)
/// and [SDIV](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SDIV--Signed-divide-) on `n`
/// bits: zero where operand2 is zero, else `RoundTowardsZero(Real(Int(operand1, unsigned)) / Real(Int(operand2,
/// unsigned)))`, of which the destination takes the low `n` bits.
pub open spec fn divide(signedness: Signedness, operand1: u64, operand2: u64, n: nat) -> int {
    let (dividend, divisor) = match signedness {
        Signedness::Unsigned => (uint(operand1 as int, n), uint(operand2 as int, n)),
        Signedness::Signed => (sint(operand1 as int, n), sint(operand2 as int, n)),
    };
    if is_zero(operand2 as int, n) { 0 } else { uint(round_towards_zero(dividend, divisor), n) }
}

/// The Operation of [MADD](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MADD--Multiply-add-)
/// and [MSUB](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MSUB--Multiply-subtract-) on
/// `n` bits: `UInt(operand3) + (UInt(operand1) * UInt(operand2))`, or its difference, of which the destination takes
/// the low `n` bits.
pub open spec fn multiply_add(subtract: bool, operand1: u64, operand2: u64, operand3: u64, n: nat) -> int {
    let product = uint(operand1 as int, n) * uint(operand2 as int, n);
    uint(if subtract { uint(operand3 as int, n) - product } else { uint(operand3 as int, n) + product }, n)
}

/// The product of [SMADDL, UMADDL, SMSUBL and UMSUBL](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SMADDL--Signed-multiply-add-long-)
/// before any bits are dropped: `Int(operand1, unsigned) * Int(operand2, unsigned)` of the 32-bit operands.
pub open spec fn long_product(operand1: u64, operand2: u64, signedness: Signedness) -> int {
    match signedness {
        Signedness::Unsigned => uint(operand1 as int, 32) * uint(operand2 as int, 32),
        Signedness::Signed => sint(operand1 as int, 32) * sint(operand2 as int, 32),
    }
}

/// The Operation of SMADDL, UMADDL, SMSUBL and UMSUBL: `Int(operand3, unsigned) + (Int(operand1, unsigned) *
/// Int(operand2, unsigned))`, or its difference, of which the destination takes the low 64 bits (the 64-bit operand3's
/// sign does not change them).
pub open spec fn multiply_add_long(subtract: bool, operand1: u64, operand2: u64, operand3: u64, signedness: Signedness) -> int {
    let product = long_product(operand1, operand2, signedness);
    uint(if subtract { operand3 - product } else { operand3 + product }, 64)
}

/// The Operation of [UMULH](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/UMULH--Unsigned-multiply-high-)
/// and [SMULH](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SMULH--Signed-multiply-high-):
/// `result = Int(operand1, unsigned) * Int(operand2, unsigned); X[d, 64] = result<127:64>`, bits 127 to 64 of the
/// product's two's complement, which is the product divided by 2^64 rounded down.
pub open spec fn multiply_high(operand1: u64, operand2: u64, signedness: Signedness) -> int {
    let product = match signedness {
        Signedness::Unsigned => uint(operand1 as int, 64) * uint(operand2 as int, 64),
        Signedness::Signed => sint(operand1 as int, 64) * sint(operand2 as int, 64),
    };
    uint(product / (pow2(64) as int), 64)
}

/// The offset of a load or store with an immediate, the `width`-bit `imm` [ZeroExtend](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate--)ed
/// or SignExtended to 64 bits and shifted left by the scale, as their decodes give it: `offset = LSL(ZeroExtend(imm12,
/// 64), scale)` for an unsigned offset, `offset = SignExtend(imm9, 64)` for a signed byte offset and `offset =
/// LSL(SignExtend(imm7, 64), scale)` for a pair.
pub open spec fn immediate_offset(imm: int, width: nat, signedness: Signedness, scale: nat) -> int {
    let extended = match signedness {
        Signedness::Unsigned => uint(imm, width),
        Signedness::Signed => sint(imm, width),
    };
    lsl(extended, scale, 64)
}

/// The scale of a load or store of `bytes` bytes, the logarithm of its size: `scale = UInt(size)`, or 4 for 16 bytes.
pub open spec fn access_scale(bytes: nat) -> nat {
    highest_set_bit(bytes) as nat
}

/// The offset of [LDR (register)](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--register---Load-register--register--)
/// and the other loads and stores with a register offset: `ExtendReg(m, extend_type, shift, 64)` with `shift = if S ==
/// '1' then scale else 0`, the extend type UXTW, LSL (UXTX), SXTW or SXTX from option: option<2> unsigned where clear,
/// option<0> the doubleword where set.
pub open spec fn register_offset(x: u64, signed: bool, doubleword: bool, scaled: bool, bytes: nat) -> int {
    extend_reg(x as int, !signed, if doubleword { 64 } else { 32 }, if scaled { access_scale(bytes) } else { 0 }, 64)
}

/// The address a load or store accesses from its base: `if !postindex then address = address + offset`, a 64-bit
/// sum, and for a literal, whose base is its own address, always the sum.
pub open spec fn accessed_address(base: u64, offset: u64, indexing: Indexing) -> int {
    match indexing {
        Indexing::PostIndex => base as int,
        _ => uint(base + offset, 64),
    }
}

/// What a load or store with writeback writes to its base: `if postindex then address = address + offset; ...
/// X[n, 64] = address`, the base plus the offset however it is indexed.
pub open spec fn written_back_address(base: u64, offset: u64) -> int {
    uint(base + offset, 64)
}

/// The assignment `x<width-1:0> = value<width-1:0>` to a bit string of up to 128 bits held in a `u128`, `mask` being
/// Ones(width).
pub open spec fn set_low_bits_128(x: u128, mask: u128, value: u128) -> u128 {
    (x & !mask) | (value & mask)
}

/// [Poly32Mod2](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-crc#impl-shared.Poly32Mod2.2)
/// of an `n`-bit `data_in` after the first `steps` steps of its loop:
/// `for i = N-1 downto 32: if data<i> == '1' then data<i-1:0> = data<i-1:0> EOR (poly:Zeros(i-32))`.
pub open spec fn poly32_mod2_steps(data_in: u128, poly: u32, n: nat, steps: nat) -> u128
    decreases steps,
{
    if steps == 0 {
        data_in
    } else {
        let data = poly32_mod2_steps(data_in, poly, n, (steps - 1) as nat);
        let i = (n - steps) as nat;
        if (data >> (i as u32)) & 1 == 1 {
            set_low_bits_128(data, ones(i) as u128, data ^ ((poly as u128) << ((i - 32) as u32)))
        } else {
            data
        }
    }
}

/// The polynomial of [CRC32B, CRC32H, CRC32W and CRC32X](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32B--CRC32H--CRC32W--CRC32X--CRC32-checksum-),
/// 0x04C11DB7, and of [CRC32CB, CRC32CH, CRC32CW and CRC32CX](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32CB--CRC32CH--CRC32CW--CRC32CX--CRC32C-checksum-),
/// 0x1EDC6F41.
pub open spec fn crc_polynomial(polynomial: CrcPolynomial) -> u32 {
    match polynomial {
        CrcPolynomial::Crc32 => 0x04C1_1DB7,
        CrcPolynomial::Crc32C => 0x1EDC_6F41,
    }
}

/// The Operation of the CRC32 and CRC32C instructions on an accumulator and a `size`-bit value:
/// `tempacc = BitReverse(acc):Zeros(size); tempval = BitReverse(val):Zeros(32);
/// X[d, 32] = BitReverse(Poly32Mod2(tempacc EOR tempval, poly))`, Poly32Mod2 returning `data<31:0>`.
pub open spec fn crc32(acc: u64, val: u64, size: nat, polynomial: CrcPolynomial) -> u64 {
    let tempacc = (reverse_bits_value(acc, 32) as u128) << (size as u32);
    let tempval = (reverse_bits_value(val, size) as u128) << 32u32;
    let n = 32 + size;
    let data = poly32_mod2_steps(tempacc ^ tempval, crc_polynomial(polynomial), n, (n - 32) as nat);
    reverse_bits_value((data & 0xffff_ffff) as u64, 32)
}

/// The Operation of [RMIF](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/RMIF--Rotate--mask-insert-flags-):
/// `tmp = ROR(X[n, 64], lsb)`, then each flag whose bit of mask is set, N at mask<3> to V at mask<0>, takes tmp's bit
/// at the same place, every other flag kept.
pub open spec fn rotate_mask_insert_flags(source: u64, lsb: nat, mask: u64, nzcv: Nzcv) -> Nzcv {
    let tmp = nzcv_immediate(ror(source as int, lsb, 64) as u64);
    let selected = nzcv_immediate(mask);
    Nzcv {
        negative: if selected.negative { tmp.negative } else { nzcv.negative },
        zero: if selected.zero { tmp.zero } else { nzcv.zero },
        carry: if selected.carry { tmp.carry } else { nzcv.carry },
        overflow: if selected.overflow { tmp.overflow } else { nzcv.overflow },
    }
}

/// The Operation of [SETF8 and SETF16](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SETF8--SETF16--Evaluation-of-8-bit-or-16-bit-flag-values-)
/// on the low `bits` bits, `msb = bits - 1`: `PSTATE.N = X[n]<msb>; PSTATE.Z = IsZeroBit(X[n]<msb:0>); PSTATE.V =
/// X[n]<msb + 1> EOR X[n]<msb>`, C kept.
pub open spec fn set_flags(source: u64, bits: u32, nzcv: Nzcv) -> Nzcv {
    let msb = (bits - 1) as u32;
    Nzcv {
        negative: (source >> msb) & 1 == 1,
        zero: is_zero(source as int, bits as nat),
        carry: nzcv.carry,
        overflow: (source >> bits) & 1 != (source >> msb) & 1,
    }
}

/// The Operation of [CFINV](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CFINV--Invert-carry-flag-):
/// `PSTATE.C = NOT(PSTATE.C)`.
pub open spec fn invert_carry(nzcv: Nzcv) -> Nzcv {
    Nzcv { carry: !nzcv.carry, ..nzcv }
}

/// The Operation of [AXFLAG](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/AXFLAG--Convert-floating-point-condition-flags-from-Arm-to-external-format-):
/// `N = '0'; Z = PSTATE.Z OR PSTATE.V; C = PSTATE.C AND NOT(PSTATE.V); V = '0'`.
pub open spec fn flags_to_external_format(nzcv: Nzcv) -> Nzcv {
    Nzcv { negative: false, zero: nzcv.zero || nzcv.overflow, carry: nzcv.carry && !nzcv.overflow, overflow: false }
}

/// The Operation of [XAFLAG](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/XAFLAG--Convert-floating-point-condition-flags-from-external-format-to-Arm-format-):
/// `N = NOT(PSTATE.C) AND NOT(PSTATE.Z); Z = PSTATE.Z AND PSTATE.C; C = PSTATE.C OR PSTATE.Z; V = NOT(PSTATE.C) AND
/// PSTATE.Z`.
pub open spec fn flags_from_external_format(nzcv: Nzcv) -> Nzcv {
    Nzcv {
        negative: !nzcv.carry && !nzcv.zero,
        zero: nzcv.zero && nzcv.carry,
        carry: nzcv.carry || nzcv.zero,
        overflow: !nzcv.carry && nzcv.zero,
    }
}

} // verus!
