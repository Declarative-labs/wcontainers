//! Form I: guest code runs in a process of its own beside the kernel, each guest memory a window of one pool in that
//! process, every access of translated code checked and cut to its window (docs/native-without-virtual-machine.md
//! section 5).

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod execution;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod memory;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod run;
