//! Everything that is only true of x86-64. The core, [`X86_64`]'s [`Personality`] and its register block
//! [`X86Block`], is `no_std`; with the `translate` feature (on by default) also the decoder (iced-x86), the
//! meaning of every instruction as WebAssembly and [`X86_64`]'s `Guest` implementation.

#![cfg_attr(not(feature = "translate"), no_std)]

use wcontainers_guest_cpu::Personality;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[cfg(feature = "translate")]
mod decode;
#[cfg(feature = "translate")]
mod flags;
#[cfg(feature = "translate")]
mod guest;
#[cfg(feature = "translate")]
mod meaning;
#[cfg(feature = "translate")]
mod operand;
#[cfg(feature = "translate")]
mod simd;

pub mod claims;
pub mod encoding;

#[cfg(feature = "translate")]
pub use decode::{Instruction, claim_group, decode};
#[cfg(feature = "translate")]
pub use flags::{Flag, Flags};
#[cfg(feature = "translate")]
pub use guest::{Control, Offer, Reason, Trap};
#[cfg(feature = "translate")]
pub use meaning::written;
#[cfg(feature = "translate")]
pub use operand::{General, general as general_register, size_of_memory as memory_size_bytes};

/// The x86-64 guest, 64-bit user mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct X86_64;

/// The saved registers of a thread: the general registers in hardware order (rax, rcx, rdx, rbx, rsp, rbp,
/// rsi, rdi, r8 to r15), rip, the six arithmetic flags at their rflags positions, the other rflags bits user
/// code keeps (TF, DF, NT and ID, at their positions), the FS and GS bases, a 512-byte area in the FXSAVE
/// layout (its x87 slots in physical order R0 to R7) and the upper halves of the sixteen ymm registers.
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
    pub reserved1: u64,
    pub fxsave: [u8; 512],
    pub ymm_high: [[u8; 16]; 16],
}

/// Byte offsets in the block.
pub const RIP: usize = core::mem::offset_of!(X86Block, rip);
pub const FLAGS_WORD: usize = core::mem::offset_of!(X86Block, flags);
pub const RFLAGS: usize = core::mem::offset_of!(X86Block, rflags);
pub const FS_BASE: usize = core::mem::offset_of!(X86Block, fs_base);
pub const GS_BASE: usize = core::mem::offset_of!(X86Block, gs_base);
pub const FXSAVE: usize = core::mem::offset_of!(X86Block, fxsave);
/// MXCSR and the xmm registers in the FXSAVE area.
pub const MXCSR: usize = FXSAVE + 24;
pub const XMM: usize = FXSAVE + 160;
pub const YMM_HIGH: usize = core::mem::offset_of!(X86Block, ymm_high);
/// The i32 stop flag after the block.
pub const STOP_FLAG: usize = size_of::<X86Block>();

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
pub const ID: u64 = 1 << 21;
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
        reserved1: 0,
        fxsave: [0; 512],
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
    const BLOCK_SIZE: usize = (STOP_FLAG + 4).next_multiple_of(16);
    const PRIVATE_BYTES: usize = 0;

    fn initial_block(pc: u64, sp: u64) -> X86Block {
        let mut block = X86Block {
            rip: pc,
            ..X86Block::ZERO
        };
        block.gpr[4] = sp;
        // The x87 control word 0x37F and MXCSR 0x1F80, as Linux starts a thread.
        block.fxsave[..2].copy_from_slice(&0x37fu16.to_le_bytes());
        block.fxsave[24..28].copy_from_slice(&0x1f80u32.to_le_bytes());
        block
    }
}
