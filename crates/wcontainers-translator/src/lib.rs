//! The translator: a blob of ARM64 machine code in, WebAssembly out. It knows nothing about ELF, files or
//! Linux.

mod code;
pub mod contract;
mod full;
mod state;
mod thread;

pub use code::{BOUND, CodeModule, Error, LookAhead, PASSED, Translation, translate};
#[cfg(feature = "measure")]
pub use code::{Omit, translate_in_modules, translate_omitting};
pub use thread::thread_module;
