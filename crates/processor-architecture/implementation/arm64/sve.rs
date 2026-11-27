//! The SVE instructions of predicates, integers and element moves at a 128-bit vector length, element by element as
//! Arm's pseudocode loops over them (for example [ADD (vectors, predicated)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/ADD--vectors--predicated---Add-vectors--predicated--)).
//!
//! A Z register is its vector register, and a predicate holds one bit for each byte of it: an element is active where
//! the lowest bit of its bits is set.

use core::convert::Infallible;

use crate::encoding::Field;
use crate::operand::{RegisterThirtyOne, Signedness};
use crate::sve_count::simulate_count;
use crate::sve_element::simulate_element;
use crate::sve_move::simulate_move;
use crate::sve_predicate::simulate_predicate;
use crate::{ConditionFlags, Exception, Instruction, Processor};

pub use crate::sve_count::{CountOperation, Counted, Step, Stepped};
pub use crate::sve_element::{
    BinaryForm, BinaryOperation, ElementOperation, ImmediateShift, Pairwise, ShiftDirection,
    TernaryLogic, UnaryOperation,
};
pub use crate::sve_move::{ElementTarget, MoveOperation, Reduction};
pub use crate::sve_predicate::{
    BreakKind, CompareWith, Comparison, PredicateLogic, PredicateOperation, PredicatePermutation,
    WhileCondition,
};

/// Bytes of a Z register.
pub const VECTOR_BYTES: u32 = u128::BITS / u8::BITS;

/// The element size field of most encodings: a byte shifted left by it.
pub(crate) const SIZE: Field = Field::new(22, 2);
pub(crate) const RD5: Field = Field::new(0, 5);
pub(crate) const RN5: Field = Field::new(5, 5);
pub(crate) const RM5: Field = Field::new(16, 5);
pub(crate) const PD4: Field = Field::new(0, 4);
pub(crate) const PN4: Field = Field::new(5, 4);
pub(crate) const PM4: Field = Field::new(16, 4);
pub(crate) const PG4: Field = Field::new(10, 4);
pub(crate) const PG3: Field = Field::new(10, 3);

/// The values a general-purpose register operand takes in a state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GeneralValues {
    /// Ordinary values of a register of 64 bits.
    Plain,
    /// Values on both sides of an element count from each other and from the ends of the signed and unsigned ranges
    /// of 32 and 64 bits, which a WHILE compares.
    Bound,
    /// Values a short way inside each end of the signed and unsigned ranges of 32 and 64 bits, where an increment or
    /// decrement saturates.
    Saturating,
}

/// The values a Z register operand takes in a state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorValues {
    /// The register is only written.
    Written,
    Data,
    /// Lanes of shift amounts around each element size, besides the data values.
    ShiftAmounts,
}

/// The values a predicate operand takes in a state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PredicateValues {
    /// The predicate is only written.
    Written,
    /// The predicate that chooses the elements an operation on vectors acts on.
    Governing,
    /// A predicate whose first, last and every active element a predicate operation reads.
    Operand,
}

/// One operand of an SVE instruction as its encoding holds it, in the order of its syntax.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalableOperand {
    ElementSize(Field),
    /// A field whose every value selects a form of the operation of its own: merging, a register's width or a shift.
    Selector(Field),
    /// A Z register; `pair` names the next register too.
    Vector {
        field: Field,
        values: VectorValues,
        pair: bool,
    },
    Predicate {
        field: Field,
        values: PredicateValues,
    },
    /// A general-purpose register, which the instruction reads where it has values.
    General {
        field: Field,
        thirty_one: RegisterThirtyOne,
        values: Option<GeneralValues>,
    },
    UnsignedImmediate(Field),
    SignedImmediate(Field),
    /// An unsigned immediate split over fields, the most significant first.
    SplitImmediate(&'static [Field]),
    /// The [pattern](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-sve#impl-aarch64.DecodePredCount.2) of a count, each value its own case.
    Pattern(Field),
    /// N, immr and imms of a 64-bit bitmask immediate.
    Bitmask(Field),
    /// NZCV, which the instruction reads.
    ConditionFlags,
    /// NZCV, which the instruction writes without reading.
    WrittenConditionFlags,
    /// FFR, which the instruction reads.
    FirstFault,
    /// FFR, which the instruction writes without reading.
    WrittenFirstFault,
}

/// What an SVE instruction computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum ScalableOperation {
    Predicate(PredicateOperation),
    Element(ElementOperation),
    Count(CountOperation),
    Move(MoveOperation),
}

/// An SVE instruction of this module: what it computes and the operands its encoding holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScalableInstruction {
    pub operation: ScalableOperation,
    pub operands: &'static [ScalableOperand],
}

/// The model of each SVE instruction of this module, none for every other instruction.
pub fn to_scalable_instruction(
    instruction: Instruction<Infallible>,
) -> Option<ScalableInstruction> {
    crate::sve_predicate::to_predicate_instruction(instruction)
        .or_else(|| crate::sve_element::to_element_instruction(instruction))
        .or_else(|| crate::sve_count::to_count_instruction(instruction))
        .or_else(|| crate::sve_move::to_move_instruction(instruction))
}

/// Applies the instruction encoded in the word; changes nothing when it returns an exception.
pub(crate) fn simulate_scalable<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: ScalableOperation,
) -> Result<(), Exception> {
    match operation {
        ScalableOperation::Predicate(operation) => simulate_predicate(processor, word, operation),
        ScalableOperation::Element(operation) => simulate_element(processor, word, operation),
        ScalableOperation::Count(operation) => simulate_count(processor, word, operation),
        ScalableOperation::Move(operation) => simulate_move(processor, word, operation),
    }
}

/// Bits of an element of the size field's value.
pub(crate) const fn to_element_bits(size: u32) -> u32 {
    u8::BITS << size
}

/// Elements of the bits in a Z register.
pub(crate) const fn count_elements(bits: u32) -> u32 {
    u128::BITS / bits
}

/// Ones in the lowest `bits` bits.
pub(crate) const fn to_mask(bits: u32) -> u128 {
    u128::MAX >> (u128::BITS - bits)
}

pub(crate) const fn read_element(vector: u128, index: u32, bits: u32) -> u128 {
    (vector >> (index * bits)) & to_mask(bits)
}

pub(crate) fn write_element(vector: &mut u128, index: u32, bits: u32, value: u128) {
    let mask = to_mask(bits) << (index * bits);
    *vector = (*vector & !mask) | ((value << (index * bits)) & mask);
}

/// The element's bits as a two's complement number, or as an unsigned one.
pub(crate) const fn to_integer(value: u128, bits: u32, signedness: Signedness) -> i128 {
    match signedness {
        Signedness::Unsigned => (value & to_mask(bits)) as i128,
        Signedness::Signed => ((value << (u128::BITS - bits)) as i128) >> (u128::BITS - bits),
    }
}

/// The smallest and largest integer of an element of the bits.
pub(crate) const fn to_range(bits: u32, signedness: Signedness) -> (i128, i128) {
    match signedness {
        Signedness::Unsigned => (0, to_mask(bits) as i128),
        Signedness::Signed => (-(1 << (bits - 1)), (1 << (bits - 1)) - 1),
    }
}

/// The integer clamped to the range of an element, as its bits.
pub(crate) fn saturate(value: i128, bits: u32, signedness: Signedness) -> u128 {
    let (low, high) = to_range(bits, signedness);
    (value.clamp(low, high) as u128) & to_mask(bits)
}

/// Bytes of a predicate's bits that one element of the bits takes.
const fn to_predicate_stride(bits: u32) -> u32 {
    bits / u8::BITS
}

/// Whether the element is active in the predicate: the lowest of its bits is set.
pub(crate) const fn is_active(predicate: u16, index: u32, bits: u32) -> bool {
    predicate >> (index * to_predicate_stride(bits)) & 1 != 0
}

/// The bits of one element of a predicate, as many as its element has bytes.
pub(crate) const fn read_predicate_element(predicate: u16, index: u32, bits: u32) -> u16 {
    let stride = to_predicate_stride(bits);
    (predicate >> (index * stride)) & ((1 << stride) - 1) as u16
}

pub(crate) fn write_predicate_element(predicate: &mut u16, index: u32, bits: u32, value: u16) {
    let stride = to_predicate_stride(bits);
    let mask = (((1_u32 << stride) - 1) as u16) << (index * stride);
    *predicate = (*predicate & !mask) | ((value << (index * stride)) & mask);
}

/// Sets the element's lowest bit to the truth and clears the rest of its bits.
pub(crate) fn write_predicate_truth(predicate: &mut u16, index: u32, bits: u32, truth: bool) {
    write_predicate_element(predicate, index, bits, u16::from(truth));
}

/// The flags of [PredTest](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-sve#impl-aarch64.PredTest.3): N for the first active element, Z for none, C for not the last, V clear.
pub(crate) fn test_predicate(mask: u16, result: u16, bits: u32) -> ConditionFlags {
    let mut active = (0..count_elements(bits)).filter(|index| is_active(mask, *index, bits));
    let first_active = active.next();
    let last_active = active.next_back().or(first_active);
    let first = first_active.is_some_and(|index| is_active(result, index, bits));
    let last = last_active.is_some_and(|index| is_active(result, index, bits));
    let none = !(0..count_elements(bits))
        .any(|index| is_active(mask, index, bits) && is_active(result, index, bits));
    let mut flags = ConditionFlags::empty();
    flags.set(ConditionFlags::NEGATIVE, first);
    flags.set(ConditionFlags::ZERO, none);
    flags.set(ConditionFlags::CARRY, !last != crate::planted::is(1002));
    flags
}

/// The elements a [pattern](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-sve#impl-aarch64.DecodePredCount.2) counts of those of the bits in a Z register.
pub(crate) fn decode_count(pattern: u32, bits: u32) -> u32 {
    let elements = count_elements(bits);
    let fixed = |count: u32| if elements >= count { count } else { 0 };
    match pattern {
        0b00000 => 1 << (u32::BITS - 1 - elements.leading_zeros()),
        0b00001..=0b01000 => fixed(pattern),
        0b01001..=0b01101 => fixed(16 << (pattern - 0b01001)),
        0b11101 => elements - elements % 4,
        0b11110 if crate::planted::is(1044) => elements - elements % 4,
        0b11110 => elements - elements % 3,
        0b11111 => elements,
        _ => 0,
    }
}

/// The Z register an encoding field names, and the one after it.
pub(crate) const fn to_next_register(number: u32) -> usize {
    ((number + 1) % crate::VECTOR_REGISTERS as u32) as usize
}
