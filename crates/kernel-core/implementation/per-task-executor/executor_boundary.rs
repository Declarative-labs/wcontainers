//! The one place this core assumes anything: what an executor does when it runs a task.

use vstd::prelude::*;
use wcontainers_kernel_core::execution::{Interrupter, Stop};
use wcontainers_kernel_core::memory::MemoryCapabilityTable;
use wcontainers_memory_shard::MemoryShard;

use crate::disjoint_memory::DisjointMemory;

verus! {

pub open spec fn allows(
    permissions: wcontainers_memory_shard::MemoryPermissions,
    access: wcontainers_kernel_core::execution::Access,
) -> bool {
    match access {
        wcontainers_kernel_core::execution::Access::Read => permissions.read,
        wcontainers_kernel_core::execution::Access::Write => permissions.write,
        wcontainers_kernel_core::execution::Access::Execute => permissions.execute,
    }
}

/// Whether some shard holds the address with a permission for the access.
pub open spec fn permits<Shard: MemoryShard>(
    shards: Map<u64, Shard>,
    address: u64,
    access: wcontainers_kernel_core::execution::Access,
) -> bool {
    exists|start: u64| #[trigger]
        shards.contains_key(start) && crate::disjoint_memory::contains_address(shards, start, address) && allows(
            shards.index(start).spec_permissions(),
            access,
        )
}

/// The part of an executor that runs one task's instructions on the task's memory.
///
/// An implementation that is not proven is trusted for all of it: that it reaches only the bytes
/// of the shards in `memory`, each only as its permissions allow; that it starts from `processor`
/// and leaves there the state it stopped at; and that only the interruption it is given stops it.
pub trait ExecutorBoundary<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>>: Sized {
    type ProcessorState;

    /// What a run reads to learn that its interrupter asked it to stop.
    type Interruption;

    type Interrupter: Interrupter;

    /// Returns the interruption of one run and the interrupter that sets it.
    fn prepare_interruption() -> (prepared: (Self::Interruption, Self::Interrupter));

    fn run(
        &mut self,
        memory: &DisjointMemory<Shard, Table>,
        processor: &mut Self::ProcessorState,
        interruption: Self::Interruption,
    ) -> (stop: Stop)
        ensures
            stop matches Stop::MemoryFault { address, access } ==> !permits(
                crate::disjoint_memory::to_shards(*memory),
                address,
                access,
            ),
    ;
}

/// What a core makes each new task from.
pub trait TaskSource<Shard: MemoryShard> {
    type Table: MemoryCapabilityTable<Shard>;

    type Runner: ExecutorBoundary<Shard, Self::Table>;

    type CreateError;

    /// Returns an empty table, the runner of a new task, and the state it starts from.
    fn create_parts(&mut self) -> (created: Result<
        (
            Self::Table,
            Self::Runner,
            <Self::Runner as ExecutorBoundary<Shard, Self::Table>>::ProcessorState,
        ),
        Self::CreateError,
    >)
        ensures
            created matches Ok(parts) ==> wcontainers_capability_table::CapabilityTable::<
                u64,
                Shard,
                wcontainers_memory_shard::MemoryShardConfig,
                wcontainers_kernel_core::memory::MemoryError,
            >::state(&parts.0) == Map::<u64, Shard>::empty(),
    ;
}

} // verus!
