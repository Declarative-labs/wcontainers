//! The inputs of LDG, the tag stores and STGP.

use alloc::vec::Vec;

use wcontainers_processor_architecture_arm64::Instruction;
use wcontainers_processor_architecture_arm64::memory_tagging::TagAccessInstruction;
use wcontainers_processor_architecture_arm64::operand::{
    Operand, RegisterThirtyOne, RegisterWidth,
};

use crate::{DataPage, OperandType};

/// The registers the list gives, each with its value (Rt's an address, whose tag a tag store takes), the base, the
/// offset, then where the access lies and what the pages hold.
pub(crate) fn to_tag_access_operand_types(access: TagAccessInstruction) -> Vec<OperandType> {
    let instruction: Instruction<core::convert::Infallible> = access.instruction();
    let Some(record) = instruction.to_record() else {
        panic!("{instruction:?} is listed")
    };
    let mut operand_types = Vec::new();
    for operand in record.operands {
        match *operand {
            Operand::GeneralPurposeRegister {
                number, thirty_one, ..
            } => {
                operand_types.push(OperandType::RegisterNumber(number));
                operand_types.push(if access.is_pair() {
                    OperandType::RegisterValue {
                        field: number,
                        width: RegisterWidth::Bits64,
                        thirty_one: RegisterThirtyOne::ZeroRegister,
                    }
                } else {
                    OperandType::Address {
                        field: number,
                        thirty_one,
                    }
                });
            }
            Operand::AddressBase { number, .. } => {
                operand_types.push(OperandType::RegisterNumber(number));
            }
            Operand::AddressOffset { field, .. } => {
                operand_types.push(OperandType::SignedImmediate(field));
            }
            _ => panic!("{instruction:?} has an operand no tag access has: {operand:?}"),
        }
    }
    operand_types.extend([
        OperandType::AccessPlacement,
        OperandType::AddressTag,
        OperandType::DataPagePermission(DataPage::Lower),
        OperandType::DataPagePermission(DataPage::Upper),
    ]);
    if access.writes_data() {
        operand_types.push(OperandType::MemoryFill);
    }
    operand_types
}
