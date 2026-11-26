use alloc::vec;
use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::encoding::{Field, RN};
use wcontainers_processor_architecture_arm64::operand::{Operand, RegisterThirtyOne};
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadStoreInstruction, User, VIRTUAL_ADDRESS_BITS,
};

use crate::{
    AccessPlacement, Arm64InputDomain, CODE_ADDRESS, CodePlacement, Comparison, DataPage,
    GENERAL_PURPOSE_REGISTERS, LOWER_DATA_ADDRESS, MemoryFill, OperandType, OperandValue,
    UPPER_DATA_ADDRESS, to_access_address,
};

const PLACEMENTS: [AccessPlacement; 7] = [
    AccessPlacement::LowerPageStart,
    AccessPlacement::ByteAfterLowerPageStart,
    AccessPlacement::AcrossEightBytes,
    AccessPlacement::AcrossSixteenBytes,
    AccessPlacement::EndsAtBoundary,
    AccessPlacement::StraddlesBoundary,
    AccessPlacement::UpperPageStart,
];

fn read_word(state: &ExecutionState<'static, Arm64<User>>) -> Option<u32> {
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

/// The byte the state's data pages hold at the address, none outside them.
fn read_data_byte(state: &ExecutionState<'static, Arm64<User>>, address: u64) -> Option<u8> {
    state.memory_mutations.iter().find_map(|delta| match delta {
        MemoryDelta::ValueAssignment { start, bytes } if *start == LOWER_DATA_ADDRESS => {
            bytes.get(address.checked_sub(*start)? as usize).copied()
        }
        MemoryDelta::ValueAssignment { .. } | MemoryDelta::PermissionAssignment { .. } => None,
    })
}

/// Reads one operand back from a state; the zero register reads as zero.
fn read_operand(
    state: &ExecutionState<'static, Arm64<User>>,
    instruction: Instruction<Infallible>,
    word: u32,
    operand_type: OperandType,
) -> Option<OperandValue> {
    let registers = &state.processor.general_purpose_registers;
    match operand_type {
        OperandType::RegisterNumber(field)
        | OperandType::VectorRegisterNumber(field)
        | OperandType::PredicateRegisterNumber(field) => {
            Some(OperandValue::RegisterNumber(field.read(word)))
        }
        OperandType::Pattern(field) => Some(OperandValue::Bits(u64::from(field.read(word)))),
        OperandType::GoverningPredicate(field) | OperandType::PredicateValue(field) => state
            .processor
            .predicate_registers
            .get(field.read(word) as usize)
            .copied()
            .map(OperandValue::Predicate),
        OperandType::FirstFaultValue | OperandType::WrittenFirstFault => Some(
            OperandValue::Predicate(state.processor.first_fault_register),
        ),
        OperandType::NextVectorValue(field) => Some(OperandValue::Vector(
            state.processor.vector_registers[(field.read(word) as usize + 1) % 32],
        )),
        OperandType::ShiftAmountVector(field) => Some(OperandValue::Vector(
            state.processor.vector_registers[field.read(word) as usize],
        )),
        OperandType::RegisterBound(field) | OperandType::RegisterNearSaturation(field) => {
            Some(OperandValue::Bits(
                registers
                    .get(field.read(word) as usize)
                    .copied()
                    .unwrap_or(0),
            ))
        }
        OperandType::FloatingPointValue { field, precision }
        | OperandType::ConvertedFloatingPoint {
            field, precision, ..
        } => Some(OperandValue::Bits(
            (state.processor.vector_registers[field.read(word) as usize]
                & (u128::MAX >> (u128::BITS - precision.to_bits()))) as u64,
        )),
        OperandType::FloatingPointControl(_) => {
            Some(OperandValue::Bits(state.processor.floating_point_control))
        }
        OperandType::StatusFlags => Some(OperandValue::Bits(state.processor.floating_point_status)),
        OperandType::MemoryFill => {
            // The second byte tells the fills apart: the pattern's first byte is all ones.
            let byte = read_data_byte(state, LOWER_DATA_ADDRESS + 1)?;
            Some(OperandValue::MemoryFill(match byte {
                0 => MemoryFill::Zeros,
                u8::MAX => MemoryFill::Ones,
                _ => MemoryFill::Pattern,
            }))
        }
        OperandType::ConvertedInteger { field, .. } => Some(OperandValue::Bits(
            registers
                .get(field.read(word) as usize)
                .copied()
                .unwrap_or(0),
        )),
        OperandType::UnsignedImmediate(field)
        | OperandType::SignedImmediate(field)
        | OperandType::ShiftAmount { field, .. }
        | OperandType::BitmaskImmediate(field)
        | OperandType::SystemRegister(field) => {
            Some(OperandValue::Bits(u64::from(field.read(word))))
        }
        OperandType::SoftwareThreadId => {
            Some(OperandValue::Bits(state.processor.software_thread_id))
        }
        OperandType::Word(_) => Some(OperandValue::Bits(u64::from(word))),
        OperandType::VectorValue { field, bytes } => Some(OperandValue::Vector(
            state.processor.vector_registers[field.read(word) as usize]
                & (u128::MAX >> (u128::BITS - bytes * u8::BITS)),
        )),
        OperandType::ProgramCounterOffset(fields) | OperandType::SplitImmediate(fields) => {
            Some(OperandValue::Bits(fields.iter().fold(0, |bits, field| {
                (bits << field.width) | u64::from(field.read(word))
            })))
        }
        OperandType::RegisterValue {
            field, thirty_one, ..
        }
        | OperandType::Address { field, thirty_one } => Some(OperandValue::Bits(
            registers
                .get(field.read(word) as usize)
                .copied()
                .unwrap_or(match thirty_one {
                    RegisterThirtyOne::ZeroRegister => 0,
                    RegisterThirtyOne::StackPointer => state.processor.stack_pointer,
                }),
        )),
        OperandType::Comparand { field, bytes, pair } => {
            let layout =
                LoadStoreInstruction::new(instruction)?.to_access_layout(word, &state.processor)?;
            let base = registers
                .get(RN.read(word) as usize)
                .copied()
                .unwrap_or(state.processor.stack_pointer);
            let address = untag(base.wrapping_add(layout.offset));
            // Each register of a pair is read back but one that is the base, which holds the base's value.
            let register_bytes = if pair { bytes / 2 } else { bytes };
            let count = if pair { 2 } else { 1 };
            let register_bits = (register_bytes * u64::from(u8::BITS)) as u32;
            let above = u64::MAX.checked_shl(register_bits).unwrap_or(0);
            let highest = 0xFF_u64 << (register_bits - u8::BITS);
            let mut kinds = Vec::new();
            for index in 0..count {
                let number = field.read(word) + index as u32;
                if number == RN.read(word) {
                    continue;
                }
                let start = address.wrapping_add(index * register_bytes);
                let mut held = 0;
                for offset in 0..register_bytes {
                    let byte = read_data_byte(state, start.wrapping_add(offset))?;
                    held |= u64::from(byte) << (offset * u64::from(u8::BITS));
                }
                let value = registers.get(number as usize).copied().unwrap_or(0);
                kinds.push(if value == held {
                    Comparison::Equal
                } else if above != 0 && value == held | above {
                    Comparison::EqualInComparedBytes
                } else if index + 1 == count && value == held ^ highest {
                    Comparison::DiffersInHighestByte
                } else {
                    Comparison::Unequal
                });
            }
            Some(OperandValue::Comparison(
                if kinds.contains(&Comparison::DiffersInHighestByte) {
                    Comparison::DiffersInHighestByte
                } else {
                    kinds.first().copied().unwrap_or(Comparison::Equal)
                },
            ))
        }
        OperandType::ConditionFlags | OperandType::WrittenConditionFlags => Some(
            OperandValue::ConditionFlags(state.processor.condition_flags),
        ),
        OperandType::AccessPlacement => {
            let layout =
                LoadStoreInstruction::new(instruction)?.to_access_layout(word, &state.processor)?;
            let base = registers
                .get(RN.read(word) as usize)
                .copied()
                .unwrap_or(state.processor.stack_pointer);
            let address = untag(base.wrapping_add(layout.offset));
            PLACEMENTS
                .into_iter()
                .find(|placement| to_access_address(*placement, layout.bytes) == address)
                .map(OperandValue::AccessPlacement)
        }
        OperandType::AddressTag => {
            let layout =
                LoadStoreInstruction::new(instruction)?.to_access_layout(word, &state.processor)?;
            let base = registers
                .get(RN.read(word) as usize)
                .copied()
                .unwrap_or(state.processor.stack_pointer);
            Some(OperandValue::Bits(
                base.wrapping_add(layout.offset) & (u64::MAX << VIRTUAL_ADDRESS_BITS),
            ))
        }
        OperandType::CodePlacement => Some(OperandValue::CodePlacement(
            if state.processor.program_counter == CODE_ADDRESS {
                CodePlacement::PageStart
            } else {
                CodePlacement::PageEnd
            },
        )),
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

/// The address without the bits above the 48 a program can use.
fn untag(address: u64) -> u64 {
    address & !(u64::MAX << VIRTUAL_ADDRESS_BITS)
}

/// Whether the instruction adds a register to its base that is its base.
fn is_base_also_offset(instruction: Instruction<Infallible>, word: u32) -> bool {
    instruction.to_record().is_some_and(|record| {
        record.operands.iter().any(|operand| {
            matches!(operand, Operand::RegisterOffset { number, .. } if number.read(word) == RN.read(word))
        })
    })
}

/// Every listed instruction that has a known behavior, in the list's order.
fn list_modeled_instructions() -> impl Iterator<Item = Instruction<Infallible>> {
    Arm64::<User>::list_instructions().filter(|instruction| {
        Arm64::<User>::list_known_behaviors(*instruction)
            .next()
            .is_some()
    })
}

/// The ten instructions of the first slice.
const FIRST_SLICE: [Instruction<Infallible>; 10] = [
    Instruction::AddShiftedRegister64,
    Instruction::AddsShiftedRegister64,
    Instruction::Csel64,
    Instruction::LdrImmediateUnsignedOffset64,
    Instruction::StrImmediateUnsignedOffset64,
    Instruction::StpSignedOffset64,
    Instruction::LdrImmediatePostIndex64,
    Instruction::Brk,
    Instruction::Rdvl,
    Instruction::MrsCtrEl0,
];

#[test]
fn states_per_instruction() {
    let counts: Vec<(Instruction<Infallible>, usize)> = FIRST_SLICE
        .into_iter()
        .map(|instruction| {
            let states = Arm64InputDomain.generate_critical_states(instruction);
            (instruction, states.count())
        })
        .collect();
    assert_eq!(
        counts,
        [
            (Instruction::AddShiftedRegister64, 42_336),
            (Instruction::AddsShiftedRegister64, 84_672),
            (Instruction::Csel64, 338_688),
            (Instruction::LdrImmediateUnsignedOffset64, 56_448),
            (Instruction::StrImmediateUnsignedOffset64, 1_185_408),
            (Instruction::StpSignedOffset64, 17_781_120),
            (Instruction::LdrImmediatePostIndex64, 48_384),
            (Instruction::Brk, 18),
            (Instruction::Rdvl, 27),
            (Instruction::MrsCtrEl0, 3),
        ]
    );
}

/// The states of an instruction checked by decoding them, spread evenly over all of them.
const DECODED_STATES: usize = 1 << 12;

/// Each state's next instruction, decoded, is the instruction asked for with the operands asked for.
///
/// The one exception is a register value the state cannot hold: one given to the zero register, or one
/// overwritten by a later value for the same register. The operands read back still build the same state.
#[test]
fn every_state_decodes_to_its_instruction_and_operands() {
    let domain = Arm64InputDomain;
    for instruction in list_modeled_instructions() {
        let fixed = instruction.to_fixed_bits();
        let operand_types: Vec<OperandType> = domain.list_operand_types(instruction).collect();
        let values: Vec<Vec<OperandValue>> = operand_types
            .iter()
            .map(|operand_type| {
                domain
                    .generate_critical_values(*operand_type)
                    .collect::<Vec<_>>()
            })
            .collect();
        let count: u128 = values.iter().map(|values| values.len() as u128).product();
        let step = (count / DECODED_STATES as u128).max(1);
        // Every `step`-th combination, its index read as one digit for each operand, the last the fastest.
        let combinations = (0..count.div_ceil(step)).map(|place| {
            let mut index = place * step;
            let mut operands = vec![OperandValue::RegisterNumber(0); values.len()];
            for (operand, values) in operands.iter_mut().zip(&values).rev() {
                *operand = values[(index % values.len() as u128) as usize];
                index /= values.len() as u128;
            }
            operands
        });
        for operands in combinations {
            let state = domain.build_state(instruction, &operands);
            let Some(word) = read_word(&state) else {
                panic!("no instruction at the program counter for {instruction:?} {operands:?}");
            };
            assert_eq!(word & fixed.mask, fixed.value, "{instruction:?}");
            // A base that is also the register offset cannot hold the value that places the access.
            let base_is_offset = is_base_also_offset(instruction, word);
            let placed = operand_types.contains(&OperandType::AccessPlacement);
            let read: Vec<OperandValue> = operand_types
                .iter()
                .zip(&operands)
                .map(
                    |(operand_type, asked)| match (operand_type, base_is_offset) {
                        (OperandType::AccessPlacement | OperandType::AddressTag, true) => {
                            Some(*asked)
                        }
                        _ => read_operand(&state, instruction, word, *operand_type),
                    },
                )
                .collect::<Option<_>>()
                .unwrap_or_else(|| {
                    panic!("an operand of {instruction:?} {operands:?} reads back as nothing")
                });
            assert!(
                base_is_offset || domain.build_state(instruction, &read) == state,
                "{instruction:?} {operands:?} reads back as {read:?}, which builds another state"
            );
            for (index, (asked, held)) in operands.iter().zip(&read).enumerate() {
                // Two placements of an access too short to straddle the boundary put it at the same address, and a
                // comparand in the zero register or the base holds what it can.
                let placements = matches!(
                    (asked, held),
                    (
                        OperandValue::AccessPlacement(_),
                        OperandValue::AccessPlacement(_)
                    ) | (OperandValue::Comparison(_), OperandValue::Comparison(_))
                );
                if asked == held || placements {
                    continue;
                }
                // A floating-point register named twice holds its last value.
                if let Some(
                    OperandType::FloatingPointValue { field, .. }
                    | OperandType::VectorValue { field, .. },
                ) = operand_types.get(index)
                    && operand_types[index + 1..].iter().any(|later| {
                        matches!(later, OperandType::FloatingPointValue { field: other, .. } | OperandType::VectorValue { field: other, .. } if other.read(word) == field.read(word))
                    })
                {
                    continue;
                }
                // A Z register or a predicate named twice holds the value written into it last.
                if let Some((register, place)) =
                    to_vector_or_predicate(operand_types[index], word, index)
                    && operand_types
                        .iter()
                        .enumerate()
                        .any(|(other, operand_type)| {
                            to_vector_or_predicate(*operand_type, word, other).is_some_and(
                                |(later, later_place)| later == register && later_place > place,
                            )
                        })
                {
                    continue;
                }
                let Some(
                    OperandType::RegisterValue { field, .. }
                    | OperandType::Address { field, .. }
                    | OperandType::ConvertedInteger { field, .. }
                    | OperandType::RegisterBound(field)
                    | OperandType::RegisterNearSaturation(field),
                ) = operand_types.get(index)
                else {
                    panic!("{instruction:?} {operands:?} holds {held:?} for {asked:?}");
                };
                let number = field.read(word);
                let names = |later: &Field| later.read(word) == number;
                // A comparand is written last, so it holds the register for every operand that names it.
                let shared = operand_types.iter().enumerate().any(|(other, operand_type)| {
                    matches!(operand_type, OperandType::Comparand { field: later, .. } if names(later))
                        || other > index
                            && matches!(operand_type, OperandType::RegisterValue { field: later, .. } | OperandType::Address { field: later, .. } | OperandType::RegisterBound(later) | OperandType::RegisterNearSaturation(later) if names(later))
                });
                // A placed access's base holds the address that places it.
                let based = placed && number == RN.read(word);
                assert!(
                    number as usize == GENERAL_PURPOSE_REGISTERS || shared || based,
                    "{instruction:?} {operands:?} holds {held:?} for {asked:?}"
                );
            }
        }
    }
}

/// The Z register or predicate a value operand writes, as a vector register's number or a predicate's past the vector
/// registers, and where its write comes among the state's: the values of the next register of a pair after every other
/// vector value, each kind in the operands' order.
fn to_vector_or_predicate(
    operand_type: OperandType,
    word: u32,
    index: usize,
) -> Option<(usize, (usize, usize))> {
    const VECTOR_REGISTERS: usize = 32;
    let number = |field: Field| field.read(word) as usize;
    match operand_type {
        OperandType::VectorValue { field, .. } | OperandType::ShiftAmountVector(field) => {
            Some((number(field), (0, index)))
        }
        OperandType::NextVectorValue(field) => {
            Some(((number(field) + 1) % VECTOR_REGISTERS, (1, index)))
        }
        OperandType::GoverningPredicate(field) | OperandType::PredicateValue(field) => {
            Some((VECTOR_REGISTERS + number(field), (0, index)))
        }
        _ => None,
    }
}
