//! What runs a container's guest threads, as the kernel's interface to its embedder names it.

#![no_std]

use core::ops::Range;

use wcontainers_abi::{CpuError, Permissions, ThreadId};

/// A runner of guest threads with its own translator settings.
pub trait Executor {
    type TranslatorConfig;
}

/// An executor that can watch one thread and not its siblings.
pub trait PerThreadWatch: Executor {
    fn set_thread_permissions(
        &mut self,
        thread: ThreadId,
        range: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), CpuError>;
}
