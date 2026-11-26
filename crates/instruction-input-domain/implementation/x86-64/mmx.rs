//! The inputs of the MMX forms, derived from each form's operand kinds as iced-x86 lists them: its registers and how
//! its bytes lie (or its memory operand), the value of each operand it reads, the immediate, the x87 state MMX shares,
//! and the flags.
//!
//! An MMX value's critical values are those of legacy SSE's lanes, rotated across the 64 bits' lanes: each rotation
//! for a source, every lane count for a destination, so that each pair of lane values meets in some lane.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded, MemoryOperand, OpCodeOperandKind, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::mmx::{MmxForm, MmxOperation};
use wcontainers_processor_architecture_x86_64::vector::{Lane, LaneOperation, join, narrows};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, InstructionFetch, PagePermission, Processor, X87, X87_INITIAL, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{CodeLayout, ImmediateClass, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::vector::VectorValues;
use crate::{CODE_ADDRESS, FlagSet, GENERAL_32, OperandType, OperandValue, Role, Splits, VECTOR, ValueSet};

const MMX: [Register; 8] =
    [Register::MM0, Register::MM1, Register::MM2, Register::MM3, Register::MM4, Register::MM5, Register::MM6, Register::MM7];
/// A byte pattern whose nibbles and halves differ, for lanes whose halves must not be swapped.
const PATTERN: u64 = 0x5AA5_C33C_0FF0_9669;
/// The exponent bits every state's x87 registers start with, which an MMX write turns to all ones.
const EXPONENT: u128 = 0x4001 << 64;

/// The x87 state an MMX form meets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum X87Environment {
    /// As FNINIT leaves it.
    Initial,
    /// TOP not 0, some registers empty, condition codes and other control set, and the last instruction's pointers.
    StackInUse,
    /// An unmasked invalid operation pending: its flag, ES and B, and its mask clear.
    PendingUnmasked,
    /// A masked invalid operation's flag, which leaves nothing pending.
    MaskedFlag,
}

pub const X87_ENVIRONMENTS: [X87Environment; 4] =
    [X87Environment::Initial, X87Environment::StackInUse, X87Environment::PendingUnmasked, X87Environment::MaskedFlag];

impl X87Environment {
    /// The x87 state, before the MMX registers' values are written into its registers.
    pub fn to_x87(self) -> X87 {
        const UNMASKED_INVALID: u16 = 0x037E;
        const INVALID: u16 = 0x0001;
        // ES with B, which every chip since the 8087 keeps equal to it.
        const SUMMARY: u16 = 0x8080;
        let registers = array::from_fn(|number| EXPONENT | !(number as u128) & u128::from(u64::MAX));
        match self {
            Self::Initial => X87 { registers, ..X87_INITIAL },
            Self::StackInUse => X87 {
                registers,
                control: 0x0C7F,
                status: 0x4700 | 5 << 11,
                tags: 0b0110_1001,
                opcode: 0x05A5,
                instruction_pointer: 0x0000_1234_5678_9ABC,
                data_pointer: 0x0000_2468_ACE0_1357,
            },
            Self::PendingUnmasked => X87 { registers, control: UNMASKED_INVALID, status: SUMMARY | INVALID, ..X87_INITIAL },
            Self::MaskedFlag => X87 { registers, status: INVALID, ..X87_INITIAL },
        }
    }
}

/// What each operand of a form is, from its code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Mmx,
    Xmm,
    General,
    MmxOrMemory,
    GeneralOrMemory,
    Memory,
    Immediate,
}

fn list_slots(code: Code) -> Vec<Slot> {
    use OpCodeOperandKind as Kind;
    code.op_code()
        .op_kinds()
        .iter()
        .map(|kind| match kind {
            Kind::mm_reg | Kind::mm_rm => Slot::Mmx,
            Kind::xmm_reg | Kind::xmm_rm => Slot::Xmm,
            Kind::r32_reg | Kind::r64_reg => Slot::General,
            Kind::mm_or_mem => Slot::MmxOrMemory,
            Kind::r32_or_mem | Kind::r64_or_mem => Slot::GeneralOrMemory,
            Kind::mem => Slot::Memory,
            Kind::imm8 => Slot::Immediate,
            other => panic!("{code:?} has an operand of a kind no MMX skeleton covers: {other:?}"),
        })
        .collect()
}

/// The values the form's source and destination take: a shift's count, a narrowing's edges, or the lanes'.
fn to_values(operation: MmxOperation, every_rotation: bool) -> VectorValues {
    match operation {
        MmxOperation::Lane(operation) if narrows(operation) => VectorValues::Narrowing { lane: operation.to_lane(), every_rotation },
        MmxOperation::Lane(operation) => VectorValues::Lanes { lane: operation.to_lane(), every_rotation },
        MmxOperation::ExtractWord | MmxOperation::InsertWord => VectorValues::Lanes { lane: Lane::Word, every_rotation },
        MmxOperation::MoveMask => VectorValues::Lanes { lane: Lane::Byte, every_rotation },
        _ => VectorValues::Lanes { lane: Lane::Quadword, every_rotation },
    }
}

fn reads_destination(operation: MmxOperation) -> bool {
    match operation {
        MmxOperation::Lane(operation) => operation.reads_destination(),
        MmxOperation::InsertWord => true,
        _ => false,
    }
}

/// The operand types of an MMX form, in the order the build reads them.
pub fn list_operand_types(form: &MmxForm) -> Vec<OperandType> {
    let slots = list_slots(form.code);
    let mut types = Vec::new();
    if form.place == OperandPlace::Register {
        let registers = slots.iter().filter(|slot| !matches!(slot, Slot::Immediate | Slot::Memory)).count();
        types.push(OperandType::Encoding(registers));
    } else {
        types.push(OperandType::Memory { splits: Splits::Allowed, offset: false });
    }
    if reads_destination(form.operation) {
        types.push(OperandType::MmxValue(Role::Destination, to_values(form.operation, false)));
    }
    let shifts = matches!(form.operation, MmxOperation::Lane(operation) if operation.shifts());
    if let Some(slot) = slots.get(1).filter(|slot| **slot != Slot::Immediate) {
        types.push(match slot {
            Slot::General | Slot::GeneralOrMemory => OperandType::RegisterValue(Role::Source, ValueSet::Distinct),
            _ if shifts => OperandType::MmxValue(Role::Source, VectorValues::Count(to_operation_lane(form))),
            _ => OperandType::MmxValue(Role::Source, to_values(form.operation, true)),
        });
    }
    if slots.contains(&Slot::Immediate) {
        let counts = shifts || form.operation == MmxOperation::Lane(LaneOperation::AlignRight);
        types.push(OperandType::Immediate(if counts { ImmediateClass::Count } else { ImmediateClass::Selector }));
    }
    types.push(OperandType::X87Environment);
    types.push(OperandType::Flags(FlagSet::Unread));
    types
}

/// The lane of the form's lane operation, a quadword for any other form.
fn to_operation_lane(form: &MmxForm) -> Lane {
    match form.operation {
        MmxOperation::Lane(operation) => operation.to_lane(),
        _ => Lane::Quadword,
    }
}

/// The critical values of an MMX operand: the lane's rotations across 64 bits, then each end of its range in every
/// lane; a count's are its edges.
pub fn generate_mmx_values(values: VectorValues) -> Vec<OperandValue> {
    let to_mask = |bits: u32| if bits >= u64::BITS { u64::MAX } else { (1 << bits) - 1 };
    let (lane, every_rotation, lane_values): (Lane, bool, Vec<u64>) = match values {
        VectorValues::Count(lane) => {
            let bits = u64::from(lane.to_bits());
            return [0, 1, bits - 1, bits, 1 << u32::BITS | 1, u64::MAX].map(OperandValue::Bits).to_vec();
        }
        VectorValues::Lanes { lane, every_rotation } => {
            let bits = lane.to_bits();
            let signed_maximum = to_mask(bits - 1);
            let values = vec![0, 1, 2, u64::from(bits) - 1, u64::from(bits), signed_maximum, signed_maximum + 1, to_mask(bits), PATTERN & to_mask(bits)];
            (lane, every_rotation, values)
        }
        VectorValues::Narrowing { lane, every_rotation } => {
            let bits = lane.to_bits();
            let narrow = bits / 2;
            let narrow_maximum = to_mask(narrow - 1);
            let narrow_minimum = !narrow_maximum & to_mask(bits);
            let values = vec![
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
            ];
            (lane, every_rotation, values)
        }
        other => panic!("no MMX operand takes {other:?}"),
    };
    let count = lane_values.len();
    let bits = lane.to_bits();
    let lanes = (u64::BITS / bits) as usize;
    let step = if every_rotation { 1 } else { lanes };
    let range_ends = [0, to_mask(bits - 1), to_mask(bits - 1) + 1, to_mask(bits)];
    let fill = |value: u64| join(lane, (0..lanes).map(|_| value)) as u64;
    (0..count)
        .step_by(step)
        .map(|rotation| join(lane, (0..lanes).map(|index| lane_values[(index + rotation) % count])) as u64)
        .chain(range_ends.map(fill))
        .map(OperandValue::Bits)
        .collect()
}

/// Builds the state of an MMX form from one value of each of its operand types.
pub fn build_state(form: &MmxForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut registers, mut access, mut layout) = ([0, 0], None, CodeLayout::PageStart);
    let (mut destination, mut source, mut immediate) = (None, None, None);
    let mut environment = X87Environment::Initial;
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
            (OperandType::MmxValue(Role::Destination, _), OperandValue::Bits(value)) => destination = Some(value),
            (OperandType::MmxValue(..) | OperandType::RegisterValue(..), OperandValue::Bits(value)) => source = Some(value),
            (OperandType::Immediate(_), OperandValue::Immediate(value)) => immediate = Some(value as u8 as u32),
            (OperandType::X87Environment, OperandValue::X87(chosen)) => environment = chosen,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    processor.x87 = environment.to_x87();
    let slots = list_slots(form.code);
    // The register each slot names: the first two registers of the encoding in order, MMX's numbered modulo 8.
    let mut free = registers.into_iter();
    let mut named: Vec<Option<Register>> = Vec::new();
    for slot in &slots {
        named.push(match slot {
            Slot::Mmx => free.next().map(|number| MMX[number % MMX.len()]),
            Slot::Xmm => free.next().map(|number| VECTOR[number]),
            Slot::General => free.next().map(|number| GENERAL_32[number]),
            Slot::MmxOrMemory if access.is_none() => free.next().map(|number| MMX[number % MMX.len()]),
            Slot::GeneralOrMemory if access.is_none() => free.next().map(|number| to_general(form.code, number)),
            _ => None,
        });
    }
    if let (Some(value), Some(register)) = (destination, named.first().copied().flatten()) {
        write(&mut processor, register, value);
    }
    let mut memory_mutations = Vec::new();
    let memory_bytes = form.memory_bytes();
    let memory_operand: Option<MemoryOperand> = access.map(|access| {
        let (operand, address) = place_memory_operand(access, memory_bytes, &mut processor, &mut memory_mutations);
        if let Some(value) = source.filter(|_| form.operation != MmxOperation::Store) {
            let bytes: Vec<u8> = (0..memory_bytes)
                .map(|offset| address.wrapping_add(offset))
                .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
                .zip(value.to_le_bytes())
                .map(|(_, byte)| byte)
                .collect();
            if !bytes.is_empty() {
                memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(bytes) });
            }
        }
        operand
    });
    // A register named twice holds the source's value, the last written.
    if let (Some(value), Some(register)) = (source, named.get(1).copied().flatten()) {
        write(&mut processor, register, value);
    }
    let operands: Vec<Operand> = slots
        .iter()
        .zip(&named)
        .map(|(slot, register)| match (slot, register) {
            (_, Some(register)) => Operand::Register(*register),
            (Slot::Immediate, _) => Operand::Immediate(immediate.expect("an immediate")),
            _ => Operand::Memory(memory_operand.expect("a memory operand")),
        })
        .collect();
    let code = form.code;
    let to_error = |error| panic!("{code:?} takes its operands: {error}");
    let encoded = match *operands.as_slice() {
        [] => Ok(Encoded::with(code)),
        [Operand::Register(first)] => Encoded::with1(code, first),
        [Operand::Register(first), Operand::Register(second)] => Encoded::with2(code, first, second),
        [Operand::Register(first), Operand::Memory(second)] => Encoded::with2(code, first, second),
        [Operand::Memory(first), Operand::Register(second)] => Encoded::with2(code, first, second),
        [Operand::Register(first), Operand::Immediate(second)] => Encoded::with2(code, first, second),
        [Operand::Register(first), Operand::Register(second), Operand::Immediate(third)] => Encoded::with3(code, first, second, third),
        [Operand::Register(first), Operand::Memory(second), Operand::Immediate(third)] => Encoded::with3(code, first, second, third),
        _ => panic!("{code:?} has no such operands"),
    }
    .unwrap_or_else(to_error);
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

/// Writes a register's value: an MMX register's low 64 bits, an XMM register's low quadword with its complement above.
fn write(processor: &mut Processor, register: Register, value: u64) {
    if register.is_mm() {
        let held = &mut processor.x87.registers[register.number()];
        *held = (*held & !u128::from(u64::MAX)) | u128::from(value);
    } else if register.is_xmm() {
        processor.vector_registers[register.number()] = u128::from(value) | u128::from(!value) << 64;
    } else {
        processor.general_purpose_registers[register.full_register().number()] = value;
    }
}

#[derive(Clone, Copy)]
enum Operand {
    Register(Register),
    Immediate(u32),
    Memory(MemoryOperand),
}

/// MOVQ's general register is 64 bits, MOVD's 32.
fn to_general(code: Code, number: usize) -> Register {
    match code {
        Code::Movq_mm_rm64 | Code::Movq_rm64_mm => crate::GENERAL_64[number],
        _ => GENERAL_32[number],
    }
}
