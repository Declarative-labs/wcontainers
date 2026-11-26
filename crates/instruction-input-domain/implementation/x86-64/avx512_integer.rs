//! The inputs of the EVEX integer forms: the vector registers they name and how their bytes lie (or a memory
//! operand and the registers besides it), the writemask, whether memory is a broadcast lane, each source's value, an
//! immediate and the flags.
//!
//! The writemask's critical values: none (k0), and k1 holding the lanes alternating, every lane but the second (with
//! every bit above the lanes set, which the form ignores) or no lane, each merging and zeroing (only merging for a
//! store, and none for a compare, which writes an opmask register). A vector's critical values are the legacy
//! skeleton's lane values rotated across all 512 bits, so that the lanes past a shorter vector's length hold values
//! too, which the form must ignore. Every state's registers hold distinct values in all 512 bits, so that a lane
//! kept, merged or zeroed is seen to be.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Instruction as Encoded, MemoryOperand, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::avx512_integer::{
    EvexIntegerForm, EvexIntegerOperation, EvexIntegerPlace, EvexVectorLength, Zmm, from_lanes, write_zmm,
};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::vector::Lane;
use wcontainers_processor_architecture_x86_64::{
    DataAccess, InstructionFetch, PagePermission, Processor, VECTOR_REGISTERS, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{CodeLayout, ImmediateClass, LAYOUTS_BEYOND_START, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::opmask::fill_opmask_registers;
use crate::{CODE_ADDRESS, FlagSet, OperandType, OperandValue, Role, Splits, ValueSet};

/// A pattern whose nibbles and halves differ.
const PATTERN: u64 = 0x5AA5_C33C_0FF0_9669;
/// What a shift count's other bits hold, which the shift ignores.
const COUNT_UPPER: u64 = 0xA5A5_A5A5_A5A5_A5A5;
/// The opmask register every writemask of a state is.
const WRITEMASK: usize = 1;
const LANE_COUNT_VALUES: usize = 8;

/// The vector registers an EVEX form names, by number (the destination's taken modulo 8 when it is an opmask
/// register, a general source's modulo 16), which of its codes encodes it, and how its bytes lie.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexIntegerEncoding {
    /// The destination, the first source (VEX.vvvv) and the last source.
    pub registers: [usize; 3],
    pub code: usize,
    pub layout: CodeLayout,
}

/// The writemask a state uses: none (k0), or k1 with its value, merging or zeroing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Writemask {
    pub mask: Option<u64>,
    pub zeroing: bool,
}

/// What a vector operand's critical values are made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZmmValues {
    /// The lane's edges, as the legacy skeleton's lanes have them, rotated: every rotation, or those a vector's lane
    /// count apart; then each end of the lane's signed and unsigned range in every lane.
    Lanes { lane: Lane, every_rotation: bool },
    /// A shift count in the low quadword: zero, one, the width less one, the width, one with a bit above 32 set,
    /// and all ones; every other bit a pattern.
    Count(Lane),
    /// Per-lane counts: zero, one, the width less one, the width and one more, the top bit alone, all ones and a
    /// count past the width by its own width plus one, every rotation.
    LaneCounts(Lane),
    /// VPTERNLOG's: the byte repeated, so that the three operands' bytes 0xF0, 0xCC and 0xAA meet in every one of
    /// the eight combinations in each lane; the last operand also takes lanes that differ, for broadcasts.
    Ternary(u8),
}

/// Distinct low registers, the destination and both sources high (EVEX's R', V' and X bits), and one register for
/// all, whose merged lanes are then a source's.
const REGISTER_CHOICES: [[usize; 3]; 3] = [[0, 1, 2], [17, 26, 31], [5, 5, 5]];
/// For a memory form: a high destination (EVEX's R') and a low first source.
const MEMORY_REGISTER_CHOICES: [[usize; 3]; 1] = [[17, 2, 0]];

/// The operand types of an EVEX integer form, in the order the build reads them.
pub fn list_operand_types(form: &EvexIntegerForm) -> Vec<OperandType> {
    let lane = form.lane;
    let in_memory = matches!(form.place, EvexIntegerPlace::Memory | EvexIntegerPlace::Store);
    let mut types = Vec::new();
    if in_memory {
        types.push(OperandType::Memory { splits: Splits::Allowed, offset: false });
    }
    types.push(OperandType::EvexIntegerEncoding { codes: form.codes.len(), in_memory });
    types.push(OperandType::Writemask { zeroing: !(form.writes_mask() || form.place == EvexIntegerPlace::Store) });
    if form.place == EvexIntegerPlace::Memory && form.can_broadcast() {
        types.push(OperandType::LaneBroadcast);
    }
    let lanes = |every_rotation| ZmmValues::Lanes { lane, every_rotation };
    let (destination, first, last) = match form.operation {
        EvexIntegerOperation::TernaryLogic => (Some(ZmmValues::Ternary(0xF0)), Some(ZmmValues::Ternary(0xCC)), ZmmValues::Ternary(0xAA)),
        EvexIntegerOperation::ShiftByCount(_) => (None, Some(lanes(true)), ZmmValues::Count(lane)),
        EvexIntegerOperation::ShiftByLane(_) | EvexIntegerOperation::Rotate { by_lane: true, .. } => {
            (None, Some(lanes(true)), ZmmValues::LaneCounts(lane))
        }
        _ if form.has_first_source() => (None, Some(lanes(false)), lanes(true)),
        _ => (None, None, lanes(true)),
    };
    types.extend(destination.map(|values| OperandType::ZmmValue(Role::Destination, values)));
    types.extend(first.map(|values| OperandType::ZmmValue(Role::Implicit(1), values)));
    if form.place == EvexIntegerPlace::General {
        types.push(OperandType::RegisterValue(Role::Source, ValueSet::Arithmetic));
    } else {
        types.push(OperandType::ZmmValue(Role::Source, last));
    }
    let class = match form.operation {
        EvexIntegerOperation::TernaryLogic => Some(ImmediateClass::TernaryTable),
        EvexIntegerOperation::CompareIntoMask(_) => Some(ImmediateClass::Predicate),
        _ if form.has_immediate() => Some(ImmediateClass::Count),
        _ => None,
    };
    types.extend(class.filter(|_| form.has_immediate()).map(OperandType::Immediate));
    types.push(OperandType::Flags(FlagSet::Unread));
    types
}

/// Every choice of registers with every code at the page's start, then the first choice with every other layout;
/// for a memory form, whose memory operand gives the layout, one choice of registers.
pub fn generate_encodings(codes: usize, in_memory: bool) -> Vec<OperandValue> {
    let choices: &[[usize; 3]] = if in_memory { &MEMORY_REGISTER_CHOICES } else { &REGISTER_CHOICES };
    let at_start = choices
        .iter()
        .flat_map(|&registers| (0..codes).map(move |code| EvexIntegerEncoding { registers, code, layout: CodeLayout::PageStart }));
    let layouts: &[CodeLayout] = if in_memory { &[] } else { &LAYOUTS_BEYOND_START };
    let laid_out = layouts.iter().map(|&layout| EvexIntegerEncoding { registers: choices[0], code: 0, layout });
    at_start.chain(laid_out).map(OperandValue::EvexIntegerEncoding).collect()
}

/// No writemask, then k1 with each value, merging and, where the form may zero, zeroing.
pub fn generate_maskings(zeroing: bool) -> Vec<OperandValue> {
    let values = [0x5555_5555_5555_5555, !2, 0];
    let modes: &[bool] = if zeroing { &[false, true] } else { &[false] };
    [Writemask { mask: None, zeroing: false }]
        .into_iter()
        .chain(values.iter().flat_map(|&mask| modes.iter().map(move |&zeroing| Writemask { mask: Some(mask), zeroing })))
        .map(OperandValue::Writemask)
        .collect()
}

const fn to_mask(bits: u32) -> u64 {
    if bits >= u64::BITS { u64::MAX } else { (1 << bits) - 1 }
}

/// The critical values of a vector operand.
pub fn generate_zmm_values(values: ZmmValues) -> Vec<OperandValue> {
    let rotated = |lane: Lane, lane_values: &[u64], step: usize| -> Vec<Zmm> {
        let lanes = (512 / lane.to_bits()) as usize;
        (0..lane_values.len())
            .step_by(step)
            .map(|rotation| from_lanes(lane, &(0..lanes).map(|index| lane_values[(index + rotation) % lane_values.len()]).collect::<Vec<_>>()))
            .collect()
    };
    let fill = |lane: Lane, value: u64| from_lanes(lane, &vec![value; (512 / lane.to_bits()) as usize]);
    let vectors: Vec<Zmm> = match values {
        ZmmValues::Lanes { lane, every_rotation } => {
            let bits = lane.to_bits();
            let signed_maximum = to_mask(bits - 1);
            let lane_values =
                [0, 1, 2, u64::from(bits) - 1, u64::from(bits), signed_maximum, signed_maximum + 1, to_mask(bits), PATTERN & to_mask(bits)];
            let step = if every_rotation { 1 } else { (512 / bits) as usize };
            let mut all = rotated(lane, &lane_values, step);
            all.extend([0, signed_maximum, signed_maximum + 1, to_mask(bits)].map(|end| fill(lane, end)));
            all
        }
        ZmmValues::Count(lane) => {
            let bits = u64::from(lane.to_bits());
            [0, 1, bits - 1, bits, 1 << u32::BITS | 1, u64::MAX]
                .map(|count| {
                    let upper = u128::from(COUNT_UPPER);
                    [u128::from(count) | (upper << 64), upper | (upper << 64), u128::from(PATTERN), u128::from(PATTERN) << 64]
                })
                .to_vec()
        }
        ZmmValues::LaneCounts(lane) => {
            let bits = u64::from(lane.to_bits());
            let lane_values: [u64; LANE_COUNT_VALUES] = [0, 1, bits - 1, bits, bits + 1, 1 << (bits - 1), to_mask(lane.to_bits()), 2 * bits + 1];
            rotated(lane, &lane_values, 1)
        }
        ZmmValues::Ternary(byte) => {
            let repeated = u128::from_le_bytes([byte; 16]);
            let mut all = vec![[repeated; 4]];
            if byte == 0xAA {
                all.extend(rotated(Lane::Doubleword, &[PATTERN as u32 as u64, 0, u64::from(u32::MAX), 0x1234_5678], 1).into_iter().take(1));
            }
            all
        }
    };
    vectors.into_iter().map(OperandValue::Zmm).collect()
}

/// The values a state's whole ZMM register starts with: its XMM and YMM parts as every state's, and the bits above
/// rotations of the register's complement.
pub fn fill_zmm(number: usize) -> Zmm {
    let complement = !(number as u128);
    [complement, complement.rotate_left(u64::BITS), complement.rotate_left(u32::BITS), complement.rotate_left(96)]
}

/// Fills the bits above YMM, ZMM16 to ZMM31 and the opmask registers with the values every EVEX state starts with.
pub fn fill_avx512_registers(processor: &mut Processor) {
    for number in 0..2 * VECTOR_REGISTERS {
        let mut value = fill_zmm(number);
        if number < VECTOR_REGISTERS {
            value[0] = processor.vector_registers[number];
            value[1] = processor.upper_vector_halves[number];
        }
        write_zmm(processor, number, value);
    }
    processor.opmask_registers = fill_opmask_registers();
}

fn to_vector_register(length: EvexVectorLength, number: usize) -> Register {
    let first = match length {
        EvexVectorLength::L128 => Register::XMM0,
        EvexVectorLength::L256 => Register::YMM0,
        EvexVectorLength::L512 => Register::ZMM0,
    };
    Register::try_from(first as usize + number).unwrap_or_else(|_| panic!("vector register {number}"))
}

fn to_opmask_register(number: usize) -> Register {
    Register::try_from(Register::K0 as usize + number % 8).unwrap_or_else(|_| panic!("opmask register {number}"))
}

/// Builds the state of an EVEX integer form from one value of each of its operand types.
pub fn build_state(form: &EvexIntegerForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    fill_avx512_registers(&mut processor);
    let (mut encoding, mut access, mut masking, mut broadcast) = (None, None, Writemask { mask: None, zeroing: false }, false);
    let (mut destination, mut first, mut last, mut general, mut immediate) = (None, None, None, None, None);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::EvexIntegerEncoding { .. }, OperandValue::EvexIntegerEncoding(chosen)) => encoding = Some(chosen),
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => access = Some(chosen),
            (OperandType::Writemask { .. }, OperandValue::Writemask(chosen)) => masking = chosen,
            (OperandType::LaneBroadcast, OperandValue::LaneBroadcast(chosen)) => broadcast = chosen,
            (OperandType::ZmmValue(Role::Destination, _), OperandValue::Zmm(value)) => destination = Some(value),
            (OperandType::ZmmValue(Role::Implicit(_), _), OperandValue::Zmm(value)) => first = Some(value),
            (OperandType::ZmmValue(_, _), OperandValue::Zmm(value)) => last = Some(value),
            (OperandType::RegisterValue(..), OperandValue::Bits(value)) => general = Some(value),
            (OperandType::Immediate(_), OperandValue::Immediate(value)) => immediate = Some(value),
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let encoding = encoding.expect("the registers");
    let [destination_number, first_number, last_number] = encoding.registers;
    if let Some(mask) = masking.mask {
        processor.opmask_registers[WRITEMASK] = mask;
    }
    // A register named twice holds the value of its last operand.
    if let Some(value) = destination {
        write_zmm(&mut processor, destination_number, value);
    }
    if let Some(value) = first {
        write_zmm(&mut processor, first_number, value);
    }
    let in_memory = access.is_some();
    if let (Some(value), false) = (last, in_memory) {
        write_zmm(&mut processor, last_number, value);
    }
    if let Some(value) = general {
        processor.general_purpose_registers[last_number % 16] = value;
    }
    let mut memory_mutations = Vec::new();
    let memory_operand: Option<MemoryOperand> = access.map(|access| {
        let bytes = form.memory_bytes(broadcast);
        let (mut operand, address) = place_memory_operand(access, bytes, &mut processor, &mut memory_mutations);
        operand.is_broadcast = broadcast;
        let value = last.expect("the memory's value");
        let value_bytes: Vec<u8> = value.iter().flat_map(|quarter| quarter.to_le_bytes()).collect();
        let held: Vec<u8> = (0..bytes)
            .map(|offset| address.wrapping_add(offset))
            .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
            .zip(value_bytes)
            .map(|(_, byte)| byte)
            .collect();
        if !held.is_empty() && form.place != EvexIntegerPlace::Store {
            memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(held) });
        }
        operand
    });
    // A store's source is its value operand's register, the first source's number.
    if let (EvexIntegerPlace::Store, Some(value)) = (form.place, last) {
        write_zmm(&mut processor, first_number, value);
    }
    let length = form.length;
    let vector = |number: usize| to_vector_register(length, number);
    let narrow = |number: usize| to_vector_register(EvexVectorLength::L128, number);
    let code = form.codes[encoding.code];
    let to_error = |error| panic!("{code:?} takes its operands: {error}");
    let target = if form.writes_mask() { to_opmask_register(destination_number) } else { vector(destination_number) };
    let last_register = match form.operation {
        _ if form.place == EvexIntegerPlace::General => {
            let general = if form.lane == Lane::Quadword { Register::RAX } else { Register::EAX };
            Register::try_from(general as usize + last_number % 16).unwrap_or_else(|_| panic!("general register {last_number}"))
        }
        EvexIntegerOperation::ShiftByCount(_) | EvexIntegerOperation::Broadcast => narrow(last_number),
        _ => vector(last_number),
    };
    let immediate = immediate.map(|value| value as u8 as u32);
    let mut encoded = match (form.place, memory_operand, form.has_first_source(), immediate) {
        (EvexIntegerPlace::Store, Some(operand), _, _) => Encoded::with2(code, operand, vector(first_number)),
        (_, Some(operand), true, Some(immediate)) => Encoded::with4(code, target, vector(first_number), operand, immediate),
        (_, Some(operand), true, None) => Encoded::with3(code, target, vector(first_number), operand),
        (_, Some(operand), false, Some(immediate)) => Encoded::with3(code, target, operand, immediate),
        (_, Some(operand), false, None) => Encoded::with2(code, target, operand),
        (_, None, true, Some(immediate)) => Encoded::with4(code, target, vector(first_number), last_register, immediate),
        (_, None, true, None) => Encoded::with3(code, target, vector(first_number), last_register),
        (_, None, false, Some(immediate)) => Encoded::with3(code, target, last_register, immediate),
        (_, None, false, None) => Encoded::with2(code, target, last_register),
    }
    .unwrap_or_else(to_error);
    if masking.mask.is_some() {
        encoded.set_op_mask(to_opmask_register(WRITEMASK));
        encoded.set_zeroing_masking(masking.zeroing);
    }
    let layout = match access {
        Some(access) => access.layout,
        None => encoding.layout,
    };
    let (code_page, start, bytes) = lay_out(layout, &encode_at_code_address(&encoded), to_padding(memory_operand));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}
