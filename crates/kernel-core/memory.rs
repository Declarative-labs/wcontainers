use vstd::prelude::*;
use wcontainers_memory_capability_table::MemoryCapabilityTable;
use wcontainers_memory_shard::MemoryShard;

verus! {

pub trait TaskMemory<Task> {
    type Memory: MemoryShard;

    type MemoryCapabilities: MemoryCapabilityTable<Self::Memory>;

    proof fn lemma_shards_are_disjoint(task: Task)
        ensures
            shards_are_disjoint(Self::memory_capabilities_of(task)),
    ;

    spec fn memory_capabilities_of(task: Task) -> Self::MemoryCapabilities;

    fn memory_capabilities(task: &Task) -> (memory_capabilities: &Self::MemoryCapabilities)
        ensures
            *memory_capabilities == Self::memory_capabilities_of(*task),
    ;

    fn memory_capabilities_mut(task: &mut Task) -> (memory_capabilities:
        &mut Self::MemoryCapabilities)
        ensures
            *memory_capabilities == Self::memory_capabilities_of(*old(task)),
            Self::memory_capabilities_of(*final(task)) == *final(memory_capabilities),
    ;
}

pub open spec fn holds_no_memory<Shard: MemoryShard, Capabilities: MemoryCapabilityTable<Shard>>(
    capabilities: Capabilities,
) -> bool {
    capabilities.state() == Map::<u64, Shard>::empty()
}

pub open spec fn shards_are_disjoint<
    Shard: MemoryShard,
    Capabilities: MemoryCapabilityTable<Shard>,
>(capabilities: Capabilities) -> bool {
    let shards = capabilities.state();
    forall|first: u64, second: u64|
        #![trigger shards.contains_key(first), shards.contains_key(second)]
        shards.contains_key(first) && shards.contains_key(second) && first != second ==> first
            + shards.index(first).spec_len() <= second || second + shards.index(second).spec_len()
            <= first
}

} // verus!
