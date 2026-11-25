//! The instructions that branch or form an address from their own: one skeleton reads the operands the list gives
//! each instruction, and one small operation per family gives the register it writes and where the next instruction is.

use core::convert::Infallible;

use crate::condition_flags::CONDITIONS;
use crate::integer::{low_ones, read_operands};
use crate::operand::{Operand, RegisterThirtyOne};
use crate::registers::write_register;
use crate::{Exception, INSTRUCTION_SIZE, Instruction, Processor, VIRTUAL_ADDRESS_BITS};

/// The register a branch with link writes its return address to.
const LINK_REGISTER: u32 = 30;
/// The highest bit of an address the top byte of which [TBI0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TCR-EL1--Translation-Control-Register--EL1-#:~:text=TBI0%2C%20bit%20%5B37%5D) leaves out; it also tells the lower half of the addresses from the upper.
const TOP_BYTE_IGNORED_BIT: u32 = 55;
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
        I::Br => O::BranchToRegister,
        I::Blr => O::BranchWithLinkToRegister,
        I::Ret => O::Return,
        _ => return None,
    })
}

/// Reads the operands, applies the operation and writes the register it writes; returns where the next instruction
/// is when it branches.
pub fn simulate_branch<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operands: &[Operand],
    operation: BranchOperation,
) -> Result<Option<u64>, Exception> {
    use BranchOperation as O;
    let read = read_operands(processor, word, operands)?;
    let address = processor.program_counter;
    let next = address.wrapping_add(INSTRUCTION_SIZE);
    let when = |when: BranchWhen, value: u64| (value == 0) == (when == BranchWhen::Zero);
    let (written, link, target) = match (operation, read.sources()) {
        (O::FormAddress, &[offset]) => (Some(address.wrapping_add(offset)), None, None),
        (O::FormPageAddress, &[offset]) => {
            let page = address & !low_ones(PAGE_OFFSET_BITS);
            (Some(page.wrapping_add(offset)), None, None)
        }
        (O::Branch, &[offset]) => (None, None, Some(address.wrapping_add(offset))),
        (O::BranchWithLink, &[offset]) => (None, Some(next), Some(address.wrapping_add(offset))),
        (O::BranchConditionally, &[condition, offset]) => {
            let taken = CONDITIONS[condition as usize].holds(processor.condition_flags);
            (None, None, taken.then(|| address.wrapping_add(offset)))
        }
        (O::CompareAndBranch(branch_when), &[value, offset]) => {
            let taken = when(branch_when, value);
            (None, None, taken.then(|| address.wrapping_add(offset)))
        }
        // [TBZ](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/TBZ--Test-bit-and-branch-if-zero-):
        // the bit b5:b40 of the register, which with b5 clear lies in its low word, the W register's.
        (O::TestBitAndBranch(branch_when), &[value, bit, offset]) => {
            let taken = when(branch_when, (value >> bit) & 1);
            (None, None, taken.then(|| address.wrapping_add(offset)))
        }
        (O::BranchToRegister | O::Return, &[target]) => (None, None, Some(target)),
        (O::BranchWithLinkToRegister, &[target]) => (None, Some(next), Some(target)),
        _ => panic!(
            "{operation:?} reads other operands than {} values",
            read.sources().len()
        ),
    };
    if let (Some(value), Some((number, thirty_one))) = (written, read.destination) {
        write_register(processor, number, value, thirty_one);
    }
    if let Some(link) = link {
        write_register(
            processor,
            LINK_REGISTER,
            link,
            RegisterThirtyOne::ZeroRegister,
        );
    }
    Ok(target.map(to_untagged_address))
}

/// Whether the address is in the upper half, which the top byte of a lower-half address does not count for.
const fn is_upper_half(address: u64) -> bool {
    (address >> TOP_BYTE_IGNORED_BIT) & 1 != 0
}

/// Whether the address, its top byte left out where TBI0 applies, lies outside the lower half's 48 bits, which
/// [faults at level 0](https://developer.arm.com/documentation/ddi0487/latest/#:~:text=Translation%20fault%20at%20level%200) as an address the translation tables do not cover.
pub(crate) const fn is_outside_lower_addresses(address: u64) -> bool {
    to_untagged_address(address) >> VIRTUAL_ADDRESS_BITS != 0
}

/// The address with its top byte left out where TBI0 applies: where a branch goes, as [BranchAddr](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#AArch64.BranchAddr.2) gives it at EL0
/// with TBI0 on and TBI1 off as Linux configures them, and what translation looks up for a data access.
pub(crate) const fn to_untagged_address(target: u64) -> u64 {
    if is_upper_half(target) {
        target
    } else {
        target & low_ones(TOP_BYTE_IGNORED_BIT + 1)
    }
}
