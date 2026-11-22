use alloc::vec::Vec;

use itertools::Itertools;
use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::encoding::RN;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction};

use crate::{
    AccessPlacement, Arm64InputDomain, DataPage, GENERAL_PURPOSE_REGISTERS, LOWER_DATA_ADDRESS,
    OperandType, OperandValue, UPPER_DATA_ADDRESS, to_access_address, to_addressing,
};

const PLACEMENTS: [AccessPlacement; 4] = [
    AccessPlacement::LowerPageStart,
    AccessPlacement::EndsAtBoundary,
    AccessPlacement::StraddlesBoundary,
    AccessPlacement::UpperPageStart,
];

fn read_word(state: &ExecutionState<'static, Arm64>) -> Option<u32> {
    state
        .memory_mutations
        .iter()
        .find_map(|delta| match delta {
            MemoryDelta::ValueAssignment { start, bytes }
                if *start == state.processor.program_counter =>
            {
                <[u8; 4]>::try_from(&bytes[..]).ok()
            }
            MemoryDelta::ValueAssignment { .. } | MemoryDelta::PermissionAssignment { .. } => None,
        })
        .map(u32::from_le_bytes)
}

/// Reads one operand back from a state; the zero register reads as zero.
fn read_operand(
    state: &ExecutionState<'static, Arm64>,
    instruction: Instruction,
    word: u32,
    operand_type: OperandType,
) -> Option<OperandValue> {
    let registers = &state.processor.general_purpose_registers;
    match operand_type {
        OperandType::RegisterNumber(field) => Some(OperandValue::RegisterNumber(field.read(word))),
        OperandType::UnsignedImmediate(field) | OperandType::SignedImmediate(field) => {
            Some(OperandValue::Bits(u64::from(field.read(word))))
        }
        OperandType::RegisterValue(field) => Some(OperandValue::Bits(
            registers
                .get(field.read(word) as usize)
                .copied()
                .unwrap_or(0),
        )),
        OperandType::ConditionFlags => Some(OperandValue::ConditionFlags(
            state.processor.condition_flags,
        )),
        OperandType::AccessPlacement => {
            let addressing = to_addressing(instruction, word)?;
            let base = registers
                .get(RN.read(word) as usize)
                .copied()
                .unwrap_or(state.processor.stack_pointer);
            let address = base.wrapping_add(addressing.offset);
            PLACEMENTS
                .into_iter()
                .find(|placement| to_access_address(*placement, addressing.access_bytes) == address)
                .map(OperandValue::AccessPlacement)
        }
        OperandType::DataPagePermission(page) => {
            let page_address = match page {
                DataPage::Lower => LOWER_DATA_ADDRESS,
                DataPage::Upper => UPPER_DATA_ADDRESS,
            };
            state.memory_mutations.iter().find_map(|delta| match delta {
                MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } if addresses.start == page_address => {
                    Some(OperandValue::PagePermission(*permission))
                }
                MemoryDelta::PermissionAssignment { .. } | MemoryDelta::ValueAssignment { .. } => {
                    None
                }
            })
        }
    }
}

#[test]
fn states_per_instruction() {
    let counts: Vec<(Instruction, usize)> = Arm64::list_instructions()
        .map(|instruction| {
            let states = Arm64InputDomain.generate_critical_states(instruction);
            (instruction, states.count())
        })
        .collect();
    assert_eq!(
        counts,
        [
            (Instruction::AddShiftedRegister64, 5184),
            (Instruction::AddsShiftedRegister64, 5184),
            (Instruction::Csel64, 6912),
            (Instruction::LdrImmediateUnsignedOffset64, 1728),
            (Instruction::StrImmediateUnsignedOffset64, 1728),
            (Instruction::StpSignedOffset64, 8640),
            (Instruction::LdrImmediatePostIndex64, 2880),
            (Instruction::Brk, 3),
            (Instruction::Rdvl, 15),
            (Instruction::MrsCtrEl0, 3),
        ]
    );
}

/// Each state's next instruction, decoded, is the instruction asked for with the operands asked for.
///
/// The one exception is a register value the state cannot hold: one given to the zero register, or one
/// overwritten by a later value for the same register. The operands read back still build the same state.
#[test]
fn every_state_decodes_to_its_instruction_and_operands() {
    let domain = Arm64InputDomain;
    for instruction in Arm64::list_instructions() {
        let fixed = instruction.to_fixed_bits();
        let operand_types: Vec<OperandType> = domain.list_operand_types(instruction).collect();
        let combinations = operand_types
            .iter()
            .map(|operand_type| {
                domain
                    .generate_critical_values(*operand_type)
                    .collect::<Vec<_>>()
            })
            .multi_cartesian_product();
        for operands in combinations {
            let state = domain.build_state(instruction, &operands);
            let Some(word) = read_word(&state) else {
                panic!("no instruction at the program counter for {instruction:?} {operands:?}");
            };
            assert_eq!(word & fixed.mask, fixed.value, "{instruction:?}");
            let read: Vec<OperandValue> = operand_types
                .iter()
                .map(|operand_type| read_operand(&state, instruction, word, *operand_type))
                .collect::<Option<_>>()
                .unwrap_or_else(|| {
                    panic!("an operand of {instruction:?} {operands:?} reads back as nothing")
                });
            assert!(
                domain.build_state(instruction, &read) == state,
                "{instruction:?} {operands:?} reads back as {read:?}, which builds another state"
            );
            for (index, (asked, held)) in operands.iter().zip(&read).enumerate() {
                if asked == held {
                    continue;
                }
                let Some(OperandType::RegisterValue(field)) = operand_types.get(index) else {
                    panic!("{instruction:?} {operands:?} holds {held:?} for {asked:?}");
                };
                let number = field.read(word);
                let shared = operand_types.iter().enumerate().any(|(other, operand_type)| {
                    other > index
                        && matches!(operand_type, OperandType::RegisterValue(later) if later.read(word) == number)
                });
                assert!(
                    number as usize == GENERAL_PURPOSE_REGISTERS || shared,
                    "{instruction:?} {operands:?} holds {held:?} for {asked:?}"
                );
            }
        }
    }
}
