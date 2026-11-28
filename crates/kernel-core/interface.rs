#![no_std]

use vstd::prelude::*;
use wcontainers_capability_table::CapabilityTable;
use wcontainers_memory_shard::MemoryShard;

verus! {

/// All of a kernel except its userspace ABI.
pub trait KernelCore {
    /// An individual thread of execution, with its own set of capabilities.
    type Task;

    type Memory: MemoryShard;

    type Capabilities: CapabilityTable<u64, Self::Memory>;

    spec fn capabilities_of(task: Self::Task) -> Self::Capabilities;

    fn capabilities(task: &Self::Task) -> (capabilities: &Self::Capabilities)
        ensures
            *capabilities == Self::capabilities_of(*task),
    ;

    fn capabilities_mut(task: &mut Self::Task) -> (capabilities: &mut Self::Capabilities)
        ensures
            *capabilities == Self::capabilities_of(*old(task)),
            Self::capabilities_of(*final(task)) == *final(capabilities),
    ;

    type CreateTaskError;

    fn create_task(&mut self) -> (created: Result<Self::Task, Self::CreateTaskError>)
        ensures
            created matches Ok(task) ==> CapabilityTable::<u64, Self::Memory>::state(
                &Self::capabilities_of(task),
            ) == Map::<u64, Self::Memory>::empty(),
    ;
}

} // verus!
