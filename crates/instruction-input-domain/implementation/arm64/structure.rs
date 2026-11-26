//! The inputs of the Advanced SIMD structure loads and stores, whose list of registers starts at Rt and wraps at V31.

use alloc::vec;
use alloc::vec::Vec;

use wcontainers_processor_architecture_arm64::StructureInstruction;
use wcontainers_processor_architecture_arm64::encoding::{Field, RM, RN, RT};
use wcontainers_processor_architecture_arm64::operand::{RegisterThirtyOne, RegisterWidth};

use crate::{DataPage, OperandType};

/// Q: the whole register, or its low 64 bits.
const Q: Field = Field::new(30, 1);

/// The register list, the arrangement or lane, the base, a register post-index's register and value, each stored
/// register's value, then where the access lies and what the pages hold.
pub(crate) fn to_structure_operand_types(structure: StructureInstruction) -> Vec<OperandType> {
    let mut operand_types = vec![
        OperandType::VectorListNumber(RT),
        OperandType::UnsignedImmediate(Q),
    ];
    operand_types.extend(
        structure
            .to_index_field()
            .map(OperandType::UnsignedImmediate),
    );
    operand_types.push(OperandType::RegisterNumber(RN));
    if structure.has_register_offset() {
        operand_types.extend([
            OperandType::RegisterNumber(RM),
            OperandType::RegisterValue {
                field: RM,
                width: RegisterWidth::Bits64,
                thirty_one: RegisterThirtyOne::ZeroRegister,
            },
        ]);
    }
    if structure.is_store() {
        operand_types.extend((0..structure.to_register_count()).map(|place| {
            OperandType::ListedVectorValue {
                field: RT,
                place,
                bytes: size_of::<u128>() as u32,
            }
        }));
    }
    operand_types.extend([
        OperandType::AccessPlacement,
        OperandType::AddressTag,
        OperandType::DataPagePermission(DataPage::Lower),
        OperandType::DataPagePermission(DataPage::Upper),
    ]);
    if structure.is_store() {
        operand_types.push(OperandType::MemoryFill);
    }
    operand_types
}
