//! TODO: say in one sentence how this implementation of wcontainers-kernel-core works.

#![no_std]

use wcontainers_kernel_core::KernelCore;

// TODO: impl TaskMemory<Self::Task> for ExecutorKernelCore
// TODO: impl TaskProcessorState<Self::Task> for ExecutorKernelCore
pub struct ExecutorKernelCore;

#[allow(
    reviewable::implementation_not_verified,
    reason = "TODO: prove it against the trait's contract, then remove this line"
)]
impl KernelCore for ExecutorKernelCore {
    type Task = core::convert::Infallible;
    type CreateTaskError = core::convert::Infallible;

    fn create_task(&mut self) -> Result<Self::Task, Self::CreateTaskError> {
        todo!()
    }

    // TODO: type Execution<'task>: Execution<'task, Self::Task, Self> where Self::Task: 'task;
    // TODO: type Interrupter: Interrupter;

    fn prepare_execution(task: &mut Self::Task) -> (Self::Execution<'_>, Self::Interrupter) {
        todo!()
    }
}
