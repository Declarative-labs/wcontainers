//! The inputs of the SVE instructions of `wcontainers_processor_architecture_arm64::scalable`, at a 128-bit vector
//! length: their operands by the fields the architecture names, and the critical values of Z and P registers.

use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_processor_architecture_arm64::Instruction;
use wcontainers_processor_architecture_arm64::scalable::{
    ScalableOperand, VECTOR_BITS, list_scalable_operands,
};

use wcontainers_processor_architecture_arm64::encoding::Field;

use crate::{
    OperandType, OperandValue, ScalableValues, low_ones, to_vector_values, with_each_bit_alone,
};

/// The element sizes of SVE in bytes, smallest first.
const ELEMENT_BYTES: [u32; 4] = [1, 2, 4, 8];
/// The bits of a predicate at a 128-bit vector length.
const PREDICATE_BITS: u32 = u16::BITS;
/// A predicate whose every bit is set: every element of every size active.
pub(crate) const ALL_ACTIVE: u16 = u16::MAX;

/// The operand types of a modeled SVE instruction in the order of its syntax, each register followed by its value
/// where the instruction reads it; none for every other instruction.
pub(crate) fn list_scalable_operand_types(
    instruction: Instruction<Infallible>,
) -> Option<Vec<OperandType>> {
    let operands = list_scalable_operands(instruction)?;
    let mut types = Vec::new();
    for operand in operands {
        match *operand {
            ScalableOperand::ElementSize(field) => {
                types.push(OperandType::UnsignedImmediate(field))
            }
            ScalableOperand::VectorSource { number, registers } => {
                types.push(OperandType::ScalableVectorNumber {
                    field: number,
                    registers,
                });
                types.extend(
                    (0..registers).map(|offset| OperandType::ScalableVectorValue {
                        field: number,
                        offset,
                        values: ScalableValues::Integer,
                    }),
                );
            }
            ScalableOperand::TableIndices(number) => types.extend([
                OperandType::ScalableVectorNumber {
                    field: number,
                    registers: 1,
                },
                OperandType::ScalableVectorValue {
                    field: number,
                    offset: 0,
                    values: ScalableValues::TableIndices,
                },
            ]),
            ScalableOperand::VectorDestination(number) => {
                types.push(OperandType::ScalableVectorNumber {
                    field: number,
                    registers: 1,
                });
            }
            ScalableOperand::VectorSourceDestination(number) => types.extend([
                OperandType::ScalableVectorNumber {
                    field: number,
                    registers: 1,
                },
                OperandType::ScalableVectorValue {
                    field: number,
                    offset: 0,
                    values: ScalableValues::Integer,
                },
            ]),
            ScalableOperand::PredicateSource(number) => types.extend([
                OperandType::PredicateNumber(number),
                OperandType::PredicateValue(number),
            ]),
            ScalableOperand::EveryValue(fields) => types.push(OperandType::EveryValue(fields)),
            ScalableOperand::BytePosition(fields) => types.push(OperandType::BytePosition(fields)),
        }
    }
    Some(types)
}

/// The values a whole Z register takes: those of a vector register, and for indices into a table, at each element
/// size, each element its own index, the indices reversed, the last index and the first past one table alternating,
/// and the indices of a second table, and the last index of two tables and the first past them alternating.
pub(crate) fn to_scalable_vector_values(values: ScalableValues) -> Vec<OperandValue> {
    let mut chosen = to_vector_values(VECTOR_BITS / u8::BITS);
    if values == ScalableValues::TableIndices {
        for bytes in ELEMENT_BYTES {
            let esize = bytes * u8::BITS;
            let elements = VECTOR_BITS / esize;
            let fill = |lane: &dyn Fn(u32) -> u32| {
                (0..elements).fold(0_u128, |vector, index| {
                    vector | (u128::from(lane(index)) << (index * esize))
                })
            };
            let alternate = |even: u32, odd: u32| {
                move |index: u32| {
                    if index.is_multiple_of(2) { even } else { odd }
                }
            };
            for vector in [
                fill(&|index| index),
                fill(&|index| elements - 1 - index),
                fill(&alternate(elements - 1, elements)),
                fill(&|index| elements + index),
                fill(&alternate(2 * elements - 1, 2 * elements)),
            ] {
                let value = OperandValue::Vector(vector);
                if !chosen.contains(&value) {
                    chosen.push(value);
                }
            }
        }
    }
    chosen
}

/// Every byte position of a Z register and the first past it, the largest the fields hold, and each bit alone.
pub(crate) fn to_byte_positions(fields: &[Field]) -> Vec<OperandValue> {
    let width: u32 = fields.iter().map(|field| field.width).sum();
    with_each_bit_alone(
        width,
        (0..=u64::from(VECTOR_BITS / u8::BITS)).chain([low_ones(width)]),
    )
}

/// The values a predicate takes: none and every bit set; and at each element size every element active, only the
/// first, only the last, the even ones and the odd ones, and only the bits no element of the size reads; and a run of
/// elements inside the vector.
pub(crate) fn to_predicate_values() -> Vec<OperandValue> {
    let mut values: Vec<u16> = [0, ALL_ACTIVE, 1].to_vec();
    for bytes in ELEMENT_BYTES {
        let elements = PREDICATE_BITS / bytes;
        let mask = |chosen: &dyn Fn(u32) -> bool| {
            (0..elements)
                .filter(|index| chosen(*index))
                .fold(0_u16, |predicate, index| predicate | 1 << (index * bytes))
        };
        let every = mask(&|_| true);
        for predicate in [
            every,
            mask(&|index| index == elements - 1),
            mask(&|index| index.is_multiple_of(2)),
            mask(&|index| index % 2 == 1),
            !every,
        ] {
            if !values.contains(&predicate) {
                values.push(predicate);
            }
        }
    }
    let run = 0x0FF0;
    if !values.contains(&run) {
        values.push(run);
    }
    values
        .into_iter()
        .map(|predicate| OperandValue::Bits(u64::from(predicate)))
        .collect()
}
