//! The one place that calls the executor, whose interface states no contract to prove against.

use vstd::prelude::*;
use wcontainers_abi::{Access as ExecutorAccess, SystemCallRequest};
use wcontainers_executor::{
    Execution as _, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, PageTableOf, ProcessorStateOf, SystemCallResponse, TrapHandler,
};
use wcontainers_kernel_core::execution::{Access, Stop};

verus! {

/// What the kernel core uses of an executor, assumed to return and to promise nothing past its types.
pub trait ExecutorBoundary: Sized {
    type ProcessorState;
    type GuestMemory;
    type PageTable;
    type Prepared<'execution> where Self: 'execution;
    type Stopper: Send + Sync;
    type CreateGuestMemoryError;

    /// Returns a new guest memory of the executor and its page table.
    fn make_guest_memory(&self) -> Result<(Self::GuestMemory, Self::PageTable), Self::CreateGuestMemoryError>;

    /// Returns the executor's execution from the processor state on the memory, and what stops it.
    fn prepare_run<'execution>(
        &'execution mut self,
        processor: Self::ProcessorState,
        memory: &'execution Self::GuestMemory,
    ) -> (Self::Prepared<'execution>, Self::Stopper);

    /// Runs the execution to its first trap, and returns why it stopped and the processor state it stopped at.
    fn run_prepared<'execution>(prepared: Self::Prepared<'execution>) -> (Stop, Self::ProcessorState);

    /// Stops the execution the stopper was prepared with.
    fn stop_run(stopper: &Self::Stopper);

    fn copy_processor_state(processor: &Self::ProcessorState) -> Self::ProcessorState;
}

#[verifier::external]
impl<E: Executor> ExecutorBoundary for E {
    type ProcessorState = ProcessorStateOf<E>;
    type GuestMemory = E::GuestMemory;
    type PageTable = PageTableOf<E>;
    type Prepared<'execution>
        = E::Execution<'execution>
    where
        E: 'execution;
    type Stopper = E::Interrupter;
    type CreateGuestMemoryError = E::CreateGuestMemoryError;

    fn make_guest_memory(
        &self,
    ) -> Result<(E::GuestMemory, PageTableOf<E>), E::CreateGuestMemoryError> {
        Executor::create_guest_memory(self)
    }

    fn prepare_run<'execution>(
        &'execution mut self,
        processor: ProcessorStateOf<E>,
        memory: &'execution E::GuestMemory,
    ) -> (E::Execution<'execution>, E::Interrupter) {
        Executor::prepare_execution(self, processor, memory)
    }

    fn run_prepared<'execution>(
        prepared: E::Execution<'execution>,
    ) -> (Stop, ProcessorStateOf<E>) {
        let mut handler = FirstTrap { stop: None };
        let processor = match prepared.run(&mut handler) {
            ExecutionStop::Terminated(processor) => processor,
            ExecutionStop::Waiting { processor, .. } => processor,
        };
        (handler.stop.unwrap_or(Stop::Interrupt), processor)
    }

    fn stop_run(stopper: &E::Interrupter) {
        stopper.interrupt();
    }

    fn copy_processor_state(processor: &ProcessorStateOf<E>) -> ProcessorStateOf<E> {
        processor.clone()
    }
}

} // verus!

/// A trap handler that ends the execution at its first trap and keeps why.
struct FirstTrap {
    stop: Option<Stop>,
}

impl TrapHandler for FirstTrap {
    fn handle_system_call(&mut self, _request: SystemCallRequest) -> SystemCallResponse {
        self.stop = Some(Stop::SystemCall);
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, address: u64, access: ExecutorAccess) -> MemoryFaultResponse {
        let access = match access {
            ExecutorAccess::Read => Access::Read,
            ExecutorAccess::Write => Access::Write,
            ExecutorAccess::Execute => Access::Execute,
        };
        self.stop = Some(Stop::MemoryFault { address, access });
        MemoryFaultResponse::Terminate
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.stop = Some(Stop::Interrupt);
        InterruptResponse::Terminate
    }
}
