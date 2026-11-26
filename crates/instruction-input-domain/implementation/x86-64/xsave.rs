//! The inputs of XGETBV, the XSAVE forms, UMWAIT and TPAUSE: where the area lies against the data pages' boundary
//! and alignment, with AC; EDX:EAX's request; what the registers hold and which components XINUSE marks; for XRSTOR
//! the header and image it reads; XGETBV's ECX; and the wait's source register, its value and its deadline.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded, MemoryOperand, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::xsave::{
    AVX_COMPONENT, EXTENDED_COMPONENTS, HEADER_OFFSET, HIGH_16_ZMM_COMPONENT, LINUX_XCR0, MXCSR_RESET, OPMASK_COMPONENT,
    PKRU_COMPONENT, SSE_COMPONENT, SaveFormat, StateForm, StateOperation, X87_COMPONENT, ZMM_HIGH_256_COMPONENT,
    find_state_form, list_components, to_extent, to_tracking,
};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, Flags, Instruction, InstructionFetch, PKRU_DEFAULT, PagePermission, Processor, X87, X87_INITIAL, X86_64,
    ZERO_PROCESSOR,
};

use crate::integer::{CodeLayout, encode_at_code_address, lay_out};
use crate::{CODE_ADDRESS, FlagSet, LOWER_DATA_ADDRESS, OperandType, OperandValue, UPPER_DATA_ADDRESS};

/// RBX, which holds the area's address.
const AREA_BASE: usize = 3;
/// The general registers a wait's source may be: ECX, EBX and R13D, none of them EDX:EAX's deadline.
const WAIT_SOURCES: [usize; 3] = [1, 3, 13];
/// What a register holds above the 32 bits a form reads of it.
const ABOVE_LOW_HALF: u64 = 0xA5A5_A5A5_0000_0000;
/// The lowest address above the lower canonical half.
const NON_CANONICAL_ADDRESS: u64 = 1 << 63;
/// A canonical address in the kernel's half, aligned to 64.
const KERNEL_ADDRESS: u64 = 0xFFFF_8000_0000_1000;
/// An MXCSR other than its reset value: every exception unmasked but precision, rounding down.
const CHANGED_MXCSR: u32 = 0x3F00;

/// Where an area lies: its address, the permissions of the two data pages, and whether AC is set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AreaPlacement {
    pub address: u64,
    pub lower: PagePermission,
    pub upper: PagePermission,
    pub alignment_check: bool,
}

/// What the registers hold: every component in use with values of its own, or in its initial configuration, each
/// with what XINUSE says of it and an MXCSR.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RegisterContents {
    /// The components that hold their initial configuration.
    pub initial: u64,
    /// XINUSE.
    pub in_use: u64,
    pub mxcsr: u32,
}

/// What XRSTOR's area holds: the header's XSTATE_BV and XCOMP_BV, whether its reserved bytes are zero, and MXCSR.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RestoredImage {
    pub state_bitmap: u64,
    pub compaction: u64,
    pub reserved_set: bool,
    pub mxcsr: u32,
}

/// The operand types of a form of this family, in the order the build reads them.
pub fn list_operand_types(form: &StateForm) -> Vec<OperandType> {
    match form.operation {
        StateOperation::ReadControl => vec![OperandType::ControlIndex, OperandType::RegisterContents, OperandType::Flags(FlagSet::Unread)],
        StateOperation::Save { format, .. } => {
            let mut types = vec![
                OperandType::XsaveArea { read: false },
                OperandType::XsaveRequest,
                OperandType::RegisterContents,
                OperandType::OldStateBitmap,
            ];
            if format == SaveFormat::Optimized {
                types.push(OperandType::XrstorTracking);
            }
            types
        }
        StateOperation::Restore { .. } => vec![
            OperandType::XsaveArea { read: true },
            OperandType::XsaveRequest,
            OperandType::RestoredImage,
            OperandType::RestoredOver,
        ],
        StateOperation::Wait => vec![OperandType::WaitSource, OperandType::WaitDeadline, OperandType::Flags(FlagSet::Unread)],
    }
}

/// The area's placements: aligned at the lower page's start; aligned so that the upper page starts inside the
/// legacy region, at the header, at each extended component and past the last, with the upper page read-only or
/// unmapped; the lower page read-only or unmapped; off 64-byte alignment by 1, 2, 4, 8, 16 and 32 bytes, with AC
/// clear and set; not canonical; and in the kernel's half.
pub fn generate_areas(read: bool) -> Vec<OperandValue> {
    let writable = mapped(DataAccess::ReadWrite);
    let read_only = mapped(DataAccess::ReadOnly);
    let placement = |address, lower, upper, alignment_check| AreaPlacement { address, lower, upper, alignment_check };
    let mut all = vec![placement(LOWER_DATA_ADDRESS, writable, writable, false)];
    let mut boundaries = vec![64, HEADER_OFFSET, HEADER_OFFSET + 64];
    boundaries.extend(EXTENDED_COMPONENTS.iter().skip(1).map(|component| component.offset));
    boundaries.push(2752);
    let refused = if read { [PagePermission::Unmapped, PagePermission::Unmapped] } else { [read_only, PagePermission::Unmapped] };
    for boundary in boundaries {
        for upper in refused {
            all.push(placement(UPPER_DATA_ADDRESS - boundary, writable, upper, false));
        }
    }
    for lower in refused {
        all.push(placement(LOWER_DATA_ADDRESS, lower, writable, false));
    }
    for offset in [1, 2, 4, 8, 16, 32] {
        for alignment_check in [false, true] {
            all.push(placement(LOWER_DATA_ADDRESS + offset, writable, writable, alignment_check));
        }
    }
    all.push(placement(NON_CANONICAL_ADDRESS, PagePermission::Unmapped, PagePermission::Unmapped, false));
    all.push(placement(KERNEL_ADDRESS, PagePermission::Unmapped, PagePermission::Unmapped, false));
    all.into_iter().map(OperandValue::XsaveArea).collect()
}

const fn mapped(data_access: DataAccess) -> PagePermission {
    PagePermission::Mapped { data_access, instruction_fetch: InstructionFetch::Forbidden }
}

/// EDX:EAX's requests: everything, nothing, each component alone, x87 with SSE, AVX-512's three, and bits XCR0 does
/// not enable; each with bits above in RAX's and RDX's upper halves, which no form reads.
pub fn generate_requests() -> Vec<OperandValue> {
    let mut requests = vec![u64::MAX, 0, X87_COMPONENT | SSE_COMPONENT, OPMASK_COMPONENT | ZMM_HIGH_256_COMPONENT | HIGH_16_ZMM_COMPONENT];
    requests.extend(list_components(LINUX_XCR0));
    requests.push(1 << 3 | 1 << 4 | 1 << 8 | 1 << 17 | 1 << 18 | 1 << 63);
    requests.into_iter().map(OperandValue::XsaveRequest).collect()
}

/// The registers' contents: every component in use, with MXCSR at reset and changed; every component initial and
/// XINUSE clear, with each MXCSR; every component initial and XINUSE set; and each component alone initial and clear.
/// PKRU's bit stays set: this chip's XRSTOR, the one way the chip source loads XINUSE, leaves PKRU in use.
pub fn generate_contents() -> Vec<OperandValue> {
    let contents = |initial, in_use: u64, mxcsr| {
        OperandValue::RegisterContents(RegisterContents { initial, in_use: in_use | PKRU_COMPONENT, mxcsr })
    };
    let mut all = vec![
        contents(0, LINUX_XCR0, MXCSR_RESET),
        contents(0, LINUX_XCR0, CHANGED_MXCSR),
        contents(LINUX_XCR0, 0, MXCSR_RESET),
        contents(LINUX_XCR0, 0, CHANGED_MXCSR),
        contents(LINUX_XCR0, LINUX_XCR0, MXCSR_RESET),
    ];
    all.extend(list_components(LINUX_XCR0 & !PKRU_COMPONENT).map(|component| contents(component, LINUX_XCR0 & !component, MXCSR_RESET)));
    all.push(contents(PKRU_COMPONENT, LINUX_XCR0, MXCSR_RESET));
    all
}

/// The registers XRSTOR restores over: every component in use; every one initial, XINUSE clear but PKRU's; and every
/// one initial with XINUSE set. What they hold is replaced where the request reaches; XINUSE says what is kept.
pub fn generate_restored_over() -> Vec<OperandValue> {
    [
        RegisterContents { initial: 0, in_use: LINUX_XCR0, mxcsr: MXCSR_RESET },
        RegisterContents { initial: LINUX_XCR0, in_use: PKRU_COMPONENT, mxcsr: CHANGED_MXCSR },
        RegisterContents { initial: LINUX_XCR0, in_use: LINUX_XCR0, mxcsr: MXCSR_RESET },
    ]
    .map(OperandValue::RegisterContents)
    .to_vec()
}

/// The XSTATE_BV an area held before XSAVE or XSAVEOPT writes it: the data page's own bytes, which set some bits of
/// each component and clear others, and every bit.
pub fn generate_old_bitmaps() -> Vec<OperandValue> {
    [None, Some(u64::MAX)].map(OperandValue::OldStateBitmap).to_vec()
}

/// XRSTOR's images: the standard form loading every component, none, x87 alone and AVX-512's three; a bit XCR0 does
/// not enable; XCOMP_BV set without its compacted bit; a reserved byte set; MXCSR with a reserved bit and changed; the
/// compacted form of every component, of x87 alone over a layout of every one, with a component outside XCOMP_BV, and
/// with a reserved byte set.
pub fn generate_images() -> Vec<OperandValue> {
    let compacted = 1 << 63 | LINUX_XCR0;
    let image = |state_bitmap, compaction, reserved_set, mxcsr| RestoredImage { state_bitmap, compaction, reserved_set, mxcsr };
    [
        image(LINUX_XCR0, 0, false, MXCSR_RESET),
        image(0, 0, false, CHANGED_MXCSR),
        image(X87_COMPONENT, 0, false, CHANGED_MXCSR),
        image(OPMASK_COMPONENT | ZMM_HIGH_256_COMPONENT | HIGH_16_ZMM_COMPONENT, 0, false, MXCSR_RESET),
        image(LINUX_XCR0 | 1 << 3, 0, false, MXCSR_RESET),
        image(LINUX_XCR0, LINUX_XCR0, false, MXCSR_RESET),
        image(LINUX_XCR0, 0, true, MXCSR_RESET),
        image(LINUX_XCR0, 0, false, 0x1_0000 | MXCSR_RESET),
        image(LINUX_XCR0, compacted, false, CHANGED_MXCSR),
        image(X87_COMPONENT, compacted, false, CHANGED_MXCSR),
        image(LINUX_XCR0, 1 << 63 | X87_COMPONENT, false, MXCSR_RESET),
        image(LINUX_XCR0, compacted, true, MXCSR_RESET),
    ]
    .map(OperandValue::RestoredImage)
    .to_vec()
}

/// What the modified optimization knows before XSAVEOPT: every component in use unmodified since an XRSTOR from the
/// area at the lower page's start; none; x87, SSE and AVX alone unmodified; and every one, of an area 64 bytes above.
pub fn generate_trackings() -> Vec<OperandValue> {
    [
        Some((LOWER_DATA_ADDRESS, u64::MAX)),
        None,
        Some((LOWER_DATA_ADDRESS, X87_COMPONENT | SSE_COMPONENT | AVX_COMPONENT)),
        Some((LOWER_DATA_ADDRESS + 64, u64::MAX)),
    ]
    .map(OperandValue::XrstorTracking)
    .to_vec()
}

/// XGETBV's ECX: XCR0, XINUSE, the first index past them and the last, and 0 and 1 below bits RCX's upper half holds.
pub fn generate_control_indices() -> Vec<OperandValue> {
    [0, 1, 2, u64::from(u32::MAX), ABOVE_LOW_HALF, ABOVE_LOW_HALF | 1].map(OperandValue::Bits).to_vec()
}

/// The wait's source register and value: each register with 0, 1 (C0.1), 2 and bit 31 (reserved bits), and 0 and 1
/// below bits its upper half holds.
pub fn generate_wait_sources() -> Vec<OperandValue> {
    let values = [0, 1, 2, 1 << 31, ABOVE_LOW_HALF, ABOVE_LOW_HALF | 1];
    WAIT_SOURCES
        .iter()
        .flat_map(|&register| values.map(|value| OperandValue::WaitSource { register, value }))
        .collect()
}

/// EDX:EAX's deadline: passed, and the last, which the operating system's limit cuts short.
pub fn generate_deadlines() -> Vec<OperandValue> {
    [0, u64::MAX].map(OperandValue::Bits).to_vec()
}

/// A processor every component of which holds values of its own, Linux's PKRU included, XINUSE all set.
fn fill_processor() -> Processor {
    Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        x87: X87 {
            registers: array::from_fn(|physical| 0x4000_8000_0000_0000_0000 | (physical as u128) << 64 | (0x1234_5678 * (physical as u128 + 1))),
            control: 0x027F,
            status: 5 << 11 | 1 << 8,
            tags: 0b1110_0101,
            opcode: 0x07DB,
            instruction_pointer: 0x0000_1234_5678_9ABC,
            data_pointer: 0x0000_7654_3210_FEDC,
        },
        zmm_upper_halves: array::from_fn(|number| [to_part(number, 2), to_part(number, 3)]),
        high_vector_registers: array::from_fn(|number| array::from_fn(|part| to_part(16 + number, part))),
        opmask_registers: array::from_fn(|number| 0x5A5A_C3C3_0F0F_9696 ^ (number as u64).wrapping_mul(0x0101_0101_0101_0101)),
        pkru: PKRU_DEFAULT,
        xinuse: LINUX_XCR0,
        ..ZERO_PROCESSOR
    }
}

/// A value distinct for each register and 128-bit part, with no zero byte.
const fn to_part(number: usize, part: usize) -> u128 {
    0x3C3C_3C3C_3C3C_3C3C_3C3C_3C3C_3C3C_3C3C ^ ((number as u128) << 8 | part as u128 | 0x10_1010_1010)
}

/// Puts each component of `initial` in its initial configuration.
fn initialize(processor: &mut Processor, initial: u64) {
    for component in list_components(initial) {
        match component {
            X87_COMPONENT => processor.x87 = X87_INITIAL,
            SSE_COMPONENT => processor.vector_registers = [0; 16],
            AVX_COMPONENT => processor.upper_vector_halves = [0; 16],
            OPMASK_COMPONENT => processor.opmask_registers = [0; 8],
            ZMM_HIGH_256_COMPONENT => processor.zmm_upper_halves = [[0; 2]; 16],
            HIGH_16_ZMM_COMPONENT => processor.high_vector_registers = [[0; 4]; 16],
            PKRU_COMPONENT => processor.pkru = 0,
            _ => {}
        }
    }
}

/// Builds the state of a form of this family from one value of each of its operand types.
pub fn build_state(form: &StateForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = fill_processor();
    let mut memory_mutations = Vec::new();
    let (mut area, mut old_bitmap, mut image, mut request, mut deadline) = (None, None, None, None, 0);
    let mut tracking = None;
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::XsaveArea { .. }, OperandValue::XsaveArea(chosen)) => area = Some(chosen),
            (OperandType::XsaveRequest, OperandValue::XsaveRequest(chosen)) => request = Some(chosen),
            (OperandType::RegisterContents | OperandType::RestoredOver, OperandValue::RegisterContents(contents)) => {
                initialize(&mut processor, contents.initial);
                processor.xinuse = contents.in_use;
                processor.mxcsr = contents.mxcsr;
            }
            (OperandType::OldStateBitmap, OperandValue::OldStateBitmap(chosen)) => old_bitmap = chosen,
            (OperandType::RestoredImage, OperandValue::RestoredImage(chosen)) => image = Some(chosen),
            (OperandType::ControlIndex, OperandValue::Bits(index)) => processor.general_purpose_registers[1] = index,
            (OperandType::WaitSource, OperandValue::WaitSource { register, value }) => {
                processor.general_purpose_registers[register] = value;
            }
            (OperandType::WaitDeadline, OperandValue::Bits(chosen)) => deadline = chosen,
            (OperandType::XrstorTracking, OperandValue::XrstorTracking(chosen)) => tracking = chosen,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    // The source loads a tracking only under Linux's PKRU, which it keeps the host's.
    if let Some((area, unmodified)) = tracking.filter(|_| processor.pkru == PKRU_DEFAULT) {
        processor.xsave_tracking = to_tracking(area, unmodified & LINUX_XCR0, processor.xinuse);
    }
    if let Some(request) = request {
        processor.general_purpose_registers[0] = ABOVE_LOW_HALF | (request & u64::from(u32::MAX));
        processor.general_purpose_registers[2] = ABOVE_LOW_HALF | request >> 32;
    }
    let encoded = match form.operation {
        StateOperation::Wait => {
            let source = operands.iter().find_map(|(_, value)| match value {
                OperandValue::WaitSource { register, .. } => Some(*register),
                _ => None,
            });
            processor.general_purpose_registers[0] = deadline & u64::from(u32::MAX) | ABOVE_LOW_HALF;
            processor.general_purpose_registers[2] = deadline >> 32 | ABOVE_LOW_HALF;
            Encoded::with1(form.code, GENERAL_32[source.expect("a source register")])
        }
        StateOperation::ReadControl => Ok(Encoded::with(Code::Xgetbv)),
        StateOperation::Save { .. } | StateOperation::Restore { .. } => {
            let placement = area.expect("an area");
            processor.general_purpose_registers[AREA_BASE] = placement.address;
            if placement.alignment_check {
                processor.flags |= Flags::ALIGNMENT_CHECK;
            }
            memory_mutations.extend(place_area(placement));
            if let Some(bitmap) = old_bitmap {
                push_bytes(&mut memory_mutations, placement.address.wrapping_add(HEADER_OFFSET), &bitmap.to_le_bytes());
            }
            if let Some(image) = image {
                write_image(&mut memory_mutations, placement.address, image);
            }
            Encoded::with1(form.code, MemoryOperand::with_base(Register::RBX))
        }
    };
    let encoded = encoded.unwrap_or_else(|error| panic!("{:?} takes its operands: {error}", form.code));
    let (code_page, start, bytes) = lay_out(CodeLayout::PageStart, &encode_at_code_address(&encoded), 0x3E);
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}

const GENERAL_32: [Register; 16] = [
    Register::EAX, Register::ECX, Register::EDX, Register::EBX, Register::ESP, Register::EBP, Register::ESI, Register::EDI,
    Register::R8D, Register::R9D, Register::R10D, Register::R11D, Register::R12D, Register::R13D, Register::R14D, Register::R15D,
];

/// The data pages' bytes and the placement's permissions; an area outside the lower half gives none.
fn place_area(placement: AreaPlacement) -> Vec<MemoryDelta<'static, X86_64>> {
    let mut mutations = Vec::new();
    if placement.address >= UPPER_DATA_ADDRESS + PAGE_BYTES {
        return mutations;
    }
    let bytes: Vec<u8> = (LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES).map(crate::to_data_byte).collect();
    mutations.push(MemoryDelta::ValueAssignment { start: LOWER_DATA_ADDRESS, bytes: Cow::Owned(bytes) });
    mutations.push(MemoryDelta::PermissionAssignment { addresses: LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS, permission: placement.lower });
    mutations.push(MemoryDelta::PermissionAssignment {
        addresses: UPPER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES,
        permission: placement.upper,
    });
    mutations
}

/// Writes bytes inside the program's addresses, dropping those past them.
fn push_bytes(mutations: &mut Vec<MemoryDelta<'static, X86_64>>, start: u64, bytes: &[u8]) {
    let held: Vec<u8> = bytes
        .iter()
        .enumerate()
        .take_while(|(offset, _)| X86_64_ADDRESSES.contains(&start.wrapping_add(*offset as u64)))
        .map(|(_, byte)| *byte)
        .collect();
    if !held.is_empty() {
        mutations.push(MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(held) });
    }
}

const X86_64_ADDRESSES: core::ops::Range<u64> = 0..1 << 47;

/// XRSTOR's image: a legacy region another processor's values fill, with the image's MXCSR, the header, and each
/// extended component's values; the bytes the image does not name keep the data pages'.
fn write_image(mutations: &mut Vec<MemoryDelta<'static, X86_64>>, address: u64, image: RestoredImage) {
    let source = to_restored_processor();
    let mut legacy = wcontainers_processor_architecture_x86_64::x87_state::write_extended(
        &source.x87,
        image.mxcsr,
        0xFFFF,
        &source.vector_registers,
        true,
    );
    legacy[24..28].copy_from_slice(&image.mxcsr.to_le_bytes());
    push_bytes(mutations, address, &legacy);
    let mut header = [0u8; 64];
    header[..8].copy_from_slice(&image.state_bitmap.to_le_bytes());
    header[8..16].copy_from_slice(&image.compaction.to_le_bytes());
    if image.reserved_set {
        header[40] = 0x01;
    }
    push_bytes(mutations, address.wrapping_add(HEADER_OFFSET), &header);
    let compacted = image.compaction & 1 << 63 != 0;
    let mut next = HEADER_OFFSET + 64;
    for component in EXTENDED_COMPONENTS {
        if compacted && image.compaction & component.bit == 0 {
            continue;
        }
        let offset = if compacted { next } else { component.offset };
        next = offset + component.bytes;
        let mut bytes: Vec<u8> = (0..component.bytes).map(|index| (index as u8).wrapping_mul(7) ^ (component.bit as u8 | 0x40)).collect();
        // PKRU loads as Linux's, which the source keeps the host's while it reads the tracking back.
        if component.bit == PKRU_COMPONENT {
            bytes[..4].copy_from_slice(&PKRU_DEFAULT.to_le_bytes());
        }
        push_bytes(mutations, address.wrapping_add(offset), &bytes);
    }
}

/// The processor whose x87 and SSE values an XRSTOR image holds: the filled one's, each changed.
fn to_restored_processor() -> Processor {
    let mut processor = fill_processor();
    processor.vector_registers = array::from_fn(|number| (number as u128 + 1).wrapping_mul(0x0123_4567_89AB_CDEF_0F1E_2D3C_4B5A_6978));
    processor.x87.control = 0x0F7F;
    processor.x87.status = 2 << 11;
    processor.x87.tags = 0b0011_1100;
    processor
}

/// Whether the values decide the state's outcome whatever the data: an area that is misaligned, not canonical or in
/// the kernel's half faults before any byte is read or written; a save whose request reaches a page that refuses the
/// write faults whatever it would write; a restore whose XCOMP_BV lies on a page that refuses the read faults before
/// it reads anything else.
pub fn decides_without_data(instruction: Instruction, operands: &[Option<OperandValue>]) -> bool {
    let Some(form) = find_state_form(instruction) else {
        return false;
    };
    let mut area = None;
    let mut request = None;
    for operand in operands.iter().flatten() {
        match *operand {
            OperandValue::XsaveArea(chosen) => area = Some(chosen),
            OperandValue::XsaveRequest(chosen) => request = Some(chosen),
            _ => {}
        }
    }
    let Some(area) = area else {
        return false;
    };
    if !area.address.is_multiple_of(64) || area.address >= X86_64_ADDRESSES.end {
        return true;
    }
    let refuses = |permission: PagePermission, write: bool| match permission {
        PagePermission::Unmapped => true,
        PagePermission::Mapped { data_access, .. } => write && data_access == DataAccess::ReadOnly,
    };
    let (extent, write) = match form.operation {
        StateOperation::Save { format, .. } => {
            let requested = LINUX_XCR0 & request.unwrap_or(0);
            (to_extent(requested, format != SaveFormat::Compacted), true)
        }
        // XRSTOR reads XCOMP_BV first, whatever the image holds.
        _ => (HEADER_OFFSET + 16, false),
    };
    let last = area.address + extent - 1;
    refuses(area.lower, write) || (last >= UPPER_DATA_ADDRESS && refuses(area.upper, write))
}
