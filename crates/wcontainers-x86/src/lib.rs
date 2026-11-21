//! Everything that is only true of x86-64. The core, [`X86_64`]'s [`Personality`] and its register block
//! [`X86Block`], is `no_std`; with the `translate` feature (on by default) also the decoder (iced-x86), the
//! meaning of every instruction as WebAssembly and [`X86_64`]'s `Guest` implementation.

#![cfg_attr(not(feature = "translate"), no_std)]

use wcontainers_guest_cpu::Personality;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[cfg(feature = "translate")]
mod decode;
#[cfg(feature = "translate")]
mod descriptors;
#[cfg(feature = "translate")]
mod exact;
#[cfg(feature = "translate")]
mod flags;
#[cfg(feature = "translate")]
mod float;
#[cfg(feature = "translate")]
mod fpu;
#[cfg(feature = "translate")]
mod guest;
#[cfg(feature = "translate")]
mod habits;
#[cfg(feature = "translate")]
mod host;
#[cfg(feature = "translate")]
mod meaning;
#[cfg(feature = "translate")]
mod operand;
#[cfg(feature = "translate")]
mod root;
#[cfg(feature = "translate")]
mod segments;
#[cfg(feature = "translate")]
mod simd;
#[cfg(feature = "translate")]
mod text;
#[cfg(feature = "translate")]
mod transcendental;
#[cfg(feature = "translate")]
pub mod x87;

pub mod claims;
pub mod cpuid;
pub mod encoding;
pub mod offer;

pub use offer::{FULL_SPEED, NotBuilt, Offer, V3};

#[cfg(feature = "translate")]
pub use decode::{Instruction, claim, decode, told_feature};
#[cfg(feature = "translate")]
pub use descriptors::{
    CPU_NUMBER, KERNEL_CS, SELECTOR_INDEX, SELECTOR_PRIVILEGE, TASK_STATE, USER_CS, USER_DS,
    USER32_CS,
};
#[cfg(feature = "translate")]
pub use flags::{Condition, Flag, Flags, Shape, Test};
#[cfg(feature = "translate")]
pub use float::reciprocal;
#[cfg(feature = "translate")]
pub use guest::{Control, Reason, Trap};
#[cfg(feature = "translate")]
pub use host::Host;
#[cfg(feature = "translate")]
pub use meaning::written;
#[cfg(feature = "translate")]
pub use operand::{General, general as general_register, size_of_memory as memory_size_bytes};
#[cfg(feature = "translate")]
pub use operand::{R8, R9, R10, R11, R12, R13, R14, R15, RAX, RBP, RBX, RCX, RDI, RDX, RSI};

/// The x86-64 guest, 64-bit user mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct X86_64;

/// The saved registers of a thread: the general registers in hardware order (rax, rcx, rdx, rbx, rsp, rbp,
/// rsi, rdi, r8 to r15), rip, the six arithmetic flags at their rflags positions, the other rflags bits user
/// code keeps (TF, DF, NT and ID, at their positions), the FS and GS bases, the selectors of ds, es, fs and
/// gs, a 512-byte area in the FXSAVE layout (its x87 slots in physical order R0 to R7) and the upper halves
/// of the sixteen ymm registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct X86Block {
    pub gpr: [u64; 16],
    pub rip: u64,
    pub flags: u32,
    pub reserved0: u32,
    pub rflags: u64,
    pub fs_base: u64,
    pub gs_base: u64,
    pub selectors: [u16; SEGMENT_REGISTERS],
    pub fxsave: Fxsave,
    pub ymm_high: [[u8; 16]; 16],
}

/// The segment registers a program can load, in `X86Block::selectors`' order; cs and ss hold Linux's 64-bit
/// user segments always.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentRegister {
    Ds,
    Es,
    Fs,
    Gs,
}

/// `X86Block::selectors`' order.
pub const SEGMENT_REGISTER_ORDER: [SegmentRegister; SEGMENT_REGISTERS] = [
    SegmentRegister::Ds,
    SegmentRegister::Es,
    SegmentRegister::Fs,
    SegmentRegister::Gs,
];
pub const SEGMENT_REGISTERS: usize = SegmentRegister::Gs as usize + 1;

/// The FXSAVE area (Intel SDM Vol. 1 10.5.1, table 10-2), its x87 slots in physical order R0 to R7.
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct Fxsave {
    pub fcw: u16,
    pub fsw: u16,
    pub ftw: u8,
    pub reserved0: u8,
    pub fop: u16,
    pub fip: u64,
    pub fdp: u64,
    pub mxcsr: u32,
    pub mxcsr_mask: u32,
    pub st: [[u8; 16]; 8],
    pub xmm: [[u8; 16]; 16],
    pub available: [u8; 96],
}

impl Fxsave {
    pub const ZERO: Self = Fxsave {
        fcw: 0,
        fsw: 0,
        ftw: 0,
        reserved0: 0,
        fop: 0,
        fip: 0,
        fdp: 0,
        mxcsr: 0,
        mxcsr_mask: 0,
        st: [[0; 16]; 8],
        xmm: [[0; 16]; 16],
        available: [0; 96],
    };
}

/// The x87 control word after `fninit` and MXCSR at reset (Linux's `MXCSR_DEFAULT`), as Linux starts a
/// thread.
pub const FCW_AT_START: u16 = 0x37f;
pub const MXCSR_AT_START: u32 = 0x1f80;

/// MXCSR's fields (Intel SDM Vol. 1 10.2.3): the six sticky exception flags (invalid, denormal, divide by
/// zero, overflow, underflow, precision) in bits 0 to 5, denormals are zero, the six masks in the same
/// order from bit 7, the rounding control (nearest, down, up, toward zero) and flush to zero.
pub const MXCSR_INVALID: u32 = 1;
pub const MXCSR_DENORMAL: u32 = 1 << 1;
pub const MXCSR_DIVIDE_BY_ZERO: u32 = 1 << 2;
pub const MXCSR_OVERFLOW: u32 = 1 << 3;
pub const MXCSR_UNDERFLOW: u32 = 1 << 4;
pub const MXCSR_PRECISION: u32 = 1 << 5;
pub const MXCSR_FLAGS: u32 = (MXCSR_PRECISION << 1) - 1;
pub const MXCSR_DENORMALS_ARE_ZERO: u32 = 1 << 6;
pub const MXCSR_MASK_SHIFT: u32 = 7;
pub const MXCSR_ROUNDING_SHIFT: u32 = 13;
pub const MXCSR_ROUNDING: u32 = 3 << MXCSR_ROUNDING_SHIFT;
pub const MXCSR_FLUSH_TO_ZERO: u32 = 1 << 15;
/// The bits that set the mode, and the bits a program may set (`ldmxcsr` of any other is general
/// protection); the i7-8750H's MXCSR_MASK.
pub const MXCSR_MODE: u32 = MXCSR_FLUSH_TO_ZERO | (MXCSR_FLUSH_TO_ZERO - 1) & !MXCSR_FLAGS;
pub const MXCSR_WRITABLE: u32 = MXCSR_MODE | MXCSR_FLAGS;

/// MXCSR's exception flags that are raised and unmasked, from which Linux's `fpu__exception_code` works out
/// the `si_code` of an unmasked SIMD floating point exception.
pub const fn mxcsr_raised(mxcsr: u32) -> u32 {
    mxcsr & !(mxcsr >> MXCSR_MASK_SHIFT) & MXCSR_FLAGS
}

/// x86's words of the thread state after the block, which the kernel never sees.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, FromBytes, IntoBytes, Immutable, KnownLayout,
)]
#[repr(C)]
pub struct X86Private {
    /// Not zero once the program wrote MXCSR, so its exception flags are kept exact.
    pub mxcsr_tracking: u32,
    /// The `si_code` of the unmasked SIMD floating point exception the float hook last met, 0 for none.
    pub float_exception: u32,
    /// Single stepping by the trap flag: idle, one instruction about to run alone at `step_at`, or running.
    pub step: u32,
    /// Not zero when the instruction running alone loaded ss, which holds its trap back one instruction.
    pub step_held: u32,
    pub step_at: u64,
}
/// The bits of an x87 slot that hold its register.
pub const X87_REGISTER_BITS: u32 = 80;
/// The bytes of an xmm register.
pub const XMM_BYTES: u32 = size_of::<u128>() as u32;
/// The bit of a string compare's (`pcmpestri` and its kin) immediate that makes its elements words.
pub const STRING_COMPARE_WORDS: u8 = 1;

/// rsp's number in the general registers.
pub const RSP_NUMBER: u8 = 4;

/// Byte offsets in the block.
pub const RIP: usize = core::mem::offset_of!(X86Block, rip);
pub const FLAGS_WORD: usize = core::mem::offset_of!(X86Block, flags);
pub const RFLAGS: usize = core::mem::offset_of!(X86Block, rflags);
pub const FS_BASE: usize = core::mem::offset_of!(X86Block, fs_base);
pub const GS_BASE: usize = core::mem::offset_of!(X86Block, gs_base);
pub const SELECTORS: usize = core::mem::offset_of!(X86Block, selectors);
pub const FXSAVE: usize = core::mem::offset_of!(X86Block, fxsave);
/// MXCSR and the xmm registers in the FXSAVE area.
pub const MXCSR: usize = FXSAVE + core::mem::offset_of!(Fxsave, mxcsr);
pub const XMM: usize = FXSAVE + core::mem::offset_of!(Fxsave, xmm);
pub const YMM_HIGH: usize = core::mem::offset_of!(X86Block, ymm_high);
/// The i32 stop flag after the block.
pub const STOP_FLAG: usize = size_of::<X86Block>();
/// The private words, from the start of the block.
pub const MXCSR_TRACKING: usize =
    <X86_64 as Personality>::BLOCK_SIZE + core::mem::offset_of!(X86Private, mxcsr_tracking);
pub const FLOAT_EXCEPTION: usize =
    <X86_64 as Personality>::BLOCK_SIZE + core::mem::offset_of!(X86Private, float_exception);
pub const STEP: usize =
    <X86_64 as Personality>::BLOCK_SIZE + core::mem::offset_of!(X86Private, step);
pub const STEP_HELD: usize =
    <X86_64 as Personality>::BLOCK_SIZE + core::mem::offset_of!(X86Private, step_held);
pub const STEP_AT: usize =
    <X86_64 as Personality>::BLOCK_SIZE + core::mem::offset_of!(X86Private, step_at);

/// The rflags bits of the six arithmetic flags, and those of `X86Block::rflags`.
pub const CF: u64 = 1;
pub const PF: u64 = 1 << 2;
pub const AF: u64 = 1 << 4;
pub const ZF: u64 = 1 << 6;
pub const SF: u64 = 1 << 7;
pub const OF: u64 = 1 << 11;
pub const TF: u64 = 1 << 8;
pub const DF: u64 = 1 << 10;
pub const NT: u64 = 1 << 14;
/// Alignment checking, which the translator never holds.
pub const ALIGNMENT_CHECK: u64 = 1 << 18;
pub const ID: u64 = 1 << 21;
/// The six arithmetic flags.
pub const ARITHMETIC_FLAGS: u64 = CF | PF | AF | ZF | SF | OF;
/// iced's bitness of 64-bit mode.
pub const MODE_BITS: u32 = u64::BITS;
/// Linear address bits under 4-level paging (Intel SDM Vol. 3A 4.5). An address is canonical when the bits
/// above them copy its top one.
pub const VIRTUAL_ADDRESS_BITS: u32 = 48;
/// The first address above the lower canonical half, and the top of user space Linux gives a program, one
/// page below it (`TASK_SIZE_MAX`).
pub const USER_END: u64 = 1 << (VIRTUAL_ADDRESS_BITS - 1);
pub const USER_TOP: u64 = USER_END - wcontainers_abi::PAGE_SIZE;
/// How far an address is shifted left and back to copy its top canonical bit into the bits above.
pub const CANONICAL_SHIFT: u32 = u64::BITS - VIRTUAL_ADDRESS_BITS;

/// Whether `address` is canonical.
pub const fn is_canonical(address: u64) -> bool {
    ((address << CANONICAL_SHIFT) as i64 >> CANONICAL_SHIFT) as u64 == address
}

/// Interrupt vectors `int n` names that user code may reach: the breakpoint and overflow traps (Intel SDM
/// Vol. 3A 6.3.1), and Linux's gate for 32-bit system calls.
pub const BREAKPOINT_VECTOR: u8 = 3;
pub const OVERFLOW_VECTOR: u8 = 4;
pub const LINUX_32_BIT_SYSTEM_CALL: u8 = 0x80;

/// Bits user code always sees set in rflags: the reserved bit 1 and IF.
pub const ALWAYS: u64 = 0x202;

impl X86Block {
    pub const ZERO: Self = X86Block {
        gpr: [0; 16],
        rip: 0,
        flags: 0,
        reserved0: 0,
        rflags: 0,
        fs_base: 0,
        gs_base: 0,
        selectors: [0; SEGMENT_REGISTERS],
        fxsave: Fxsave::ZERO,
        ymm_high: [[0; 16]; 16],
    };

    /// rflags as user code sees it.
    pub fn user_rflags(&self) -> u64 {
        u64::from(self.flags) | self.rflags | ALWAYS
    }
}

impl Personality for X86_64 {
    type Block = X86Block;
    const PC_OFFSET: usize = RIP;
    const STOP_FLAG_OFFSET: usize = STOP_FLAG;
    const BLOCK_SIZE: usize =
        (STOP_FLAG + size_of::<i32>()).next_multiple_of(wcontainers_guest_cpu::BLOCK_ALIGNMENT);
    const PRIVATE_BYTES: usize = size_of::<X86Private>();

    fn initial_block(pc: u64, sp: u64) -> X86Block {
        let mut block = X86Block {
            rip: pc,
            ..X86Block::ZERO
        };
        block.gpr[usize::from(RSP_NUMBER)] = sp;
        block.fxsave.fcw = FCW_AT_START;
        block.fxsave.mxcsr = MXCSR_AT_START;
        block
    }
}
