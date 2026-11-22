#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::ops::Range;

use wcontainers_abi::{CpuError, Permissions, ThreadId};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};

/// The runner of guest threads.
pub trait Executor {
    type Architecture: ProcessorArchitecture;
    type TranslatorConfig: Default;

    /// Executes exactly one instruction on the given processor + memory.
    ///
    /// Processor changes are written in place, memory changes are written through the hook.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut <Self::Architecture as ProcessorArchitecture>::Processor,
        memory: &[MemoryDelta<'_, Self::Architecture>],
        on_memory_mutated: impl FnMut(MemoryDelta<'_, Self::Architecture>),
    );

    /// Executes exactly one instruction from the before state.
    fn execute_single_instruction<'memory>(
        &mut self,
        mut before: ExecutionState<'memory, Self::Architecture>,
    ) -> ExecutionState<'memory, Self::Architecture> {
        let mut mutations = Vec::new();
        self.execute_single_instruction_in_place(
            &mut before.processor,
            &before.memory_mutations,
            |mutation| mutations.push(mutation.into_owned()),
        );
        before.memory_mutations.append(&mut mutations);
        before
    }
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
