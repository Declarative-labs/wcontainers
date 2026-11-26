//! The inputs of the AVX family's forms (the architecture's `avx` module): one skeleton derived from each form's
//! operation and the operand kinds of its iced-x86 code.
//!
//! A form's operands are how its registers and bytes lie (or its memory operand), the value of each source in
//! Intel's order, its immediate, MXCSR for a form that computes under it, and the flags. The register form names
//! ModRM.reg and ModRM.rm as the encoding's pair gives them and takes VEX.vvvv from that pair, so that the four
//! pairs meet the pairings that decide which operand a register named twice holds: VEX.vvvv distinct from both,
//! equal to ModRM.rm, equal to ModRM.reg, and all three one register. A 256-bit source's upper half is its lower
//! half with the lanes rotated by one, as the VEX skeleton of [`crate::vector`] gives it.
//!
//! The compares, FMA, rounding, the conversions to integers and half precision take their sources, immediate and
//! MXCSR as one value: every combination of their lane values under the default MXCSR (the predicates that
//! reveal each pair's order), and the pairs or triples that reach each exception, each in every lane, under every
//! MXCSR value and every immediate that changes what they compute.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded, MemoryOperand, OpCodeOperandKind as Kind, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::avx::{AvxForm, AvxOperation};
use wcontainers_processor_architecture_x86_64::float::{
    Arrangement, Conversion, DENORMALS_ARE_ZEROS, FLUSH_TO_ZERO, FloatOperation, MASK_SHIFT, Precision, ROUNDING_SHIFT,
};
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::vector::{Lane, join};
use wcontainers_processor_architecture_x86_64::{DataAccess, Instruction, InstructionFetch, MXCSR_DEFAULT, PagePermission, Processor, X86_64, ZERO_PROCESSOR};

use crate::float::{FloatOperands, MxcsrSet};
use crate::integer::{CodeLayout, ImmediateClass, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::vector::{VectorValues, fill_lanes, rotate_floats, to_float_lane};
use crate::{CODE_ADDRESS, FlagSet, OperandType, OperandValue, Role, Splits, ValueSet, VECTOR, VECTOR_256};

const GENERAL_32: [Register; 16] = [
    Register::EAX, Register::ECX, Register::EDX, Register::EBX, Register::ESP, Register::EBP, Register::ESI,
    Register::EDI, Register::R8D, Register::R9D, Register::R10D, Register::R11D, Register::R12D, Register::R13D,
    Register::R14D, Register::R15D,
];
const GENERAL_64: [Register; 16] = [
    Register::RAX, Register::RCX, Register::RDX, Register::RBX, Register::RSP, Register::RBP, Register::RSI,
    Register::RDI, Register::R8, Register::R9, Register::R10, Register::R11, Register::R12, Register::R13,
    Register::R14, Register::R15,
];
/// Every exception flag of MXCSR.
const EXCEPTION_FLAGS: u32 = (1 << MASK_SHIFT) - 1 - DENORMALS_ARE_ZEROS;

/// The sources, immediate and MXCSR the forms of [`AvxDataSet`] take together.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AvxData {
    pub vectors: [u128; 3],
    pub mxcsr: u32,
    pub immediate: u8,
}

/// What an [`AvxData`] operand's critical values are made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AvxDataSet {
    /// VCMPPS to VCMPSD: two sources, the predicate and MXCSR.
    Compare { precision: Precision, scalar: bool },
    /// The FMA forms: three sources and MXCSR.
    Fused { precision: Precision, scalar: bool },
    /// VROUNDPS to VROUNDSD: the source (after the first source, whose other lanes a scalar form keeps), the
    /// immediate and MXCSR.
    Round { precision: Precision, scalar: bool },
    /// VCVTSS2SI to VCVTTSD2SI: the source and MXCSR.
    ToInteger { precision: Precision, bits: u32 },
    /// VCVTPH2PS: the halves and MXCSR.
    HalfToSingle,
    /// VCVTPS2PH: the singles, the immediate and MXCSR.
    SingleToHalf,
    /// The masked loads and stores: the mask and the value.
    Mask(Lane),
    /// VDPPS, VDPPD: two sources, the immediate and MXCSR.
    Dot(Precision),
    /// VPCMPESTRI to VPCMPISTRM: two strings, the lengths in RAX and RDX (the third vector's quadwords) and the
    /// immediate.
    Strings { explicit: bool, quadword_lengths: bool },
    /// The gathers: the destination, the indices (the scale in the immediate) and the mask.
    Gather { data: Lane, index: Lane },
}

impl AvxDataSet {
    /// How many sources a value holds.
    const fn count(self) -> usize {
        match self {
            Self::Compare { .. } | Self::Mask(_) | Self::Dot(_) => 2,
            Self::Strings { .. } | Self::Gather { .. } => 3,
            Self::Fused { .. } => 3,
            Self::Round { scalar, .. } => 1 + scalar as usize,
            Self::ToInteger { .. } | Self::HalfToSingle | Self::SingleToHalf => 1,
        }
    }
}

/// Whether the operand kind names a general register, or memory read as one.
const fn is_general(kind: Kind) -> bool {
    matches!(kind, Kind::r32_reg | Kind::r64_reg | Kind::r32_or_mem | Kind::r64_or_mem | Kind::r32_rm | Kind::r64_rm)
}

/// The operand kinds of the form's code, the immediate left out.
fn list_kinds(form: &AvxForm) -> Vec<Kind> {
    form.code.op_code().op_kinds().iter().copied().filter(|kind| *kind != Kind::imm8).collect()
}

/// The kinds of the operands the form reads, in Intel's order.
fn list_source_kinds(form: &AvxForm) -> Vec<Kind> {
    let skip = if form.operation.reads_first_operand() { 0 } else { 1 };
    list_kinds(form).into_iter().skip(skip).collect()
}

/// The width of the lanes the form's upper halves rotate by.
fn to_lane_bits(form: &AvxForm) -> u32 {
    match form.operation {
        AvxOperation::Halves { operation, .. }
        | AvxOperation::ShiftByImmediate(operation)
        | AvxOperation::ShiftByCount(operation)
        | AvxOperation::Extend(operation) => operation.to_lane().to_bits(),
        AvxOperation::ShiftEach { lane, .. }
        | AvxOperation::TestSigns(lane)
        | AvxOperation::BlendVariable(lane)
        | AvxOperation::Broadcast(lane)
        | AvxOperation::ExtractLane(lane)
        | AvxOperation::InsertLane(lane)
        | AvxOperation::Signs(lane)
        | AvxOperation::PermuteVariable(lane)
        | AvxOperation::MaskedLoad(lane)
        | AvxOperation::MaskedStore(lane) => lane.to_bits(),
        AvxOperation::Float { precision, .. }
        | AvxOperation::Compare { precision, .. }
        | AvxOperation::RoundToIntegral { precision, .. }
        | AvxOperation::Fused { precision, .. }
        | AvxOperation::DotProduct(precision) => precision.to_bits(),
        AvxOperation::ConvertWidening(Conversion::IntegerToFloat) | AvxOperation::PermuteDoublewords => u32::BITS,
        AvxOperation::HalfToSingle | AvxOperation::SingleToHalf => u32::BITS,
        AvxOperation::MultipleSumsOfDifferences | AvxOperation::GaloisMultiply | AvxOperation::GaloisAffine { .. } => u8::BITS,
        _ => u64::BITS,
    }
}

/// The operand types of an AVX form, in the order the build reads them.
pub fn list_operand_types(form: &AvxForm) -> Vec<OperandType> {
    let place = match (form.place, form.operation) {
        (_, AvxOperation::ZeroUpper | AvxOperation::ZeroAll) => OperandType::Encoding(0),
        (_, AvxOperation::Gather { .. }) => OperandType::Encoding(2),
        (OperandPlace::Register, _) => OperandType::Encoding(2),
        _ => OperandType::Memory { splits: Splits::Allowed, offset: false },
    };
    let mut types = vec![place];
    let sources = list_source_kinds(form);
    let lanes = |lane: Lane, every_rotation: bool| VectorValues::Lanes { lane, every_rotation };
    let vector = |role: Role, values: VectorValues| OperandType::VectorValue(role, values);
    // One value of each source: the first a lane count apart, the others every rotation.
    // A stored value's bytes all differ from the bytes memory holds, so that a store in part is seen.
    let stored = form.writes_memory();
    let each_source = |types: &mut Vec<OperandType>, lane: Lane| {
        let lane = if stored { Lane::Byte } else { lane };
        for (index, kind) in sources.iter().enumerate() {
            types.push(match (is_general(*kind), index) {
                (true, _) => OperandType::RegisterValue(Role::Source, ValueSet::Few),
                (false, 0) if sources.len() > 1 => vector(Role::Destination, lanes(lane, false)),
                (false, _) => vector(Role::Source, lanes(lane, true)),
            });
        }
    };
    let data = |set: AvxDataSet| OperandType::AvxData(set);
    let arithmetic = OperandType::Mxcsr(MxcsrSet::Arithmetic);
    let (immediate, mxcsr) = match form.operation {
        AvxOperation::Halves { operation, .. } => {
            each_source(&mut types, operation.to_lane());
            let counts = matches!(
                operation,
                wcontainers_processor_architecture_x86_64::vector::LaneOperation::AlignRight
            );
            (Some(if counts { ImmediateClass::Count } else { ImmediateClass::Selector }), None)
        }
        AvxOperation::ShiftByImmediate(operation) => {
            types.push(vector(Role::Source, lanes(operation.to_lane(), true)));
            (Some(ImmediateClass::Count), None)
        }
        AvxOperation::ShiftByCount(operation) => {
            types.push(vector(Role::Destination, lanes(operation.to_lane(), false)));
            types.push(vector(Role::Source, VectorValues::Count(operation.to_lane())));
            (None, None)
        }
        AvxOperation::Extend(operation) => {
            let from = match operation {
                wcontainers_processor_architecture_x86_64::vector::LaneOperation::Extend { from, .. } => from,
                _ => Lane::Byte,
            };
            types.push(vector(Role::Source, lanes(from, true)));
            (None, None)
        }
        AvxOperation::ShiftEach { lane, .. }
        | AvxOperation::TestSigns(lane)
        | AvxOperation::BlendVariable(lane)
        | AvxOperation::Broadcast(lane)
        | AvxOperation::Signs(lane)
        | AvxOperation::PermuteVariable(lane) => {
            each_source(&mut types, lane);
            (None, None)
        }
        AvxOperation::ExtractLane(lane) | AvxOperation::InsertLane(lane) => {
            each_source(&mut types, lane);
            (Some(ImmediateClass::Count), None)
        }
        AvxOperation::Test | AvxOperation::BroadcastHalf | AvxOperation::Aes(_) | AvxOperation::CarrylessMultiply => {
            each_source(&mut types, Lane::Quadword);
            (Some(ImmediateClass::Selector), None)
        }
        AvxOperation::InsertHalf | AvxOperation::ExtractHalf | AvxOperation::PermuteHalves | AvxOperation::PermuteQuadwords => {
            each_source(&mut types, Lane::Quadword);
            (Some(ImmediateClass::Selector), None)
        }
        AvxOperation::PermuteDoublewords => {
            types.push(vector(Role::Source, lanes(Lane::Doubleword, true)));
            types.push(vector(Role::Destination, lanes(Lane::Doubleword, false)));
            (None, None)
        }
        AvxOperation::MultipleSumsOfDifferences | AvxOperation::GaloisMultiply | AvxOperation::GaloisAffine { .. } => {
            each_source(&mut types, Lane::Byte);
            (Some(ImmediateClass::Selector), None)
        }
        AvxOperation::MaskedLoad(lane) | AvxOperation::MaskedStore(lane) => {
            types.push(data(AvxDataSet::Mask(lane)));
            (None, None)
        }
        AvxOperation::ZeroUpper | AvxOperation::ZeroAll => (None, None),
        AvxOperation::LoadMxcsr => {
            types.push(OperandType::RegisterValue(Role::Source, ValueSet::Arithmetic));
            (None, None)
        }
        AvxOperation::StoreMxcsr => (None, Some(arithmetic)),
        AvxOperation::Float { operation, precision, arrangement } => {
            let comparison = matches!(operation, FloatOperation::OrderedCompare { .. });
            let floats = VectorValues::Floats { precision, every_rotation: true };
            let kept = vector(Role::Destination, lanes(to_float_lane(precision), false));
            match (operation, sources.len()) {
                (FloatOperation::SquareRoot, 1) => types.push(vector(Role::Source, floats)),
                (FloatOperation::SquareRoot, _) => types.extend([kept, vector(Role::Source, floats)]),
                (FloatOperation::Convert(conversion), 1) => {
                    types.push(vector(Role::Source, VectorValues::Conversion { conversion, precision }));
                }
                (FloatOperation::Convert(conversion), _) => {
                    types.extend([kept, vector(Role::Source, VectorValues::Conversion { conversion, precision })]);
                }
                _ => types.push(OperandType::FloatOperands(FloatOperands { operation, precision, arrangement })),
            }
            (None, Some(OperandType::Mxcsr(if comparison { MxcsrSet::Comparison } else { MxcsrSet::Arithmetic })))
        }
        AvxOperation::Compare { precision, arrangement } => {
            types.push(data(AvxDataSet::Compare { precision, scalar: arrangement == Arrangement::Scalar }));
            (None, None)
        }
        AvxOperation::ConvertWidening(conversion) | AvxOperation::ConvertNarrowing(conversion) => {
            let precision = match conversion {
                Conversion::IntegerToFloat | Conversion::DoubleToSingle => Precision::Double,
                Conversion::SingleToDouble => Precision::Single,
                Conversion::FloatToInteger { .. } => Precision::Double,
            };
            types.push(vector(Role::Source, VectorValues::Conversion { conversion, precision }));
            (None, Some(arithmetic))
        }
        AvxOperation::IntegerToScalar(precision) => {
            types.push(vector(Role::Destination, lanes(to_float_lane(precision), false)));
            types.push(OperandType::RegisterValue(Role::Source, ValueSet::Arithmetic));
            (None, Some(arithmetic))
        }
        AvxOperation::ScalarToInteger { precision, .. } => {
            let bits = if matches!(list_kinds(form)[0], Kind::r64_reg) { u64::BITS } else { u32::BITS };
            types.push(data(AvxDataSet::ToInteger { precision, bits }));
            (None, None)
        }
        AvxOperation::RoundToIntegral { precision, arrangement } => {
            types.push(data(AvxDataSet::Round { precision, scalar: arrangement == Arrangement::Scalar }));
            (None, None)
        }
        AvxOperation::Fused { precision, arrangement, .. } => {
            types.push(data(AvxDataSet::Fused { precision, scalar: arrangement == Arrangement::Scalar }));
            (None, None)
        }
        AvxOperation::HalfToSingle => {
            types.push(data(AvxDataSet::HalfToSingle));
            (None, None)
        }
        AvxOperation::SingleToHalf => {
            types.push(data(AvxDataSet::SingleToHalf));
            (None, None)
        }
        AvxOperation::DotProduct(precision) => {
            types.push(data(AvxDataSet::Dot(precision)));
            (None, None)
        }
        AvxOperation::StringCompare { explicit, quadword_lengths, .. } => {
            types.push(data(AvxDataSet::Strings { explicit, quadword_lengths }));
            (None, None)
        }
        AvxOperation::Gather { data: lane, index } => {
            types.extend([
                OperandType::DataPagePermission(crate::DataPage::Lower),
                OperandType::DataPagePermission(crate::DataPage::Upper),
                data(AvxDataSet::Gather { data: lane, index }),
            ]);
            (None, None)
        }
        AvxOperation::MaskedByteStore => {
            types.extend([
                OperandType::AccessPlacement(Splits::Allowed),
                OperandType::DataPagePermission(crate::DataPage::Lower),
                OperandType::DataPagePermission(crate::DataPage::Upper),
                data(AvxDataSet::Mask(Lane::Byte)),
            ]);
            (None, None)
        }
    };
    let has_immediate = form.code.op_code().op_kinds().contains(&Kind::imm8);
    if has_immediate && let Some(class) = immediate {
        types.push(OperandType::Immediate(class));
    }
    types.extend(mxcsr);
    types.push(OperandType::Flags(FlagSet::Unread));
    types
}

/// The critical values of an [`AvxData`] operand.
pub fn generate_data(set: AvxDataSet) -> Vec<OperandValue> {
    let values: Vec<AvxData> = match set {
        AvxDataSet::Compare { precision, scalar } => generate_compares(precision, scalar),
        AvxDataSet::Fused { precision, scalar } => generate_fused(precision, scalar),
        AvxDataSet::Round { precision, scalar } => generate_rounds(precision, scalar),
        AvxDataSet::ToInteger { precision, bits } => generate_integer_conversions(precision, bits),
        AvxDataSet::HalfToSingle => generate_half_widenings(),
        AvxDataSet::SingleToHalf => generate_half_narrowings(),
        AvxDataSet::Mask(lane) => generate_masks(lane),
        AvxDataSet::Dot(precision) => generate_dots(precision),
        AvxDataSet::Strings { explicit, quadword_lengths } => generate_strings(explicit, quadword_lengths),
        AvxDataSet::Gather { data, index } => generate_gathers(data, index),
    };
    values.into_iter().map(OperandValue::AvxData).collect()
}

/// MXCSR's values for arithmetic: every exception masked in each rounding mode, every one unmasked, all but inexact
/// unmasked, inexact alone unmasked, FTZ alone, DAZ alone and every flag set.
fn list_mxcsr_values() -> [u32; 10] {
    let rounding = |mode: u32| (MXCSR_DEFAULT & !(3 << ROUNDING_SHIFT)) | (mode << ROUNDING_SHIFT);
    let inexact = 1 << 5;
    [
        MXCSR_DEFAULT,
        rounding(1),
        rounding(2),
        rounding(3),
        0,
        inexact << MASK_SHIFT,
        MXCSR_DEFAULT & !(inexact << MASK_SHIFT),
        MXCSR_DEFAULT | FLUSH_TO_ZERO,
        MXCSR_DEFAULT | DENORMALS_ARE_ZEROS,
        MXCSR_DEFAULT | EXCEPTION_FLAGS,
    ]
}

/// The lane values of the precision, from the domain's floating-point critical values.
fn list_floats(precision: Precision) -> Vec<u64> {
    let lane = to_float_lane(precision);
    let mask = if lane.to_bits() == u64::BITS { u64::MAX } else { (1 << lane.to_bits()) - 1 };
    rotate_floats(precision, true).into_iter().map(|vector| vector as u64 & mask).collect()
}

/// Lane values joined into vectors, `lanes` at a time, the last vector filled from the start again.
fn pack(lane: Lane, values: &[u64], lanes: usize) -> Vec<u128> {
    values
        .chunks(lanes)
        .map(|chunk| join(lane, (0..lanes).map(|index| chunk.get(index).copied().unwrap_or(values[index % values.len()]))))
        .collect()
}

/// 2^exponent times one plus `fraction` units of the last place, in the precision, with the sign.
fn make_float(precision: Precision, negative: bool, exponent: i32, fraction: u64) -> u64 {
    let fraction_bits = precision.to_significand_bits() - 1;
    let bias = (1i32 << (precision.to_bits() - precision.to_significand_bits() - 1)) - 1;
    let sign = if negative { 1u64 << (precision.to_bits() - 1) } else { 0 };
    sign | (((exponent + bias) as u64) << fraction_bits) | fraction
}

/// Every ordered pair of critical values under the predicates that reveal their order (EQ, LT, UNORD), then each
/// predicate on pairs of each order (less, equal, greater, a quiet NaN, a signaling NaN, a denormal), each pair in
/// every lane, under each comparison MXCSR value.
fn generate_compares(precision: Precision, scalar: bool) -> Vec<AvxData> {
    let lane = to_float_lane(precision);
    let lanes = if scalar { 1 } else { (u128::BITS / lane.to_bits()) as usize };
    let floats = list_floats(precision);
    let pairs: Vec<(u64, u64)> = floats.iter().flat_map(|&first| floats.iter().map(move |&second| (first, second))).collect();
    let firsts: Vec<u64> = pairs.iter().map(|pair| pair.0).collect();
    let seconds: Vec<u64> = pairs.iter().map(|pair| pair.1).collect();
    let mut values = Vec::new();
    for predicate in [0, 1, 3] {
        for (first, second) in pack(lane, &firsts, lanes).into_iter().zip(pack(lane, &seconds, lanes)) {
            values.push(AvxData { vectors: [first, second, 0], mxcsr: MXCSR_DEFAULT, immediate: predicate });
        }
    }
    let one = make_float(precision, false, 0, 0);
    let two = make_float(precision, false, 1, 0);
    let quiet = precision.to_default_nan() & !(1 << (precision.to_bits() - 1));
    let signaling = (quiet & !precision.to_quiet_bit()) | 1;
    let classes = [(one, two), (one, one), (two, one), (one, quiet), (signaling, one), (1, one)];
    let comparison = [MXCSR_DEFAULT, 0, MXCSR_DEFAULT | DENORMALS_ARE_ZEROS, MXCSR_DEFAULT | EXCEPTION_FLAGS];
    for predicate in 0..32 {
        for &(first, second) in &classes {
            for mxcsr in comparison {
                values.push(AvxData { vectors: [fill_lanes(lane, first), fill_lanes(lane, second), 0], mxcsr, immediate: predicate });
            }
        }
    }
    values
}

/// Every triple of twelve critical values under the default MXCSR, a quiet NaN's payload told apart by its place;
/// then the triples that reach each rounding and exception, each in every lane, under every MXCSR value.
fn generate_fused(precision: Precision, scalar: bool) -> Vec<AvxData> {
    let lane = to_float_lane(precision);
    let lanes = if scalar { 1 } else { (u128::BITS / lane.to_bits()) as usize };
    let digits = precision.to_significand_bits() as i32;
    let bias = (1i32 << (precision.to_bits() - precision.to_significand_bits() - 1)) - 1;
    let sign = 1u64 << (precision.to_bits() - 1);
    let quiet = precision.to_default_nan() & !sign;
    let infinity = quiet & !precision.to_quiet_bit();
    let largest = make_float(precision, false, bias, (1 << (digits - 1)) - 1);
    let one = make_float(precision, false, 0, 0);
    let values = [
        0,
        sign,
        1,
        make_float(precision, false, 1 - bias, 0),
        one,
        make_float(precision, false, 0, 1),
        make_float(precision, true, 0, 1 << (digits - 2)),
        largest,
        infinity,
        sign | infinity,
        quiet,
        infinity | 1,
    ];
    // Each place's NaNs carry a payload of their own, so that the NaN returned shows whose it is.
    let placed = |place: usize, value: u64| {
        let exponent_all_ones = (value & !sign) >> (digits - 1) == (1 << (precision.to_bits() - digits as u32)) - 1;
        let is_nan = exponent_all_ones && value & ((1 << (digits - 1)) - 1) != 0;
        if is_nan { value ^ ((place as u64 + 1) << 4) } else { value }
    };
    let mut triples: [Vec<u64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for &a in &values {
        for &b in &values {
            for &c in &values {
                for (place, value) in [a, b, c].into_iter().enumerate() {
                    triples[place].push(placed(place, value));
                }
            }
        }
    }
    let packed: [Vec<u128>; 3] = array::from_fn(|place| pack(lane, &triples[place], lanes));
    let mut data: Vec<AvxData> = (0..packed[0].len())
        .map(|index| AvxData { vectors: [packed[0][index], packed[1][index], packed[2][index]], mxcsr: MXCSR_DEFAULT, immediate: 0 })
        .collect();
    let ulp = make_float(precision, false, 0, 1);
    let minus_one = make_float(precision, true, 0, 0);
    let half = make_float(precision, false, -1, 0);
    let smallest = make_float(precision, false, 1 - bias, 0);
    let special = [
        // (1 + ulp)^2 - 1: the product's low bits decide the single rounding.
        (ulp, ulp, minus_one),
        (ulp, make_float(precision, true, 0, 1), one),
        // An exact zero, whose sign the rounding mode decides.
        (one, one, minus_one),
        // Overflow, and underflow exact and not.
        (largest, make_float(precision, false, 1, 0), 0),
        (largest, one, largest),
        (smallest, half, 0),
        (smallest, make_float(precision, false, -1, 1), 0),
        (smallest, one, sign | 1),
        // A denormal operand.
        (1, one, one),
        // 0 times infinity plus a quiet NaN, and an infinite product less infinity.
        (0, infinity, quiet | 0x30),
        (infinity, one, sign | infinity),
    ];
    for (a, b, c) in special {
        for mxcsr in list_mxcsr_values() {
            data.push(AvxData { vectors: [a, b, c].map(|value| fill_lanes(lane, value)), mxcsr, immediate: 0 });
        }
    }
    data
}

/// The critical values and the ties and halves rounding meets, every rotation, under each of the sixteen
/// immediates' rounding and suppression with the default MXCSR, then MXCSR's own rounding in every MXCSR value.
fn generate_rounds(precision: Precision, scalar: bool) -> Vec<AvxData> {
    let lane = to_float_lane(precision);
    let lanes = if scalar { 1 } else { (u128::BITS / lane.to_bits()) as usize };
    let digits = precision.to_significand_bits() as i32;
    let mut values = list_floats(precision);
    values.extend([
        make_float(precision, false, -1, 0),
        make_float(precision, true, -1, 0),
        make_float(precision, false, 1, 1 << (digits - 3)),
        make_float(precision, true, 1, 1 << (digits - 3)),
        make_float(precision, false, 0, 3 << (digits - 3)),
        make_float(precision, false, digits - 2, 1),
        make_float(precision, false, -1, 1),
    ]);
    let rotations: Vec<u128> = (0..values.len())
        .map(|rotation| join(lane, (0..lanes).map(|index| values[(index + rotation) % values.len()])))
        .collect();
    let kept = fill_lanes(lane, make_float(precision, false, 3, 5));
    let mut data = Vec::new();
    for immediate in 0..16 {
        for &vector in &rotations {
            data.push(AvxData { vectors: if scalar { [kept, vector, 0] } else { [vector, 0, 0] }, mxcsr: MXCSR_DEFAULT, immediate });
        }
    }
    for mxcsr in list_mxcsr_values() {
        for immediate in [4, 0xC] {
            for &vector in &rotations {
                data.push(AvxData { vectors: if scalar { [kept, vector, 0] } else { [vector, 0, 0] }, mxcsr, immediate });
            }
        }
    }
    data
}

/// The critical values and each end of the integer range, with the values a half and one past it, under every
/// MXCSR value.
fn generate_integer_conversions(precision: Precision, bits: u32) -> Vec<AvxData> {
    let lane = to_float_lane(precision);
    let digits = precision.to_significand_bits() as i32;
    let edge = bits as i32 - 1;
    let below = |units: u64| if digits - 1 > edge { (1u64 << (digits - 1)) - (units << (digits - 1 - edge)) } else { (1 << (digits - 1)) - 1 };
    let beyond = |units: u64| if digits - 1 > edge { units << (digits - 2 - edge) } else { 1 };
    let mut values = list_floats(precision);
    values.extend([
        make_float(precision, false, -1, 0),
        make_float(precision, false, 0, 1 << (digits - 2)),
        make_float(precision, true, 1, 1 << (digits - 3)),
        make_float(precision, false, edge - 1, below(1)),
        make_float(precision, false, edge - 1, below(2)),
        make_float(precision, false, edge, 0),
        make_float(precision, true, edge, 0),
        make_float(precision, true, edge, beyond(1)),
        make_float(precision, true, edge, beyond(2)),
    ]);
    let mut data = Vec::new();
    for mxcsr in list_mxcsr_values() {
        for &value in &values {
            data.push(AvxData { vectors: [join(lane, [value].into_iter()), 0, 0], mxcsr, immediate: 0 });
        }
    }
    data
}

/// Half-precision values: each zero, the smallest and largest subnormal, the smallest normal, one, the largest
/// normal, each infinity and a quiet and a signaling NaN with a payload, in every rotation; under each MXCSR value.
fn generate_half_widenings() -> Vec<AvxData> {
    let halves: [u64; 12] = [0, 0x8000, 1, 0x3FF, 0x0400, 0x3C00, 0xBE00, 0x7BFF, 0x7C00, 0xFC00, 0x7E05, 0x7C21];
    let rotations: Vec<u128> = (0..halves.len()).map(|rotation| join(Lane::Word, (0..8).map(|index| halves[(index + rotation) % halves.len()]))).collect();
    let mut data = Vec::new();
    for mxcsr in [MXCSR_DEFAULT, 0, MXCSR_DEFAULT | DENORMALS_ARE_ZEROS, MXCSR_DEFAULT | FLUSH_TO_ZERO] {
        for &vector in &rotations {
            data.push(AvxData { vectors: [vector, 0, 0], mxcsr, immediate: 0 });
        }
    }
    data
}

/// Singles at half precision's edges: its largest normal, the value that rounds to it or overflows, its smallest
/// normal and subnormal and the ties below them, with the single critical values, in every rotation; under each
/// rounding immediate with the default MXCSR, then MXCSR's own rounding in every MXCSR value.
fn generate_half_narrowings() -> Vec<AvxData> {
    let single = Precision::Single;
    let mut values = list_floats(single);
    values.extend([
        make_float(single, false, 15, 0x7FE000),
        make_float(single, false, 15, 0x7FF000),
        make_float(single, true, 15, 0x7FF001),
        make_float(single, false, 16, 0),
        make_float(single, false, -14, 0),
        make_float(single, false, -24, 0),
        make_float(single, false, -25, 0),
        make_float(single, true, -25, 1),
        make_float(single, false, -15, 0x400000),
        make_float(single, false, 0, 0x1000),
        make_float(single, false, 0, 0x3000),
    ]);
    let rotations: Vec<u128> =
        (0..values.len()).map(|rotation| join(Lane::Doubleword, (0..4).map(|index| values[(index + rotation) % values.len()]))).collect();
    let mut data = Vec::new();
    for immediate in 0..8 {
        for &vector in &rotations {
            data.push(AvxData { vectors: [vector, 0, 0], mxcsr: MXCSR_DEFAULT, immediate });
        }
    }
    for mxcsr in list_mxcsr_values() {
        for &vector in &rotations {
            data.push(AvxData { vectors: [vector, 0, 0], mxcsr, immediate: 4 });
        }
    }
    data
}

/// Masks: none; the lowest and the highest lane (the representative, so that a store across a page boundary
/// shows what it writes below the fault, and the lanes between are seen not to fault); the lowest alone, every
/// lane, the highest alone, and alternate lanes; each with a value whose lanes differ.
fn generate_masks(lane: Lane) -> Vec<AvxData> {
    let bits = lane.to_bits();
    let lanes = (u128::BITS / bits) as usize;
    let top = 1u64 << (bits - 1);
    let mask = |selected: &dyn Fn(usize) -> bool| join(lane, (0..lanes).map(|index| if selected(index) { top | index as u64 } else { index as u64 }));
    let value = join(lane, (0..lanes).map(|index| 0x5AA5_C33C_0FF0_9669u64.rotate_left(8 * index as u32)));
    let masks = [
        mask(&|_| false),
        mask(&|index| index == 0 || index == lanes - 1),
        mask(&|index| index == 0),
        mask(&|_| true),
        mask(&|index| index == lanes - 1),
        mask(&|index| index % 2 == 1),
    ];
    masks.into_iter().map(|mask| AvxData { vectors: [mask, value, 0], mxcsr: MXCSR_DEFAULT, immediate: 0 }).collect()
}

/// Every ordered pair of critical values under the immediate that sums every product into every lane, the first
/// pairs under immediates that select other lanes, and the pairs that overflow, underflow or cancel, each in every
/// lane, under every MXCSR value.
fn generate_dots(precision: Precision) -> Vec<AvxData> {
    let lane = to_float_lane(precision);
    let lanes = (u128::BITS / lane.to_bits()) as usize;
    let all: u8 = if lanes == 4 { 0xFF } else { 0x33 };
    let floats = list_floats(precision);
    let pairs: Vec<(u64, u64)> = floats.iter().flat_map(|&first| floats.iter().map(move |&second| (first, second))).collect();
    let firsts: Vec<u64> = pairs.iter().map(|pair| pair.0).collect();
    let seconds: Vec<u64> = pairs.iter().map(|pair| pair.1).collect();
    let packed: Vec<(u128, u128)> = pack(lane, &firsts, lanes).into_iter().zip(pack(lane, &seconds, lanes)).collect();
    let mut data: Vec<AvxData> = packed.iter().map(|&(a, b)| AvxData { vectors: [a, b, 0], mxcsr: MXCSR_DEFAULT, immediate: all }).collect();
    for immediate in [0x00, 0x31, 0x5A, 0xC3, 0xF0, 0x0F, 0x9E, 0x67, 0x12, 0x21] {
        data.extend(packed.iter().take(16).map(|&(a, b)| AvxData { vectors: [a, b, 0], mxcsr: MXCSR_DEFAULT, immediate }));
    }
    let digits = precision.to_significand_bits() as i32;
    let bias = (1i32 << (precision.to_bits() - precision.to_significand_bits() - 1)) - 1;
    let largest = make_float(precision, false, bias, (1 << (digits - 1)) - 1);
    let one = make_float(precision, false, 0, 0);
    let special = [
        (largest, make_float(precision, false, 1, 0)),
        (largest, one),
        (make_float(precision, false, 1 - bias, 0), make_float(precision, false, -1, 1)),
        (make_float(precision, false, 0, 1), make_float(precision, false, 0, 1)),
        (1, one),
    ];
    for (a, b) in special {
        for mxcsr in list_mxcsr_values() {
            data.push(AvxData { vectors: [fill_lanes(lane, a), fill_lanes(lane, b), 0], mxcsr, immediate: all });
        }
    }
    // Products that cancel: a and b, then a and minus b.
    let alternating = join(lane, (0..lanes).map(|index| if index % 2 == 0 { one } else { one | (1 << (precision.to_bits() - 1)) }));
    for mxcsr in list_mxcsr_values() {
        data.push(AvxData { vectors: [fill_lanes(lane, one), alternating, 0], mxcsr, immediate: all });
    }
    data
}

/// String pairs that match each aggregation in part (characters in common, ranges that hold some, equal prefixes, a
/// substring), with elements whose sign decides a range, nulls that end a string early, and an empty string; each
/// under every immediate with the lengths their nulls give, then under each pair of lengths below, at, beyond and
/// past the element count both ways, with eight immediates.
fn generate_strings(explicit: bool, quadword_lengths: bool) -> Vec<AvxData> {
    let bytes = |text: &[u8]| -> u128 {
        let mut all = [0u8; 16];
        all[..text.len()].copy_from_slice(text);
        u128::from_le_bytes(all)
    };
    let pairs: [(u128, u128); 8] = [
        (bytes(b"ABCDEFGHIJKLMNOP"), bytes(b"ABCDEFGHIJKLMNOP")),
        (bytes(b"ab"), bytes(b"xxabyyabzzzzzzab")),
        (bytes(b"09AZ"), bytes(b"a5B\x7f0Z9!~@[`{/:")),
        (bytes(&[0x80, 0xFF, 0x01, 0x7F, 0x41, 0x41]), bytes(&[0x90, 0x00, 0x7F, 0x80, 0x41, 0x20, 0xFF, 0x01])),
        (bytes(b"ABCDEFGHIJKLMNOP"), bytes(b"ABCDEFG\0IJKLMNOP")),
        (0, 0),
        (bytes(&[0x00, 0x80, 0xFF, 0x7F, 0x34, 0x12]), bytes(&[0x01, 0x80, 0x34, 0x12, 0xFF, 0x7F, 0x00, 0x80])),
        (bytes(b"CDE"), bytes(b"ABCDECDEFGCDECDX")),
    ];
    let lengths = |first: u64, second: u64| u128::from(first) | (u128::from(second) << 64);
    let natural = |(a, b): (u128, u128)| {
        let length = |value: u128| value.to_le_bytes().iter().position(|&byte| byte == 0).unwrap_or(16) as u64;
        lengths(length(a), length(b))
    };
    let mut data = Vec::new();
    for immediate in 0..0x80 {
        for &pair in &pairs {
            data.push(AvxData { vectors: [pair.0, pair.1, natural(pair)], mxcsr: MXCSR_DEFAULT, immediate });
        }
    }
    if explicit {
        let wide = if quadword_lengths { 1 << 32 } else { 0 };
        let combinations = [
            lengths(16, 16),
            lengths(3, 10),
            lengths(0, 0),
            lengths(20, -5i64 as u64),
            lengths(-20i64 as u64, 7),
            lengths(wide | 3, 2),
            lengths(8, wide | 9),
            lengths(i64::MIN as u64, 17),
        ];
        for combination in combinations {
            for immediate in [0x00, 0x0C, 0x08, 0x04, 0x30, 0x14, 0x48, 0x71] {
                for &pair in &pairs {
                    data.push(AvxData { vectors: [pair.0, pair.1, combination], mxcsr: MXCSR_DEFAULT, immediate });
                }
            }
        }
    }
    data
}

/// A gather's indices, each set with its scale and every lane on one page, so that a lane faults only where the
/// lowest lane selected does (a single step ends a gather that faults after loading a lane without the fault):
/// consecutive lanes on the lower page; out of order on the upper; repeated, at scale 2; each across the boundary;
/// negative, below the base on page zero; and far beyond, at 2 GiB or past the canonical half. Each with every
/// mask of [`generate_masks`].
fn generate_gathers(data: Lane, index: Lane) -> Vec<AvxData> {
    let bytes = u64::from(data.to_bits() / u8::BITS);
    let lanes = (u128::BITS / index.to_bits()) as usize;
    let page = PAGE_BYTES;
    let far = if index == Lane::Quadword { 1 << 62 } else { 0x7FFF_FFFF };
    let sets: [(u8, [u64; 4]); 6] = [
        (bytes as u8, [0, 1, 2, 3]),
        (1, [page, page + 3 * bytes, page + bytes, page + 2 * bytes]),
        (2, [3, 0, 3, 1]),
        (1, [page - bytes / 2; 4]),
        (1, [(-(page as i64) - 8) as u64, (-(page as i64) - 16) as u64, (-(page as i64) - 8) as u64, (-(page as i64) - 24) as u64]),
        (1, [far, far + 8, far, far + 16]),
    ];
    let destination = join(data, (0..u128::BITS / data.to_bits()).map(|lane| 0x5AA5_C33C_0FF0_9669u64.rotate_left(8 * lane)));
    let mut values = Vec::new();
    for (scale, offsets) in sets {
        let indices = join(index, (0..lanes).map(|lane| offsets[lane % 4]));
        for mask in generate_masks(data) {
            values.push(AvxData { vectors: [destination, indices, mask.vectors[0]], mxcsr: MXCSR_DEFAULT, immediate: scale });
        }
    }
    values
}

/// Builds the state of a gather: the destination at ModRM.reg, the indices at the VSIB index the pair's other
/// register names (another where the pair names one), the mask at VEX.vvvv in the lowest other register but XMM0,
/// the base RBX at the lower data page.
fn build_gather_state(form: &AvxForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    use crate::{LOWER_DATA_ADDRESS, UPPER_DATA_ADDRESS, to_data_byte};
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut registers, mut layout) = ([0, 0], CodeLayout::PageStart);
    let (mut lower, mut upper, mut chosen) = (PagePermission::Unmapped, PagePermission::Unmapped, None);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => (registers, layout) = (encoding.registers, encoding.layout),
            (OperandType::DataPagePermission(crate::DataPage::Lower), OperandValue::PagePermission(permission)) => lower = permission,
            (OperandType::DataPagePermission(crate::DataPage::Upper), OperandValue::PagePermission(permission)) => upper = permission,
            (OperandType::AvxData(_), OperandValue::AvxData(data)) => chosen = Some(data),
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let chosen = chosen.expect("a gather's data");
    // The destination, index and mask registers are three, as iced-x86 decodes no other gather.
    if registers[0] == registers[1] {
        registers[1] = (registers[0] + 8) % 16;
    }
    let mask = (0..16).find(|number| !registers.contains(number) && *number != 0).expect("a free register");
    let lane_bits = to_lane_bits(form);
    for (number, value) in [(registers[0], chosen.vectors[0]), (mask, chosen.vectors[2]), (registers[1], chosen.vectors[1])] {
        processor.vector_registers[number] = value;
        processor.upper_vector_halves[number] = value.rotate_left(lane_bits);
    }
    // RBX, register 3, is the base.
    processor.general_purpose_registers[3] = LOWER_DATA_ADDRESS;
    let kinds = form.code.op_code().op_kinds();
    let to_vector = |wide: bool, number: usize| if wide { VECTOR_256[number] } else { VECTOR[number] };
    let index_wide = matches!(kinds[1], Kind::mem_vsib32y | Kind::mem_vsib64y);
    let destination_wide = kinds[0] == Kind::ymm_reg;
    let memory = MemoryOperand::with_base_index_scale(Register::RBX, to_vector(index_wide, registers[1]), u32::from(chosen.immediate));
    let encoded = Encoded::with3(form.code, to_vector(destination_wide, registers[0]), memory, to_vector(destination_wide, mask))
        .unwrap_or_else(|error| panic!("{:?} takes its operands: {error}", form.code));
    let (code_page, start, bytes) = lay_out(layout, &encode_at_code_address(&encoded), to_padding(None));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let pages: Vec<u8> = (LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES).map(to_data_byte).collect();
    ExecutionState {
        processor,
        memory_mutations: vec![
            MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
            MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
            MemoryDelta::ValueAssignment { start: LOWER_DATA_ADDRESS, bytes: Cow::Owned(pages) },
            MemoryDelta::PermissionAssignment { addresses: LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS, permission: lower },
            MemoryDelta::PermissionAssignment { addresses: UPPER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES, permission: upper },
        ],
    }
}

/// The VEX.vvvv register of the register form whose ModRM.reg and ModRM.rm registers are the pair.
const fn to_vvvv(registers: [usize; 2]) -> usize {
    match registers {
        [1, 13] => 0,
        [13, 4] | [4, 7] => 4,
        [first, _] => first,
    }
}

/// The is4 register: distinct from the others, or all three one register.
const fn to_is4(registers: [usize; 2]) -> usize {
    if registers[0] == registers[1] { registers[0] } else { 5 }
}

/// An operand to encode.
#[derive(Clone, Copy)]
enum Encodable {
    Register(Register),
    Memory(MemoryOperand),
    /// VMASKMOVDQU's RDI, which the encoding does not name.
    Implicit,
}

/// The address of VMASKMOVDQU's store at its placement around the data pages, whose bytes and permissions it adds.
fn place_at_rdi(placement: crate::AccessPlacement, lower: PagePermission, upper: PagePermission, mutations: &mut Vec<MemoryDelta<'static, X86_64>>) -> u64 {
    use crate::{AccessPlacement, LOWER_DATA_ADDRESS, UPPER_DATA_ADDRESS, to_data_byte};
    const BYTES: u64 = 16;
    let pages: Vec<u8> = (LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES).map(to_data_byte).collect();
    mutations.extend([
        MemoryDelta::ValueAssignment { start: LOWER_DATA_ADDRESS, bytes: Cow::Owned(pages) },
        MemoryDelta::PermissionAssignment { addresses: LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS, permission: lower },
        MemoryDelta::PermissionAssignment { addresses: UPPER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES, permission: upper },
    ]);
    match placement {
        AccessPlacement::LowerPageStart => LOWER_DATA_ADDRESS,
        AccessPlacement::Misaligned => LOWER_DATA_ADDRESS + 1,
        AccessPlacement::EndsAtBoundary => UPPER_DATA_ADDRESS - BYTES,
        AccessPlacement::StraddlesBoundary => UPPER_DATA_ADDRESS - BYTES / 2,
        AccessPlacement::UpperPageStart => UPPER_DATA_ADDRESS,
        AccessPlacement::NonCanonical => 1 << 63,
    }
}

/// Builds the state of an AVX form from one value of each of its operand types.
pub fn build_state(form: &AvxForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    if let AvxOperation::Gather { .. } = form.operation {
        return build_gather_state(form, operands);
    }
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut registers, mut access, mut layout) = ([0, 0], None, CodeLayout::PageStart);
    let mut values: Vec<u128> = Vec::new();
    let mut immediate = None;
    let (mut placement, mut lower, mut upper) = (None, PagePermission::Unmapped, PagePermission::Unmapped);
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
            (OperandType::VectorValue(..), OperandValue::Vector(value)) => values.push(value),
            (OperandType::FloatOperands(_), OperandValue::Vectors { destination, source }) => values.extend([destination, source]),
            (OperandType::RegisterValue(..), OperandValue::Bits(value)) => values.push(u128::from(value)),
            (OperandType::AvxData(set), OperandValue::AvxData(data)) => {
                values.extend(&data.vectors[..set.count()]);
                processor.mxcsr = data.mxcsr;
                if form.code.op_code().op_kinds().contains(&Kind::imm8) {
                    immediate = Some(data.immediate);
                }
            }
            (OperandType::Immediate(_), OperandValue::Immediate(value)) => immediate = Some(value as u8),
            (OperandType::Mxcsr(_), OperandValue::Mxcsr(mxcsr)) => processor.mxcsr = mxcsr,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (OperandType::AccessPlacement(_), OperandValue::AccessPlacement(chosen)) => placement = Some(chosen),
            (OperandType::DataPagePermission(crate::DataPage::Lower), OperandValue::PagePermission(permission)) => lower = permission,
            (OperandType::DataPagePermission(crate::DataPage::Upper), OperandValue::PagePermission(permission)) => upper = permission,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let mut placed_mutations = Vec::new();
    if let AvxOperation::StringCompare { .. } = form.operation {
        // The lengths in RAX and RDX, registers 0 and 2.
        let lengths = values.pop().expect("the lengths");
        processor.general_purpose_registers[0] = lengths as u64;
        processor.general_purpose_registers[2] = (lengths >> 64) as u64;
    }
    if form.operation == AvxOperation::MaskedByteStore {
        // The value comes first, the mask second; RDI, register 7, holds the address.
        values.swap(0, 1);
        let placement = placement.expect("a placement");
        processor.general_purpose_registers[7] = place_at_rdi(placement, lower, upper, &mut placed_mutations);
    }
    if form.operation == AvxOperation::LoadMxcsr {
        values[0] &= u128::from(u32::MAX);
    }
    let kinds = list_kinds(form);
    let has_reg = kinds.iter().any(|kind| matches!(kind, Kind::xmm_reg | Kind::ymm_reg | Kind::r32_reg | Kind::r64_reg));
    let vvvv = match access {
        Some(_) if has_reg => 2,
        Some(chosen) => chosen.register,
        None if has_reg => to_vvvv(registers),
        None => registers[0],
    };
    let is4 = if access.is_some() { 6 } else { to_is4(registers) };
    let lane_bits = to_lane_bits(form);
    let first_source = if form.operation.reads_first_operand() { 0 } else { 1 };
    let memory_bytes = u64::from(form.memory_bytes);
    let mut memory_value: Option<[u128; 2]> = None;
    let mut encodables = Vec::new();
    let mut deferred_memory = false;
    for (index, kind) in kinds.iter().enumerate() {
        let value = index.checked_sub(first_source).and_then(|source| values.get(source)).copied();
        let number = match kind {
            Kind::xmm_reg | Kind::ymm_reg | Kind::r32_reg | Kind::r64_reg => Some(registers[0]),
            Kind::xmm_vvvv | Kind::ymm_vvvv => Some(vvvv),
            Kind::xmm_is4 | Kind::ymm_is4 => Some(is4),
            Kind::seg_rDI => {
                encodables.push(Encodable::Implicit);
                continue;
            }
            _ if access.is_some() => None,
            _ => Some(registers[1]),
        };
        let general = is_general(*kind);
        match number {
            Some(number) => {
                let register = match (general, kind) {
                    (true, Kind::r64_reg | Kind::r64_or_mem | Kind::r64_rm) => GENERAL_64[number],
                    (true, _) => GENERAL_32[number],
                    (false, Kind::ymm_reg | Kind::ymm_vvvv | Kind::ymm_or_mem | Kind::ymm_rm | Kind::ymm_is4) => VECTOR_256[number],
                    (false, _) => VECTOR[number],
                };
                if let Some(value) = value {
                    if general {
                        processor.general_purpose_registers[number] = value as u64;
                    } else {
                        processor.vector_registers[number] = value;
                        if form.wide {
                            processor.upper_vector_halves[number] = value.rotate_left(lane_bits);
                        }
                    }
                }
                encodables.push(Encodable::Register(register));
            }
            None => {
                memory_value = value.map(|value| [value, value.rotate_left(lane_bits)]);
                deferred_memory = true;
                encodables.push(Encodable::Register(Register::None));
            }
        }
    }
    let mut memory_mutations = placed_mutations;
    let memory_operand: Option<MemoryOperand> = access.map(|access| {
        let (operand, address) = place_memory_operand(access, memory_bytes, &mut processor, &mut memory_mutations);
        if let Some([low, high]) = memory_value {
            let bytes: Vec<u8> = (0..memory_bytes)
                .map(|offset| address.wrapping_add(offset))
                .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
                .zip([low.to_le_bytes(), high.to_le_bytes()].concat())
                .map(|(_, byte)| byte)
                .collect();
            if !bytes.is_empty() {
                memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(bytes) });
            }
        }
        operand
    });
    if deferred_memory {
        let operand = memory_operand.expect("a memory form has a memory operand");
        for encodable in &mut encodables {
            if matches!(encodable, Encodable::Register(Register::None)) {
                *encodable = Encodable::Memory(operand);
            }
        }
    }
    encodables.retain(|encodable| !matches!(encodable, Encodable::Implicit));
    let encoded = encode(form.code, &encodables, immediate);
    let code = if is_vex_w_set(form.instruction) { encode_with_vex_w(&encoded) } else { encode_at_code_address(&encoded) };
    let (code_page, start, bytes) = lay_out(layout, &code, to_padding(memory_operand));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}

/// Whether the form is the VEX.W1 encoding of a code that ignores VEX.W, which iced-x86 encodes W0 by default.
fn is_vex_w_set(instruction: Instruction) -> bool {
    matches!(instruction, Instruction::Vpcmpistri64XmmdqXmmdqImmb | Instruction::Vpcmpistri64XmmdqMemdqImmb)
}

/// The instruction's bytes at the code address with VEX.W set.
fn encode_with_vex_w(instruction: &Encoded) -> Vec<u8> {
    let mut encoder = iced_x86::Encoder::new(64);
    encoder.set_vex_wig(1);
    encoder
        .encode(instruction, CODE_ADDRESS)
        .unwrap_or_else(|error| panic!("{:?} encodes: {error}", instruction.code()));
    encoder.take_buffer()
}

/// The instruction with its operands in order and the immediate last.
fn encode(code: Code, operands: &[Encodable], immediate: Option<u8>) -> Encoded {
    use Encodable::{Memory as M, Register as R};
    let imm = immediate.map(u32::from);
    let encoded = match (operands, imm) {
        ([R(a), R(b)], None) if code == Code::VEX_Vmaskmovdqu_rDI_xmm_xmm => {
            Encoded::with_vmaskmovdqu(64, *a, *b, Register::None)
        }
        ([], None) => Ok(Encoded::with(code)),
        ([R(a), R(b)], None) => Encoded::with2(code, *a, *b),
        ([R(a), M(b)], None) => Encoded::with2(code, *a, *b),
        ([M(a), R(b)], None) => Encoded::with2(code, *a, *b),
        ([M(a)], None) => Encoded::with1(code, *a),
        ([R(a), R(b)], Some(i)) => Encoded::with3(code, *a, *b, i),
        ([R(a), M(b)], Some(i)) => Encoded::with3(code, *a, *b, i),
        ([M(a), R(b)], Some(i)) => Encoded::with3(code, *a, *b, i),
        ([R(a), R(b), R(c)], None) => Encoded::with3(code, *a, *b, *c),
        ([R(a), R(b), M(c)], None) => Encoded::with3(code, *a, *b, *c),
        ([M(a), R(b), R(c)], None) => Encoded::with3(code, *a, *b, *c),
        ([R(a), R(b), R(c)], Some(i)) => Encoded::with4(code, *a, *b, *c, i),
        ([R(a), R(b), M(c)], Some(i)) => Encoded::with4(code, *a, *b, *c, i),
        ([R(a), R(b), R(c), R(d)], None) => Encoded::with4(code, *a, *b, *c, *d),
        ([R(a), R(b), M(c), R(d)], None) => Encoded::with4(code, *a, *b, *c, *d),
        _ => panic!("{code:?} has no encoding of {} operands", operands.len()),
    };
    encoded.unwrap_or_else(|error| panic!("{code:?} takes its operands: {error}"))
}
