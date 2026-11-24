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

/// A sink of bytes.
pub trait ByteSink {
    type Error;

    /// Writes bytes from the start of the buffer, once, and returns the number of bytes written.
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error>;
}
