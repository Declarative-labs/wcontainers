//! The inputs of the Advanced SIMD instructions of the architecture's `advanced_simd` module.
//!
//! A register's value is a pattern of lanes, chosen by its index among the patterns of its kind, at the lane width the
//! word gives: so that one operand's values follow the arrangement the fields choose. An integer lane takes each end
//! of its signed range, zero, one and all ones; a lane a narrowing reads takes both sides of each bound of the
//! narrower element; a floating-point lane takes each critical value of its precision.

use alloc::vec;
use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_processor_architecture_arm64::advanced_simd::{
    AdvancedSimdOperation, DifferentOperation, ElementOperation, FloatingPointShape, MiscOperation,
    PrecisionField, SameOperation, ShiftOperation, to_advanced_simd_operation,
};
use wcontainers_processor_architecture_arm64::encoding::{Field, RD, RM, RN};
use wcontainers_processor_architecture_arm64::floating_point::Precision;
use wcontainers_processor_architecture_arm64::floating_point::lanes::LaneOperation;
use wcontainers_processor_architecture_arm64::operand::{RegisterThirtyOne, RegisterWidth};
use wcontainers_processor_architecture_arm64::{Instruction, Processor};

use super::{ControlReads, OperandType, OperandValue, to_floating_point_bits};

const SIZE: Field = Field::new(22, 2);
const SZ: Field = Field::new(22, 1);
const IMMH: Field = Field::new(19, 4);
const IMMH_IMMB: Field = Field::new(16, 7);
const ELEMENT_H: Field = Field::new(11, 1);
const ELEMENT_L: Field = Field::new(21, 1);
const ELEMENT_M: Field = Field::new(20, 1);
const ELEMENT_RM: Field = Field::new(16, 4);
const ABC: Field = Field::new(16, 3);
const DEFGH: Field = Field::new(5, 5);
/// The modified immediate's eight bits, the most significant first.
const IMM8: [Field; 2] = [ABC, DEFGH];
/// The element index of a by-element instruction, as many of H, L and M as its element size reads.
const ELEMENT_INDEX: [Field; 3] = [ELEMENT_H, ELEMENT_L, ELEMENT_M];
/// The widest field the domain gives every value of; a free run of the word wider than this is split.
const EXHAUSTIVE_FIELD_BITS: u32 = 4;
/// FPSR.QC.
const SATURATION: u64 = 1 << 27;
/// FPSR's cumulative flags besides QC.
const FLOATING_POINT_FLAGS: u64 = 0b1001_1111;
const VECTOR_BITS: u32 = u128::BITS;
const VECTOR_BYTES: u32 = VECTOR_BITS / u8::BITS;

/// An operand of an Advanced SIMD instruction of this module.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SimdOperand {
    /// The register `field` names, `offset` registers on: value k is pattern k of the kind, at the width the word gives.
    Lanes {
        field: Field,
        offset: u32,
        kind: LaneKind,
        width: LaneWidth,
    },
    /// The register a by-element instruction's element is read from, and the element's index in H, L and M: value k
    /// puts the lowest lane of pattern k of the kind at the index `k` gives, every index in turn, and the lowest lanes
    /// of the patterns after it in the other lanes, so that each lane holds a value of its own.
    Element { kind: LaneKind, width: LaneWidth },
    /// immh:immb of a shift by an immediate, for each element size `sizes` holds (bit i for 8 << i bits).
    ShiftImmediate {
        direction: ShiftDirection,
        sizes: u32,
    },
    /// FPSR before an instruction that may set QC: clear, and every cumulative flag set.
    SaturationStatus,
}

/// What the cut does with an operand of this module.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SimdRole {
    Field,
    Data,
    Written,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShiftDirection {
    /// A right shift of the element: 1, 2, half of it, one less than it and all of it.
    Right,
    /// A left shift: none, 1, half of the element and one less than it.
    Left,
}

/// The patterns a register's lanes take.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneKind {
    /// Zero, all ones, bytes counted from one with the top bit clear and set, each lane at the most negative, at the
    /// largest positive and the two alternating, and each lane one.
    Integer,
    /// The signed low byte a shift by register reads: 0, 1, -1, 127 and -128, and both sides of the element size.
    ShiftCount,
    /// Both sides of each bound of an element half as wide, shifted up as far as a narrowing shift by an immediate
    /// brings it down where `shifted`.
    Narrowed { shifted: bool },
    /// Bytes counted from one with the top bit clear and set: what a move shows the order of.
    Moved,
    /// Every byte its own, numbered on from the bytes of the registers before it in the table.
    Table,
    /// Indices into a table of these registers: in its range, at its last byte, just past it and far past it.
    TableIndex { registers: u32 },
    /// Every nine-bit prefix an unsigned estimate looks up, and the values on both sides of where it starts.
    UnsignedEstimate,
    /// Each critical value of the precision in every lane, and lanes of different values.
    FloatingPoint,
    /// Each critical value in some lane, two in each pattern: the second source of a binary operation.
    FloatingPointGrouped,
    /// Every pair of critical values in the two lowest lanes, other values above: the source of a pairwise operation.
    FloatingPointPairs,
    /// One pattern of lanes of different critical values: the second source of a pairwise operation, whose lanes the
    /// first source's pairs already cover.
    FloatingPointFixed,
    /// BFloat16 lanes: each critical value of a single, its low half dropped, in every lane, and lanes of different
    /// values.
    BrainFloat,
    /// Each of those values in some lane, two in each pattern.
    BrainFloatGrouped,
    /// What an addend of a fused multiply-add takes: both zeros, one, the half step that rounds, the smallest
    /// subnormal, the largest normal, both infinities and both kinds of NaN.
    FloatingPointAddend,
    /// The critical values and the bounds of an integer as wide as the lane.
    ConvertedFloatingPoint,
    /// An integer as wide as the lane: its critical values and those just past the precision's significand.
    ConvertedInteger,
    /// The critical values, and every prefix of eight fraction bits with each parity of exponent that an estimate
    /// looks up.
    FloatingPointEstimate,
}

/// The width of a register's lanes, as the word gives it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneWidth {
    /// 8 << size, doubled for the wider elements of a long, wide or narrowing instruction.
    Size {
        doubled: bool,
    },
    /// 32 << sz.
    FloatSize,
    /// 16 << sz: the source of FCVTL.
    FloatSizeHalved,
    /// 8 << the highest set bit of immh, doubled for a narrowing source.
    ShiftImmediate {
        doubled: bool,
    },
    Fixed(u32),
}

impl SimdOperand {
    pub const fn to_role(self) -> SimdRole {
        match self {
            Self::ShiftImmediate { .. } => SimdRole::Field,
            Self::Lanes { .. } | Self::Element { .. } => SimdRole::Data,
            Self::SaturationStatus => SimdRole::Written,
        }
    }
}

/// The lane width the word gives.
const fn to_width(width: LaneWidth, word: u32) -> u32 {
    let immh = IMMH.read(word);
    let highest = if immh == 0 {
        0
    } else {
        u32::BITS - 1 - immh.leading_zeros()
    };
    match width {
        LaneWidth::Size { doubled } => (u8::BITS << SIZE.read(word)) << doubled as u32,
        LaneWidth::FloatSize => u32::BITS << SZ.read(word),
        LaneWidth::FloatSizeHalved => u16::BITS << SZ.read(word),
        LaneWidth::ShiftImmediate { doubled } => {
            let bits = (u8::BITS << highest) << doubled as u32;
            if bits > u64::BITS { u64::BITS } else { bits }
        }
        LaneWidth::Fixed(bits) => bits,
    }
}

/// The precision of a lane this wide; a byte, which no floating-point lane is, as a single.
const fn to_precision(bits: u32) -> Precision {
    match bits {
        16 => Precision::Half,
        64 => Precision::Double,
        _ => Precision::Single,
    }
}

const fn ones(bits: u32) -> u128 {
    if bits >= u128::BITS {
        u128::MAX
    } else {
        (1 << bits) - 1
    }
}

/// A vector whose lane i of `bits` bits is `lane(i)`.
fn fill(bits: u32, lane: impl Fn(u32) -> u128) -> u128 {
    (0..VECTOR_BITS / bits).fold(0, |vector, index| {
        vector | ((lane(index) & ones(bits)) << (index * bits))
    })
}

/// Bytes counted from one, with the top bit set where asked.
fn counted(top: bool) -> u128 {
    (0..VECTOR_BYTES).fold(0, |vector, place| {
        vector | (u128::from(place + 1 + if top { 0x80 } else { 0 }) << (place * u8::BITS))
    })
}

/// How many patterns a kind has, the same for every width.
const fn to_pattern_count(kind: LaneKind) -> usize {
    match kind {
        LaneKind::Integer => 9,
        LaneKind::ShiftCount => 12,
        LaneKind::Narrowed { .. } => 19,
        LaneKind::Moved => 2,
        LaneKind::Table => 1,
        LaneKind::TableIndex { .. } => 4,
        LaneKind::UnsignedEstimate => 194,
        LaneKind::FloatingPoint => 20,
        LaneKind::FloatingPointGrouped => 10,
        LaneKind::FloatingPointPairs => 361,
        LaneKind::FloatingPointFixed => 1,
        LaneKind::BrainFloat => 20,
        LaneKind::BrainFloatGrouped => 10,
        LaneKind::FloatingPointAddend => 10,
        LaneKind::ConvertedFloatingPoint => 32,
        LaneKind::ConvertedInteger => 16,
        LaneKind::FloatingPointEstimate => 276,
    }
}

/// The values a lane of a kind takes in turn, for the kinds whose patterns are lists of lanes.
fn to_lane_values(kind: LaneKind, bits: u32, word: u32) -> Vec<u128> {
    let most_negative = 1_u128 << (bits - 1);
    match kind {
        LaneKind::ShiftCount => {
            let size = u128::from(bits);
            let negative = |value: u128| value.wrapping_neg() & 0xFF;
            vec![
                0,
                1,
                0xFF,
                size - 2,
                size - 1,
                size,
                size + 1,
                negative(size - 1),
                negative(size),
                negative(size + 1),
                0x7F,
                0x80,
            ]
        }
        LaneKind::Narrowed { shifted } => {
            // The narrower element's bounds, as the wider element holds them, each shifted up by the shift a narrowing
            // shift by an immediate takes away, and less half a step, which a rounding shift brings up to the bound.
            let half = bits / 2;
            let shift = if shifted {
                (2 * half).saturating_sub(IMMH_IMMB.read(word))
            } else {
                0
            };
            let bounds: [i128; 8] = [
                (1 << (half - 1)) - 1,
                1 << (half - 1),
                -(1 << (half - 1)),
                -(1 << (half - 1)) - 1,
                (1 << half) - 1,
                1 << half,
                0,
                -1,
            ];
            let mut values = Vec::new();
            for bound in bounds {
                let shifted = bound << shift;
                values.push(shifted as u128 & ones(bits));
                let below = if shift == 0 {
                    shifted
                } else {
                    shifted - (1 << (shift - 1))
                };
                values.push(below as u128 & ones(bits));
            }
            values
        }
        LaneKind::FloatingPointGrouped => to_floating_point_bits(to_precision(bits))
            .into_iter()
            .map(u128::from)
            .collect(),
        LaneKind::BrainFloat | LaneKind::BrainFloatGrouped => to_brain_float_bits(),
        _ => vec![most_negative],
    }
}

/// Pattern k of the kind at lanes of `bits` bits.
fn to_pattern(kind: LaneKind, bits: u32, k: usize, word: u32, offset: u32) -> u128 {
    let most_negative = 1_u128 << (bits - 1);
    let largest = most_negative - 1;
    // A list of lane values laid over the patterns in turn, `step` new values in the lowest lanes of each.
    let laid = |values: Vec<u128>, step: u32| {
        let count = values.len();
        fill(bits, |lane| {
            values[(k * step as usize + lane as usize) % count]
        })
    };
    match kind {
        LaneKind::Integer => match k {
            0 => 0,
            1 => u128::MAX,
            2 => counted(false),
            3 => counted(true),
            4 => fill(bits, |_| most_negative),
            5 => fill(bits, |_| largest),
            6 => fill(bits, |lane| {
                if lane % 2 == 0 {
                    largest
                } else {
                    most_negative
                }
            }),
            7 => fill(bits, |lane| {
                if lane % 2 == 0 {
                    most_negative
                } else {
                    largest
                }
            }),
            _ => fill(bits, |_| 1),
        },
        LaneKind::ShiftCount => laid(to_lane_values(kind, bits, word), 1),
        LaneKind::Narrowed { .. } => match k {
            0 => 0,
            1 => u128::MAX,
            2 => counted(false),
            _ => {
                let values = to_lane_values(kind, bits, word);
                let count = values.len();
                fill(bits, |lane| values[(k - 3 + lane as usize) % count])
            }
        },
        LaneKind::Moved => counted(k == 1),
        LaneKind::Table => (0..VECTOR_BYTES).fold(0, |vector, place| {
            vector | (u128::from(VECTOR_BYTES * offset + place + 1) << (place * u8::BITS))
        }),
        LaneKind::TableIndex { registers } => {
            let size = VECTOR_BYTES * registers;
            let indices: [u32; 8] = [
                size - 1,
                size,
                0,
                u32::from(u8::MAX),
                size + 1,
                0x7F,
                0x80,
                size - VECTOR_BYTES,
            ];
            fill(u8::BITS, |byte| {
                u128::from(match k {
                    0 => byte,
                    1 => size - 1 - byte,
                    2 => size - VECTOR_BYTES + byte,
                    _ => indices[byte as usize % indices.len()],
                })
            })
        }
        LaneKind::UnsignedEstimate => match k {
            0 => fill(u32::BITS, |lane| {
                [0x3FFF_FFFF, 0x4000_0000, 0x7FFF_FFFF, 0x8000_0000][lane as usize]
            }),
            1 => fill(u32::BITS, |lane| {
                [u32::MAX as u128, 0xBFFF_FFFF, 0x8080_0001, 0x0][lane as usize]
            }),
            // Every nine-bit prefix from 0x80 up in the two lowest lanes, with low bits set beside it.
            _ => fill(u32::BITS, |lane| {
                let prefix = 0x80 + ((k as u128 - 2) + 0xC0 * u128::from(lane % 2)) % 0x180;
                (prefix << 23) | 0x55_5555
            }),
        },
        LaneKind::FloatingPoint
        | LaneKind::FloatingPointEstimate
        | LaneKind::ConvertedFloatingPoint => {
            let precision = to_precision(bits);
            let base: Vec<u128> = match kind {
                LaneKind::ConvertedFloatingPoint => {
                    to_converted_floating_point_bits(precision, bits)
                }
                _ => to_floating_point_bits(precision),
            }
            .into_iter()
            .map(u128::from)
            .collect();
            let base_count = match kind {
                LaneKind::FloatingPointEstimate => to_pattern_count(LaneKind::FloatingPoint),
                _ => to_pattern_count(kind),
            };
            if k < base.len() {
                fill(bits, |_| base[k])
            } else if k < base_count {
                // Lanes of different values: each lane its own critical value.
                let start = k * 5;
                fill(bits, |lane| base[(start + lane as usize * 7) % base.len()])
            } else {
                // An estimate's prefixes: each eight fraction bits below the binary point in the lowest lane, its
                // exponent's parity the prefix's lowest bit there and the other in the next lane.
                let prefix = (k - base_count) as u128;
                let fraction_bits = precision.to_fraction_bits();
                let bias = (1_u128 << (bits - fraction_bits - 2)) - 1;
                fill(bits, |lane| {
                    let parity = (prefix + u128::from(lane)) & 1;
                    ((bias + parity) << fraction_bits) | (prefix << (fraction_bits - 8)) | 1
                })
            }
        }
        LaneKind::FloatingPointGrouped => laid(to_lane_values(kind, bits, word), 2),
        LaneKind::BrainFloat | LaneKind::BrainFloatGrouped => {
            let values = to_brain_float_bits();
            let count = values.len();
            let lane = |index: u32| -> u128 {
                match kind {
                    LaneKind::BrainFloatGrouped => values[(2 * k + index as usize) % count],
                    _ if k < count => values[k],
                    _ => values[(k * 5 + index as usize * 7) % count],
                }
            };
            fill(u16::BITS, lane)
        }
        LaneKind::FloatingPointPairs | LaneKind::FloatingPointFixed => {
            let base = to_floating_point_bits(to_precision(bits));
            let count = base.len();
            if kind == LaneKind::FloatingPointFixed {
                return fill(bits, |lane| {
                    u128::from(base[(5 + 7 * lane as usize) % count])
                });
            }
            // The pair in the two lowest lanes, and other values above them, so that a reduction shows each half.
            let (first, second) = ((k / 19) % count, (k % 19) % count);
            fill(bits, |lane| {
                u128::from(match lane {
                    0 => base[first],
                    1 => base[second],
                    _ => base[(first * 3 + second + lane as usize * 5) % count],
                })
            })
        }
        LaneKind::FloatingPointAddend => {
            let precision = to_precision(bits);
            let base = to_floating_point_bits(precision);
            // +0, -0, the smallest subnormal, one, the half step, the largest normal, both infinities, both NaNs.
            let chosen = [
                0,
                1,
                2,
                5,
                7,
                base.len() - 6,
                base.len() - 5,
                base.len() - 4,
                base.len() - 3,
                base.len() - 1,
            ];
            fill(bits, |_| u128::from(base[chosen[k]]))
        }
        LaneKind::ConvertedInteger => {
            let values = to_converted_integer_bits(bits);
            fill(bits, |_| values[k % values.len()])
        }
    }
}

/// The critical values of a single as BFloat16 keeps them, its low half dropped, each once.
fn to_brain_float_bits() -> Vec<u128> {
    let mut values = Vec::new();
    for bits in to_floating_point_bits(Precision::Single) {
        let value = u128::from(bits >> u16::BITS);
        if !values.contains(&value) {
            values.push(value);
        }
    }
    values
}

/// The critical values of the precision and, for a conversion to an integer of `bits` bits, both signs of two to the
/// width less one and of two to the width, each with the values next to it, where the precision holds them.
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
            for value in [magnitude, sign | magnitude] {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
        }
    }
    values
}

/// An integer of `bits` bits: zero, one, all ones, the sign bits of a byte and a halfword, the top bit, the largest
/// positive, and both signs of two to the significand's bits plus one and plus three, and of the next power's, where
/// the lane holds them, which round to the precision of the lane.
fn to_converted_integer_bits(bits: u32) -> Vec<u128> {
    let mask = ones(bits);
    let significand_bits = to_precision(bits).to_fraction_bits() + 1;
    let mut values = vec![0, 1, mask, 0x8080 & mask, 1 << (bits - 1), mask >> 1];
    for power in [significand_bits, significand_bits + 1] {
        if power + 1 >= bits {
            continue;
        }
        for odd in [1, 3] {
            let integer = (1_u128 << power) + odd;
            values.extend([integer, integer.wrapping_neg() & mask]);
        }
    }
    values
}

/// The critical values of an operand of this module.
pub fn generate_critical_values(operand: SimdOperand) -> Vec<OperandValue> {
    match operand {
        SimdOperand::Lanes { kind, .. } | SimdOperand::Element { kind, .. } => (0
            ..to_pattern_count(kind))
            .map(|k| OperandValue::Bits(k as u64))
            .collect(),
        SimdOperand::ShiftImmediate { direction, sizes } => {
            let mut values = Vec::new();
            for size in 0..4 {
                if sizes & (1 << size) == 0 {
                    continue;
                }
                let bits = u8::BITS << size;
                let shifts: &[u32] = match direction {
                    ShiftDirection::Right => &[1, 2, bits / 2, bits - 1, bits],
                    ShiftDirection::Left => &[0, 1, bits / 2, bits - 1],
                };
                for &shift in shifts {
                    let immediate = match direction {
                        ShiftDirection::Right => 2 * bits - shift,
                        ShiftDirection::Left => bits + shift,
                    };
                    let value = OperandValue::Bits(u64::from(immediate));
                    if !values.contains(&value) {
                        values.push(value);
                    }
                }
            }
            values
        }
        SimdOperand::SaturationStatus => [0, SATURATION | FLOATING_POINT_FLAGS]
            .map(OperandValue::Bits)
            .to_vec(),
    }
}

/// The bits the operand puts in a word that holds every field but these: an element's index, and for an element of
/// a word or more M, the top bit of its register, from the bit above the index.
pub fn encode(operand: SimdOperand, value: OperandValue, word: u32) -> u32 {
    match (operand, value) {
        (SimdOperand::ShiftImmediate { .. }, OperandValue::Bits(bits)) => {
            IMMH_IMMB.to_bits(bits as u32)
        }
        (SimdOperand::Element { width, .. }, OperandValue::Bits(k)) => {
            let bits = to_width(width, word);
            let fields = to_index_fields(bits);
            let count = fields.len() as u32;
            let index = k as u32 % (1 << count);
            let indexed = fields
                .iter()
                .rev()
                .enumerate()
                .fold(0, |encoded, (place, field)| {
                    encoded | field.to_bits(index >> place)
                });
            if count < ELEMENT_INDEX.len() as u32 {
                indexed | ELEMENT_M.to_bits((k as u32 >> count) & 1)
            } else {
                indexed
            }
        }
        _ => 0,
    }
}

/// The index fields an element of this width reads, the most significant first.
fn to_index_fields(bits: u32) -> &'static [Field] {
    match bits {
        8 | 16 => &ELEMENT_INDEX,
        64 => &ELEMENT_INDEX[..1],
        _ => &ELEMENT_INDEX[..2],
    }
}

/// Puts the operand's value in the state, once the word holds every field.
pub fn apply<LevelSpecific>(
    operand: SimdOperand,
    value: OperandValue,
    word: u32,
    processor: &mut Processor<LevelSpecific>,
) {
    let OperandValue::Bits(k) = value else {
        return;
    };
    match operand {
        SimdOperand::Lanes {
            field,
            offset,
            kind,
            width,
        } => {
            let register = (field.read(word) + offset) as usize % processor.vector_registers.len();
            processor.vector_registers[register] =
                to_pattern(kind, to_width(width, word), k as usize, word, offset);
        }
        SimdOperand::Element { kind, width } => {
            let bits = to_width(width, word);
            let (register, index) = to_element_place(word, bits);
            processor.vector_registers[register] =
                to_element_register(kind, bits, k as usize, index, word);
        }
        SimdOperand::ShiftImmediate { .. } => {}
        SimdOperand::SaturationStatus => processor.floating_point_status = k,
    }
}

/// The register of a by-element instruction's element: the lowest lane of pattern k at the index, of the patterns
/// after it elsewhere.
fn to_element_register(kind: LaneKind, bits: u32, k: usize, index: u32, word: u32) -> u128 {
    let count = to_pattern_count(kind);
    let lowest = |pattern: usize| to_pattern(kind, bits, pattern % count, word, 0) & ones(bits);
    fill(bits, |lane| {
        if lane == index {
            lowest(k)
        } else {
            lowest(k + 1 + lane as usize)
        }
    })
}

/// The register and index of a by-element instruction's element, as the architecture reads them.
fn to_element_place(word: u32, bits: u32) -> (usize, u32) {
    let (h, l, m) = (
        ELEMENT_H.read(word),
        ELEMENT_L.read(word),
        ELEMENT_M.read(word),
    );
    let rm = ELEMENT_RM.read(word) as usize;
    match bits {
        8 | 16 => (rm, (h << 2) | (l << 1) | m),
        32 => (rm | ((m as usize) << 4), (h << 1) | l),
        _ => (rm | ((m as usize) << 4), h),
    }
}

/// Whether every lane of the operand's value is a NaN or an infinity, so that no result is rounded.
pub fn is_nonfinite(operand: SimdOperand, value: OperandValue, word: u32) -> bool {
    to_floating_point_lanes(operand, value, word).is_some_and(|(lanes, precision)| {
        lanes.iter().all(|lane| {
            let exponent =
                (lane >> precision.to_fraction_bits()) & ((1 << precision.to_exponent_bits()) - 1);
            exponent == (1 << precision.to_exponent_bits()) - 1
        })
    })
}

/// Whether some lane of the operand's value is subnormal, so that a flush control can change it.
pub fn is_subnormal(operand: SimdOperand, value: OperandValue, word: u32) -> bool {
    to_floating_point_lanes(operand, value, word).is_some_and(|(lanes, precision)| {
        lanes.iter().any(|lane| {
            let exponent =
                (lane >> precision.to_fraction_bits()) & ((1 << precision.to_exponent_bits()) - 1);
            exponent == 0 && lane & ((1 << precision.to_fraction_bits()) - 1) != 0
        })
    })
}

/// The lanes of a floating-point operand's value and their precision; none for every other operand.
fn to_floating_point_lanes(
    operand: SimdOperand,
    value: OperandValue,
    word: u32,
) -> Option<(Vec<u64>, Precision)> {
    let (SimdOperand::Lanes { kind, width, .. } | SimdOperand::Element { kind, width }) = operand
    else {
        return None;
    };
    // A pairwise source's lanes give results of their own beside the other source's, so it decides no rounding.
    if !matches!(
        kind,
        LaneKind::FloatingPoint
            | LaneKind::FloatingPointGrouped
            | LaneKind::FloatingPointAddend
            | LaneKind::ConvertedFloatingPoint
            | LaneKind::FloatingPointEstimate
    ) {
        return None;
    }
    let OperandValue::Bits(k) = value else {
        return None;
    };
    let bits = to_width(width, word);
    // An element is the one lane every result reads.
    if let SimdOperand::Element { .. } = operand {
        let lane = (to_pattern(kind, bits, k as usize, word, 0) & ones(bits)) as u64;
        return Some((vec![lane], to_precision(bits)));
    }
    let pattern = to_pattern(kind, bits, k as usize, word, 0);
    let lanes = (0..VECTOR_BITS / bits)
        .map(|lane| ((pattern >> (lane * bits)) & ones(bits)) as u64)
        .collect();
    Some((lanes, to_precision(bits)))
}

/// Reads the operand's value back from a state, for the domain's own check.
pub fn read_back(
    operand: SimdOperand,
    word: u32,
    processor: &Processor<()>,
) -> Option<OperandValue> {
    match operand {
        SimdOperand::ShiftImmediate { .. } => {
            Some(OperandValue::Bits(u64::from(IMMH_IMMB.read(word))))
        }
        SimdOperand::SaturationStatus => Some(OperandValue::Bits(processor.floating_point_status)),
        SimdOperand::Lanes { kind, .. } | SimdOperand::Element { kind, .. } => (0
            ..to_pattern_count(kind))
            .map(|k| OperandValue::Bits(k as u64))
            .find(|value| {
                let mut built = processor.clone();
                apply(operand, *value, word, &mut built);
                built.vector_registers == processor.vector_registers
                    && encode(operand, *value, word) == encode(operand, *value, word) & word
            }),
    }
}

/// The element sizes a shift by an immediate may name where the encoding fixes the top bits of immh, as a set of
/// sizes, bit i for 8 << i bits.
fn to_shift_sizes(mask: u32, value: u32, narrow_or_long: bool, floating: bool) -> u32 {
    (0..4)
        .filter(|size| {
            let immh = 1_u32 << size;
            let word = IMMH.to_bits(immh);
            let fits = word & mask & IMMH.to_mask() == value & IMMH.to_mask();
            fits && !(narrow_or_long && *size == 3) && !(floating && *size == 0)
        })
        .fold(0, |sizes, size| sizes | (1 << size))
}

/// The operands of an instruction of this module, none for every other instruction: its registers, the lanes it
/// reads, its immediates, the fields its encoding leaves free, then FPCR and FPSR where it reads or adds to them.
pub fn list_operand_types(instruction: Instruction<Infallible>) -> Option<Vec<OperandType>> {
    use AdvancedSimdOperation as A;
    use LaneKind as K;
    let operation = to_advanced_simd_operation(instruction)?;
    let record = instruction.to_record()?;
    let mask = record.fixed_bits.mask;
    let lanes = |field: Field, kind: LaneKind, width: LaneWidth| {
        OperandType::AdvancedSimd(SimdOperand::Lanes {
            field,
            offset: 0,
            kind,
            width,
        })
    };
    let size = |doubled| LaneWidth::Size { doubled };
    let shift_width = |doubled| LaneWidth::ShiftImmediate { doubled };
    let vector = OperandType::VectorRegisterNumber;
    let mut types = Vec::new();
    // The bits the operands below put in the word, which are not free fields.
    let mut covered = 0_u32;
    let register = |types: &mut Vec<OperandType>, field: Field| {
        types.push(vector(field));
    };
    let floating_point = |precision: PrecisionField| match precision {
        PrecisionField::Size => LaneWidth::FloatSize,
        PrecisionField::Half => LaneWidth::Fixed(u16::BITS),
    };
    let float_kind = |operation: LaneOperation| match operation {
        LaneOperation::ToInteger { .. } => K::ConvertedFloatingPoint,
        LaneOperation::FromInteger { .. } => K::ConvertedInteger,
        LaneOperation::ReciprocalEstimate | LaneOperation::ReciprocalSquareRootEstimate => {
            K::FloatingPointEstimate
        }
        _ => K::FloatingPoint,
    };
    let mut control = None;
    let mut status = None;
    let scalar = record.fixed_bits.value & (1 << 28) != 0;
    match operation {
        A::Same(operation) => {
            let reads_destination = matches!(
                operation,
                SameOperation::AbsoluteDifferenceAccumulate(_)
                    | SameOperation::MultiplyAccumulate { .. }
                    | SameOperation::RoundingDoublingMultiplyAccumulate { .. }
                    | SameOperation::BitwiseSelect(_)
            );
            let second = if matches!(operation, SameOperation::SaturatingShift { .. }) {
                K::ShiftCount
            } else {
                K::Integer
            };
            register(&mut types, RD);
            register(&mut types, RN);
            register(&mut types, RM);
            types.push(lanes(RN, K::Integer, size(false)));
            types.push(lanes(RM, second, size(false)));
            if reads_destination {
                types.push(lanes(RD, K::Integer, size(false)));
            }
        }
        A::Different(operation) => {
            let wide = matches!(
                operation,
                DifferentOperation::AddWide(_) | DifferentOperation::SubtractWide(_)
            );
            let narrow = matches!(
                operation,
                DifferentOperation::AddHighNarrow { .. }
                    | DifferentOperation::SubtractHighNarrow { .. }
            );
            let accumulates = matches!(
                operation,
                DifferentOperation::AbsoluteDifferenceLong { accumulate: true, .. }
                    | DifferentOperation::MultiplyLong {
                        accumulate: wcontainers_processor_architecture_arm64::advanced_simd::Accumulate::Add
                            | wcontainers_processor_architecture_arm64::advanced_simd::Accumulate::Subtract,
                        ..
                    }
                    | DifferentOperation::SaturatingDoublingMultiplyLong(
                        wcontainers_processor_architecture_arm64::advanced_simd::Accumulate::Add
                            | wcontainers_processor_architecture_arm64::advanced_simd::Accumulate::Subtract
                    )
            );
            register(&mut types, RD);
            register(&mut types, RN);
            register(&mut types, RM);
            types.push(lanes(RN, K::Integer, size(wide || narrow)));
            types.push(lanes(RM, K::Integer, size(narrow)));
            if accumulates {
                types.push(lanes(RD, K::Integer, size(true)));
            } else if narrow {
                types.push(lanes(RD, K::Moved, size(false)));
            }
        }
        A::Misc(operation) => {
            let (source, width) = match operation {
                MiscOperation::Reverse(_) => (K::Moved, size(false)),
                MiscOperation::ExtractNarrow(_) => (K::Narrowed { shifted: false }, size(true)),
                MiscOperation::UnsignedReciprocalEstimate
                | MiscOperation::UnsignedReciprocalSquareRootEstimate => {
                    (K::UnsignedEstimate, LaneWidth::Fixed(u32::BITS))
                }
                _ => (K::Integer, size(false)),
            };
            register(&mut types, RD);
            register(&mut types, RN);
            types.push(lanes(RN, source, width));
            match operation {
                MiscOperation::AddPairwiseLong {
                    accumulate: true, ..
                } => {
                    types.push(lanes(RD, K::Integer, size(true)));
                }
                MiscOperation::SaturatingAccumulate(_) => {
                    types.push(lanes(RD, K::Integer, size(false)))
                }
                MiscOperation::ExtractNarrow(_) if record.fixed_bits.value & (1 << 28) == 0 => {
                    types.push(lanes(RD, K::Moved, size(false)));
                }
                _ => {}
            }
        }
        A::Across(_) | A::ScalarAddPairwise => {
            register(&mut types, RD);
            register(&mut types, RN);
            types.push(lanes(RN, K::Integer, size(false)));
        }
        A::Element(operation) => {
            let (width, element_width) = match operation {
                ElementOperation::Dot { .. } => {
                    (LaneWidth::Fixed(u8::BITS), LaneWidth::Fixed(u32::BITS))
                }
                _ => (size(false), size(false)),
            };
            let long = matches!(
                operation,
                ElementOperation::MultiplyLong { .. }
                    | ElementOperation::SaturatingDoublingMultiplyLong(_)
            );
            let accumulates = match operation {
                ElementOperation::Multiply(accumulate)
                | ElementOperation::MultiplyLong { accumulate, .. }
                | ElementOperation::SaturatingDoublingMultiplyLong(accumulate) => {
                    accumulate
                        != wcontainers_processor_architecture_arm64::advanced_simd::Accumulate::None
                }
                ElementOperation::RoundingDoublingMultiplyAccumulate { .. }
                | ElementOperation::Dot { .. } => true,
                ElementOperation::DoublingMultiplyHigh { .. } => false,
            };
            register(&mut types, RD);
            register(&mut types, RN);
            register(&mut types, ELEMENT_RM);
            types.push(lanes(RN, K::Integer, width));
            types.push(OperandType::AdvancedSimd(SimdOperand::Element {
                kind: K::Integer,
                width: element_operand_width(element_width, mask, record.fixed_bits.value),
            }));
            covered |= ELEMENT_H.to_mask() | ELEMENT_L.to_mask() | ELEMENT_M.to_mask();
            if accumulates {
                let accumulator_width = match operation {
                    ElementOperation::Dot { .. } => LaneWidth::Fixed(u32::BITS),
                    _ => size(long),
                };
                types.push(lanes(RD, K::Integer, accumulator_width));
            }
        }
        A::Shift(operation) => {
            let narrow = matches!(operation, ShiftOperation::ShiftRightNarrow { .. });
            let long = matches!(operation, ShiftOperation::ShiftLeftLong(_));
            let floating = matches!(
                operation,
                ShiftOperation::ToFixed(_) | ShiftOperation::FromFixed(_)
            );
            let direction = match operation {
                ShiftOperation::ShiftLeft
                | ShiftOperation::SaturatingShiftLeft { .. }
                | ShiftOperation::ShiftLeftLong(_)
                | ShiftOperation::Insert { left: true } => ShiftDirection::Left,
                _ => ShiftDirection::Right,
            };
            let sizes = to_shift_sizes(mask, record.fixed_bits.value, narrow || long, floating);
            let source = match operation {
                ShiftOperation::ShiftRightNarrow { .. } => K::Narrowed { shifted: true },
                ShiftOperation::ToFixed(_) => K::ConvertedFloatingPoint,
                ShiftOperation::FromFixed(_) => K::ConvertedInteger,
                _ => K::Integer,
            };
            register(&mut types, RD);
            register(&mut types, RN);
            types.push(OperandType::AdvancedSimd(SimdOperand::ShiftImmediate {
                direction,
                sizes,
            }));
            covered |= IMMH_IMMB.to_mask();
            types.push(lanes(RN, source, shift_width(narrow)));
            match operation {
                ShiftOperation::ShiftRight {
                    accumulate: true, ..
                }
                | ShiftOperation::Insert { .. } => {
                    types.push(lanes(RD, K::Integer, shift_width(false)));
                }
                ShiftOperation::ShiftRightNarrow { .. }
                    if record.fixed_bits.value & (1 << 28) == 0 =>
                {
                    types.push(lanes(RD, K::Moved, shift_width(false)));
                }
                _ => {}
            }
            if floating {
                control = Some(ControlReads::Half);
                status = Some(OperandType::StatusFlags);
            }
        }
        A::Table {
            registers,
            extension,
        } => {
            register(&mut types, RM);
            register(&mut types, RD);
            register(&mut types, RN);
            for offset in 0..registers {
                types.push(OperandType::AdvancedSimd(SimdOperand::Lanes {
                    field: RN,
                    offset,
                    kind: K::Table,
                    width: LaneWidth::Fixed(u8::BITS),
                }));
            }
            if extension {
                types.push(lanes(RD, K::Moved, LaneWidth::Fixed(u8::BITS)));
            }
            // The indices last, so that a table register they share keeps them.
            types.push(lanes(
                RM,
                K::TableIndex { registers },
                LaneWidth::Fixed(u8::BITS),
            ));
        }
        A::Extract => {
            register(&mut types, RD);
            register(&mut types, RN);
            register(&mut types, RM);
            types.push(lanes(RN, K::Moved, LaneWidth::Fixed(u8::BITS)));
            types.push(lanes(RM, K::Integer, LaneWidth::Fixed(u8::BITS)));
        }
        A::DuplicateElement | A::DuplicateToScalar | A::InsertElement | A::MoveToGeneral { .. } => {
            if matches!(operation, A::MoveToGeneral { .. }) {
                types.push(OperandType::RegisterNumber(RD));
            } else {
                register(&mut types, RD);
            }
            register(&mut types, RN);
            types.push(lanes(RN, K::Moved, LaneWidth::Fixed(u8::BITS)));
            if operation == A::InsertElement {
                types.push(lanes(RD, K::Integer, LaneWidth::Fixed(u8::BITS)));
            }
        }
        A::DuplicateGeneral | A::InsertGeneral => {
            register(&mut types, RD);
            types.push(OperandType::RegisterNumber(RN));
            types.push(OperandType::RegisterValue {
                field: RN,
                width: RegisterWidth::Bits64,
                thirty_one: RegisterThirtyOne::ZeroRegister,
            });
            if operation == A::InsertGeneral {
                types.push(lanes(RD, K::Moved, LaneWidth::Fixed(u8::BITS)));
            }
        }
        A::ModifiedImmediate => {
            register(&mut types, RD);
            types.push(OperandType::SplitImmediate(&IMM8));
            covered |= ABC.to_mask() | DEFGH.to_mask();
            // ORR and BIC keep what they do not set or clear.
            let cmode = (record.fixed_bits.value >> 12) & 0xF;
            if cmode < 0b1100 && cmode & 1 == 1 && mask & (1 << 12) != 0 {
                types.push(lanes(RD, K::Moved, LaneWidth::Fixed(u8::BITS)));
            }
        }
        A::Dot { .. } | A::MatrixMultiply { .. } => {
            register(&mut types, RD);
            register(&mut types, RN);
            register(&mut types, RM);
            types.push(lanes(RN, K::Integer, LaneWidth::Fixed(u8::BITS)));
            types.push(lanes(RM, K::Integer, LaneWidth::Fixed(u8::BITS)));
            types.push(lanes(RD, K::Integer, LaneWidth::Fixed(u32::BITS)));
        }
        A::FloatingPoint {
            shape,
            precision,
            operation,
        } => {
            let width = floating_point(precision);
            let source = float_kind(operation);
            // A scalar reads its lowest lane only, which every value reaches; a vector's two lowest lanes do.
            let second = if scalar {
                K::FloatingPoint
            } else {
                K::FloatingPointGrouped
            };
            register(&mut types, RD);
            register(&mut types, RN);
            match shape {
                FloatingPointShape::Misc => types.push(lanes(RN, source, width)),
                FloatingPointShape::ScalarPairwise | FloatingPointShape::Across => {
                    types.push(lanes(RN, K::FloatingPointPairs, width));
                }
                FloatingPointShape::Pairwise => {
                    register(&mut types, RM);
                    types.push(lanes(RN, K::FloatingPointPairs, width));
                    types.push(lanes(RM, K::FloatingPointFixed, width));
                }
                FloatingPointShape::Same => {
                    register(&mut types, RM);
                    types.push(lanes(RN, source, width));
                    types.push(lanes(RM, second, width));
                }
                FloatingPointShape::Accumulating => {
                    register(&mut types, RM);
                    types.push(lanes(RN, source, width));
                    types.push(lanes(RM, second, width));
                    types.push(lanes(RD, K::FloatingPointAddend, width));
                }
                FloatingPointShape::Element { accumulating } => {
                    register(&mut types, ELEMENT_RM);
                    let first = if accumulating { second } else { source };
                    types.push(lanes(RN, first, width));
                    types.push(OperandType::AdvancedSimd(SimdOperand::Element {
                        kind: K::FloatingPoint,
                        width: element_operand_width(width, mask, record.fixed_bits.value),
                    }));
                    covered |= ELEMENT_H.to_mask() | ELEMENT_L.to_mask() | ELEMENT_M.to_mask();
                    if accumulating {
                        types.push(lanes(RD, K::FloatingPointAddend, width));
                    }
                }
            }
            if operation_reads_control(operation) {
                control = Some(match precision {
                    PrecisionField::Half => ControlReads::Half,
                    PrecisionField::Size => ControlReads::SingleOrDouble,
                });
                status = Some(OperandType::StatusFlags);
            }
        }
        A::ConvertLonger | A::ConvertNarrower { .. } => {
            let width = if operation == A::ConvertLonger {
                LaneWidth::FloatSizeHalved
            } else {
                LaneWidth::FloatSize
            };
            register(&mut types, RD);
            register(&mut types, RN);
            types.push(lanes(RN, K::FloatingPoint, width));
            if operation != A::ConvertLonger && record.fixed_bits.value & (1 << 28) == 0 {
                types.push(lanes(RD, K::Moved, LaneWidth::Fixed(u8::BITS)));
            }
            control = Some(
                if matches!(operation, A::ConvertNarrower { to_odd: true }) {
                    ControlReads::SingleOrDouble
                } else {
                    ControlReads::Conversion
                },
            );
            status = Some(OperandType::StatusFlags);
        }
        A::ComplexMultiplyAccumulate { element } => {
            register(&mut types, RD);
            register(&mut types, RN);
            register(&mut types, RM);
            types.push(lanes(RN, K::FloatingPoint, size(false)));
            types.push(lanes(RM, K::FloatingPointGrouped, size(false)));
            types.push(lanes(RD, K::FloatingPointAddend, size(false)));
            if element {
                covered |= ELEMENT_H.to_mask() | ELEMENT_L.to_mask();
                types.push(OperandType::SplitImmediate(&ELEMENT_INDEX[..2]));
            }
            control = Some(ControlReads::Half);
            status = Some(OperandType::StatusFlags);
        }
        A::ComplexAdd => {
            register(&mut types, RD);
            register(&mut types, RN);
            register(&mut types, RM);
            types.push(lanes(RN, K::FloatingPoint, size(false)));
            types.push(lanes(RM, K::FloatingPointGrouped, size(false)));
            control = Some(ControlReads::Half);
            status = Some(OperandType::StatusFlags);
        }
        A::ConvertToBrainFloat => {
            register(&mut types, RD);
            register(&mut types, RN);
            types.push(lanes(RN, K::FloatingPoint, LaneWidth::Fixed(u32::BITS)));
            types.push(lanes(RD, K::Moved, LaneWidth::Fixed(u8::BITS)));
            control = Some(ControlReads::SingleOrDouble);
            status = Some(OperandType::StatusFlags);
        }
        A::BrainFloatDot { element } | A::BrainFloatMultiplyAccumulate { element } => {
            let multiply_accumulate = matches!(operation, A::BrainFloatMultiplyAccumulate { .. });
            register(&mut types, RD);
            register(&mut types, RN);
            let first = if element {
                K::BrainFloatGrouped
            } else {
                K::BrainFloat
            };
            types.push(lanes(RN, first, LaneWidth::Fixed(u16::BITS)));
            if element {
                register(&mut types, ELEMENT_RM);
                let width = if multiply_accumulate {
                    u16::BITS
                } else {
                    u32::BITS
                };
                types.push(OperandType::AdvancedSimd(SimdOperand::Element {
                    kind: K::BrainFloat,
                    width: LaneWidth::Fixed(width),
                }));
                covered |= ELEMENT_H.to_mask() | ELEMENT_L.to_mask() | ELEMENT_M.to_mask();
            } else {
                register(&mut types, RM);
                types.push(lanes(RM, K::BrainFloatGrouped, LaneWidth::Fixed(u16::BITS)));
            }
            types.push(lanes(
                RD,
                K::FloatingPointAddend,
                LaneWidth::Fixed(u32::BITS),
            ));
            // BFDOT reads no FPCR on a chip without FEAT_EBF16; BFMLAL is a fused multiply-add of singles.
            if multiply_accumulate {
                control = Some(ControlReads::SingleOrDouble);
                status = Some(OperandType::StatusFlags);
            }
        }
        A::BrainFloatMatrixMultiply => {
            register(&mut types, RD);
            register(&mut types, RN);
            register(&mut types, RM);
            types.push(lanes(RN, K::BrainFloat, LaneWidth::Fixed(u16::BITS)));
            types.push(lanes(RM, K::BrainFloatGrouped, LaneWidth::Fixed(u16::BITS)));
            types.push(lanes(
                RD,
                K::FloatingPointAddend,
                LaneWidth::Fixed(u32::BITS),
            ));
        }
        A::WideningMultiplyAccumulate { element, .. } => {
            register(&mut types, RD);
            register(&mut types, RN);
            types.push(lanes(RN, K::FloatingPoint, LaneWidth::Fixed(u16::BITS)));
            if element {
                register(&mut types, ELEMENT_RM);
                types.push(OperandType::AdvancedSimd(SimdOperand::Element {
                    kind: K::FloatingPointGrouped,
                    width: LaneWidth::Fixed(u16::BITS),
                }));
                covered |= ELEMENT_H.to_mask() | ELEMENT_L.to_mask() | ELEMENT_M.to_mask();
            } else {
                register(&mut types, RM);
                types.push(lanes(
                    RM,
                    K::FloatingPointGrouped,
                    LaneWidth::Fixed(u16::BITS),
                ));
            }
            types.push(lanes(
                RD,
                K::FloatingPointAddend,
                LaneWidth::Fixed(u32::BITS),
            ));
            control = Some(ControlReads::Half);
            status = Some(OperandType::StatusFlags);
        }
    }
    // Every register field named above is covered, and so are the fields the element and lanes operands name.
    for operand_type in &types {
        match *operand_type {
            OperandType::VectorRegisterNumber(field) | OperandType::RegisterNumber(field) => {
                covered |= field.to_mask();
            }
            OperandType::SplitImmediate(fields) => {
                covered |= fields.iter().fold(0, |all, field| all | field.to_mask());
            }
            _ => {}
        }
    }
    types.extend(to_free_fields(!mask & !covered));
    if operation.saturates() {
        types.push(OperandType::AdvancedSimd(SimdOperand::SaturationStatus));
    }
    if let Some(reads) = control {
        types.push(OperandType::FloatingPointControl(reads));
    }
    if let Some(status) = status {
        types.push(status);
    }
    Some(types)
}

/// Whether a floating-point lane operation reads FPCR.
const fn operation_reads_control(operation: LaneOperation) -> bool {
    use wcontainers_processor_architecture_arm64::floating_point::FloatingPointOperation as F;
    !matches!(operation, LaneOperation::Scalar(F::Absolute | F::Negate))
}

/// The width of a by-element instruction's element: fixed where the encoding fixes its size, else the size field's.
fn element_operand_width(width: LaneWidth, mask: u32, value: u32) -> LaneWidth {
    match width {
        LaneWidth::Size { .. } if mask & SIZE.to_mask() == SIZE.to_mask() => {
            LaneWidth::Fixed(u8::BITS << SIZE.read(value))
        }
        LaneWidth::FloatSize if mask & SZ.to_mask() == SZ.to_mask() => {
            LaneWidth::Fixed(u32::BITS << SZ.read(value))
        }
        other => other,
    }
}

/// Each run of free bits of the word as fields of at most four bits, each of which takes every value.
fn to_free_fields(free: u32) -> Vec<OperandType> {
    let mut fields = Vec::new();
    let mut bit = 0;
    while bit < u32::BITS {
        if free >> bit & 1 == 0 {
            bit += 1;
            continue;
        }
        let mut width = 0;
        while bit + width < u32::BITS
            && free >> (bit + width) & 1 == 1
            && width < EXHAUSTIVE_FIELD_BITS
        {
            width += 1;
        }
        fields.push(OperandType::UnsignedImmediate(Field::new(bit, width)));
        bit += width;
    }
    fields
}
