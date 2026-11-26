//! The inputs of the string forms: the address size; where the source and destination elements lie, around the
//! boundary of a pair of pages each, and through which segment the source is read; how the code lies; DF and AC; the
//! count of a repeated form; and the accumulator a store or a scan takes, or the two elements a comparison takes.
//!
//! One element varies at a time while the other lies at the start of a writable page, so that each fault and each
//! store across a page is tried once against a form that would otherwise complete, and once both faults meet to show
//! which comes first. The source's pages hold the domain's data bytes; the destination's another pattern, so that a
//! moved element changes memory.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::string::{Repetition, StringForm, StringOperation, find_string_form};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, Flags, Instruction, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{CodeLayout, lay_out};
use crate::{
    AccessPlacement, CODE_ADDRESS, DATA_PAGES, FlagSet, LOWER_DATA_ADDRESS, NON_CANONICAL_ADDRESS, OperandType,
    OperandValue, SegmentChoice, to_access_address, to_data_pages,
};

/// How far the destination's pair of pages lies above the source's.
const DESTINATION_SHIFT: u64 = 0x6000;
/// The writable page that ends at 4 GiB.
const FOUR_GIB_PAGE: u64 = (1 << u32::BITS) - PAGE_BYTES;
/// Small FS base, so that ESI reaches the data pages through it.
const FS_BASE: u64 = 0x1000;
/// A GS base in the upper half, which the address wraps past zero from.
const GS_BASE: u64 = 0xFFFF_FFFF_FFFF_F000;
/// What a register 32-bit addressing reads holds above its low half.
const ABOVE_ADDRESS_HALF: u64 = 0xA5A5_A5A5_0000_0000;
/// What the accumulator holds before LODS, so that a narrow load is seen to keep the bits above it.
const LOAD_ACCUMULATOR: u64 = 0xA5A5_A5A5_A5A5_A5A5;
const ACCUMULATOR: usize = 0;
const COUNT: usize = 1;
const SOURCE_INDEX: usize = 6;
const DESTINATION_INDEX: usize = 7;
const READ_WRITE: PagePermission =
    PagePermission::Mapped { data_access: DataAccess::ReadWrite, instruction_fetch: InstructionFetch::Forbidden };
const READ_ONLY: PagePermission =
    PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Forbidden };
const UNMAPPED: PagePermission = PagePermission::Unmapped;

/// The bytes of the destination's pair of pages: each the low byte of its address exclusive-ored with 0x5A.
static DESTINATION_PAGES: [u8; 2 * PAGE_BYTES as usize] = {
    let mut bytes = [0; 2 * PAGE_BYTES as usize];
    let mut offset = 0;
    while offset < bytes.len() {
        bytes[offset] = (offset as u8) ^ 0x5A;
        offset += 1;
    }
    bytes
};

/// Where one element lies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    /// Around the boundary of the element's pair of pages.
    Pages { placement: AccessPlacement, lower: PagePermission, upper: PagePermission },
    /// Its last byte the last below 4 GiB, on a writable page, so that the next element wraps to zero under the 0x67
    /// prefix.
    EndsAtFourGiB,
}

/// The elements' placements, the source's segment, the code's layout, and whether the 0x67 prefix makes the
/// addresses and the count 32-bit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StringLayout {
    pub source: Placement,
    pub destination: Placement,
    pub segment: SegmentChoice,
    pub code: CodeLayout,
    pub narrow: bool,
}

/// What the data operand of a string form holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StringDataSet {
    /// The two elements a comparison takes, as wide as `bytes`; for a repeated form only those that tell equal from
    /// different both ways.
    Comparison { bytes: u64, repeated: bool },
    /// The accumulator STOS stores.
    Accumulator,
}

pub(crate) const PLAIN: Placement = Placement::Pages { placement: AccessPlacement::LowerPageStart, lower: READ_WRITE, upper: UNMAPPED };

/// The placements one element takes: the plain one first, then each fault, each way across the boundary, and the
/// edges of the address space; an element of one byte never crosses a boundary nor is misaligned.
pub(crate) fn list_placements(bytes: u64) -> Vec<Placement> {
    let pages = |placement, lower, upper| Placement::Pages { placement, lower, upper };
    let mut placements = vec![
        PLAIN,
        pages(AccessPlacement::LowerPageStart, READ_ONLY, UNMAPPED),
        pages(AccessPlacement::LowerPageStart, UNMAPPED, UNMAPPED),
        pages(AccessPlacement::EndsAtBoundary, READ_WRITE, UNMAPPED),
        pages(AccessPlacement::UpperPageStart, UNMAPPED, READ_WRITE),
        pages(AccessPlacement::NonCanonical, UNMAPPED, UNMAPPED),
        Placement::EndsAtFourGiB,
    ];
    if bytes > 1 {
        placements.extend([
            pages(AccessPlacement::Misaligned, READ_WRITE, UNMAPPED),
            pages(AccessPlacement::StraddlesBoundary, READ_WRITE, UNMAPPED),
            pages(AccessPlacement::StraddlesBoundary, READ_WRITE, READ_ONLY),
            pages(AccessPlacement::StraddlesBoundary, READ_WRITE, READ_WRITE),
            pages(AccessPlacement::StraddlesBoundary, UNMAPPED, READ_WRITE),
        ]);
    }
    placements
}

/// The operand types of a string form, in the order the build reads them.
pub fn list_operand_types(form: &StringForm) -> Vec<OperandType> {
    let port = matches!(form.operation, StringOperation::Input | StringOperation::Output);
    let mut types = vec![
        OperandType::StringLayout {
            source: form.reads_source() && !port,
            destination: form.reaches_destination() && !port,
            bytes: form.bytes,
        },
        OperandType::Flags(FlagSet::Condition(Flags::DIRECTION.union(Flags::ALIGNMENT_CHECK))),
    ];
    if form.is_repeated() {
        types.push(OperandType::StringCount);
    }
    match form.operation {
        StringOperation::Scan | StringOperation::Compare => {
            types.push(OperandType::StringData(StringDataSet::Comparison { bytes: form.bytes, repeated: form.is_repeated() }));
        }
        StringOperation::Store => types.push(OperandType::StringData(StringDataSet::Accumulator)),
        _ => {}
    }
    types
}

/// The layouts of a form that reaches the source, the destination, both or neither: each with 64-bit addresses, then
/// under the 0x67 prefix those whose addresses or registers it changes, the plain one, a fault of each element, the
/// element that ends at 4 GiB and each segment.
pub fn generate_layouts(source: bool, destination: bool, bytes: u64) -> Vec<OperandValue> {
    let layout = |source, destination| StringLayout {
        source,
        destination,
        segment: SegmentChoice::Default,
        code: CodeLayout::PageStart,
        narrow: false,
    };
    let placements = list_placements(bytes);
    let mut layouts: Vec<StringLayout> = match (source, destination) {
        (true, true) => placements
            .iter()
            .map(|&placement| layout(placement, PLAIN))
            .chain(placements[1..].iter().map(|&placement| layout(PLAIN, placement)))
            .chain([layout(placements[2], placements[2])])
            .collect(),
        (true, false) => placements.iter().map(|&placement| layout(placement, PLAIN)).collect(),
        (false, true) => placements.iter().map(|&placement| layout(PLAIN, placement)).collect(),
        (false, false) => vec![layout(PLAIN, PLAIN)],
    };
    if source {
        layouts.extend([SegmentChoice::Fs, SegmentChoice::Gs].map(|segment| StringLayout { segment, ..layout(PLAIN, PLAIN) }));
    }
    layouts.extend(
        [CodeLayout::Padded15, CodeLayout::Padded16, CodeLayout::AcrossPageIntoUnmapped]
            .map(|code| StringLayout { code, ..layout(PLAIN, PLAIN) }),
    );
    let unmapped = placements[2];
    let mut narrow = vec![layout(PLAIN, PLAIN)];
    if source {
        narrow.extend([layout(unmapped, PLAIN), layout(Placement::EndsAtFourGiB, PLAIN)]);
        narrow.extend([SegmentChoice::Fs, SegmentChoice::Gs].map(|segment| StringLayout { segment, ..layout(PLAIN, PLAIN) }));
    }
    if destination {
        narrow.extend([layout(PLAIN, unmapped), layout(PLAIN, Placement::EndsAtFourGiB)]);
    }
    layouts.extend(narrow.into_iter().map(|chosen| StringLayout { narrow: true, ..chosen }));
    layouts.into_iter().map(OperandValue::StringLayout).collect()
}

/// A repeated form's count: none, one, and with bits above the low half that 32-bit addressing ignores none, one and
/// all, so that each address size meets a count that runs out and one that does not.
pub fn generate_counts() -> Vec<OperandValue> {
    [0, 1, 1 << 32, (1 << 32) | 1, u64::MAX].map(OperandValue::Bits).to_vec()
}

/// The comparison's elements, the first less the second: equal, below, above, a signed overflow, a borrow out of
/// the low nibble alone, a borrow out of bit 3 that stops there (8 less 1, the one pair whose bits 3 and 4 of the
/// operands and difference differ, so that AF is told from bit 3), and zero less every bit; or two accumulators that
/// differ in every byte.
pub fn generate_data(set: StringDataSet) -> Vec<OperandValue> {
    let pairs: Vec<(u64, u64)> = match set {
        StringDataSet::Comparison { bytes, repeated: true } => {
            vec![(0x0123_4567_89AB_CDEF & to_mask(bytes), 0x0123_4567_89AB_CDEF & to_mask(bytes)), (1, 2), (2, 1), (0x08, 0x01)]
        }
        StringDataSet::Comparison { bytes, repeated: false } => {
            let mask = to_mask(bytes);
            let top = 1u64 << (8 * bytes - 1);
            vec![
                (0x0123_4567_89AB_CDEF & mask, 0x0123_4567_89AB_CDEF & mask),
                (1, 2),
                (2, 1),
                (top, 1),
                (0x10, 0x01),
                (0x08, 0x01),
                (0, mask),
            ]
        }
        StringDataSet::Accumulator => vec![(0x0123_4567_89AB_CDEF, 0), (0xFEDC_BA98_7654_3210, 0)],
    };
    pairs.into_iter().map(|(first, second)| OperandValue::StringData { first, second }).collect()
}

const fn to_mask(bytes: u64) -> u64 {
    if bytes >= 8 { u64::MAX } else { (1 << (8 * bytes)) - 1 }
}

/// The address of the element's first byte; the destination's pair of pages lies above the source's.
pub(crate) fn to_address(placement: Placement, bytes: u64, destination: bool) -> u64 {
    match placement {
        Placement::Pages { placement: AccessPlacement::NonCanonical, .. } => NON_CANONICAL_ADDRESS,
        Placement::Pages { placement, .. } => {
            to_access_address(placement, bytes) + if destination { DESTINATION_SHIFT } else { 0 }
        }
        Placement::EndsAtFourGiB => FOUR_GIB_PAGE + PAGE_BYTES - bytes,
    }
}

/// Maps the element's pages with their bytes.
pub(crate) fn place(placement: Placement, destination: bool, memory: &mut Vec<MemoryDelta<'static, X86_64>>) {
    let pattern: &'static [u8] = if destination { &DESTINATION_PAGES } else { &DATA_PAGES };
    match (placement, destination) {
        (Placement::Pages { lower, upper, .. }, false) => memory.extend(to_data_pages(lower, upper)),
        (Placement::Pages { lower, upper, .. }, true) => {
            let start = LOWER_DATA_ADDRESS + DESTINATION_SHIFT;
            memory.extend([
                MemoryDelta::ValueAssignment { start, bytes: Cow::Borrowed(pattern) },
                MemoryDelta::PermissionAssignment { addresses: start..start + PAGE_BYTES, permission: lower },
                MemoryDelta::PermissionAssignment { addresses: start + PAGE_BYTES..start + 2 * PAGE_BYTES, permission: upper },
            ]);
        }
        (Placement::EndsAtFourGiB, _) => memory.extend([
            MemoryDelta::ValueAssignment { start: FOUR_GIB_PAGE, bytes: Cow::Borrowed(&pattern[..PAGE_BYTES as usize]) },
            MemoryDelta::PermissionAssignment { addresses: FOUR_GIB_PAGE..FOUR_GIB_PAGE + PAGE_BYTES, permission: READ_WRITE },
        ]),
    }
}

/// Writes the value's bytes from the address, those in the program's addresses.
fn write_at(address: u64, bytes: u64, value: u64, memory: &mut Vec<MemoryDelta<'static, X86_64>>) {
    let written: Vec<u8> = (0..bytes)
        .take_while(|offset| X86_64::ADDRESSABLE.contains(&address.wrapping_add(*offset)))
        .map(|offset| value.to_le_bytes()[offset as usize])
        .collect();
    if !written.is_empty() {
        memory.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(written) });
    }
}

/// The form's bytes: the operand-size, address-size, segment and repeat prefixes, REX.W for 64 bits, the opcode.
fn encode(form: &StringForm, narrow: bool, segment: SegmentChoice) -> Vec<u8> {
    let mut bytes = Vec::new();
    if form.bytes == 2 {
        bytes.push(0x66);
    }
    if narrow {
        bytes.push(0x67);
    }
    match segment {
        SegmentChoice::Fs => bytes.push(0x64),
        SegmentChoice::Gs => bytes.push(0x65),
        _ => {}
    }
    match form.repetition {
        Repetition::Once => {}
        Repetition::Repeat | Repetition::WhileEqual => bytes.push(0xF3),
        Repetition::WhileNotEqual => bytes.push(0xF2),
    }
    if form.bytes == 8 {
        bytes.push(0x48);
    }
    let opcode = match form.operation {
        StringOperation::Move => 0xA4,
        StringOperation::Store => 0xAA,
        StringOperation::Load => 0xAC,
        StringOperation::Scan => 0xAE,
        StringOperation::Compare => 0xA6,
        StringOperation::Input => 0x6C,
        StringOperation::Output => 0x6E,
    };
    bytes.push(opcode + u8::from(form.bytes > 1));
    bytes
}

/// Builds the state of a string form from one value of each of its operand types.
pub fn build_state(form: &StringForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut layout, mut data) = (None, None);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::StringLayout { .. }, OperandValue::StringLayout(chosen)) => layout = Some(chosen),
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (OperandType::StringCount, OperandValue::Bits(count)) => processor.general_purpose_registers[COUNT] = count,
            (OperandType::StringData(_), OperandValue::StringData { first, second }) => data = Some((first, second)),
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let layout: StringLayout = layout.expect("a layout");
    let narrow = layout.narrow;
    let bytes = form.bytes;
    let set_index = |processor: &mut Processor, number: usize, value: u64| {
        processor.general_purpose_registers[number] = if narrow { ABOVE_ADDRESS_HALF | (value & u64::from(u32::MAX)) } else { value };
    };
    let mut memory_mutations = Vec::new();
    let port = matches!(form.operation, StringOperation::Input | StringOperation::Output);
    let source_address = to_address(layout.source, bytes, false);
    let destination_address = to_address(layout.destination, bytes, true);
    if form.reads_source() && !port {
        place(layout.source, false, &mut memory_mutations);
        let base = match layout.segment {
            SegmentChoice::Fs => {
                processor.fs_base = FS_BASE;
                FS_BASE
            }
            SegmentChoice::Gs => {
                processor.gs_base = GS_BASE;
                GS_BASE
            }
            _ => 0,
        };
        set_index(&mut processor, SOURCE_INDEX, source_address.wrapping_sub(base));
    }
    if form.reaches_destination() && !port {
        place(layout.destination, true, &mut memory_mutations);
        set_index(&mut processor, DESTINATION_INDEX, destination_address);
    }
    let mask = to_mask(bytes);
    match (form.operation, data) {
        (StringOperation::Load, _) => processor.general_purpose_registers[ACCUMULATOR] = LOAD_ACCUMULATOR,
        (StringOperation::Store, Some((accumulator, _))) => processor.general_purpose_registers[ACCUMULATOR] = accumulator,
        (StringOperation::Scan, Some((accumulator, element))) => {
            processor.general_purpose_registers[ACCUMULATOR] = (LOAD_ACCUMULATOR & !mask) | accumulator;
            write_at(destination_address, bytes, element, &mut memory_mutations);
        }
        (StringOperation::Compare, Some((first, second))) => {
            write_at(source_address, bytes, first, &mut memory_mutations);
            write_at(destination_address, bytes, second, &mut memory_mutations);
        }
        _ => {}
    }
    let encoded = encode(form, narrow, layout.segment);
    let (code_page, start, code) = lay_out(layout.code, &encoded, 0x3E);
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(code) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}

/// Whether the values decide the state's outcome whatever the data operands hold: a repeated form's count is zero
/// under its address size; the code faults while it is fetched or measured; or an element the form reads faults, or
/// the element it stores faults at its first byte, whatever is stored, as a misaligned element does under AC.
pub fn decides_without_data(instruction: Instruction, operands: &[Option<OperandValue>]) -> bool {
    let Some(form) = find_string_form(instruction) else {
        return false;
    };
    let (mut layout, mut flags, mut count) = (None, Flags::empty(), None);
    for operand in operands.iter().flatten() {
        match *operand {
            OperandValue::StringLayout(chosen) => layout = Some(chosen),
            OperandValue::Flags(chosen) => flags = chosen,
            OperandValue::Bits(chosen) => count = Some(chosen),
            _ => {}
        }
    }
    let Some(layout) = layout else {
        return false;
    };
    let mask = if layout.narrow { u64::from(u32::MAX) } else { u64::MAX };
    if count.is_some_and(|count| count & mask == 0) || layout.code.faults_before_execution() {
        return true;
    }
    let bytes = form.bytes;
    let alignment_check = flags.contains(Flags::ALIGNMENT_CHECK);
    let source = form.reads_source() && refuses(layout.source, bytes, alignment_check, false, false);
    let destination = form.reaches_destination() && refuses(layout.destination, bytes, alignment_check, true, form.stores());
    source || destination
}

/// Whether an access of `bytes` at the placement faults whatever it reads or stores: misaligned under AC, not
/// canonical, or a page refusing a byte a read reaches or the first byte a store reaches.
pub(crate) fn refuses(placement: Placement, bytes: u64, alignment_check: bool, destination: bool, write: bool) -> bool {
    let address = to_address(placement, bytes, destination);
    if alignment_check && bytes > 1 && !address.is_multiple_of(bytes) {
        return true;
    }
    let Placement::Pages { placement, lower, upper } = placement else {
        return false;
    };
    if placement == AccessPlacement::NonCanonical {
        return true;
    }
    let lower_page = LOWER_DATA_ADDRESS + if destination { DESTINATION_SHIFT } else { 0 };
    let allows = |byte: u64| {
        let permission = if byte < lower_page + PAGE_BYTES { lower } else { upper };
        match permission {
            PagePermission::Unmapped => false,
            PagePermission::Mapped { data_access, .. } => !write || data_access == DataAccess::ReadWrite,
        }
    };
    if write { !allows(address) } else { (address..address + bytes).any(|byte| !allows(byte)) }
}
