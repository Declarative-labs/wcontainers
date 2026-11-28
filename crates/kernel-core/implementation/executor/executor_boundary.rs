//! The one place that calls the executor, whose interface states no contract to prove against.

use wcontainers_abi::{Access as ExecutorAccess, SystemCallRequest};
use wcontainers_executor::{
    Execution as _, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, PageTableOf, ProcessorStateOf, SystemCallResponse, TrapHandler,
};
use wcontainers_kernel_core::execution::{Access, Stop};

/// Returns a new guest memory of the executor and its page table.
pub fn create_guest_memory_on<E: Executor>(
    executor: &E,
) -> Result<(E::GuestMemory, PageTableOf<E>), E::CreateGuestMemoryError> {
    executor.create_guest_memory()
}

/// Returns the executor's execution from the processor state on the memory, and its interrupter.
pub fn prepare_on_executor<'execution, E: Executor>(
    executor: &'execution mut E,
    processor: ProcessorStateOf<E>,
    memory: &'execution E::GuestMemory,
) -> (E::Execution<'execution>, E::Interrupter) {
    executor.prepare_execution(processor, memory)
}

/// Runs the executor's execution to its first trap, and returns why it stopped and the processor state it stopped at.
pub fn run_on_executor<'execution, E: Executor>(
    execution: E::Execution<'execution>,
) -> (Stop, ProcessorStateOf<E>) {
    let mut handler = FirstTrap { stop: None };
    let processor = match execution.run(&mut handler) {
        ExecutionStop::Terminated(processor) => processor,
        ExecutionStop::Waiting { processor, .. } => processor,
    };
    (handler.stop.unwrap_or(Stop::Interrupt), processor)
}

/// Stops the executor's execution that the interrupter was prepared with.
pub fn interrupt_on_executor<E: Executor>(interrupter: &E::Interrupter) {
    interrupter.interrupt();
}

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
