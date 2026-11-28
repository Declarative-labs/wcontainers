//! A kernel core whose tasks each run on their own copy of an executor, with a list of the memory they hold.

#![no_std]

extern crate alloc;

mod executor_boundary;

use alloc::sync::Arc;

use vstd::prelude::*;
use vstd::rwlock::{RwLock, RwLockPredicate};
use wcontainers_executor::{Executor, PageTableOf, ProcessorStateOf};
use wcontainers_kernel_core::KernelCore;
use wcontainers_kernel_core::execution::{Execution, Interrupter, Stop};
use wcontainers_kernel_core::memory::TaskMemory;
use wcontainers_kernel_core::processor_state::TaskProcessorState;
use wcontainers_memory_capability_table_list::ListMemoryCapabilityTable;
use wcontainers_memory_shard_shared_allocation::SharedAllocationMemoryShard;

use crate::executor_boundary::{prepare_on_executor, run_on_executor};

verus! {

/// A kernel core that gives each new task a copy of its executor, starting from its processor state.
pub struct ExecutorKernelCore<E: Executor> {
    executor: E,
    processor: ProcessorStateOf<E>,
}

impl<E: Executor + Clone> ExecutorKernelCore<E> {
    /// Returns a core whose tasks run on copies of the executor and start from the processor state.
    pub fn new(executor: E, processor: ProcessorStateOf<E>) -> (core: Self) {
        ExecutorKernelCore { executor, processor }
    }
}

/// A thread of execution: its memory, its saved processor state and the executor it runs on.
pub struct ExecutorTask<E: Executor> {
    memory: ListMemoryCapabilityTable,
    processor: ProcessorStateOf<E>,
    executor: E,
    guest_memory: E::GuestMemory,
    page_table: PageTableOf<E>,
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
pub struct ExecutorExecution<'task, E: Executor> {
    task: &'task mut ExecutorTask<E>,
    progress: Arc<RwLock<Progress<E::Interrupter>, AnyProgress>>,
}

/// What stops the one execution it was prepared with.
pub struct ExecutorInterrupter<E: Executor> {
    progress: Arc<RwLock<Progress<E::Interrupter>, AnyProgress>>,
}

impl<E: Executor> Interrupter for ExecutorInterrupter<E> {
    fn interrupt(&self) {
        let (progress, handle) = self.progress.acquire_write();
        match progress {
            Progress::Prepared => handle.release_write(Progress::Interrupted),
            Progress::Running(stopper) => {
                executor_boundary::interrupt_on_executor::<E>(&stopper);
                handle.release_write(Progress::Running(stopper));
            },
            other => handle.release_write(other),
        }
    }
}

impl<'task, E: Executor + Clone> Execution<'task, ExecutorTask<E>, ExecutorKernelCore<E>> for ExecutorExecution<'task, E> {
    fn run(self) -> (stop: Stop) {
        let ExecutorExecution { task, progress } = self;
        let (before, handle) = progress.acquire_write();
        if let Progress::Interrupted = before {
            handle.release_write(Progress::Ran);
            return Stop::Interrupt;
        }
        let (execution, stopper) = prepare_on_executor(&mut task.executor, task.processor.clone(), &task.guest_memory);
        handle.release_write(Progress::Running(stopper));
        let (stop, processor) = run_on_executor::<E>(execution);
        let (_, handle) = progress.acquire_write();
        handle.release_write(Progress::Ran);
        task.processor = processor;
        stop
    }
}

impl<E: Executor + Clone> TaskMemory<ExecutorTask<E>> for ExecutorKernelCore<E> {
    type Memory = SharedAllocationMemoryShard;

    type MemoryCapabilities = ListMemoryCapabilityTable;

    fn memory_capabilities(task: &ExecutorTask<E>) -> (memory_capabilities: &ListMemoryCapabilityTable) {
        &task.memory
    }

    fn memory_capabilities_mut(task: &mut ExecutorTask<E>) -> (memory_capabilities: &mut ListMemoryCapabilityTable) {
        &mut task.memory
    }
}

impl<E: Executor + Clone> TaskProcessorState<ExecutorTask<E>> for ExecutorKernelCore<E> {
    type SavedProcessorState = ProcessorStateOf<E>;

    fn saved_processor_state(task: &ExecutorTask<E>) -> (saved_processor_state: &ProcessorStateOf<E>) {
        &task.processor
    }

    fn saved_processor_state_mut(task: &mut ExecutorTask<E>) -> (saved_processor_state: &mut ProcessorStateOf<E>) {
        &mut task.processor
    }
}

impl<E: Executor + Clone + 'static> KernelCore for ExecutorKernelCore<E> {
    type Task = ExecutorTask<E>;

    type CreateTaskError = E::CreateGuestMemoryError;

    fn create_task(&mut self) -> (created: Result<ExecutorTask<E>, E::CreateGuestMemoryError>) {
        let (guest_memory, page_table) = executor_boundary::create_guest_memory_on(&self.executor)?;
        Ok(ExecutorTask {
            memory: ListMemoryCapabilityTable::new(),
            processor: self.processor.clone(),
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
