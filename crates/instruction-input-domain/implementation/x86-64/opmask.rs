//! The inputs of the opmask forms: the registers they name and how their bytes lie (or a memory operand), each
//! opmask source's value, a general register's or memory's value for KMOV, KSHIFT's count and the flags.
//!
//! An opmask value's critical values, for a width of w bits: zero, one, the top bit alone, the w bits set, the bits
//! alternating both ways, every bit set (above w too, which the form ignores), only the bits above w set, and a
//! pattern whose halves differ (KUNPCK's halves). KADD's carries out of w come from the top bit and all ones.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Instruction as Encoded, MemoryOperand, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::opmask::{OpmaskForm, OpmaskOperation, OpmaskPlace, to_mask};
use wcontainers_processor_architecture_x86_64::{DataAccess, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR};

use crate::integer::{CodeLayout, ImmediateClass, LAYOUTS_BEYOND_START, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::{CODE_ADDRESS, FlagSet, OperandType, OperandValue, Role, Splits, ValueSet};

/// A pattern whose bytes and halves differ.
const PATTERN: u64 = 0x0123_4567_89AB_CDEF;
const OPMASK: [Register; 8] =
    [Register::K0, Register::K1, Register::K2, Register::K3, Register::K4, Register::K5, Register::K6, Register::K7];
const GENERAL_32: [Register; 16] = [
    Register::EAX, Register::ECX, Register::EDX, Register::EBX, Register::ESP, Register::EBP,
    Register::ESI, Register::EDI, Register::R8D, Register::R9D, Register::R10D, Register::R11D,
    Register::R12D, Register::R13D, Register::R14D, Register::R15D,
];
const GENERAL_64: [Register; 16] = [
    Register::RAX, Register::RCX, Register::RDX, Register::RBX, Register::RSP, Register::RBP,
    Register::RSI, Register::RDI, Register::R8, Register::R9, Register::R10, Register::R11,
    Register::R12, Register::R13, Register::R14, Register::R15,
];

/// The register numbers an opmask form names, in operand order (an opmask register's taken modulo 8), and how its
/// bytes lie.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OpmaskEncoding {
    pub registers: [usize; 3],
    pub layout: CodeLayout,
}

/// Distinct registers, all one register, the first two one register, the first and last one, and registers that
/// need VEX's R and B bits where the operand is a general register.
const REGISTER_CHOICES: [[usize; 3]; 5] = [[1, 2, 3], [0, 0, 0], [7, 7, 6], [4, 5, 4], [13, 14, 5]];

/// The operand types of an opmask form, in the order the build reads them.
pub fn list_operand_types(form: &OpmaskForm) -> Vec<OperandType> {
    let value = |operand: usize| OperandType::OpmaskValue { operand, bits: form.to_source_bits() };
    let memory = OperandType::Memory { splits: Splits::Allowed, offset: false };
    let unread = OperandType::Flags(FlagSet::Unread);
    match form.place {
        OpmaskPlace::Masks => match form.operation {
            OpmaskOperation::OrTest | OpmaskOperation::Test => vec![OperandType::OpmaskEncoding(2), value(0), value(1), unread],
            OpmaskOperation::ShiftLeft | OpmaskOperation::ShiftRight => {
                vec![OperandType::OpmaskEncoding(2), value(1), OperandType::Immediate(ImmediateClass::Count), unread]
            }
            OpmaskOperation::Not | OpmaskOperation::Move => vec![OperandType::OpmaskEncoding(2), value(1), unread],
            _ => vec![OperandType::OpmaskEncoding(3), value(1), value(2), unread],
        },
        OpmaskPlace::ToGeneral => vec![OperandType::OpmaskEncoding(2), value(1), unread],
        OpmaskPlace::FromGeneral => {
            vec![OperandType::OpmaskEncoding(2), OperandType::RegisterValue(Role::Source, ValueSet::Arithmetic), unread]
        }
        OpmaskPlace::Load => vec![memory, OperandType::MemoryValue(ValueSet::Arithmetic), unread],
        OpmaskPlace::Store => vec![memory, value(1), unread],
    }
}

/// Every choice of registers at the page's start, then the first with every other layout.
pub fn generate_encodings() -> Vec<OperandValue> {
    let at_start = REGISTER_CHOICES.iter().map(|&registers| OpmaskEncoding { registers, layout: CodeLayout::PageStart });
    let laid_out = LAYOUTS_BEYOND_START.map(|layout| OpmaskEncoding { registers: REGISTER_CHOICES[0], layout });
    at_start.chain(laid_out).map(OperandValue::OpmaskEncoding).collect()
}

/// The critical values of an opmask source of `bits` bits.
pub fn generate_values(bits: u32) -> Vec<OperandValue> {
    let mask = to_mask(bits);
    let above = if bits >= u64::BITS { PATTERN.rotate_left(32) } else { !mask };
    // The pattern second, as the value a state takes where the data decides nothing: it differs from every data byte.
    [0, PATTERN, 1, 1 << (bits - 1), mask, 0x5555_5555_5555_5555 & mask, 0xAAAA_AAAA_AAAA_AAAA & mask, u64::MAX, above]
        .map(OperandValue::Bits)
        .to_vec()
}

/// The values every state's opmask registers start with: distinct patterns, k0's too, none all zeros or all ones.
pub fn fill_opmask_registers() -> [u64; 8] {
    array::from_fn(|number| 0x5A5A_C3C3_0F0F_9696 ^ (number as u64).wrapping_mul(0x0101_0101_0101_0101))
}

/// Builds the state of an opmask form from one value of each of its operand types.
pub fn build_state(form: &OpmaskForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    processor.opmask_registers = fill_opmask_registers();
    let (mut encoding, mut access, mut immediate, mut memory_value) = (None, None, None, None);
    let mut values: Vec<(usize, u64)> = Vec::new();
    let mut general = None;
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::OpmaskEncoding(_), OperandValue::OpmaskEncoding(chosen)) => encoding = Some(chosen),
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => access = Some(chosen),
            (OperandType::OpmaskValue { operand, .. }, OperandValue::Bits(value)) => values.push((*operand, value)),
            (OperandType::RegisterValue(..), OperandValue::Bits(value)) => general = Some(value),
            (OperandType::MemoryValue(_), OperandValue::Bits(value)) => memory_value = Some(value),
            (OperandType::Immediate(_), OperandValue::Immediate(value)) => immediate = Some(value),
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let registers = encoding.map_or([access.map_or(1, |access| access.register % 8), 0, 0], |encoding| encoding.registers);
    let wide = form.bits == u64::BITS;
    let to_general = |number: usize| if wide { GENERAL_64[number] } else { GENERAL_32[number] };
    // The operands in order, opmask registers by their number modulo 8.
    let named: Vec<Register> = match form.place {
        OpmaskPlace::ToGeneral => vec![to_general(registers[0]), OPMASK[registers[1] % 8]],
        OpmaskPlace::FromGeneral => vec![OPMASK[registers[0] % 8], to_general(registers[1])],
        OpmaskPlace::Load | OpmaskPlace::Store => vec![OPMASK[registers[0] % 8]],
        OpmaskPlace::Masks => {
            let count = if matches!(form.operation, OpmaskOperation::OrTest | OpmaskOperation::Test)
                || form.operation.reads_one_source()
            {
                2
            } else {
                3
            };
            registers[..count].iter().map(|number| OPMASK[number % 8]).collect()
        }
    };
    // A register named twice holds the value of its last operand.
    for (operand, value) in values {
        let register = if form.place == OpmaskPlace::Store { named[0] } else { named[operand] };
        processor.opmask_registers[register as usize - Register::K0 as usize] = value;
    }
    if let Some(value) = general {
        processor.general_purpose_registers[named[1].full_register().number()] = value;
    }
    let mut memory_mutations = Vec::new();
    let bytes = u64::from(form.bits / 8);
    let memory_operand: Option<MemoryOperand> = access.map(|access| {
        let (operand, address) = place_memory_operand(access, bytes, &mut processor, &mut memory_mutations);
        if let Some(value) = memory_value {
            let held: Vec<u8> = (0..bytes)
                .map(|offset| address.wrapping_add(offset))
                .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
                .zip(value.to_le_bytes())
                .map(|(_, byte)| byte)
                .collect();
            if !held.is_empty() {
                memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(held) });
            }
        }
        operand
    });
    let code = form.code;
    let to_error = |error| panic!("{code:?} takes its operands: {error}");
    let encoded = match (form.place, memory_operand) {
        (OpmaskPlace::Store, Some(operand)) => Encoded::with2(code, operand, named[0]),
        (_, Some(operand)) => Encoded::with2(code, named[0], operand),
        (_, None) => match (named.len(), immediate) {
            (2, Some(immediate)) => Encoded::with3(code, named[0], named[1], immediate as u8 as u32),
            (2, None) => Encoded::with2(code, named[0], named[1]),
            _ => Encoded::with3(code, named[0], named[1], named[2]),
        },
    }
    .unwrap_or_else(to_error);
    let layout = match (access, encoding) {
        (Some(access), _) => access.layout,
        (None, Some(encoding)) => encoding.layout,
        (None, None) => CodeLayout::PageStart,
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
