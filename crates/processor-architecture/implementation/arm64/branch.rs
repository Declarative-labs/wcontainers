//! The instructions that branch or form an address from their own: one skeleton reads the operands the list gives
//! each instruction, and one small operation per family gives the register it writes and where the next instruction is.

use core::convert::Infallible;

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::pow2;
use vstd::prelude::*;

use crate::condition_flags::{CONDITIONS, Nzcv};
use crate::integer::{MAXIMUM_SOURCES, low_ones, read_operands};
#[cfg(verus_keep_ghost)]
use crate::integer::{effect_applied, operands_read};
use crate::operand::{Operand, RegisterThirtyOne};
#[cfg(verus_keep_ghost)]
use crate::pseudocode::{condition_holds, ones, uint};
use crate::registers::write_register;
use crate::{Exception, INSTRUCTION_SIZE, Instruction, Processor, VIRTUAL_ADDRESS_BITS};

verus! {

/// The register a branch with link writes its return address to.
const LINK_REGISTER: u32 = 30;
/// The bits of an address below its 4 KiB page, which ADRP clears.
const PAGE_OFFSET_BITS: u32 = 12;

/// Whether a branch is taken when the register or bit it tests is zero, or when it is not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BranchWhen {
    Zero,
    NonZero,
}

/// What one family of branches and address forms computes from its operands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BranchOperation {
    /// ADR: the address of the label.
    FormAddress,
    /// ADRP: the address of the label's 4 KiB page.
    FormPageAddress,
    Branch,
    BranchWithLink,
    BranchConditionally,
    CompareAndBranch(BranchWhen),
    TestBitAndBranch(BranchWhen),
    BranchToRegister,
    BranchWithLinkToRegister,
    Return,
}

} // verus!

impl BranchOperation {
    pub const fn reads_condition_flags(self) -> bool {
        matches!(self, Self::BranchConditionally)
    }

    /// Whether it writes the address of the next instruction to the link register.
    pub const fn links(self) -> bool {
        matches!(self, Self::BranchWithLink | Self::BranchWithLinkToRegister)
    }
}

/// The family of each branch and address form, none for every other instruction.
pub const fn to_branch_operation(instruction: Instruction<Infallible>) -> Option<BranchOperation> {
    use BranchOperation as O;
    use Instruction as I;
    Some(match instruction {
        I::Adr64 => O::FormAddress,
        I::Adrp => O::FormPageAddress,
        I::B => O::Branch,
        I::Bl => O::BranchWithLink,
        I::BCond => O::BranchConditionally,
        I::Cbz32 | I::Cbz64 => O::CompareAndBranch(BranchWhen::Zero),
        I::Cbnz32 | I::Cbnz64 => O::CompareAndBranch(BranchWhen::NonZero),
        I::Tbz => O::TestBitAndBranch(BranchWhen::Zero),
        I::Tbnz => O::TestBitAndBranch(BranchWhen::NonZero),
        // With the instruction keys off, AuthIA and AuthIB leave the target as it is, whatever the modifier.
        I::Br | I::Braaz | I::Brabz | I::Braa | I::Brab => O::BranchToRegister,
        I::Blr | I::Blraaz | I::Blrabz | I::Blraa | I::Blrab => O::BranchWithLinkToRegister,
        I::Ret | I::Retaa | I::Retab => O::Return,
        _ => return None,
    })
}

verus! {

/// `address + offset` in 64 bits, as the pseudocode's `PC64 + offset` gives it.
pub open spec fn offset_address(address: u64, offset: u64) -> u64 {
    uint(address + offset, 64) as u64
}

/// Whether a compare or test branch is taken: CBZ and TBZ when the register or bit is zero, CBNZ and TBNZ when not.
pub open spec fn branch_is_taken(when: BranchWhen, value: int) -> bool {
    (value == 0) == (when == BranchWhen::Zero)
}

/// What one family of branches and address forms gives from its operands' values in the order of the syntax, the
/// instruction's address, the flags and X30, as Arm's pseudocode does with every key off: the value written to the
/// register the instruction names, the return address written to X30, and the target before
/// [BranchAddr](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#AArch64.BranchAddr.2),
/// each where it has one; none where the operands are not those the family reads.
///
/// - [ADR](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADR--Form-PC-relative-address-): `X[d] = PC64 + imm`.
/// - [ADRP](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADRP--Form-PC-relative-address-to-4KB-page-): `base = PC64; base<11:0> = Zeros(12); X[d] = base + imm`.
/// - [B](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/B--Branch-) and [BL](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BL--Branch-with-link-): `if branch_type == BranchType_DIRCALL then X[30] = PC64 + 4; BranchTo(PC64 + offset)`.
/// - [B.cond](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/B-cond--Branch-conditionally-): `if ConditionHolds(cond) then BranchTo(PC64 + offset)`.
/// - [CBZ and CBNZ](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CBZ--Compare-and-branch-on-zero-): `if IsZero(X[t, datasize]) == iszero then BranchTo(PC64 + offset)`.
/// - [TBZ and TBNZ](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/TBZ--Test-bit-and-branch-if-zero-): `if operand<bit_pos> == op then BranchTo(PC64 + offset)`.
/// - [BR, BLR and RET](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BLR--Branch-with-link-to-register-) and their authenticated forms: `target = X[n]`, or X30 for RETAA and RETAB, which AuthIA and AuthIB leave as it is with the keys off; `if branch_type == BranchType_INDCALL then X[30] = PC64 + 4; BranchTo(target)`.
pub open spec fn branch_outcome(operation: BranchOperation, address: u64, sources: Seq<u64>, flags: Nzcv, link: u64)
    -> Option<(Option<u64>, Option<u64>, Option<u64>)>
{
    let count = sources.len();
    // The numbers are the pseudocode's own, not the constants the code uses: `PC64 + 4`, `base<11:0> = Zeros(12)`, X30.
    let next = offset_address(address, 4);
    match operation {
        BranchOperation::FormAddress => if count == 1 {
            Some((Some(offset_address(address, sources[0])), None, None))
        } else {
            None
        },
        BranchOperation::FormPageAddress => if count == 1 {
            let base = address as int - address as int % pow2(12) as int;
            Some((Some(offset_address(base as u64, sources[0])), None, None))
        } else {
            None
        },
        BranchOperation::Branch => if count == 1 {
            Some((None, None, Some(offset_address(address, sources[0]))))
        } else {
            None
        },
        BranchOperation::BranchWithLink => if count == 1 {
            Some((None, Some(next), Some(offset_address(address, sources[0]))))
        } else {
            None
        },
        BranchOperation::BranchConditionally => if count == 2 && sources[0] < CONDITIONS.len() {
            let taken = condition_holds(sources[0] as nat, flags);
            Some((None, None, if taken { Some(offset_address(address, sources[1])) } else { None }))
        } else {
            None
        },
        BranchOperation::CompareAndBranch(when) => if count == 2 {
            let taken = branch_is_taken(when, sources[0] as int);
            Some((None, None, if taken { Some(offset_address(address, sources[1])) } else { None }))
        } else {
            None
        },
        BranchOperation::TestBitAndBranch(when) => if count == 3 && sources[1] < u64::BITS {
            let taken = branch_is_taken(when, (sources[0] as int / pow2(sources[1] as nat) as int) % 2);
            Some((None, None, if taken { Some(offset_address(address, sources[2])) } else { None }))
        } else {
            None
        },
        BranchOperation::BranchToRegister => if count == 1 || count == 2 {
            Some((None, None, Some(sources[0])))
        } else {
            None
        },
        BranchOperation::BranchWithLinkToRegister => if count == 1 || count == 2 {
            Some((None, Some(next), Some(sources[0])))
        } else {
            None
        },
        BranchOperation::Return => if count == 0 {
            Some((None, None, Some(link)))
        } else if count == 1 {
            Some((None, None, Some(sources[0])))
        } else {
            None
        },
    }
}

/// The register a branch or address form writes, with the value, and where the next instruction is when it branches,
/// from the processor and the word: the family's outcome on its operands' values, its target through BranchAddr; or the
/// exception, UNDEFINED.
pub open spec fn branch_effect<L>(processor: Processor<L>, word: u32, operands: Seq<Operand>, operation: BranchOperation)
    -> Result<(Option<(u32, u64, RegisterThirtyOne)>, Option<u64>), Exception>
{
    match operands_read(processor, word, operands) {
        None => Err(Exception::UndefinedInstruction),
        Some((destination, _, sources)) => {
            let link = processor.general_purpose_registers@[30];
            let values = sources.map_values(|value: int| value as u64);
            match branch_outcome(operation, processor.program_counter, values, processor.condition_flags, link) {
                None => Err(Exception::UndefinedInstruction),
                Some((written, linked, target)) => Ok((
                    match (written, destination, linked) {
                        (Some(value), Some((number, thirty_one)), _) => Some((number, value, thirty_one)),
                        (_, _, Some(next)) => Some((30, next, RegisterThirtyOne::ZeroRegister)),
                        _ => None,
                    },
                    match target {
                        Some(target) => Some(branch_address(target)),
                        None => None,
                    },
                )),
            }
        },
    }
}

/// Reads the operands, applies the operation and writes the register it writes; returns where the next instruction
/// is when it branches; changes nothing when it returns an exception.
pub fn simulate_branch<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operands: &[Operand],
    operation: BranchOperation,
) -> (outcome: Result<Option<u64>, Exception>)
    ensures
        match branch_effect(*old(processor), word, operands@, operation) {
            Err(exception) => outcome == Err::<Option<u64>, Exception>(exception) && *final(processor) == *old(processor),
            Ok((write, target)) => outcome == Ok::<Option<u64>, Exception>(target)
                && effect_applied(*old(processor), *final(processor), write, None),
        },
{
    // VERUS: `==` of `BranchOperation` has no specification in executable code, and `planted::any` none at all, so the
    // operation is matched and the bugs are `is`.
    if (crate::planted::is(425) || (crate::planted::is(145) && is_first_register_link(word, operands)))
        && matches!(operation, BranchOperation::BranchWithLinkToRegister)
    {
        let next = processor.program_counter.wrapping_add(INSTRUCTION_SIZE);
        write_register(processor, LINK_REGISTER, next, RegisterThirtyOne::ZeroRegister);
    }
    let read = read_operands(processor, word, operands)?;
    let ghost sources = match operands_read(*old(processor), word, operands@) {
        Some((_, _, sources)) => sources,
        None => Seq::empty(),
    };
    proof {
        assert(read.sources@.take(read.count as int) =~= sources.map_values(|value: int| value as u64));
    }
    let link_value = processor.general_purpose_registers[LINK_REGISTER as usize];
    let Some((written, link, target)) = compute_branch(
        operation,
        processor.program_counter,
        &read.sources,
        read.count,
        processor.condition_flags,
        link_value,
    ) else {
        return Err(Exception::UndefinedInstruction);
    };
    if let (Some(value), Some((number, thirty_one))) = (written, read.destination) {
        write_register(processor, number, value, thirty_one);
    }
    if let Some(link) = link {
        write_register(processor, LINK_REGISTER, link, RegisterThirtyOne::ZeroRegister);
    }
    let Some(target) = target else {
        return Ok(None);
    };
    Ok(Some(if crate::planted::is(146) {
        target
    } else {
        to_untagged_address(target)
    }))
}

/// Whether the first register operand the word names is X30.
fn is_first_register_link(word: u32, operands: &[Operand]) -> bool {
    let mut index: usize = 0;
    while index < operands.len()
        invariant
            index <= operands.len(),
        decreases operands.len() - index,
    {
        if let Operand::GeneralPurposeRegister { number, .. } = operands[index] {
            return number.is_inside() && number.read(word) == LINK_REGISTER;
        }
        index += 1;
    }
    false
}

/// `address + offset` in 64 bits.
fn add_offset(address: u64, offset: u64) -> (sum: u64)
    ensures
        sum == offset_address(address, offset),
{
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        let sum = address + offset;
        if sum >= pow2(64) {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(sum, pow2(64) as int, 1, sum - pow2(64));
        } else {
            vstd::arithmetic::div_mod::lemma_small_mod(sum as nat, pow2(64));
        }
    }
    address.wrapping_add(offset)
}

/// Whether a compare or test branch is taken on the value it tests.
fn is_taken(when: BranchWhen, value: u64) -> (taken: bool)
    ensures
        taken == branch_is_taken(when, value as int),
{
    // VERUS: `==` of `BranchWhen` has no specification in executable code, so it is matched.
    (value == 0) == matches!(when, BranchWhen::Zero)
}

/// The family's outcome on the first `count` operand values: what it writes to its register, what it writes to X30,
/// and its target before BranchAddr; none where the values are not those it reads.
fn compute_branch(
    operation: BranchOperation,
    address: u64,
    sources: &[u64; MAXIMUM_SOURCES],
    count: usize,
    flags: Nzcv,
    link: u64,
) -> (outcome: Option<(Option<u64>, Option<u64>, Option<u64>)>)
    requires
        count <= MAXIMUM_SOURCES,
    ensures
        outcome == branch_outcome(operation, address, sources@.take(count as int), flags, link),
{
    let next = add_offset(address, INSTRUCTION_SIZE);
    // VERUS: the operand lists are a table Verus does not read, so values other than those a family reads are refused
    // as UNDEFINED where they panicked.
    match operation {
        BranchOperation::FormAddress if count == 1 => {
            let offset = if crate::planted::is(28) {
                sources[0] << 2
            } else {
                sources[0]
            };
            Some((Some(add_offset(address, offset)), None, None))
        },
        BranchOperation::FormPageAddress if count == 1 => {
            let offset_bits = low_ones(PAGE_OFFSET_BITS);
            proof {
                vstd::arithmetic::power2::lemma2_to64();
                assert(address & !offset_bits == address - address % 4096) by (bit_vector)
                    requires
                        offset_bits == 4095;
            }
            let page = if crate::planted::is(147) {
                address
            } else {
                address & !offset_bits
            };
            let formed = add_offset(page, sources[0]);
            let formed = if crate::planted::is(29) {
                formed & low_ones(u32::BITS)
            } else {
                formed
            };
            Some((Some(formed), None, None))
        },
        BranchOperation::Branch if count == 1 => Some((None, None, Some(add_offset(address, sources[0])))),
        BranchOperation::BranchWithLink if count == 1 => {
            let link = if crate::planted::is(141) {
                address
            } else {
                next
            };
            Some((None, Some(link), Some(add_offset(address, sources[0]))))
        },
        BranchOperation::BranchConditionally if count == 2 && sources[0] < CONDITIONS.len() as u64 => {
            // GT is condition 12, GE condition 10.
            let condition = if crate::planted::is(25) && sources[0] == 12 {
                10
            } else {
                sources[0]
            };
            let taken = CONDITIONS[condition as usize].holds_in(flags);
            proof {
                assert(CONDITIONS[condition as int].encoding() == condition);
            }
            Some((None, None, if taken { Some(add_offset(address, sources[1])) } else { None }))
        },
        BranchOperation::CompareAndBranch(when) if count == 2 => {
            let value = if crate::planted::is(22) {
                sources[0] & low_ones(u32::BITS)
            } else {
                sources[0]
            };
            let taken = is_taken(when, value);
            Some((None, None, if taken { Some(add_offset(address, sources[1])) } else { None }))
        },
        // [TBZ](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/TBZ--Test-bit-and-branch-if-zero-):
        // the bit b5:b40 of the register, which with b5 clear lies in its low word, the W register's.
        BranchOperation::TestBitAndBranch(when) if count == 3 && sources[1] < u64::BITS as u64 => {
            let bit = if crate::planted::is(424) || crate::planted::is(143) || crate::planted::is(23) {
                sources[1] % 32
            } else {
                sources[1]
            };
            let value = sources[0];
            let tested = (value >> bit) & 1;
            proof {
                vstd::bits::lemma_u64_shr_is_div(value, bit);
                assert((value >> bit) & 1 == (value >> bit) % 2) by (bit_vector);
            }
            let taken = is_taken(when, tested) != crate::planted::is(252);
            Some((None, None, if taken { Some(add_offset(address, sources[2])) } else { None }))
        },
        BranchOperation::Return if count == 0 => Some((None, None, Some(if crate::planted::is(712) {
            0
        } else {
            link
        }))),
        BranchOperation::Return if count == 1 && crate::planted::is(27) => Some((None, None, Some(sources[0] & !3))),
        BranchOperation::Return if count == 1 && (crate::planted::is(426) || crate::planted::is(148)) => {
            Some((None, None, Some(link)))
        },
        BranchOperation::BranchToRegister if (count == 1 || count == 2) && crate::planted::is(427)
            && !is_upper_half(sources[0]) => {
            Some((None, None, Some(sources[0] & low_ones(VIRTUAL_ADDRESS_BITS))))
        },
        BranchOperation::BranchToRegister if count == 1 || count == 2 => Some((None, None, Some(sources[0]))),
        BranchOperation::Return if count == 1 => Some((None, None, Some(sources[0]))),
        BranchOperation::BranchWithLinkToRegister if (count == 1 || count == 2) && crate::planted::is(26) => {
            Some((None, Some(add_offset(next, INSTRUCTION_SIZE)), Some(sources[0])))
        },
        BranchOperation::BranchWithLinkToRegister if count == 1 || count == 2 => {
            Some((None, Some(next), Some(sources[0])))
        },
        _ => None,
    }
}

/// The highest bit of an address the top byte of which [TBI0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TCR-EL1--Translation-Control-Register--EL1-#:~:text=TBI0%2C%20bit%20%5B37%5D) leaves out; it also tells the lower half of the addresses from the upper.
const TOP_BYTE_IGNORED_BIT: u32 = 55;

/// [BranchAddr](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#AArch64.BranchAddr.2)
/// of an address at EL0 with TBI0 on and TBI1 off, as Linux configures them: AddrTop is bit 55 for a lower-half address
/// (bit 55 clear), whose `SignExtend(vaddress<55:0>, 64)` clears the top byte, and bit 63 for an upper-half one, which
/// it keeps whole.
pub open spec fn branch_address(vaddress: u64) -> u64 {
    if (vaddress >> 55u32) & 1 == 1 {
        vaddress
    } else {
        vaddress & (ones(56) as u64)
    }
}

/// Whether an address lies outside the lower half's 48 bits once its top byte is left out where TBI0 applies.
pub open spec fn is_outside_lower(address: u64) -> bool {
    branch_address(address) >> 48u32 != 0
}

/// Whether the address is in the upper half, which the top byte of a lower-half address does not count for.
const fn is_upper_half(address: u64) -> (upper: bool)
    ensures
        upper == ((address >> 55u32) & 1 == 1),
{
    proof {
        assert(((address >> 55u32) & 1 != 0) == ((address >> 55u32) & 1 == 1)) by (bit_vector);
    }
    (address >> TOP_BYTE_IGNORED_BIT) & 1 != 0
}

/// Whether the address, its top byte left out where TBI0 applies, lies outside the lower half's 48 bits, which
/// [faults at level 0](https://developer.arm.com/documentation/ddi0487/latest/#:~:text=Translation%20fault%20at%20level%200) as an address the translation tables do not cover.
pub(crate) fn is_outside_lower_addresses(address: u64) -> (outside: bool)
    ensures
        outside == is_outside_lower(address),
{
    if crate::planted::is(447) {
        return to_untagged_address(address) >> (TOP_BYTE_IGNORED_BIT + 1) != 0;
    }
    to_untagged_address(address) >> VIRTUAL_ADDRESS_BITS != 0
}

/// The address with its top byte left out where TBI0 applies: where a branch goes, as [BranchAddr](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#AArch64.BranchAddr.2) gives it at EL0
/// with TBI0 on and TBI1 off as Linux configures them, and what translation looks up for a data access.
pub(crate) fn to_untagged_address(target: u64) -> (untagged: u64)
    ensures
        untagged == branch_address(target),
{
    if is_upper_half(target) && !crate::planted::is(446) {
        target
    } else {
        target & low_ones(TOP_BYTE_IGNORED_BIT + 1)
    }
}

} // verus!
