//! The inputs of the x87 forms: how the code lies (or the memory operand), ST(i)'s index, the x87 state around the
//! operands, ST(0)'s and ST(i)'s values, the value in memory, and the flags FCMOVcc reads.
//!
//! A register's critical values are every class FXAM tells apart, each sign of zero and infinity, a QNaN and an
//! SNaN, the indefinite QNaN, the formats no x87 operation makes (pseudo-denormal, unnormal, pseudo-infinity and
//! pseudo-NaN), and empty.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded, MemoryOperand, OpCodeOperandKind, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::x87::{Source, X87Form, X87Operation};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, InstructionFetch, PagePermission, Processor, X87, X87_INITIAL, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{CodeLayout, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::{CODE_ADDRESS, FlagSet, OperandType, OperandValue, Role, Splits, ValueSet};

/// The value that marks a register empty, above the 80 bits a register holds.
pub const EMPTY: u128 = 1 << 127;
/// The bits every state's x87 registers start with above 64.
const EXPONENT: u128 = 0x4001 << 64;
const ST: [Register; 8] =
    [Register::ST0, Register::ST1, Register::ST2, Register::ST3, Register::ST4, Register::ST5, Register::ST6, Register::ST7];
/// ST(i)'s critical indices: ST(0) itself, the next, and the last.
pub const STACK_INDICES: [usize; 3] = [1, 7, 0];

/// The register values, by class: in a register, empty among them.
const REGISTER_VALUES: [u128; 16] = [
    EMPTY,
    0,
    1 << 79,
    0x3FFF_8000_0000_0000_0000,
    0xBFFF_C000_0000_0000_0000,
    0x7FFE_FFFF_FFFF_FFFF_FFFF,
    0x0001_8000_0000_0000_0000,
    0x0000_0000_0000_0000_0001,
    0x0000_8000_0000_0000_0000,
    0x7FFF_8000_0000_0000_0000,
    0xFFFF_8000_0000_0000_0000,
    0x7FFF_C000_0000_0000_1234,
    0x7FFF_A000_0000_0000_5678,
    0xFFFF_C000_0000_0000_0000,
    0x4000_4000_0000_0000_0000,
    0x7FFF_0000_0000_0000_0001,
];

/// ST(0)'s values for FIST: each side of a half between integers, either sign, each end of the integer ranges,
/// tiny ones, and the classes it cannot round.
const ROUNDING_VALUES: [u128; 22] = [
    EMPTY,
    0,
    0x3FFE_8000_0000_0000_0000,
    0x3FFF_C000_0000_0000_0000,
    0x4000_A000_0000_0000_0000,
    0xC000_A000_0000_0000_0000,
    0x3FFF_A000_0000_0000_0000,
    0xBFFE_C000_0000_0000_0000,
    0x400D_FFFF_0000_0000_0000,
    0xC00E_8000_8000_0000_0000,
    0x400E_8000_0000_0000_0000,
    0x401D_FFFF_FFFF_0000_0000,
    0xC01E_8000_0000_0000_0000,
    0x403E_8000_0000_0000_0000,
    0xC03E_8000_0000_0000_0000,
    0x403D_FFFF_FFFF_FFFF_F800,
    0x3F9B_8000_0000_0000_0000,
    0x7FFF_8000_0000_0000_0000,
    0x7FFF_C000_0000_0000_1234,
    0x0000_0000_0000_0000_0001,
    0x4000_4000_0000_0000_0000,
    0xFFFF_8000_0000_0000_0000,
];

/// Where an x87 value lies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum X87ValueSet {
    /// A register, which may be empty.
    Register,
    /// Memory, which holds no empty.
    Memory,
    /// ST(0) of FIST: the values about the integers.
    Rounding,
}

/// The x87 state around a form's operands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum X87Stack {
    /// As FNINIT leaves it.
    Initial,
    /// Every register in use, TOP 3, so a push overflows.
    Full,
    /// An unmasked invalid operation pending.
    Pending,
    /// Every exception unmasked and none pending, TOP 6 with ST(7) in use, so a push overflows unmasked.
    Unmasked,
    /// The condition codes and a masked precision flag set, other precision and rounding toward zero, half the
    /// registers in use, and the last instruction's pointers.
    CodesSet,
    /// Every exception unmasked and none pending, every register empty, so a push meets the source's own exceptions.
    UnmaskedEmpty,
    /// Rounding down.
    RoundDown,
    /// Rounding up.
    RoundUp,
}

pub const X87_STACKS: [X87Stack; 8] = [
    X87Stack::Initial,
    X87Stack::Full,
    X87Stack::Pending,
    X87Stack::Unmasked,
    X87Stack::UnmaskedEmpty,
    X87Stack::CodesSet,
    X87Stack::RoundDown,
    X87Stack::RoundUp,
];

impl X87Stack {
    pub fn to_x87(self) -> X87 {
        let registers = array::from_fn(|number| EXPONENT | !(number as u128) & u128::from(u64::MAX));
        let initial = X87 { registers, ..X87_INITIAL };
        match self {
            Self::Initial => initial,
            Self::Full => X87 { status: 3 << 11, tags: 0xFF, ..initial },
            Self::Pending => X87 { control: 0x037E, status: 0x8081, ..initial },
            Self::Unmasked => X87 { control: 0x0340, status: 6 << 11, tags: 1 << 5, ..initial },
            Self::CodesSet => X87 {
                control: 0x0E7F,
                status: 0x4720 | 1 << 11,
                tags: 0b1010_0101,
                opcode: 0x05A5,
                instruction_pointer: 0x0000_1234_5678_9ABC,
                data_pointer: 0x0000_2468_ACE0_1357,
                ..initial
            },
            Self::UnmaskedEmpty => X87 { control: 0x0340, ..initial },
            Self::RoundDown => X87 { control: 0x077F, ..initial },
            Self::RoundUp => X87 { control: 0x0B7F, ..initial },
        }
    }
}

/// The critical values of an x87 value.
pub fn generate_x87_values(set: X87ValueSet) -> Vec<OperandValue> {
    let values: &[u128] = match set {
        X87ValueSet::Register => &REGISTER_VALUES,
        X87ValueSet::Memory => &REGISTER_VALUES[1..],
        X87ValueSet::Rounding => &ROUNDING_VALUES,
    };
    values.iter().map(|&value| OperandValue::Vector(value)).collect()
}

/// What each operand of a form is, from its code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Top,
    Indexed,
    Accumulator,
    Memory,
}

fn list_slots(code: Code) -> Vec<Slot> {
    use OpCodeOperandKind as Kind;
    code.op_code()
        .op_kinds()
        .iter()
        .map(|kind| match kind {
            Kind::st0 => Slot::Top,
            Kind::sti_opcode => Slot::Indexed,
            Kind::ax => Slot::Accumulator,
            Kind::mem => Slot::Memory,
            other => panic!("{code:?} has an operand of a kind no x87 skeleton covers: {other:?}"),
        })
        .collect()
}

const fn reads_top(operation: X87Operation) -> bool {
    matches!(
        operation,
        X87Operation::Exchange
            | X87Operation::StoreRegister { .. }
            | X87Operation::ChangeSign
            | X87Operation::Absolute
            | X87Operation::MoveIf(_)
            | X87Operation::Examine
            | X87Operation::StoreExtended
            | X87Operation::Compare { .. }
    )
}

const fn reads_indexed(operation: X87Operation) -> bool {
    matches!(
        operation,
        X87Operation::Exchange
            | X87Operation::LoadRegister
            | X87Operation::MoveIf(_)
            | X87Operation::Compare { source: Source::Register, .. }
    )
}

/// The operand types of an x87 form, in the order the build reads them.
pub fn list_operand_types(form: &X87Form) -> Vec<OperandType> {
    let slots = list_slots(form.code);
    let mut types = vec![if form.place == OperandPlace::Register {
        OperandType::Encoding(0)
    } else {
        OperandType::Memory { splits: Splits::Allowed, offset: false }
    }];
    if slots.contains(&Slot::Indexed) {
        types.push(OperandType::StackRegister);
    }
    types.push(OperandType::X87Stack);
    if reads_top(form.operation) {
        types.push(OperandType::X87Value(Role::Destination, X87ValueSet::Register));
    }
    if matches!(form.operation, X87Operation::StoreInteger { .. }) {
        types.push(OperandType::X87Value(Role::Destination, X87ValueSet::Rounding));
    }
    if reads_indexed(form.operation) {
        types.push(OperandType::X87Value(Role::Source, X87ValueSet::Register));
    }
    match form.operation {
        X87Operation::LoadExtended => types.push(OperandType::X87Value(Role::Source, X87ValueSet::Memory)),
        X87Operation::LoadInteger => types.push(OperandType::MemoryValue(ValueSet::Arithmetic)),
        X87Operation::LoadControl => types.push(OperandType::MemoryValue(ValueSet::ControlWord)),
        X87Operation::LoadFloat | X87Operation::Compare { source: Source::Single | Source::Double, .. } => {
            let set = if form.memory_bytes() == 4 { ValueSet::SingleFloat } else { ValueSet::DoubleFloat };
            types.push(OperandType::MemoryValue(set));
        }
        X87Operation::Compare { source: Source::Integer, .. } => types.push(OperandType::MemoryValue(ValueSet::Arithmetic)),
        _ => {}
    }
    types.push(OperandType::Flags(match form.operation {
        X87Operation::MoveIf(condition) => FlagSet::Condition(condition.reads()),
        _ => FlagSet::Unread,
    }));
    types
}

/// Builds the state of an x87 form from one value of each of its operand types.
pub fn build_state(form: &X87Form, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut access, mut layout, mut index) = (None, CodeLayout::PageStart, form.to_implied_index());
    let (mut stack, mut top, mut indexed, mut memory_value) = (X87Stack::Initial, None, None, None);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => layout = encoding.layout,
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => {
                layout = chosen.layout;
                access = Some(chosen);
            }
            (OperandType::StackRegister, OperandValue::Register(number)) => index = number,
            (OperandType::X87Stack, OperandValue::X87Stack(chosen)) => stack = chosen,
            (OperandType::X87Value(Role::Destination, _), OperandValue::Vector(value)) => top = Some(value),
            (OperandType::X87Value(_, X87ValueSet::Register), OperandValue::Vector(value)) => indexed = Some(value),
            (OperandType::X87Value(_, X87ValueSet::Memory), OperandValue::Vector(value)) => memory_value = Some(value),
            (OperandType::MemoryValue(_), OperandValue::Bits(value)) => memory_value = Some(u128::from(value)),
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    processor.x87 = stack.to_x87();
    // ST(i)'s value comes last, so that ST(0) named twice holds it.
    for (slot, value) in [(0, top), (index, indexed)] {
        if let Some(value) = value {
            let physical = processor.x87.to_physical(slot);
            if value == EMPTY {
                processor.x87.tags &= !(1 << physical);
            } else {
                processor.x87.registers[physical] = value;
                processor.x87.tags |= 1 << physical;
            }
        }
    }
    let mut memory_mutations = Vec::new();
    let memory_bytes = form.memory_bytes();
    let memory_operand: Option<MemoryOperand> = access.map(|access| {
        let (operand, address) = place_memory_operand(access, memory_bytes, &mut processor, &mut memory_mutations);
        if let Some(value) = memory_value {
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
    let code = form.code;
    let slots = list_slots(code);
    let to_error = |error| panic!("{code:?} takes its operands: {error}");
    let encoded = match slots.as_slice() {
        [] => Ok(Encoded::with(code)),
        [Slot::Memory] => Encoded::with1(code, memory_operand.expect("a memory operand")),
        [Slot::Indexed] => Encoded::with1(code, ST[index]),
        [Slot::Accumulator] => Encoded::with1(code, Register::AX),
        [Slot::Top, Slot::Indexed] => Encoded::with2(code, Register::ST0, ST[index]),
        [Slot::Indexed, Slot::Top] => Encoded::with2(code, ST[index], Register::ST0),
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
