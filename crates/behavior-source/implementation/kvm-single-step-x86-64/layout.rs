//! Where everything lies in the guest, and what the handler and the host exchange there.
//!
//! The kernel's pages lie at the bottom of guest-physical memory and are mapped, supervisor only, in the last 2 MiB
//! of the address space, so that the handler reaches every one of them by a sign-extended 32-bit address. A program's
//! pages lie above them and are mapped in the lower half, where a state's addresses are.

use std::ops::Range;

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

pub const PAGE_BYTES: u64 = 1 << 12;
pub const MEMORY_BYTES: u64 = 64 << 20;
/// The kernel's virtual address of guest-physical 0.
pub const KERNEL_BASE: u64 = 0xFFFF_FFFF_FFE0_0000;

// Guest-physical pages of the kernel.
pub const GLOBAL_DESCRIPTOR_TABLE: u64 = 0x1000;
pub const TASK_STATE_SEGMENT: u64 = GLOBAL_DESCRIPTOR_TABLE + 0x800;
pub const INTERRUPT_DESCRIPTOR_TABLE: u64 = 0x2000;
pub const HANDLER: u64 = 0x3000;
pub const HANDLER_BYTES: u64 = 2 * PAGE_BYTES;
pub const SLOT: u64 = 0x5000;
/// The stack an exception from ring 3 switches to, through the task state segment's RSP0.
pub const STACK_TOP: u64 = 0x8000;
/// The stack of the double fault, NMI and machine check, through IST1.
pub const FAULT_STACK_TOP: u64 = 0x9000;
pub const KERNEL_DIRECTORY_POINTERS: u64 = 0x9000;
pub const KERNEL_DIRECTORY: u64 = 0xA000;
pub const KERNEL_TABLE: u64 = 0xB000;
/// The directory pointers of every top-level entry of the upper half but the kernel's: 1 GiB supervisor pages.
pub const UPPER_HALF_DIRECTORY_POINTERS: u64 = 0xC000;
pub const KERNEL_PAGES: u64 = 0xD;
/// What full loading takes a state's tables and frames from.
pub const ARENA: Range<u64> = 0x20_0000..0x200_0000;
/// INCREMENTAL MEMORY: the top-level table, the lower tables and the frames the guest keeps between states.
pub const RESIDENT: Range<u64> = 0x200_0000..MEMORY_BYTES;

// Selectors, as Linux numbers them.
pub const KERNEL_CODE: u16 = 0x10;
pub const KERNEL_DATA: u16 = 0x18;
pub const USER_DATA: u16 = 0x2B;
pub const USER_CODE: u16 = 0x33;
pub const TASK_STATE: u16 = 0x40;
pub const DESCRIPTOR_ENTRIES: u64 = 10;

// Page table entry bits.
pub const PRESENT: u64 = 1 << 0;
pub const WRITABLE: u64 = 1 << 1;
pub const USER: u64 = 1 << 2;
pub const ACCESSED: u64 = 1 << 5;
pub const DIRTY: u64 = 1 << 6;
/// A 1 GiB or 2 MiB page, in a directory pointer or directory entry.
pub const LARGE_PAGE: u64 = 1 << 7;
pub const GLOBAL: u64 = 1 << 8;
pub const NO_EXECUTE: u64 = 1 << 63;
pub const ADDRESS_MASK: u64 = 0x000F_FFFF_FFFF_F000;
pub const ENTRY_BYTES: u64 = 8;
pub const ENTRIES: u64 = 512;
pub const LEVELS: u32 = 4;
/// The top-level entry of the kernel's half.
pub const KERNEL_ENTRY: u64 = ENTRIES - 1;

/// The `vector` of a slot no exception has ended yet.
pub const NOT_RUN: u64 = u64::MAX;

/// What a program at ring 3 holds besides its x87 and SSE state, as the handler loads and saves it.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Registers {
    /// In the order of their encodings.
    pub general: [u64; 16],
    pub instruction_pointer: u64,
    pub flags: u64,
    pub code_segment: u64,
    pub stack_segment: u64,
    pub fs_base: u64,
    pub gs_base: u64,
}

/// The image XSAVE64 writes and XRSTOR64 reads in the standard form, of the components [`SAVED_COMPONENTS`] names.
#[repr(C, align(64))]
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct XsaveImage {
    pub bytes: [u8; XSAVE_IMAGE_BYTES],
}

/// One state in guest memory, as the handler reads and writes it.
#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Slot {
    pub before_xsave: XsaveImage,
    pub after_xsave: XsaveImage,
    /// CR3 while the instruction runs.
    pub page_table: u64,
    /// The vector the handler was entered by, [`NOT_RUN`] before.
    pub vector: u64,
    /// The error code the chip pushed, zero for a vector that has none.
    pub error_code: u64,
    /// CR2 after.
    pub fault_address: u64,
    /// DR6 after.
    pub debug_status: u64,
    pub before: Registers,
    pub after: Registers,
    /// Fills the slot to a multiple of its alignment.
    pub reserved: [u64; 7],
}

// Where the image keeps each part: the legacy area as FXSAVE64 lays it out, then the header, then the AVX area of the
// standard form, https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html (volume 1, 13.4).
pub const FX_CONTROL_WORD: usize = 0;
pub const FX_STATUS_WORD: usize = 2;
/// The abridged tag word, one bit a register.
pub const FX_TAG_WORD: usize = 4;
pub const FX_OPCODE: usize = 6;
pub const FX_INSTRUCTION_POINTER: usize = 8;
pub const FX_DATA_POINTER: usize = 16;
pub const FX_MXCSR: usize = 24;
pub const FX_X87_REGISTERS: usize = 32;
pub const FX_VECTOR_REGISTERS: usize = 160;
/// XSTATE_BV: which components the image holds; the others are in their initial state.
pub const XSAVE_HEADER: usize = 512;
/// The upper halves of YMM0 to YMM15, at the offset CPUID leaf 0xD, subleaf 2 gives in EBX.
pub const XSAVE_AVX: usize = 576;
/// The legacy area, the header and the AVX area, rounded up to the image's alignment.
pub const XSAVE_IMAGE_BYTES: usize = 832;
/// XCR0's bits: x87, SSE, AVX, then the opmask, ZMM_Hi256 and Hi16_ZMM components of AVX-512, then PKRU.
pub const X87_COMPONENT: u64 = 1 << 0;
pub const SSE_COMPONENT: u64 = 1 << 1;
pub const AVX_COMPONENT: u64 = 1 << 2;
pub const AVX_512_COMPONENTS: u64 = 0b111 << 5;
pub const PKRU_COMPONENT: u64 = 1 << 9;
/// XCR0 as the source sets it: Linux's on this chip but for AMX's tile components, which KVM gives a guest only after
/// a system call kvm-ioctls does not make.
pub const EXTENDED_CONTROL_0: u64 = X87_COMPONENT | SSE_COMPONENT | AVX_COMPONENT | AVX_512_COMPONENTS | PKRU_COMPONENT;
/// The components the handler loads and saves: those the state holds, and x87's, which it checks is left alone.
pub const SAVED_COMPONENTS: u64 = X87_COMPONENT | SSE_COMPONENT | AVX_COMPONENT;

const _: () = assert!(size_of::<Slot>() as u64 <= PAGE_BYTES);

/// The kernel's virtual address of a guest-physical address of the kernel.
pub const fn to_kernel(physical: u64) -> u64 {
    KERNEL_BASE + physical
}

/// The index at a level, 0 the top, of a virtual address under 4-level paging.
pub const fn to_index(address: u64, level: u32) -> u64 {
    (address >> (PAGE_BYTES.trailing_zeros() + (LEVELS - 1 - level) * 9)) & (ENTRIES - 1)
}

/// A code or data segment descriptor of base 0 and the largest limit.
const fn to_segment(access: u64, flags: u64) -> u64 {
    0xFFFF | (access << 40) | (0xF << 48) | (flags << 52)
}

/// The global descriptor table's entries: null, Linux's kernel and user segments, then the task state segment.
pub fn list_descriptors() -> [u64; DESCRIPTOR_ENTRIES as usize] {
    const LONG: u64 = 0b1010;
    const BIG: u64 = 0b1100;
    let tss = to_kernel(TASK_STATE_SEGMENT);
    let limit = size_of::<TaskStateSegment>() as u64 - 1;
    let tss_low = limit | ((tss & 0xFF_FFFF) << 16) | (0x89 << 40) | (((tss >> 24) & 0xFF) << 56);
    [
        0,
        to_segment(0x9B, BIG),
        to_segment(0x9B, LONG),
        to_segment(0x93, BIG),
        to_segment(0xFB, BIG),
        to_segment(0xF3, BIG),
        to_segment(0xFB, LONG),
        0,
        tss_low,
        tss >> 32,
    ]
}

/// A 64-bit task state segment: the stacks an exception switches to.
#[repr(C, packed)]
#[derive(Clone, Copy, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct TaskStateSegment {
    pub reserved_0: u32,
    pub privilege_stacks: [u64; 3],
    pub reserved_1: u64,
    pub interrupt_stacks: [u64; 7],
    pub reserved_2: u64,
    pub reserved_3: u16,
    pub io_map_base: u16,
}

/// A 64-bit interrupt gate to `handler` in the kernel's code segment.
pub fn to_gate(handler: u64, privilege: u64, interrupt_stack: u64) -> [u64; 2] {
    let low = (handler & 0xFFFF)
        | (u64::from(KERNEL_CODE) << 16)
        | (interrupt_stack << 32)
        | ((0x8E | (privilege << 5)) << 40)
        | (((handler >> 16) & 0xFFFF) << 48);
    [low, handler >> 32]
}

/// Whether the chip pushes an error code for the vector.
pub const fn pushes_error_code(vector: u64) -> bool {
    matches!(vector, 8 | 10..=14 | 17 | 21 | 29 | 30)
}

/// The gates ring 3 may use with INT n, as Linux has them: INT3, INTO and 0x80.
pub const fn to_gate_privilege(vector: u64) -> u64 {
    match vector {
        3 | 4 | 0x80 => 3,
        _ => 0,
    }
}

/// The double fault, NMI and machine check run on their own stack.
pub const fn to_interrupt_stack(vector: u64) -> u64 {
    match vector {
        2 | 8 | 18 => 1,
        _ => 0,
    }
}
