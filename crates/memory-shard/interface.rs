#![no_std]

use thiserror::Error;
use vstd::prelude::*;
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

#[derive(Debug, Error)]
#[verifier::external_derive]
pub enum BorrowError {
    #[error("the bytes do not all lie inside the shard")]
    OutsideShard,
}

} // verus!
