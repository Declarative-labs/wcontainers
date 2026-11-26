//! The skeleton of the x87 forms: #MF while an exception is pending for a waiting form, the memory operand read or
//! written, the stack operation with its faults, the undefined condition codes, and FIP, FOP and FDP.

use iced_x86::{Instruction as Decoded, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{
    LEGACY_PREFIXES, Memory, MemoryOperand, check_access, check_alignment, check_canonical, expect_code, is_rex, locate,
    store, write_register,
};
use crate::memory::read_bytes;
use crate::mmx::is_pending;
use crate::planted::{Bug, is_active};
use crate::x87::{
    BUSY, C1, CodeRule, EXCEPTION_FLAGS, INDEFINITE, INVALID, PointerUpdate, STACK_FAULT, SUMMARY, X87Behavior,
    X87Operation, convert_integer, examine, find_x87_form, initialize,
};
use crate::{Access, Exception, Instruction, Processor, X87, X86_64};

/// The bits of FCW FLDCW keeps: the masks, precision, rounding and infinity control; bit 6 always reads as set.
const CONTROL_KEPT: u16 = 0x1F3F;
const CONTROL_SET: u16 = 0x0040;
/// The alignment AC checks of an 80-bit operand.
const EXTENDED_ALIGNMENT: u64 = 8;
const EXTENDED_BYTES: u64 = 10;
const QUADWORD_BYTES: usize = 8;
const SIGN: u128 = 1 << 79;
/// The bits of the opcode FOP keeps: the low three of the first byte, then the ModRM byte.
const OPCODE_LOW_BITS: u8 = 0b111;

pub(super) fn execute_x87(
    instruction: Instruction,
    behavior: X87Behavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_x87_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no x87 form"));
    expect_code(decoded, &[form.code]);
    if form.waits() && is_pending(&processor.x87) && !is_active(Bug::X87IgnoresPendingException) {
        return Err(Exception::X87FloatingPoint);
    }
    let before = processor.x87;
    let mut x87 = processor.x87;
    let operand = form.has_memory_operand().then(|| locate(processor, decoded));
    let length = form.memory_bytes();
    let load = |processor: &Processor| -> Result<u128, Exception> {
        let operand = operand.expect("a memory operand");
        check_extended_alignment(processor, operand, length)?;
        check_access(processor.flags, memory, operand, length, Access::Read)?;
        let bytes: [u8; 16] = read_bytes(memory, operand.address);
        Ok(u128::from_le_bytes(bytes) & (u128::MAX >> (128 - 8 * length)))
    };
    let index = (0..decoded.op_count())
        .map(|operand| decoded.op_register(operand))
        .find(|register| register.is_st() && *register != Register::ST0 || decoded.op_count() == 1 && register.is_st())
        .map_or(0, |register| register.number());
    let mut stored: Option<u128> = None;
    match form.operation {
        X87Operation::NoOperation | X87Operation::Wait | X87Operation::LegacyNoOperation => {}
        X87Operation::ClearExceptions => x87.status &= !(EXCEPTION_FLAGS | STACK_FAULT | SUMMARY | BUSY),
        X87Operation::Initialize => initialize(&mut x87),
        X87Operation::StoreStatus | X87Operation::StoreControl => {
            let value = if form.operation == X87Operation::StoreStatus { x87.status } else { x87.control };
            match operand {
                Some(_) => stored = Some(u128::from(value)),
                None => write_register(processor, Register::AX, u64::from(value)),
            }
        }
        X87Operation::LoadControl => {
            let kept = if is_active(Bug::LoadControlKeepsEveryBit) { u16::MAX } else { CONTROL_KEPT };
            x87.control = load(processor)? as u16 & kept | CONTROL_SET;
            // ES and B follow the flags the new masks leave unmasked.
            let pending = x87.is_unmasked(x87.status);
            x87.set_code(SUMMARY | BUSY, pending);
        }
        X87Operation::IncrementTop | X87Operation::DecrementTop => {
            let step = if form.operation == X87Operation::IncrementTop { 1 } else { 7 };
            x87.set_top(x87.top() + step);
            x87.set_code(C1, false);
        }
        X87Operation::Free { pop } => {
            x87.free(index);
            if pop {
                x87.pop();
            }
        }
        X87Operation::Exchange => {
            let (first, second) = (x87.is_empty(0), x87.is_empty(index));
            if (first || second) && !underflow(&mut x87) {
                // Unmasked, nothing is exchanged.
            } else {
                let value = |empty: bool, index: usize| if empty { INDEFINITE } else { x87.read(index) };
                let (top, other) = (value(first, 0), value(second, index));
                x87.write(0, other);
                x87.write(index, top);
                x87.set_code(C1, false);
            }
        }
        X87Operation::LoadRegister => {
            // An empty source is an underflow before a full stack is an overflow.
            if x87.is_empty(index) && !is_active(Bug::X87OverflowBeforeUnderflow) {
                if underflow(&mut x87) {
                    x87.set_top(x87.top() + 7);
                    x87.write(0, INDEFINITE);
                }
            } else {
                let value = (!x87.is_empty(index)).then(|| x87.read(index));
                push(&mut x87, value);
            }
        }
        X87Operation::LoadConstant(value) => push(&mut x87, Some(value)),
        X87Operation::LoadExtended => {
            let value = load(processor)?;
            push(&mut x87, Some(value));
        }
        X87Operation::LoadInteger => {
            let raw = load(processor)? as u64;
            let unused = 64 - 8 * length as u32;
            let value = ((raw << unused) as i64) >> unused;
            push(&mut x87, Some(convert_integer(value)));
        }
        X87Operation::StoreRegister { pop } => {
            if !x87.is_empty(0) || underflow(&mut x87) {
                let value = if x87.is_empty(0) { INDEFINITE } else { x87.read(0) };
                x87.write(index, value);
                if pop {
                    x87.pop();
                }
                x87.set_code(C1, false);
            }
        }
        X87Operation::StoreRegisterUnchecked => {
            if !x87.is_empty(0) {
                x87.write(index, x87.read(0));
            }
            x87.pop();
            x87.set_code(C1, false);
        }
        X87Operation::StoreExtended => {
            if !x87.is_empty(0) || underflow(&mut x87) {
                stored = Some(if x87.is_empty(0) { INDEFINITE } else { x87.read(0) });
                x87.pop();
                x87.set_code(C1, false);
            }
        }
        X87Operation::ChangeSign | X87Operation::Absolute => {
            if x87.is_empty(0) {
                // Masked, the underflow leaves the indefinite QNaN as it is.
                if underflow(&mut x87) {
                    x87.write(0, INDEFINITE);
                }
            } else {
                let value = x87.read(0);
                let changed = if form.operation == X87Operation::ChangeSign { value ^ SIGN } else { value & !SIGN };
                x87.write(0, changed);
                x87.set_code(C1, false);
            }
        }
        X87Operation::MoveIf(condition) => {
            if x87.is_empty(0) || x87.is_empty(index) {
                if underflow(&mut x87) {
                    x87.write(0, INDEFINITE);
                }
            } else if condition.holds(processor.flags) {
                x87.write(0, x87.read(index));
            }
        }
        X87Operation::Examine => {
            let codes = examine(x87.read(0), x87.is_empty(0));
            x87.status = (x87.status & !(crate::x87::C0 | crate::x87::C2 | crate::x87::C3)) | codes;
            x87.set_code(C1, x87.read(0) & SIGN != 0);
        }
    }
    if let (Some(value), Some(operand)) = (stored, operand) {
        check_extended_alignment(processor, operand, length)?;
        let bytes = value.to_le_bytes();
        let mut on_memory_mutated = on_memory_mutated;
        // The chip stores 80 bits as eight bytes, then two, so a fault in the last two leaves the first eight written.
        let split = if length == EXTENDED_BYTES && !is_active(Bug::ExtendedStoreWhole) { QUADWORD_BYTES } else { length as usize };
        store(processor.flags, memory, operand, &bytes[..split], behavior.across_fault, &mut on_memory_mutated)?;
        if split < length as usize {
            let rest = MemoryOperand { address: operand.address.wrapping_add(split as u64), ..operand };
            store(processor.flags, memory, rest, &bytes[split..length as usize], behavior.across_fault, &mut on_memory_mutated)?;
        }
    }
    let undefined = form.list_undefined_codes();
    for (codes, rule) in [(undefined & C1, behavior.codes.c1), (undefined & !C1, behavior.codes.others)] {
        let kept = match rule {
            CodeRule::Unchanged => before.status & codes,
            CodeRule::Cleared => 0,
        };
        x87.status = (x87.status & !codes) | kept;
    }
    if !form.is_control() && !is_active(Bug::X87InstructionPointerKept) {
        let unmasked = x87.is_unmasked(x87.status & !before.status);
        x87.instruction_pointer = processor.instruction_pointer;
        if behavior.opcode == PointerUpdate::Always || unmasked {
            x87.opcode = read_opcode(memory, processor.instruction_pointer);
        }
        if let Some(operand) = operand.filter(|_| behavior.data == PointerUpdate::Always || unmasked) {
            x87.data_pointer = operand.address;
        }
    }
    processor.x87 = x87;
    Ok(())
}

/// #AC for an 80-bit operand not aligned to 8 while AC is set, which the integer check leaves to wider accesses;
/// after a first byte that is not canonical, as for every access.
fn check_extended_alignment(processor: &Processor, operand: MemoryOperand, length: u64) -> Result<(), Exception> {
    if length != EXTENDED_BYTES || is_active(Bug::ExtendedAlignmentUnchecked) {
        return Ok(());
    }
    check_canonical(operand, 1)?;
    check_alignment(processor.flags, operand, EXTENDED_ALIGNMENT)
}

/// Pushes the value, or with `None` reports the underflow of reading an empty register: a stack fault either way
/// where ST(7) is in use, which masked pushes the indefinite QNaN.
fn push(x87: &mut X87, value: Option<u128>) {
    let overflow = !x87.is_empty(7) && !is_active(Bug::X87PushIgnoresOverflow);
    if overflow {
        if fault(x87, true) {
            x87.set_top(x87.top() + 7);
            x87.write(0, INDEFINITE);
        }
        return;
    }
    let Some(value) = value.or_else(|| underflow(x87).then_some(INDEFINITE)) else {
        return;
    };
    x87.set_top(x87.top() + 7);
    x87.write(0, value);
    x87.set_code(C1, false);
}

/// Reports a stack underflow, and whether IE is masked, so that the form goes on with the indefinite QNaN.
fn underflow(x87: &mut X87) -> bool {
    fault(x87, false)
}

fn fault(x87: &mut X87, overflow: bool) -> bool {
    x87.raise(INVALID | STACK_FAULT);
    x87.set_code(C1, overflow);
    !x87.is_unmasked(INVALID)
}

/// FOP's value for the form at the address: the low three bits of the opcode byte after the prefixes, then the next.
fn read_opcode(memory: Memory<'_, '_>, address: u64) -> u16 {
    let bytes: [u8; 15] = read_bytes(memory, address);
    let start = bytes.iter().take_while(|byte| LEGACY_PREFIXES.contains(byte) || is_rex(**byte)).count();
    u16::from(bytes[start] & OPCODE_LOW_BITS) << 8 | u16::from(bytes.get(start + 1).copied().unwrap_or(0))
}
