//! The skeleton of the opmask forms.

use iced_x86::{Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, check_access, expect_code, locate, read_value, store, write_register};
use crate::behavior::StoreAcrossFault;
use crate::opmask::{OpmaskPlace, find_opmask_form, to_mask};
use crate::{Access, Exception, Flags, Instruction, Processor, X86_64};

/// The number of an opmask register, k0 to k7.
pub(super) fn to_opmask_number(register: Register) -> usize {
    register as usize - Register::K0 as usize
}

/// Reads the sources (opmask registers, a general register or memory), applies the operation and writes an opmask
/// register, a general register, memory or the flags; a store that faults part-way writes what the behavior says.
pub(super) fn execute_opmask(
    instruction: Instruction,
    across_fault: StoreAcrossFault,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_opmask_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no opmask form"));
    expect_code(decoded, &[form.code]);
    let bytes = u64::from(form.bits / 8);
    let read_mask = |processor: &Processor, operand: u32| processor.opmask_registers[to_opmask_number(decoded.op_register(operand))];
    match form.place {
        OpmaskPlace::Store => {
            let value = read_mask(processor, 1) & to_mask(form.bits);
            let operand = locate(processor, decoded);
            return store(processor.flags, memory, operand, &value.to_le_bytes()[..bytes as usize], across_fault, on_memory_mutated);
        }
        OpmaskPlace::ToGeneral => {
            let value = read_mask(processor, 1) & to_mask(form.bits);
            write_register(processor, decoded.op0_register(), value);
            return Ok(());
        }
        OpmaskPlace::Masks | OpmaskPlace::FromGeneral | OpmaskPlace::Load => {}
    }
    let source_operand = if form.operation.reads_one_source() || form.operation.writes_flags() { 1 } else { 2 };
    let last = match decoded.op_kind(source_operand) {
        OpKind::Register if form.place == OpmaskPlace::FromGeneral => {
            processor.general_purpose_registers[decoded.op_register(source_operand).full_register().number()]
        }
        OpKind::Register => read_mask(processor, source_operand),
        _ => {
            let operand = locate(processor, decoded);
            check_access(processor.flags, memory, operand, bytes, Access::Read)?;
            read_value(memory, operand.address, bytes as usize)
        }
    };
    let immediate = if decoded.op_count() > 2 && decoded.op_kind(2) == OpKind::Immediate8 { decoded.immediate8() } else { 0 };
    let (first, second) = match form.operation {
        _ if form.operation.writes_flags() => (read_mask(processor, 0), last),
        _ if form.operation.reads_one_source() => (last, 0),
        _ => (read_mask(processor, 1), last),
    };
    let result = form.operation.apply(form.bits, first, second, immediate);
    if form.operation.writes_flags() {
        processor.flags = processor.flags.difference(Flags::STATUS) | Flags::from_bits_truncate(result);
    } else {
        processor.opmask_registers[to_opmask_number(decoded.op0_register())] = result;
    }
    Ok(())
}
