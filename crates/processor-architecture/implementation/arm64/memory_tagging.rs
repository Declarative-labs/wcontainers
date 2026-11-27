//! Memory tagging at EL0 as the executor's guest configures it: SCTLR_EL1.ATA0 clear, so that
//! [AllocationTagAccessIsEnabled](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-mte#impl-aarch64.AllocationTagAccessIsEnabled.1)
//! is false: ADDG, SUBG and IRG give tag zero, LDG reads tag zero, and a tag store writes no tag but still translates
//! and checks its access as a store. GMI, SUBP and SUBPS read no tag storage.

use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_processor_architecture::{MemoryDelta, MemorySink, ProcessorArchitecture};

use crate::behavior::{FaultAddressTag, MisalignedStackPointer, StoreAcrossFault};
use crate::branch::to_untagged_address;
use crate::encoding::{Field, RD, RM, RN, RT, RT2};
use crate::load_store::{AccessLayout, find_fault, store, to_fault_address};
use crate::operand::RegisterThirtyOne;
use crate::registers::{read_register, write_register};
use crate::{
    AccessDirection, ConditionFlags, Exception, FaultStatus, GENERAL_PURPOSE_REGISTERS,
    Instruction, Nzcv, PagePermission, PreciseInstruction, Processor,
};

/// The bytes one allocation tag covers.
const TAG_GRANULE: u64 = 16;
/// Where an address's allocation tag lies: bits 59 to 56.
const TAG_SHIFT: u32 = 56;
const TAG_BITS: u64 = 0xF << TAG_SHIFT;
/// The bits SUBP and SUBPS compare of each address, below the top byte.
const POINTER_BITS: u32 = 56;
/// ADDG's and SUBG's offset, in granules.
const OFFSET_GRANULES: Field = Field::new(16, 6);
/// The signed offset of a tag store or LDG, in granules.
const TAG_OFFSET: Field = Field::new(12, 9);
/// The signed offset of STGP, in granules.
const PAIR_OFFSET: Field = Field::new(15, 7);
/// The alignment [CheckSPAlignment](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.CheckSPAlignment.0) requires.
const STACK_POINTER_ALIGNMENT: u64 = 16;

/// What one memory tagging instruction that reaches no memory computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TagOperation {
    /// [ADDG](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADDG--Add-with-tag-) and SUBG: the
    /// address moved by whole granules, with tag zero.
    AddWithTag { subtract: bool },
    /// [IRG](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/IRG--Insert-random-tag-): the address with tag zero.
    InsertRandomTag,
    /// [GMI](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/GMI--Tag-mask-insert-): the mask with the bit of the address's tag set.
    TagMaskInsert,
    /// [SUBP](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SUBP--Subtract-pointer-) and SUBPS: the difference of the two addresses' low 56 bits, sign-extended.
    SubtractPointer { sets_flags: bool },
}

/// The operation of each memory tagging instruction that reaches no memory, none for every other instruction.
pub const fn to_tag_operation(instruction: Instruction<Infallible>) -> Option<TagOperation> {
    use Instruction as I;
    use TagOperation as O;
    Some(match instruction {
        I::Addg => O::AddWithTag { subtract: false },
        I::Subg => O::AddWithTag { subtract: true },
        I::Irg => O::InsertRandomTag,
        I::Gmi => O::TagMaskInsert,
        I::Subp64 => O::SubtractPointer { sets_flags: false },
        I::Subps => O::SubtractPointer { sets_flags: true },
        _ => return None,
    })
}

const fn with_tag_zero(address: u64) -> u64 {
    address & !TAG_BITS
}

const fn sign_extend_pointer(address: u64) -> u64 {
    let shift = u64::BITS - POINTER_BITS;
    (((address << shift) as i64) >> shift) as u64
}

pub fn simulate_tag_operation<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: TagOperation,
) -> Result<(), Exception> {
    use TagOperation as O;
    let n = read_register(processor, RN.read(word), RegisterThirtyOne::StackPointer);
    let (value, destination) = match operation {
        O::AddWithTag { subtract } => {
            let offset = u64::from(OFFSET_GRANULES.read(word)) * TAG_GRANULE;
            let moved = if subtract {
                n.wrapping_sub(offset)
            } else {
                n.wrapping_add(offset)
            };
            let tagged = if crate::planted::is(719) {
                moved
            } else {
                with_tag_zero(moved)
            };
            (tagged, RegisterThirtyOne::StackPointer)
        }
        O::InsertRandomTag => (with_tag_zero(n), RegisterThirtyOne::StackPointer),
        O::TagMaskInsert => {
            let mask = read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister);
            let tag = (n >> TAG_SHIFT) & 0xF;
            let tag = if crate::planted::is(720) {
                tag + 1
            } else {
                tag
            };
            (mask | (1 << tag), RegisterThirtyOne::ZeroRegister)
        }
        O::SubtractPointer { sets_flags } => {
            let first = sign_extend_pointer(n);
            let second = sign_extend_pointer(read_register(
                processor,
                RM.read(word),
                RegisterThirtyOne::StackPointer,
            ));
            let difference = first.wrapping_sub(second);
            if sets_flags {
                processor.condition_flags =
                    Nzcv::from(to_subtraction_flags(first, second, difference));
            }
            (difference, RegisterThirtyOne::ZeroRegister)
        }
    };
    write_register(processor, RD.read(word), value, destination);
    Ok(())
}

/// The flags of [AddWithCarry](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.AddWithCarry.3) of the first, the second's complement and one.
fn to_subtraction_flags(first: u64, second: u64, difference: u64) -> ConditionFlags {
    let mut flags = ConditionFlags::empty();
    flags.set(ConditionFlags::NEGATIVE, (difference as i64) < 0);
    flags.set(ConditionFlags::ZERO, difference == 0);
    flags.set(ConditionFlags::CARRY, first >= second);
    flags.set(
        ConditionFlags::OVERFLOW,
        (first as i64).overflowing_sub(second as i64).1,
    );
    flags
}

/// LDG, a tag store or STGP, whose behavior is modeled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TagAccessInstruction(Instruction<Infallible>);

/// What a tag load or store does in each case the manual leaves open or a chip does its own way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::memory_tagging)
)]
pub struct TagAccessChoices {
    pub misaligned_stack_pointer: MisalignedStackPointer,
    /// None for an instruction that writes no data across a page.
    pub across_fault: Option<StoreAcrossFault>,
    pub fault_address_tag: FaultAddressTag,
}

/// What a tag load or store moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TagAccess {
    /// LDG: the tag of the granule holding the address into Rt.
    Load,
    /// STG, STZG, ST2G and STZ2G: the tag of one or two granules, zeroing their data or not.
    Store { granules: u64, zeroes: bool },
    /// STGP: two registers' data and the tag of the granule.
    StorePair,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Indexing {
    Offset,
    PreIndex,
    PostIndex,
}

impl TagAccessInstruction {
    pub const fn new(instruction: Instruction<Infallible>) -> Option<Self> {
        match Self(instruction).to_access() {
            Some(_) => Some(Self(instruction)),
            None => None,
        }
    }

    pub const fn instruction(self) -> Instruction<Infallible> {
        self.0
    }

    const fn to_access(self) -> Option<(TagAccess, Indexing)> {
        use Indexing::{Offset, PostIndex, PreIndex};
        use Instruction as I;
        const fn store(granules: u64, zeroes: bool) -> TagAccess {
            TagAccess::Store { granules, zeroes }
        }
        Some(match self.0 {
            I::Ldg => (TagAccess::Load, Offset),
            I::StgPostIndex64 => (store(1, false), PostIndex),
            I::StgImmediate64 => (store(1, false), Offset),
            I::StgPreIndex64 => (store(1, false), PreIndex),
            I::StzgPostIndex64 => (store(1, true), PostIndex),
            I::StzgImmediate64 => (store(1, true), Offset),
            I::StzgPreIndex64 => (store(1, true), PreIndex),
            I::St2gPostIndex64 => (store(2, false), PostIndex),
            I::St2gImmediate64 => (store(2, false), Offset),
            I::St2gPreIndex64 => (store(2, false), PreIndex),
            I::Stz2gPostIndex64 => (store(2, true), PostIndex),
            I::Stz2gImmediate64 => (store(2, true), Offset),
            I::Stz2gPreIndex64 => (store(2, true), PreIndex),
            I::StgpPostIndex64 => (TagAccess::StorePair, PostIndex),
            I::StgpSignedOffset64 => (TagAccess::StorePair, Offset),
            I::StgpPreIndex64 => (TagAccess::StorePair, PreIndex),
            _ => return None,
        })
    }

    fn to_parts(self) -> (TagAccess, Indexing) {
        self.to_access().expect("`new` makes only tag accesses")
    }

    /// Whether it writes data, which a page may refuse part of.
    pub fn writes_data(self) -> bool {
        matches!(
            self.to_parts().0,
            TagAccess::Store { zeroes: true, .. } | TagAccess::StorePair
        )
    }

    pub fn is_pair(self) -> bool {
        self.to_parts().0 == TagAccess::StorePair
    }

    fn to_bytes(self) -> u64 {
        match self.to_parts().0 {
            TagAccess::Load | TagAccess::StorePair => TAG_GRANULE,
            TagAccess::Store { granules, .. } => granules * TAG_GRANULE,
        }
    }

    fn to_offset(self, word: u32) -> u64 {
        let field = if self.is_pair() {
            PAIR_OFFSET
        } else {
            TAG_OFFSET
        };
        (field.read_signed(word) as u64).wrapping_mul(TAG_GRANULE)
    }

    /// The bytes the encoding reaches and what it adds to its base to find the first.
    pub fn to_access_layout(self, word: u32) -> AccessLayout {
        AccessLayout {
            bytes: self.to_bytes(),
            offset: match self.to_parts().1 {
                Indexing::PostIndex => 0,
                Indexing::Offset | Indexing::PreIndex => self.to_offset(word),
            },
        }
    }

    /// Applies the instruction encoded in the word; changes no register when it returns an exception.
    pub(crate) fn simulate<LevelSpecific, Architecture>(
        self,
        choices: TagAccessChoices,
        processor: &mut Processor<LevelSpecific>,
        memory: &[MemoryDelta<'_, Architecture>],
        word: u32,
        mutation_hook: &mut impl MemorySink<Architecture>,
    ) -> Result<(), Exception>
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    {
        let (access, indexing) = self.to_parts();
        let n = RN.read(word);
        let base = read_register(processor, n, RegisterThirtyOne::StackPointer);
        if n as usize == GENERAL_PURPOSE_REGISTERS
            && !base.is_multiple_of(STACK_POINTER_ALIGNMENT)
            && choices.misaligned_stack_pointer == MisalignedStackPointer::Faults
        {
            return Err(Exception::StackPointerAlignmentFault);
        }
        let offset = self.to_offset(word);
        let address = match indexing {
            Indexing::PostIndex => base,
            Indexing::Offset | Indexing::PreIndex => base.wrapping_add(offset),
        };
        let tag = choices.fault_address_tag;
        let fault = |direction, at: u64, bytes| {
            find_fault(memory, to_untagged_address(at), bytes, direction).map(
                |(location, status)| Exception::DataAbort {
                    status,
                    access: direction,
                    address: to_fault_address(at, location, tag),
                },
            )
        };
        match access {
            TagAccess::Load => {
                let aligned = if crate::planted::is(721) {
                    address
                } else {
                    address & !(TAG_GRANULE - 1)
                };
                if let Some(exception) = fault(AccessDirection::Read, aligned, 1) {
                    return Err(exception);
                }
                let t = RT.read(word);
                let value = read_register(processor, t, RegisterThirtyOne::ZeroRegister);
                write_register(
                    processor,
                    t,
                    with_tag_zero(value),
                    RegisterThirtyOne::ZeroRegister,
                );
            }
            TagAccess::Store { .. } | TagAccess::StorePair => {
                let zeroes = matches!(access, TagAccess::Store { zeroes: true, .. });
                if !address.is_multiple_of(TAG_GRANULE) && !crate::planted::is(722) {
                    return Err(Exception::DataAbort {
                        status: FaultStatus::AlignmentFault,
                        access: AccessDirection::Write,
                        address: to_fault_address(address, address, tag),
                    });
                }
                let bytes = self.to_bytes();
                let data: Vec<u8> = if access == TagAccess::StorePair {
                    [RT.read(word), RT2.read(word)]
                        .into_iter()
                        .flat_map(|number| {
                            read_register(processor, number, RegisterThirtyOne::ZeroRegister)
                                .to_le_bytes()
                        })
                        .collect()
                } else {
                    alloc::vec![0; bytes as usize]
                };
                if zeroes || access == TagAccess::StorePair {
                    store(
                        memory,
                        address,
                        to_untagged_address(address),
                        tag,
                        &data,
                        choices
                            .across_fault
                            .unwrap_or(StoreAcrossFault::WritesNothing),
                        mutation_hook,
                    )?;
                } else if let Some(exception) = fault(AccessDirection::Write, address, bytes) {
                    return Err(exception);
                }
            }
        }
        if indexing != Indexing::Offset {
            let written_back = match indexing {
                Indexing::PostIndex => base.wrapping_add(offset),
                Indexing::Offset | Indexing::PreIndex => address,
            };
            write_register(processor, n, written_back, RegisterThirtyOne::StackPointer);
        }
        Ok(())
    }
}

/// Each combination of the choices the instruction has; the behavior of a chip without memory tagging is the caller's.
pub(crate) fn list_tag_access_behaviors(
    instruction: TagAccessInstruction,
) -> Vec<PreciseInstruction<Infallible>> {
    // Only STZ2G writes data that may cross a page: every other access is one aligned granule.
    let across: &[Option<StoreAcrossFault>] =
        if instruction.writes_data() && instruction.to_bytes() > TAG_GRANULE {
            &[
                Some(StoreAcrossFault::WritesNothing),
                Some(StoreAcrossFault::WritesBytesBelowFault),
            ]
        } else {
            &[None]
        };
    let mut behaviors = Vec::new();
    for misaligned_stack_pointer in [
        MisalignedStackPointer::Faults,
        MisalignedStackPointer::Accesses,
    ] {
        for across_fault in across {
            for fault_address_tag in [
                FaultAddressTag::KeepsTopByte,
                FaultAddressTag::ClearsTopNibble,
            ] {
                behaviors.push(PreciseInstruction::TagAccess {
                    instruction,
                    choices: TagAccessChoices {
                        misaligned_stack_pointer,
                        across_fault: *across_fault,
                        fault_address_tag,
                    },
                });
            }
        }
    }
    behaviors
}
