//! The skeleton of the stack forms: the operand read, the stack read or written through RSP (or RBP for LEAVE), the
//! target checked as canonical before anything is written, and RSP and RIP moved only once nothing faults.

use iced_x86::{Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{
    Memory, MemoryOperand, check_access, expect_code, is_canonical, locate, read_register, read_value, store,
    write_register,
};
use crate::planted::{Bug, is_active};
use crate::stack::{SegmentPush, StackBehavior, StackForm, StackOperation, find_stack_form};
use crate::{Access, Exception, Instruction, Processor, STACK_POINTER, X86_64};

/// The selector of SS, which Linux gives a 64-bit program and ring 3 cannot change.
const STACK_SELECTOR: u64 = 0x2B;
const RETURN_ADDRESS_BYTES: u64 = 8;
const SELECTOR_BYTES: usize = 2;

pub(super) fn execute_stack(
    instruction: Instruction,
    behavior: StackBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_stack_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no stack form"));
    expect_code(decoded, form.codes);
    let size = StackForm::to_stack_bytes(decoded.code());
    let stack_pointer = processor.general_purpose_registers[STACK_POINTER];
    let at_stack = |address: u64| MemoryOperand { address, stack: true };
    let mut next = decoded.next_ip();
    let read_operand = |processor: &Processor, width: u64| -> Result<u64, Exception> {
        match decoded.op0_kind() {
            OpKind::Register => Ok(read_register(processor, decoded.op0_register())),
            OpKind::Memory => {
                let operand = locate(processor, decoded);
                check_access(processor.flags, memory, operand, width, Access::Read)?;
                Ok(read_value(memory, operand.address, width as usize))
            }
            _ => Ok(decoded.immediate(0)),
        }
    };
    let branch = |target: u64| if is_canonical(target) || is_active(Bug::StackBranchToNonCanonical) {
        Ok(target)
    } else {
        Err(Exception::GeneralProtection { error_code: 0 })
    };
    match form.operation {
        StackOperation::Push => {
            let value = read_operand(processor, size)?;
            push(processor, memory, value, size, behavior, &mut on_memory_mutated)?;
        }
        StackOperation::PushSegment => {
            let address = stack_pointer.wrapping_sub(size);
            let written = if size == RETURN_ADDRESS_BYTES && behavior.segment_push == SegmentPush::SelectorOnly {
                SELECTOR_BYTES
            } else {
                size as usize
            };
            let selector = u64::from(read_selector(processor, decoded.op0_register()));
            store(processor.flags, memory, at_stack(address), &selector.to_le_bytes()[..written], behavior.across_fault, &mut on_memory_mutated)?;
            processor.general_purpose_registers[STACK_POINTER] = address;
        }
        StackOperation::Pop => {
            check_access(processor.flags, memory, at_stack(stack_pointer), size, Access::Read)?;
            let value = read_value(memory, stack_pointer, size as usize);
            processor.general_purpose_registers[STACK_POINTER] = stack_pointer.wrapping_add(size);
            if decoded.op0_kind() == OpKind::Memory {
                let operand = locate(processor, decoded);
                let bytes = value.to_le_bytes();
                store(processor.flags, memory, operand, &bytes[..size as usize], behavior.across_fault, &mut on_memory_mutated)?;
            } else {
                write_register(processor, decoded.op0_register(), value);
            }
        }
        StackOperation::CallRelative | StackOperation::CallIndirect => {
            let target = if form.operation == StackOperation::CallRelative {
                decoded.near_branch_target()
            } else {
                read_operand(processor, RETURN_ADDRESS_BYTES)?
            };
            // The chip pushes the return address before it checks the target, which leaves the push written.
            push(processor, memory, next, RETURN_ADDRESS_BYTES, behavior, &mut on_memory_mutated)?;
            next = branch(target)?;
        }
        StackOperation::JumpIndirect => next = branch(read_operand(processor, RETURN_ADDRESS_BYTES)?)?,
        StackOperation::Return => {
            check_access(processor.flags, memory, at_stack(stack_pointer), RETURN_ADDRESS_BYTES, Access::Read)?;
            let target = branch(read_value(memory, stack_pointer, RETURN_ADDRESS_BYTES as usize))?;
            let released = if decoded.op_count() > 0 { u64::from(decoded.immediate16()) } else { 0 };
            let released = if is_active(Bug::ReturnReleasesBeforePop) { 0 } else { released };
            processor.general_purpose_registers[STACK_POINTER] =
                stack_pointer.wrapping_add(RETURN_ADDRESS_BYTES).wrapping_add(released);
            next = target;
        }
        StackOperation::Leave => {
            let frame = processor.general_purpose_registers[5];
            check_access(processor.flags, memory, at_stack(frame), size, Access::Read)?;
            let value = read_value(memory, frame, size as usize);
            processor.general_purpose_registers[STACK_POINTER] = frame.wrapping_add(size);
            let frame_register = if size == RETURN_ADDRESS_BYTES { Register::RBP } else { Register::BP };
            write_register(processor, frame_register, value);
        }
        StackOperation::Translate => {
            let operand = locate(processor, decoded);
            check_access(processor.flags, memory, operand, 1, Access::Read)?;
            write_register(processor, Register::AL, read_value(memory, operand.address, 1));
        }
        StackOperation::MoveSegment => {
            let selector = u64::from(read_selector(processor, decoded.op1_register()));
            if decoded.op0_kind() == OpKind::Memory {
                let operand = locate(processor, decoded);
                store(processor.flags, memory, operand, &selector.to_le_bytes()[..SELECTOR_BYTES], behavior.across_fault, &mut on_memory_mutated)?;
            } else {
                write_register(processor, decoded.op0_register(), selector);
            }
        }
    }
    processor.instruction_pointer = next;
    Ok(())
}

/// The selector a segment register holds: CS's and the data segments' from the state, SS Linux's.
pub(super) fn read_selector(processor: &Processor, register: Register) -> u16 {
    match register {
        Register::CS => processor.code_segment,
        Register::SS => STACK_SELECTOR as u16,
        Register::ES => processor.data_segments[0],
        Register::DS => processor.data_segments[1],
        Register::FS => processor.data_segments[2],
        _ => processor.data_segments[3],
    }
}

/// Writes the value's `size` bytes below RSP, then lowers RSP.
fn push(
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    value: u64,
    size: u64,
    behavior: StackBehavior,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let address = processor.general_purpose_registers[STACK_POINTER].wrapping_sub(size);
    let bytes = value.to_le_bytes();
    let operand = MemoryOperand { address, stack: true };
    store(processor.flags, memory, operand, &bytes[..size as usize], behavior.across_fault, on_memory_mutated)?;
    processor.general_purpose_registers[STACK_POINTER] = address;
    Ok(())
}
