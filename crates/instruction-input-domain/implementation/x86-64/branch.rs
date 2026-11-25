//! The inputs of the near jumps, from what decides each: its layout, its displacement or target register, its
//! address size and count where it counts, and the flags it reads.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_x86_64::branch::{BranchForm, Decision};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::{
    DataAccess, Flags, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{AddressSize, CodeLayout, ImmediateClass, encode_at_code_address, lay_out};
use crate::{CODE_ADDRESS, FlagSet, GENERAL_64, OperandType, OperandValue, Role, ValueSet};

/// The count register, RCX.
const COUNT: usize = 1;
/// A target the measuring encoding reaches with any displacement size.
const NEAR_TARGET: u64 = CODE_ADDRESS + 0x10;

/// The operand types of a near jump, in the order the build reads them.
pub fn list_operand_types(form: &BranchForm) -> Vec<OperandType> {
    let mut types = Vec::new();
    if form.codes.len() > 1 {
        types.push(OperandType::AddressSize);
    }
    types.push(OperandType::Encoding(usize::from(form.indirect)));
    if form.indirect {
        types.push(OperandType::RegisterValue(Role::Destination, ValueSet::Arithmetic));
    } else {
        let wide = form.codes[0].op_code().op_kinds().contains(&iced_x86::OpCodeOperandKind::br64_4);
        types.push(OperandType::Immediate(if wide { ImmediateClass::Relative32 } else { ImmediateClass::Relative8 }));
    }
    if form.decision.counts() {
        types.push(OperandType::RegisterValue(Role::Implicit(COUNT), ValueSet::Arithmetic));
    }
    let read = match form.decision {
        Decision::If(condition) => condition.reads(),
        Decision::LoopWhileZero | Decision::LoopWhileNotZero => Flags::ZERO,
        _ => Flags::empty(),
    };
    types.push(OperandType::Flags(if read.is_empty() { FlagSet::Unread } else { FlagSet::Condition(read) }));
    types
}

/// Builds the state of a near jump from one value of each of its operand types.
pub fn build_state(form: &BranchForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        ..ZERO_PROCESSOR
    };
    let (mut code, mut layout, mut register, mut displacement) = (form.codes[0], CodeLayout::PageStart, 0, 0);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::AddressSize, OperandValue::AddressSize(size)) => {
                code = form.codes[usize::from(size == AddressSize::Bits32)];
            }
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => {
                (register, layout) = (encoding.registers[0], encoding.layout);
            }
            (OperandType::Immediate(_), OperandValue::Immediate(value)) => displacement = value,
            (OperandType::RegisterValue(Role::Implicit(number), _), OperandValue::Bits(value)) => {
                processor.general_purpose_registers[*number] = value;
            }
            (OperandType::RegisterValue(_, _), OperandValue::Bits(value)) => {
                processor.general_purpose_registers[register] = value;
            }
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let bytes = if form.indirect {
        let encoded = Encoded::with1(code, GENERAL_64[register]).unwrap_or_else(|error| panic!("{code:?}: {error}"));
        encode_at_code_address(&encoded)
    } else {
        encode_relative(code, displacement)
    };
    let (code_page, start, bytes) = lay_out(layout, &bytes, crate::integer::to_padding(None));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    };
    let memory_mutations = vec![
        MemoryDelta::PermissionAssignment {
            addresses: code_page..code_page + PAGE_BYTES,
            permission: executable,
        },
        MemoryDelta::ValueAssignment {
            start,
            bytes: Cow::Owned(bytes),
        },
    ];
    ExecutionState {
        processor,
        memory_mutations,
    }
}

/// The bytes of a relative jump whose displacement is the value, whatever its length.
fn encode_relative(code: Code, displacement: i64) -> Vec<u8> {
    let encode = |target: u64| encode_at_code_address(&Encoded::with_branch(code, target).unwrap_or_else(|error| panic!("{code:?}: {error}")));
    let length = encode(NEAR_TARGET).len() as u64;
    encode(CODE_ADDRESS.wrapping_add(length).wrapping_add(displacement as u64))
}
