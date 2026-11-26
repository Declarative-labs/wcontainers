//! The inputs of PUSHF, POPF and ENTER: where the stack access lies around the data pages' boundary and what those
//! pages allow, or how the code lies; the flags, which PUSHF pushes and whose AC checks the stack; the image POPF pops;
//! and ENTER's operand size, frame size and nesting level, with RBP pointing into the lower data page, whose bytes
//! the outer levels' frame pointers are copied from.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::system::{SystemForm, SystemOperation, find_system_form};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, Flags, Instruction, InstructionFetch, PagePermission, Processor, STACK_POINTER, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{CodeLayout, lay_out};
use crate::string::{PLAIN, Placement, list_placements, place, refuses, to_address};
use crate::{CODE_ADDRESS, FlagSet, LOWER_DATA_ADDRESS, OperandSize, OperandSizes, OperandType, OperandValue};

const FRAME_POINTER: usize = 5;
/// Where RBP points for ENTER: inside the lower data page, so that the outer levels' frame pointers are read from it.
const FRAME_ADDRESS: u64 = LOWER_DATA_ADDRESS + 0x800;
const OPERAND_SIZE_PREFIX: u8 = 0x66;

/// Where the stack access lies, or the plain one with the code laid out otherwise.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SystemLayout {
    pub placement: Placement,
    pub code: CodeLayout,
}

/// The operand types of a form of this family, in the order the build reads them.
pub fn list_operand_types(form: &SystemForm) -> Vec<OperandType> {
    let mut types = Vec::new();
    // No byte string encodes JCXZ in 64-bit mode, so it has no state.
    if form.operation == SystemOperation::JumpIfCountZero {
        return types;
    }
    if form.codes.len() == 2 {
        types.push(OperandType::OperandSize(OperandSizes::WithoutThirtyTwo));
    }
    let bytes = if form.codes[0] == iced_x86::Code::Pushfw || form.codes[0] == iced_x86::Code::Popfw { 2 } else { 8 };
    types.push(OperandType::SystemLayout { bytes });
    match form.operation {
        SystemOperation::PushFlags => types.push(OperandType::SystemFlags),
        SystemOperation::PopFlags => types.extend([OperandType::Flags(FlagSet::Unread), OperandType::PoppedFlags]),
        SystemOperation::Enter => types.extend([OperandType::Flags(FlagSet::Unread), OperandType::EnterOperands]),
        SystemOperation::JumpIfCountZero => {}
        SystemOperation::SystemCall => {
            types.clear();
            types.extend([OperandType::Encoding(0), OperandType::SystemFlags]);
        }
    }
    types
}

/// Each placement of an access of `bytes`, then the plain one with each code layout.
pub fn generate_layouts(bytes: u64) -> Vec<OperandValue> {
    list_placements(bytes)
        .into_iter()
        .map(|placement| SystemLayout { placement, code: CodeLayout::PageStart })
        .chain(
            [CodeLayout::Padded15, CodeLayout::Padded16, CodeLayout::AcrossPageIntoUnmapped]
                .map(|code| SystemLayout { placement: PLAIN, code }),
        )
        .map(OperandValue::SystemLayout)
        .collect()
}

/// The flags PUSHF pushes: none, all, the status flags alone, and the other flags alone, AC among them.
pub fn generate_flags() -> Vec<OperandValue> {
    [Flags::empty(), Flags::all(), Flags::STATUS, Flags::all().difference(Flags::STATUS)].map(OperandValue::Flags).to_vec()
}

/// The images POPF pops: none, every bit, the flags a program may change, TF, IF with IOPL, RF with VM, VIF and VIP,
/// ID, AC with NT, the status flags with DF, and the reserved bits.
pub fn generate_popped() -> Vec<OperandValue> {
    [
        0,
        u64::MAX,
        Flags::all().bits(),
        1 << 8,
        1 << 9 | 0b11 << 12,
        1 << 16 | 1 << 17 | 1 << 19 | 1 << 20,
        1 << 21,
        1 << 18 | 1 << 14,
        Flags::STATUS.bits() | 1 << 10,
        1 << 1 | 1 << 3 | 1 << 5 | 1 << 15 | 0xFFFF_FFFF_FFC0_0000,
    ]
    .map(OperandValue::Bits)
    .to_vec()
}

/// ENTER's frame size and nesting level: no frame, a frame of one element, of a page and of the most a frame takes,
/// with no outer level, one, two, 33 (which is 1 modulo 32) and the last, 31.
pub fn generate_enter_operands() -> Vec<OperandValue> {
    [(0, 0), (8, 0), (0x1000, 0), (0xFFFF, 0), (0, 1), (8, 2), (0, 33), (0x1000, 0xFF), (0xFFFF, 5)]
        .map(|(allocation, level)| OperandValue::EnterOperands { allocation, level })
        .to_vec()
}

/// Builds the state of a form of this family from one value of each of its operand types.
pub fn build_state(form: &SystemForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut size, mut layout, mut popped, mut enter) = (None, None, None, None);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::OperandSize(_), OperandValue::OperandSize(chosen)) => size = Some(chosen),
            (OperandType::SystemLayout { .. }, OperandValue::SystemLayout(chosen)) => layout = Some(chosen),
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => {
                layout = Some(SystemLayout { placement: PLAIN, code: encoding.layout });
            }
            (OperandType::SystemFlags | OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (OperandType::PoppedFlags, OperandValue::Bits(value)) => popped = Some(value),
            (OperandType::EnterOperands, OperandValue::EnterOperands { allocation, level }) => enter = Some((allocation, level)),
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let layout: SystemLayout = layout.expect("a layout");
    let narrow = size == Some(OperandSize::Bits16) || form.codes[0] == iced_x86::Code::Pushfw || form.codes[0] == iced_x86::Code::Popfw;
    let bytes = if narrow { 2 } else { 8 };
    let mut memory_mutations = Vec::new();
    if form.operation != SystemOperation::SystemCall {
        place(layout.placement, false, &mut memory_mutations);
    }
    let address = to_address(layout.placement, bytes, false);
    processor.general_purpose_registers[STACK_POINTER] =
        if form.operation == SystemOperation::PopFlags { address } else { address.wrapping_add(bytes) };
    if let Some(value) = popped {
        let written: Vec<u8> = value.to_le_bytes()[..bytes as usize].to_vec();
        if address < 1 << 47 {
            memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(written) });
        }
    }
    let mut encoded = Vec::new();
    if narrow && form.operation == SystemOperation::Enter {
        encoded.push(OPERAND_SIZE_PREFIX);
    }
    match form.operation {
        SystemOperation::PushFlags | SystemOperation::PopFlags => {
            if narrow {
                encoded.push(OPERAND_SIZE_PREFIX);
            }
            encoded.push(if form.operation == SystemOperation::PushFlags { 0x9C } else { 0x9D });
        }
        SystemOperation::JumpIfCountZero => unreachable!("JCXZ has no state"),
        SystemOperation::SystemCall => encoded.extend([0x0F, 0x05]),
        SystemOperation::Enter => {
            let (allocation, level) = enter.expect("ENTER's operands");
            processor.general_purpose_registers[FRAME_POINTER] = FRAME_ADDRESS;
            encoded.extend([0xC8, allocation as u8, (allocation >> 8) as u8, level]);
        }
    }
    let (code_page, start, code) = lay_out(layout.code, &encoded, 0x3E);
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(code) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}

/// Whether the values decide the state's outcome whatever POPF pops: the code faults while it is fetched or
/// measured, or the stack access faults.
pub fn decides_without_data(instruction: Instruction, operands: &[Option<OperandValue>]) -> bool {
    if find_system_form(instruction).is_none_or(|form| form.operation != SystemOperation::PopFlags) {
        return false;
    }
    let (mut layout, mut flags) = (None, Flags::empty());
    for operand in operands.iter().flatten() {
        match *operand {
            OperandValue::SystemLayout(chosen) => layout = Some(chosen),
            OperandValue::Flags(chosen) => flags = chosen,
            _ => {}
        }
    }
    let bytes = if instruction == Instruction::Popf { 2 } else { 8 };
    layout.is_some_and(|layout: SystemLayout| {
        layout.code.faults_before_execution() || refuses(layout.placement, bytes, flags.contains(Flags::ALIGNMENT_CHECK), false, false)
    })
}
