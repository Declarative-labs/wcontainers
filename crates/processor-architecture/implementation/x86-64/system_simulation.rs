//! The skeleton of PUSHF, POPF and ENTER: each access to the stack through SS in the order the chip makes it, and RSP,
//! RBP and RFLAGS changed only once nothing faults.

use iced_x86::{Code, Instruction as Decoded, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, MemoryOperand, check_access, expect_code, is_canonical, read_value, store, write_register};
use crate::planted::{Bug, is_active};
use crate::system::{FrameProbe, SystemBehavior, SystemOperation, find_system_form};
use crate::{Access, Exception, Flags, Instruction, Processor, STACK_POINTER, X86_64};

const FRAME_POINTER: usize = 5;
/// RFLAGS' reserved bit 1 and IF, which every pushed image holds.
const ALWAYS_PUSHED: u64 = 1 << 1 | 1 << 9;
/// The flags the 16-bit POPF reaches.
const LOW_FLAGS: u64 = 0xFFFF;
/// ENTER takes its nesting level modulo this.
const NESTING_LEVELS: u8 = 32;
/// RCX, which SYSCALL writes with the next instruction's address.
const RETURN_ADDRESS: usize = 1;
/// R11, which SYSCALL writes with RFLAGS.
const SAVED_FLAGS: usize = 11;

pub(super) fn execute_system(
    instruction: Instruction,
    behavior: SystemBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_system_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no form of this family"));
    expect_code(decoded, form.codes);
    let size: u64 = if matches!(decoded.code(), Code::Pushfw | Code::Popfw | Code::Enterw_imm16_imm8) { 2 } else { 8 };
    let stack_pointer = processor.general_purpose_registers[STACK_POINTER];
    let flags = processor.flags;
    let at_stack = |address: u64| MemoryOperand { address, stack: true };
    let mut push = |processor: &Processor, address: u64, value: u64| {
        store(processor.flags, memory, at_stack(address), &value.to_le_bytes()[..size as usize], behavior.across_fault, &mut on_memory_mutated)
    };
    match form.operation {
        SystemOperation::PushFlags => {
            let always = if is_active(Bug::PushFlagsOmitsInterrupt) { 1 << 1 } else { ALWAYS_PUSHED };
            push(processor, stack_pointer.wrapping_sub(size), flags.bits() | always)?;
            processor.general_purpose_registers[STACK_POINTER] = stack_pointer.wrapping_sub(size);
        }
        SystemOperation::PopFlags => {
            check_access(flags, memory, at_stack(stack_pointer), size, Access::Read)?;
            let popped = read_value(memory, stack_pointer, size as usize);
            let reached = if size == 2 && !is_active(Bug::PopFlagsSixteenReachesUpper) { LOW_FLAGS } else { u64::MAX };
            let kept = if is_active(Bug::PopFlagsKeepsAlignmentCheck) { Flags::ALIGNMENT_CHECK.bits() } else { 0 };
            let value = (flags.bits() & !reached) | (popped & reached & !kept) | (flags.bits() & kept);
            processor.flags = Flags::from_bits_truncate(value);
            processor.general_purpose_registers[STACK_POINTER] = stack_pointer.wrapping_add(size);
        }
        SystemOperation::Enter => {
            let allocation = u64::from(decoded.immediate16());
            let level = if is_active(Bug::EnterNestingUnreduced) { decoded.immediate8_2nd() } else { decoded.immediate8_2nd() % NESTING_LEVELS };
            let mut pointer = stack_pointer.wrapping_sub(size);
            push(processor, pointer, processor.general_purpose_registers[FRAME_POINTER])?;
            let frame = pointer;
            let mut outer = processor.general_purpose_registers[FRAME_POINTER];
            for _ in 1..level {
                outer = outer.wrapping_sub(size);
                check_access(flags, memory, at_stack(outer), size, Access::Read)?;
                let copied = read_value(memory, outer, size as usize);
                pointer = pointer.wrapping_sub(size);
                push(processor, pointer, copied)?;
            }
            if level > 0 {
                pointer = pointer.wrapping_sub(size);
                push(processor, pointer, frame)?;
            }
            let last = pointer.wrapping_sub(allocation);
            if behavior.probe == FrameProbe::OperandAtFinalPointer && !is_active(Bug::EnterProbeSkipped) {
                check_access(flags, memory, at_stack(last), size, Access::Write)?;
            }
            let frame_register = if size == 2 { Register::BP } else { Register::RBP };
            write_register(processor, frame_register, frame);
            processor.general_purpose_registers[STACK_POINTER] = last;
        }
        SystemOperation::SystemCall => {
            let return_address = if is_active(Bug::SystemCallReturnsToItself) { processor.instruction_pointer } else { decoded.next_ip() };
            processor.general_purpose_registers[RETURN_ADDRESS] = return_address;
            let always = if is_active(Bug::PushFlagsOmitsInterrupt) { 1 << 1 } else { ALWAYS_PUSHED };
            processor.general_purpose_registers[SAVED_FLAGS] = flags.bits() | always;
            return Err(Exception::SystemCall);
        }
        SystemOperation::JumpIfCountZero => {
            let target = decoded.near_branch_target();
            if processor.general_purpose_registers[1] & 0xFFFF == 0 {
                if !is_canonical(target) {
                    return Err(Exception::GeneralProtection { error_code: 0 });
                }
                processor.instruction_pointer = target;
                return Ok(());
            }
        }
    }
    processor.instruction_pointer = decoded.next_ip();
    Ok(())
}
