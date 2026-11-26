//! The inputs of the stack forms: the operand size, the register or memory operand, where the stack access lies
//! around the data pages' boundary and what those pages allow (or, with an explicit memory operand, a stack on a
//! writable page of its own), the value pushed, popped or jumped to, the immediate, the segment register, and the
//! flags, whose AC checks the stack access too.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded, MemoryOperand, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::stack::{StackForm, StackOperation};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, InstructionFetch, PagePermission, Processor, STACK_POINTER, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{CodeLayout, ImmediateClass, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::{
    AddressSize, CODE_ADDRESS, DataPage, FlagSet, GENERAL_16, GENERAL_32, GENERAL_64, OperandSize,
    OperandSizes, OperandType, OperandValue, Role, Splits, ValueSet, to_access_address, to_data_pages,
};

/// The page a form with an explicit memory operand keeps its stack on, and RSP within it.
const STACK_PAGE: u64 = 0x6000;
const STACK_TOP: u64 = STACK_PAGE + PAGE_BYTES / 2;
const SEGMENTS: [Register; 6] = [Register::ES, Register::CS, Register::SS, Register::DS, Register::FS, Register::GS];
const FRAME_POINTER: usize = 5;
const BASE: usize = 3;
const XLAT: u8 = 0xD7;
const ADDRESS_SIZE_PREFIX: u8 = 0x67;

pub const SEGMENT_NUMBERS: [usize; 6] = [0, 1, 2, 3, 4, 5];

/// Writes the value's bytes from the address, those in the program's addresses.
fn write_at(address: u64, bytes: u64, chosen: u64, mutations: &mut Vec<MemoryDelta<'static, X86_64>>) {
    let bytes: Vec<u8> = (0..bytes)
        .map(|offset| address.wrapping_add(offset))
        .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
        .zip(chosen.to_le_bytes())
        .map(|(_, byte)| byte)
        .collect();
    if !bytes.is_empty() {
        mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(bytes) });
    }
}

/// Whether the form reaches the stack.
const fn uses_stack(form: &StackForm) -> bool {
    !matches!(form.operation, StackOperation::JumpIndirect | StackOperation::Translate | StackOperation::MoveSegment)
}

/// The operand types of a stack form, in the order the build reads them.
pub fn list_operand_types(form: &StackForm) -> Vec<OperandType> {
    use StackOperation as Operation;
    let mut types = Vec::new();
    match form.codes.len() {
        2 => types.push(OperandType::OperandSize(OperandSizes::WithoutThirtyTwo)),
        3 => types.push(OperandType::OperandSize(OperandSizes::All)),
        _ => {}
    }
    let in_memory = form.place != OperandPlace::Register;
    if form.operation == Operation::Translate {
        types.extend([
            OperandType::AddressSize,
            OperandType::AccessPlacement(Splits::Excluded),
            OperandType::DataPagePermission(DataPage::Lower),
            OperandType::DataPagePermission(DataPage::Upper),
            OperandType::RegisterValue(Role::Implicit(0), ValueSet::Byte),
            OperandType::Flags(FlagSet::Unread),
        ]);
        return types;
    }
    if in_memory {
        types.push(OperandType::Memory { splits: Splits::Allowed, offset: false });
    } else {
        let named = matches!(form.codes[0], Code::Push_rm16 | Code::Pop_r16 | Code::Pop_rm16 | Code::Call_rm64 | Code::Mov_rm16_Sreg);
        types.push(OperandType::Encoding(usize::from(named)));
    }
    if uses_stack(form) && !in_memory {
        types.extend([
            OperandType::AccessPlacement(Splits::Allowed),
            OperandType::DataPagePermission(DataPage::Lower),
            OperandType::DataPagePermission(DataPage::Upper),
        ]);
    }
    match (form.operation, in_memory) {
        (Operation::Push, true) | (Operation::Pop | Operation::Leave, false) => types.push(OperandType::MemoryValue(ValueSet::Distinct)),
        (Operation::Push, false) if form.codes[0] == Code::Push_rm16 => {
            types.push(OperandType::RegisterValue(Role::Source, ValueSet::Distinct));
        }
        (Operation::Push, false) => {
            let class = if form.codes[0] == Code::Pushw_imm8 { ImmediateClass::Byte } else { ImmediateClass::Full };
            types.push(OperandType::Immediate(class));
        }
        (Operation::CallIndirect, false) => types.push(OperandType::RegisterValue(Role::Source, ValueSet::Target)),
        (Operation::CallIndirect | Operation::JumpIndirect, true) | (Operation::Return, _) => {
            types.push(OperandType::MemoryValue(ValueSet::Target));
        }
        (Operation::CallRelative, _) => types.push(OperandType::Immediate(ImmediateClass::Relative32)),
        (Operation::MoveSegment, _) => types.push(OperandType::SegmentRegister),
        _ => {}
    }
    if form.codes[0] == Code::Retnq_imm16 {
        types.push(OperandType::Immediate(ImmediateClass::Count));
    }
    types.push(OperandType::Flags(FlagSet::Unread));
    types
}

/// Builds the state of a stack form from one value of each of its operand types.
pub fn build_state(form: &StackForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut size, mut layout, mut register, mut access) = (None, CodeLayout::PageStart, 0, None);
    let (mut placement, mut lower, mut upper) = (None, PagePermission::Unmapped, PagePermission::Unmapped);
    let (mut value, mut memory_value, mut immediate, mut segment, mut address_size) = (None, None, None, 0, AddressSize::Bits64);
    let mut accumulator = 0;
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::OperandSize(_), OperandValue::OperandSize(chosen)) => size = Some(chosen),
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => {
                register = encoding.registers[0];
                layout = encoding.layout;
            }
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => {
                register = chosen.register;
                layout = chosen.layout;
                access = Some(chosen);
            }
            (OperandType::AccessPlacement(_), OperandValue::AccessPlacement(chosen)) => placement = Some(chosen),
            (OperandType::DataPagePermission(DataPage::Lower), OperandValue::PagePermission(chosen)) => lower = chosen,
            (OperandType::DataPagePermission(DataPage::Upper), OperandValue::PagePermission(chosen)) => upper = chosen,
            (OperandType::RegisterValue(Role::Implicit(_), _), OperandValue::Bits(chosen)) => accumulator = chosen & 0xFF,
            (OperandType::RegisterValue(..), OperandValue::Bits(chosen)) => value = Some(chosen),
            (OperandType::MemoryValue(_), OperandValue::Bits(chosen)) => memory_value = Some(chosen),
            (OperandType::Immediate(_), OperandValue::Immediate(chosen)) => immediate = Some(chosen),
            (OperandType::SegmentRegister, OperandValue::Register(number)) => segment = number,
            (OperandType::AddressSize, OperandValue::AddressSize(chosen)) => address_size = chosen,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let code = match size {
        None | Some(OperandSize::Bits16) => form.codes[0],
        Some(OperandSize::Bits32) => form.codes[1],
        Some(OperandSize::Bits64) => form.codes[form.codes.len() - 1],
    };
    let stack_bytes = StackForm::to_stack_bytes(code);
    let mut memory_mutations = Vec::new();
    let general = |number: usize| match code {
        Code::Push_rm16 | Code::Pop_r16 | Code::Pop_rm16 | Code::Mov_rm16_Sreg => GENERAL_16[number],
        Code::Mov_r32m16_Sreg => GENERAL_32[number],
        _ => GENERAL_64[number],
    };
    if let Some(chosen) = value {
        processor.general_purpose_registers[register] = chosen;
    }
    // The stack access around the data pages, or the stack on a page of its own beside an explicit memory operand.
    if let Some(placement) = placement {
        let address = to_access_address(placement, if form.operation == StackOperation::Translate { 1 } else { stack_bytes });
        memory_mutations.extend(to_data_pages(lower, upper));
        match form.operation {
            StackOperation::Translate => {
                let base = address.wrapping_sub(accumulator);
                let base = if address_size == AddressSize::Bits32 { 0xA5A5_A5A5_0000_0000 | (base & u64::from(u32::MAX)) } else { base };
                processor.general_purpose_registers[BASE] = base;
                let held = &mut processor.general_purpose_registers[0];
                *held = (*held & !0xFF) | accumulator;
            }
            StackOperation::Leave => processor.general_purpose_registers[FRAME_POINTER] = address,
            _ if form.pushes() => processor.general_purpose_registers[STACK_POINTER] = address.wrapping_add(stack_bytes),
            _ => processor.general_purpose_registers[STACK_POINTER] = address,
        }
        if let Some(chosen) = memory_value {
            write_at(address, stack_bytes, chosen, &mut memory_mutations);
        }
    } else if uses_stack(form) {
        processor.general_purpose_registers[STACK_POINTER] = STACK_TOP;
        memory_mutations.extend([
            MemoryDelta::ValueAssignment { start: STACK_PAGE, bytes: Cow::Owned(vec![0x5A; PAGE_BYTES as usize]) },
            MemoryDelta::PermissionAssignment {
                addresses: STACK_PAGE..STACK_PAGE + PAGE_BYTES,
                permission: PagePermission::Mapped { data_access: DataAccess::ReadWrite, instruction_fetch: InstructionFetch::Forbidden },
            },
        ]);
    }
    let memory_operand: Option<MemoryOperand> = access.map(|access| {
        let width = StackForm::to_memory_bytes(code);
        let (operand, address) = place_memory_operand(access, width, &mut processor, &mut memory_mutations);
        if let Some(chosen) = memory_value {
            write_at(address, width, chosen, &mut memory_mutations);
        }
        operand
    });
    let to_error = |error| panic!("{code:?} takes its operands: {error}");
    let encoded_bytes = if form.operation == StackOperation::Translate {
        if address_size == AddressSize::Bits32 { vec![ADDRESS_SIZE_PREFIX, XLAT] } else { vec![XLAT] }
    } else {
        let encoded = match (code, memory_operand) {
            (Code::Call_rel32_64, _) => {
                let displacement = immediate.expect("a displacement");
                Encoded::with_branch(code, CODE_ADDRESS.wrapping_add(5).wrapping_add(displacement as u64))
            }
            (Code::Mov_rm16_Sreg | Code::Mov_r32m16_Sreg | Code::Mov_r64m16_Sreg, Some(operand)) => {
                Encoded::with2(code, operand, SEGMENTS[segment])
            }
            (Code::Mov_rm16_Sreg | Code::Mov_r32m16_Sreg | Code::Mov_r64m16_Sreg, None) => {
                Encoded::with2(code, general(register), SEGMENTS[segment])
            }
            (_, Some(operand)) => Encoded::with1(code, operand),
            (Code::Push_rm16 | Code::Push_rm64 | Code::Pop_r16 | Code::Pop_r64 | Code::Pop_rm16 | Code::Pop_rm64 | Code::Call_rm64, None) => {
                Encoded::with1(code, general(register))
            }
            (Code::Pushw_imm8 | Code::Pushq_imm8 | Code::Pushq_imm32, None) => Encoded::with1(code, immediate.expect("an immediate") as i32),
            (Code::Push_imm16 | Code::Retnq_imm16, None) => Encoded::with1(code, immediate.expect("an immediate") as u16 as u32),
            (Code::Pushw_FS | Code::Pushq_FS, None) => Encoded::with1(code, Register::FS),
            (Code::Pushw_GS | Code::Pushq_GS, None) => Encoded::with1(code, Register::GS),
            _ => Ok(Encoded::with(code)),
        }
        .unwrap_or_else(to_error);
        encode_at_code_address(&encoded)
    };
    let (code_page, start, bytes) = lay_out(layout, &encoded_bytes, to_padding(memory_operand));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}
