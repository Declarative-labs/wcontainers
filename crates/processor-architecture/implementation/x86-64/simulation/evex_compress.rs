//! VCOMPRESS and VEXPAND, as the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! volume 2 describes them: the opmask selects which of the vector's elements move, and the moved elements are
//! contiguous on the other side, from the lowest. A register destination keeps or zeroes the elements nothing moves
//! into; a memory destination or source is reached only for the elements that move, so the rest cannot fault.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use iced_x86::{Instruction as Decoded, OpKind};
use wcontainers_processor_architecture::MemoryDelta;

use super::evex_simulation::read_selection;
use super::{Memory, check_access, check_canonical, expect_code, find_fault, locate};
use crate::behavior::StoreAcrossFault;
use crate::evex::{EvexForm, EvexOperation, find_evex_form, read_element, write_element};
use crate::memory::read_bytes;
use crate::planted::{Bug, is_active};
use crate::{Access, Exception, Flags, Instruction, Processor, X86_64};

/// VCOMPRESS into a register and VEXPAND from a register or memory.
pub(super) fn move_selected(
    form: &EvexForm,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    selected: u64,
) -> Result<(), Exception> {
    let (elements, bytes) = (form.elements(), form.element_bytes());
    let destination = decoded.op0_register().number();
    let old = processor.read_zmm(destination);
    let chosen: Vec<usize> = (0..elements).filter(|index| selected & (1 << index) != 0).collect();
    let mut result = old;
    let zeroing = decoded.zeroing_masking() && !is_active(Bug::EvexZeroingMerges);
    let moved: Vec<(usize, u64)> = match form.operation {
        EvexOperation::Compress { .. } => {
            let source = processor.read_zmm(decoded.op1_register().number());
            chosen.iter().enumerate().map(|(at, &index)| (at, read_element(&source, index, bytes))).collect()
        }
        _ => {
            let source = match decoded.op_kind(1) {
                OpKind::Register => processor.read_zmm(decoded.op1_register().number()),
                _ => read_contiguous(processor, memory, decoded, chosen.len(), bytes)?,
            };
            // Expanding in the wrong order reads the source's elements from the highest.
            let reversed = is_active(Bug::EvexExpandReadsFromTheTop);
            chosen
                .iter()
                .enumerate()
                .map(|(at, &index)| (index, read_element(&source, if reversed { chosen.len() - 1 - at } else { at }, bytes)))
                .collect()
        }
    };
    let written: Vec<usize> = moved.iter().map(|&(index, _)| index).collect();
    for index in (0..elements).filter(|index| !written.contains(index)) {
        if zeroing {
            write_element(&mut result, index, bytes, 0);
        }
    }
    for (index, value) in moved {
        write_element(&mut result, index, bytes, value);
    }
    // Every bit from the last element up to 512 is zeroed.
    let vector = elements * bytes;
    for (number, part) in result.iter_mut().enumerate() {
        let kept = vector.saturating_sub(number * 16).min(16);
        *part &= if kept == 16 { u128::MAX } else { (1u128 << (kept * 8)) - 1 };
    }
    processor.write_zmm(destination, result);
    Ok(())
}

/// The lowest `count` elements of memory as a vector; no access when none moves.
fn read_contiguous(
    processor: &Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    count: usize,
    bytes: usize,
) -> Result<[u128; 4], Exception> {
    let mut vector = [0; 4];
    if count == 0 {
        return Ok(vector);
    }
    let operand = locate(processor, decoded);
    let length = if is_active(Bug::EvexCompressReachesWholeVector) { 64 } else { count * bytes };
    // Alignment is not checked, whatever AC says, as the chip shows.
    check_access(processor.flags.difference(Flags::ALIGNMENT_CHECK), memory, operand, length as u64, Access::Read)?;
    for index in 0..count {
        let value: [u8; 8] = read_bytes(memory, operand.address.wrapping_add((index * bytes) as u64));
        write_element(&mut vector, index, bytes, u64::from_le_bytes(value));
    }
    Ok(vector)
}

/// VCOMPRESS into memory: the selected elements, packed, written from the operand's address; where a byte of them
/// faults, what the behavior across a fault writes below it.
pub(super) fn store_compressed(
    instruction: Instruction,
    across_fault: StoreAcrossFault,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_evex_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no EVEX form"));
    expect_code(decoded, &[form.code]);
    let (elements, bytes) = (form.elements(), form.element_bytes());
    let selected = read_selection(processor, decoded, elements);
    let source = processor.read_zmm(decoded.op1_register().number());
    let packed: Vec<u8> = (0..elements)
        .filter(|index| selected & (1 << index) != 0)
        .flat_map(|index| read_element(&source, index, bytes).to_le_bytes().into_iter().take(bytes))
        .collect();
    if packed.is_empty() {
        return Ok(());
    }
    let operand = locate(processor, decoded);
    let length = if is_active(Bug::EvexCompressReachesWholeVector) { 64 } else { packed.len() as u64 };
    // The canonical check comes before any byte is written; alignment is not checked.
    check_canonical(operand, length)?;
    let Some((address, page_present)) = find_fault(memory, operand.address, length, Access::Write) else {
        on_memory_mutated(MemoryDelta::ValueAssignment { start: operand.address, bytes: Cow::Owned(packed) });
        return Ok(());
    };
    let below = (address.wrapping_sub(operand.address) as usize).min(packed.len());
    if across_fault == StoreAcrossFault::WritesBytesBelowFault && below > 0 {
        on_memory_mutated(MemoryDelta::ValueAssignment { start: operand.address, bytes: Cow::Owned(packed[..below].to_vec()) });
    }
    Err(Exception::PageFault { access: Access::Write, page_present, address })
}
