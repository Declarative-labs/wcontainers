//! The inputs of JCXZ, which only a 16-bit address size encodes: its states start in Linux's 32-bit code segment,
//! where the prefix 0x67 gives that size. One case holds what decides the jump together: the operand size (32, or 16
//! under the prefix 0x66), where the code lies, the displacement and RCX.
//!
//! RCX takes CX zero with the bits above it set and clear, so that a test of ECX or RCX shows; the code lies at a page
//! start, where a forward jump past 64 KiB shows the 16-bit target's wrap, just below 4 GiB, where the 32-bit target
//! wraps, and across the end of its page with the next unmapped, where the fetch faults.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::{
    DataAccess, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
};

use crate::{FlagSet, OperandType, OperandValue};

/// Linux's 32-bit code segment.
const COMPATIBILITY_CODE_SEGMENT: u16 = 0x23;
const COUNT: usize = 1;
/// The prefixes: operand size and address size; then JCXZ's opcode.
const OPERAND_SIZE: u8 = 0x66;
const ADDRESS_SIZE: u8 = 0x67;
const OPCODE: u8 = 0xE3;

/// One case of JCXZ.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CountJump {
    /// Under the prefix 0x66.
    pub sixteen_bit_operands: bool,
    pub address: u64,
    pub displacement: i8,
    pub count: u64,
}

pub fn list_operand_types() -> Vec<OperandType> {
    vec![OperandType::CountJump, OperandType::Flags(FlagSet::Unread)]
}

/// The instruction's bytes.
fn encode(case: &CountJump) -> Vec<u8> {
    let prefix = case.sixteen_bit_operands.then_some(OPERAND_SIZE);
    prefix.into_iter().chain([ADDRESS_SIZE, OPCODE, case.displacement as u8]).collect()
}

pub fn generate_cases() -> Vec<OperandValue> {
    const COUNTS: [u64; 5] = [0, 1, 0x1_0000, 0xFFFF_FFFF_0000_0000, 0xFFFF];
    const DISPLACEMENTS: [i8; 3] = [0x10, -0x80, 0];
    // A page start; just below 64 KiB, where a forward jump passes it; just below 4 GiB.
    const ADDRESSES: [u64; 3] = [0x1000, 0xFFF0, 0xFFFF_FFF8];
    let mut cases = Vec::new();
    for sixteen_bit_operands in [false, true] {
        for address in ADDRESSES {
            for displacement in DISPLACEMENTS {
                for count in COUNTS {
                    cases.push(CountJump { sixteen_bit_operands, address, displacement, count });
                }
            }
        }
        // Across the end of the page, whose next page is unmapped.
        let length = if sixteen_bit_operands { 4 } else { 3 };
        cases.push(CountJump { sixteen_bit_operands, address: 0x2000 - length + 1, displacement: 0x10, count: 0 });
    }
    cases.into_iter().map(OperandValue::CountJump).collect()
}

pub fn build_state(operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        code_segment: COMPATIBILITY_CODE_SEGMENT,
        ..ZERO_PROCESSOR
    };
    let mut case = None;
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::CountJump, OperandValue::CountJump(chosen)) => case = Some(chosen),
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let case = case.expect("a case of JCXZ");
    processor.general_purpose_registers[COUNT] = case.count;
    processor.instruction_pointer = case.address;
    let page = case.address & !(PAGE_BYTES - 1);
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let bytes = encode(&case);
    let in_page = bytes.len().min((page + PAGE_BYTES - case.address) as usize);
    let memory_mutations = vec![
        MemoryDelta::PermissionAssignment { addresses: page..page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start: case.address, bytes: Cow::Owned(bytes[..in_page].to_vec()) },
    ];
    ExecutionState { processor, memory_mutations }
}
