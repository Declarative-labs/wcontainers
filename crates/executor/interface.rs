#![no_std]

use core::ops::Range;

use wcontainers_abi::{CpuError, Permissions, ThreadId};

/// Executes guest threads.
pub trait Executor {
    type TranslatorConfig: Default;
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
