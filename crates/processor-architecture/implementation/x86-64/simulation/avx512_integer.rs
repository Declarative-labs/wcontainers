//! The skeleton of the EVEX integer forms: the writemask, the sources (a broadcast among them), the memory access
//! with fault suppression, and the destination's write.
//!
//! A memory operand is read, or written, only in the lanes the writemask lets through: a lane whose bit is clear
//! raises no fault, as the manual's memory fault suppression gives it. A broadcast lane and VPBROADCAST's memory are
//! read once any lane is let through, and checked as a legacy access of their size is (with a writemask other than
//! k0, for being canonical before for alignment);
//! VBROADCASTI reads each lane of its tuple that some lane let through repeats; a shift's count, and the memory of a
//! form whose lanes are not the destination's, is read whole always.
//! Where some lane is accessed, VMOVDQA's alignment is checked first, then every lane for being canonical, then for
//! its page. A load faults at the lowest byte refused; so does a store, but under a writemask other than k0, where
//! that byte's page is not the page of the store's first byte, at the highest byte refused on it.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use iced_x86::{Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::opmask::to_opmask_number;
use super::{Memory, MemoryOperand, check_access, check_alignment, check_canonical, expect_code, find_fault, locate};
use crate::behavior::StoreAcrossFault;
use crate::avx512_integer::{EvexIntegerForm, EvexIntegerOperation, EvexIntegerPlace, Zmm, find_evex_integer_form, from_lanes, read_zmm, to_lanes, write_zmm};
use crate::memory::{PAGE_BYTES, read_bytes};
use crate::planted::{Bug, is_active};
use crate::{Access, Exception, Instruction, Processor, X86_64};

/// The bytes of a ZMM register.
const ZMM_BYTES: usize = 64;

pub(super) fn execute_evex_integer(
    instruction: Instruction,
    across_fault: StoreAcrossFault,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_evex_integer_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no EVEX integer form"));
    expect_code(decoded, form.codes);
    let mask = match decoded.op_mask() {
        Register::None => None,
        register => Some(processor.opmask_registers[to_opmask_number(register)]),
    };
    let all_lanes = to_low_bits(form.count_lanes());
    let active = mask.map_or(all_lanes, |mask| mask & all_lanes);
    let zeroing = decoded.zeroing_masking() && !is_active(Bug::EvexIntegerZeroingMerges);
    if form.place == EvexIntegerPlace::Store {
        return store_vector(form, processor, memory, decoded, active, across_fault, on_memory_mutated);
    }
    let destination = if form.writes_mask() { [0; 4] } else { read_vector(processor, decoded.op0_register()) };
    let (first, last) = if form.has_first_source() { (read_vector(processor, decoded.op1_register()), 2) } else { ([0; 4], 1) };
    let second = match decoded.op_kind(last) {
        OpKind::Register if form.place == EvexIntegerPlace::General => {
            let register = decoded.op_register(last);
            [u128::from(processor.general_purpose_registers[register.full_register().number()]), 0, 0, 0]
        }
        OpKind::Register => read_vector(processor, decoded.op_register(last)),
        _ => read_source(form, processor, memory, decoded, active)?,
    };
    let immediate = if form.has_immediate() { decoded.immediate8() } else { 0 };
    let result = form.compute(&destination, &first, &second, immediate);
    if form.writes_mask() {
        let writemask = if is_active(Bug::CompareIgnoresWritemask) { None } else { mask };
        let value = form.apply_mask_writemask(&result, writemask);
        processor.opmask_registers[to_opmask_number(decoded.op0_register())] = value;
        return Ok(());
    }
    let blends = form.operation == EvexIntegerOperation::Blend && !is_active(Bug::BlendKeepsDestination);
    let kept = if blends { first } else { destination };
    let mut written = form.apply_writemask(&result, &kept, mask, zeroing);
    if is_active(Bug::EvexIntegerUpperBitsKept) {
        let quarters = form.length.to_quarters();
        written[quarters..].copy_from_slice(&destination[quarters..]);
    }
    write_vector(processor, decoded.op0_register(), written);
    Ok(())
}

/// The low `count` bits set.
const fn to_low_bits(count: usize) -> u64 {
    if count >= 64 { u64::MAX } else { (1 << count) - 1 }
}

fn to_vector_number(register: Register) -> usize {
    let number = register.number();
    if is_active(Bug::EvexIntegerHighRegistersAliasLow) { number % 16 } else { number }
}

fn read_vector(processor: &Processor, register: Register) -> Zmm {
    read_zmm(processor, to_vector_number(register))
}

fn write_vector(processor: &mut Processor, register: Register, value: Zmm) {
    write_zmm(processor, to_vector_number(register), value);
}

/// The runs of bytes, from the operand's address, that the form reads or writes of its memory operand.
fn list_accessed(form: &EvexIntegerForm, broadcast: bool, active: u64) -> Vec<(u64, u64)> {
    let bytes = form.memory_bytes(broadcast);
    let lane_bytes = u64::from(form.lane.to_bits() / 8);
    let active = if is_active(Bug::EvexIntegerNoFaultSuppression) { u64::MAX } else { active };
    let lanes = form.count_lanes() as u64;
    match form.operation {
        _ if !form.is_element_wise() && !is_active(Bug::EvexWholeReadSuppressed) => alloc::vec![(0, bytes)],
        _ if broadcast || form.operation == EvexIntegerOperation::Broadcast => {
            if active & to_low_bits(lanes as usize) != 0 { alloc::vec![(0, bytes)] } else { Vec::new() }
        }
        EvexIntegerOperation::BroadcastTuple(_) => {
            let tuple = bytes / lane_bytes;
            (0..tuple)
                .filter(|element| (0..lanes).any(|lane| lane % tuple == *element && active >> lane & 1 != 0))
                .map(|element| (element * lane_bytes, lane_bytes))
                .collect()
        }
        _ => (0..lanes).filter(|lane| active >> lane & 1 != 0).map(|lane| (lane * lane_bytes, lane_bytes)).collect(),
    }
}

/// Faults as the access of the runs does: where it accesses anything, the alignment VMOVDQA needs, then each run's
/// bytes canonical, then the lowest byte of a run whose page refuses the access, or for a write under a writemask
/// past its first page the highest refused byte of that page.
fn check_runs(
    form: &EvexIntegerForm,
    memory: Memory<'_, '_>,
    operand: MemoryOperand,
    runs: &[(u64, u64)],
    access: Access,
    masked: bool,
) -> Result<(), Exception> {
    if runs.is_empty() {
        return Ok(());
    }
    if let Some(alignment) = form.to_alignment()
        && !operand.address.is_multiple_of(alignment)
    {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    let moved = |offset: u64| MemoryOperand {
        address: operand.address.wrapping_add(offset),
        stack: operand.stack,
    };
    for &(offset, length) in runs {
        check_canonical(moved(offset), length)?;
    }
    let bytes = runs.iter().flat_map(|&(offset, length)| (0..length).map(move |byte| moved(offset).address.wrapping_add(byte)));
    let refused: Vec<(u64, bool)> = bytes.filter_map(|address| find_fault(memory, address, 1, access)).collect();
    let page = |address: u64| address / PAGE_BYTES;
    let first_page = page(moved(runs[0].0).address);
    let fault = match refused.first() {
        Some(&(lowest, _)) if masked && access == Access::Write && page(lowest) != first_page => {
            refused.iter().rev().find(|(address, _)| page(*address) == page(lowest)).copied()
        }
        lowest => lowest.copied(),
    };
    match fault {
        Some((address, page_present)) => Err(Exception::PageFault { access, page_present, address }),
        None => Ok(()),
    }
}

/// The memory source: its accessed lanes read, a broadcast lane repeated in every lane.
fn read_source(form: &EvexIntegerForm, processor: &Processor, memory: Memory<'_, '_>, decoded: &Decoded, active: u64) -> Result<Zmm, Exception> {
    let broadcast = decoded.is_broadcast() && !is_active(Bug::EvexIntegerBroadcastIgnored);
    let operand = locate(processor, decoded);
    let runs = list_accessed(form, broadcast, active);
    let masked = decoded.op_mask() != Register::None;
    // A memory operand of at most eight bytes is one access of its size, which alignment checking covers; under a
    // writemask that suppresses faults, after the bytes read are checked for being canonical.
    let length = form.memory_bytes(broadcast);
    if !runs.is_empty() && length <= 8 {
        if masked && form.is_element_wise() {
            for &(offset, run) in &runs {
                check_canonical(MemoryOperand { address: operand.address.wrapping_add(offset), stack: operand.stack }, run)?;
            }
            check_alignment(processor.flags, operand, length)?;
        } else {
            check_access(processor.flags, memory, operand, length, Access::Read)?;
        }
    }
    check_runs(form, memory, operand, &runs, Access::Read, masked)?;
    let bytes: [u8; ZMM_BYTES] = read_bytes(memory, operand.address);
    let mut value = [0u8; ZMM_BYTES];
    for &(offset, run) in &runs {
        let (offset, run) = (offset as usize, run as usize);
        value[offset..offset + run].copy_from_slice(&bytes[offset..offset + run]);
    }
    let whole: Zmm = core::array::from_fn(|quarter| u128::from_le_bytes(value[16 * quarter..16 * quarter + 16].try_into().expect("sixteen bytes")));
    if broadcast {
        let lane = if is_active(Bug::EvexBroadcastDestinationLane) { form.lane } else { form.source_lane() };
        let count = (form.length.to_bits() / lane.to_bits()) as usize;
        return Ok(from_lanes(lane, &alloc::vec![to_lanes(lane, &whole, 1)[0]; count]));
    }
    Ok(whole)
}

/// VMOVDQA's and VMOVDQU's store: the lanes the writemask lets through, each run of them one write; where a lane's
/// page refuses the store, what the behavior across a fault writes below it.
fn store_vector(
    form: &EvexIntegerForm,
    processor: &Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    active: u64,
    across_fault: StoreAcrossFault,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let source = read_vector(processor, decoded.op1_register());
    let operand = locate(processor, decoded);
    let runs = list_accessed(form, false, active);
    let mut bytes = [0u8; ZMM_BYTES];
    for (quarter, value) in source.iter().enumerate() {
        bytes[16 * quarter..16 * quarter + 16].copy_from_slice(&value.to_le_bytes());
    }
    let fault = check_runs(form, memory, operand, &runs, Access::Write, decoded.op_mask() != Register::None).err();
    let below = match fault {
        Some(Exception::PageFault { address, .. }) if across_fault == StoreAcrossFault::WritesBytesBelowFault => address,
        Some(_) => operand.address,
        None => u64::MAX,
    };
    // Adjacent lanes are one run of bytes.
    let mut merged: Vec<(u64, u64)> = Vec::new();
    for &(offset, length) in &runs {
        match merged.last_mut() {
            Some((start, run)) if *start + *run == offset => *run += length,
            _ => merged.push((offset, length)),
        }
    }
    for (offset, length) in merged {
        let start = operand.address.wrapping_add(offset);
        let end = start.wrapping_add(length).min(below.max(start));
        if end > start {
            let (from, to) = (offset as usize, (offset + (end - start)) as usize);
            on_memory_mutated(MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes[from..to].to_vec()) });
        }
    }
    fault.map_or(Ok(()), Err)
}
