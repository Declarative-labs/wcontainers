//! The inputs of the optional forms of the architecture's `optional` family: the registers each names and how its
//! bytes lie (or its memory operand), its writemask, whether memory is one element broadcast, each source's value,
//! the immediate, MXCSR, and the state the system forms read: a segment base, the address a register holds, a jump's
//! displacement.
//!
//! The floating-point sources are vectors of every class rotated across all 512 bits, so that each element meets
//! each value: each zero, denormals at each end and in the middle, the smallest normal, one exactly and one ulp
//! above it, two, each end of a table entry's span and the next entry's start, exact powers of two of each exponent
//! parity, the values whose reciprocal is tiny or overflows, the largest number, each infinity, a QNaN, an SNaN and
//! negatives. MXCSR takes DAZ and FTZ each alone and together, every exception unmasked, a rounding mode and flags
//! already set, which the reciprocal forms must ignore but for DAZ and FTZ.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, EncodingKind, Instruction as Encoded, MemoryOperand, OpCodeOperandKind, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::avx512_integer::{Zmm, from_lanes, write_zmm};
use wcontainers_processor_architecture_x86_64::float::Precision;
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::optional::{OptionalForm, OptionalOperation};
use wcontainers_processor_architecture_x86_64::vector::Lane;
use wcontainers_processor_architecture_x86_64::{DataAccess, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR};

use crate::avx512_integer::{EvexIntegerEncoding, Writemask, ZmmValues, fill_avx512_registers};
use crate::integer::{
    CodeLayout, ImmediateClass, LAST_CANONICAL_PAGE, LAYOUTS_BEYOND_START, encode_at_code_address, lay_out, place_memory_operand,
    to_padding,
};
use crate::{CODE_ADDRESS, FlagSet, OperandType, OperandValue, Role, Splits, ValueSet};

/// The opmask register every writemask of a state is.
const WRITEMASK: Register = Register::K1;
const MXCSR_DEFAULT: u32 = 0x1F80;
const DAZ: u32 = 1 << 6;
const FTZ: u32 = 1 << 15;
/// Distinct low registers, high ones (EVEX's R', V' and X bits), and one register for all.
const EVEX_REGISTERS: [[usize; 3]; 3] = [[0, 1, 2], [17, 26, 31], [5, 5, 5]];
/// VEX names XMM0 to XMM15 alone.
const VEX_REGISTERS: [[usize; 3]; 3] = [[0, 1, 2], [13, 7, 15], [5, 5, 5]];
/// A form that names one general register: RAX, then R9, which needs REX.B.
const GENERAL_REGISTERS: [[usize; 3]; 2] = [[0, 1, 2], [9, 1, 2]];
/// The register a memory form's vector or general register is, besides the access's own.
const MEMORY_REGISTER_EVEX: usize = 17;
const MEMORY_REGISTER_VEX: usize = 11;
/// XBEGIN's opcode and ModRM byte.
const TRANSACTION_BEGIN: [u8; 2] = [0xC7, 0xF8];
const OPERAND_SIZE_PREFIX: u8 = 0x66;
/// What a register holds above the 32 bits a 32-bit register gives.
const UPPER_ADDRESS_BITS: u64 = 0xA5A5_A5A5_0000_0000;
/// The page MOVDIR64B's destination and UMONITOR's address lie on, apart from the data pages and the code.
const BLOCK_PAGE: u64 = 0x9000;

/// The vectors a form's source takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorSet {
    /// Floating-point values of the precision, each at every element.
    Floats(Precision),
    /// RCPPS's and RSQRTPS's single values, four to a 128-bit part, every part a rotation of the first.
    Singles,
    /// Elements of the lane each distinct, then each with its top bit set, all ones and none: an element moved from
    /// the wrong place, or sign-extended, is seen to, and no element equals the data pages' bytes.
    Elements(Lane),
}

/// What a form's register or address operand holds besides its value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    /// MOVDIR64B's destination: where it lies; the source's address size (the 0x67 prefix) is the register's.
    Block(BlockPlacement),
    /// UMONITOR's address: where it lies, and whether a 32-bit register names it.
    Monitored { placement: BlockPlacement, narrow: bool },
    /// XBEGIN's fallback: the displacement, whether the 0x66 prefix makes it 16 bits, and whether the code ends at
    /// the canonical edge.
    Fallback { displacement: i32, short: bool, at_edge: bool },
}

/// Where an address a register holds lies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockPlacement {
    /// The start of a writable page.
    Writable,
    /// One byte past it.
    Misaligned,
    /// 32 bytes past it.
    HalfBlock,
    /// The start of a read-only page.
    ReadOnly,
    /// The start of an unmapped page.
    Unmapped,
    /// The kernel's half.
    Kernel,
    /// Not canonical.
    NonCanonical,
    /// The writable page's address with bits above 32 set, which only a 32-bit register leaves behind.
    UpperBitsSet,
}

const BLOCK_PLACEMENTS: [BlockPlacement; 8] = [
    BlockPlacement::Writable,
    BlockPlacement::Misaligned,
    BlockPlacement::HalfBlock,
    BlockPlacement::ReadOnly,
    BlockPlacement::Unmapped,
    BlockPlacement::Kernel,
    BlockPlacement::NonCanonical,
    BlockPlacement::UpperBitsSet,
];

fn is_vex(form: &OptionalForm) -> bool {
    form.code().op_code().encoding() == EncodingKind::VEX
}

/// The operand types of a form of the family, in the order the build reads them.
pub fn list_operand_types(form: &OptionalForm) -> Vec<OperandType> {
    let in_memory = form.place == OperandPlace::Memory;
    let unread = OperandType::Flags(FlagSet::Unread);
    let mut types = vec![if in_memory {
        OperandType::Memory { splits: Splits::Allowed, offset: false }
    } else {
        OperandType::OptionalEncoding { vex: is_vex(form), codes: form.list_codes().len() }
    }];
    match form.operation {
        OptionalOperation::ExtractElement { bytes } => {
            types.push(OperandType::OptionalVector(VectorSet::Elements(to_lane(bytes))));
            if form.has_immediate() {
                types.push(OperandType::Immediate(ImmediateClass::Count));
            }
        }
        OptionalOperation::StoreQuadword { .. } => {
            types.push(OperandType::ZmmValue(Role::Source, ZmmValues::Lanes { lane: Lane::Quadword, every_rotation: true }));
        }
        OptionalOperation::MoveScalar { bytes } => {
            let lanes = ZmmValues::Lanes { lane: to_lane(bytes), every_rotation: false };
            let store = form.writes_memory();
            types.push(OperandType::Writemask { zeroing: !store });
            if !store {
                types.push(OperandType::ZmmValue(Role::Destination, lanes));
            }
            if !in_memory {
                types.push(OperandType::ZmmValue(Role::Implicit(1), lanes));
            }
            types.push(OperandType::ZmmValue(Role::Source, lanes));
        }
        OptionalOperation::ConvertToHalves => {
            types.push(OperandType::Writemask { zeroing: false });
            types.push(OperandType::OptionalVector(VectorSet::Floats(Precision::Single)));
            types.push(OperandType::Immediate(ImmediateClass::Rounding));
            types.push(OperandType::OptionalMxcsr { half: false });
        }
        OptionalOperation::VexReciprocal { .. } => types.push(OperandType::OptionalVector(VectorSet::Singles)),
        OptionalOperation::Reciprocal { precision, .. } => {
            types.push(OperandType::Writemask { zeroing: true });
            if form.can_broadcast() {
                types.push(OperandType::LaneBroadcast);
            }
            types.push(OperandType::OptionalVector(VectorSet::Floats(precision)));
            types.push(OperandType::OptionalMxcsr { half: precision == Precision::Half });
        }
        OptionalOperation::DirectStore => types.push(OperandType::RegisterValue(Role::Source, ValueSet::Distinct)),
        OptionalOperation::DirectStoreBlock => types.push(OperandType::OptionalPlace { block: true }),
        OptionalOperation::ReadBase { .. } => types.push(OperandType::OptionalBase),
        OptionalOperation::WriteBase { .. } => types.push(OperandType::RegisterValue(Role::Source, ValueSet::Target)),
        OptionalOperation::Monitor => types = vec![OperandType::OptionalPlace { block: false }],
        OptionalOperation::TransactionBegin => {
            types = vec![OperandType::OptionalFallback, OperandType::RegisterValue(Role::Implicit(0), ValueSet::Few)];
        }
    }
    types.push(unread);
    types
}

const fn to_lane(bytes: u64) -> Lane {
    match bytes {
        1 => Lane::Byte,
        2 => Lane::Word,
        4 => Lane::Doubleword,
        _ => Lane::Quadword,
    }
}

/// Every choice of registers with every code at the page's start, then the first choice with every other layout.
pub fn generate_encodings(vex: bool, codes: usize) -> Vec<OperandValue> {
    let choices: &[[usize; 3]] = match (vex, codes) {
        (_, 2) => &GENERAL_REGISTERS,
        (true, _) => &VEX_REGISTERS,
        (false, _) => &EVEX_REGISTERS,
    };
    let at_start = choices
        .iter()
        .flat_map(|&registers| (0..codes).map(move |code| EvexIntegerEncoding { registers, code, layout: CodeLayout::PageStart }));
    let laid_out = LAYOUTS_BEYOND_START.iter().map(|&layout| EvexIntegerEncoding { registers: choices[0], code: codes - 1, layout });
    at_start.chain(laid_out).map(OperandValue::EvexIntegerEncoding).collect()
}

/// The values every element of a floating-point source meets, as bits of the precision.
fn list_float_values(precision: Precision) -> Vec<u64> {
    match precision {
        Precision::Half => vec![
            0, 0x8000, 1, 0x0200, 0x03FF, 0x0400, 0x3C00, 0x3C01, 0x3DFF, 0x3E00, 0x4000, 0x4001, 0x3555, 0x7400, 0x7401,
            0x77FF, 0x7800, 0x7BFF, 0x7C00, 0xFC00, 0x7E01, 0x7D01, 0xBC00, 0xC200, 0x8001, 0x5A5A,
        ],
        Precision::Single => vec![
            0, 0x8000_0000, 1, 0x0040_0000, 0x007F_FFFF, 0x0080_0000, 0x3F80_0000, 0x3F80_0001, 0x3F80_007F, 0x3F80_0080,
            0x3FFF_FFFF, 0x4000_0000, 0x4000_00FF, 0x4000_0100, 0x3EAA_AAAB, 0x7E80_0000, 0x7E80_0001, 0x7EFF_FFFF,
            0x7F7F_FFFF, 0x7F80_0000, 0xFF80_0000, 0x7FC0_0001, 0x7FA0_0001, 0xBF80_0000, 0xC040_0000, 0x5A5A_5A5A,
            0x4780_0000, 0x477F_EFFF, 0x3380_0000, 0x3380_0001, 0x0000_0002,
        ],
        _ => vec![
            0,
            1 << 63,
            1,
            0x0008_0000_0000_0000,
            0x000F_FFFF_FFFF_FFFF,
            0x0010_0000_0000_0000,
            0x3FF0_0000_0000_0000,
            0x3FF0_0000_0000_0001,
            0x3FF0_000F_FFFF_FFFF,
            0x3FF0_0010_0000_0000,
            0x3FFF_FFFF_FFFF_FFFF,
            0x4000_0000_0000_0000,
            0x4000_001F_FFFF_FFFF,
            0x4000_0020_0000_0000,
            0x3FD5_5555_5555_5555,
            0x7FD0_0000_0000_0000,
            0x7FD0_0000_0000_0001,
            0x7FEF_FFFF_FFFF_FFFF,
            0x7FF0_0000_0000_0000,
            0xFFF0_0000_0000_0000,
            0x7FF8_0000_0000_0001,
            0x7FF4_0000_0000_0001,
            0xBFF0_0000_0000_0000,
            0xC008_0000_0000_0000,
            0x5A5A_5A5A_5A5A_5A5A,
            2,
        ],
    }
}

/// The critical values of a vector source.
pub fn generate_vectors(set: VectorSet) -> Vec<OperandValue> {
    let vectors: Vec<Zmm> = match set {
        VectorSet::Floats(precision) => {
            let values = list_float_values(precision);
            let lane = match precision {
                Precision::Half => Lane::Word,
                Precision::Single => Lane::Doubleword,
                _ => Lane::Quadword,
            };
            let lanes = (512 / lane.to_bits()) as usize;
            // Each rotation puts each value in the lowest element once; a packed form meets the rest in the others.
            (0..values.len())
                .map(|rotation| from_lanes(lane, &(0..lanes).map(|index| values[(index + rotation) % values.len()]).collect::<Vec<_>>()))
                .collect()
        }
        VectorSet::Elements(lane) => {
            let lanes = (512 / lane.to_bits()) as usize;
            let mask = if lane == Lane::Quadword { u64::MAX } else { (1 << lane.to_bits()) - 1 };
            let top = 1u64 << (lane.to_bits() - 1);
            let distinct: Vec<u64> = (0..lanes as u64).map(|index| (0x5AC3_3CA5_6996_0FF0u64.rotate_left(5 * index as u32) ^ index) & mask).collect();
            let signed: Vec<u64> = distinct.iter().map(|value| (value | top) & !1).collect();
            [from_lanes(lane, &distinct), from_lanes(lane, &signed), [u128::MAX; 4], [0; 4]].to_vec()
        }
        VectorSet::Singles => crate::reciprocal::generate_sources()
            .into_iter()
            .map(|value| match value {
                OperandValue::Vector(low) => [low, low.rotate_left(32), low.rotate_left(64), low.rotate_left(96)],
                other => panic!("{other:?} is no vector"),
            })
            .collect(),
    };
    vectors.into_iter().map(OperandValue::Zmm).collect()
}

/// MXCSR's values: the default, DAZ, FTZ, both, every exception unmasked, rounding up and flags already set; for a
/// half, the default and DAZ with FTZ, which it ignores.
pub fn generate_mxcsr_values(half: bool) -> Vec<OperandValue> {
    let values: &[u32] = if half {
        &[MXCSR_DEFAULT, MXCSR_DEFAULT | DAZ | FTZ]
    } else {
        &[MXCSR_DEFAULT, MXCSR_DEFAULT | DAZ, MXCSR_DEFAULT | FTZ, MXCSR_DEFAULT | DAZ | FTZ, 0, 0x5F80, MXCSR_DEFAULT | 0x3F]
    };
    values.iter().map(|&value| OperandValue::Mxcsr(value)).collect()
}

/// The segment base RDFSBASE and RDGSBASE read: zero, one in each half and one above 32 bits.
pub fn generate_bases() -> Vec<OperandValue> {
    [0, 0x0000_7FFF_1234_5678, 0xFFFF_8000_0000_1000, 0x0000_0001_8000_0001].map(OperandValue::Bits).to_vec()
}

/// MOVDIR64B's destination: each placement. UMONITOR's address: each placement through a 64-bit register, and those
/// a 32-bit register reaches through one.
pub fn generate_places(block: bool) -> Vec<OperandValue> {
    if block {
        return BLOCK_PLACEMENTS.map(|placement| OperandValue::OptionalPlace(Place::Block(placement))).to_vec();
    }
    BLOCK_PLACEMENTS
        .iter()
        .flat_map(|&placement| {
            let narrow_reaches = matches!(placement, BlockPlacement::Writable | BlockPlacement::UpperBitsSet | BlockPlacement::Misaligned);
            let sizes: &[bool] = if narrow_reaches { &[false, true] } else { &[false] };
            sizes.iter().map(move |&narrow| Place::Monitored { placement, narrow })
        })
        .map(OperandValue::OptionalPlace)
        .collect()
}

/// XBEGIN's fallback: no displacement, each way and the farthest; 16 bits that wrap; then the code at the canonical
/// edge, past which the fallback is not canonical.
pub fn generate_fallbacks() -> Vec<OperandValue> {
    let mut all: Vec<Place> =
        [0, 0x40, -0x40, 0x7FFF_FFFF].map(|displacement| Place::Fallback { displacement, short: false, at_edge: false }).to_vec();
    all.extend([0, -0x2000].map(|displacement| Place::Fallback { displacement, short: true, at_edge: false }));
    all.extend([0, -0x40].map(|displacement| Place::Fallback { displacement, short: false, at_edge: true }));
    all.into_iter().map(OperandValue::OptionalPlace).collect()
}

fn to_vector_register(bytes: u64, number: usize) -> Register {
    let first = match bytes {
        16 => Register::XMM0,
        32 => Register::YMM0,
        _ => Register::ZMM0,
    };
    Register::try_from(first as usize + number).unwrap_or_else(|_| panic!("vector register {number}"))
}

fn to_general(number: usize, wide: bool) -> Register {
    let first = if wide { Register::RAX } else { Register::EAX };
    Register::try_from(first as usize + number % 16).unwrap_or_else(|_| panic!("general register {number}"))
}

/// The address of a placement and what its page allows, `None` for a page the state leaves unmapped.
fn to_block_address(placement: BlockPlacement) -> (u64, Option<PagePermission>) {
    let writable = PagePermission::Mapped { data_access: DataAccess::ReadWrite, instruction_fetch: InstructionFetch::Forbidden };
    let read_only = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Forbidden };
    match placement {
        BlockPlacement::Writable => (BLOCK_PAGE, Some(writable)),
        BlockPlacement::Misaligned => (BLOCK_PAGE + 1, Some(writable)),
        BlockPlacement::HalfBlock => (BLOCK_PAGE + 32, Some(writable)),
        BlockPlacement::ReadOnly => (BLOCK_PAGE, Some(read_only)),
        BlockPlacement::Unmapped => (BLOCK_PAGE, None),
        BlockPlacement::Kernel => (0xFFFF_FFFF_FFE0_5000, None),
        BlockPlacement::NonCanonical => (1 << 63, None),
        BlockPlacement::UpperBitsSet => (UPPER_ADDRESS_BITS | BLOCK_PAGE, Some(writable)),
    }
}

/// Maps the page the address's low 32 bits lie on, as a 32-bit register reaches it, each byte the complement of its
/// address's low byte.
fn map_block_page(address: u64, permission: Option<PagePermission>, memory: &mut Vec<MemoryDelta<'static, X86_64>>) {
    let Some(permission) = permission else {
        return;
    };
    let page = (address & u64::from(u32::MAX)) & !(PAGE_BYTES - 1);
    let bytes: Vec<u8> = (0..PAGE_BYTES).map(|offset| !((page + offset) as u8)).collect();
    memory.push(MemoryDelta::ValueAssignment { start: page, bytes: Cow::Owned(bytes) });
    memory.push(MemoryDelta::PermissionAssignment { addresses: page..page + PAGE_BYTES, permission });
}

/// What a state's operands chose.
#[derive(Default)]
struct OptionalChoice {
    encoding: Option<EvexIntegerEncoding>,
    access: Option<crate::Access>,
    masking: Option<Writemask>,
    broadcast: bool,
    destination: Option<Zmm>,
    first: Option<Zmm>,
    source: Option<Zmm>,
    immediate: Option<u8>,
    general: Option<u64>,
    base: Option<u64>,
    place: Option<Place>,
}

/// Builds the state of a form of the family from one value of each of its operand types.
pub fn build_state(form: &OptionalForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        mxcsr: MXCSR_DEFAULT,
        ..ZERO_PROCESSOR
    };
    if form.code().op_code().encoding() == EncodingKind::EVEX {
        fill_avx512_registers(&mut processor);
    }
    let mut choice = OptionalChoice::default();
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::OptionalEncoding { .. }, OperandValue::EvexIntegerEncoding(chosen)) => choice.encoding = Some(chosen),
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => choice.access = Some(chosen),
            (OperandType::Writemask { .. }, OperandValue::Writemask(chosen)) => choice.masking = Some(chosen),
            (OperandType::LaneBroadcast, OperandValue::LaneBroadcast(chosen)) => choice.broadcast = chosen,
            (OperandType::ZmmValue(Role::Destination, _), OperandValue::Zmm(value)) => choice.destination = Some(value),
            (OperandType::ZmmValue(Role::Implicit(_), _), OperandValue::Zmm(value)) => choice.first = Some(value),
            (OperandType::ZmmValue(..) | OperandType::OptionalVector(_), OperandValue::Zmm(value)) => choice.source = Some(value),
            (OperandType::Immediate(_), OperandValue::Immediate(value)) => choice.immediate = Some(value as u8),
            (OperandType::OptionalMxcsr { .. }, OperandValue::Mxcsr(value)) => processor.mxcsr = value,
            (OperandType::RegisterValue(..), OperandValue::Bits(value)) => choice.general = Some(value),
            (OperandType::OptionalBase, OperandValue::Bits(value)) => choice.base = Some(value),
            (OperandType::OptionalPlace { .. } | OperandType::OptionalFallback, OperandValue::OptionalPlace(chosen)) => {
                choice.place = Some(chosen);
            }
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let mut memory_mutations = Vec::new();
    if let Some(Place::Fallback { displacement, short, at_edge }) = choice.place {
        processor.general_purpose_registers[0] = choice.general.expect("RAX");
        let mut bytes = if short { vec![OPERAND_SIZE_PREFIX] } else { Vec::new() };
        bytes.extend(TRANSACTION_BEGIN);
        if short {
            bytes.extend((displacement as i16).to_le_bytes());
        } else {
            bytes.extend(displacement.to_le_bytes());
        }
        let start = if at_edge { LAST_CANONICAL_PAGE + PAGE_BYTES - bytes.len() as u64 } else { CODE_ADDRESS };
        processor.instruction_pointer = start;
        return finish(processor, start & !(PAGE_BYTES - 1), start, bytes, memory_mutations);
    }
    if let Some(Writemask { mask: Some(mask), .. }) = choice.masking {
        processor.opmask_registers[WRITEMASK.number()] = mask;
    }
    let memory_operand: Option<MemoryOperand> = choice.access.map(|access| {
        let bytes = form.memory_bytes(choice.broadcast);
        let (mut operand, address) = place_memory_operand(access, bytes, &mut processor, &mut memory_mutations);
        operand.is_broadcast = choice.broadcast;
        // A load's memory holds its source; a store's keeps the data pages' bytes.
        if let (false, Some(value)) = (form.writes_memory(), choice.source) {
            let value_bytes: Vec<u8> = value.iter().flat_map(|part| part.to_le_bytes()).collect();
            let held: Vec<u8> = (0..bytes)
                .map(|offset| address.wrapping_add(offset))
                .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
                .zip(value_bytes)
                .map(|(_, byte)| byte)
                .collect();
            if !held.is_empty() {
                memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(held) });
            }
        }
        operand
    });
    let mut encoded = encode_operands(form, &choice, memory_operand, &mut processor, &mut memory_mutations);
    if let Some(Writemask { mask: Some(_), zeroing }) = choice.masking {
        encoded.set_op_mask(WRITEMASK);
        encoded.set_zeroing_masking(zeroing);
    }
    let layout = match (choice.access, choice.encoding) {
        (Some(access), _) => access.layout,
        (None, Some(chosen)) => chosen.layout,
        (None, None) => CodeLayout::PageStart,
    };
    let (code_page, start, bytes) = lay_out(layout, &encode_at_code_address(&encoded), to_padding(memory_operand));
    processor.instruction_pointer = start;
    finish(processor, code_page, start, bytes, memory_mutations)
}

/// Writes the registers the form reads and gives its encoding.
fn encode_operands(
    form: &OptionalForm,
    choice: &OptionalChoice,
    memory_operand: Option<MemoryOperand>,
    processor: &mut Processor,
    memory_mutations: &mut Vec<MemoryDelta<'static, X86_64>>,
) -> Encoded {
    let memory_register = if is_vex(form) { MEMORY_REGISTER_VEX } else { MEMORY_REGISTER_EVEX };
    let [destination, first, last] = match (choice.encoding, choice.access) {
        (Some(chosen), _) => chosen.registers,
        (None, Some(access)) => [memory_register, 2, access.register],
        (None, None) => [0, 1, 2],
    };
    let code = choice.encoding.map_or_else(|| form.code(), |chosen| form.list_codes()[chosen.code]);
    let vector = |number: usize| to_vector_register(form.vector_bytes(), number);
    let narrow = |number: usize| to_vector_register(16, number);
    let operand = || memory_operand.expect("a memory operand");
    let write = |processor: &mut Processor, number: usize, value: Option<Zmm>| {
        if let Some(value) = value {
            write_zmm(processor, number, value);
        }
    };
    let encoded = match form.operation {
        OptionalOperation::ExtractElement { .. } | OptionalOperation::StoreQuadword { .. } => {
            // The source is a store's register besides memory, or the encoding's last register.
            let source = if memory_operand.is_some() { destination } else { last };
            write(processor, source, choice.source);
            let wide = code.op_code().op_kinds().iter().any(|kind| matches!(kind, OpCodeOperandKind::r64_or_mem | OpCodeOperandKind::r64_reg));
            let target = || to_general(destination, wide);
            match (memory_operand, choice.immediate) {
                (Some(_), Some(immediate)) => Encoded::with3(code, operand(), narrow(source), u32::from(immediate)),
                (Some(_), None) => Encoded::with2(code, operand(), narrow(source)),
                (None, Some(immediate)) => Encoded::with3(code, target(), narrow(source), u32::from(immediate)),
                (None, None) => Encoded::with2(code, target(), narrow(source)),
            }
        }
        OptionalOperation::MoveScalar { .. } => {
            write(processor, destination, choice.destination);
            write(processor, first, choice.first);
            match memory_operand {
                Some(_) if form.writes_memory() => {
                    write(processor, destination, choice.source);
                    Encoded::with2(code, operand(), narrow(destination))
                }
                Some(_) => Encoded::with2(code, narrow(destination), operand()),
                None => {
                    write(processor, last, choice.source);
                    Encoded::with3(code, narrow(destination), narrow(first), narrow(last))
                }
            }
        }
        OptionalOperation::ConvertToHalves => {
            write(processor, destination, choice.source);
            Encoded::with3(code, operand(), vector(destination), u32::from(choice.immediate.unwrap_or(0)))
        }
        OptionalOperation::VexReciprocal { .. } | OptionalOperation::Reciprocal { .. } => {
            if memory_operand.is_none() {
                write(processor, last, choice.source);
            }
            match (form.is_scalar(), memory_operand) {
                (true, Some(_)) => Encoded::with3(code, narrow(destination), narrow(first), operand()),
                (true, None) => Encoded::with3(code, narrow(destination), narrow(first), narrow(last)),
                (false, Some(_)) => Encoded::with2(code, vector(destination), operand()),
                (false, None) => Encoded::with2(code, vector(destination), vector(last)),
            }
        }
        OptionalOperation::DirectStore => {
            if let Some(value) = choice.general {
                processor.general_purpose_registers[last] = value;
            }
            Encoded::with2(code, operand(), to_general(last, form.memory_bytes(false) == 8))
        }
        OptionalOperation::DirectStoreBlock => {
            let Some(Place::Block(placement)) = choice.place else { panic!("MOVDIR64B's destination") };
            let (address, permission) = to_block_address(placement);
            map_block_page(address, permission, memory_mutations);
            processor.general_purpose_registers[last] = address;
            let narrow = choice.access.is_some_and(|access| access.address_size == crate::AddressSize::Bits32);
            let code = if narrow { Code::Movdir64b_r32_m512 } else { Code::Movdir64b_r64_m512 };
            Encoded::with2(code, to_general(last, !narrow), operand())
        }
        OptionalOperation::ReadBase { gs } | OptionalOperation::WriteBase { gs } => {
            let chosen = choice.encoding.expect("the register");
            if let Some(value) = choice.base {
                if gs { processor.gs_base = value } else { processor.fs_base = value }
            }
            if let Some(value) = choice.general {
                processor.general_purpose_registers[destination % 16] = value;
            }
            Encoded::with1(code, to_general(destination, chosen.code == 1))
        }
        OptionalOperation::Monitor => {
            let Some(Place::Monitored { placement, narrow }) = choice.place else { panic!("UMONITOR's address") };
            let (address, permission) = to_block_address(placement);
            map_block_page(address, permission, memory_mutations);
            processor.general_purpose_registers[0] = address;
            let code = if narrow { Code::Umonitor_r32 } else { Code::Umonitor_r64 };
            Encoded::with1(code, to_general(0, !narrow))
        }
        OptionalOperation::TransactionBegin => panic!("XBEGIN is laid out by its fallback"),
    };
    encoded.unwrap_or_else(|error| panic!("{code:?} takes its operands: {error}"))
}

/// The state with the code's page and bytes.
fn finish(
    processor: Processor,
    code_page: u64,
    start: u64,
    bytes: Vec<u8>,
    mut memory_mutations: Vec<MemoryDelta<'static, X86_64>>,
) -> ExecutionState<'static, X86_64> {
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}
