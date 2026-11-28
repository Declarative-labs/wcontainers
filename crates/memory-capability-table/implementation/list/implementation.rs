//! TODO: say in one sentence how this implementation of wcontainers-memory-capability-table works.

#![no_std]

use wcontainers_memory_capability_table::{MemoryCapabilityTable, SetPermissionsError, SplitError};
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard};

// TODO: impl CapabilityTable<u64, Shard, MemoryShardConfig, MemoryError> for ListMemoryCapabilityTable
pub struct ListMemoryCapabilityTable;

#[allow(
    reviewable::implementation_not_verified,
    reason = "TODO: prove it against the trait's contract, then remove this line"
)]
impl<Shard: MemoryShard> MemoryCapabilityTable<Shard> for ListMemoryCapabilityTable {
    fn set_permissions(
        &mut self,
        address: &u64,
        permissions: MemoryPermissions,
    ) -> Result<(), SetPermissionsError> {
        todo!()
    }

    fn split(&mut self, address: &u64, offset: u64) -> Result<(), SplitError> {
        todo!()
    }
}
