#![no_std]

extern crate alloc;

mod foreign_implementation {
    #[path = "queue/implementation.rs"]
    mod queue;
    #[path = "slice/implementation.rs"]
    mod slice;
}

use vstd::prelude::*;

verus! {

/// A source of bytes.
pub trait ByteSource {
    type Error;

    /// Reads bytes into the start of the buffer, once, and returns the number of bytes read.
    fn read(&mut self, into: &mut [u8]) -> (read: Result<usize, Self::Error>)
        ensures
            final(into)@.len() == old(into)@.len(),
            read matches Ok(count) ==> count <= old(into)@.len(),
    ;
}

}
