#![no_std]

extern crate alloc;

mod foreign_implementation {
    #[path = "queue/implementation.rs"]
    mod queue;
    #[path = "slice/implementation.rs"]
    mod slice;
}

/// A source of bytes.
#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait ByteSource {
    type Error;

    /// Reads bytes into the start of the buffer, once, and returns the number of bytes read.
    fn read(&mut self, into: &mut [u8]) -> Result<usize, Self::Error>;
}
