//! Everything that is only true of ARM64: the instruction type, the decoder wrapper, the meaning of every
//! instruction as WebAssembly, and Arm's list of every instruction form. Nothing here knows about look-ahead or layout.

mod decode;
mod decode_forms;
mod decode_simd;
mod effects;
pub mod exact;
mod float;
mod float_simd;
pub mod forms;
pub mod habits;
pub mod host;
mod instruction;
mod meaning;
mod memory;
mod simd;
mod vector;

pub use decode::decode;
pub use decode_forms::{CACHE_TYPE, MAIN_ID, ZERO_BLOCK_ID};
pub use effects::{Effects, Registers};
pub use instruction::*;
pub use meaning::{
    Access, Carry, Control, Emitter, Flag, Flags, Hook, Host, Target, Trap, condition_direct,
    condition_from_flags, emit, flag_value, untag,
};
