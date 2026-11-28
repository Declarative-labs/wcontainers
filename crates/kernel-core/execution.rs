use vstd::prelude::*;

use crate::memory::TaskMemory;

verus! {

/// One run of a task, which borrows the task until it has run.
pub trait Execution<'task, Task: 'task, Memory: TaskMemory<Task> + ?Sized>: Sized {
    spec fn task(self) -> &'task mut Task;

    /// Runs the task's instructions until it stops, and returns why it stopped.
    fn run(self) -> (stop: Stop)
        ensures
            Memory::memory_capabilities_of(*final(self.task())) == Memory::memory_capabilities_of(
                *self.task(),
            ),
    ;
}

/// What stops the one execution it was prepared with.
pub trait Interrupter: Send + Sync {
    /// Stops the execution this was prepared with; does nothing once it has run.
    fn interrupt(&self);
}

/// Why a task stopped running.
pub enum Stop {
    /// The task made a system call.
    SystemCall,
    /// The task accessed an address in a way that nothing it holds allows.
    MemoryFault { address: u64, access: Access },
    /// The execution's interrupter stopped it.
    Interrupt,
}

pub enum Access {
    Read,
    Write,
    Execute,
}

} // verus!
