#![no_std]

pub mod execution;
pub mod memory;
pub mod processor_state;

use vstd::prelude::*;

use crate::execution::{Execution, Interrupter};
use crate::memory::TaskMemory;
use crate::processor_state::TaskProcessorState;

verus! {

/// All of a kernel except its userspace ABI.
pub trait KernelCore: TaskMemory<Self::Task> + TaskProcessorState<Self::Task> {
    /// An individual thread of execution, with its own set of capabilities.
    type Task;

    type CreateTaskError;

    fn create_task(&mut self) -> (created: Result<Self::Task, Self::CreateTaskError>)
        ensures
            created matches Ok(task) ==> memory::holds_no_memory(
                Self::memory_capabilities_of(task),
            ),
    ;

    type Execution<'task>: Execution<'task, Self::Task, Self> where Self::Task: 'task;

    type Interrupter: Interrupter;

    fn prepare_execution(task: &mut Self::Task) -> (prepared: (
        Self::Execution<'_>,
        Self::Interrupter,
    ))
        ensures
            *prepared.0.task() == *old(task),
            *final(prepared.0.task()) == *final(task),
    ;
}

} // verus!
