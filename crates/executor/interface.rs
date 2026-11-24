#![no_std]

extern crate alloc;

pub mod memory_space;
#[cfg(creusot)]
mod memory_space_proofs;

use alloc::vec::Vec;
use core::future::Future;
use core::ops::Range;

use wcontainers_abi::{Access, CpuError, Permissions, SystemCallRequest, ThreadId};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};

use crate::memory_space::MemorySpace;

/// The runner of guest threads.
pub trait Executor {
    type Architecture: ProcessorArchitecture;
    type TranslatorConfig: Default;
    type GuestMemory: MemorySpace;
    type Interrupter: ExecutionInterrupter + 'static;

    type CreateGuestMemoryError;
    fn create_guest_memory(&self) -> Result<Self::GuestMemory, Self::CreateGuestMemoryError>;

    /// Returns the execution from the given processor state on the given memory, and its interrupter.
    ///
    /// The execution runs until a trap terminates it, and completes with the processor state it stopped at.
    fn execute<'execution>(
        &'execution mut self,
        processor: ProcessorStateOf<Self>,
        memory: &'execution Self::GuestMemory,
        handler: &'execution mut impl TrapHandler,
    ) -> (
        impl Future<Output = ProcessorStateOf<Self>> + 'execution,
        Self::Interrupter,
    );

    /// Executes exactly one instruction on the given processor + memory.
    ///
    /// Processor changes are written in place, memory changes are written through the hook.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut ProcessorStateOf<Self>,
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

pub type ProcessorStateOf<E> =
    <<E as Executor>::Architecture as ProcessorArchitecture>::ProcessorState;

/// The handler of a guest's traps: the points where the guest stops and control passes to the system.
///
/// The guest stays stopped at the trap while a method runs.
pub trait TrapHandler {
    fn handle_system_call(&mut self, request: SystemCallRequest) -> SystemCallResponse;

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse;

    fn handle_interrupt(&mut self) -> InterruptResponse;
}

pub enum SystemCallResponse {
    /// Continues after the system call with the given result.
    Return(u64),
    Terminate,
}

pub enum MemoryFaultResponse {
    /// Executes the faulting instruction again.
    Retry,
    Terminate,
}

pub enum InterruptResponse {
    Continue,
    Terminate,
}

pub trait ExecutionInterrupter: Send + Sync {
    fn interrupt(&self);
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
