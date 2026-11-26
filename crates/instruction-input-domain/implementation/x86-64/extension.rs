//! The inputs of the general-purpose extension forms, derived from each form's operand kinds as iced-x86 lists
//! them: the operand size, the registers it names (VEX.vvvv's apart, as an operand of its own) or its memory operand,
//! the value of each operand it reads, EDX for MULX, the pair CMPXCHG8B and CMPXCHG16B compare, and the flags it reads.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded, MemoryOperand, OpCodeOperandKind, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::extension::{ExtensionForm, ExtensionOperation};
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::{DataAccess, Flags, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR};

use crate::integer::{CodeLayout, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::{
    CODE_ADDRESS, Comparison, FlagSet, GENERAL_16, GENERAL_32, GENERAL_64, ImmediateClass, OperandSize, OperandSizes,
    OperandType, OperandValue, Role, Splits, ValueSet, to_accumulator,
};

/// EDX or RDX, which MULX multiplies.
const DATA: usize = 2;

/// What each operand of a form is, from its first code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    /// ModRM.reg.
    Register,
    /// VEX.vvvv.
    Vvvv,
    /// ModRM.rm: a register or memory.
    RegisterOrMemory,
    Memory,
    Immediate,
}

fn to_slot(kind: OpCodeOperandKind) -> Slot {
    use OpCodeOperandKind as Kind;
    match kind {
        Kind::r16_reg | Kind::r32_reg | Kind::r64_reg => Slot::Register,
        Kind::r32_vvvv | Kind::r64_vvvv => Slot::Vvvv,
        Kind::r16_or_mem | Kind::r32_or_mem | Kind::r64_or_mem => Slot::RegisterOrMemory,
        Kind::mem => Slot::Memory,
        Kind::imm8 => Slot::Immediate,
        other => panic!("an extension form has an operand of a kind no skeleton covers: {other:?}"),
    }
}

fn list_slots(form: &ExtensionForm) -> Vec<Slot> {
    form.codes[0].op_code().op_kinds().iter().map(|&kind| to_slot(kind)).collect()
}

/// The values the operand at the index takes when the form reads it; `None` when it does not.
fn to_value_set(operation: ExtensionOperation, index: usize) -> Option<ValueSet> {
    use ExtensionOperation as Operation;
    Some(match (operation, index) {
        (Operation::AndNot, 1) => ValueSet::Few,
        (Operation::AndNot, 2) => ValueSet::Arithmetic,
        (Operation::ExtractBitField, 2) => ValueSet::BitField,
        (Operation::ZeroHighBits | Operation::ShiftRightArithmeticWithoutFlags | Operation::ShiftLeftWithoutFlags
            | Operation::ShiftRightLogicalWithoutFlags, 2) => ValueSet::Index,
        (Operation::DepositBits | Operation::ExtractBits, 2) => ValueSet::Mask,
        (Operation::MultiplyWithoutFlags, 2) => ValueSet::Arithmetic,
        (Operation::MultiplyWithoutFlags, _) => return None,
        (Operation::AddWithCarryFlag | Operation::AddWithOverflowFlag, 0) => ValueSet::Arithmetic,
        (Operation::AddWithCarryFlag | Operation::AddWithOverflowFlag, _) => ValueSet::Few,
        (Operation::MoveSwappingBytes | Operation::CompareExchangePair, _) => ValueSet::Distinct,
        (_, 0) => return None,
        _ => ValueSet::Arithmetic,
    })
}

/// The operand types of an extension form, in the order the build reads them.
pub fn list_operand_types(form: &ExtensionForm) -> Vec<OperandType> {
    let slots = list_slots(form);
    let in_memory = form.place != OperandPlace::Register;
    let mut types = Vec::new();
    if form.codes.len() == 3 {
        types.push(OperandType::OperandSize(OperandSizes::All));
    }
    if in_memory {
        let splits = if form.place == OperandPlace::LockedMemory { Splits::Excluded } else { Splits::Allowed };
        types.push(OperandType::Memory { splits, offset: false });
    } else {
        let free = slots.iter().filter(|slot| matches!(slot, Slot::Register | Slot::RegisterOrMemory)).count();
        types.push(OperandType::Encoding(free));
    }
    for (index, slot) in slots.iter().enumerate() {
        if *slot == Slot::Vvvv {
            types.push(OperandType::Register(Role::Operand(index)));
        }
    }
    for (index, slot) in slots.iter().enumerate() {
        let Some(set) = to_value_set(form.operation, index) else {
            continue;
        };
        match slot {
            Slot::RegisterOrMemory | Slot::Memory if in_memory => types.push(OperandType::MemoryValue(set)),
            Slot::Immediate => types.push(OperandType::Immediate(ImmediateClass::Count)),
            _ => types.push(OperandType::RegisterValue(Role::Operand(index), set)),
        }
    }
    match form.operation {
        ExtensionOperation::MultiplyWithoutFlags => {
            types.push(OperandType::RegisterValue(Role::Implicit(DATA), ValueSet::Few));
        }
        // The low halves and the high halves, each equal or not.
        ExtensionOperation::CompareExchangePair => types.extend([OperandType::Comparison, OperandType::Comparison]),
        _ => {}
    }
    let read = form.operation.reads_flags();
    types.push(OperandType::Flags(if read == Flags::CARRY {
        FlagSet::Carry
    } else if read.is_empty() {
        FlagSet::Unread
    } else {
        FlagSet::Condition(read)
    }));
    types
}

/// The operand values of one state, by what they choose.
#[derive(Default)]
struct ExtensionChoice {
    size: Option<OperandSize>,
    registers: Vec<usize>,
    layout: Option<CodeLayout>,
    access: Option<crate::Access>,
    vvvv: Option<usize>,
    values: Vec<(Role, u64)>,
    memory_value: Option<u64>,
    immediate: Option<i64>,
    comparisons: Vec<Comparison>,
    flags: Option<Flags>,
}

/// Builds the state of an extension form from one value of each of its operand types.
pub fn build_state(form: &ExtensionForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut choice = ExtensionChoice::default();
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::OperandSize(_), OperandValue::OperandSize(size)) => choice.size = Some(size),
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => {
                choice.registers = encoding.registers.to_vec();
                choice.layout = Some(encoding.layout);
            }
            (OperandType::Memory { .. }, OperandValue::Access(access)) => {
                choice.registers = vec![access.register];
                choice.layout = Some(access.layout);
                choice.access = Some(access);
            }
            (OperandType::Register(_), OperandValue::Register(number)) => choice.vvvv = Some(number),
            (OperandType::RegisterValue(role, _), OperandValue::Bits(value)) => choice.values.push((*role, value)),
            (OperandType::MemoryValue(_), OperandValue::Bits(value)) => choice.memory_value = Some(value),
            (OperandType::Immediate(_), OperandValue::Immediate(value)) => choice.immediate = Some(value),
            (OperandType::Comparison, OperandValue::Comparison(comparison)) => choice.comparisons.push(comparison),
            (OperandType::Flags(_), OperandValue::Flags(flags)) => choice.flags = Some(flags),
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let code = match choice.size {
        None | Some(OperandSize::Bits16) => form.codes[0],
        Some(OperandSize::Bits32) => form.codes[1],
        Some(OperandSize::Bits64) => form.codes[2],
    };
    let width = ExtensionForm::to_memory_bytes(code) as usize;
    let slots = list_slots(form);
    let in_memory = form.place != OperandPlace::Register;
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        flags: choice.flags.unwrap_or(Flags::empty()),
        ..ZERO_PROCESSOR
    };
    let to_register = |number: usize| match width {
        2 => GENERAL_16[number],
        4 => GENERAL_32[number],
        _ => GENERAL_64[number],
    };
    // The register each operand names, in operand order; the free registers in the order of their slots.
    let mut free = choice.registers.clone();
    free.reverse();
    let named: Vec<Option<Register>> = slots
        .iter()
        .map(|slot| match slot {
            Slot::Register => free.pop().map(to_register),
            Slot::RegisterOrMemory if !in_memory => free.pop().map(to_register),
            Slot::Vvvv => choice.vvvv.map(to_register),
            _ => None,
        })
        .collect();
    for (role, value) in &choice.values {
        match role {
            Role::Operand(index) => {
                let register = named[*index].expect("a register for the value");
                processor.general_purpose_registers[register.full_register().number()] = *value;
            }
            Role::Implicit(number) => processor.general_purpose_registers[*number] = *value,
            other => panic!("no extension form reads a {other:?} register"),
        }
    }
    let mut memory_mutations = Vec::new();
    let memory_operand = in_memory.then(|| {
        let access = choice.access.expect("a memory operand");
        let (operand, address) = place_memory_operand(access, width as u64, &mut processor, &mut memory_mutations);
        if let Some(value) = choice.memory_value {
            let pair = [value.to_le_bytes(), value.swap_bytes().to_le_bytes()].concat();
            let bytes: Vec<u8> = (0..width as u64)
                .map(|offset| address.wrapping_add(offset))
                .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
                .zip(pair.iter().copied())
                .map(|(_, byte)| byte)
                .collect();
            if !bytes.is_empty() {
                memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(bytes) });
            }
            if let [low, high] = choice.comparisons[..] {
                let half = width as u64 / 2;
                let found = |offset: usize| {
                    let mut bytes = [0; 8];
                    bytes[..half as usize].copy_from_slice(&pair[offset..offset + half as usize]);
                    u64::from_le_bytes(bytes)
                };
                processor.general_purpose_registers[0] = to_accumulator(low, found(0), half);
                processor.general_purpose_registers[DATA] = to_accumulator(high, found(half as usize), half);
            }
        }
        operand
    });
    let operands: Vec<Operand> = slots
        .iter()
        .zip(&named)
        .map(|(slot, register)| match (slot, register) {
            (_, Some(register)) => Operand::Register(*register),
            (Slot::Immediate, _) => Operand::Immediate(choice.immediate.expect("an immediate")),
            _ => Operand::Memory(memory_operand.expect("a memory operand")),
        })
        .collect();
    let encoded = encode(code, &operands, form.place == OperandPlace::LockedMemory);
    let (code_page, start, bytes) = lay_out(choice.layout.unwrap_or(CodeLayout::PageStart), &encoded, to_padding(memory_operand));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}

#[derive(Clone, Copy)]
enum Operand {
    Register(Register),
    Immediate(i64),
    Memory(MemoryOperand),
}

fn encode(code: Code, operands: &[Operand], locked: bool) -> Vec<u8> {
    use Operand::{Immediate, Memory, Register};
    let to_error = |error| panic!("{code:?} takes its operands: {error}");
    let encoded = match *operands {
        [Memory(first)] => Encoded::with1(code, first),
        [Register(first), Register(second)] => Encoded::with2(code, first, second),
        [Register(first), Memory(second)] => Encoded::with2(code, first, second),
        [Memory(first), Register(second)] => Encoded::with2(code, first, second),
        [Register(first), Register(second), Register(third)] => Encoded::with3(code, first, second, third),
        [Register(first), Register(second), Memory(third)] => Encoded::with3(code, first, second, third),
        [Register(first), Memory(second), Register(third)] => Encoded::with3(code, first, second, third),
        [Register(first), Register(second), Immediate(third)] => Encoded::with3(code, first, second, third as u8 as u32),
        [Register(first), Memory(second), Immediate(third)] => Encoded::with3(code, first, second, third as u8 as u32),
        _ => panic!("{code:?} has no such operands"),
    };
    let mut instruction = encoded.unwrap_or_else(to_error);
    instruction.set_has_lock_prefix(locked);
    encode_at_code_address(&instruction)
}
