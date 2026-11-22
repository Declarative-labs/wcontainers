#![no_std]

use core::ops::Range;

use wcontainers_abi::{CpuError, Permissions, ThreadId};
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};

/// The runner of guest threads.
pub trait Executor {
    type Architecture: ProcessorArchitecture;
    type TranslatorConfig: Default;

    /// Executes exactly one instruction from the before state.
    fn execute_single_instruction(
        &mut self,
        before: ExecutionState<Self::Architecture>,
    ) -> ExecutionState<Self::Architecture>;
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
