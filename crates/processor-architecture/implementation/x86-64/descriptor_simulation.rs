//! The skeleton of LAR, LSL, VERR and VERW: the selector read from a register's low 16 bits or two bytes of memory,
//! the descriptor it reaches looked up, ZF set where the form succeeds, and LAR's or LSL's destination written then.

use iced_x86::{Instruction as Decoded, OpKind};

use super::{Memory, check_access, expect_code, locate, read_register, read_value, write_register};
use crate::descriptor::{CPU_SEGMENT_INDEX, DescriptorOperation, find_descriptor, find_descriptor_form};
use crate::planted::{Bug, is_active};
use crate::{Access, Exception, Flags, Instruction, Processor};

const SELECTOR_BYTES: u64 = 2;

/// The selector the form reads.
fn read_selector(processor: &Processor, memory: Memory<'_, '_>, decoded: &Decoded) -> Result<u16, Exception> {
    let last = decoded.op_count() - 1;
    Ok(match decoded.op_kind(last) {
        OpKind::Register => read_register(processor, decoded.op_register(last)) as u16,
        _ => {
            let operand = locate(processor, decoded);
            check_access(processor.flags, memory, operand, SELECTOR_BYTES, Access::Read)?;
            read_value(memory, operand.address, SELECTOR_BYTES as usize) as u16
        }
    })
}

pub(super) fn execute_descriptor(instruction: Instruction, processor: &mut Processor, memory: Memory<'_, '_>, decoded: &Decoded) -> Result<(), Exception> {
    let form = find_descriptor_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no form of this family"));
    expect_code(decoded, form.codes);
    let selector = read_selector(processor, memory, decoded)?;
    let selector = if is_active(Bug::DescriptorTableIndicatorIgnored) { selector & !(1 << 2) } else { selector };
    let found = find_descriptor(selector).filter(|_| !(is_active(Bug::DescriptorRequestedPrivilegeChecked) && selector & 3 != 3));
    let (succeeds, value) = match form.operation {
        DescriptorOperation::AccessRights => (found.is_some(), found.map(|descriptor| u64::from(descriptor.access_rights))),
        DescriptorOperation::SegmentLimit => {
            let limit = |descriptor: crate::descriptor::Descriptor| {
                let limit = descriptor.limit.unwrap_or(0);
                u64::from(if is_active(Bug::SegmentLimitInPages) { limit >> 12 } else { limit })
            };
            (found.is_some(), found.map(limit))
        }
        DescriptorOperation::VerifyRead => (found.is_some_and(|descriptor| descriptor.readable), None),
        DescriptorOperation::VerifyWrite => {
            (found.is_some_and(|descriptor| descriptor.writable || is_active(Bug::VerifyWriteTakesCode)), None)
        }
    };
    processor.flags.set(Flags::ZERO, succeeds);
    if let Some(value) = value {
        write_register(processor, decoded.op0_register(), value);
    }
    Ok(())
}

/// The register LSL writes with the per-CPU segment's limit, which the state does not decide.
pub(super) fn find_undecided_register(instruction: Instruction, before: &Processor, memory: Memory<'_, '_>, decoded: &Decoded) -> Option<usize> {
    let form = find_descriptor_form(instruction)?;
    let selector = read_selector(before, memory, decoded).ok()?;
    let per_cpu = form.operation == DescriptorOperation::SegmentLimit && selector & (1 << 2) == 0 && selector >> 3 == CPU_SEGMENT_INDEX;
    per_cpu.then(|| decoded.op0_register().full_register().number())
}
