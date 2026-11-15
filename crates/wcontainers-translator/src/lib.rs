//! The translator: a blob of ARM64 machine code in, WebAssembly out. It knows nothing about ELF, files or
//! Linux.

mod code;
pub mod contract;
mod thread;

pub use code::{Error, LookAhead, PASSED, Translation, translate};
pub use thread::thread_module;
