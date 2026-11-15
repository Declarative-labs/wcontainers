//! Everything that is only true of ARM64: the instruction type, the decoder wrapper, the meaning of every
//! instruction as WebAssembly, and Arm's list of every instruction form. Nothing here knows about look-ahead or layout.

mod decode;
mod effects;
pub mod forms;
pub mod habits;
mod instruction;
mod meaning;
mod vector;

pub use decode::decode;
pub use effects::{Effects, Registers};
pub use instruction::*;
pub use meaning::{
    Access, Carry, Emitter, Flag, Flags, Target, Trap, condition_direct, condition_from_flags,
    emit, flag_value,
};
