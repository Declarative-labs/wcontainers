//! Everything that is only true of ARM64. The core, [`Arm64`]'s [`Personality`], is `no_std`; with the
//! `translate` feature (on by default) also the instruction type, the decoder wrapper, the meaning of every
//! instruction as WebAssembly, Arm's list of every instruction form, and [`Arm64`]'s `Guest` implementation.
//! Nothing here knows about look-ahead or layout.

#![cfg_attr(not(feature = "translate"), no_std)]

use wcontainers_abi::{RegisterBlock, THREAD_BLOCK_SIZE};
use wcontainers_guest_cpu::Personality;

#[cfg(feature = "translate")]
mod decode;
#[cfg(feature = "translate")]
mod decode_forms;
#[cfg(feature = "translate")]
mod decode_simd;
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
#[cfg(feature = "translate")]
mod simd;
#[cfg(feature = "translate")]
mod vector;

#[cfg(feature = "translate")]
pub use decode::decode;
#[cfg(feature = "translate")]
pub use decode_forms::{CACHE_TYPE, MAIN_ID, ZERO_BLOCK_ID};
#[cfg(feature = "translate")]
pub use guest::Shape;
#[cfg(feature = "translate")]
pub use instruction::*;
#[cfg(feature = "translate")]
pub use meaning::{
    Access, Carry, Control, Emitter, Flag, Flags, Hook, Host, Target, Trap, condition_direct,
    condition_from_flags, emit, flag_value, untag,
};

/// The ARM64 guest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Arm64;

impl Personality for Arm64 {
    type Block = RegisterBlock;
    const PC_OFFSET: usize = core::mem::offset_of!(RegisterBlock, pc);
    const STOP_FLAG_OFFSET: usize = wcontainers_abi::STOP_FLAG_OFFSET;
    const BLOCK_SIZE: usize = THREAD_BLOCK_SIZE;
    const PRIVATE_BYTES: usize = 32;
}

// ARM64's words of the thread state after the block, which the kernel never sees.
/// i32: not zero once the program wrote FPSR, so its flags are kept exact.
pub const FLOAT_TRACKING: usize = THREAD_BLOCK_SIZE;
/// i32: the size of the access a load exclusive remembered, 0 for none.
pub const EXCLUSIVE_BYTES: usize = FLOAT_TRACKING + 4;
/// i64: the address it remembered.
pub const EXCLUSIVE_ADDRESS: usize = FLOAT_TRACKING + 8;
/// Two i64: the value it loaded.
pub const EXCLUSIVE_VALUE: usize = FLOAT_TRACKING + 16;
