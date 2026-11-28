//! TODO: say in one sentence how this implementation of wcontainers-memory-shard works.

#![no_std]

use wcontainers_memory_shard::{BorrowError, MemoryPermissions, MemoryShard};

pub struct SharedAllocationMemoryShard;

#[allow(
    reviewable::implementation_not_verified,
    reason = "TODO: prove it against the trait's contract, then remove this line"
)]
impl MemoryShard for SharedAllocationMemoryShard {
    // TODO: type ByteView<'shard>: VolatileBytes where Self: 'shard;

    fn len(&self) -> u64 {
        todo!()
    }

    fn permissions(&self) -> MemoryPermissions {
        todo!()
    }

    fn borrow(&self, offset: u64, length: u64) -> Result<Self::ByteView<'_>, BorrowError> {
        todo!()
    }
}
