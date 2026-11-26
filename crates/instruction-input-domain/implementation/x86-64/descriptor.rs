//! The inputs of LAR, LSL, VERR and VERW: the operand size, the registers or the memory operand, the selector, and
//! the flags, of which the forms change ZF alone.
//!
//! The selectors are null with each RPL, each entry of Linux's table (the kernel's, the user segments with RPL 0 and
//! 3, the empty ones, the task state segment's two halves, the LDT's and the TLS entries, the per-CPU segment), one
//! naming the LDT, the first past the table, and every bit; in a register each has bits set above its 16.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::descriptor::DescriptorForm;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::{DataAccess, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR};

use crate::integer::{CodeLayout, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::{CODE_ADDRESS, FlagSet, GENERAL_16, GENERAL_32, GENERAL_64, OperandSize, OperandSizes, OperandType, OperandValue, Splits};

/// What a register holds above the selector in its low 16 bits.
const ABOVE_SELECTOR: u64 = 0xA5A5_A5A5_A5A5_0000;

/// The operand types of a form of this family, in the order the build reads them.
pub fn list_operand_types(form: &DescriptorForm) -> Vec<OperandType> {
    let mut types = Vec::new();
    if form.codes.len() > 1 {
        types.push(OperandType::OperandSize(OperandSizes::All));
    }
    types.push(if form.in_memory {
        OperandType::Memory { splits: Splits::Allowed, offset: false }
    } else {
        OperandType::Encoding(1)
    });
    types.extend([OperandType::Selector, OperandType::Flags(FlagSet::Unread)]);
    types
}

pub fn generate_selectors() -> Vec<OperandValue> {
    [
        0, 3, 0x08, 0x10, 0x1B, 0x20, 0x23, 0x28, 0x2B, 0x31, 0x33, 0x38, 0x43, 0x4B, 0x53, 0x5B, 0x63, 0x73, 0x78, 0x7B,
        0x7F, 0x80, 0xFFFF,
    ]
    .map(OperandValue::Bits)
    .to_vec()
}

/// Builds the state of a form of this family from one value of each of its operand types.
pub fn build_state(form: &DescriptorForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut size, mut registers, mut layout, mut access, mut selector) = (OperandSize::Bits16, [0, 0], CodeLayout::PageStart, None, 0);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::OperandSize(_), OperandValue::OperandSize(chosen)) => size = chosen,
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => (registers, layout) = (encoding.registers, encoding.layout),
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => {
                registers[0] = chosen.register;
                layout = chosen.layout;
                access = Some(chosen);
            }
            (OperandType::Selector, OperandValue::Bits(chosen)) => selector = chosen,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let code = match size {
        OperandSize::Bits16 => form.codes[0],
        OperandSize::Bits32 => form.codes[1],
        OperandSize::Bits64 => form.codes[form.codes.len() - 1],
    };
    let destination = match code {
        Code::Lar_r32_r32m16 | Code::Lsl_r32_r32m16 => GENERAL_32[registers[0]],
        Code::Lar_r64_r64m16 | Code::Lsl_r64_r64m16 => GENERAL_64[registers[0]],
        _ => GENERAL_16[registers[0]],
    };
    let source_register = match code {
        Code::Lar_r32_r32m16 | Code::Lsl_r32_r32m16 => GENERAL_32[registers[1]],
        Code::Lar_r64_r64m16 | Code::Lsl_r64_r64m16 => GENERAL_64[registers[1]],
        Code::Verr_rm16 | Code::Verw_rm16 => GENERAL_16[registers[0]],
        _ => GENERAL_16[registers[1]],
    };
    let one_operand = matches!(code, Code::Verr_rm16 | Code::Verw_rm16);
    let mut memory_mutations = Vec::new();
    let (encoded, padding) = match access {
        Some(access) => {
            let (operand, address) = place_memory_operand(access, 2, &mut processor, &mut memory_mutations);
            let bytes: Vec<u8> = (selector as u16)
                .to_le_bytes()
                .into_iter()
                .enumerate()
                .take_while(|(offset, _)| X86_64::ADDRESSABLE.contains(&address.wrapping_add(*offset as u64)))
                .map(|(_, byte)| byte)
                .collect();
            if !bytes.is_empty() {
                memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(bytes) });
            }
            let encoded = if one_operand { Encoded::with1(code, operand) } else { Encoded::with2(code, destination, operand) };
            (encoded, to_padding(Some(operand)))
        }
        None => {
            processor.general_purpose_registers[registers[if one_operand { 0 } else { 1 }]] = ABOVE_SELECTOR | selector;
            let encoded = if one_operand { Encoded::with1(code, source_register) } else { Encoded::with2(code, destination, source_register) };
            (encoded, to_padding(None))
        }
    };
    let encoded = encoded.unwrap_or_else(|error| panic!("{code:?} takes its operands: {error}"));
    let (code_page, start, bytes) = lay_out(layout, &encode_at_code_address(&encoded), padding);
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}
