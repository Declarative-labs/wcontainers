//! The inputs of the ARM64 instructions.
//!
//! Every state holds the instruction on a page of its own and, for an instruction that reads or writes memory,
//! two data pages right above it whose permissions are operands. Each data byte holds the complement of its
//! address's low byte, so that no loaded value equals an address near it.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_arm64::encoding::{
    COND, Field, IMM7, IMM9, IMM12, IMM16, RD, RDVL_IMM6, RM, RN, RT, RT2, SHIFT, SHIFT_IMM6,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, DataAccess, GENERAL_PURPOSE_REGISTERS, Instruction, InstructionFetch,
    PagePermission, Processor, VECTOR_REGISTERS,
};

use crate::InstructionInputDomain;

/// The 4 KiB translation granule the executors use.
const PAGE_BYTES: u64 = 1 << 12;
/// The first page above the page at zero, which stays unmapped as on Linux.
const CODE_ADDRESS: u64 = PAGE_BYTES;
const LOWER_DATA_ADDRESS: u64 = CODE_ADDRESS + PAGE_BYTES;
const UPPER_DATA_ADDRESS: u64 = LOWER_DATA_ADDRESS + PAGE_BYTES;
const REGISTER_BYTES: u64 = size_of::<u64>() as u64;
/// A field this narrow gets every value it can hold.
const EXHAUSTIVE_FIELD_BITS: u32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arm64Operand {
    /// The register an encoding field names.
    Register(Field),
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
pub enum Arm64OperandValue {
    Register(u32),
    /// A register's value, or an immediate's bits.
    Integer(u64),
    ConditionFlags(ConditionFlags),
    AccessPlacement(AccessPlacement),
    PagePermission(PagePermission),
}

impl InstructionInputDomain for Arm64 {
    type Operand = Arm64Operand;
    type OperandValue = Arm64OperandValue;

    fn list_operands(instruction: Instruction) -> impl Iterator<Item = Arm64Operand> {
        use Arm64Operand::{
            AccessPlacement, ConditionFlags, DataPagePermission, Register, RegisterValue,
            SignedImmediate, UnsignedImmediate,
        };
        const PAGES: [Arm64Operand; 2] = [
            DataPagePermission(DataPage::Lower),
            DataPagePermission(DataPage::Upper),
        ];
        let operands: &[Arm64Operand] = match instruction {
            Instruction::AddShiftedRegister64 | Instruction::AddsShiftedRegister64 => &[
                Register(RD),
                Register(RN),
                Register(RM),
                UnsignedImmediate(SHIFT),
                UnsignedImmediate(SHIFT_IMM6),
                RegisterValue(RN),
                RegisterValue(RM),
            ],
            Instruction::Csel64 => &[
                Register(RD),
                Register(RN),
                Register(RM),
                UnsignedImmediate(COND),
                ConditionFlags,
            ],
            Instruction::LdrImmediateUnsignedOffset64
            | Instruction::StrImmediateUnsignedOffset64 => &[
                Register(RT),
                Register(RN),
                UnsignedImmediate(IMM12),
                AccessPlacement,
                PAGES[0],
                PAGES[1],
            ],
            Instruction::StpSignedOffset64 => &[
                Register(RT),
                Register(RT2),
                Register(RN),
                SignedImmediate(IMM7),
                AccessPlacement,
                PAGES[0],
                PAGES[1],
            ],
            Instruction::LdrImmediatePostIndex64 => &[
                Register(RT),
                Register(RN),
                SignedImmediate(IMM9),
                AccessPlacement,
                PAGES[0],
                PAGES[1],
            ],
            Instruction::Brk => &[UnsignedImmediate(IMM16)],
            Instruction::Rdvl => &[Register(RD), SignedImmediate(RDVL_IMM6)],
            Instruction::MrsCtrEl0 => &[Register(RT)],
        };
        operands.iter().copied()
    }

    fn generate_values(operand: Arm64Operand) -> impl Iterator<Item = Arm64OperandValue> {
        let values: Vec<Arm64OperandValue> = match operand {
            // Two ordinary registers, so that one can appear twice, and the zero register or stack pointer.
            Arm64Operand::Register(_) => [0, 1, GENERAL_PURPOSE_REGISTERS as u32]
                .map(Arm64OperandValue::Register)
                .to_vec(),
            Arm64Operand::RegisterValue(_) => [0, 1, i64::MIN as u64, u64::MAX]
                .map(Arm64OperandValue::Integer)
                .to_vec(),
            Arm64Operand::UnsignedImmediate(field) | Arm64Operand::SignedImmediate(field)
                if field.width <= EXHAUSTIVE_FIELD_BITS =>
            {
                (0..1 << field.width)
                    .map(Arm64OperandValue::Integer)
                    .collect()
            }
            Arm64Operand::UnsignedImmediate(field) => {
                [0, 1, u64::from(field.to_mask() >> field.lowest_bit)]
                    .map(Arm64OperandValue::Integer)
                    .to_vec()
            }
            Arm64Operand::SignedImmediate(field) => {
                let all_ones = u64::from(field.to_mask() >> field.lowest_bit);
                let sign_bit = 1 << (field.width - 1);
                [0, 1, all_ones, sign_bit - 1, sign_bit]
                    .map(Arm64OperandValue::Integer)
                    .to_vec()
            }
            Arm64Operand::ConditionFlags => {
                let flags: Vec<ConditionFlags> = ConditionFlags::all().iter().collect();
                (0..1_u32 << flags.len())
                    .map(|subset| {
                        let chosen = flags
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| subset >> index & 1 == 1);
                        chosen.fold(ConditionFlags::empty(), |union, (_, flag)| union | *flag)
                    })
                    .map(Arm64OperandValue::ConditionFlags)
                    .collect()
            }
            Arm64Operand::AccessPlacement => [
                AccessPlacement::LowerPageStart,
                AccessPlacement::EndsAtBoundary,
                AccessPlacement::StraddlesBoundary,
                AccessPlacement::UpperPageStart,
            ]
            .map(Arm64OperandValue::AccessPlacement)
            .to_vec(),
            Arm64Operand::DataPagePermission(_) => {
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
                .map(Arm64OperandValue::PagePermission)
                .to_vec()
            }
        };
        values.into_iter()
    }

    /// Every register not named by an operand holds the complement of its number, so that none holds zero.
    fn to_state(
        instruction: Instruction,
        values: &[Arm64OperandValue],
    ) -> ExecutionState<'static, Arm64> {
        let operands: Vec<(Arm64Operand, Arm64OperandValue)> = Self::list_operands(instruction)
            .zip(values.iter().copied())
            .collect();
        assert_eq!(
            operands.len(),
            values.len(),
            "one value for each operand of {instruction:?}"
        );
        let mut word = instruction.to_fixed_bits().value;
        let mut processor = Processor {
            general_purpose_registers: array::from_fn(|number| !(number as u64)),
            stack_pointer: 0,
            program_counter: CODE_ADDRESS,
            condition_flags: ConditionFlags::empty(),
            vector_registers: [0; VECTOR_REGISTERS],
            floating_point_control: 0,
            floating_point_status: 0,
            software_thread_id: 0,
            exception: None,
        };
        let mut placement = None;
        let mut lower = PagePermission::Unmapped;
        let mut upper = PagePermission::Unmapped;
        for (operand, value) in &operands {
            match (*operand, *value) {
                (Arm64Operand::Register(field), Arm64OperandValue::Register(number)) => {
                    word |= field.to_bits(number)
                }
                (
                    Arm64Operand::UnsignedImmediate(field) | Arm64Operand::SignedImmediate(field),
                    Arm64OperandValue::Integer(bits),
                ) => word |= field.to_bits(bits as u32),
                (Arm64Operand::RegisterValue(_), Arm64OperandValue::Integer(_)) => {}
                (Arm64Operand::ConditionFlags, Arm64OperandValue::ConditionFlags(flags)) => {
                    processor.condition_flags = flags
                }
                (Arm64Operand::AccessPlacement, Arm64OperandValue::AccessPlacement(chosen)) => {
                    placement = Some(chosen)
                }
                (
                    Arm64Operand::DataPagePermission(page),
                    Arm64OperandValue::PagePermission(permission),
                ) => match page {
                    DataPage::Lower => lower = permission,
                    DataPage::Upper => upper = permission,
                },
                (operand, value) => panic!("{value:?} is no value of {operand:?}"),
            }
        }
        // Register values go in once every register field is encoded.
        for (operand, value) in &operands {
            if let (Arm64Operand::RegisterValue(field), Arm64OperandValue::Integer(integer)) =
                (*operand, *value)
                && let Some(register) = processor
                    .general_purpose_registers
                    .get_mut(field.read(word) as usize)
            {
                *register = integer;
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
