//! The inputs of the ARM64 instructions.
//!
//! Every state holds the instruction on a page of its own and, for an instruction that reads or writes memory,
//! two data pages right above it whose permissions are operands. Each data byte holds the complement of its
//! address's low byte, so that no loaded value equals an address near it.

#![no_std]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use itertools::Itertools;

use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_arm64::encoding::{
    COND, Field, IMM7, IMM9, IMM12, IMM16, RD, RDVL_IMM6, RM, RN, RT, RT2, SHIFT, SHIFT_IMM6,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, DataAccess, GENERAL_PURPOSE_REGISTERS, Instruction, InstructionFetch,
    PagePermission, Processor, ZERO_PROCESSOR,
};

#[cfg(test)]
mod tests;

/// The 4 KiB translation granule the executors use.
const PAGE_BYTES: u64 = 1 << 12;
/// The first page above the page at zero, which stays unmapped as on Linux.
const CODE_ADDRESS: u64 = PAGE_BYTES;
const LOWER_DATA_ADDRESS: u64 = CODE_ADDRESS + PAGE_BYTES;
const UPPER_DATA_ADDRESS: u64 = LOWER_DATA_ADDRESS + PAGE_BYTES;
const REGISTER_BYTES: u64 = size_of::<u64>() as u64;
/// A field this narrow gets every value it can hold.
const EXHAUSTIVE_FIELD_BITS: u32 = 4;

#[derive(Clone, Copy, Default, Debug)]
pub struct Arm64InputDomain;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandType {
    /// The register an encoding field names.
    RegisterNumber(Field),
    /// The value of the register an encoding field names.
    RegisterValue(Field),
    UnsignedImmediate(Field),
    SignedImmediate(Field),
    ConditionFlags,
    /// Where the first byte an instruction reads or writes lies, around the boundary of the two data pages.
    AccessPlacement,
    DataPagePermission(DataPage),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataPage {
    Lower,
    Upper,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccessPlacement {
    LowerPageStart,
    EndsAtBoundary,
    StraddlesBoundary,
    UpperPageStart,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandValue {
    RegisterNumber(u32),
    /// A register's value, or an immediate's bits.
    Bits(u64),
    ConditionFlags(ConditionFlags),
    AccessPlacement(AccessPlacement),
    PagePermission(PagePermission),
}

impl InstructionInputDomain<Arm64> for Arm64InputDomain {
    type OperandType = OperandType;
    type OperandValue = OperandValue;

    /// The operands of the assembly syntax in its order, then the state the instruction reads besides them.
    fn list_operand_types(&self, instruction: Instruction) -> impl Iterator<Item = OperandType> {
        use OperandType::{
            AccessPlacement, ConditionFlags, DataPagePermission, RegisterNumber, RegisterValue,
            SignedImmediate, UnsignedImmediate,
        };
        const PAGES: [OperandType; 2] = [
            DataPagePermission(DataPage::Lower),
            DataPagePermission(DataPage::Upper),
        ];
        let operand_types: &[OperandType] = match instruction {
            Instruction::AddShiftedRegister64 | Instruction::AddsShiftedRegister64 => &[
                RegisterNumber(RD),
                RegisterNumber(RN),
                RegisterNumber(RM),
                UnsignedImmediate(SHIFT),
                UnsignedImmediate(SHIFT_IMM6),
                RegisterValue(RN),
                RegisterValue(RM),
            ],
            Instruction::Csel64 => &[
                RegisterNumber(RD),
                RegisterNumber(RN),
                RegisterNumber(RM),
                UnsignedImmediate(COND),
                ConditionFlags,
            ],
            Instruction::LdrImmediateUnsignedOffset64
            | Instruction::StrImmediateUnsignedOffset64 => &[
                RegisterNumber(RT),
                RegisterNumber(RN),
                UnsignedImmediate(IMM12),
                AccessPlacement,
                PAGES[0],
                PAGES[1],
            ],
            Instruction::StpSignedOffset64 => &[
                RegisterNumber(RT),
                RegisterNumber(RT2),
                RegisterNumber(RN),
                SignedImmediate(IMM7),
                AccessPlacement,
                PAGES[0],
                PAGES[1],
            ],
            Instruction::LdrImmediatePostIndex64 => &[
                RegisterNumber(RT),
                RegisterNumber(RN),
                SignedImmediate(IMM9),
                AccessPlacement,
                PAGES[0],
                PAGES[1],
            ],
            Instruction::Brk => &[UnsignedImmediate(IMM16)],
            Instruction::Rdvl => &[RegisterNumber(RD), SignedImmediate(RDVL_IMM6)],
            Instruction::MrsCtrEl0 => &[RegisterNumber(RT)],
        };
        operand_types.iter().copied()
    }

    fn generate_critical_values(
        &self,
        operand_type: OperandType,
    ) -> impl Iterator<Item = OperandValue> {
        let values: Vec<OperandValue> = match operand_type {
            // Two ordinary registers, so that one can appear twice, and the zero register or stack pointer.
            OperandType::RegisterNumber(_) => [0, 1, GENERAL_PURPOSE_REGISTERS as u32]
                .map(OperandValue::RegisterNumber)
                .to_vec(),
            OperandType::RegisterValue(_) => [0, 1, i64::MIN as u64, u64::MAX]
                .map(OperandValue::Bits)
                .to_vec(),
            OperandType::UnsignedImmediate(field) | OperandType::SignedImmediate(field)
                if field.width <= EXHAUSTIVE_FIELD_BITS =>
            {
                (0..1 << field.width).map(OperandValue::Bits).collect()
            }
            OperandType::UnsignedImmediate(field) => {
                [0, 1, u64::from(field.to_mask() >> field.lowest_bit)]
                    .map(OperandValue::Bits)
                    .to_vec()
            }
            OperandType::SignedImmediate(field) => {
                let all_ones = u64::from(field.to_mask() >> field.lowest_bit);
                let sign_bit = 1 << (field.width - 1);
                [0, 1, all_ones, sign_bit - 1, sign_bit]
                    .map(OperandValue::Bits)
                    .to_vec()
            }
            OperandType::ConditionFlags => ConditionFlags::all()
                .iter()
                .powerset()
                .map(|subset| OperandValue::ConditionFlags(subset.into_iter().collect()))
                .collect(),
            OperandType::AccessPlacement => [
                AccessPlacement::LowerPageStart,
                AccessPlacement::EndsAtBoundary,
                AccessPlacement::StraddlesBoundary,
                AccessPlacement::UpperPageStart,
            ]
            .map(OperandValue::AccessPlacement)
            .to_vec(),
            OperandType::DataPagePermission(_) => {
                let mapped = |data_access| PagePermission::Mapped {
                    data_access,
                    instruction_fetch: InstructionFetch::Forbidden,
                };
                [
                    PagePermission::Unmapped,
                    mapped(DataAccess::Forbidden),
                    mapped(DataAccess::ReadOnly),
                    mapped(DataAccess::ReadWrite),
                ]
                .map(OperandValue::PagePermission)
                .to_vec()
            }
        };
        values.into_iter()
    }

    /// Every register no operand names holds the complement of its number, so that none holds zero.
    ///
    /// A register named twice holds the value of its last register value operand; the zero register holds none.
    ///
    /// # Panics
    ///
    /// When the operands are not one value of each operand type the instruction lists, in its order.
    fn build_state(
        &self,
        instruction: Instruction,
        operands: &[OperandValue],
    ) -> ExecutionState<'static, Arm64> {
        let typed: Vec<(OperandType, OperandValue)> = self
            .list_operand_types(instruction)
            .zip(operands.iter().copied())
            .collect();
        assert_eq!(
            typed.len(),
            operands.len(),
            "one value for each operand of {instruction:?}"
        );
        let mut word = instruction.to_fixed_bits().value;
        let mut processor = Processor {
            general_purpose_registers: array::from_fn(|number| !(number as u64)),
            program_counter: CODE_ADDRESS,
            ..ZERO_PROCESSOR
        };
        let mut placement = None;
        let mut lower = PagePermission::Unmapped;
        let mut upper = PagePermission::Unmapped;
        let mut register_values = Vec::new();
        for (operand_type, operand) in typed {
            match operand_type {
                OperandType::RegisterNumber(field) => {
                    let OperandValue::RegisterNumber(number) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    word |= field.to_bits(number);
                }
                OperandType::UnsignedImmediate(field) | OperandType::SignedImmediate(field) => {
                    let OperandValue::Bits(bits) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    word |= field.to_bits(bits as u32);
                }
                OperandType::RegisterValue(field) => {
                    let OperandValue::Bits(value) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    register_values.push((field, value));
                }
                OperandType::ConditionFlags => {
                    let OperandValue::ConditionFlags(flags) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    processor.condition_flags = flags;
                }
                OperandType::AccessPlacement => {
                    let OperandValue::AccessPlacement(chosen) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    placement = Some(chosen);
                }
                OperandType::DataPagePermission(page) => {
                    let OperandValue::PagePermission(permission) = operand else {
                        reject_mismatch(operand_type, operand)
                    };
                    match page {
                        DataPage::Lower => lower = permission,
                        DataPage::Upper => upper = permission,
                    }
                }
            }
        }
        // Register values go in once every register field is encoded.
        for (field, value) in register_values {
            if let Some(register) = processor
                .general_purpose_registers
                .get_mut(field.read(word) as usize)
            {
                *register = value;
            }
        }
        let readable_and_executable = PagePermission::Mapped {
            data_access: DataAccess::ReadOnly,
            instruction_fetch: InstructionFetch::Allowed,
        };
        let mut memory_mutations = vec![
            MemoryDelta::PermissionAssignment {
                addresses: CODE_ADDRESS..CODE_ADDRESS + PAGE_BYTES,
                permission: readable_and_executable,
            },
            MemoryDelta::ValueAssignment {
                start: CODE_ADDRESS,
                bytes: Cow::Owned(word.to_le_bytes().to_vec()),
            },
        ];
        if let (Some(placement), Some(addressing)) = (placement, to_addressing(instruction, word)) {
            let address = to_access_address(placement, addressing.access_bytes);
            let base = address.wrapping_sub(addressing.offset);
            match processor
                .general_purpose_registers
                .get_mut(RN.read(word) as usize)
            {
                Some(register) => *register = base,
                None => processor.stack_pointer = base,
            }
            let data = (LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES)
                .map(|address| !(address as u8));
            memory_mutations.extend([
                MemoryDelta::ValueAssignment {
                    start: LOWER_DATA_ADDRESS,
                    bytes: Cow::Owned(data.collect()),
                },
                MemoryDelta::PermissionAssignment {
                    addresses: LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS,
                    permission: lower,
                },
                MemoryDelta::PermissionAssignment {
                    addresses: UPPER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES,
                    permission: upper,
                },
            ]);
        }
        ExecutionState {
            processor,
            memory_mutations,
        }
    }
}

fn reject_mismatch(operand_type: OperandType, operand: OperandValue) -> ! {
    panic!("{operand:?} is no value of {operand_type:?}")
}

/// How an instruction that reads or writes memory finds its first byte.
struct Addressing {
    access_bytes: u64,
    /// What it adds to its base register.
    offset: u64,
}

fn to_addressing(instruction: Instruction, word: u32) -> Option<Addressing> {
    match instruction {
        Instruction::LdrImmediateUnsignedOffset64 | Instruction::StrImmediateUnsignedOffset64 => {
            Some(Addressing {
                access_bytes: REGISTER_BYTES,
                offset: u64::from(IMM12.read(word)) * REGISTER_BYTES,
            })
        }
        Instruction::StpSignedOffset64 => Some(Addressing {
            access_bytes: 2 * REGISTER_BYTES,
            offset: IMM7.read_signed(word).wrapping_mul(REGISTER_BYTES as i64) as u64,
        }),
        Instruction::LdrImmediatePostIndex64 => Some(Addressing {
            access_bytes: REGISTER_BYTES,
            offset: 0,
        }),
        Instruction::AddShiftedRegister64
        | Instruction::AddsShiftedRegister64
        | Instruction::Csel64
        | Instruction::Brk
        | Instruction::Rdvl
        | Instruction::MrsCtrEl0 => None,
    }
}

fn to_access_address(placement: AccessPlacement, access_bytes: u64) -> u64 {
    match placement {
        AccessPlacement::LowerPageStart => LOWER_DATA_ADDRESS,
        AccessPlacement::EndsAtBoundary => UPPER_DATA_ADDRESS - access_bytes,
        AccessPlacement::StraddlesBoundary => UPPER_DATA_ADDRESS - access_bytes / 2,
        AccessPlacement::UpperPageStart => UPPER_DATA_ADDRESS,
    }
}
