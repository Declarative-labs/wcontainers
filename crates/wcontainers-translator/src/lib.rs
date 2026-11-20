//! The translator: a blob of guest machine code in, WebAssembly out. It knows nothing about ELF, files or
//! Linux, and nothing about any one guest CPU: everything it needs of one comes through
//! [`wcontainers_guest_cpu::Guest`], and each public function takes the guest as a type parameter, resolved
//! at compile time.

mod code;
pub mod contract;
mod full;
mod layout;
mod state;
mod thread;

pub use code::{
    BOUND, CodeModule, Error, FUNCTION_BLOCKS, LookAhead, Means, Optimise, Sites, Translation,
    translate,
};
#[cfg(feature = "measure")]
pub use code::{Omit, translate_in_modules, translate_omitting};
pub use thread::thread_module;
