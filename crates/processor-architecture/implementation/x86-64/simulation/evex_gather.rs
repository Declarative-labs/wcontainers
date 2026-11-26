//! The EVEX gathers and scatters, as the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! volume 2 describes them: each element the opmask selects is loaded from, or stored to, the base plus its index
//! (sign-extended) times the scale plus the displacement, lowest first, and its opmask bit cleared once done. At the
//! first element that faults the instruction stops: the elements done so far stay done and only their opmask bits are
//! cleared. Alignment is not checked.

use alloc::borrow::Cow;

use iced_x86::{Instruction as Decoded, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, MemoryOperand, check_canonical, expect_code, find_fault, read_register};
use crate::behavior::StoreAcrossFault;
use crate::evex::{EvexForm, EvexOperation, find_evex_form, read_element, write_element};
use crate::memory::read_bytes;
use crate::planted::{Bug, is_active};
use crate::{Access, Exception, Instruction, Processor, X86_64};

/// The low `count` bits.
const fn to_low_bits(count: usize) -> u64 {
    if count >= 64 { u64::MAX } else { (1 << count) - 1 }
}

/// The elements a gather or scatter moves: as many as both its data vector and its index vector hold.
fn count_elements(form: &EvexForm, decoded: &Decoded) -> usize {
    let (data_bytes, index_bytes) = form.to_gather_bytes();
    let index_vector = decoded.memory_index().size();
    (form.vector_bytes() / data_bytes).min(index_vector / index_bytes)
}

/// The address of the element of the index, and whether it is reached through the stack segment.
fn locate_element(processor: &Processor, decoded: &Decoded, index_bytes: usize, element: usize) -> MemoryOperand {
    let indices = processor.read_zmm(decoded.memory_index().number());
    let unused = 64 - 8 * index_bytes as u32;
    let index = (((read_element(&indices, element, index_bytes) << unused) as i64) >> unused) as u64;
    let base = match decoded.memory_base() {
        Register::None => 0,
        register => read_register(processor, register),
    };
    let scale = if is_active(Bug::GatherScaleIgnored) { 1 } else { u64::from(decoded.memory_index_scale()) };
    let address = base.wrapping_add(index.wrapping_mul(scale)).wrapping_add(decoded.memory_displacement64());
    MemoryOperand { address, stack: matches!(decoded.memory_base(), Register::RSP | Register::RBP) }
}

/// Where an element's access first faults: its first byte not canonical, or a byte whose page refuses it.
fn find_element_fault(memory: Memory<'_, '_>, operand: MemoryOperand, bytes: u64, access: Access) -> Option<Exception> {
    if let Err(exception) = check_canonical(operand, bytes) {
        return Some(exception);
    }
    find_fault(memory, operand.address, bytes, access).map(|(address, page_present)| Exception::PageFault { access, page_present, address })
}

/// The opmask register a gather or scatter names, which k0 cannot be.
fn to_opmask(decoded: &Decoded) -> Result<usize, Exception> {
    match decoded.op_mask() {
        Register::None => Err(Exception::InvalidOpcode),
        register => Ok(register.number()),
    }
}

/// A gather: the selected elements loaded into the destination; the processor holds what the loads did even when it
/// returns the fault that stopped them.
pub(super) fn gather(form: &EvexForm, processor: &mut Processor, memory: Memory<'_, '_>, decoded: &Decoded) -> Result<(), Exception> {
    let (data_bytes, index_bytes) = form.to_gather_bytes();
    let destination = decoded.op0_register().number();
    let mask = to_opmask(decoded)?;
    if destination == decoded.memory_index().number() {
        return Err(Exception::InvalidOpcode);
    }
    let count = count_elements(form, decoded);
    let mut values = processor.read_zmm(destination);
    let mut remaining = processor.read_opmask(mask) & to_low_bits(count);
    let mut fault = None;
    for element in 0..count {
        if remaining & (1 << element) == 0 {
            continue;
        }
        let operand = locate_element(processor, decoded, index_bytes, element);
        if let Some(exception) = find_element_fault(memory, operand, data_bytes as u64, Access::Read) {
            fault = Some(exception);
            break;
        }
        let value: [u8; 8] = read_bytes(memory, operand.address);
        write_element(&mut values, element, data_bytes, u64::from_le_bytes(value));
        remaining &= !(1 << element);
    }
    // The bits above the elements are zeroed once every element is loaded; after a fault, those above the vector.
    let vector = if fault.is_none() { count * data_bytes } else { form.vector_bytes() };
    for (number, part) in values.iter_mut().enumerate() {
        let kept = vector.saturating_sub(number * 16).min(16);
        *part &= if kept == 16 { u128::MAX } else { (1u128 << (kept * 8)) - 1 };
    }
    processor.write_zmm(destination, values);
    // A fault clears only the bits of the elements loaded, and leaves those beyond the elements as they were.
    let beyond = processor.read_opmask(mask) & !to_low_bits(count);
    let after_fault = if is_active(Bug::GatherFaultClearsMask) { 0 } else { remaining | beyond };
    processor.opmask_registers[mask] = if fault.is_none() { 0 } else { after_fault };
    fault.map_or(Ok(()), Err)
}

/// A scatter: the selected elements stored, lowest first, a later one over an earlier one where they overlap; where
/// an element's bytes cross onto a page that refuses them, what the behavior across a fault writes below it.
pub(super) fn scatter(
    instruction: Instruction,
    across_fault: StoreAcrossFault,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_evex_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no EVEX form"));
    expect_code(decoded, &[form.code]);
    let (data_bytes, index_bytes) = form.to_gather_bytes();
    let mask = to_opmask(decoded)?;
    let count = count_elements(&form, decoded);
    let source = processor.read_zmm(decoded.op1_register().number());
    let mut remaining = processor.read_opmask(mask) & to_low_bits(count);
    for element in 0..count {
        if remaining & (1 << element) == 0 {
            continue;
        }
        let operand = locate_element(processor, decoded, index_bytes, element);
        let bytes: alloc::vec::Vec<u8> = read_element(&source, element, data_bytes).to_le_bytes()[..data_bytes].to_vec();
        if let Some(exception) = find_element_fault(memory, operand, data_bytes as u64, Access::Write) {
            if let Exception::PageFault { address, .. } = exception
                && across_fault == StoreAcrossFault::WritesBytesBelowFault
                && address != operand.address
            {
                let below = address.wrapping_sub(operand.address) as usize;
                on_memory_mutated(MemoryDelta::ValueAssignment { start: operand.address, bytes: Cow::Owned(bytes[..below].to_vec()) });
            }
            processor.opmask_registers[mask] = remaining | (processor.read_opmask(mask) & !to_low_bits(count));
            return Err(exception);
        }
        on_memory_mutated(MemoryDelta::ValueAssignment { start: operand.address, bytes: Cow::Owned(bytes) });
        remaining &= !(1 << element);
    }
    processor.opmask_registers[mask] = if is_active(Bug::ScatterMaskKept) { processor.opmask_registers[mask] } else { 0 };
    Ok(())
}

impl EvexForm {
    /// The bytes of a gather's or scatter's data element and of its index element.
    pub(crate) const fn to_gather_bytes(self) -> (usize, usize) {
        match self.operation {
            EvexOperation::Gather { data_bytes, index_bytes } | EvexOperation::Scatter { data_bytes, index_bytes } => (data_bytes, index_bytes),
            _ => (0, 0),
        }
    }
}
