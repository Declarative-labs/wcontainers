//! The skeleton of the MMX forms: #MF while an x87 exception is pending, then the source read from an MMX, XMM or
//! general register, memory or the immediate, the result written, and the x87 state put in MMX's use.

use iced_x86::{Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, check_access, expect_code, locate, read_register, read_value, store, write_register};
use crate::mmx::{MmxBehavior, MmxOperation, apply_lanes, enter_mmx, find_mmx_form, is_pending};
use crate::planted::{Bug, is_active};
use crate::{Access, Exception, Instruction, Processor, X86_64};

const WORD_MASK: u64 = 0xFFFF;
const TOP_MASK: u16 = 0b111 << 11;

pub(super) fn execute_mmx(
    instruction: Instruction,
    behavior: MmxBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_mmx_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no MMX form"));
    expect_code(decoded, &[form.code]);
    if is_pending(&processor.x87) && !is_active(Bug::MmxIgnoresPendingException) {
        return Err(Exception::X87FloatingPoint);
    }
    if form.operation == MmxOperation::EmptyState {
        processor.x87.tags = if is_active(Bug::EmptyStateMarksValid) { u8::MAX } else { 0 };
        processor.x87.status &= !(0b111 << 11);
        return Ok(());
    }
    let length = form.memory_bytes();
    let read = |processor: &Processor, index: u32| -> Result<u64, Exception> {
        Ok(match decoded.op_kind(index) {
            OpKind::Register => read_any(processor, decoded.op_register(index)),
            OpKind::Memory => {
                let operand = locate(processor, decoded);
                check_access(processor.flags, memory, operand, length, Access::Read)?;
                read_value(memory, operand.address, length as usize)
            }
            _ => u64::from(decoded.immediate8()),
        })
    };
    let immediate = if decoded.op_count() > 2 { decoded.immediate8() } else { 0 };
    let written = match form.operation {
        MmxOperation::Lane(operation) => {
            let destination = read_any(processor, decoded.op0_register());
            let source = read(processor, 1)?;
            Some(apply_lanes(operation, destination, source, immediate))
        }
        MmxOperation::Load | MmxOperation::MoveFromVector => Some(read(processor, 1)?),
        MmxOperation::InsertWord => {
            let word = read(processor, 1)? & WORD_MASK;
            let shift = 16 * u32::from(immediate & 3);
            let kept = read_any(processor, decoded.op0_register()) & !(WORD_MASK << shift);
            Some(kept | word << shift)
        }
        MmxOperation::Store => {
            let value = read(processor, 1)?;
            match decoded.op0_kind() {
                OpKind::Memory => {
                    let operand = locate(processor, decoded);
                    let bytes = value.to_le_bytes();
                    // The chip makes TOP 0 before the store, which keeps it 0 when the store faults.
                    if !is_active(Bug::MmxStoreFaultKeepsTop) {
                        processor.x87.status &= !TOP_MASK;
                    }
                    store(processor.flags, memory, operand, &bytes[..length as usize], behavior.across_fault, on_memory_mutated)?;
                    None
                }
                _ if decoded.op0_register().is_mm() => Some(value),
                _ => {
                    write_register(processor, decoded.op0_register(), value);
                    None
                }
            }
        }
        MmxOperation::ExtractWord => {
            let word = read(processor, 1)? >> (16 * u32::from(immediate & 3)) & WORD_MASK;
            write_register(processor, decoded.op0_register(), word);
            None
        }
        MmxOperation::MoveMask => {
            let value = read(processor, 1)?;
            let mask = (0..8).fold(0, |mask, byte| mask | (value >> (8 * byte + 7) & 1) << byte);
            write_register(processor, decoded.op0_register(), mask);
            None
        }
        MmxOperation::MoveToVector => {
            let value = read(processor, 1)?;
            processor.vector_registers[decoded.op0_register().number()] = u128::from(value);
            None
        }
        MmxOperation::EmptyState => unreachable!("EMMS returned above"),
    };
    let written = written.map(|value| (decoded.op0_register().number(), value));
    if is_active(Bug::MmxKeepsTop) {
        let top = processor.x87.status & TOP_MASK;
        enter_mmx(&mut processor.x87, written);
        processor.x87.status |= top;
    } else {
        enter_mmx(&mut processor.x87, written);
    }
    if let Some((number, value)) = written.filter(|_| is_active(Bug::MmxWriteKeepsExponent)) {
        processor.x87.registers[number] = u128::from(value);
    }
    Ok(())
}

/// The value of an MMX register (the low 64 bits of its x87 register), an XMM register's low quadword, or a general
/// register.
fn read_any(processor: &Processor, register: Register) -> u64 {
    if register.is_mm() {
        processor.x87.registers[register.number()] as u64
    } else if register.is_xmm() {
        processor.vector_registers[register.number()] as u64
    } else {
        read_register(processor, register)
    }
}
