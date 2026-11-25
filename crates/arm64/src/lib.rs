//! Everything that is only true of ARM64. The core, [`Arm64`]'s [`Personality`], is `no_std`; with the
//! `translate` feature (on by default) also the instruction type, the decoder wrapper, the meaning of every
//! instruction as WebAssembly, Arm's list of every instruction form, and [`Arm64`]'s `Guest` implementation.
//! Nothing here knows about look-ahead or layout.

#![cfg_attr(not(feature = "translate"), no_std)]

use core::mem::offset_of;
use wcontainers_abi::{RegisterBlock, THREAD_BLOCK_SIZE};
use wcontainers_guest_cpu::Personality;

#[cfg(feature = "translate")]
pub mod carry;
#[cfg(feature = "translate")]
mod decode;
#[cfg(feature = "translate")]
mod decode_forms;
#[cfg(feature = "translate")]
mod decode_simd;
#[cfg(feature = "translate")]
pub mod encode;
#[cfg(feature = "translate")]
pub mod exact;
#[cfg(feature = "translate")]
mod float;
#[cfg(feature = "translate")]
mod float_simd;
#[cfg(feature = "translate")]
pub mod forms;
#[cfg(feature = "translate")]
mod guest;
#[cfg(feature = "translate")]
pub mod habits;
#[cfg(feature = "translate")]
pub mod host;
#[cfg(feature = "translate")]
mod instruction;
#[cfg(feature = "translate")]
mod meaning;
#[cfg(feature = "translate")]
mod memory;
pub mod mirror;
#[cfg(feature = "translate")]
mod native;
#[cfg(feature = "translate")]
mod simd;
pub mod thread_state;
pub use thread_state::{FPCR_BITS, FPSR_BITS};
pub mod way;
pub use way::Way;
#[cfg(feature = "translate")]
mod vector;

#[cfg(feature = "translate")]
pub use decode::decode;
#[cfg(feature = "translate")]
pub use decode_forms::{MAIN_ID, ZERO_BLOCK_ID};
#[cfg(feature = "translate")]
pub use guest::Shape;
#[cfg(feature = "translate")]
pub use instruction::*;
#[cfg(feature = "translate")]
pub use meaning::{
    Access, Carry, Control, Emitter, Flag, Flags, Hook, Host, Target, Trap, condition_direct,
    condition_from_flags, emit, flag_value, untag,
};

/// ADRP's page, whatever the guest's page size: it clears the low 12 bits (Arm ARM C6.2, ADRP).
pub const ADRP_PAGE_SHIFT: u32 = 12;
pub const ADRP_PAGE_MASK: u64 = (1 << ADRP_PAGE_SHIFT) - 1;

/// Whether `code`, whole instructions from an aligned start, reads FPSR. A program that does has FPSR's flags
/// kept exact from its start (`FLOAT_TRACKING` set before it runs): none can be worked out afterwards.
#[cfg(feature = "translate")]
pub fn reads_float_status(code: &[u8]) -> bool {
    let (words, _) = code.as_chunks::<{ size_of::<u32>() }>();
    words.iter().any(|&word| {
        matches!(
            decode(u32::from_le_bytes(word)),
            Instruction::ReadSystem {
                register: SystemRegister::Fpsr,
                ..
            }
        )
    })
}

/// The value of `CTR_EL0` we present: 64 byte cache lines, a physically indexed instruction cache, and both
/// the clean to the point of unification (IDC clear) and the instruction cache invalidation (DIC clear)
/// required, so programs that write code tell us with `ic ivau`.
pub const CACHE_TYPE: u64 = 0x8444_c004;

/// The ARM64 guest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Arm64;

impl Personality for Arm64 {
    type Block = RegisterBlock;
    const PC_OFFSET: usize = offset_of!(RegisterBlock, pc);
    const STOP_FLAG_OFFSET: usize = wcontainers_abi::STOP_FLAG_OFFSET;
    const BLOCK_SIZE: usize = THREAD_BLOCK_SIZE;
    const PRIVATE_BYTES: usize = size_of::<PrivateState>();

    fn initial_block(pc: u64, sp: u64) -> RegisterBlock {
        RegisterBlock {
            pc,
            sp,
            ..RegisterBlock::ZERO
        }
    }
}

/// The bytes of every A64 instruction: one 32 bit word.
pub const INSTRUCTION_BYTES: usize = size_of::<u32>();

/// ARM64's words of the thread state after the block, which the kernel never sees.
#[repr(C)]
pub struct PrivateState {
    /// Not zero once FPSR's flags are kept exact: from the start for a program that reads FPSR
    /// (`reads_float_status`), else from its first write of FPSR.
    pub float_tracking: u32,
    /// The size of the access a load exclusive remembered, 0 for none.
    pub exclusive_bytes: u32,
    /// The address it remembered.
    pub exclusive_address: u64,
    /// The value it loaded, low then high.
    pub exclusive_value: [u64; 2],
}

pub const FLOAT_TRACKING: usize = THREAD_BLOCK_SIZE + offset_of!(PrivateState, float_tracking);
pub const EXCLUSIVE_BYTES: usize = THREAD_BLOCK_SIZE + offset_of!(PrivateState, exclusive_bytes);
pub const EXCLUSIVE_ADDRESS: usize =
    THREAD_BLOCK_SIZE + offset_of!(PrivateState, exclusive_address);
pub const EXCLUSIVE_VALUE: usize = THREAD_BLOCK_SIZE + offset_of!(PrivateState, exclusive_value);
