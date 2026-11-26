//! XGETBV, XSAVE, XSAVEC, XSAVEOPT, XRSTOR, UMWAIT and TPAUSE: each area's accesses in the order of its bytes, and the
//! registers, XINUSE and the tracking changed only once nothing faults.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;

use iced_x86::{Code, Instruction as Decoded};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, MemoryOperand, check_canonical, expect_code, find_fault, locate, read_register};
use crate::behavior::StoreAcrossFault;
use crate::planted::{Bug, is_active};
use crate::x87_state;
use crate::xsave::{
    AREA_ALIGNMENT, AVX_COMPONENT, HEADER_OFFSET, HIGH_16_ZMM_COMPONENT, MXCSR_RESET,
    MisalignedArea, OPMASK_COMPONENT, PKRU_COMPONENT, RestoredInitial, SSE_COMPONENT, SaveFormat, StateBehavior,
    StateOperation, WaitControl, X87_COMPONENT, ZMM_HIGH_256_COMPONENT, find_state_form, list_component_offsets, to_extent,
    to_tracking,
};
use crate::{Access, Exception, Flags, Instruction, Processor, VECTOR_REGISTERS, X87_INITIAL, X86_64};

/// The bits of MXCSR this chip implements, which XSAVE stores as MXCSR_MASK.
const MXCSR_MASK: u32 = 0xFFFF;
/// The legacy region's parts: x87's first 24 bytes, MXCSR and MXCSR_MASK, x87's registers, and XMM0 to XMM15.
const X87_HEAD: core::ops::Range<usize> = 0..24;
const MXCSR_FIELDS: core::ops::Range<usize> = 24..32;
const X87_REGISTERS: core::ops::Range<usize> = 32..160;
const SSE_REGISTERS: core::ops::Range<usize> = 160..416;
/// XCOMP_BV's bit that marks the compacted format.
const COMPACTED: u64 = 1 << 63;
/// The flags UMWAIT and TPAUSE write: CF from the time, OF, SF, ZF, AF and PF cleared.
const WAIT_FLAGS: Flags = Flags::CARRY
    .union(Flags::OVERFLOW)
    .union(Flags::SIGN)
    .union(Flags::ZERO)
    .union(Flags::AUXILIARY_CARRY)
    .union(Flags::PARITY);

/// One run of bytes a form writes, at its offset in the area.
struct Piece {
    offset: u64,
    bytes: Vec<u8>,
}

pub(super) fn execute_state(
    instruction: Instruction,
    behavior: StateBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_state_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no form of this family"));
    match (form.operation, behavior) {
        (StateOperation::ReadControl, _) => {
            expect_code(decoded, &[form.code]);
            read_control(processor)
        }
        (StateOperation::Wait, StateBehavior::Wait(control)) => {
            let wide = if form.code == Code::Umwait_r32 { Code::Umwait_r64 } else { Code::Tpause_r64 };
            expect_code(decoded, &[form.code, wide]);
            wait(control, processor, decoded)
        }
        (StateOperation::Save { format, wide }, StateBehavior::Save { misaligned, across_fault }) => {
            expect_code(decoded, &[form.code]);
            let operand = locate(processor, decoded);
            check_area(processor.flags, operand, misaligned)?;
            let pieces = list_saved_pieces(processor, memory, operand.address, format, wide);
            let standard = format != SaveFormat::Compacted || is_active(Bug::CompactedStandardOffsets);
            let extent = to_extent(to_requested(processor), standard);
            write_pieces(memory, operand.address, extent, &pieces, across_fault, &mut on_memory_mutated)
        }
        (StateOperation::Restore { wide }, StateBehavior::Restore { misaligned, restored_initial }) => {
            expect_code(decoded, &[form.code]);
            let operand = locate(processor, decoded);
            check_area(processor.flags, operand, misaligned)?;
            restore(processor, memory, operand.address, wide, restored_initial)
        }
        (operation, behavior) => panic!("{behavior:?} is no behavior of {operation:?}"),
    }
}

/// XGETBV: XCR0 for ECX 0; XCR0 AND XINUSE for ECX 1, SSE's bit also set where MXCSR is not its reset value; #GP(0)
/// for any other ECX. RAX and RDX take the low and high halves, zero-extended.
fn read_control(processor: &mut Processor) -> Result<(), Exception> {
    let registers = &mut processor.general_purpose_registers;
    let value = match registers[1] as u32 {
        0 => processor.xcr0,
        1 if is_active(Bug::ControlReadIgnoresInUse) => processor.xcr0,
        1 => {
            let changed_mxcsr = processor.mxcsr != MXCSR_RESET && !is_active(Bug::ControlReadIgnoresMxcsr);
            let mxcsr = if changed_mxcsr { SSE_COMPONENT } else { 0 };
            processor.xcr0 & (processor.xinuse | mxcsr)
        }
        _ => return Err(Exception::GeneralProtection { error_code: 0 }),
    };
    registers[0] = value & u64::from(u32::MAX);
    registers[2] = value >> 32;
    Ok(())
}

/// UMWAIT and TPAUSE: #GP(0) for a source with a bit above bit 0, else OF, SF, ZF, AF and PF cleared and CF from the
/// time, which the expectation does not compare.
fn wait(control: WaitControl, processor: &mut Processor, decoded: &Decoded) -> Result<(), Exception> {
    if control == WaitControl::FaultsUnderTimeStampDisable {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    let source = read_register(processor, decoded.op0_register()) as u32;
    if source & !1 != 0 && !is_active(Bug::WaitIgnoresReservedBits) {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    if !is_active(Bug::WaitKeepsFlags) {
        processor.flags.remove(WAIT_FLAGS);
    }
    Ok(())
}

/// #GP(0) for a first byte that is not canonical; then for an area not aligned to 64, #AC where the behavior checks
/// the width the address misses and AC is set, else #GP(0); then #GP(0) or #SS(0) for a last byte that is not
/// canonical.
fn check_area(flags: Flags, operand: MemoryOperand, misaligned: MisalignedArea) -> Result<(), Exception> {
    check_canonical(operand, 1)?;
    let alignment = if is_active(Bug::AreaAlignedToSixteen) { 16 } else { AREA_ALIGNMENT };
    if !operand.address.is_multiple_of(alignment) {
        return Err(match misaligned {
            MisalignedArea::AlignmentCheckBelow { width }
                if flags.contains(Flags::ALIGNMENT_CHECK) && !operand.address.is_multiple_of(width) =>
            {
                Exception::AlignmentCheck
            }
            _ => Exception::GeneralProtection { error_code: 0 },
        });
    }
    check_canonical(operand, crate::xsave::STANDARD_AREA_BYTES)
}

/// The requested-feature bitmap: XCR0 AND EDX:EAX, the upper halves of RDX and RAX ignored.
fn to_requested(processor: &Processor) -> u64 {
    let registers = &processor.general_purpose_registers;
    let (high, low) = if is_active(Bug::SaveRequestHalvesSwapped) { (registers[0], registers[2]) } else { (registers[2], registers[0]) };
    let mask = u64::from(high as u32) << 32 | u64::from(low as u32);
    processor.xcr0 & mask
}

/// The bytes a save writes, lowest first, from the processor and, for XSAVE's and XSAVEOPT's header, the old
/// XSTATE_BV in memory.
fn list_saved_pieces(processor: &Processor, memory: Memory<'_, '_>, address: u64, format: SaveFormat, wide: bool) -> Vec<Piece> {
    let requested = to_requested(processor);
    let in_use = processor.xinuse;
    let wide_pointers = wide || is_active(Bug::NarrowSaveWidePointers);
    let legacy = x87_state::write_extended(&processor.x87, processor.mxcsr, MXCSR_MASK, &processor.vector_registers, wide_pointers);
    let tracked = processor
        .xsave_tracking
        .filter(|tracking| tracking.area == address)
        .map_or(0, |tracking| tracking.unmodified);
    // The components saved, and the header's bitmaps.
    let (saved, state_bitmap, compaction) = match format {
        SaveFormat::Standard => (requested, to_kept_bitmap(memory, address, requested, in_use), None),
        SaveFormat::Optimized => {
            let optimized = if is_active(Bug::OptimizedSavesInitial) { requested } else { requested & in_use & !tracked };
            (optimized, to_kept_bitmap(memory, address, requested, in_use), None)
        }
        SaveFormat::Compacted => {
            let mxcsr_changed = processor.mxcsr != MXCSR_RESET && !is_active(Bug::CompactedIgnoresMxcsr);
            let sse = if requested & SSE_COMPONENT != 0 && mxcsr_changed { SSE_COMPONENT } else { 0 };
            let bitmap = requested & in_use | sse;
            (bitmap, bitmap, Some(requested | COMPACTED))
        }
    };
    let mut pieces = Vec::new();
    let mut push = |offset: usize, bytes: &[u8]| pieces.push(Piece { offset: offset as u64, bytes: bytes.to_vec() });
    if saved & X87_COMPONENT != 0 {
        push(X87_HEAD.start, &legacy[X87_HEAD]);
    }
    let mxcsr_saved = match format {
        SaveFormat::Compacted => saved & SSE_COMPONENT != 0,
        _ if is_active(Bug::SaveMxcsrOnlyWithSse) => requested & SSE_COMPONENT != 0,
        _ => requested & (SSE_COMPONENT | AVX_COMPONENT) != 0,
    };
    if mxcsr_saved {
        push(MXCSR_FIELDS.start, &legacy[MXCSR_FIELDS]);
    }
    if saved & X87_COMPONENT != 0 {
        push(X87_REGISTERS.start, &legacy[X87_REGISTERS]);
    }
    if saved & SSE_COMPONENT != 0 {
        push(SSE_REGISTERS.start, &legacy[SSE_REGISTERS]);
    }
    let mut header = state_bitmap.to_le_bytes().to_vec();
    if let Some(compaction) = compaction {
        header.extend_from_slice(&compaction.to_le_bytes());
    }
    push(HEADER_OFFSET as usize, &header);
    let standard = compaction.is_none() || is_active(Bug::CompactedStandardOffsets);
    for (component, offset) in list_component_offsets(requested, standard) {
        if saved & component.bit != 0 {
            push(offset as usize, &to_component_bytes(processor, component.bit));
        }
    }
    pieces
}

/// XSAVE's and XSAVEOPT's XSTATE_BV: the old one's bits outside the request, XINUSE's inside it.
fn to_kept_bitmap(memory: Memory<'_, '_>, address: u64, requested: u64, in_use: u64) -> u64 {
    let old = u64::from_le_bytes(read_area(memory, address.wrapping_add(HEADER_OFFSET), 8).try_into().expect("eight bytes"));
    let kept = if is_active(Bug::SaveHeaderDropsUnrequested) { 0 } else { old & !requested };
    kept | in_use & requested
}


/// The bytes an extended component holds in the area.
fn to_component_bytes(processor: &Processor, component: u64) -> Vec<u8> {
    let halves = |values: &mut dyn Iterator<Item = u128>| values.flat_map(u128::to_le_bytes).collect::<Vec<u8>>();
    match component {
        AVX_COMPONENT => halves(&mut processor.upper_vector_halves.iter().copied()),
        OPMASK_COMPONENT => processor.opmask_registers.iter().flat_map(|value| value.to_le_bytes()).collect(),
        ZMM_HIGH_256_COMPONENT => halves(&mut processor.zmm_upper_halves.iter().flatten().copied()),
        HIGH_16_ZMM_COMPONENT => halves(&mut processor.high_vector_registers.iter().flatten().copied()),
        PKRU_COMPONENT => processor.pkru.to_le_bytes().to_vec(),
        _ => panic!("{component:#x} is no extended component"),
    }
}

/// The lowest byte of the access whose page refuses it, and whether that page is present, found page by page.
fn find_page_fault(memory: Memory<'_, '_>, address: u64, extent: u64, access: Access) -> Option<(u64, bool)> {
    const PAGE_BYTES: u64 = crate::memory::PAGE_BYTES;
    let first_page = address & !(PAGE_BYTES - 1);
    let pages = (address.wrapping_add(extent - 1) & !(PAGE_BYTES - 1)).wrapping_sub(first_page) / PAGE_BYTES + 1;
    (0..pages).find_map(|page| {
        let byte = if page == 0 { address } else { first_page.wrapping_add(page * PAGE_BYTES) };
        find_fault(memory, byte, 1, access)
    })
}

/// The page fault of an access of `extent` bytes, which the chip reports at the access's last byte wherever the
/// byte that faults lies.
fn find_area_fault(memory: Memory<'_, '_>, address: u64, extent: u64, access: Access) -> Option<Exception> {
    let (faulting, page_present) = find_page_fault(memory, address, extent, access)?;
    let reported = if is_active(Bug::AreaFaultsAtLowestByte) { faulting } else { address.wrapping_add(extent - 1) };
    Some(Exception::PageFault { access, page_present, address: reported })
}

/// Writes the pieces, or faults where any byte of the access's extent refuses the write, having written the pieces'
/// bytes below the lowest such byte where the behavior does.
fn write_pieces(
    memory: Memory<'_, '_>,
    address: u64,
    extent: u64,
    pieces: &[Piece],
    across_fault: StoreAcrossFault,
    on_memory_mutated: &mut impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let fault = find_page_fault(memory, address, extent, Access::Write);
    // Pieces that meet are written as one run.
    let mut run: Option<(u64, Vec<u8>)> = None;
    for piece in pieces {
        let start = address.wrapping_add(piece.offset);
        let length = match fault {
            None => piece.bytes.len(),
            Some(_) if across_fault == StoreAcrossFault::WritesNothing => 0,
            Some((faulting, _)) => faulting.saturating_sub(start).min(piece.bytes.len() as u64) as usize,
        };
        if length == 0 {
            continue;
        }
        match &mut run {
            Some((run_start, bytes)) if run_start.wrapping_add(bytes.len() as u64) == start => {
                bytes.extend_from_slice(&piece.bytes[..length]);
            }
            _ => {
                if let Some((run_start, bytes)) = run.take() {
                    on_memory_mutated(MemoryDelta::ValueAssignment { start: run_start, bytes: Cow::Owned(bytes) });
                }
                run = Some((start, piece.bytes[..length].to_vec()));
            }
        }
    }
    if let Some((start, bytes)) = run {
        on_memory_mutated(MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) });
    }
    match find_area_fault(memory, address, extent, Access::Write) {
        None => Ok(()),
        Some(exception) => Err(exception),
    }
}

/// XRSTOR: reads the header and checks it, then loads or initializes each requested component and MXCSR, and sets
/// XINUSE and the tracking.
fn restore(
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    address: u64,
    wide: bool,
    restored_initial: RestoredInitial,
) -> Result<(), Exception> {
    let requested = to_requested(processor);
    let header_address = address.wrapping_add(HEADER_OFFSET);
    // XCOMP_BV first, which chooses the form, at its own first byte.
    let compaction_address = header_address.wrapping_add(8);
    let compaction_bytes = if is_active(Bug::RestoreReadsHeaderFromStart) { 16 } else { 8 };
    let compaction_start = if compaction_bytes == 16 { header_address } else { compaction_address };
    if let Some((faulting, page_present)) = find_page_fault(memory, compaction_start, compaction_bytes, Access::Read) {
        return Err(Exception::PageFault { access: Access::Read, page_present, address: faulting });
    }
    let header = read_area(memory, header_address, crate::xsave::HEADER_BYTES);
    let quad = |index: usize| u64::from_le_bytes(header[index..index + 8].try_into().expect("eight bytes"));
    let (state_bitmap, compaction) = (quad(0), quad(8));
    let compacted = compaction & COMPACTED != 0;
    let offsets = list_component_offsets(if compacted { compaction & !COMPACTED } else { requested }, !compacted);
    // Then every byte of the request's components in the form's layout, reported at the last.
    let extent = offsets
        .iter()
        .filter(|(component, _)| requested & component.bit != 0)
        .map(|(component, offset)| offset + component.bytes)
        .max()
        .unwrap_or(HEADER_OFFSET + crate::xsave::HEADER_BYTES);
    if let Some(exception) = find_area_fault(memory, address, extent, Access::Read) {
        return Err(exception);
    }
    let refused = if compacted {
        compaction & !COMPACTED & !processor.xcr0 != 0 || state_bitmap & !compaction != 0 || header[16..].iter().any(|byte| *byte != 0)
    } else {
        let checked = if is_active(Bug::RestoreAcceptsUnenabledComponents) { 0 } else { !processor.xcr0 };
        state_bitmap & checked != 0 || header[8..24].iter().any(|byte| *byte != 0)
    };
    if refused {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    let area = read_area(memory, address, extent.max(HEADER_OFFSET + crate::xsave::HEADER_BYTES));
    let legacy = &area[..416];
    let loads_mxcsr = if compacted {
        requested & SSE_COMPONENT != 0 && state_bitmap & SSE_COMPONENT != 0
    } else if is_active(Bug::RestoreMxcsrOnlyWithSse) {
        requested & SSE_COMPONENT != 0
    } else {
        requested & (SSE_COMPONENT | AVX_COMPONENT) != 0
    };
    let mxcsr = u32::from_le_bytes(legacy[MXCSR_FIELDS.start..MXCSR_FIELDS.start + 4].try_into().expect("four bytes"));
    if loads_mxcsr && mxcsr & !MXCSR_MASK != 0 {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    if loads_mxcsr {
        processor.mxcsr = mxcsr;
    } else if compacted && requested & SSE_COMPONENT != 0 {
        processor.mxcsr = MXCSR_RESET;
    }
    let loaded = requested & state_bitmap;
    let (mut x87, _, vectors) = x87_state::read_extended(legacy, wide);
    x87_state::set_summary(&mut x87);
    if requested & X87_COMPONENT != 0 {
        processor.x87 = if loaded & X87_COMPONENT != 0 { x87 } else { X87_INITIAL };
    }
    if requested & SSE_COMPONENT != 0 {
        processor.vector_registers = if loaded & SSE_COMPONENT != 0 { vectors } else { [0; VECTOR_REGISTERS] };
    }
    for (component, offset) in offsets.into_iter().filter(|(component, _)| requested & component.bit != 0) {
        let bytes = if loaded & component.bit != 0 {
            area[offset as usize..(offset + component.bytes) as usize].to_vec()
        } else {
            vec![0; component.bytes as usize]
        };
        load_component(processor, component.bit, &bytes);
    }
    let initialized = requested & !state_bitmap;
    let in_use_initialized = match restored_initial {
        RestoredInitial::TrackedInUse => initialized,
        RestoredInitial::TrackedInitialButPkru => initialized & PKRU_COMPONENT,
        RestoredInitial::TrackedInitial => 0,
    };
    processor.xinuse = processor.xinuse & !requested | loaded | in_use_initialized;
    // PKRU that the behavior leaves in use when it puts it in its initial configuration is written, so modified.
    let unmodified = if is_active(Bug::RestoreTracksModified) { 0 } else { requested & !(in_use_initialized & PKRU_COMPONENT) };
    processor.xsave_tracking = if compacted { None } else { to_tracking(address, unmodified, processor.xinuse) };
    Ok(())
}

/// The bytes memory holds from the address on, each change laid over the ones before it, zero where none wrote.
fn read_area(memory: Memory<'_, '_>, address: u64, length: u64) -> Vec<u8> {
    let mut area = vec![0; length as usize];
    for mutation in memory {
        if let MemoryDelta::ValueAssignment { start, bytes } = mutation {
            let end = start.wrapping_add(bytes.len() as u64);
            let (from, to) = ((*start).max(address), end.min(address.wrapping_add(length)));
            if from < to && *start < end {
                area[(from - address) as usize..(to - address) as usize]
                    .copy_from_slice(&bytes[(from - start) as usize..(to - start) as usize]);
            }
        }
    }
    area
}

/// Loads an extended component's registers from its bytes in the area.
fn load_component(processor: &mut Processor, component: u64, bytes: &[u8]) {
    let half = |index: usize| u128::from_le_bytes(bytes[16 * index..16 * index + 16].try_into().expect("sixteen bytes"));
    match component {
        AVX_COMPONENT => processor.upper_vector_halves = core::array::from_fn(half),
        OPMASK_COMPONENT => {
            processor.opmask_registers =
                core::array::from_fn(|index| u64::from_le_bytes(bytes[8 * index..8 * index + 8].try_into().expect("eight bytes")));
        }
        ZMM_HIGH_256_COMPONENT => processor.zmm_upper_halves = core::array::from_fn(|number| core::array::from_fn(|part| half(2 * number + part))),
        HIGH_16_ZMM_COMPONENT => {
            processor.high_vector_registers = core::array::from_fn(|number| core::array::from_fn(|part| half(4 * number + part)));
        }
        PKRU_COMPONENT => processor.pkru = u32::from_le_bytes(bytes[..4].try_into().expect("four bytes")),
        _ => panic!("{component:#x} is no extended component"),
    }
}
