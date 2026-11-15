//! Everything that is only true of ARM64: the instruction type, the decoder wrapper and the meaning of every
//! instruction as WebAssembly. Nothing here knows about look-ahead or layout.

mod decode;
mod instruction;
mod meaning;

pub use decode::decode;
pub use instruction::*;
pub use meaning::{
    Access, Carry, Emitter, Flag, Flags, Target, Trap, condition_from_flags, emit, flag_value,
};
