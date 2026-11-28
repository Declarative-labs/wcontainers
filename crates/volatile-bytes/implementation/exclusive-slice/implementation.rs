//! A slice borrowed exclusively and kept in vstd's `InvCell`, whose invariant fixes the slice's length.

#![no_std]

use vstd::cell::invcell::InvCell;
use vstd::prelude::*;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};

impl<'a> StandaloneVolatileBytes for ExclusiveSliceBytes<'a> {
    type Config = &'a mut [u8];

    fn new(slice: &'a mut [u8]) -> ExclusiveSliceBytes<'a> {
        hold(slice)
    }
}

verus! {

/// The cell's invariant: it holds a slice of the length, or nothing while a sink or a source has the slice.
spec fn holds_length(held: Option<&mut [u8]>, length: usize) -> bool {
    held matches Some(slice) ==> slice@.len() == length
}

/// A slice borrowed exclusively, which `read_to` and `write_from` lend to the sink or the source in place.
///
/// While the slice is lent, `read_at` and `write_at` through the same value count as the contract says and move no
/// byte, and `read_to` and `write_from` return `Ok(0)`.
pub struct ExclusiveSliceBytes<'a> {
    held: LengthCell<'a>,
}

/// The slice's length and the cell that holds the slice, whose invariant keeps the two equal.
struct LengthCell<'a> {
    length: usize,
    cell: InvCell<Option<&'a mut [u8]>, spec_fn(Option<&'a mut [u8]>) -> bool>,
}

impl<'a> LengthCell<'a> {
    #[verifier::type_invariant]
    closed spec fn holds(self) -> bool {
        forall|held: Option<&'a mut [u8]>| #[trigger] self.cell.inv(held) == holds_length(held, self.length)
    }
}

/// Returns the bytes of the slice, with the cell's invariant fixed to the slice's length.
fn hold<'a>(slice: &'a mut [u8]) -> (bytes: ExclusiveSliceBytes<'a>)
    ensures
        bytes.spec_len() == old(slice)@.len(),
{
    let length = slice.len();
    let held = LengthCell {
        length,
        cell: InvCell::new(Some(slice), Ghost(|held: Option<&'a mut [u8]>| holds_length(held, length))),
    };
    ExclusiveSliceBytes { held }
}

/// Returns how many bytes a copy at the offset moves in `length` bytes, as `copied` says, for while the slice is lent.
fn count_copied(length: usize, offset: u64, buffer: usize) -> (count: usize)
    ensures
        count == wcontainers_volatile_bytes::copied(length as nat, offset, buffer as nat),
{
    if offset >= length as u64 {
        usize::MIN
    } else if buffer <= length - offset as usize {
        buffer
    } else {
        length - offset as usize
    }
}

impl<'a> VolatileBytes for ExclusiveSliceBytes<'a> {
    closed spec fn spec_len(&self) -> nat {
        self.held.length as nat
    }

    fn len(&self) -> u64 {
        self.held.length as u64
    }

    // While the slice is here, the copy is cut from it as a plain slice copy is, so the build sees both copies are in
    // bounds; while it is lent, the count is still `copied` and no byte is reachable to move.
    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        proof {
            use_type_invariant(&self.held);
        }
        match self.held.cell.replace(None) {
            Some(slice) => {
                let count = if offset < slice.len() as u64 {
                    let from = &slice[offset as usize..];
                    let count = if into.len() <= from.len() {
                        into.len()
                    } else {
                        from.len()
                    };
                    into[..count].copy_from_slice(&from[..count]);
                    count
                } else {
                    usize::MIN
                };
                self.held.cell.set(Some(slice));
                count
            },
            None => count_copied(self.held.length, offset, into.len()),
        }
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        proof {
            use_type_invariant(&self.held);
        }
        match self.held.cell.replace(None) {
            Some(slice) => {
                let count = if offset < slice.len() as u64 {
                    let to = &mut slice[offset as usize..];
                    let count = if bytes.len() <= to.len() {
                        bytes.len()
                    } else {
                        to.len()
                    };
                    to[..count].copy_from_slice(&bytes[..count]);
                    count
                } else {
                    usize::MIN
                };
                self.held.cell.set(Some(slice));
                count
            },
            None => count_copied(self.held.length, offset, bytes.len()),
        }
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        proof {
            use_type_invariant(&self.held);
        }
        match self.held.cell.replace(None) {
            Some(slice) => {
                let read = sink.write(slice);
                self.held.cell.set(Some(slice));
                read
            },
            None => Ok(usize::MIN),
        }
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        proof {
            use_type_invariant(&self.held);
        }
        match self.held.cell.replace(None) {
            Some(slice) => {
                let written = source.read(slice);
                self.held.cell.set(Some(slice));
                written
            },
            None => Ok(usize::MIN),
        }
    }
}

} // verus!
