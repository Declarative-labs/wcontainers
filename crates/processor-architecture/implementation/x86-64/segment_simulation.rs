//! The skeleton of the segment loads and far transfers: the operand and the stack read first, in the chip's order and
//! sizes, then the selector checked against Linux's table, then the target, and nothing changed until all of them
//! pass. Entering compatibility mode keeps ESP alone of RSP, and the instruction pointer's low 32 bits.

use iced_x86::{Code, Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, MemoryOperand, check_access, expect_code, is_canonical, locate, read_register, read_value, store, write_register};

use crate::planted::{Bug, is_active};
use crate::segment::{SegmentBehavior, SegmentKind, SegmentOperation, classify_selector, find_segment_form};
use crate::stack::SegmentPush;
use crate::{Access, Exception, Flags, Instruction, Processor, STACK_POINTER, X86_64};

const SELECTOR_BYTES: u64 = 2;
/// The bits of RFLAGS a 16-bit IRET reaches.
const LOW_FLAGS: u64 = 0xFFFF;

/// The operand size of the code, in bytes.
fn to_size(code: Code) -> u64 {
    match code {
        Code::Popw_FS | Code::Popw_GS | Code::Lfs_r16_m1616 | Code::Lgs_r16_m1616 | Code::Lss_r16_m1616 | Code::Call_m1616
        | Code::Jmp_m1616 | Code::Retfw | Code::Retfw_imm16 | Code::Iretw => 2,
        Code::Lfs_r32_m1632 | Code::Lgs_r32_m1632 | Code::Lss_r32_m1632 | Code::Call_m1632 | Code::Jmp_m1632 | Code::Retfd
        | Code::Retfd_imm16 | Code::Iretd => 4,
        _ => 8,
    }
}

pub(super) fn execute_segment(
    instruction: Instruction,
    behavior: SegmentBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_segment_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no form of this family"));
    expect_code(decoded, form.codes);
    let size = to_size(decoded.code());
    let stack_pointer = processor.general_purpose_registers[STACK_POINTER];
    let at_stack = |address: u64| MemoryOperand { address, stack: true };
    let flags = processor.flags;
    let mut next = decoded.next_ip();
    match form.operation {
        SegmentOperation::Move => {
            let selector = match decoded.op1_kind() {
                OpKind::Register => read_register(processor, decoded.op1_register()) as u16,
                _ => {
                    let operand = locate(processor, decoded);
                    check_access(flags, memory, operand, SELECTOR_BYTES, Access::Read)?;
                    read_value(memory, operand.address, SELECTOR_BYTES as usize) as u16
                }
            };
            load_data_segment(processor, decoded.op0_register(), selector)?;
        }
        SegmentOperation::Pop => {
            // The chip reads only the selector's two bytes, however many it pops.
            let read = if is_active(Bug::SelectorPopReadsWholeSlot) { size } else { SELECTOR_BYTES };
            check_access(flags, memory, at_stack(stack_pointer), read, Access::Read)?;
            let selector = read_value(memory, stack_pointer, SELECTOR_BYTES as usize) as u16;
            load_data_segment(processor, decoded.op0_register(), selector)?;
            processor.general_purpose_registers[STACK_POINTER] = stack_pointer.wrapping_add(size);
        }
        SegmentOperation::LoadFar => {
            let (offset, selector) = read_far_pointer(processor, memory, decoded, size)?;
            let segment = match decoded.mnemonic() {
                iced_x86::Mnemonic::Lfs => Register::FS,
                iced_x86::Mnemonic::Lgs => Register::GS,
                _ => Register::SS,
            };
            load_data_segment(processor, segment, selector)?;
            write_register(processor, decoded.op0_register(), offset);
        }
        SegmentOperation::CallFar | SegmentOperation::JumpFar => {
            let (offset, selector) = read_far_pointer(processor, memory, decoded, size)?;
            let (code_segment, compatibility) = check_code_segment(selector, false)?;
            // The far CALL pushes before it checks the target, which leaves the pushes written.
            if form.operation == SegmentOperation::JumpFar || is_active(Bug::CallFarChecksTargetFirst) {
                to_target(offset, compatibility)?;
            }
            // In compatibility mode the stack is ESP's, so the pushes and RSP keep 32 bits.
            let stack_mask = if compatibility && !is_active(Bug::CompatibilityKeepsWholeStackPointer) { u64::from(u32::MAX) } else { u64::MAX };
            if form.operation == SegmentOperation::CallFar {
                let written = if size == 8 && behavior.segment_push == SegmentPush::SelectorOnly { SELECTOR_BYTES } else { size } as usize;
                let selector_slot = stack_pointer.wrapping_sub(size) & stack_mask;
                let return_slot = stack_pointer.wrapping_sub(2 * size) & stack_mask;
                let pushed_selector = u64::from(processor.code_segment);
                store(flags, memory, at_stack(selector_slot), &pushed_selector.to_le_bytes()[..written], behavior.across_fault, &mut on_memory_mutated)?;
                store(flags, memory, at_stack(return_slot), &next.to_le_bytes()[..size as usize], behavior.across_fault, &mut on_memory_mutated)?;
                processor.general_purpose_registers[STACK_POINTER] = return_slot;
            } else {
                processor.general_purpose_registers[STACK_POINTER] &= stack_mask;
            }
            next = to_target(offset, compatibility)?;
            processor.code_segment = code_segment;
        }
        SegmentOperation::ReturnFar => {
            // The chip reads CS's slot, then the return address's.
            let reads = [(1, SELECTOR_BYTES), (0, size)];
            check_slots(flags, memory, stack_pointer, size, &reads)?;
            let offset = read_value(memory, stack_pointer, size as usize);
            let selector = read_value(memory, stack_pointer.wrapping_add(size), SELECTOR_BYTES as usize) as u16;
            let (code_segment, compatibility) = check_code_segment(selector, true)?;
            let target = to_target(offset, compatibility)?;
            let released = if decoded.op_count() > 0 { u64::from(decoded.immediate16()) } else { 0 };
            let stack_mask = if compatibility && !is_active(Bug::CompatibilityKeepsWholeStackPointer) { u64::from(u32::MAX) } else { u64::MAX };
            processor.general_purpose_registers[STACK_POINTER] = stack_pointer.wrapping_add(2 * size).wrapping_add(released) & stack_mask;
            processor.code_segment = code_segment;
            next = target;
        }
        SegmentOperation::InterruptReturn => {
            if flags.contains(Flags::NESTED_TASK) && !is_active(Bug::InterruptReturnIgnoresNestedTask) {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            // The chip reads RFLAGS's slot first, then the others in order, of CS and SS the selectors alone.
            let reads = [(2, size), (0, size), (1, SELECTOR_BYTES), (3, size), (4, SELECTOR_BYTES)];
            check_slots(flags, memory, stack_pointer, size, &reads)?;
            let slot = |index: u64| read_value(memory, stack_pointer.wrapping_add(index * size), size as usize);
            let (offset, selector, popped_flags, popped_stack, stack_selector) = (slot(0), slot(1) as u16, slot(2), slot(3), slot(4) as u16);
            let (code_segment, compatibility) = check_code_segment(selector, true)?;
            match classify_selector(stack_selector) {
                SegmentKind::Null => return Err(Exception::GeneralProtection { error_code: 0 }),
                SegmentKind::Data if stack_selector & 3 == 3 => {}
                _ => return Err(Exception::GeneralProtection { error_code: u32::from(stack_selector & !3) }),
            }
            let target = to_target(offset, compatibility)?;
            let reached = if size == 2 { LOW_FLAGS } else { u64::MAX };
            processor.flags = Flags::from_bits_truncate((flags.bits() & !reached) | (popped_flags & reached));
            let stack_mask = if compatibility && !is_active(Bug::CompatibilityKeepsWholeStackPointer) { u64::from(u32::MAX) } else { u64::MAX };
            processor.general_purpose_registers[STACK_POINTER] = popped_stack & stack_mask;
            processor.code_segment = code_segment;
            next = target;
        }
    }
    processor.instruction_pointer = next;
    Ok(())
}

/// Loads the segment register with the selector, or raises the fault Linux's table gives it.
fn load_data_segment(processor: &mut Processor, segment: Register, selector: u16) -> Result<(), Exception> {
    let kind = classify_selector(selector);
    let refused = Exception::GeneralProtection { error_code: u32::from(selector & !3) };
    if segment == Register::SS {
        return match kind {
            SegmentKind::Data if selector & 3 == 3 => Ok(()),
            SegmentKind::Null => Err(Exception::GeneralProtection { error_code: 0 }),
            _ => Err(refused),
        };
    }
    if kind == SegmentKind::Unreachable {
        return Err(refused);
    }
    let index = match segment {
        Register::ES => 0,
        Register::DS => 1,
        Register::FS => 2,
        _ => 3,
    };
    processor.data_segments[index] = selector;
    let keeps_base = is_active(Bug::SegmentLoadKeepsBase) || (kind == SegmentKind::Null && is_active(Bug::NullLoadKeepsBase));
    match segment {
        Register::FS if !keeps_base => processor.fs_base = 0,
        Register::GS if !keeps_base => processor.gs_base = 0,
        _ => {}
    }
    Ok(())
}

/// The CS a far transfer to the selector loads, and whether it enters compatibility mode; a return may not lower
/// RPL below 3.
fn check_code_segment(selector: u16, returning: bool) -> Result<(u16, bool), Exception> {
    let refused = Exception::GeneralProtection { error_code: u32::from(selector & !3) };
    let compatibility = match classify_selector(selector) {
        SegmentKind::Null => return Err(Exception::GeneralProtection { error_code: 0 }),
        SegmentKind::LongCode => false,
        SegmentKind::CompatibilityCode => true,
        _ => return Err(refused),
    };
    if returning && selector & 3 != 3 && !is_active(Bug::ReturnToInnerPrivilegeAllowed) {
        return Err(refused);
    }
    Ok((selector | 3, compatibility))
}

/// The instruction pointer a far transfer leaves: in compatibility mode the target's low 32 bits; in 64-bit mode the
/// target, which must be canonical or the form raises #GP(0).
fn to_target(offset: u64, compatibility: bool) -> Result<u64, Exception> {
    if compatibility {
        return Ok(offset & u64::from(u32::MAX));
    }
    if is_canonical(offset) || is_active(Bug::FarTargetUnchecked) {
        Ok(offset)
    } else {
        Err(Exception::GeneralProtection { error_code: 0 })
    }
}

/// Reads a far pointer: the offset of `size` bytes, then the selector after it, each its own access.
fn read_far_pointer(processor: &Processor, memory: Memory<'_, '_>, decoded: &Decoded, size: u64) -> Result<(u64, u16), Exception> {
    let operand = locate(processor, decoded);
    check_access(processor.flags, memory, operand, size, Access::Read)?;
    let selector_operand = MemoryOperand { address: operand.address.wrapping_add(size), ..operand };
    check_access(processor.flags, memory, selector_operand, SELECTOR_BYTES, Access::Read)?;
    let offset = read_value(memory, operand.address, size as usize);
    let selector = read_value(memory, selector_operand.address, SELECTOR_BYTES as usize) as u16;
    Ok((offset, selector))
}

/// Checks the stack slots a far return or IRET reads, in the order the chip reads them, each of the bytes given.
fn check_slots(flags: Flags, memory: Memory<'_, '_>, stack_pointer: u64, size: u64, reads: &[(u64, u64)]) -> Result<(), Exception> {
    let mut ordered = reads.to_vec();
    if is_active(Bug::StackSlotsReadInOrder) {
        ordered.sort_unstable();
    }
    for (slot, bytes) in ordered {
        let address = stack_pointer.wrapping_add(slot * size);
        check_access(flags, memory, MemoryOperand { address, stack: true }, bytes, Access::Read)?;
    }
    Ok(())
}
