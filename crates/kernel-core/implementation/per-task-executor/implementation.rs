//! Each task holds its memory as disjoint shards, its saved registers, and its own part of an executor, which runs it.

#![no_std]

extern crate alloc;

pub mod byte_code;
pub mod disjoint_memory;
pub mod executor;
pub mod executor_boundary;

use core::marker::PhantomData;

use vstd::prelude::*;
use wcontainers_kernel_core::KernelCore;
use wcontainers_kernel_core::execution::{Execution, Stop};
use wcontainers_kernel_core::memory::TaskMemory;
use wcontainers_kernel_core::processor_state::TaskProcessorState;
use wcontainers_memory_shard::MemoryShard;

use crate::disjoint_memory::DisjointMemory;
use crate::executor_boundary::{ExecutorBoundary, TaskSource};

verus! {

pub type ProcessorStateOf<Shard, Source> = <<Source as TaskSource<Shard>>::Runner as ExecutorBoundary<
    Shard,
    <Source as TaskSource<Shard>>::Table,
>>::ProcessorState;

pub type InterruptionOf<Shard, Source> = <<Source as TaskSource<Shard>>::Runner as ExecutorBoundary<
    Shard,
    <Source as TaskSource<Shard>>::Table,
>>::Interruption;

pub type InterrupterOf<Shard, Source> = <<Source as TaskSource<Shard>>::Runner as ExecutorBoundary<
    Shard,
    <Source as TaskSource<Shard>>::Table,
>>::Interrupter;

/// A task, which only its core makes and which nobody copies or compares.
pub struct Task<Shard: MemoryShard, Source: TaskSource<Shard>> {
    memory: DisjointMemory<Shard, Source::Table>,
    processor: ProcessorStateOf<Shard, Source>,
    runner: Source::Runner,
}

/// One run of a task, holding the task until it has run.
pub struct PreparedExecution<'task, Shard: MemoryShard, Source: TaskSource<Shard>> {
    task: &'task mut Task<Shard, Source>,
    interruption: InterruptionOf<Shard, Source>,
}

pub struct PerTaskExecutorKernelCore<Shard, Source> {
    source: Source,
    shard: PhantomData<Shard>,
}

impl<Shard: MemoryShard, Source: TaskSource<Shard>> PerTaskExecutorKernelCore<Shard, Source> {
    pub fn new(source: Source) -> Self {
        PerTaskExecutorKernelCore { source, shard: PhantomData }
    }
}

impl<Shard: MemoryShard, Source: TaskSource<Shard>> TaskMemory<
    Task<Shard, Source>,
> for PerTaskExecutorKernelCore<Shard, Source> {
    type Memory = Shard;

    type MemoryCapabilities = DisjointMemory<Shard, Source::Table>;

    proof fn lemma_shards_are_disjoint(task: Task<Shard, Source>) {
        DisjointMemory::lemma_shards_are_disjoint(task.memory);
    }

    closed spec fn memory_capabilities_of(task: Task<Shard, Source>) -> DisjointMemory<
        Shard,
        Source::Table,
    > {
        task.memory
    }

    fn memory_capabilities(task: &Task<Shard, Source>) -> (memory_capabilities: &DisjointMemory<
        Shard,
        Source::Table,
    >) {
        &task.memory
    }

    fn memory_capabilities_mut(task: &mut Task<Shard, Source>) -> (memory_capabilities:
        &mut DisjointMemory<Shard, Source::Table>) {
        &mut task.memory
    }
}

impl<Shard: MemoryShard, Source: TaskSource<Shard>> TaskProcessorState<
    Task<Shard, Source>,
> for PerTaskExecutorKernelCore<Shard, Source> {
    type SavedProcessorState = ProcessorStateOf<Shard, Source>;

    closed spec fn saved_processor_state_of(task: Task<Shard, Source>) -> ProcessorStateOf<
        Shard,
        Source,
    > {
        task.processor
    }

    fn saved_processor_state(task: &Task<Shard, Source>) -> (saved_processor_state:
        &ProcessorStateOf<Shard, Source>) {
        &task.processor
    }

    fn saved_processor_state_mut(task: &mut Task<Shard, Source>) -> (saved_processor_state:
        &mut ProcessorStateOf<Shard, Source>) {
        &mut task.processor
    }
}

impl<'task, Shard: MemoryShard, Source: TaskSource<Shard>> Execution<
    'task,
    Task<Shard, Source>,
    PerTaskExecutorKernelCore<Shard, Source>,
> for PreparedExecution<'task, Shard, Source> {
    closed spec fn task(self) -> &'task mut Task<Shard, Source> {
        self.task
    }

    fn run(self) -> (stop: Stop) {
        let PreparedExecution { task, interruption } = self;
        let Task { memory, processor, runner } = task;
        runner.run(memory, processor, interruption)
    }
}

impl<Shard: MemoryShard + 'static, Source: TaskSource<Shard> + 'static> KernelCore for PerTaskExecutorKernelCore<
    Shard,
    Source,
> {
    type Task = Task<Shard, Source>;

    type CreateTaskError = Source::CreateError;

    fn create_task(&mut self) -> (created: Result<Task<Shard, Source>, Source::CreateError>) {
        match self.source.create_parts() {
            Ok((table, runner, processor)) => Ok(
                Task { memory: DisjointMemory::new(table), processor, runner },
            ),
            Err(error) => Err(error),
        }
    }

    type Execution<'task> = PreparedExecution<'task, Shard, Source>;

    type Interrupter = InterrupterOf<Shard, Source>;

    fn prepare_execution(task: &mut Task<Shard, Source>) -> (prepared: (
        PreparedExecution<'_, Shard, Source>,
        InterrupterOf<Shard, Source>,
    )) {
        let (interruption, interrupter) = Source::Runner::prepare_interruption();
        (PreparedExecution { task, interruption }, interrupter)
    }
}

} // verus!
