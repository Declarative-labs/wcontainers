#![no_std]

extern crate alloc;

pub mod memory_space;

use core::future::Future;
use core::ops::Range;

use vstd::prelude::*;
use wcontainers_abi::{Access, CpuError, Permissions, SystemCallRequest, ThreadId};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::ProcessorArchitecture;

use crate::memory_space::MemorySpace;

verus! {

/// The runner of guest threads.
pub trait Executor: BehaviorSource {
    type TranslatorConfig: Default;
    type GuestMemory: MemorySpace;
    type Interrupter: ExecutionInterrupter + 'static;
    // VERUS: no `impl Trait` in a trait, so the returned execution is a named type.
    type Execution<'execution>: Execution<ProcessorState = ProcessorStateOf<Self>>
    where
        Self: 'execution;

    type CreateGuestMemoryError;
    fn create_guest_memory(
        &self,
    ) -> Result<(Self::GuestMemory, PageTableOf<Self>), Self::CreateGuestMemoryError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Returns the execution from the given processor state on the given memory, and its interrupter.
    fn prepare_execution<'execution>(
        &'execution mut self,
        processor: ProcessorStateOf<Self>,
        memory: &'execution Self::GuestMemory,
    ) -> (Self::Execution<'execution>, Self::Interrupter)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

/// One execution of a guest thread, not yet run.
pub trait Execution {
    type ProcessorState;
    /// What the executor waits for when it cannot go on.
    type Wait: Future<Output = ()>;

    /// Runs until a trap terminates the execution or the executor must wait.
    // VERUS: no proof of an `async` body in an implementation of a trait, so this is a plain call that returns when it must wait and not a future.
    fn run<Handler: TrapHandler>(self, handler: &mut Handler)
    -> ExecutionStop<Self::ProcessorState, Self::Wait>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

/// Why an execution stopped, with the processor state it stopped at.
pub enum ExecutionStop<ProcessorState, Wait> {
    Terminated(ProcessorState),
    /// The executor cannot go on until the wait completes.
    Waiting {
        processor: ProcessorState,
        wait: Wait,
    },
}

pub type ProcessorStateOf<E> =
    <<E as BehaviorSource>::Architecture as ProcessorArchitecture>::ProcessorState;

pub type PageTableOf<E> = <<E as Executor>::GuestMemory as MemorySpace>::PageTable;

/// The handler of a guest's traps: the points where the guest stops and control passes to the system.
///
/// The guest stays stopped at the trap while a method runs.
pub trait TrapHandler {
    fn handle_system_call(&mut self, request: SystemCallRequest) -> SystemCallResponse
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    fn handle_interrupt(&mut self) -> InterruptResponse
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
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
    fn interrupt(&self)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

/// An executor that can watch one thread and not its siblings.
pub trait PerThreadWatch: Executor {
    fn set_thread_permissions(
        &mut self,
        thread: ThreadId,
        range: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), CpuError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

} // verus!
