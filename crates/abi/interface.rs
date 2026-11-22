//! What the translator and the kernel must agree on.
//!
//! Two sides meet here. The **CPU side** runs the guest's instructions (the translator's output, or a
//! stand-in emulator in tests). The **kernel** answers the guest's system calls. Neither depends on the
//! other; both depend on this crate.
//!
//! The kernel never waits on its own stack, so the boundary is a conversation driven from outside:
//!
//! 1. The driver asks the kernel what to do next and gets a [`Next`].
//! 2. For [`Next::Run`] it runs that thread on the CPU side until the thread comes back with an [`Exit`].
//! 3. It hands the [`Exit`] to the kernel, which may call back into [`Cpu`] and [`Host`], and returns to 1.
//!
//! A system call that finishes at once comes back as "run the same thread, [`Resume::Return`]", which a
//! driver turns into a plain return from the translated code's system call import, with no stack switch.

#![no_std]
// TODO: remove these two lines, and `RUSTC_BOOTSTRAP` in .cargo/config.toml, once Rust lets a tool register its own lints on a stable compiler (https://github.com/rust-lang/rust/issues/66079).
#![feature(register_tool)]
#![register_tool(reviewable)]

pub mod readiness;

mod cpu;
mod exit;
mod host;
mod machine;
mod memory;
mod registers;

pub use cpu::{Cpu, CpuError, MemoryId, PerThreadWatch, SharedId, ThreadId};
pub use exit::{
    Access, Exit, Next, Resume, SIGNALLED_EXIT_BASE, SYSTEM_CALL_ARGUMENT_COUNT,
    SYSTEM_CALL_NUMBER_REGISTER, SystemCallNumber, SystemCallRequest,
};
pub use host::{
    ConnectionId, CounterReading, End, Event, Host, HostError, NANOSECONDS_PER_SECOND, Warning,
};
pub use machine::{HWCAP, HWCAP2, hwcap};
pub use memory::{
    AddressSpace, KIB, LOWEST_ADDRESS, PAGE_MASK, PAGE_SHIFT, PAGE_SIZE, Permissions, WORD,
};
pub use registers::{RegisterBlock, STACK_ALIGNMENT, STOP_FLAG_OFFSET, THREAD_BLOCK_SIZE, pstate};
