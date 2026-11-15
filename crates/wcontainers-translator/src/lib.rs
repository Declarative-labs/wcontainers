//! The translator: a blob of ARM64 machine code in, WebAssembly out. It knows nothing about ELF, files or
//! Linux.

mod code;
pub mod contract;
mod full;
mod state;
mod thread;

pub use code::{Error, LookAhead, Omit, PASSED, Translation, translate, translate_omitting};
pub use thread::thread_module;
