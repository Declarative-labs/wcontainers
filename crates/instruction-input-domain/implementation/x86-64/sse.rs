//! The inputs of the forms of the architecture's `sse` module, derived from each form's operation and its code's
//! operand kinds as iced-x86 lists them.
//!
//! A form's operands are the registers it names and how its bytes lie (or, with a memory operand, the memory
//! operand), the value of each register it reads, its immediate, MXCSR for the floating-point forms, and the flags,
//! which none reads. A form that reads XMM0 besides its operands names XMM0 among its registers too, so that each
//! operand meets XMM0 as itself. An operand's value goes to operand 0 for the destination, to operand 1 (or the
//! memory operand) for the source, and to XMM0 last, so a register named twice holds the last value written.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded, MemoryOperand, OpCodeOperandKind, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::float::{Arrangement, MASK_SHIFT, Precision};
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::sse::{
    Combination, GaloisStep, IGNORED_WIDTH_FORMS, SseForm, SseOperation, StringLength, StringOutput,
    VECTOR_BYTES,
};
use wcontainers_processor_architecture_x86_64::vector::{Lane, join};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, InstructionFetch, MXCSR_DEFAULT, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
};

use crate::float::MxcsrSet;
use crate::integer::{CodeLayout, Encoding, LAYOUTS_BEYOND_START, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::vector::{VectorValues, fill_lanes, generate_vectors, rotate_floats};
use crate::{
    AccessPlacement, AddressSize, CODE_ADDRESS, FlagSet, NON_CANONICAL_ADDRESS, OperandSize, OperandSizes, OperandType,
    OperandValue, Role, SegmentChoice, Splits, VECTOR, to_access_address, to_data_pages,
};

/// An operand of a form of this family.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SseOperand {
    /// The register numbers of operands 0 and 1 and how the bytes lie, with XMM0 among the pairs.
    RegistersWithXmm0,
    /// An XMM register's value: the destination's, the source's, or XMM0's as `Role::Implicit(0)`.
    Vector(Role, VectorSet),
    /// A general register's value, or memory's where the form reads its source there.
    General(Role, GeneralSet),
    Immediate(ImmediateSet),
    /// The two strings of a string compare, and for an explicit-length form RAX and RDX.
    Strings { explicit: bool },
    /// Where MASKMOVDQU's RDI points, and through which address size and segment.
    MaskedDestination,
}

impl SseOperand {
    /// Whether the operand holds data the instruction computes with, rather than where things lie.
    pub const fn is_data(self) -> bool {
        !matches!(self, Self::RegistersWithXmm0 | Self::MaskedDestination)
    }
}

/// The critical values of an XMM operand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorSet {
    /// Two values whose bytes all differ, the bytes of one with their top bits set.
    Distinct,
    /// Sign bits in lane patterns, for the masks MOVMSK and the variable blends read.
    SignBits,
    /// The low lane a conversion to an integer reads: the floating-point critical values, and the edges of the
    /// 32-bit and 64-bit ranges and the halves rounding meets.
    ToInteger(Precision),
    /// ROUND's lanes: the floating-point critical values, and the halves, the quarters and the edge of the values
    /// with a fraction.
    Round(Precision),
    /// The bytes' own edges in every rotation, or in the first alone, for MPSADBW.
    Bytes { every_rotation: bool },
    /// FIPS-197's test values, zeros, ones and digits of pi, for AES, SHA and GFNI's multiply.
    Cryptographic,
    /// Quadwords for PCLMULQDQ: zeros, one, ones, the two ends set, and digits of pi.
    CarryLess,
    /// GFNI's matrices: the identity, ones, zeros, a pattern and digits of pi.
    Matrices,
    /// A dot product's operands: the floating-point critical values (the destination's rotations a lane count
    /// apart), and in every lane the largest normal and the smallest for the destination, two, a half and the
    /// largest negated for the source, whose products and sums overflow, underflow and cancel.
    DotProduct { precision: Precision, destination: bool },
}

/// The critical values of a general register's or memory's operand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GeneralSet {
    /// Two values that differ in every byte.
    Distinct,
    /// Signed integers at the edges of both precisions' significands, of both widths, and with upper halves a
    /// 32-bit source ignores.
    Integers,
    /// MXCSR images: each field set, and reserved bits.
    MxcsrImages,
    /// CRC32's remainder so far, with an upper half the instruction clears.
    Remainders,
    /// CRC32's data: each width's edges.
    CrcData,
}

/// The critical values of an immediate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImmediateSet {
    /// Each lane of `lanes`, the lane count, and all ones, whose bits beyond the lanes the form ignores.
    Lanes(u32),
    /// Each rounding control, and two with the bits above set.
    Round,
    /// MPSADBW's blocks, and two with the bits above set.
    Blocks,
    /// AESKEYGENASSIST's round constants.
    RoundConstants,
    /// PCLMULQDQ's selections, and two with the bits between set.
    Quadwords,
    /// SHA1RNDS4's functions, and two with the bits above set.
    Functions,
    /// GFNI's affine constants.
    AffineConstants,
    /// Every string compare control below bit 7, and two with bit 7 set.
    StringControls,
    /// Dot product selections: every lane, none, one product into every lane, every product into one, and mixed.
    DotProductMasks,
}

/// A value of an operand of this family that no other operand value holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SseValue {
    Strings { first: u128, second: u128, lengths: Option<[u64; 2]> },
    MaskedDestination { placement: AccessPlacement, lower: PagePermission, upper: PagePermission, address_size: AddressSize, segment: SegmentChoice },
}

/// The register pairs of a form that reads XMM0: each operand XMM0, both, and the pairs without it.
const PAIRS_WITH_XMM0: [[usize; 2]; 5] = [[1, 0], [0, 1], [0, 0], [1, 13], [13, 4]];

/// The operand types of a form, in the order the build reads them.
pub fn list_operand_types(form: &SseForm) -> Vec<OperandType> {
    use OperandType::Sse;
    use SseOperand::{General, Immediate, Vector};
    let in_memory = form.place != OperandPlace::Register;
    let memory = OperandType::Memory { splits: Splits::Allowed, offset: false };
    let place = |with_xmm0: bool| match (in_memory, with_xmm0) {
        (true, _) => memory,
        (false, true) => Sse(SseOperand::RegistersWithXmm0),
        (false, false) => OperandType::Encoding(2),
    };
    let flags = OperandType::Flags(FlagSet::Unread);
    let immediate = |set: ImmediateSet| form.codes[0].op_code().op_kinds().contains(&OpCodeOperandKind::imm8).then_some(Sse(Immediate(set)));
    let lanes = |bytes: u32| ImmediateSet::Lanes(VECTOR_BYTES / bytes);
    let arithmetic = OperandType::Mxcsr(MxcsrSet::Arithmetic);
    let types: Vec<Option<OperandType>> = match form.operation {
        SseOperation::Extract { bytes, .. } => {
            vec![Some(place(false)), Some(Sse(Vector(Role::Source, VectorSet::Distinct))), immediate(lanes(bytes))]
        }
        SseOperation::Insert { bytes, zeroing, .. } => {
            let source = if bytes == VECTOR_BYTES { Vector(Role::Source, VectorSet::Distinct) } else { General(Role::Source, GeneralSet::Distinct) };
            vec![
                Some(place(false)),
                (!zeroing).then_some(Sse(Vector(Role::Destination, VectorSet::Distinct))),
                Some(Sse(source)),
                immediate(lanes(bytes)),
            ]
        }
        SseOperation::MoveMask(_) => vec![Some(place(false)), Some(Sse(Vector(Role::Source, VectorSet::SignBits)))],
        SseOperation::StoreGeneral => vec![Some(memory), Some(Sse(General(Role::Source, GeneralSet::Distinct)))],
        SseOperation::LoadMxcsr => vec![Some(memory), Some(Sse(General(Role::Source, GeneralSet::MxcsrImages)))],
        SseOperation::StoreMxcsr => vec![Some(memory), Some(OperandType::Mxcsr(MxcsrSet::Comparison))],
        SseOperation::Fence => vec![Some(OperandType::Encoding(0))],
        SseOperation::Prefetch => vec![Some(memory)],
        SseOperation::Combine(combination) => {
            let (destination, source, set) = to_combination_values(combination);
            vec![
                Some(place(combination.reads_xmm0())),
                combination.reads_destination().then_some(Sse(Vector(Role::Destination, destination))),
                Some(Sse(Vector(Role::Source, source))),
                combination.reads_xmm0().then_some(Sse(Vector(Role::Implicit(0), source))),
                set.and_then(immediate),
            ]
        }
        SseOperation::CompareStrings { length, output } => vec![
            Some(place(output == StringOutput::Mask)),
            Some(Sse(SseOperand::Strings { explicit: length != StringLength::Implicit })),
            immediate(ImmediateSet::StringControls),
        ],
        SseOperation::Crc32 => {
            let sizes = if form.codes.len() == 2 { OperandSizes::WithoutSixteen } else { OperandSizes::All };
            vec![
                Some(OperandType::OperandSize(sizes)),
                Some(place(false)),
                Some(Sse(General(Role::Destination, GeneralSet::Remainders))),
                Some(Sse(General(Role::Source, GeneralSet::CrcData))),
            ]
        }
        SseOperation::IntegerToFloat(_) => vec![
            Some(place(false)),
            Some(Sse(Vector(Role::Destination, VectorSet::Distinct))),
            Some(Sse(General(Role::Source, GeneralSet::Integers))),
            Some(arithmetic),
        ],
        SseOperation::FloatToInteger { precision, .. } => {
            vec![Some(place(false)), Some(Sse(Vector(Role::Source, VectorSet::ToInteger(precision)))), Some(arithmetic)]
        }
        SseOperation::DotProduct(precision) => vec![
            Some(place(false)),
            Some(Sse(Vector(Role::Destination, VectorSet::DotProduct { precision, destination: true }))),
            Some(Sse(Vector(Role::Source, VectorSet::DotProduct { precision, destination: false }))),
            immediate(ImmediateSet::DotProductMasks),
            Some(arithmetic),
        ],
        SseOperation::MaskedStore => vec![
            Some(OperandType::Encoding(2)),
            Some(Sse(SseOperand::MaskedDestination)),
            Some(Sse(Vector(Role::Source, VectorSet::Distinct))),
            Some(Sse(Vector(Role::Implicit(MASK_OPERAND), VectorSet::SignBits))),
        ],
        SseOperation::Round { precision, arrangement } => vec![
            Some(place(false)),
            (arrangement == Arrangement::Scalar).then_some(Sse(Vector(Role::Destination, VectorSet::Distinct))),
            Some(Sse(Vector(Role::Source, VectorSet::Round(precision)))),
            immediate(ImmediateSet::Round),
            Some(arithmetic),
        ],
    };
    types.into_iter().flatten().chain([flags]).collect()
}

/// The value sets of a combination's destination and source, and of its immediate if it reads one.
const fn to_combination_values(combination: Combination) -> (VectorSet, VectorSet, Option<ImmediateSet>) {
    match combination {
        Combination::VariableBlend(_) => (VectorSet::Distinct, VectorSet::SignBits, None),
        Combination::MultipleSumAbsoluteDifferences => {
            (VectorSet::Bytes { every_rotation: false }, VectorSet::Bytes { every_rotation: true }, Some(ImmediateSet::Blocks))
        }
        Combination::Aes(_) => (VectorSet::Cryptographic, VectorSet::Cryptographic, Some(ImmediateSet::RoundConstants)),
        Combination::CarryLessMultiply => (VectorSet::CarryLess, VectorSet::CarryLess, Some(ImmediateSet::Quadwords)),
        Combination::Sha(_) => (VectorSet::Cryptographic, VectorSet::Cryptographic, Some(ImmediateSet::Functions)),
        Combination::Galois(GaloisStep::Multiply) => (VectorSet::Cryptographic, VectorSet::Cryptographic, None),
        Combination::Galois(_) => (VectorSet::Cryptographic, VectorSet::Matrices, Some(ImmediateSet::AffineConstants)),
    }
}

/// The critical values of an operand of this family.
pub fn generate_values(operand: SseOperand) -> Vec<OperandValue> {
    match operand {
        SseOperand::RegistersWithXmm0 => {
            let at_start = PAIRS_WITH_XMM0.map(|registers| Encoding { registers, layout: CodeLayout::PageStart });
            let laid_out = LAYOUTS_BEYOND_START.map(|layout| Encoding { registers: PAIRS_WITH_XMM0[0], layout });
            at_start.into_iter().chain(laid_out).map(OperandValue::Encoding).collect()
        }
        SseOperand::Vector(_, set) => generate_vectors_of(set).into_iter().map(OperandValue::Vector).collect(),
        SseOperand::General(_, set) => generate_generals(set).into_iter().map(OperandValue::Bits).collect(),
        SseOperand::Immediate(set) => generate_immediates(set).into_iter().map(|value| OperandValue::Immediate(i64::from(value))).collect(),
        SseOperand::Strings { explicit } => generate_strings(explicit).into_iter().map(OperandValue::Sse).collect(),
        SseOperand::MaskedDestination => generate_masked_destinations().into_iter().map(OperandValue::Sse).collect(),
    }
}

/// 16 bytes from `first` up, one apart.
const fn ascend(first: u8) -> u128 {
    let mut value = 0u128;
    let mut index = 0;
    while index < VECTOR_BYTES {
        value |= ((first + index as u8) as u128) << (index * u8::BITS);
        index += 1;
    }
    value
}

/// FIPS-197's example plaintext and cipher key (appendix C.1), and the fraction of pi in hexadecimal.
const FIPS_PLAINTEXT: u128 = u128::from_be_bytes([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
const FIPS_KEY: u128 = ascend(0);
const PI_FRACTION: u128 = 0x243F_6A88_85A3_08D3_1319_8A2E_0370_7344;
/// The 8×8 identity matrix as GFNI reads a quadword: row `7 - i` in byte `i`.
const IDENTITY_MATRIX: u64 = 0x0102_0408_1020_4080;
/// A byte pattern whose nibbles and halves differ.
const PATTERN: u64 = 0x5AA5_C33C_0FF0_9669;

fn generate_vectors_of(set: VectorSet) -> Vec<u128> {
    let quadwords = |low: u64, high: u64| join(Lane::Quadword, [low, high].into_iter());
    match set {
        VectorSet::Distinct => vec![ascend(0x80), ascend(0x70)],
        VectorSet::SignBits => {
            const MASKS: [u16; 8] = [0x0000, 0xFFFF, 0x5555, 0xAAAA, 0x1234, 0x8001, 0x0808, 0x8080];
            MASKS
                .iter()
                .map(|&mask| {
                    let bytes: [u8; 16] = array::from_fn(|index| if mask >> index & 1 != 0 { 0x80 | index as u8 } else { 0x7F - index as u8 });
                    u128::from_le_bytes(bytes)
                })
                .collect()
        }
        VectorSet::ToInteger(precision) => {
            let lane = crate::vector::to_float_lane(precision);
            let edges = [u32::BITS, u64::BITS].into_iter().flat_map(|width| list_integer_edges(precision, width)).chain(list_halves(precision));
            rotate_floats(precision, true).into_iter().chain(edges.map(|edge| fill_lanes(lane, edge))).collect()
        }
        VectorSet::Round(precision) => {
            let lane = crate::vector::to_float_lane(precision);
            let digits = precision.to_significand_bits() as i32;
            let largest_with_fraction = encode_float(precision, false, (1 << digits) - 1, -1);
            let edges = list_halves(precision).into_iter().chain([
                encode_float(precision, false, 1, -2),
                encode_float(precision, false, 3, -2),
                encode_float(precision, true, 3, -2),
                largest_with_fraction,
                largest_with_fraction | (1 << (precision.to_bits() - 1)),
                encode_float(precision, false, 1, digits - 1),
                encode_float(precision, false, (1 << (digits - 1)) + 1, 1 - digits),
                encode_float(precision, false, (1 << digits) - 1, -digits - 1),
            ]);
            rotate_floats(precision, true).into_iter().chain(edges.map(|edge| fill_lanes(lane, edge))).collect()
        }
        VectorSet::Bytes { every_rotation } => generate_vectors(VectorValues::Lanes { lane: Lane::Byte, every_rotation })
            .into_iter()
            .map(|value| match value {
                OperandValue::Vector(vector) => vector,
                other => panic!("{other:?} is no vector"),
            })
            .collect(),
        VectorSet::Cryptographic => vec![0, u128::MAX, FIPS_PLAINTEXT, FIPS_KEY, PI_FRACTION],
        VectorSet::CarryLess => vec![0, 1, u128::MAX, quadwords(1 | 1 << 63, 1 | 1 << 63), PI_FRACTION],
        VectorSet::Matrices => vec![quadwords(IDENTITY_MATRIX, IDENTITY_MATRIX), u128::MAX, 0, quadwords(PATTERN, !PATTERN), PI_FRACTION],
        VectorSet::DotProduct { precision, destination } => {
            let lane = crate::vector::to_float_lane(precision);
            let digits = precision.to_significand_bits() as i32;
            let bias = (1 << (precision.to_bits() as i32 - digits - 1)) - 1;
            let largest = encode_float(precision, false, (1 << digits) - 1, bias + 1 - digits);
            let edges = if destination {
                vec![largest, encode_float(precision, false, 1, 1 - bias)]
            } else {
                vec![encode_float(precision, false, 1, 1), encode_float(precision, false, 1, -1), largest | (1 << (precision.to_bits() - 1))]
            };
            rotate_floats(precision, !destination).into_iter().chain(edges.into_iter().map(|edge| fill_lanes(lane, edge))).collect()
        }
    }
}

/// The bits of `significand × 2^exponent` in the precision, the significand at most as wide as the precision's.
fn encode_float(precision: Precision, negative: bool, significand: u64, exponent: i32) -> u64 {
    let digits = precision.to_significand_bits() as i32;
    let sign = if negative { 1 << (precision.to_bits() - 1) } else { 0 };
    if significand == 0 {
        return sign;
    }
    let width = (u64::BITS - significand.leading_zeros()) as i32;
    let normalized = significand << (digits - width);
    let top = exponent + width - 1;
    let bias = (1 << (precision.to_bits() as i32 - digits - 1)) - 1;
    let fraction = normalized & ((1 << (digits - 1)) - 1);
    sign | (((top + bias) as u64) << (digits - 1)) | fraction
}

/// The values at an integer width's edges: each end of the range, the value just inside and just outside each, and
/// a half below the top where the precision holds it.
fn list_integer_edges(precision: Precision, width: u32) -> Vec<u64> {
    let digits = precision.to_significand_bits() as i32;
    let top = width as i32 - 1;
    let mut edges = vec![
        encode_float(precision, false, 1, top),
        encode_float(precision, true, 1, top),
        encode_float(precision, false, (1 << digits) - 1, top - digits),
        encode_float(precision, true, (1 << (digits - 1)) + 1, top - digits + 1),
    ];
    if digits > top + 1 {
        edges.push(encode_float(precision, false, (1 << (top + 1)) - 1, -1));
        edges.push(encode_float(precision, true, (1 << top) + 1, 0));
    }
    edges
}

/// One half, one and a half, two and a half, and their negatives.
fn list_halves(precision: Precision) -> Vec<u64> {
    [1, 3, 5].into_iter().flat_map(|halves| [false, true].map(|negative| encode_float(precision, negative, halves, -1))).collect()
}

fn generate_generals(set: GeneralSet) -> Vec<u64> {
    const ABOVE_LOW_HALF: u64 = 0xA5A5_A5A5_0000_0000;
    let single_edge = 1u64 << f32::MANTISSA_DIGITS;
    let double_edge = 1u64 << f64::MANTISSA_DIGITS;
    match set {
        GeneralSet::Distinct => vec![0x0123_4567_89AB_CDEF, 0xFEDC_BA98_7654_3210],
        GeneralSet::Integers => vec![
            0,
            1,
            u64::MAX,
            i32::MAX as u64,
            i32::MIN as u64,
            single_edge + 1,
            single_edge + 2,
            single_edge + 3,
            (single_edge + 1).wrapping_neg(),
            ABOVE_LOW_HALF | (single_edge + 1),
            double_edge + 1,
            (double_edge + 3).wrapping_neg(),
            i64::MAX as u64,
            i64::MIN as u64,
        ],
        GeneralSet::MxcsrImages => {
            const FLAGS: u32 = (1 << MASK_SHIFT) - 1;
            let reserved = 1u32 << u16::BITS;
            [
                MXCSR_DEFAULT,
                0,
                u32::from(u16::MAX),
                MXCSR_DEFAULT | FLAGS,
                FLAGS,
                MXCSR_DEFAULT | reserved,
                MXCSR_DEFAULT | 1 << (u32::BITS - 1),
                u32::MAX,
            ]
            .map(|image| ABOVE_LOW_HALF | u64::from(image))
            .to_vec()
        }
        GeneralSet::Remainders => vec![0, 1, 0x8000_0000, u64::MAX, ABOVE_LOW_HALF | 0x1234_5678],
        GeneralSet::CrcData => vec![0, 1, 0x80, 0xFF, 0x8000, 0x8000_0000, 0x0123_4567_89AB_CDEF, u64::MAX],
    }
}

fn generate_immediates(set: ImmediateSet) -> Vec<u8> {
    match set {
        ImmediateSet::Lanes(lanes) => (0..=lanes as u8).chain([u8::MAX]).collect(),
        ImmediateSet::Round => (0..16).chain([0xF4, 0xFB]).collect(),
        ImmediateSet::Blocks => (0..8).chain([0xF8, 0xFD]).collect(),
        ImmediateSet::RoundConstants => vec![0, 1, 0x1B, 0x36, 0x80, u8::MAX],
        ImmediateSet::Quadwords => vec![0x00, 0x01, 0x10, 0x11, 0xEE, u8::MAX],
        ImmediateSet::Functions => vec![0, 1, 2, 3, 0xFC, u8::MAX],
        ImmediateSet::AffineConstants => vec![0, 1, 0x63, 0x80, u8::MAX, 0x5A],
        ImmediateSet::StringControls => (0..0x80).chain([0x8C, u8::MAX]).collect(),
        ImmediateSet::DotProductMasks => vec![u8::MAX, 0x00, 0x1F, 0xF1, 0x31, 0xC3, 0x3C, 0x8E, 0x75, 0x23],
    }
}

/// The strings of a string compare, first then second: needles, haystacks, ranges, signed bytes and nulls at each
/// place, read as bytes or as words.
fn list_string_pairs() -> Vec<(u128, u128)> {
    let text = |bytes: &[u8]| {
        let mut all = [0u8; 16];
        all[..bytes.len()].copy_from_slice(bytes);
        u128::from_le_bytes(all)
    };
    let alphabet = text(b"abcdefghijklmnop");
    let haystack = text(b"xxabcxxxabcdxxab");
    let needle = text(b"abc");
    let ranges = text(b"az09");
    let signed = text(&[0x80, 0x7F, 0xFF, 0x01, 0x41, 0x00, 0x80, 0x80, 0xC0, 0x3F]);
    let mixed = text(&[0x41, 0x80, 0xFF, 0x7F, 0x00, 0x41, 0x61, 0x7A, 0x30, 0x39, 0x80, 0xFF, 0x00, 0x00, 0x61, 0x62]);
    vec![
        (alphabet, alphabet),
        (needle, haystack),
        (ranges, mixed),
        (signed, mixed),
        (mixed, signed),
        (alphabet, 0),
        (0, alphabet),
        (needle, needle),
        (haystack, alphabet),
    ]
}

/// Each pair of strings, and for an explicit-length form each with lengths that cover all, part and none of them,
/// then the lengths' own edges with the first pair: past the elements, negative, the 32-bit minimum, and upper
/// halves that only the forms with REX.W read.
fn generate_strings(explicit: bool) -> Vec<SseValue> {
    let pairs = list_string_pairs();
    if !explicit {
        return pairs.into_iter().map(|(first, second)| SseValue::Strings { first, second, lengths: None }).collect();
    }
    let negative = |value: i64| value as u64;
    let covering: [[u64; 2]; 3] = [[16, 16], [5, 11], [0, 3]];
    let edges: [[u64; 2]; 9] = [
        [17, 8],
        [9, 16],
        [negative(-4), 7],
        [7, negative(-17)],
        [i32::MIN as u64, 1],
        [0xFFFF_FFFF_0000_0005, 0x8000_0000_0000_0010],
        [0x0000_0001_0000_0002, negative(-2)],
        [1, 0],
        [8, 9],
    ];
    let (first, second) = pairs[1];
    pairs
        .iter()
        .flat_map(|&(first, second)| covering.map(|lengths| SseValue::Strings { first, second, lengths: Some(lengths) }))
        .chain(edges.map(|lengths| SseValue::Strings { first, second, lengths: Some(lengths) }))
        .collect()
}

/// The code a form takes at the operand size: its first, or CRC32's of that size.
pub fn to_code(form: &SseForm, size: Option<OperandSize>) -> Code {
    if form.operation != SseOperation::Crc32 {
        return form.codes[0];
    }
    match (form.codes, size) {
        ([bits32, _], Some(OperandSize::Bits32)) => *bits32,
        ([_, bits64], Some(OperandSize::Bits64)) => *bits64,
        ([bits16, _, _], Some(OperandSize::Bits16)) => *bits16,
        ([_, bits32, _], Some(OperandSize::Bits32)) => *bits32,
        ([_, _, bits64], Some(OperandSize::Bits64)) => *bits64,
        (codes, size) => panic!("{:?} has no code of {size:?} among {codes:?}", form.instruction),
    }
}

/// The bytes the form's memory operand reaches at the operand size.
pub fn to_access_bytes(form: &SseForm, size: Option<OperandSize>) -> u64 {
    match form.operation {
        SseOperation::Crc32 => match to_code(form, size) {
            Code::Crc32_r32_rm8 | Code::Crc32_r64_rm8 => 1,
            Code::Crc32_r32_rm16 => 2,
            Code::Crc32_r32_rm32 => 4,
            _ => 8,
        },
        _ => form.memory_bytes(),
    }
}

/// The 8-bit registers 4 to 7 name without a REX prefix.
const HIGH_BYTES: [Register; 4] = [Register::AH, Register::CH, Register::DH, Register::BH];

const GENERAL_8: [Register; 16] = [
    Register::AL, Register::CL, Register::DL, Register::BL, Register::SPL, Register::BPL,
    Register::SIL, Register::DIL, Register::R8L, Register::R9L, Register::R10L, Register::R11L,
    Register::R12L, Register::R13L, Register::R14L, Register::R15L,
];

/// The general register of the number at the width, AH to BH for 4 to 7 at one byte without REX.
fn to_general(number: usize, bytes: usize, rex: bool) -> Register {
    match bytes {
        1 if !rex && (4..8).contains(&number) => HIGH_BYTES[number - 4],
        1 => GENERAL_8[number],
        2 => crate::GENERAL_16[number],
        4 => crate::GENERAL_32[number],
        _ => crate::GENERAL_64[number],
    }
}

/// What an operand of the code is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Vector,
    General(usize),
    Memory,
    Immediate,
}

fn to_slot(kind: OpCodeOperandKind, in_memory: bool) -> Slot {
    use OpCodeOperandKind as Kind;
    match kind {
        Kind::xmm_reg | Kind::xmm_rm => Slot::Vector,
        Kind::xmm_or_mem if in_memory => Slot::Memory,
        Kind::xmm_or_mem => Slot::Vector,
        Kind::r32_reg | Kind::r32_rm => Slot::General(4),
        Kind::r64_reg | Kind::r64_rm => Slot::General(8),
        Kind::r8_or_mem | Kind::r16_or_mem | Kind::r32_or_mem | Kind::r64_or_mem if in_memory => Slot::Memory,
        Kind::r8_or_mem => Slot::General(1),
        Kind::r16_or_mem => Slot::General(2),
        Kind::r32_or_mem => Slot::General(4),
        Kind::r64_or_mem => Slot::General(8),
        Kind::mem => Slot::Memory,
        Kind::imm8 => Slot::Immediate,
        other => panic!("no form of this family has an operand of {other:?}"),
    }
}

/// A value an operand role takes.
#[derive(Clone, Copy, Debug)]
enum Value {
    Vector(u128),
    Bits(u64),
}

impl Value {
    const fn to_vector(self) -> u128 {
        match self {
            Self::Vector(value) => value,
            Self::Bits(value) => value as u128,
        }
    }
}

/// MASKMOVDQU's mask register, its operand after the data register.
const MASK_OPERAND: usize = 2;
/// An FS base MASKMOVDQU's RDI makes up for.
const MASKED_FS_BASE: u64 = 0x0000_7000_0000_0000;
/// What RDI holds above EDI when the 0x67 prefix makes MASKMOVDQU read EDI.
const ABOVE_ADDRESS_HALF: u64 = 0xA5A5_A5A5_0000_0000;

/// Every place MASKMOVDQU's 16 bytes can lie around the data pages' boundary with each permission of each page, a
/// non-canonical address, and at the lower page's start through EDI and through FS.
fn generate_masked_destinations() -> Vec<SseValue> {
    let mapped = |data_access| PagePermission::Mapped { data_access, instruction_fetch: InstructionFetch::Forbidden };
    let permissions = [PagePermission::Unmapped, mapped(DataAccess::ReadOnly), mapped(DataAccess::ReadWrite)];
    let at = |placement, lower, upper, address_size, segment| SseValue::MaskedDestination { placement, lower, upper, address_size, segment };
    let placements = [
        AccessPlacement::LowerPageStart,
        AccessPlacement::Misaligned,
        AccessPlacement::EndsAtBoundary,
        AccessPlacement::StraddlesBoundary,
        AccessPlacement::UpperPageStart,
    ];
    let mut all = Vec::new();
    for placement in placements {
        for lower in permissions {
            for upper in permissions {
                all.push(at(placement, lower, upper, AddressSize::Bits64, SegmentChoice::Default));
            }
        }
    }
    let writable = permissions[2];
    all.extend([
        at(AccessPlacement::NonCanonical, writable, writable, AddressSize::Bits64, SegmentChoice::Default),
        at(AccessPlacement::LowerPageStart, writable, writable, AddressSize::Bits32, SegmentChoice::Default),
        at(AccessPlacement::LowerPageStart, writable, writable, AddressSize::Bits64, SegmentChoice::Fs),
    ]);
    all
}

/// Builds MASKMOVDQU's state: the data register and the mask register, the mask written last, RDI at the
/// destination, the data pages' permissions.
fn build_masked_store_state(form: &SseForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut registers, mut layout, mut data, mut mask, mut destination) = ([0, 0], CodeLayout::PageStart, 0, 0, None);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => (registers, layout) = (encoding.registers, encoding.layout),
            (OperandType::Sse(SseOperand::MaskedDestination), OperandValue::Sse(value)) => destination = Some(value),
            (OperandType::Sse(SseOperand::Vector(Role::Source, _)), OperandValue::Vector(value)) => data = value,
            (OperandType::Sse(SseOperand::Vector(_, _)), OperandValue::Vector(value)) => mask = value,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    processor.vector_registers[registers[0]] = data;
    processor.vector_registers[registers[1]] = mask;
    let Some(SseValue::MaskedDestination { placement, lower, upper, address_size, segment }) = destination else {
        panic!("{:?} has a destination", form.instruction);
    };
    let mut memory_mutations: Vec<MemoryDelta<'static, X86_64>> = to_data_pages(lower, upper).into();
    let address = match placement {
        AccessPlacement::NonCanonical => NON_CANONICAL_ADDRESS,
        placement => to_access_address(placement, u64::from(VECTOR_BYTES)),
    };
    let (segment_prefix, base) = match segment {
        SegmentChoice::Fs => (Register::FS, MASKED_FS_BASE),
        _ => (Register::None, 0),
    };
    processor.fs_base = base;
    let offset = address.wrapping_sub(base);
    let (bits, rdi) = match address_size {
        AddressSize::Bits64 => (u64::BITS, offset),
        AddressSize::Bits32 => (u32::BITS, ABOVE_ADDRESS_HALF | offset),
    };
    processor.general_purpose_registers[MASKED_DESTINATION_REGISTER] = rdi;
    let encoded = Encoded::with_maskmovdqu(bits, VECTOR[registers[0]], VECTOR[registers[1]], segment_prefix)
        .unwrap_or_else(|error| panic!("MASKMOVDQU takes its operands: {error}"));
    let mut padding_operand = MemoryOperand::with_base(Register::RDI);
    padding_operand.segment_prefix = segment_prefix;
    let (code_page, start, bytes) = lay_out(layout, &encode_at_code_address(&encoded), to_padding(Some(padding_operand)));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}

/// RDI's number among the general registers.
const MASKED_DESTINATION_REGISTER: usize = 7;

/// Builds the state of a form from one value of each of its operand types.
pub fn build_state(form: &SseForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    if form.operation == SseOperation::MaskedStore {
        return build_masked_store_state(form, operands);
    }
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut registers, mut layout, mut access, mut size, mut immediate) = ([0, 0], CodeLayout::PageStart, None, None, None);
    let mut values: Vec<(Role, Value)> = Vec::new();
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::Encoding(_) | OperandType::Sse(SseOperand::RegistersWithXmm0), OperandValue::Encoding(encoding)) => {
                registers = encoding.registers;
                layout = encoding.layout;
            }
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => {
                layout = chosen.layout;
                access = Some(chosen);
            }
            (OperandType::OperandSize(_), OperandValue::OperandSize(chosen)) => size = Some(chosen),
            (OperandType::Sse(SseOperand::Vector(role, _)), OperandValue::Vector(value)) => values.push((*role, Value::Vector(value))),
            (OperandType::Sse(SseOperand::General(role, _)), OperandValue::Bits(value)) => values.push((*role, Value::Bits(value))),
            (OperandType::Sse(SseOperand::Immediate(_)), OperandValue::Immediate(value)) => immediate = Some(value as u32),
            (OperandType::Sse(SseOperand::Strings { .. }), OperandValue::Sse(SseValue::Strings { first, second, lengths })) => {
                values.push((Role::Destination, Value::Vector(first)));
                values.push((Role::Source, Value::Vector(second)));
                if let Some([first_length, second_length]) = lengths {
                    processor.general_purpose_registers[0] = first_length;
                    processor.general_purpose_registers[2] = second_length;
                }
            }
            (OperandType::Mxcsr(_), OperandValue::Mxcsr(mxcsr)) => processor.mxcsr = mxcsr,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let code = to_code(form, size);
    let in_memory = access.is_some();
    let slots: Vec<Slot> = code.op_code().op_kinds().iter().map(|&kind| to_slot(kind, in_memory)).collect();
    // The register operands' numbers in order: the encoding's pair, or the access's register in ModRM.reg.
    let mut numbers = match access {
        Some(chosen) => vec![chosen.register],
        None => registers.to_vec(),
    }
    .into_iter();
    let named: Vec<Option<usize>> = slots
        .iter()
        .map(|slot| matches!(slot, Slot::Vector | Slot::General(_)).then(|| numbers.next().expect("a number for each register")))
        .collect();
    let rex = named.iter().flatten().any(|&number| number >= 8) || slots.contains(&Slot::General(8));
    let mut memory_mutations = Vec::new();
    // Each value at its operand, the destination's first and XMM0's last; the source's at operand 1, or at the
    // memory operand of a form with none.
    let source_slot = if slots.len() > 1 && slots[1] != Slot::Immediate { 1 } else { 0 };
    let mut memory_value: Option<Value> = None;
    for role in [Role::Destination, Role::Source, Role::Implicit(0)] {
        for (_, value) in values.iter().filter(|(chosen, _)| *chosen == role) {
            let index = match role {
                Role::Destination => 0,
                Role::Implicit(_) => {
                    processor.vector_registers[0] = value.to_vector();
                    continue;
                }
                _ => source_slot,
            };
            match (slots[index], named[index]) {
                (Slot::Vector, Some(number)) => processor.vector_registers[number] = value.to_vector(),
                (Slot::General(bytes), Some(number)) => {
                    let Value::Bits(bits) = *value else { panic!("{value:?} is no general register's value") };
                    let full = &mut processor.general_purpose_registers[number];
                    *full = if HIGH_BYTES.contains(&to_general(number, bytes, rex)) { bits.rotate_left(u8::BITS) } else { bits };
                }
                (Slot::Memory, _) => memory_value = Some(*value),
                (slot, _) => panic!("{slot:?} takes no value"),
            }
        }
    }
    let memory_bytes = to_access_bytes(form, size);
    let memory_operand: Option<MemoryOperand> = access.map(|chosen| {
        let (operand, address) = place_memory_operand(chosen, memory_bytes, &mut processor, &mut memory_mutations);
        if let Some(value) = memory_value {
            let bytes: Vec<u8> = (0..memory_bytes)
                .map(|offset| address.wrapping_add(offset))
                .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
                .zip(value.to_vector().to_le_bytes())
                .map(|(_, byte)| byte)
                .collect();
            if !bytes.is_empty() {
                memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(bytes) });
            }
        }
        operand
    });
    let encoded = encode(code, &slots, &named, rex, memory_operand, immediate);
    let mut bytes = encode_at_code_address(&encoded);
    if IGNORED_WIDTH_FORMS.contains(&form.instruction) {
        set_rex_width(&mut bytes);
    }
    let (code_page, start, bytes) = lay_out(layout, &bytes, to_padding(memory_operand));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}

/// The instruction with its operands in the code's order.
fn encode(code: Code, slots: &[Slot], named: &[Option<usize>], rex: bool, memory: Option<MemoryOperand>, immediate: Option<u32>) -> Encoded {
    #[derive(Clone, Copy)]
    enum Operand {
        Register(Register),
        Memory(MemoryOperand),
        Immediate(u32),
    }
    let operands: Vec<Operand> = slots
        .iter()
        .zip(named)
        .map(|(slot, number)| match (*slot, *number) {
            (Slot::Vector, Some(number)) => Operand::Register(VECTOR[number]),
            (Slot::General(bytes), Some(number)) => Operand::Register(to_general(number, bytes, rex)),
            (Slot::Memory, _) => Operand::Memory(memory.expect("a memory operand")),
            (Slot::Immediate, _) => Operand::Immediate(immediate.expect("an immediate")),
            (slot, number) => panic!("{slot:?} has no register for {number:?}"),
        })
        .collect();
    let to_error = |error| panic!("{code:?} takes its operands: {error}");
    let with = |instruction: Result<Encoded, iced_x86::IcedError>| instruction.unwrap_or_else(to_error);
    use Operand::{Immediate, Memory, Register as Named};
    match operands.as_slice() {
        [] => Encoded::with(code),
        [Named(first)] => with(Encoded::with1(code, *first)),
        [Memory(first)] => with(Encoded::with1(code, *first)),
        [Named(first), Named(second)] => with(Encoded::with2(code, *first, *second)),
        [Named(first), Memory(second)] => with(Encoded::with2(code, *first, *second)),
        [Memory(first), Named(second)] => with(Encoded::with2(code, *first, *second)),
        [Named(first), Named(second), Immediate(third)] => with(Encoded::with3(code, *first, *second, *third)),
        [Named(first), Memory(second), Immediate(third)] => with(Encoded::with3(code, *first, *second, *third)),
        [Memory(first), Named(second), Immediate(third)] => with(Encoded::with3(code, *first, *second, *third)),
        _ => panic!("{code:?} has operands no form of this family has"),
    }
}

/// Sets REX.W on the encoded instruction: in its REX prefix, or in one put right before its opcode.
fn set_rex_width(bytes: &mut Vec<u8>) {
    const LEGACY_PREFIXES: [u8; 11] = [0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65, 0x66, 0x67, 0xF0, 0xF2, 0xF3];
    const REX: u8 = 0x40;
    const REX_WIDTH: u8 = 0x08;
    let opcode = bytes.iter().take_while(|byte| LEGACY_PREFIXES.contains(byte)).count();
    match bytes.get(opcode) {
        Some(byte) if byte & 0xF0 == REX => bytes[opcode] |= REX_WIDTH,
        _ => bytes.insert(opcode, REX | REX_WIDTH),
    }
}
