//! The skeleton of the string forms: the count tested, one element read and written with the source's fault first,
//! the status flags of a comparison, RSI, RDI and the count moved, and the instruction pointer left at the form
//! while the repetition goes on.

use iced_x86::{Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, MemoryOperand, check_access, expect_code, read_value, store, write_register};
use crate::planted::{Bug, is_active};
use crate::string::{Repetition, StringBehavior, StringOperation, find_string_form};
use crate::{Access, Exception, Flags, Instruction, Processor, X86_64};

const COUNT: usize = 1;
const SOURCE_INDEX: usize = 6;
const DESTINATION_INDEX: usize = 7;
const ACCUMULATORS: [Register; 4] = [Register::AL, Register::AX, Register::EAX, Register::RAX];

pub(super) fn execute_string(
    instruction: Instruction,
    behavior: StringBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_string_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no string form"));
    expect_code(decoded, &[form.code]);
    let narrow = (0..decoded.op_count()).any(|operand| matches!(decoded.op_kind(operand), OpKind::MemorySegESI | OpKind::MemoryESEDI));
    let mask = if narrow { u64::from(u32::MAX) } else { u64::MAX };
    if matches!(form.operation, StringOperation::Input | StringOperation::Output) {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    // Under the 0x67 prefix a repeated form first writes ECX, and MOVS and STOS their pointers, as 32-bit registers,
    // which stays so even where the iteration then faults or does not run.
    if form.is_repeated() && narrow && !is_active(Bug::StringNarrowKeepsUpperHalves) {
        let normalized: &[usize] = match form.operation {
            StringOperation::Move => &[COUNT, SOURCE_INDEX, DESTINATION_INDEX],
            StringOperation::Store => &[COUNT, DESTINATION_INDEX],
            _ => &[COUNT],
        };
        for &number in normalized {
            processor.general_purpose_registers[number] &= mask;
        }
    }
    let registers = processor.general_purpose_registers;
    let count = registers[COUNT] & mask;
    if form.is_repeated() && count == 0 && !is_active(Bug::RepeatCountZeroRuns) {
        processor.instruction_pointer = decoded.next_ip();
        return Ok(());
    }
    let segment_base = match decoded.memory_segment() {
        _ if is_active(Bug::StringSourceSegmentIgnored) => 0,
        Register::FS => processor.fs_base,
        Register::GS => processor.gs_base,
        _ => 0,
    };
    let source = MemoryOperand {
        address: segment_base.wrapping_add(registers[SOURCE_INDEX] & mask),
        stack: decoded.memory_segment() == Register::SS,
    };
    let destination = MemoryOperand { address: registers[DESTINATION_INDEX] & mask, stack: false };
    let bytes = form.bytes;
    let accumulator = ACCUMULATORS[bytes.trailing_zeros() as usize];
    let element_mask = if bytes == 8 { u64::MAX } else { (1 << (8 * bytes)) - 1 };
    let flags = processor.flags;
    let mut compared = None;
    match form.operation {
        StringOperation::Move => {
            if is_active(Bug::MoveChecksDestinationFirst) {
                check_access(flags, memory, destination, bytes, Access::Write)?;
            }
            check_access(flags, memory, source, bytes, Access::Read)?;
            let value = read_value(memory, source.address, bytes as usize);
            store(flags, memory, destination, &value.to_le_bytes()[..bytes as usize], behavior.across_fault, &mut on_memory_mutated)?;
        }
        StringOperation::Store => {
            let value = registers[0];
            store(flags, memory, destination, &value.to_le_bytes()[..bytes as usize], behavior.across_fault, &mut on_memory_mutated)?;
        }
        StringOperation::Load => {
            check_access(flags, memory, source, bytes, Access::Read)?;
            write_register(processor, accumulator, read_value(memory, source.address, bytes as usize));
        }
        StringOperation::Scan => {
            check_access(flags, memory, destination, bytes, Access::Read)?;
            compared = Some((registers[0] & element_mask, read_value(memory, destination.address, bytes as usize)));
        }
        StringOperation::Compare => {
            // The chip reads the destination element first.
            let order = if is_active(Bug::CompareReadsSourceFirst) { [source, destination] } else { [destination, source] };
            for operand in order {
                check_access(flags, memory, operand, bytes, Access::Read)?;
            }
            compared = Some((read_value(memory, source.address, bytes as usize), read_value(memory, destination.address, bytes as usize)));
        }
        StringOperation::Input | StringOperation::Output => unreachable!("a port form faults first"),
    }
    let compared_flags = compared.map(|(first, second)| {
        let (first, second) = if is_active(Bug::StringCompareOperandsSwapped) { (second, first) } else { (first, second) };
        compare(first, second, bytes, processor.flags)
    });
    let backward = flags.contains(Flags::DIRECTION) && !is_active(Bug::StringDirectionIgnored);
    let step = if backward { bytes.wrapping_neg() } else { bytes };
    let write_index = |processor: &mut Processor, number: usize, value: u64| {
        let kept = if narrow && is_active(Bug::StringNarrowKeepsUpperHalves) { registers[number] & !mask } else { 0 };
        processor.general_purpose_registers[number] = kept | (value & mask);
    };
    if form.reads_source() {
        write_index(processor, SOURCE_INDEX, registers[SOURCE_INDEX].wrapping_add(step));
    }
    if form.reaches_destination() {
        write_index(processor, DESTINATION_INDEX, registers[DESTINATION_INDEX].wrapping_add(step));
    }
    let mut done = true;
    if form.is_repeated() {
        let left = count.wrapping_sub(1) & mask;
        write_index(processor, COUNT, left);
        let equal = compared_flags.is_some_and(|flags| flags.contains(Flags::ZERO));
        let ended_by_comparison = match form.repetition {
            Repetition::WhileEqual if is_active(Bug::RepeatWhileEqualReversed) => equal,
            Repetition::WhileEqual => !equal,
            Repetition::WhileNotEqual => equal,
            Repetition::Once | Repetition::Repeat => false,
        };
        done = left == 0 || ended_by_comparison || is_active(Bug::RepeatEndsAfterOneIteration);
    }
    // A comparison that leaves the repetition going on leaves the status flags as they were, as the chip does.
    if let Some(flags) = compared_flags.filter(|_| done || is_active(Bug::RepeatFlagsWrittenEachIteration)) {
        processor.flags = flags;
    }
    if done {
        processor.instruction_pointer = decoded.next_ip();
    }
    Ok(())
}

/// The status flags of CMP of the two elements, the first less the second.
fn compare(first: u64, second: u64, bytes: u64, flags: Flags) -> Flags {
    let top = 1u64 << (8 * bytes - 1);
    let mask = if bytes == 8 { u64::MAX } else { (1 << (8 * bytes)) - 1 };
    let result = first.wrapping_sub(second) & mask;
    let mut changed = flags.difference(Flags::STATUS);
    changed.set(Flags::CARRY, first < second);
    changed.set(Flags::ZERO, result == 0);
    changed.set(Flags::SIGN, result & top != 0);
    changed.set(Flags::OVERFLOW, (first ^ second) & (first ^ result) & top != 0);
    let nibble_carry = if is_active(Bug::StringCompareAuxiliaryFromBitThree) { 0x08 } else { 0x10 };
    changed.set(Flags::AUXILIARY_CARRY, (first ^ second ^ result) & nibble_carry != 0);
    changed.set(Flags::PARITY, (result as u8).count_ones().is_multiple_of(2));
    changed
}
