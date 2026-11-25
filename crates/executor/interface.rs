#![no_std]

extern crate alloc;

pub mod memory_space;

use core::future::Future;
use core::ops::Range;

use wcontainers_abi::{Access, CpuError, Permissions, SystemCallRequest, ThreadId};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::ProcessorArchitecture;

use crate::memory_space::MemorySpace;

/// The runner of guest threads.
pub trait Executor: BehaviorSource {
    type TranslatorConfig: Default;
    type GuestMemory: MemorySpace;
    type Interrupter: ExecutionInterrupter + 'static;
    // VERUS: no `impl Trait` in a trait, so the returned future is a named type.
    type Execution<'execution>: Future<Output = ProcessorStateOf<Self>> + 'execution
    where
        Self: 'execution;

    type CreateGuestMemoryError;
    fn create_guest_memory(
        &self,
    ) -> Result<(Self::GuestMemory, PageTableOf<Self>), Self::CreateGuestMemoryError>;

    /// Returns the execution from the given processor state on the given memory, and its interrupter.
    ///
    /// The execution runs until a trap terminates it, and completes with the processor state it stopped at.
    fn execute<'execution>(
        &'execution mut self,
        processor: ProcessorStateOf<Self>,
        memory: &'execution Self::GuestMemory,
        handler: &'execution mut impl TrapHandler,
    ) -> (Self::Execution<'execution>, Self::Interrupter);
}

pub type ProcessorStateOf<E> =
    <<E as BehaviorSource>::Architecture as ProcessorArchitecture>::ProcessorState;

pub type PageTableOf<E> = <<E as Executor>::GuestMemory as MemorySpace>::PageTable;

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
