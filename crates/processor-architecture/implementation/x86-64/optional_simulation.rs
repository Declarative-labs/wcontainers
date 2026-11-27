//! The skeleton of the optional forms of [`crate::optional`]: each reads its operands from the decoded instruction,
//! checks its memory accesses in the chip's order, and writes its destination; an EVEX writemask's elements left out
//! are neither read nor written and never fault.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use iced_x86::{Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{
    Memory, MemoryOperand, check_access, check_alignment, check_canonical, expect_code, find_fault, is_canonical, locate,
    read_register, store, write_register,
};
use crate::behavior::StoreAcrossFault;
use crate::evex::{read_element, write_element};
use crate::float::{FLAGS, LaneOutcome, MASK_SHIFT, ROUNDING_SHIFT, narrow_single, read_rounding};
use crate::memory::read_bytes;
use crate::optional::{BLOCK_BYTES, OptionalForm, OptionalOperation, compute_reciprocal, compute_root_reciprocal, find_optional_form};
use crate::planted::{Bug, is_active};
use crate::reciprocal::{XEON_PLATINUM_8488C_RECIPROCALS, compute};
use crate::{Access, Exception, Flags, Instruction, Processor, X86_64};

type Zmm = [u128; 4];

pub(super) fn execute_optional(
    instruction: Instruction,
    across_fault: StoreAcrossFault,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_optional_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no optional form"));
    expect_code(decoded, &form.list_codes());
    match form.operation {
        OptionalOperation::ExtractElement { bytes } => {
            let source = processor.read_zmm(decoded.op1_register().number());
            let lanes = (16 / bytes) as u8;
            let index = if form.has_immediate() { decoded.immediate8() % lanes } else { 0 };
            let index = if is_active(Bug::ExtractIndexUnmasked) { decoded.immediate8() & 0xF } else { index };
            let value = read_element(&source, usize::from(index), bytes as usize);
            put(processor, memory, decoded, value, bytes, across_fault, on_memory_mutated)
        }
        OptionalOperation::StoreQuadword { high } => {
            let source = processor.read_zmm(decoded.op1_register().number());
            let high = high != is_active(Bug::QuadwordStoreHalvesSwapped);
            let value = read_element(&source, usize::from(high), 8);
            put(processor, memory, decoded, value, 8, across_fault, on_memory_mutated)
        }
        OptionalOperation::MoveScalar { bytes } => move_scalar(bytes, across_fault, processor, memory, decoded, on_memory_mutated),
        OptionalOperation::ConvertToHalves => convert_to_halves(form, across_fault, processor, memory, decoded, on_memory_mutated),
        OptionalOperation::VexReciprocal { root } => vex_reciprocal(form, root, processor, memory, decoded),
        OptionalOperation::Reciprocal { root, precision } => {
            let compute = |value: u64, mxcsr: u32| {
                if root { compute_root_reciprocal(precision, value, mxcsr) } else { compute_reciprocal(precision, value, mxcsr) }
            };
            evex_reciprocal(form, processor, memory, decoded, compute)
        }
        OptionalOperation::DirectStore => {
            let value = read_register(processor, decoded.op1_register());
            let bytes = form.memory_bytes(false);
            put(processor, memory, decoded, value, bytes, across_fault, on_memory_mutated)
        }
        OptionalOperation::DirectStoreBlock => store_block(processor, memory, decoded, on_memory_mutated),
        OptionalOperation::ReadBase { gs } => {
            let base = if gs { processor.gs_base } else { processor.fs_base };
            write_register(processor, decoded.op0_register(), base);
            Ok(())
        }
        OptionalOperation::WriteBase { gs } => {
            let value = read_register(processor, decoded.op0_register());
            if !is_canonical(value) && !is_active(Bug::BaseWriteCanonicalUnchecked) {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            if gs { processor.gs_base = value } else { processor.fs_base = value }
            Ok(())
        }
        OptionalOperation::Monitor => {
            let operand = to_register_address(processor, decoded.op0_register());
            if is_active(Bug::MonitorChecksRead) {
                check_access(processor.flags.difference(Flags::ALIGNMENT_CHECK), memory, operand, 1, Access::Read)?;
            }
            Ok(())
        }
        OptionalOperation::TransactionBegin => {
            let mut target = decoded.near_branch_target();
            if is_active(Bug::TransactionFallbackWrapsToSixteenBits) && decoded.code() == iced_x86::Code::Xbegin_rel16 {
                target &= u64::from(u16::MAX);
            }
            if !is_canonical(target) {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            if is_active(Bug::TransactionBeginFallsThrough) {
                processor.instruction_pointer = decoded.next_ip();
                return Ok(());
            }
            // The abort status is written as EAX, which clears RAX's upper half.
            let accumulator = &mut processor.general_purpose_registers[0];
            if is_active(Bug::TransactionAbortKeepsUpperHalf) {
                *accumulator &= !u64::from(u32::MAX);
            } else if !is_active(Bug::TransactionAbortKeepsAccumulator) {
                *accumulator = 0;
            }
            processor.instruction_pointer = target;
            Ok(())
        }
    }
}

/// Writes the value to the destination: a general register whole, zero-extended, or `bytes` of memory.
fn put(
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    value: u64,
    bytes: u64,
    across_fault: StoreAcrossFault,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    match decoded.op0_kind() {
        OpKind::Register => {
            let register = decoded.op0_register();
            let full = register.full_register();
            processor.general_purpose_registers[full.number()] = value & to_mask(register.size() as u64 * 8);
            Ok(())
        }
        _ => {
            let operand = locate(processor, decoded);
            let bytes_of = value.to_le_bytes();
            store(processor.flags, memory, operand, &bytes_of[..bytes as usize], across_fault, on_memory_mutated)
        }
    }
}

const fn to_mask(bits: u64) -> u64 {
    if bits >= 64 { u64::MAX } else { (1 << bits) - 1 }
}

/// The elements the writemask selects, lowest bit first; every element without one.
fn read_selection(processor: &Processor, decoded: &Decoded, elements: usize) -> u64 {
    let all = to_mask(elements as u64);
    match decoded.op_mask() {
        Register::None => all,
        mask if is_active(Bug::OptionalMaskIgnored) => {
            let _ = mask;
            all
        }
        mask => processor.read_opmask(mask.number()) & all,
    }
}

/// EVEX VMOVSS and VMOVSD: a load, a store or a move between registers of the lowest element.
fn move_scalar(
    bytes: u64,
    across_fault: StoreAcrossFault,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let selected = read_selection(processor, decoded, 1) != 0;
    let width = bytes as usize;
    if decoded.op0_kind() == OpKind::Memory {
        if !selected {
            return Ok(());
        }
        let value = read_element(&processor.read_zmm(decoded.op1_register().number()), 0, width);
        let operand = locate(processor, decoded);
        let all = value.to_le_bytes();
        return store(processor.flags, memory, operand, &all[..width], across_fault, &mut on_memory_mutated);
    }
    let destination = decoded.op0_register().number();
    let old = processor.read_zmm(destination);
    let kept = if decoded.zeroing_masking() { 0 } else { read_element(&old, 0, width) };
    let mut result: Zmm = [0; 4];
    if decoded.op_count() == 3 {
        let first = processor.read_zmm(decoded.op1_register().number());
        result[0] = first[0];
        let low = if selected { read_element(&processor.read_zmm(decoded.op2_register().number()), 0, width) } else { kept };
        write_element(&mut result, 0, width, low);
        if is_active(Bug::ScalarMoveKeepsUpperBits) {
            result[1] = old[1];
        }
    } else {
        let low = if selected {
            let operand = locate(processor, decoded);
            if decoded.op_mask() == Register::None {
                check_access(processor.flags, memory, operand, bytes, Access::Read)?;
            } else {
                // Masked, the element's bytes must be canonical before its alignment is checked.
                check_canonical(operand, bytes)?;
                check_alignment(processor.flags, operand, bytes)?;
                check_access(processor.flags.difference(Flags::ALIGNMENT_CHECK), memory, operand, bytes, Access::Read)?;
            }
            let value: [u8; 8] = read_bytes(memory, operand.address);
            u64::from_le_bytes(value) & to_mask(8 * bytes)
        } else {
            kept
        };
        write_element(&mut result, 0, width, low);
    }
    processor.write_zmm(destination, result);
    Ok(())
}

/// The bytes of each selected element's place, lowest first, merged where they touch.
fn to_runs(selected: u64, elements: usize, element_bytes: u64) -> Vec<(u64, u64)> {
    let mut runs: Vec<(u64, u64)> = Vec::new();
    for index in (0..elements).filter(|index| selected & (1 << index) != 0) {
        let start = index as u64 * element_bytes;
        match runs.last_mut() {
            Some((first, length)) if *first + *length == start => *length += element_bytes,
            _ => runs.push((start, element_bytes)),
        }
    }
    runs
}

/// EVEX VCVTPS2PH to memory: each selected element rounded as the register form rounds it, #XM for an unmasked
/// exception before anything is stored, then each selected half stored in its place.
fn convert_to_halves(
    form: &OptionalForm,
    across_fault: StoreAcrossFault,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let elements = form.elements();
    let selected = read_selection(processor, decoded, elements);
    let source = processor.read_zmm(decoded.op1_register().number());
    let immediate = decoded.immediate8();
    let mxcsr = processor.mxcsr;
    let rounding = if immediate & 4 != 0 { read_rounding(mxcsr) } else { read_rounding(u32::from(immediate & 3) << ROUNDING_SHIFT) };
    let mut halves: Zmm = [0; 4];
    let (mut before, mut after) = (0, 0);
    for index in (0..elements).filter(|index| selected & (1 << index) != 0) {
        let LaneOutcome { bits, before: found, after: late } = narrow_single(read_element(&source, index, 4), rounding, mxcsr);
        before |= found;
        after |= late;
        write_element(&mut halves, index, 2, bits);
    }
    let unmasked = |flags: u32| flags & !(mxcsr >> MASK_SHIFT) & FLAGS != 0;
    if unmasked(before) {
        processor.mxcsr |= before;
        return Err(Exception::SimdFloatingPoint);
    }
    if unmasked(before | after) && !is_active(Bug::HalfStoreExceptionsIgnored) {
        processor.mxcsr |= before | after;
        return Err(Exception::SimdFloatingPoint);
    }
    // The flags are recorded before the store, which keeps them though it faults.
    if !is_active(Bug::HalfStoreFaultClearsFlags) {
        processor.mxcsr |= before | after;
    }
    let operand = locate(processor, decoded);
    let length = form.memory_bytes(false);
    let all: Vec<u8> = halves.iter().flat_map(|part| part.to_le_bytes()).take(length as usize).collect();
    let runs = if decoded.op_mask() == Register::None { alloc::vec![(0, length)] } else { to_runs(selected, elements, 2) };
    if runs.is_empty() {
        processor.mxcsr |= before | after;
        return Ok(());
    }
    // No alignment is checked, as the chip shows.
    for &(start, bytes) in &runs {
        check_canonical(MemoryOperand { address: operand.address.wrapping_add(start), stack: operand.stack }, bytes)?;
    }
    let mut written = Vec::new();
    for &(start, bytes) in &runs {
        let address = operand.address.wrapping_add(start);
        if let Some((fault, page_present)) = find_fault(memory, address, bytes, Access::Write) {
            // Masked, the chip reports the operand's first byte when its page refuses the store, else the last
            // selected byte on a page that does.
            let same_page = fault / crate::memory::PAGE_BYTES == operand.address / crate::memory::PAGE_BYTES;
            let fault = match decoded.op_mask() {
                Register::None => fault,
                _ if is_active(Bug::HalfStoreFaultAtFirstByte) => fault,
                _ if same_page => operand.address,
                _ => find_last_refused(memory, operand.address, &runs).unwrap_or(fault),
            };
            if across_fault == StoreAcrossFault::WritesBytesBelowFault {
                written.push((start, fault.wrapping_sub(address)));
                for &(start, bytes) in &written {
                    if bytes > 0 {
                        let slice = &all[start as usize..(start + bytes) as usize];
                        on_memory_mutated(MemoryDelta::ValueAssignment { start: operand.address.wrapping_add(start), bytes: Cow::Owned(slice.to_vec()) });
                    }
                }
            }
            return Err(Exception::PageFault { access: Access::Write, page_present, address: fault });
        }
        written.push((start, bytes));
    }
    processor.mxcsr |= before | after;
    for (start, bytes) in written {
        let slice = &all[start as usize..(start + bytes) as usize];
        on_memory_mutated(MemoryDelta::ValueAssignment { start: operand.address.wrapping_add(start), bytes: Cow::Owned(slice.to_vec()) });
    }
    Ok(())
}

/// The highest selected byte whose page refuses a write.
fn find_last_refused(memory: Memory<'_, '_>, start: u64, runs: &[(u64, u64)]) -> Option<u64> {
    runs.iter()
        .rev()
        .flat_map(|&(offset, bytes)| (0..bytes).rev().map(move |byte| start.wrapping_add(offset + byte)))
        .find(|&address| find_fault(memory, address, 1, Access::Write).is_some())
}

/// The VEX forms of RCPPS to RSQRTSS: each lane from the legacy forms' tables; a scalar form keeps its first
/// source's other lanes, and an XMM destination's upper half is zeroed.
fn vex_reciprocal(
    form: &OptionalForm,
    root: bool,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
) -> Result<(), Exception> {
    let scalar = form.is_scalar();
    let source_index = if scalar { 2 } else { 1 };
    let length = form.memory_bytes(false);
    let source: Zmm = match decoded.op_kind(source_index) {
        OpKind::Register => processor.read_zmm(decoded.op_register(source_index).number()),
        _ => {
            let operand = locate(processor, decoded);
            check_access(processor.flags, memory, operand, length, Access::Read)?;
            let bytes: [u8; 32] = read_bytes(memory, operand.address);
            let low = u128::from_le_bytes(bytes[..16].try_into().expect("16 bytes"));
            let high = u128::from_le_bytes(bytes[16..].try_into().expect("16 bytes"));
            let all = [low, high, 0, 0];
            let mut kept: Zmm = [0; 4];
            for index in 0..(length / 4) as usize {
                write_element(&mut kept, index, 4, read_element(&all, index, 4));
            }
            kept
        }
    };
    let destination = decoded.op0_register().number();
    let table = &XEON_PLATINUM_8488C_RECIPROCALS;
    let mut result: Zmm = if scalar { [processor.read_zmm(decoded.op1_register().number())[0], 0, 0, 0] } else { [0; 4] };
    for index in 0..form.elements() {
        let lane = read_element(&source, index, 4) as u32;
        write_element(&mut result, index, 4, u64::from(compute(table, root, lane)));
    }
    if is_active(Bug::VexReciprocalKeepsUpperHalf) && form.vector_bytes() == 16 {
        result[1] = processor.read_zmm(destination)[1];
    }
    let mut whole = processor.read_zmm(destination);
    whole[0] = result[0];
    whole[1] = result[1];
    processor.write_zmm(destination, whole);
    Ok(())
}

/// VRCP14, VRSQRT14, VRCPPH and VRSQRTPH: each selected element computed, the others merged or zeroed; a scalar
/// form keeps its first source's other elements up to 127, and every form zeroes the destination above its vector.
fn evex_reciprocal(
    form: &OptionalForm,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    compute: impl Fn(u64, u32) -> u64,
) -> Result<(), Exception> {
    let scalar = form.is_scalar();
    let elements = form.elements();
    let bytes = form.element_bytes() as usize;
    let selected = read_selection(processor, decoded, elements);
    let source_index = if scalar { 2 } else { 1 };
    let source: Zmm = match decoded.op_kind(source_index) {
        OpKind::Register => processor.read_zmm(decoded.op_register(source_index).number()),
        _ => read_masked_source(form, processor, memory, decoded, selected)?,
    };
    let destination = decoded.op0_register().number();
    let old = processor.read_zmm(destination);
    let mut result: Zmm = if scalar { [processor.read_zmm(decoded.op1_register().number())[0], 0, 0, 0] } else { [0; 4] };
    for index in 0..elements {
        let value = if selected & (1 << index) != 0 {
            compute(read_element(&source, index, bytes), processor.mxcsr)
        } else if decoded.zeroing_masking() {
            0
        } else {
            read_element(&old, index, bytes)
        };
        write_element(&mut result, index, bytes, value);
    }
    processor.write_zmm(destination, result);
    Ok(())
}

/// A memory source of selected elements: the whole reference checked as a plain access without a writemask, each
/// selected element's bytes canonical and then the reference's alignment with one, then each selected element read;
/// a broadcast reads its one element for every element.
fn read_masked_source(
    form: &OptionalForm,
    processor: &Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    selected: u64,
) -> Result<Zmm, Exception> {
    let operand = locate(processor, decoded);
    let broadcast = decoded.is_broadcast();
    let bytes = form.element_bytes();
    let elements = if form.is_scalar() { 1 } else { form.elements() };
    let reference = form.memory_bytes(broadcast);
    let read = |index: usize| selected & (1 << index) != 0;
    let to_element = |index: usize| {
        let at = if broadcast { 0 } else { index as u64 * bytes };
        MemoryOperand { address: operand.address.wrapping_add(at), stack: operand.stack }
    };
    let unchecked = processor.flags.difference(Flags::ALIGNMENT_CHECK);
    let masked = decoded.op_mask() != Register::None && !is_active(Bug::OptionalMaskedFaultNotSuppressed);
    if !masked {
        check_access(processor.flags, memory, operand, reference, Access::Read)?;
    } else if (0..elements).any(read) {
        for index in (0..elements).filter(|index| read(*index)) {
            check_canonical(to_element(index), bytes)?;
        }
        check_alignment(processor.flags, operand, reference)?;
    }
    let mut vector: Zmm = [0; 4];
    for index in (0..elements).filter(|index| !masked || read(*index)) {
        let element = to_element(index);
        check_access(unchecked, memory, element, bytes, Access::Read)?;
        let value: [u8; 8] = read_bytes(memory, element.address);
        write_element(&mut vector, index, bytes as usize, u64::from_le_bytes(value));
    }
    Ok(vector)
}

/// MOVDIR64B: the source is read, then the destination's address must lie on 64 bytes and be canonical, and the
/// destination is written.
fn store_block(
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let source = locate(processor, decoded);
    check_access(processor.flags.difference(Flags::ALIGNMENT_CHECK), memory, source, BLOCK_BYTES, Access::Read)?;
    let block: [u8; BLOCK_BYTES as usize] = read_bytes(memory, source.address);
    let destination = to_register_address(processor, decoded.op0_register());
    let aligned = destination.address.is_multiple_of(BLOCK_BYTES) || is_active(Bug::BlockStoreAlignmentUnchecked);
    if !aligned || !is_canonical(destination.address) {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    store(processor.flags, memory, destination, &block, StoreAcrossFault::WritesNothing, &mut on_memory_mutated)
}

/// The address a register operand holds, as wide as the register (a 32-bit register under the 0x67 prefix).
fn to_register_address(processor: &Processor, register: Register) -> MemoryOperand {
    MemoryOperand { address: read_register(processor, register), stack: false }
}
