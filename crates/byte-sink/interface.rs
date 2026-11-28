#![no_std]

extern crate alloc;

mod foreign_implementation {
    #[path = "queue/implementation.rs"]
    mod queue;
    #[path = "slice/implementation.rs"]
    mod slice;
    #[path = "vector/implementation.rs"]
    mod vector;
}

use vstd::prelude::*;

verus! {

/// A sink of bytes.
pub trait ByteSink {
    type Error;

    /// Writes bytes from the start of the buffer, once, and returns the number of bytes written.
    fn write(&mut self, bytes: &[u8]) -> (written: Result<usize, Self::Error>)
        ensures
            written matches Ok(count) ==> count <= bytes@.len(),
    ;
}

}
