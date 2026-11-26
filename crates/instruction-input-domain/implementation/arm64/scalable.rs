//! The inputs of the SVE instructions of `wcontainers_processor_architecture_arm64::scalable`, at a 128-bit vector
//! length: their operands by the fields the architecture names, and the critical values of Z and P registers.

use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_processor_architecture_arm64::Instruction;
use wcontainers_processor_architecture_arm64::encoding::{Field, RN};
use wcontainers_processor_architecture_arm64::floating_point::{FLUSH_HALF_TO_ZERO, Precision};
use wcontainers_processor_architecture_arm64::load_store::scalable_access::{
    AccessKind, ElementSize, Offset, PT, RM, ScalableAccess, to_scalable_access,
};
use wcontainers_processor_architecture_arm64::operand::{RegisterThirtyOne, RegisterWidth};
use wcontainers_processor_architecture_arm64::scalable::{
    PG, ScalableOperand, ScalableOperation, VECTOR_BITS, ZD, list_scalable_operands,
    to_element_bits, to_scalable_operation,
};
use wcontainers_processor_architecture_arm64::scalable_floating_point::{
    FloatingPointForm, LaneKind, Lanes, VectorRole, to_conversion_lanes,
};

use crate::{
    ControlReads, DataPage, OperandType, OperandValue, ScalableValues, low_ones,
    to_floating_point_bits, to_vector_values,
};

/// The element sizes of SVE in bytes, smallest first.
const ELEMENT_BYTES: [u32; 4] = [1, 2, 4, 8];
/// The bits of a predicate at a 128-bit vector length.
const PREDICATE_BITS: u32 = u16::BITS;
/// The values of a structure's register: zero, all ones, and each byte its own place with its top bit clear and set.
const STRUCTURE_VALUES: usize = 4;
/// A predicate whose every bit is set: every element of every size active.
pub(crate) const ALL_ACTIVE: u16 = u16::MAX;

/// The operand types of a modeled SVE instruction in the order of its syntax, each register followed by its value
/// where the instruction reads it; none for every other instruction.
pub(crate) fn list_scalable_operand_types(
    instruction: Instruction<Infallible>,
) -> Option<Vec<OperandType>> {
    if let Some(access) = to_scalable_access(instruction) {
        return Some(list_access_operand_types(
            access,
            instruction.to_fixed_bits().mask,
        ));
    }
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
                OperandType::ScalablePredicateValue(number),
            ]),
            ScalableOperand::FloatingPointSource { number, role } => types.extend([
                OperandType::ScalableVectorNumber {
                    field: number,
                    registers: 1,
                },
                OperandType::ScalableVectorValue {
                    field: number,
                    offset: 0,
                    values: ScalableValues::FloatingPoint { lanes: None, role },
                },
            ]),
            ScalableOperand::ConvertedSource(number) => {
                let Some(ScalableOperation::FloatingPoint(FloatingPointForm::Convert(conversion))) =
                    to_scalable_operation(instruction)
                else {
                    unreachable!("only a conversion has a converted source");
                };
                types.extend([
                    OperandType::ScalableVectorNumber {
                        field: number,
                        registers: 1,
                    },
                    OperandType::ScalableVectorValue {
                        field: number,
                        offset: 0,
                        values: ScalableValues::Converted(conversion),
                    },
                ]);
            }
            ScalableOperand::PredicateDestination(number) => {
                types.push(OperandType::PredicateNumber(number));
            }
            ScalableOperand::ImmediateChoice(field) => {
                types.push(OperandType::UnsignedImmediate(field))
            }
            ScalableOperand::FloatingPointControl { conversion } => types.extend([
                OperandType::FloatingPointControl(if conversion {
                    ControlReads::Conversion
                } else {
                    ControlReads::Half
                }),
                OperandType::StatusFlags,
            ]),
        }
    }
    Some(types)
}

/// The operand types of an SVE contiguous access: its element size where a field gives it, the registers it moves,
/// with their values for a store, its predicate, base and offset, then where the access lies, and for a store what
/// the data pages hold.
fn list_access_operand_types(access: ScalableAccess, fixed: u32) -> Vec<OperandType> {
    let mut types = Vec::new();
    // A bit of the size field the encoding fixes takes no value of its own, as ST1W's size is 1x.
    if let ElementSize::Field(field) = access.element {
        let free = field.to_mask() & !fixed;
        types.push(OperandType::UnsignedImmediate(Field::new(
            free.trailing_zeros(),
            free.count_ones(),
        )));
    }
    let stores = access.kind == AccessKind::Store;
    if access.predicate_register {
        types.push(OperandType::PredicateNumber(PT));
        if stores {
            types.push(OperandType::ScalablePredicateValue(PT));
        }
    } else if access.kind != AccessKind::Prefetch {
        types.push(OperandType::ScalableVectorNumber {
            field: ZD,
            registers: access.registers,
        });
        if stores {
            // Each register of a structure stores bytes of its own, so a few values each tell them apart.
            let values = if access.registers > 1 {
                ScalableValues::Structure
            } else {
                ScalableValues::Integer
            };
            types.extend(
                (0..access.registers).map(|offset| OperandType::ScalableVectorValue {
                    field: ZD,
                    offset,
                    values,
                }),
            );
        }
    }
    if access.predicated {
        types.extend([
            OperandType::PredicateNumber(PG),
            OperandType::ScalablePredicateValue(PG),
        ]);
    }
    types.push(OperandType::RegisterNumber(RN));
    match access.offset {
        Offset::Register => types.extend([
            OperandType::RegisterNumber(RM),
            OperandType::RegisterValue {
                field: RM,
                width: RegisterWidth::Bits64,
                thirty_one: RegisterThirtyOne::ZeroRegister,
            },
        ]),
        Offset::Immediate {
            fields: [field],
            signed: true,
            ..
        } => types.push(OperandType::SignedImmediate(*field)),
        Offset::Immediate {
            fields: [field], ..
        } => types.push(OperandType::UnsignedImmediate(*field)),
        Offset::Immediate { fields, .. } => types.push(OperandType::SplitImmediate(fields)),
    }
    // A store of two or four registers can straddle the boundary inside a vector only at a placement of its own.
    let placement = if stores && matches!(access.registers, 2 | 4) {
        OperandType::ScalableStructurePlacement
    } else {
        OperandType::AccessPlacement
    };
    types.extend([
        placement,
        OperandType::AddressTag,
        OperandType::DataPagePermission(DataPage::Lower),
        OperandType::DataPagePermission(DataPage::Upper),
    ]);
    if stores {
        types.push(OperandType::MemoryFill);
    }
    types
}

/// The values a whole Z register takes: those of a vector register, or of the lanes its values name.
pub(crate) fn to_scalable_vector_values(values: ScalableValues) -> Vec<OperandValue> {
    match values {
        ScalableValues::FloatingPoint { lanes: None, role } => {
            return [Precision::Half, Precision::Single, Precision::Double]
                .into_iter()
                .flat_map(|precision| {
                    let lanes = Lanes {
                        kind: LaneKind::FloatingPoint(precision),
                        container_bits: precision.to_bits(),
                    };
                    pack(&to_floating_point_bits(precision), lanes, role)
                })
                .collect();
        }
        ScalableValues::FloatingPoint {
            lanes: Some(lanes),
            role,
        } => return pack(&to_lane_values(lanes.kind, None), lanes, role),
        ScalableValues::Converted(conversion) => {
            let (lanes, target) = to_conversion_lanes(conversion);
            return pack(
                &to_lane_values(lanes.kind, Some(target.kind)),
                lanes,
                VectorRole::Packed,
            );
        }
        ScalableValues::Structure => {
            return to_vector_values(VECTOR_BITS / u8::BITS)
                .into_iter()
                .take(STRUCTURE_VALUES)
                .collect();
        }
        ScalableValues::Integer => {}
    }
    to_vector_values(VECTOR_BITS / u8::BITS)
}

/// The values one lane of the kind takes: a floating-point value's, with the bounds of an integer it converts to; an
/// integer's, with the integers just past the significand of the precision it converts to.
fn to_lane_values(kind: LaneKind, target: Option<LaneKind>) -> Vec<u64> {
    match (kind, target) {
        (LaneKind::FloatingPoint(precision), Some(LaneKind::Integer { bits })) => {
            to_converted_floating_point_bits(precision, bits)
        }
        (LaneKind::FloatingPoint(precision), _) => to_floating_point_bits(precision),
        (LaneKind::Integer { bits }, target) => {
            let mask = low_ones(bits);
            let mut values = [
                0,
                1,
                mask,
                1 << (bits - 1),
                mask >> 1,
                (1 << (u8::BITS - 1)) & mask,
            ]
            .to_vec();
            if let Some(LaneKind::FloatingPoint(precision)) = target {
                let significand_bits = precision.to_fraction_bits() + 1;
                for power in [significand_bits, significand_bits + 1] {
                    if power + 1 >= bits {
                        continue;
                    }
                    for odd in [1, 3] {
                        let integer = (1_u64 << power) + odd;
                        values.extend([integer, integer.wrapping_neg() & mask]);
                    }
                }
            }
            to_distinct(values)
        }
    }
}

/// The values in their order, each once.
fn to_distinct(values: Vec<u64>) -> Vec<u64> {
    let mut distinct = Vec::new();
    for value in values {
        if !distinct.contains(&value) {
            distinct.push(value);
        }
    }
    distinct
}

/// The values of a precision, and both signs of two to the integer's bits less one and of two to its bits, each with
/// the values beside it, and two to the bits less one, less a half: the bounds of the integer's range and the
/// roundings at them.
fn to_converted_floating_point_bits(precision: Precision, bits: u32) -> Vec<u64> {
    let fraction_bits = precision.to_fraction_bits();
    let exponent_ones = (1_u64 << precision.to_exponent_bits()) - 1;
    let bias = exponent_ones >> 1;
    let sign = 1 << (precision.to_bits() - 1);
    let mut values = to_floating_point_bits(precision);
    for power in [bits - 1, bits] {
        let exponent = bias + u64::from(power);
        if exponent >= exponent_ones {
            continue;
        }
        let bound = exponent << fraction_bits;
        for magnitude in [bound - 1, bound, bound + 1] {
            values.extend([magnitude, sign | magnitude]);
        }
        if power == bits - 1 && fraction_bits >= power {
            let fraction = (1 << fraction_bits) - (1 << (fraction_bits - power));
            values.push(((exponent - 1) << fraction_bits) | fraction);
        }
    }
    to_distinct(values)
}

/// The values in the lanes: packed, each value once in turn over as few registers as hold them; or rotated, each value
/// in the first lane of a register of its own with the values after it in the next lanes. A container wider than its
/// value holds zeros above it in its even lanes and ones in its odd ones, which no instruction reads.
fn pack(values: &[u64], lanes: Lanes, role: VectorRole) -> Vec<OperandValue> {
    let count = VECTOR_BITS / lanes.container_bits;
    let value_bits = lanes.kind.to_bits();
    let registers = match role {
        VectorRole::Packed => values.len().div_ceil(count as usize),
        VectorRole::Rotated => values.len(),
    };
    (0..registers)
        .map(|register| {
            let bits = (0..count).fold(0_u128, |vector, lane| {
                let place = match role {
                    VectorRole::Packed => register * count as usize + lane as usize,
                    VectorRole::Rotated => register + lane as usize,
                };
                let value = u128::from(values[place % values.len()]);
                let above = if lane % 2 == 1 && value_bits < lanes.container_bits {
                    (u128::MAX >> (u128::BITS - lanes.container_bits))
                        & !(u128::MAX >> (u128::BITS - value_bits))
                } else {
                    0
                };
                vector | ((value | above) << (lane * lanes.container_bits))
            });
            OperandValue::ScalableVector { bits, lanes }
        })
        .collect()
}

/// Whether the value is one of the lanes the word gives its operand: a floating-point source of an instruction whose
/// size field sizes its elements takes only the values of that size, and its FPCR sets FZ16 only for half precision,
/// as a scalar instruction of single or double precision reads FPCR.
pub(crate) fn is_value_of_word(
    instruction: Instruction<Infallible>,
    operand_type: OperandType,
    value: OperandValue,
    word: u32,
) -> bool {
    match (operand_type, value) {
        (
            OperandType::ScalableVectorValue {
                values: ScalableValues::FloatingPoint { lanes: None, .. },
                ..
            },
            OperandValue::ScalableVector { lanes, .. },
        ) => to_scalable_operation(instruction)
            .and_then(|operation| to_element_bits(operation, word))
            .is_some_and(|bits| bits == lanes.container_bits),
        (OperandType::FloatingPointControl(ControlReads::Half), OperandValue::Bits(control)) => {
            match to_scalable_operation(instruction) {
                Some(
                    operation @ ScalableOperation::FloatingPoint(
                        FloatingPointForm::Unpredicated(_)
                        | FloatingPointForm::Predicated(_)
                        | FloatingPointForm::Unary(_)
                        | FloatingPointForm::Immediate { .. }
                        | FloatingPointForm::Compare { .. }
                        | FloatingPointForm::MultiplyAdd { .. },
                    ),
                ) => {
                    control & FLUSH_HALF_TO_ZERO == 0
                        || to_element_bits(operation, word) == Some(u16::BITS)
                }
                _ => true,
            }
        }
        _ => true,
    }
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
