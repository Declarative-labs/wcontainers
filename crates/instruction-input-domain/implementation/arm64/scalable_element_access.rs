//! The inputs of the SVE accesses of `wcontainers_processor_architecture_arm64::load_store::scalable_element_access`:
//! their operands, and the offsets and addresses a vector holds.
//!
//! A vector of offsets or addresses holds, element by element, where each element lies from the first byte the
//! placement gives: one after the other, the other way round, all at one address, the odd ones on the next page, all
//! below it, and one after the other one byte on, so that each element is misaligned and one can straddle a page's end. A vector base holds those
//! displacements plus the placed address, which the state's builder adds; a vector of 32-bit addresses also holds them
//! with bit 31 set, which only an extension with the sign would carry into the bits above.

use alloc::vec::Vec;

use wcontainers_processor_architecture_arm64::encoding::RN;
use wcontainers_processor_architecture_arm64::load_store::scalable_access::AccessKind;
use wcontainers_processor_architecture_arm64::load_store::scalable_element_access::{
    Addressing, ElementAccess, IMMEDIATE4, IMMEDIATE5, PREFETCH_OPERATION, RM,
};
use wcontainers_processor_architecture_arm64::operand::{RegisterThirtyOne, RegisterWidth};
use wcontainers_processor_architecture_arm64::scalable::{PG, VECTOR_BITS, ZD, ZM, ZN};
use wcontainers_processor_architecture_arm64::{Processor, VECTOR_REGISTERS};

use crate::{DataPage, OperandType, OperandValue, ScalableValues};

/// The operand types of an access of elements: its register, with its value for a store, or a prefetch's operation;
/// its predicate; its base and offset; FFR before a first-fault or non-fault load; then where the access lies, and for
/// a store what the data pages hold.
pub(crate) fn list_operand_types(access: ElementAccess) -> Vec<OperandType> {
    let mut types = Vec::new();
    let stores = access.kind == AccessKind::Store;
    if access.kind == AccessKind::Prefetch {
        types.push(OperandType::UnsignedImmediate(PREFETCH_OPERATION));
    } else {
        types.push(OperandType::ScalableVectorNumber {
            field: ZD,
            registers: 1,
        });
        if stores {
            types.push(OperandType::ScalableVectorValue {
                field: ZD,
                offset: 0,
                values: ScalableValues::Integer,
            });
        }
    }
    types.extend([
        OperandType::PredicateNumber(PG),
        OperandType::ScalablePredicateValue(PG),
    ]);
    let scalar_offset = [
        OperandType::RegisterNumber(RM),
        OperandType::RegisterValue {
            field: RM,
            width: RegisterWidth::Bits64,
            thirty_one: RegisterThirtyOne::ZeroRegister,
        },
    ];
    let addresses = [
        OperandType::ScalableVectorNumber {
            field: ZN,
            registers: 1,
        },
        OperandType::ElementAddresses {
            field: ZN,
            element_bits: access.element_bits,
            memory_bytes: access.memory_bytes,
        },
    ];
    match access.addressing {
        Addressing::ScalarPlusScalar => {
            types.push(OperandType::RegisterNumber(RN));
            types.extend(scalar_offset);
        }
        Addressing::ScalarPlusImmediate => {
            types.extend([
                OperandType::RegisterNumber(RN),
                OperandType::SignedImmediate(IMMEDIATE4),
            ]);
        }
        Addressing::ScalarPlusVector {
            offset_bits,
            extension,
            scaled,
        } => {
            types.push(OperandType::RegisterNumber(RN));
            types.extend(extension.map(OperandType::UnsignedImmediate));
            types.extend([
                OperandType::ScalableVectorNumber {
                    field: ZM,
                    registers: 1,
                },
                OperandType::ElementOffsets {
                    field: ZM,
                    element_bits: access.element_bits,
                    memory_bytes: access.memory_bytes,
                    offset_bits,
                    scaled,
                },
            ]);
        }
        Addressing::VectorPlusImmediate => {
            types.extend(addresses);
            types.push(OperandType::UnsignedImmediate(IMMEDIATE5));
        }
        Addressing::VectorPlusScalar => {
            types.extend(addresses);
            types.extend(scalar_offset);
        }
    }
    if access.suppresses_faults() {
        types.push(OperandType::FirstFaultValue);
    }
    // An element of more than one byte can lie across the boundary.
    if access.memory_bytes > 1 && access.kind != AccessKind::Prefetch {
        types.push(OperandType::ElementPlacement);
    } else {
        types.push(OperandType::AccessPlacement);
    }
    // Only an address of 64 bits has a top byte.
    if !(access.has_vector_base() && access.element_bits < u64::BITS) {
        types.push(OperandType::AddressTag);
    }
    types.extend([
        OperandType::DataPagePermission(DataPage::Lower),
        OperandType::DataPagePermission(DataPage::Upper),
    ]);
    if stores {
        types.push(OperandType::MemoryFill);
    }
    types
}

/// A page's bytes, which the odd elements of one pattern lie further on, on the page after.
const PAGE_BYTES: i64 = 1 << 12;
/// Bit 31, the sign of a 32-bit element.
const WORD_SIGN_BIT: i64 = 1 << 31;

/// Each element's displacement from the first byte, in bytes: one after the other, the other way round, all at one
/// address, the odd elements a page further on, and one after the other one byte on where an element has more than one
/// byte.
fn to_displacements(element_bits: u32, memory_bytes: u32) -> Vec<Vec<i64>> {
    let elements = i64::from(VECTOR_BITS / element_bits);
    let size = i64::from(memory_bytes);
    let mut patterns = [
        (0..elements).map(|element| element * size).collect(),
        (0..elements)
            .map(|element| (elements - 1 - element) * size)
            .collect(),
        (0..elements).map(|_| 0).collect(),
        (0..elements)
            .map(|element| element * size + (element % 2) * PAGE_BYTES)
            .collect(),
    ]
    .to_vec();
    if memory_bytes > 1 {
        patterns.push((0..elements).map(|element| element * size + 1).collect());
    }
    patterns
}

/// The elements in a vector, each truncated to its bits.
fn pack(values: impl IntoIterator<Item = u64>, element_bits: u32) -> u128 {
    let mask = u128::MAX >> (u128::BITS - element_bits);
    values
        .into_iter()
        .enumerate()
        .fold(0, |vector, (element, value)| {
            vector | ((u128::from(value) & mask) << (element as u32 * element_bits))
        })
}

/// The values of a vector of offsets: each displacement in units of the memory's element where scaled, those below the
/// first byte too, and where an offset is narrower than its element, ones above it, which the access leaves out.
pub(crate) fn to_offset_values(
    element_bits: u32,
    memory_bytes: u32,
    offset_bits: u32,
    scaled: bool,
) -> Vec<OperandValue> {
    let unit = if scaled { i64::from(memory_bytes) } else { 1 };
    let elements = i64::from(VECTOR_BITS / element_bits);
    let size = i64::from(memory_bytes);
    let offset_mask = u64::MAX >> (u64::BITS - offset_bits);
    let mut patterns: Vec<Vec<i64>> = to_displacements(element_bits, memory_bytes)
        .into_iter()
        .filter(|pattern| pattern.iter().all(|bytes| bytes % unit == 0))
        .collect();
    patterns.push((0..elements).map(|element| -(element + 1) * size).collect());
    let mut values: Vec<u128> = patterns
        .iter()
        .map(|pattern| {
            pack(
                pattern
                    .iter()
                    .map(|bytes| (bytes / unit) as u64 & offset_mask),
                element_bits,
            )
        })
        .collect();
    if offset_bits < element_bits {
        values.push(pack(
            (0..elements).map(|element| ((element * size / unit) as u64) | !offset_mask),
            element_bits,
        ));
    }
    values.into_iter().map(OperandValue::Vector).collect()
}

/// The values of a vector base before the placed address is added: each displacement, and for elements of 32 bits,
/// one after the other with bit 31 set, which a base extended with its sign would carry into the bits above.
pub(crate) fn to_address_values(element_bits: u32, memory_bytes: u32) -> Vec<OperandValue> {
    let mut patterns = to_displacements(element_bits, memory_bytes);
    if element_bits == u32::BITS {
        let elements = i64::from(VECTOR_BITS / element_bits);
        patterns.push(
            (0..elements)
                .map(|element| element * i64::from(memory_bytes) + WORD_SIGN_BIT)
                .collect(),
        );
    }
    patterns
        .into_iter()
        .map(|pattern| {
            OperandValue::Vector(pack(
                pattern.into_iter().map(|bytes| bytes as u64),
                element_bits,
            ))
        })
        .collect()
}

/// Adds the base, the address of the first byte less the access's offset, to every element of the vector base.
pub(crate) fn place_vector_base(
    access: ElementAccess,
    word: u32,
    base: u64,
    processor: &mut Processor<()>,
) {
    let element_bits = access.element_bits;
    let mask = u128::MAX >> (u128::BITS - element_bits);
    let register = &mut processor.vector_registers[ZN.read(word) as usize % VECTOR_REGISTERS];
    let placed = (0..VECTOR_BITS / element_bits).fold(0_u128, |vector, element| {
        let shift = element * element_bits;
        let held = ((*register >> shift) & mask) as u64;
        vector | ((u128::from(held.wrapping_add(base)) & mask) << shift)
    });
    *register = placed;
}
