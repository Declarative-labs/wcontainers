//! The inputs of the SVE instructions of the architecture's `sve_remaining` module: their operands by the fields it
//! names, whole registers as Advanced SIMD lane patterns, and operands of this module's own where a pattern depends
//! on fields the Advanced SIMD ones do not read: an indexed element, the wide source of a narrowing, floating-point lanes
//! sized by FLOGB's field, and an element size that leaves out a value another instruction's encoding holds.

use alloc::vec;
use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_processor_architecture_arm64::encoding::Field;
use wcontainers_processor_architecture_arm64::floating_point::{FLUSH_HALF_TO_ZERO, Precision};
use wcontainers_processor_architecture_arm64::sve_remaining::{
    IndexLayout, PG, RemainingOperation, SIZE, ZD, ZM, ZM_LOW, ZN, to_remaining_operation,
};
use wcontainers_processor_architecture_arm64::sve_remaining_floating_point::{
    ADD_ROTATION, COEFFICIENT, INDEXED_ROTATION, LOGARITHM_SIZE, MULTIPLY_ROTATION,
    RemainingFloatingPoint, SecondElement as FloatSecond, to_element_precision, to_index_layout,
    to_size_precision,
};
use wcontainers_processor_architecture_arm64::sve_remaining_integer::{
    self as integer, AddressOffset, IMM3, LongMultiply, MSZ, RemainingInteger, SZ, SameMultiply,
    SecondElement as IntegerSecond, TSZH, TSZL, read_tsize, to_shift_narrow_bits,
};
use wcontainers_processor_architecture_arm64::{Instruction, Processor};

use crate::advanced_simd::{LaneKind, LaneWidth, SimdOperand, SimdRole};
use crate::{ControlReads, OperandType, OperandValue, to_floating_point_bits};

/// The bits of a Z register at a 128-bit vector length.
const VECTOR_BITS: u32 = u128::BITS;
/// The patterns of a narrowing source: zero, all ones, bytes counted, then the bounds' lanes in turn.
const NARROW_PATTERNS: usize = 19;
/// The patterns of floating-point lanes: each critical value in every lane, then lanes of different values.
const FLOATING_POINT_PATTERNS: usize = 20;
/// The shift of a shift by an immediate: tszh, tszl and imm3, the most significant first.
const SHIFT_FIELDS: [Field; 3] = [TSZH, TSZL, IMM3];
/// The element size of an extract: tszh and tszl.
const EXTRACT_FIELDS: [Field; 2] = [TSZH, TSZL];
/// PMULLB and PMULLT's sizes: 01, 10 and 11, since 00 is the encoding of their 128-bit form.
const POLYNOMIAL_SIZES: u32 = 0b1110;
/// Every size of a size field.
const EVERY_SIZE: u32 = 0b1111;

/// An operand of this module's own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemainingOperand {
    /// A size field whose values are the sizes `sizes` holds, bit i for size i, 00 last: the generator reads which
    /// sources a state holds with every field at its first value, where a size of 00 would give floating-point lanes
    /// of a byte, which hold no value of their own.
    ElementSize { field: Field, sizes: u32 },
    /// The wide source of a narrowing shift or extract, at the element size tsize gives: zero, all ones, bytes counted,
    /// and lanes on both sides of each bound of the narrower element, shifted up by the shift where `shifted` and less
    /// half a step there, which a rounding shift brings up to the bound.
    NarrowSource { shifted: bool },
    /// Floating-point lanes of the precision a size field gives: each critical value in every lane, then lanes of
    /// different values.
    SizedFloatingPoint { field: Field, size: Field },
    /// The element an indexed form reads, `group` lanes of `bits` bits from the index's group, and the index: value k
    /// puts the kind's value k and those after it in the indexed group, every index in turn, and later values in the
    /// other lanes, so that each lane holds a value of its own.
    IndexedElement {
        layout: IndexLayout,
        bits: u32,
        group: u32,
        values: ElementValues,
    },
}

/// What an indexed element holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ElementValues {
    /// Zero, one, all ones, each end of the signed range, the sign bit of each byte and bytes counted.
    Integer,
    /// The critical values of the precision of the lane's width.
    FloatingPoint,
    /// The critical values of a single with its low half dropped.
    BrainFloat,
}

impl RemainingOperand {
    pub const fn to_role(self) -> SimdRole {
        match self {
            Self::ElementSize { .. } => SimdRole::Field,
            Self::NarrowSource { .. }
            | Self::SizedFloatingPoint { .. }
            | Self::IndexedElement { .. } => SimdRole::Data,
        }
    }
}

const fn to_precision(bits: u32) -> Precision {
    match bits {
        16 => Precision::Half,
        64 => Precision::Double,
        _ => Precision::Single,
    }
}

const fn to_mask(bits: u32) -> u128 {
    if bits >= u128::BITS {
        u128::MAX
    } else {
        (1 << bits) - 1
    }
}

/// A vector whose lane i of `bits` bits is `lane(i)`.
fn fill(bits: u32, lane: impl Fn(u32) -> u128) -> u128 {
    (0..VECTOR_BITS / bits).fold(0, |vector, index| {
        vector | ((lane(index) & to_mask(bits)) << (index * bits))
    })
}

/// Bytes counted from one.
fn counted() -> u128 {
    (0..VECTOR_BITS / u8::BITS).fold(0, |vector, place| {
        vector | (u128::from(place + 1) << (place * u8::BITS))
    })
}

/// The values a lane of an indexed element takes in turn.
fn to_element_values(values: ElementValues, bits: u32) -> Vec<u128> {
    match values {
        ElementValues::Integer => {
            let most_negative = 1_u128 << (bits - 1);
            let byte_signs = fill(u8::BITS, |_| 0x80) & to_mask(bits);
            vec![
                0,
                1,
                to_mask(bits),
                most_negative,
                most_negative - 1,
                byte_signs,
                counted() & to_mask(bits),
                2,
            ]
        }
        ElementValues::FloatingPoint => to_floating_point_bits(to_precision(bits))
            .into_iter()
            .map(u128::from)
            .collect(),
        ElementValues::BrainFloat => {
            let mut chosen = Vec::new();
            for value in to_floating_point_bits(Precision::Single) {
                let value = u128::from(value >> u16::BITS);
                if !chosen.contains(&value) {
                    chosen.push(value);
                }
            }
            chosen
        }
    }
}

fn to_index_count(layout: IndexLayout) -> u32 {
    1 << layout
        .to_index_fields()
        .iter()
        .map(|field| field.width)
        .sum::<u32>()
}

fn count_values(operand: RemainingOperand) -> usize {
    match operand {
        RemainingOperand::ElementSize { sizes, .. } => sizes.count_ones() as usize,
        RemainingOperand::NarrowSource { .. } => NARROW_PATTERNS,
        RemainingOperand::SizedFloatingPoint { .. } => FLOATING_POINT_PATTERNS,
        RemainingOperand::IndexedElement {
            layout,
            bits,
            values,
            ..
        } => to_element_values(values, bits)
            .len()
            .max(to_index_count(layout) as usize),
    }
}

/// The critical values of an operand of this module.
pub fn generate_critical_values(operand: RemainingOperand) -> Vec<OperandValue> {
    match operand {
        RemainingOperand::ElementSize { sizes, .. } => [1, 2, 3, 0_u64]
            .into_iter()
            .filter(|size| sizes >> size & 1 == 1)
            .map(OperandValue::Bits)
            .collect(),
        _ => (0..count_values(operand) as u64)
            .map(OperandValue::Bits)
            .collect(),
    }
}

/// The bits the operand puts in the word: a size, or an element's index.
pub fn encode(operand: RemainingOperand, value: OperandValue) -> u32 {
    let OperandValue::Bits(value) = value else {
        return 0;
    };
    match operand {
        RemainingOperand::ElementSize { field, .. } => field.to_bits(value as u32),
        RemainingOperand::IndexedElement { layout, .. } => {
            let index = value as u32 % to_index_count(layout);
            let mut remaining = index;
            let mut word = 0;
            for field in layout.to_index_fields().iter().rev() {
                word |= field.to_bits(remaining);
                remaining >>= field.width;
            }
            word
        }
        RemainingOperand::NarrowSource { .. } | RemainingOperand::SizedFloatingPoint { .. } => 0,
    }
}

/// The wide lanes of a narrowing source: the bounds of the narrower element, shifted up by the shift the word gives
/// where `shifted`, each also less half a step.
fn to_narrow_lanes(word: u32, shifted: bool) -> (u32, Vec<u128>) {
    let narrow = to_shift_narrow_bits(word)
        .unwrap_or(u8::BITS)
        .min(u32::BITS);
    let wide = 2 * narrow;
    let shift = if shifted {
        let immediate = (read_tsize(word) << IMM3.width) | IMM3.read(word);
        wide.saturating_sub(immediate).min(wide - 1)
    } else {
        0
    };
    let bounds: [i128; 8] = [
        (1 << (narrow - 1)) - 1,
        1 << (narrow - 1),
        -(1 << (narrow - 1)),
        -(1 << (narrow - 1)) - 1,
        (1 << narrow) - 1,
        1 << narrow,
        0,
        -1,
    ];
    let mut lanes = Vec::new();
    for bound in bounds {
        let value = bound << shift;
        lanes.push(value as u128 & to_mask(wide));
        let below = if shift == 0 {
            value
        } else {
            value - (1 << (shift - 1))
        };
        lanes.push(below as u128 & to_mask(wide));
    }
    (wide, lanes)
}

/// The register an operand of these kinds holds for value k, and where it goes.
fn to_register(operand: RemainingOperand, k: usize, word: u32) -> Option<(usize, u128)> {
    match operand {
        RemainingOperand::ElementSize { .. } => None,
        RemainingOperand::NarrowSource { shifted } => {
            let (wide, lanes) = to_narrow_lanes(word, shifted);
            let vector = match k {
                0 => 0,
                1 => u128::MAX,
                2 => counted(),
                _ => fill(wide, |lane| lanes[(k - 3 + lane as usize) % lanes.len()]),
            };
            Some((ZN.read(word) as usize, vector))
        }
        RemainingOperand::SizedFloatingPoint { field, size } => {
            let precision = to_size_precision(size.read(word)).unwrap_or(Precision::Half);
            let bits = precision.to_bits();
            let values = to_floating_point_bits(precision);
            let vector = if k < values.len() {
                fill(bits, |_| u128::from(values[k]))
            } else {
                fill(bits, |lane| {
                    u128::from(values[(k * 5 + lane as usize * 7) % values.len()])
                })
            };
            Some((field.read(word) as usize, vector))
        }
        RemainingOperand::IndexedElement {
            layout,
            bits,
            group,
            values,
        } => {
            let values = to_element_values(values, bits);
            let index = k as u32 % to_index_count(layout);
            let vector = fill(bits, |lane| {
                let place = if lane / group == index {
                    k + (lane % group) as usize
                } else {
                    k + group as usize + lane as usize
                };
                values[place % values.len()]
            });
            Some((layout.to_register_field().read(word) as usize, vector))
        }
    }
}

/// Puts the operand's value in the state, once the word holds every field.
pub fn apply<LevelSpecific>(
    operand: RemainingOperand,
    value: OperandValue,
    word: u32,
    processor: &mut Processor<LevelSpecific>,
) {
    let OperandValue::Bits(k) = value else {
        return;
    };
    if let Some((register, vector)) = to_register(operand, k as usize, word) {
        processor.vector_registers[register] = vector;
    }
}

/// Reads the operand's value back from a state, for the domain's own check: the first value that builds the same
/// register and puts the same bits in the word.
pub fn read_back(
    operand: RemainingOperand,
    word: u32,
    processor: &Processor<()>,
) -> Option<OperandValue> {
    if let RemainingOperand::ElementSize { field, .. } = operand {
        return Some(OperandValue::Bits(u64::from(field.read(word))));
    }
    let fields = match operand {
        RemainingOperand::IndexedElement { layout, .. } => layout
            .to_index_fields()
            .iter()
            .fold(0, |mask, field| mask | field.to_mask()),
        _ => 0,
    };
    let values = generate_critical_values(operand);
    let encoded = |value: &OperandValue| encode(operand, *value) == word & fields;
    values
        .iter()
        .find(|value| {
            let mut built = processor.clone();
            apply(operand, **value, word, &mut built);
            built.vector_registers == processor.vector_registers && encoded(value)
        })
        .or_else(|| values.iter().find(|value| encoded(value)))
        .copied()
}

/// The floating-point lanes of the operand's value and their precision, the indexed element's alone; none for every
/// other operand.
fn to_floating_point_lanes(
    operand: RemainingOperand,
    value: OperandValue,
    word: u32,
) -> Option<(Vec<u64>, Precision)> {
    let OperandValue::Bits(k) = value else {
        return None;
    };
    let (register, precision, lanes) = match operand {
        RemainingOperand::SizedFloatingPoint { size, .. } => {
            let precision = to_size_precision(size.read(word))?;
            let (_, register) = to_register(operand, k as usize, word)?;
            (register, precision, None)
        }
        RemainingOperand::IndexedElement {
            layout,
            bits,
            group,
            values: ElementValues::FloatingPoint,
        } => {
            let (_, register) = to_register(operand, k as usize, word)?;
            let index = k as u32 % to_index_count(layout);
            (register, to_precision(bits), Some((index * group, group)))
        }
        _ => return None,
    };
    let bits = precision.to_bits();
    let (first, count) = lanes.unwrap_or((0, VECTOR_BITS / bits));
    Some((
        (first..first + count)
            .map(|lane| ((register >> (lane * bits)) & to_mask(bits)) as u64)
            .collect(),
        precision,
    ))
}

/// Whether every lane the operand's value gives the instruction is a NaN or an infinity, so that no result is rounded.
pub fn is_nonfinite(operand: RemainingOperand, value: OperandValue, word: u32) -> bool {
    to_floating_point_lanes(operand, value, word).is_some_and(|(lanes, precision)| {
        let exponent_ones = (1 << precision.to_exponent_bits()) - 1;
        lanes
            .iter()
            .all(|lane| (lane >> precision.to_fraction_bits()) & exponent_ones == exponent_ones)
    })
}

/// Whether some lane of the operand's value is subnormal, so that a flush control can change it.
pub fn is_subnormal(operand: RemainingOperand, value: OperandValue, word: u32) -> bool {
    to_floating_point_lanes(operand, value, word).is_some_and(|(lanes, precision)| {
        let exponent_ones = (1 << precision.to_exponent_bits()) - 1;
        let fraction = (1 << precision.to_fraction_bits()) - 1;
        lanes.iter().any(|lane| {
            (lane >> precision.to_fraction_bits()) & exponent_ones == 0 && lane & fraction != 0
        })
    })
}

/// Whether the value is one the word gives its operand: an instruction whose size field sizes its floating-point
/// elements sets FZ16 only for half precision, as a scalar instruction of single or double precision reads FPCR.
pub fn is_value_of_word(
    instruction: Instruction<Infallible>,
    operand_type: OperandType,
    value: OperandValue,
    word: u32,
) -> bool {
    let (OperandType::FloatingPointControl(ControlReads::Half), OperandValue::Bits(control)) =
        (operand_type, value)
    else {
        return true;
    };
    match to_remaining_operation(instruction) {
        Some(RemainingOperation::FloatingPoint(operation)) if sizes_by_field(operation) => {
            control & FLUSH_HALF_TO_ZERO == 0
                || to_element_precision(operation, word) == Some(Precision::Half)
        }
        _ => true,
    }
}

/// Whether a size field gives the operation's precision.
const fn sizes_by_field(operation: RemainingFloatingPoint) -> bool {
    use RemainingFloatingPoint as R;
    matches!(
        operation,
        R::Pairwise(_)
            | R::ComplexMultiplyAdd { bits: None, .. }
            | R::ComplexAdd
            | R::Reduce(_)
            | R::OrderedAdd
            | R::Estimate { .. }
            | R::Logarithm
            | R::TrigonometricMultiply
            | R::TrigonometricMultiplyAdd
    )
}

fn lanes(field: Field, kind: LaneKind, width: LaneWidth) -> OperandType {
    OperandType::AdvancedSimd(SimdOperand::Lanes {
        field,
        offset: 0,
        kind,
        width,
    })
}

fn element(layout: IndexLayout, bits: u32, group: u32, values: ElementValues) -> OperandType {
    OperandType::Remaining(RemainingOperand::IndexedElement {
        layout,
        bits,
        group,
        values,
    })
}

/// The width of lanes of a fixed size, or of the size field's.
fn to_width(bits: Option<u32>) -> LaneWidth {
    bits.map_or(LaneWidth::Size { doubled: false }, LaneWidth::Fixed)
}

/// The operand types of a modeled instruction of the module, none for every other instruction: its register numbers,
/// the fields that choose its form, its predicate, the registers it reads, then FPCR and FPSR where it reads them.
pub fn list_operand_types(instruction: Instruction<Infallible>) -> Option<Vec<OperandType>> {
    let operation = to_remaining_operation(instruction)?;
    let mut types = Vec::new();
    match operation {
        RemainingOperation::Integer(operation) => list_integer(operation, &mut types),
        RemainingOperation::FloatingPoint(operation) => list_floating_point(operation, &mut types),
    }
    Some(types)
}

fn register(types: &mut Vec<OperandType>, field: Field) {
    types.push(OperandType::VectorRegisterNumber(field));
}

fn predicate(types: &mut Vec<OperandType>) {
    types.extend([
        OperandType::PredicateNumber(PG),
        OperandType::ScalablePredicateValue(PG),
    ]);
}

fn list_integer(operation: RemainingInteger, types: &mut Vec<OperandType>) {
    use LaneKind::{Integer, Moved};
    use RemainingInteger as R;
    let byte = LaneWidth::Fixed(u8::BITS);
    // The second source's register and the operand of its values.
    let second = |types: &mut Vec<OperandType>,
                  second: IntegerSecond,
                  bits: u32,
                  group: u32,
                  width: LaneWidth| {
        match second {
            IntegerSecond::Vector => {
                register(types, ZM);
                types.push(lanes(ZM, Integer, width));
            }
            IntegerSecond::Indexed(layout) => {
                register(types, layout.to_register_field());
                types.push(element(layout, bits, group, ElementValues::Integer));
            }
        }
    };
    match operation {
        R::Multiply {
            operation,
            second: second_element,
            bits,
        } => {
            if bits.is_none() {
                types.push(OperandType::UnsignedImmediate(SIZE));
            }
            let width = to_width(bits);
            register(types, ZD);
            register(types, ZN);
            types.push(lanes(ZN, Integer, width));
            second(types, second_element, bits.unwrap_or(u8::BITS), 1, width);
            if matches!(
                operation,
                SameMultiply::Accumulate { .. } | SameMultiply::RoundingDoublingAccumulate { .. }
            ) {
                types.push(lanes(ZD, Integer, width));
            }
        }
        R::MultiplyLong {
            operation,
            layout,
            narrow_bits,
            ..
        } => {
            register(types, ZD);
            register(types, ZN);
            types.push(lanes(ZN, Integer, LaneWidth::Fixed(narrow_bits)));
            second(types, IntegerSecond::Indexed(layout), narrow_bits, 1, byte);
            if !matches!(
                operation,
                LongMultiply::Multiply | LongMultiply::SaturatingDoubling
            ) {
                types.push(lanes(ZD, Integer, LaneWidth::Fixed(2 * narrow_bits)));
            }
        }
        R::Dot {
            second: second_element,
            bits,
            ..
        }
        | R::ComplexDot {
            second: second_element,
            bits,
        } => {
            if bits.is_none() {
                let field = if matches!(operation, R::Dot { .. }) {
                    SZ
                } else {
                    SIZE
                };
                types.push(OperandType::UnsignedImmediate(field));
            }
            let accumulator = bits.unwrap_or(u32::BITS);
            register(types, ZD);
            register(types, ZN);
            types.push(lanes(ZN, Integer, byte));
            second(types, second_element, accumulator / 4, 4, byte);
            types.push(lanes(ZD, Integer, LaneWidth::Fixed(accumulator)));
            if matches!(operation, R::ComplexDot { .. }) {
                types.push(OperandType::UnsignedImmediate(integer::ROTATION));
            }
        }
        R::ComplexMultiplyAccumulate {
            second: second_element,
            bits,
            ..
        } => {
            if bits.is_none() {
                types.push(OperandType::UnsignedImmediate(SIZE));
            }
            let width = to_width(bits);
            register(types, ZD);
            register(types, ZN);
            types.push(lanes(ZN, Integer, width));
            second(types, second_element, bits.unwrap_or(u8::BITS), 2, width);
            types.push(lanes(ZD, Integer, width));
            types.push(OperandType::UnsignedImmediate(integer::ROTATION));
        }
        R::MatrixMultiply { .. } => {
            register(types, ZD);
            register(types, ZN);
            register(types, ZM);
            types.push(lanes(ZN, Integer, byte));
            types.push(lanes(ZM, Integer, byte));
            types.push(lanes(ZD, Integer, LaneWidth::Fixed(u32::BITS)));
        }
        R::ShiftLeftLong { .. } => {
            register(types, ZD);
            register(types, ZN);
            types.push(OperandType::EveryValue(&SHIFT_FIELDS));
            types.push(lanes(ZN, Integer, byte));
        }
        R::ShiftRightNarrow { top, .. } | R::ExtractNarrow { top, .. } => {
            let shifted = matches!(operation, R::ShiftRightNarrow { .. });
            register(types, ZD);
            register(types, ZN);
            types.push(OperandType::EveryValue(if shifted {
                &SHIFT_FIELDS
            } else {
                &EXTRACT_FIELDS
            }));
            types.push(OperandType::Remaining(RemainingOperand::NarrowSource {
                shifted,
            }));
            if top {
                types.push(lanes(ZD, Moved, byte));
            }
        }
        R::Address(offset) => {
            if offset == AddressOffset::Same {
                types.push(OperandType::UnsignedImmediate(SZ));
            }
            types.push(OperandType::UnsignedImmediate(MSZ));
            register(types, ZD);
            register(types, ZN);
            register(types, ZM);
            types.push(lanes(ZN, Integer, LaneWidth::Fixed(u32::BITS)));
            types.push(lanes(ZM, Integer, LaneWidth::Fixed(u32::BITS)));
        }
        R::PolynomialMultiplyLong { .. } => {
            types.push(OperandType::Remaining(RemainingOperand::ElementSize {
                field: SIZE,
                sizes: POLYNOMIAL_SIZES,
            }));
            register(types, ZD);
            register(types, ZN);
            register(types, ZM);
            types.push(lanes(ZN, Integer, byte));
            types.push(lanes(ZM, Integer, byte));
        }
        R::BitPermute(_) => {
            types.push(OperandType::UnsignedImmediate(SIZE));
            register(types, ZD);
            register(types, ZN);
            register(types, ZM);
            types.push(lanes(ZN, Integer, LaneWidth::Size { doubled: false }));
            types.push(lanes(ZM, Integer, LaneWidth::Size { doubled: false }));
        }
        R::LongCarry { .. } => {
            types.push(OperandType::UnsignedImmediate(SZ));
            register(types, ZD);
            register(types, ZN);
            register(types, ZM);
            let word = LaneWidth::Fixed(u32::BITS);
            types.push(lanes(ZN, Integer, word));
            types.push(lanes(ZM, Integer, word));
            types.push(lanes(ZD, Integer, word));
        }
        R::ComplexAdd { .. } => {
            types.push(OperandType::UnsignedImmediate(SIZE));
            types.push(OperandType::UnsignedImmediate(integer::ADD_ROTATION));
            register(types, ZD);
            register(types, ZM_LOW);
            types.push(lanes(ZM_LOW, Integer, LaneWidth::Size { doubled: false }));
            types.push(lanes(ZD, Integer, LaneWidth::Size { doubled: false }));
        }
        R::HistogramSegment => {
            register(types, ZD);
            register(types, ZN);
            register(types, ZM);
            types.push(lanes(ZN, Integer, byte));
            types.push(lanes(ZM, Integer, byte));
        }
        R::HistogramCount => {
            types.push(OperandType::UnsignedImmediate(SIZE));
            register(types, ZD);
            register(types, ZN);
            register(types, ZM);
            predicate(types);
            types.push(lanes(ZN, Integer, LaneWidth::Size { doubled: false }));
            types.push(lanes(ZM, Integer, LaneWidth::Size { doubled: false }));
        }
        R::Aes(step) => {
            register(types, ZD);
            if matches!(step, integer::AesStep::Encrypt | integer::AesStep::Decrypt) {
                register(types, ZM_LOW);
                types.push(lanes(ZM_LOW, Integer, byte));
            }
            types.push(lanes(ZD, Integer, byte));
        }
        R::RotateAndExclusiveOr => {
            register(types, ZD);
            register(types, ZN);
            register(types, ZM);
            let doubleword = LaneWidth::Fixed(u64::BITS);
            types.push(lanes(ZN, Integer, doubleword));
            types.push(lanes(ZM, Integer, doubleword));
        }
    }
}

/// The size field of a floating-point instruction, a valid size first.
const fn float_size() -> OperandType {
    OperandType::Remaining(RemainingOperand::ElementSize {
        field: SIZE,
        sizes: EVERY_SIZE,
    })
}

fn list_floating_point(operation: RemainingFloatingPoint, types: &mut Vec<OperandType>) {
    use LaneKind::{
        BrainFloat, BrainFloatGrouped, FloatingPoint, FloatingPointAddend, FloatingPointEstimate,
        FloatingPointFixed, FloatingPointGrouped, FloatingPointPairs, Integer, Moved, ShiftCount,
    };
    use RemainingFloatingPoint as R;
    let sized = LaneWidth::Size { doubled: false };
    let half = LaneWidth::Fixed(u16::BITS);
    let single = LaneWidth::Fixed(u32::BITS);
    let byte = LaneWidth::Fixed(u8::BITS);
    let control = |types: &mut Vec<OperandType>, reads: ControlReads| {
        types.extend([
            OperandType::FloatingPointControl(reads),
            OperandType::StatusFlags,
        ]);
    };
    let fixed_control = |bits: u32| {
        if bits == u16::BITS {
            ControlReads::Half
        } else {
            ControlReads::SingleOrDouble
        }
    };
    match operation {
        R::Pairwise(_) => {
            types.push(float_size());
            register(types, ZD);
            register(types, ZM_LOW);
            predicate(types);
            types.push(lanes(ZD, FloatingPointPairs, sized));
            types.push(lanes(ZM_LOW, FloatingPointFixed, sized));
            control(types, ControlReads::Half);
        }
        R::IndexedMultiplyAdd { bits, .. } | R::IndexedMultiply { bits } => {
            let layout = to_index_layout(bits);
            let width = LaneWidth::Fixed(bits);
            register(types, ZD);
            register(types, ZN);
            register(types, layout.to_register_field());
            // Every lane multiplies the one element, so lanes of different values pair each value with every element.
            types.push(lanes(ZN, FloatingPointGrouped, width));
            types.push(element(layout, bits, 1, ElementValues::FloatingPoint));
            if matches!(operation, R::IndexedMultiplyAdd { .. }) {
                types.push(lanes(ZD, FloatingPointAddend, width));
            }
            control(types, fixed_control(bits));
        }
        R::ComplexMultiplyAdd { second, bits } => {
            let width = to_width(bits);
            match second {
                FloatSecond::Vector => {
                    types.push(float_size());
                    types.push(OperandType::UnsignedImmediate(MULTIPLY_ROTATION));
                    register(types, ZD);
                    register(types, ZN);
                    register(types, ZM);
                    predicate(types);
                    types.push(lanes(ZN, FloatingPoint, width));
                    types.push(lanes(ZM, FloatingPointGrouped, width));
                }
                FloatSecond::Indexed(layout) => {
                    let bits = bits.unwrap_or(u16::BITS);
                    types.push(OperandType::UnsignedImmediate(INDEXED_ROTATION));
                    register(types, ZD);
                    register(types, ZN);
                    register(types, layout.to_register_field());
                    types.push(lanes(ZN, FloatingPointGrouped, width));
                    types.push(element(layout, bits, 2, ElementValues::FloatingPoint));
                }
            }
            types.push(lanes(ZD, FloatingPointAddend, width));
            control(types, bits.map_or(ControlReads::Half, fixed_control));
        }
        R::ComplexAdd => {
            types.push(float_size());
            types.push(OperandType::UnsignedImmediate(ADD_ROTATION));
            register(types, ZD);
            register(types, ZM_LOW);
            predicate(types);
            types.push(lanes(ZD, FloatingPoint, sized));
            types.push(lanes(ZM_LOW, FloatingPointGrouped, sized));
            control(types, ControlReads::Half);
        }
        R::Reduce(_) => {
            types.push(float_size());
            register(types, ZD);
            register(types, ZN);
            predicate(types);
            types.push(lanes(ZN, FloatingPointPairs, sized));
            control(types, ControlReads::Half);
        }
        R::OrderedAdd => {
            types.push(float_size());
            register(types, ZD);
            register(types, ZM_LOW);
            predicate(types);
            types.push(lanes(ZM_LOW, FloatingPointGrouped, sized));
            types.push(lanes(ZD, FloatingPoint, sized));
            control(types, ControlReads::Half);
        }
        R::Estimate { .. } => {
            types.push(float_size());
            register(types, ZD);
            register(types, ZN);
            types.push(lanes(ZN, FloatingPointEstimate, sized));
            control(types, ControlReads::Half);
        }
        R::ConvertNarrowTop { from, to, .. } | R::ConvertLongTop { from, to } => {
            register(types, ZD);
            register(types, ZN);
            predicate(types);
            types.push(lanes(ZN, FloatingPoint, LaneWidth::Fixed(from.to_bits())));
            types.push(lanes(ZD, Moved, byte));
            let reads = if from == Precision::Half || to == Precision::Half {
                ControlReads::Conversion
            } else {
                ControlReads::SingleOrDouble
            };
            control(types, reads);
        }
        R::ConvertToOdd | R::ConvertToBrainFloat => {
            let from = if operation == R::ConvertToOdd {
                u64::BITS
            } else {
                u32::BITS
            };
            register(types, ZD);
            register(types, ZN);
            predicate(types);
            types.push(lanes(ZN, FloatingPoint, LaneWidth::Fixed(from)));
            types.push(lanes(ZD, Moved, byte));
            control(types, ControlReads::SingleOrDouble);
        }
        R::Logarithm => {
            types.push(OperandType::UnsignedImmediate(LOGARITHM_SIZE));
            register(types, ZD);
            register(types, ZN);
            predicate(types);
            types.push(OperandType::Remaining(
                RemainingOperand::SizedFloatingPoint {
                    field: ZN,
                    size: LOGARITHM_SIZE,
                },
            ));
            types.push(lanes(ZD, Moved, byte));
            control(types, ControlReads::Half);
        }
        R::TrigonometricSelect | R::ExponentialAccelerator | R::TrigonometricMultiply => {
            types.push(float_size());
            register(types, ZD);
            register(types, ZN);
            if operation == R::ExponentialAccelerator {
                types.push(lanes(ZN, Integer, sized));
            } else {
                register(types, ZM);
                types.push(lanes(ZN, FloatingPoint, sized));
                // FTSSEL reads the second source's two lowest bits, which the shift counts hold in each combination.
                types.push(lanes(ZM, ShiftCount, sized));
            }
            if operation == R::TrigonometricMultiply {
                control(types, ControlReads::Half);
            }
        }
        R::TrigonometricMultiplyAdd => {
            types.push(float_size());
            types.push(OperandType::UnsignedImmediate(COEFFICIENT));
            register(types, ZD);
            register(types, ZM_LOW);
            types.push(lanes(ZD, FloatingPoint, sized));
            types.push(lanes(ZM_LOW, FloatingPointGrouped, sized));
            control(types, ControlReads::Half);
        }
        R::BrainFloatDot { second } | R::BrainFloatMultiplyAdd { second, .. } => {
            let multiply_add = matches!(operation, R::BrainFloatMultiplyAdd { .. });
            register(types, ZD);
            register(types, ZN);
            let first = if second == FloatSecond::Vector {
                BrainFloat
            } else {
                BrainFloatGrouped
            };
            types.push(lanes(ZN, first, half));
            match second {
                FloatSecond::Vector => {
                    register(types, ZM);
                    types.push(lanes(ZM, BrainFloatGrouped, half));
                }
                FloatSecond::Indexed(layout) => {
                    register(types, layout.to_register_field());
                    let group = if multiply_add { 1 } else { 2 };
                    types.push(element(layout, u16::BITS, group, ElementValues::BrainFloat));
                }
            }
            types.push(lanes(ZD, FloatingPointAddend, single));
            // BFDOT reads no FPCR on a chip without FEAT_EBF16; BFMLALB is a fused multiply-add of singles.
            if multiply_add {
                control(types, ControlReads::SingleOrDouble);
            }
        }
        R::BrainFloatMatrixMultiply => {
            register(types, ZD);
            register(types, ZN);
            register(types, ZM);
            types.push(lanes(ZN, BrainFloat, half));
            types.push(lanes(ZM, BrainFloatGrouped, half));
            types.push(lanes(ZD, FloatingPointAddend, single));
        }
        R::WideningMultiplyAdd { second, .. } => {
            register(types, ZD);
            register(types, ZN);
            let first = if second == FloatSecond::Vector {
                FloatingPoint
            } else {
                FloatingPointGrouped
            };
            types.push(lanes(ZN, first, half));
            match second {
                FloatSecond::Vector => {
                    register(types, ZM);
                    types.push(lanes(ZM, FloatingPointGrouped, half));
                }
                FloatSecond::Indexed(layout) => {
                    register(types, layout.to_register_field());
                    types.push(element(layout, u16::BITS, 1, ElementValues::FloatingPoint));
                }
            }
            types.push(lanes(ZD, FloatingPointAddend, single));
            control(types, ControlReads::Half);
        }
    }
}
