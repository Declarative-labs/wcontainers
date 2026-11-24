use core::cell::RefCell;

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;

use crate::VolatileBytes;

/// A slice held exclusively, which a source or a sink reaches in place.
impl VolatileBytes for RefCell<&mut [u8]> {
    fn len(&self) -> u64 {
        self.borrow().len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let held = self.borrow();
        let count = into.len().min(held.len());
        into[..count].copy_from_slice(&held[..count]);
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let mut held = self.borrow_mut();
        let count = bytes.len().min(held.len());
        held[..count].copy_from_slice(&bytes[..count]);
        count
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        sink.write(&self.borrow())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        source.read(&mut self.borrow_mut())
    }
}
