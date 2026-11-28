use core::cell::RefCell;

use vstd::prelude::*;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;

use crate::VolatileBytes;

verus! {

/// A slice held exclusively, which a source or a sink reaches in place.
#[allow(reviewable::implementation_not_verified, reason = "TODO: prove it against the trait's contract, then remove this line")]
#[verifier::external]
impl VolatileBytes for RefCell<&mut [u8]> {
    spec fn spec_len(&self) -> nat {
        arbitrary()
    }

    fn len(&self) -> u64 {
        self.borrow().len() as u64
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        let held = self.borrow();
        let Some(from) = usize::try_from(offset).ok().and_then(|offset| held.get(offset..)) else {
            return usize::MIN;
        };
        let count = into.len().min(from.len());
        into[..count].copy_from_slice(&from[..count]);
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        let mut held = self.borrow_mut();
        let Some(to) = usize::try_from(offset).ok().and_then(|offset| held.get_mut(offset..)) else {
            return usize::MIN;
        };
        let count = bytes.len().min(to.len());
        to[..count].copy_from_slice(&bytes[..count]);
        count
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        sink.write(&self.borrow())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        source.read(&mut self.borrow_mut())
    }
}

}
