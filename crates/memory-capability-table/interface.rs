#![no_std]

use thiserror::Error;
use vstd::prelude::*;
use wcontainers_capability_table::CapabilityTable;
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};

verus! {

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
            ({
                let before = old(self).state();
                let after = final(self).state();
                match set {
                    Ok(_) => {
                        let shard = before.index(*address);
                        let changed = after.index(*address);

                        &&& before.contains_key(*address)
                        &&& after.contains_key(*address)
                        &&& after.remove(*address) == before.remove(*address)
                        &&& views_bytes_of(changed, shard, 0, shard.spec_len() as int)
                        &&& changed.spec_permissions() == permissions
                    },
                    Err(SetPermissionsError::NothingHeld) => !before.contains_key(*address) && after
                        == before,
                    Err(SetPermissionsError::Failed(_)) => before.contains_key(*address) && after
                        == before,
                }
            }),
    ;

    /// Splits a shard `offset` bytes in, into two views of its allocation with its permissions.
    fn split(&mut self, address: &u64, offset: u64) -> (split: Result<(), SplitError>)
        ensures
            ({
                let before = old(self).state();
                let after = final(self).state();
                match split {
                    Ok(_) => {
                        let shard = before.index(*address);
                        let second_address = (*address + offset) as u64;
                        let first = after.index(*address);
                        let second = after.index(second_address);

                        &&& before.contains_key(*address)
                        &&& 0 < offset < shard.spec_len()
                        &&& after.contains_key(*address)
                        &&& after.contains_key(second_address)
                        &&& after.remove(*address).remove(second_address) == before.remove(*address)
                        &&& views_bytes_of(first, shard, 0, offset as int)
                        &&& views_bytes_of(second, shard, offset as int, shard.spec_len() - offset)
                        &&& first.spec_permissions() == shard.spec_permissions()
                        &&& second.spec_permissions() == shard.spec_permissions()
                    },
                    Err(SplitError::NothingHeld) => !before.contains_key(*address) && after
                        == before,
                    Err(SplitError::OutsideShard) => before.contains_key(*address) && !(0 < offset
                        < before.index(*address).spec_len()) && after == before,
                    Err(SplitError::Failed(_)) => before.contains_key(*address) && after == before,
                }
            }),
    ;
}

/// Whether a shard is a view of `length` bytes of another shard's bytes, starting `offset` bytes in.
pub open spec fn views_bytes_of<Shard: MemoryShard>(
    part: Shard,
    whole: Shard,
    offset: int,
    length: int,
) -> bool {
    &&& part.spec_len() == length
    &&& part.spec_viewed_allocation() == whole.spec_viewed_allocation()
    &&& part.spec_view_start() == whole.spec_view_start() + offset
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
