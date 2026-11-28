//! A kernel core whose tasks each run on their own copy of an executor, with a list of the memory they hold.

#![no_std]

extern crate alloc;

pub mod executor_boundary;

use alloc::sync::Arc;

use vstd::prelude::*;
use vstd::rwlock::{RwLock, RwLockPredicate};
use wcontainers_kernel_core::KernelCore;
use wcontainers_kernel_core::execution::{Execution, Interrupter, Stop};
use wcontainers_kernel_core::memory::TaskMemory;
use wcontainers_kernel_core::processor_state::TaskProcessorState;
use wcontainers_memory_capability_table_list::ListMemoryCapabilityTable;
use wcontainers_memory_shard_shared_allocation::SharedAllocationMemoryShard;

use crate::executor_boundary::ExecutorBoundary;

verus! {

/// A kernel core that gives each new task a copy of its executor, starting from its processor state.
pub struct ExecutorKernelCore<E: ExecutorBoundary> {
    executor: E,
    processor: E::ProcessorState,
}

impl<E: ExecutorBoundary + Clone> ExecutorKernelCore<E> {
    /// Returns a core whose tasks run on copies of the executor and start from the processor state.
    pub fn new(executor: E, processor: E::ProcessorState) -> (core: Self) {
        ExecutorKernelCore { executor, processor }
    }
}

/// A thread of execution: its memory, its saved processor state and the executor it runs on.
pub struct ExecutorTask<E: ExecutorBoundary> {
    memory: ListMemoryCapabilityTable,
    processor: E::ProcessorState,
    executor: E,
    guest_memory: E::GuestMemory,
    page_table: E::PageTable,
}

// Where one execution is, which its interrupter reads to know what to stop.
enum Progress<Stopper> {
    Prepared,
    Interrupted,
    Running(Stopper),
    Ran,
}

// The rule every progress meets, which any does.
struct AnyProgress;

impl<Stopper> RwLockPredicate<Progress<Stopper>> for AnyProgress {
    open spec fn inv(self, progress: Progress<Stopper>) -> bool {
        true
    }
}

/// One run of a task, not yet started.
pub struct ExecutorExecution<'task, E: ExecutorBoundary> {
    task: &'task mut ExecutorTask<E>,
    progress: Arc<RwLock<Progress<E::Stopper>, AnyProgress>>,
}

/// What stops the one execution it was prepared with.
pub struct ExecutorInterrupter<E: ExecutorBoundary> {
    progress: Arc<RwLock<Progress<E::Stopper>, AnyProgress>>,
}

impl<E: ExecutorBoundary> Interrupter for ExecutorInterrupter<E> {
    fn interrupt(&self) {
        let (progress, handle) = self.progress.acquire_write();
        match progress {
            Progress::Prepared => handle.release_write(Progress::Interrupted),
            Progress::Running(stopper) => {
                E::stop_run(&stopper);
                handle.release_write(Progress::Running(stopper));
            },
            other => handle.release_write(other),
        }
    }
}

impl<'task, E: ExecutorBoundary + Clone> Execution<'task, ExecutorTask<E>, ExecutorKernelCore<E>> for ExecutorExecution<'task, E> {
    closed spec fn task(self) -> &'task mut ExecutorTask<E> {
        self.task
    }

    fn run(self) -> (stop: Stop) {
        let ExecutorExecution { task, progress } = self;
        let (before, handle) = progress.acquire_write();
        if let Progress::Interrupted = before {
            handle.release_write(Progress::Ran);
            return Stop::Interrupt;
        }
        let processor = E::copy_processor_state(&task.processor);
        let (prepared, stopper) = task.executor.prepare_run(processor, &task.guest_memory);
        handle.release_write(Progress::Running(stopper));
        let (stop, processor) = E::run_prepared(prepared);
        let (_, handle) = progress.acquire_write();
        handle.release_write(Progress::Ran);
        task.processor = processor;
        stop
    }
}

impl<E: ExecutorBoundary + Clone> TaskMemory<ExecutorTask<E>> for ExecutorKernelCore<E> {
    type Memory = SharedAllocationMemoryShard;

    type MemoryCapabilities = ListMemoryCapabilityTable;

    proof fn lemma_shards_are_disjoint(task: ExecutorTask<E>) {
        wcontainers_memory_capability_table_list::lemma_shards_are_disjoint(task.memory);
    }

    closed spec fn memory_capabilities_of(task: ExecutorTask<E>) -> ListMemoryCapabilityTable {
        task.memory
    }

    fn memory_capabilities(task: &ExecutorTask<E>) -> (memory_capabilities: &ListMemoryCapabilityTable) {
        &task.memory
    }

    fn memory_capabilities_mut(task: &mut ExecutorTask<E>) -> (memory_capabilities: &mut ListMemoryCapabilityTable) {
        &mut task.memory
    }
}

impl<E: ExecutorBoundary + Clone> TaskProcessorState<ExecutorTask<E>> for ExecutorKernelCore<E> {
    type SavedProcessorState = E::ProcessorState;

    closed spec fn saved_processor_state_of(task: ExecutorTask<E>) -> E::ProcessorState {
        task.processor
    }

    fn saved_processor_state(task: &ExecutorTask<E>) -> (saved_processor_state: &E::ProcessorState) {
        &task.processor
    }

    fn saved_processor_state_mut(task: &mut ExecutorTask<E>) -> (saved_processor_state: &mut E::ProcessorState) {
        &mut task.processor
    }
}

impl<E: ExecutorBoundary + Clone + 'static> KernelCore for ExecutorKernelCore<E> {
    type Task = ExecutorTask<E>;

    type CreateTaskError = E::CreateGuestMemoryError;

    fn create_task(&mut self) -> (created: Result<ExecutorTask<E>, E::CreateGuestMemoryError>) {
        let (guest_memory, page_table) = match self.executor.make_guest_memory() {
            Ok(made) => made,
            Err(error) => return Err(error),
        };
        Ok(ExecutorTask {
            memory: ListMemoryCapabilityTable::new(),
            processor: E::copy_processor_state(&self.processor),
            executor: self.executor.clone(),
            guest_memory,
            page_table,
        })
    }

    type Execution<'task> = ExecutorExecution<'task, E> where ExecutorTask<E>: 'task;

    type Interrupter = ExecutorInterrupter<E>;

    fn prepare_execution(task: &mut ExecutorTask<E>) -> (prepared: (ExecutorExecution<'_, E>, ExecutorInterrupter<E>)) {
        let progress = Arc::new(RwLock::new(Progress::Prepared, Ghost(AnyProgress)));
        let interrupter = ExecutorInterrupter { progress: Arc::clone(&progress) };
        (ExecutorExecution { task, progress }, interrupter)
    }
}

} // verus!
