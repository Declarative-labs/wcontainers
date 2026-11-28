#![no_std]

use thiserror::Error;
use vstd::prelude::*;
use wcontainers_capability_table::CapabilityConfig;
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

pub trait MemoryShard {
    type ByteView<'shard>: VolatileBytes where Self: 'shard;

    spec fn spec_len(&self) -> u64;

    #[verifier::when_used_as_spec(spec_len)]
    fn len(&self) -> (length: u64)
        ensures
            length == self.spec_len(),
    ;

    spec fn spec_permissions(&self) -> MemoryPermissions;

    #[verifier::when_used_as_spec(spec_permissions)]
    fn permissions(&self) -> (permissions: MemoryPermissions)
        ensures
            permissions == self.spec_permissions(),
    ;

    fn borrow(&self, offset: u64, length: u64) -> (borrowed: Result<
        Self::ByteView<'_>,
        BorrowError,
    >)
        ensures
            match borrowed {
                Ok(bytes) => offset + length <= self.len() && bytes.spec_len() == length,
                Err(_) => offset + length > self.len(),
            },
    ;
}

/// What the instructions of a task may do with the bytes of a shard.
#[derive(Clone, Copy)]
pub struct MemoryPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

pub struct MemoryShardConfig {
    /// The length of the shard in bytes.
    pub length: u64,
    pub permissions: MemoryPermissions,
}

impl<Shard: MemoryShard> CapabilityConfig<Shard> for MemoryShardConfig {
    open spec fn describes(self, shard: Shard) -> bool {
        shard.spec_len() == self.length && shard.spec_permissions() == self.permissions
    }
}

#[derive(Debug, Error)]
#[verifier::external_derive]
pub enum BorrowError {
    #[error("the bytes do not all lie inside the shard")]
    OutsideShard,
}

} // verus!
