#![no_std]

mod foreign_implementation {
    #[path = "ref_cell/implementation.rs"]
    mod ref_cell;
}

use vstd::prelude::*;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;

verus! {

/// Bytes that something else may be changing while they are held.
pub trait VolatileBytes {
    /// How many bytes there are, which never changes.
    spec fn spec_len(&self) -> nat;

    fn len(&self) -> (length: u64)
        ensures
            length == self.spec_len(),
    ;

    fn is_empty(&self) -> (empty: bool)
        ensures
            empty == (self.spec_len() == 0),
    {
        self.len() == u64::MIN
    }

    /// Copies bytes from the offset into the buffer, as many as the shorter of the two holds, and returns how many; none from past the end.
    fn read_at(&self, offset: u64, into: &mut [u8]) -> (count: usize)
        ensures
            reads(self.spec_len(), offset, old(into)@, final(into)@, count),
    ;

    /// Copies bytes from the buffer to the offset, as many as the shorter of the two holds, and returns how many; none to past the end.
    fn write_at(&self, offset: u64, bytes: &[u8]) -> (count: usize)
        ensures
            count == copied(self.spec_len(), offset, bytes@.len()),
    ;

    /// Copies bytes from the start into the buffer, as many as the shorter of the two holds, and returns the number of bytes read.
    fn read(&self, into: &mut [u8]) -> (count: usize)
        ensures
            reads(self.spec_len(), 0, old(into)@, final(into)@, count),
    {
        self.read_at(u64::MIN, into)
    }

    /// Copies bytes from the buffer to the start, as many as the shorter of the two holds, and returns the number of bytes written.
    fn write(&self, bytes: &[u8]) -> (count: usize)
        ensures
            count == copied(self.spec_len(), 0, bytes@.len()),
    {
        self.write_at(u64::MIN, bytes)
    }

    /// Reads to the sink in one write of all the bytes, and returns the number of bytes read.
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> (read: Result<usize, Sink::Error>)
        ensures
            read matches Ok(count) ==> count <= self.spec_len(),
    ;

    /// Writes from the source in one read into all the bytes, and returns the number of bytes written.
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> (written: Result<usize, Source::Error>)
        ensures
            written matches Ok(count) ==> count <= self.spec_len(),
    ;
}

/// What a read at the offset does: it moves exactly `copied` bytes and leaves the rest of the buffer as it was.
pub open spec fn reads(length: nat, offset: u64, before: Seq<u8>, after: Seq<u8>, count: usize) -> bool {
    &&& count == copied(length, offset, before.len())
    &&& after.len() == before.len()
    &&& forall|index: int| count <= index < before.len() ==> after[index] == before[index]
}

/// How many bytes a copy at the offset moves: the shorter of the buffer and what lies from the offset to the end.
pub open spec fn copied(length: nat, offset: u64, buffer: nat) -> nat {
    if offset >= length {
        0
    } else if buffer <= length - offset {
        buffer
    } else {
        (length - offset) as nat
    }
}

pub trait StandaloneVolatileBytes: VolatileBytes {
    type Config;

    fn new(config: Self::Config) -> Self
    where
        Self: Sized,
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

} // verus!
