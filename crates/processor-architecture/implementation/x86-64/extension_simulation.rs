//! The skeleton of the extension forms: the sources read from registers, memory or an immediate in the order the
//! operation names them, the result written to the first operand (and MULX's low half to the second), or for MOVBE
//! to memory the result stored; CMPXCHG8B and CMPXCHG16B read, compare and store their pair.

use iced_x86::{Code, Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, check_access, expect_code, locate, read_register, read_value, store, write_register};
use crate::extension::{ExtensionBehavior, ExtensionForm, ExtensionOperation, compute, find_extension_form};
use crate::memory::read_bytes;
use crate::planted::{Bug, is_active};
use crate::{Access, Exception, Flags, Instruction, Processor, X86_64};

/// CMPXCHG16B's operand must be aligned to 16, or it raises #GP(0).
const PAIR_ALIGNMENT: u64 = 16;
const ACCUMULATOR: usize = 0;
const COUNTER: usize = 1;
const DATA: usize = 2;
const BASE: usize = 3;

pub(super) fn execute_extension(
    instruction: Instruction,
    behavior: ExtensionBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    use ExtensionOperation as Operation;
    let form = find_extension_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no extension form"));
    expect_code(decoded, form.codes);
    let length = ExtensionForm::to_memory_bytes(decoded.code()) as usize;
    if form.operation == Operation::CompareExchangePair {
        return exchange_pair(length, behavior, processor, memory, decoded, on_memory_mutated);
    }
    let read = |index: u32| -> Result<u64, Exception> {
        match decoded.op_kind(index) {
            OpKind::Register => Ok(read_register(processor, decoded.op_register(index))),
            OpKind::Memory => {
                let operand = locate(processor, decoded);
                check_access(processor.flags, memory, operand, length as u64, Access::Read)?;
                Ok(read_value(memory, operand.address, length))
            }
            _ => Ok(u64::from(decoded.immediate8())),
        }
    };
    let sources = match form.operation {
        Operation::AndNot
        | Operation::ExtractBitField
        | Operation::ZeroHighBits
        | Operation::DepositBits
        | Operation::ExtractBits
        | Operation::RotateRightWithoutFlags
        | Operation::ShiftRightArithmeticWithoutFlags
        | Operation::ShiftLeftWithoutFlags
        | Operation::ShiftRightLogicalWithoutFlags => [read(1)?, read(2)?],
        Operation::MultiplyWithoutFlags => [read(2)?, 0],
        Operation::AddWithCarryFlag | Operation::AddWithOverflowFlag => [read(0)?, read(1)?],
        _ => [read(1)?, 0],
    };
    let data = processor.general_purpose_registers[DATA];
    let outcome = compute(form.operation, sources, data, length, processor.flags, behavior.undefined);
    if decoded.op0_kind() == OpKind::Memory {
        let operand = locate(processor, decoded);
        let bytes = outcome.result.to_le_bytes();
        return store(processor.flags, memory, operand, &bytes[..length], behavior.across_fault, on_memory_mutated);
    }
    let low_wins = is_active(Bug::MultiplyWithoutFlagsLowWins);
    if let Some(low) = outcome.low.filter(|_| !low_wins) {
        write_register(processor, decoded.op1_register(), low);
    }
    let destination = decoded.op0_register();
    if is_active(Bug::MoveSwappingBytesKeepsUpperHalf) && form.operation == Operation::MoveSwappingBytes && length == 4 {
        let full = &mut processor.general_purpose_registers[destination.full_register().number()];
        *full = (*full & !u64::from(u32::MAX)) | outcome.result;
    } else {
        write_register(processor, destination, outcome.result);
    }
    if let Some(low) = outcome.low.filter(|_| low_wins) {
        write_register(processor, decoded.op1_register(), low);
    }
    processor.flags = outcome.flags;
    Ok(())
}

/// CMPXCHG8B and CMPXCHG16B on a pair of `length` bytes: the access checked as a write before it is read, as a locked
/// read-modify-write is, and stored whether or not the comparison holds.
fn exchange_pair(
    length: usize,
    behavior: ExtensionBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let operand = locate(processor, decoded);
    let aligned = decoded.code() != Code::Cmpxchg16b_m128
        || operand.address.is_multiple_of(PAIR_ALIGNMENT)
        || is_active(Bug::CompareExchangePairAlignmentUnchecked);
    if !aligned {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    check_access(processor.flags, memory, operand, length as u64, Access::Write).or_else(|exception| match exception {
        // A write that faults part-way is the store's to report, with what it writes below the fault.
        Exception::PageFault { .. } => Ok(()),
        other => Err(other),
    })?;
    let half = length / 2;
    let found: [u8; 16] = read_bytes(memory, operand.address);
    let (found_low, found_high) = (read_half(&found[..half]), read_half(&found[half..length]));
    let registers = processor.general_purpose_registers;
    let half_mask = crate::integer::to_mask(half);
    let (expected_low, expected_high) = (registers[ACCUMULATOR] & half_mask, registers[DATA] & half_mask);
    let high_compared = !is_active(Bug::CompareExchangePairComparesLowHalf);
    let equal = expected_low == found_low && (expected_high == found_high || !high_compared);
    let stored: [u64; 2] = if equal {
        [registers[BASE] & half_mask, registers[COUNTER] & half_mask]
    } else {
        [found_low, found_high]
    };
    let mut bytes = [0; 16];
    bytes[..half].copy_from_slice(&stored[0].to_le_bytes()[..half]);
    bytes[half..length].copy_from_slice(&stored[1].to_le_bytes()[..half]);
    store(processor.flags, memory, operand, &bytes[..length], behavior.across_fault, on_memory_mutated)?;
    let width = if half == 4 { [Register::EAX, Register::EDX] } else { [Register::RAX, Register::RDX] };
    if !equal || is_active(Bug::CompareExchangePairWritesAccumulatorWhenEqual) {
        let [accumulator, data] = width;
        let loaded = if equal { [expected_low, expected_high] } else { [found_low, found_high] };
        write_register(processor, accumulator, loaded[0]);
        write_register(processor, data, loaded[1]);
    }
    processor.flags.set(Flags::ZERO, equal);
    Ok(())
}

fn read_half(bytes: &[u8]) -> u64 {
    let mut value = [0; 8];
    value[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(value)
}
