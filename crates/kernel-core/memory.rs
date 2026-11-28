use thiserror::Error;
use vstd::prelude::*;
use wcontainers_capability_table::CapabilityTable;
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};

verus! {

pub trait TaskMemory<Task> {
    type Memory: MemoryShard;

    type MemoryCapabilities: MemoryCapabilityTable<Self::Memory>;

    proof fn lemma_shards_are_disjoint(task: Task)
        ensures
            shards_are_disjoint(Self::memory_capabilities_of(task).state()),
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

/// The memory a task holds, as shards under their starting virtual addresses.
pub trait MemoryCapabilityTable<Shard: MemoryShard>: CapabilityTable<
    u64,
    Shard,
    MemoryShardConfig,
    MemoryError,
> {
    fn set_permissions(&mut self, address: &u64, permissions: MemoryPermissions) -> (set: Result<
        (),
        SetPermissionsError,
    >)
        ensures
            match set {
                Ok(_) => {
                    &&& old(self).state().contains_key(*address)
                    &&& final(self).state().contains_key(*address)
                    &&& final(self).state().remove(*address) == old(self).state().remove(*address)
                    &&& final(self).state().index(*address).spec_len() == old(self).state().index(
                        *address,
                    ).spec_len()
                    &&& final(self).state().index(*address).spec_permissions() == permissions
                },
                Err(SetPermissionsError::NothingHeld) => !old(self).state().contains_key(*address)
                    && final(self).state() == old(self).state(),
                Err(SetPermissionsError::Failed(_)) => old(self).state().contains_key(*address)
                    && final(self).state() == old(self).state(),
            },
    ;

    /// Splits the shard at an address into two shards with its permissions, the second starting
    /// `offset` bytes in.
    fn split(&mut self, address: &u64, offset: u64) -> (split: Result<(), SplitError>)
        ensures
            match split {
                Ok(_) => {
                    let shard = old(self).state().index(*address);
                    let second = (*address + offset) as u64;
                    &&& old(self).state().contains_key(*address)
                    &&& 0 < offset < shard.spec_len()
                    &&& final(self).state().contains_key(*address)
                    &&& final(self).state().contains_key(second)
                    &&& final(self).state().remove(*address).remove(second) == old(
                        self,
                    ).state().remove(*address)
                    &&& final(self).state().index(*address).spec_len() == offset
                    &&& final(self).state().index(second).spec_len() == shard.spec_len() - offset
                    &&& final(self).state().index(*address).spec_permissions()
                        == shard.spec_permissions()
                    &&& final(self).state().index(second).spec_permissions()
                        == shard.spec_permissions()
                },
                Err(SplitError::NothingHeld) => !old(self).state().contains_key(*address)
                    && final(self).state() == old(self).state(),
                Err(SplitError::OutsideShard) => old(self).state().contains_key(*address) && !(0
                    < offset < old(self).state().index(*address).spec_len()) && final(self).state()
                    == old(self).state(),
                Err(SplitError::Failed(_)) => old(self).state().contains_key(*address)
                    && final(self).state() == old(self).state(),
            },
    ;
}

pub open spec fn holds_no_memory<Shard: MemoryShard, Capabilities: MemoryCapabilityTable<Shard>>(
    capabilities: Capabilities,
) -> bool {
    capabilities.state() == Map::<u64, Shard>::empty()
}

pub open spec fn shards_are_disjoint<Shard: MemoryShard>(shards: Map<u64, Shard>) -> bool {
    forall|first: u64, second: u64|
        #![trigger shards.contains_key(first), shards.contains_key(second)]
        shards.contains_key(first) && shards.contains_key(second) && first != second ==> first
            + shards.index(first).spec_len() <= second || second + shards.index(second).spec_len()
            <= first
}

#[derive(Debug, Error)]
#[verifier::external_derive]
pub enum MemoryError {
    #[error("the shard would overlap a shard the task already holds")]
    Overlap,
    #[error("no memory is left")]
    OutOfMemory,
}

#[derive(Debug, Error)]
#[verifier::external_derive]
pub enum SetPermissionsError {
    #[error("no shard is held at the address")]
    NothingHeld,
    #[error(transparent)]
    Failed(MemoryError),
}

#[derive(Debug, Error)]
#[verifier::external_derive]
pub enum SplitError {
    #[error("no shard is held at the address")]
    NothingHeld,
    #[error("the offset does not lie inside the shard")]
    OutsideShard,
    #[error(transparent)]
    Failed(MemoryError),
}

} // verus!
