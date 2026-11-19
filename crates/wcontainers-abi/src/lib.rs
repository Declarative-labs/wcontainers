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

mod cpu;
mod exit;
mod host;
mod machine;
mod memory;
mod registers;

pub use cpu::{Cpu, CpuError, MemoryId, PerThreadWatch, SharedId, ThreadId};
pub use exit::{Access, Exit, Next, Resume, Syscall};
pub use host::{
    BackendRequestId, ConnectionId, CounterReading, End, Event, Host, HostError, Warning,
};
pub use machine::{HWCAP, HWCAP2, hwcap};
pub use memory::{AddressSpace, LOWEST_ADDRESS, PAGE_SIZE, Permissions};
pub use registers::{RegisterBlock, STOP_FLAG_OFFSET, THREAD_BLOCK_SIZE, pstate};
