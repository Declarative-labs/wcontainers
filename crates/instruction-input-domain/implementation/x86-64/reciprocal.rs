//! The inputs of RCPPS to RSQRTSS: the registers or the memory operand, the source's four lanes, and the flags, which
//! the forms leave alone.
//!
//! Each lane value meets the low lane once, the others following it: zeros and denormals of each sign, the smallest
//! normal, each end of a table entry's span and the next entry's start for each exponent parity, the exponents whose
//! reciprocal leaves the normal range, the largest number, each infinity, a QNaN and an SNaN, and negatives.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::Instruction as Encoded;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::reciprocal::ReciprocalForm;
use wcontainers_processor_architecture_x86_64::{DataAccess, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR};

use crate::integer::{CodeLayout, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::{CODE_ADDRESS, FlagSet, OperandType, OperandValue, Splits, VECTOR};

const LANES: [u32; 24] = [
    0, 0x8000_0000, 1, 0x807F_FFFF, 0x0080_0000, 0x3F80_0000, 0x3F80_0FFF, 0x3F80_1000, 0x3FFF_FFFF, 0x4000_0000,
    0x4000_1FFF, 0x4000_2000, 0x7E7F_FFFF, 0x7E80_0000, 0x7F7F_FFFF, 0x7F80_0000, 0xFF80_0000, 0x7FC0_0001, 0x7FA0_0001,
    0xBF80_0000, 0xC040_0000, 0x3EAA_AAAB, 0x00FF_FFFF, 0x5F00_0000,
];

pub fn list_operand_types(form: &ReciprocalForm) -> Vec<OperandType> {
    let place = if form.place == OperandPlace::Register {
        OperandType::Encoding(2)
    } else {
        OperandType::Memory { splits: Splits::Allowed, offset: false }
    };
    vec![place, OperandType::ReciprocalSource, OperandType::Flags(FlagSet::Unread)]
}

pub fn generate_sources() -> Vec<OperandValue> {
    (0..LANES.len())
        .map(|index| {
            let lanes: [u32; 4] = array::from_fn(|lane| LANES[(index + lane) % LANES.len()]);
            OperandValue::Vector(lanes.iter().rev().fold(0u128, |all, &lane| all << 32 | u128::from(lane)))
        })
        .collect()
}

pub fn build_state(form: &ReciprocalForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut registers, mut layout, mut access, mut source) = ([0, 0], CodeLayout::PageStart, None, 0);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => (registers, layout) = (encoding.registers, encoding.layout),
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => {
                registers[0] = chosen.register;
                layout = chosen.layout;
                access = Some(chosen);
            }
            (OperandType::ReciprocalSource, OperandValue::Vector(value)) => source = value,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let mut memory_mutations = Vec::new();
    let destination = VECTOR[registers[0]];
    let (encoded, padding) = match access {
        Some(access) => {
            let width = form.memory_bytes();
            let (operand, address) = place_memory_operand(access, width, &mut processor, &mut memory_mutations);
            let bytes: Vec<u8> = (0..width)
                .take_while(|offset| X86_64::ADDRESSABLE.contains(&address.wrapping_add(*offset)))
                .map(|offset| source.to_le_bytes()[offset as usize])
                .collect();
            if !bytes.is_empty() {
                memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(bytes) });
            }
            (Encoded::with2(form.code, destination, operand), to_padding(Some(operand)))
        }
        None => {
            processor.vector_registers[registers[1]] = source;
            (Encoded::with2(form.code, destination, VECTOR[registers[1]]), to_padding(None))
        }
    };
    let encoded = encoded.unwrap_or_else(|error| panic!("{:?} takes its operands: {error}", form.code));
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
