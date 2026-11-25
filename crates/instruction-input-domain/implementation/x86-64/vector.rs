//! The inputs of the legacy SSE integer forms, derived from each form's lane width: one skeleton for every form, so
//! a new form of a modeled operation needs no line here.
//!
//! A form's operands are its two XMM registers and how its bytes lie (or, with a memory operand, the memory
//! operand), the destination's value, the source's value, in a register or in memory, and the flags it does not
//! read.
//!
//! A vector's critical values are rotations of its lane's critical values across its lanes, so that each pair of
//! lane values meets in some lane of some pair of vectors, and the lanes beside it hold other values (the
//! destination's rotations a lane count apart, which still meet every source rotation in some lane); then each end
//! of the lane's signed and unsigned range in every lane at once, so that each end lies in every lane position and
//! beside itself, as two words of 0x8000 make PMADDWD's pair sum wrap.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded, MemoryOperand, OpCodeOperandKind};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::float::{Conversion, Precision};
use wcontainers_processor_architecture_x86_64::vector::{Lane, LaneOperation, VectorForm, join, narrows};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{CodeLayout, ImmediateClass, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::{CODE_ADDRESS, FlagSet, OperandType, OperandValue, Role, Splits, VECTOR};

/// The bytes a legacy SSE memory operand reaches.
pub const VECTOR_BYTES: u64 = size_of::<u128>() as u64;
/// A byte pattern whose nibbles and halves differ, for lanes whose halves must not be swapped.
const PATTERN: u64 = 0x5AA5_C33C_0FF0_9669;
/// What the source's upper quadword holds for a shift by its lower one, which the shift ignores.
const COUNT_UPPER_QUADWORD: u64 = 0xA5A5_A5A5_A5A5_A5A5;

/// The single-precision critical values, in the order [`VectorValues::Floats`] lists them.
const SINGLE_VALUES: [u64; 16] = [
    0x0000_0000, 0x8000_0000, 0x0000_0001, 0x007F_FFFF, 0x0080_0000, 0x8080_0000, 0x3F80_0000, 0x3F80_0001,
    0xBFC0_0000, 0x3380_0000, 0x33C0_0000, 0x7F7F_FFFF, 0x7F80_0000, 0xFF80_0000, 0x7FC1_2345, 0xFF81_2345,
];
/// The double-precision critical values, in the same order.
const DOUBLE_VALUES: [u64; 16] = [
    0x0000_0000_0000_0000, 0x8000_0000_0000_0000, 0x0000_0000_0000_0001, 0x000F_FFFF_FFFF_FFFF,
    0x0010_0000_0000_0000, 0x8010_0000_0000_0000, 0x3FF0_0000_0000_0000, 0x3FF0_0000_0000_0001,
    0xBFF8_0000_0000_0000, 0x3CA0_0000_0000_0000, 0x3CA8_0000_0000_0000, 0x7FEF_FFFF_FFFF_FFFF,
    0x7FF0_0000_0000_0000, 0xFFF0_0000_0000_0000, 0x7FF8_0000_0001_2345, 0xFFF0_0000_0001_2345,
];

/// What a vector operand's critical values are made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorValues {
    /// The lane's own edges: zero, one, two, the width less one and the width (a shift's edges), the signed
    /// extremes, all ones, and a pattern. `every_rotation` as for [`Self::Floats`].
    Lanes { lane: Lane, every_rotation: bool },
    /// Those of a lane narrowed to half its width and saturated: each edge of the narrower lane's signed and
    /// unsigned range and the values one past it, and the wider lane's extremes.
    Narrowing { lane: Lane, every_rotation: bool },
    /// A shift count in the low quadword: zero, one, the width less one, the width, one with a bit above 32 set,
    /// and all ones; the high quadword a pattern.
    Count(Lane),
    /// Floating-point lanes: each sign of zero, the smallest and largest subnormal, the smallest normal of each
    /// sign, one and the next number above it, a negative number, the values half and three quarters of one's unit
    /// in the last place, the largest normal, each infinity, a quiet NaN and a signaling NaN, each with a payload.
    /// `every_rotation` is false for the destination of a packed form, whose rotations step by the lanes in a
    /// vector, so that every pair of lane values still meets in some lane.
    Floats { precision: Precision, every_rotation: bool },
    /// A conversion's source: its format's critical values in every rotation, doublewords' as [`Self::Lanes`]
    /// gives them, then the conversion's own edges, each in every lane.
    Conversion { conversion: Conversion, precision: Precision },
}

/// The operand types of a vector form, in the order the build reads them.
pub fn list_operand_types(form: &VectorForm) -> Vec<OperandType> {
    let lane = form.operation.to_lane();
    let values = |every_rotation| match narrows(form.operation) {
        true => VectorValues::Narrowing { lane, every_rotation },
        false => VectorValues::Lanes { lane, every_rotation },
    };
    let source = if form.operation.shifts() { VectorValues::Count(lane) } else { values(true) };
    // A shift by an immediate names its destination alone; a control immediate follows the source.
    let kinds = form.code.op_code().op_kinds();
    let reads_source = kinds.get(1).is_some_and(|kind| *kind != OpCodeOperandKind::imm8);
    let place = match form.place {
        OperandPlace::Register => OperandType::Encoding(if reads_source { 2 } else { 1 }),
        _ => OperandType::Memory {
            splits: Splits::Allowed,
            offset: false,
        },
    };
    let destination = form.operation.reads_destination().then_some(OperandType::VectorValue(Role::Destination, values(false)));
    let counts = form.operation.shifts()
        || matches!(form.operation, LaneOperation::AlignRight | LaneOperation::ShiftBytesLeft | LaneOperation::ShiftBytesRight);
    let class = if counts { ImmediateClass::Count } else { ImmediateClass::Selector };
    let immediate = kinds.contains(&OpCodeOperandKind::imm8).then_some(OperandType::Immediate(class));
    [place]
        .into_iter()
        .chain(destination)
        .chain(reads_source.then_some(OperandType::VectorValue(Role::Source, source)))
        .chain(immediate)
        .chain([OperandType::Flags(FlagSet::Unread)])
        .collect()
}

/// The critical values of a vector operand.
pub fn generate_vectors(values: VectorValues) -> Vec<OperandValue> {
    let to_mask = |bits: u32| if bits >= u64::BITS { u64::MAX } else { (1 << bits) - 1 };
    let lane_values: Vec<u64> = match values {
        VectorValues::Lanes { lane, .. } => {
            let bits = lane.to_bits();
            let signed_maximum = to_mask(bits - 1);
            vec![0, 1, 2, u64::from(bits) - 1, u64::from(bits), signed_maximum, signed_maximum + 1, to_mask(bits), PATTERN & to_mask(bits)]
        }
        VectorValues::Narrowing { lane, .. } => {
            let bits = lane.to_bits();
            let narrow = bits / 2;
            let narrow_maximum = to_mask(narrow - 1);
            let narrow_minimum = !narrow_maximum & to_mask(bits);
            vec![
                0,
                1,
                narrow_maximum,
                narrow_maximum + 1,
                narrow_minimum,
                narrow_minimum - 1,
                to_mask(narrow),
                to_mask(narrow) + 1,
                to_mask(bits),
                to_mask(bits - 1),
                to_mask(bits - 1) + 1,
            ]
        }
        VectorValues::Floats { precision, every_rotation } => {
            return rotate_floats(precision, every_rotation).into_iter().map(OperandValue::Vector).collect();
        }
        VectorValues::Conversion { conversion, precision } => {
            let source = match conversion {
                Conversion::SingleToDouble => Precision::Single,
                Conversion::DoubleToSingle => Precision::Double,
                _ => precision,
            };
            let (lane, rotations) = match conversion {
                Conversion::IntegerToFloat => (
                    Lane::Doubleword,
                    generate_vectors(VectorValues::Lanes { lane: Lane::Doubleword, every_rotation: true }),
                ),
                _ => (to_float_lane(source), rotate_floats(source, true).into_iter().map(OperandValue::Vector).collect()),
            };
            let edges = crate::float::generate_conversion_edges(conversion, source);
            return rotations.into_iter().chain(edges.into_iter().map(|edge| OperandValue::Vector(fill_lanes(lane, edge)))).collect();
        }
        VectorValues::Count(lane) => {
            let bits = u64::from(lane.to_bits());
            let counts = [0, 1, bits - 1, bits, 1 << u32::BITS | 1, u64::MAX];
            return counts
                .map(|count| OperandValue::Vector(join(Lane::Quadword, [count, COUNT_UPPER_QUADWORD].into_iter())))
                .to_vec();
        }
    };
    let (lane, every_rotation) = match values {
        VectorValues::Lanes { lane, every_rotation } | VectorValues::Narrowing { lane, every_rotation } => (lane, every_rotation),
        VectorValues::Count(_) | VectorValues::Floats { .. } | VectorValues::Conversion { .. } => unreachable!("returned above"),
    };
    let count = lane_values.len();
    let lanes = (u128::BITS / lane.to_bits()) as usize;
    let step = if every_rotation { 1 } else { lanes };
    let bits = lane.to_bits();
    let range_ends = [0, to_mask(bits - 1), to_mask(bits - 1) + 1, to_mask(bits)];
    (0..count)
        .step_by(step)
        .map(|rotation| join(lane, (0..lanes).map(|index| lane_values[(index + rotation) % count])))
        .chain(range_ends.map(|end| fill_lanes(lane, end)))
        .map(OperandValue::Vector)
        .collect()
}

/// The value in every lane.
pub fn fill_lanes(lane: Lane, value: u64) -> u128 {
    join(lane, (0..u128::BITS / lane.to_bits()).map(|_| value))
}

/// The rotations of the floating-point critical values across the lanes: every one, or those a lane count apart,
/// which still meet every value with every other in some lane against a source's every rotation.
pub fn rotate_floats(precision: Precision, every_rotation: bool) -> Vec<u128> {
    let values: &[u64] = match precision {
        Precision::Single => &SINGLE_VALUES,
        Precision::Double => &DOUBLE_VALUES,
    };
    let lane = to_float_lane(precision);
    let lanes = (u128::BITS / lane.to_bits()) as usize;
    let step = if every_rotation { 1 } else { lanes };
    (0..values.len())
        .step_by(step)
        .map(|rotation| join(lane, (0..lanes).map(|index| values[(index + rotation) % values.len()])))
        .collect()
}

/// The lane a floating-point number of the precision fills.
pub const fn to_float_lane(precision: Precision) -> Lane {
    match precision {
        Precision::Single => Lane::Doubleword,
        Precision::Double => Lane::Quadword,
    }
}

/// Builds the state of a vector form from one value of each of its operand types.
pub fn build_state(form: &VectorForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    build_xmm_state(form.code, form.memory_bytes(), operands)
}

/// Builds the state of a form of two XMM operands, the second a register or `memory_bytes` of memory, from one
/// value of each of its operand types; a form that does not read its destination leaves it as every register is.
pub fn build_xmm_state(
    code: Code,
    memory_bytes: u64,
    operands: &[(OperandType, OperandValue)],
) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        ..ZERO_PROCESSOR
    };
    let (mut registers, mut access, mut layout) = ([0, 0], None, CodeLayout::PageStart);
    let (mut destination, mut source, mut reads_source, mut immediate) = (None, 0, false, None);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => {
                registers = encoding.registers;
                layout = encoding.layout;
            }
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => {
                registers[0] = chosen.register;
                layout = chosen.layout;
                access = Some(chosen);
            }
            (OperandType::VectorValue(Role::Destination, _), OperandValue::Vector(value)) => destination = Some(value),
            (OperandType::FloatOperands(_), OperandValue::Vectors { destination: first, source: second }) => {
                destination = Some(first);
                (source, reads_source) = (second, true);
            }
            (OperandType::VectorValue(_, _), OperandValue::Vector(value)) => (source, reads_source) = (value, true),
            (OperandType::Immediate(_), OperandValue::Immediate(value)) => immediate = Some(value as u32),
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (OperandType::Mxcsr(_), OperandValue::Mxcsr(mxcsr)) => processor.mxcsr = mxcsr,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    // A register named twice holds the source's value, the last written.
    if let Some(destination) = destination {
        processor.vector_registers[registers[0]] = destination;
    }
    let mut memory_mutations = Vec::new();
    let memory_operand: Option<MemoryOperand> = access.map(|access| {
        let (operand, address) = place_memory_operand(access, memory_bytes, &mut processor, &mut memory_mutations);
        let bytes: Vec<u8> = (0..memory_bytes)
            .map(|offset| address.wrapping_add(offset))
            .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
            .zip(source.to_le_bytes())
            .map(|(_, byte)| byte)
            .collect();
        if !bytes.is_empty() {
            memory_mutations.push(MemoryDelta::ValueAssignment {
                start: address,
                bytes: Cow::Owned(bytes),
            });
        }
        operand
    });
    if memory_operand.is_none() && reads_source {
        processor.vector_registers[registers[1]] = source;
    }
    let destination_register = VECTOR[registers[0]];
    let encoded = match (memory_operand, immediate) {
        (Some(operand), Some(immediate)) => Encoded::with3(code, destination_register, operand, immediate),
        (Some(operand), None) => Encoded::with2(code, destination_register, operand),
        (None, Some(immediate)) if reads_source => Encoded::with3(code, destination_register, VECTOR[registers[1]], immediate),
        (None, Some(immediate)) => Encoded::with2(code, destination_register, immediate),
        (None, None) => Encoded::with2(code, destination_register, VECTOR[registers[1]]),
    }
    .unwrap_or_else(|error| panic!("{code:?} takes its operands: {error}"));
    let (code_page, start, bytes) = lay_out(layout, &encode_at_code_address(&encoded), to_padding(memory_operand));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    };
    let mut all = vec![
        MemoryDelta::PermissionAssignment {
            addresses: code_page..code_page + PAGE_BYTES,
            permission: executable,
        },
        MemoryDelta::ValueAssignment {
            start,
            bytes: Cow::Owned(bytes),
        },
    ];
    all.append(&mut memory_mutations);
    ExecutionState {
        processor,
        memory_mutations: all,
    }
}
