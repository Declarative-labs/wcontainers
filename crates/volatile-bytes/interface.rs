#![no_std]

mod foreign_implementation {
    #[path = "ref_cell/implementation.rs"]
    mod ref_cell;
}

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;

/// Bytes that something else may be changing while they are held.
pub trait VolatileBytes {
    fn len(&self) -> u64;

    fn is_empty(&self) -> bool {
        self.len() == u64::MIN
    }

    /// Copies bytes from the start into the buffer, as many as the shorter of the two holds, and returns the number of bytes read.
    fn read(&self, into: &mut [u8]) -> usize;

    /// Copies bytes from the buffer to the start, as many as the shorter of the two holds, and returns the number of bytes written.
    fn write(&self, bytes: &[u8]) -> usize;

    /// Reads to the sink in one write of all the bytes, and returns the number of bytes read.
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error>;

    /// Writes from the source in one read into all the bytes, and returns the number of bytes written.
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error>;
}

pub trait StandaloneVolatileBytes: VolatileBytes {
    type Config;

    fn new(config: Self::Config) -> Self;
}
