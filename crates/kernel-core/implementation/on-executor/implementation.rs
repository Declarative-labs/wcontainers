//! A kernel core whose every task holds its own executor, guest memory and saved processor state beside an ordered table of the memory it holds.

#![no_std]

extern crate alloc;

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

use spin::Once;
use vstd::prelude::*;
use wcontainers_abi::{self as abi, SystemCallRequest};
use wcontainers_executor::{
    Execution as _, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, PageTableOf, ProcessorStateOf, SystemCallResponse, TrapHandler,
};
use wcontainers_kernel_core::KernelCore;
use wcontainers_kernel_core::execution::{Access, Execution, Interrupter, Stop};
use wcontainers_kernel_core::memory::TaskMemory;
use wcontainers_kernel_core::processor_state::TaskProcessorState;
use wcontainers_memory_capability_table_ordered::OrderedMemoryCapabilityTable;
use wcontainers_memory_shard_shared_bytes::SharedBytesMemoryShard;

verus! {

/// What the core needs of an executor: the one place where the core trusts code it does not prove.
///
/// Its one implementation, for every `Executor` that can be cloned, sits below outside the proof and is trusted to
/// return; no clause about the executor is assumed, so nothing the core proves rests on what an executor does.
pub trait ExecutorBoundary: Sized {
    /// The registers of the processor.
    type ProcessorState;

    /// The memory the executor runs a task's instructions on.
    type GuestMemory;

    /// What a run holds of its interrupter.
    type RunSide;

    /// What stops the run whose side was made with it.
    type InterruptSide: Send + Sync;

    type CreateError;

    /// Returns an executor for a new task, its own guest memory and a copy of the processor state it starts from.
    fn create_for_task(&mut self, initial: &Self::ProcessorState) -> Result<
        (Self, Self::GuestMemory, Self::ProcessorState),
        Self::CreateError,
    >;

    /// Returns the two sides of the interrupter of one run.
    fn prepare_interrupter() -> (Self::RunSide, Self::InterruptSide);

    /// Stops the run made with the other side; does nothing once it has run.
    fn interrupt(interrupter: &Self::InterruptSide);

    /// Runs the instructions from the processor state until they stop, and leaves the state they stopped at.
    fn run(
        &mut self,
        processor: &mut Self::ProcessorState,
        memory: &Self::GuestMemory,
        interrupter: Self::RunSide,
    ) -> Stop;
}

/// A kernel core that runs its tasks on copies of one executor.
pub struct OnExecutorKernelCore<Executor: ExecutorBoundary> {
    executor: Executor,
    initial: Executor::ProcessorState,
}

/// A thread of execution: the memory it holds, the processor state it stopped at, and what runs it.
pub struct OnExecutorTask<Executor: ExecutorBoundary> {
    memory: OrderedMemoryCapabilityTable,
    processor: Executor::ProcessorState,
    executor: Executor,
    guest_memory: Executor::GuestMemory,
}

impl<Executor: ExecutorBoundary> OnExecutorKernelCore<Executor> {
    /// Returns a core whose new tasks run on copies of the executor from the processor state.
    pub fn new(executor: Executor, initial: Executor::ProcessorState) -> (core: Self) {
        OnExecutorKernelCore { executor, initial }
    }
}

impl<Executor: ExecutorBoundary> TaskMemory<OnExecutorTask<Executor>> for OnExecutorKernelCore<
    Executor,
> {
    type Memory = SharedBytesMemoryShard;

    type MemoryCapabilities = OrderedMemoryCapabilityTable;

    proof fn lemma_shards_are_disjoint(task: OnExecutorTask<Executor>) {
        OrderedMemoryCapabilityTable::lemma_disjoint(task.memory);
    }

    closed spec fn memory_capabilities_of(task: OnExecutorTask<Executor>) -> OrderedMemoryCapabilityTable {
        task.memory
    }

    fn memory_capabilities(task: &OnExecutorTask<Executor>) -> (memory_capabilities:
        &OrderedMemoryCapabilityTable) {
        &task.memory
    }

    fn memory_capabilities_mut(task: &mut OnExecutorTask<Executor>) -> (memory_capabilities:
        &mut OrderedMemoryCapabilityTable) {
        &mut task.memory
    }
}

impl<Executor: ExecutorBoundary> TaskProcessorState<OnExecutorTask<Executor>> for
    OnExecutorKernelCore<Executor> {
    type SavedProcessorState = Executor::ProcessorState;

    closed spec fn saved_processor_state_of(task: OnExecutorTask<Executor>) -> Executor::ProcessorState {
        task.processor
    }

    fn saved_processor_state(task: &OnExecutorTask<Executor>) -> (saved_processor_state:
        &Executor::ProcessorState) {
        &task.processor
    }

    fn saved_processor_state_mut(task: &mut OnExecutorTask<Executor>) -> (saved_processor_state:
        &mut Executor::ProcessorState) {
        &mut task.processor
    }
}

/// One run of a task, which borrows the task until it has run.
pub struct OnExecutorExecution<'task, Executor: ExecutorBoundary> {
    task: &'task mut OnExecutorTask<Executor>,
    interrupter: Executor::RunSide,
}

/// What stops the one run it was prepared with.
pub struct OnExecutorInterrupter<Executor: ExecutorBoundary> {
    side: Executor::InterruptSide,
}

impl<'task, Executor: ExecutorBoundary> Execution<
    'task,
    OnExecutorTask<Executor>,
    OnExecutorKernelCore<Executor>,
> for OnExecutorExecution<'task, Executor> {
    closed spec fn task(self) -> &'task mut OnExecutorTask<Executor> {
        self.task
    }

    fn run(self) -> (stop: Stop) {
        let task = self.task;
        Executor::run(&mut task.executor, &mut task.processor, &task.guest_memory, self.interrupter)
    }
}

impl<Executor: ExecutorBoundary> Interrupter for OnExecutorInterrupter<Executor> {
    fn interrupt(&self) {
        Executor::interrupt(&self.side);
    }
}

// VERUS: no bound on an associated type of an impl, so the executor outlives every borrow of a task.
impl<Executor: ExecutorBoundary + 'static> KernelCore for OnExecutorKernelCore<Executor> {
    type Task = OnExecutorTask<Executor>;

    type CreateTaskError = Executor::CreateError;

    fn create_task(&mut self) -> (created: Result<OnExecutorTask<Executor>, Executor::CreateError>) {
        let (executor, guest_memory, processor) = match self.executor.create_for_task(&self.initial) {
            Ok(created) => created,
            Err(error) => return Err(error),
        };
        Ok(OnExecutorTask { memory: OrderedMemoryCapabilityTable::new(), processor, executor, guest_memory })
    }

    type Execution<'task> = OnExecutorExecution<'task, Executor>;

    type Interrupter = OnExecutorInterrupter<Executor>;

    fn prepare_execution(task: &mut OnExecutorTask<Executor>) -> (prepared: (
        OnExecutorExecution<'_, Executor>,
        OnExecutorInterrupter<Executor>,
    )) {
        let (interrupter, side) = Executor::prepare_interrupter();
        (OnExecutorExecution { task, interrupter }, OnExecutorInterrupter { side })
    }
}

/// The executor boundary for every executor that can be cloned, trusted to return and proven by nobody.
#[allow(
    reviewable::implementation_not_verified,
    reason = "TODO: prove it against the trait's contract, then remove this line; the executor interface states no clause to prove it from"
)]
#[verifier::external]
impl<Runner: Executor + Clone> ExecutorBoundary for Runner {
    type ProcessorState = ProcessorStateOf<Runner>;
    type GuestMemory = (Runner::GuestMemory, PageTableOf<Runner>);
    type RunSide = Arc<InterruptSlot<Runner::Interrupter>>;
    type InterruptSide = Arc<InterruptSlot<Runner::Interrupter>>;
    type CreateError = Runner::CreateGuestMemoryError;

    fn create_for_task(&mut self, initial: &Self::ProcessorState) -> Result<
        (Self, Self::GuestMemory, Self::ProcessorState),
        Self::CreateError,
    > {
        let executor = self.clone();
        let memory = executor.create_guest_memory()?;
        Ok((executor, memory, initial.clone()))
    }

    fn prepare_interrupter() -> (Self::RunSide, Self::InterruptSide) {
        let slot = Arc::new(InterruptSlot { interrupted: AtomicBool::new(false), executor: Once::new() });
        (Arc::clone(&slot), slot)
    }

    fn interrupt(interrupter: &Self::InterruptSide) {
        interrupter.interrupted.store(true, Ordering::SeqCst);
        if let Some(executor) = interrupter.executor.get() {
            executor.interrupt();
        }
    }

    fn run(
        &mut self,
        processor: &mut Self::ProcessorState,
        memory: &Self::GuestMemory,
        interrupter: Self::RunSide,
    ) -> Stop {
        let (execution, executor_interrupter) = self.prepare_execution(processor.clone(), &memory.0);
        let executor_interrupter = interrupter.executor.call_once(|| executor_interrupter);
        if interrupter.interrupted.load(Ordering::SeqCst) {
            executor_interrupter.interrupt();
        }
        let mut handler = StopAtTrap { stop: None };
        *processor = match execution.run(&mut handler) {
            ExecutionStop::Terminated(stopped) => stopped,
            ExecutionStop::Waiting { processor: stopped, .. } => stopped,
        };
        handler.stop.unwrap_or(Stop::Interrupt)
    }
}

} // verus!

/// Where a run's interrupter finds the executor's, once the run has made it.
pub struct InterruptSlot<ExecutorInterrupter> {
    interrupted: AtomicBool,
    executor: Once<ExecutorInterrupter>,
}

// Ends a run at its first trap and keeps why.
struct StopAtTrap {
    stop: Option<Stop>,
}

impl TrapHandler for StopAtTrap {
    fn handle_system_call(&mut self, _request: SystemCallRequest) -> SystemCallResponse {
        self.stop = Some(Stop::SystemCall);
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, address: u64, access: abi::Access) -> MemoryFaultResponse {
        let access = match access {
            abi::Access::Read => Access::Read,
            abi::Access::Write => Access::Write,
            abi::Access::Execute => Access::Execute,
        };
        self.stop = Some(Stop::MemoryFault { address, access });
        MemoryFaultResponse::Terminate
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.stop = Some(Stop::Interrupt);
        InterruptResponse::Terminate
    }
}
