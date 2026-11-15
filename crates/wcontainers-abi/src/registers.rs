//! The saved register block: every register of one guest thread, in a fixed layout.

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// Every register of one guest thread.
///
/// The first 272 bytes match Linux's `user_pt_regs` and the next 528 match `user_fpsimd_state`, so signal
/// frames and core dumps can be filled by copying. All fields are little endian and there is no padding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct RegisterBlock {
    /// x0 to x30.
    pub x: [u64; 31],
    pub sp: u64,
    pub pc: u64,
    /// Only the four flag bits are kept; see [`pstate`].
    pub pstate: u64,
    /// v0 to v31, each as its low half then its high half.
    pub v: [[u64; 2]; 32],
    pub fpsr: u32,
    pub fpcr: u32,
    /// The thread pointer, `TPIDR_EL0`.
    pub tpidr_el0: u64,
}

impl RegisterBlock {
    pub const ZERO: Self = Self {
        x: [0; 31],
        sp: 0,
        pc: 0,
        pstate: 0,
        v: [[0; 2]; 32],
        fpsr: 0,
        fpcr: 0,
        tpidr_el0: 0,
    };
}

impl Default for RegisterBlock {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Where the flag bits sit in [`RegisterBlock::pstate`], as in Linux's saved `pstate`.
pub mod pstate {
    pub const N: u64 = 1 << 31;
    pub const Z: u64 = 1 << 30;
    pub const C: u64 = 1 << 29;
    pub const V: u64 = 1 << 28;
    pub const NZCV: u64 = N | Z | C | V;
}

/// Where a thread's block is kept in memory by the CPU side, one 32 bit word follows the registers: the
/// stop flag. Zero means carry on. Anything else means "call the stop hook at the next check". It is the
/// only part of the block that another host thread writes while the guest thread runs.
pub const STOP_FLAG_OFFSET: usize = size_of::<RegisterBlock>();

/// Size of a thread's block in memory: the registers, the stop flag, and padding to a multiple of 16.
pub const THREAD_BLOCK_SIZE: usize = 816;

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::offset_of;

    #[test]
    fn layout_is_fixed() {
        assert_eq!(offset_of!(RegisterBlock, x), 0);
        assert_eq!(offset_of!(RegisterBlock, sp), 248);
        assert_eq!(offset_of!(RegisterBlock, pc), 256);
        assert_eq!(offset_of!(RegisterBlock, pstate), 264);
        assert_eq!(offset_of!(RegisterBlock, v), 272);
        assert_eq!(offset_of!(RegisterBlock, fpsr), 784);
        assert_eq!(offset_of!(RegisterBlock, fpcr), 788);
        assert_eq!(offset_of!(RegisterBlock, tpidr_el0), 792);
        assert_eq!(size_of::<RegisterBlock>(), 800);
        assert_eq!(STOP_FLAG_OFFSET, 800);
    }

    // The stop flag fits inside the block.
    const _: () = assert!(STOP_FLAG_OFFSET + 4 <= THREAD_BLOCK_SIZE);
}
