//! A memory shard as a view of part of an allocation of bytes, held by reference so that every view of it shares them.

#![no_std]

extern crate alloc;

use alloc::rc::Rc;
use alloc::vec::Vec;

use vstd::cell::invcell::InvCell;
use vstd::predicate::Predicate;
use vstd::prelude::*;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_memory_shard::{BorrowError, MemoryPermissions, MemoryShard};
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

/// The most bytes one allocation holds.
pub const MOST_BYTES: u64 = 1u64 << 26;

/// A view of `length` bytes of an allocation, starting `start` bytes in, with what a task may do with them.
pub struct SharedAllocationMemoryShard {
    allocation: Rc<Allocation>,
    start: u64,
    length: u64,
    permissions: MemoryPermissions,
}

// The bytes of one allocation, each in a cell that a shared borrow may write.
struct Allocation {
    bytes: Vec<InvCell<u8, AnyByte>>,
}

// The rule every byte meets, which any byte does.
struct AnyByte;

impl Predicate<u8> for AnyByte {
    open spec fn predicate(&self, byte: u8) -> bool {
        true
    }
}

impl SharedAllocationMemoryShard {
    /// Returns a view of all of a new allocation of zero bytes, or `None` past the most bytes an allocation holds.
    pub fn allocate(length: u64, permissions: MemoryPermissions) -> (allocated: Option<Self>) {
        if length > MOST_BYTES {
            return None;
        }
        let mut bytes: Vec<InvCell<u8, AnyByte>> = Vec::new();
        let mut made: u64 = 0;
        while made < length {
            bytes.push(InvCell::new(0, Ghost(AnyByte)));
            made += 1;
        }
        Some(SharedAllocationMemoryShard {
            allocation: Rc::new(Allocation { bytes }),
            start: 0,
            length,
            permissions,
        })
    }

    /// Returns a second view of the same bytes with the same permissions.
    pub fn duplicate(&self) -> (duplicate: Self) {
        SharedAllocationMemoryShard {
            allocation: Rc::clone(&self.allocation),
            start: self.start,
            length: self.length,
            permissions: self.permissions,
        }
    }

    /// Returns a view of the same bytes with other permissions.
    pub fn protect(&self, permissions: MemoryPermissions) -> (protected: Self) {
        SharedAllocationMemoryShard {
            allocation: Rc::clone(&self.allocation),
            start: self.start,
            length: self.length,
            permissions,
        }
    }

    /// Returns the views of the bytes before and from the offset, or `None` unless the offset lies strictly inside.
    pub fn split_at(&self, offset: u64) -> (halves: Option<(Self, Self)>) {
        if offset == 0 || offset >= self.length {
            return None;
        }
        let first = SharedAllocationMemoryShard {
            allocation: Rc::clone(&self.allocation),
            start: self.start,
            length: offset,
            permissions: self.permissions,
        };
        let second = SharedAllocationMemoryShard {
            allocation: Rc::clone(&self.allocation),
            start: self.start + offset,
            length: self.length - offset,
            permissions: self.permissions,
        };
        Some((first, second))
    }
}

impl MemoryShard for SharedAllocationMemoryShard {
    type ByteView<'shard> = SharedBytes<'shard>;

    fn len(&self) -> (length: u64) {
        self.length
    }

    fn permissions(&self) -> (permissions: MemoryPermissions) {
        self.permissions
    }

    fn borrow(&self, offset: u64, length: u64) -> (borrowed: Result<SharedBytes<'_>, BorrowError>) {
        if offset > self.length || length > self.length - offset {
            return Err(BorrowError::OutsideShard);
        }
        Ok(SharedBytes { allocation: &self.allocation, start: self.start + offset, length })
    }
}

/// Borrowed bytes of an allocation, which another view may be changing while they are held.
pub struct SharedBytes<'shard> {
    allocation: &'shard Allocation,
    start: u64,
    length: u64,
}

// Returns how many bytes a copy at the offset moves for a buffer of the length.
fn count_copied(length: u64, offset: u64, buffer: usize) -> (count: usize) {
    if offset >= length {
        return 0;
    }
    let rest = length - offset;
    if buffer as u64 <= rest {
        buffer
    } else {
        rest as usize
    }
}

impl VolatileBytes for SharedBytes<'_> {
    fn len(&self) -> (length: u64) {
        self.length
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> (count: usize) {
        let count = count_copied(self.length, offset, into.len());
        let first = (self.start + offset) as usize;
        let mut index: usize = 0;
        while index < count {
            into[index] = self.allocation.bytes[first + index].get();
            index += 1;
        }
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> (count: usize) {
        let count = count_copied(self.length, offset, bytes.len());
        let first = (self.start + offset) as usize;
        let mut index: usize = 0;
        while index < count {
            self.allocation.bytes[first + index].set(bytes[index]);
            index += 1;
        }
        count
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> (read: Result<usize, Sink::Error>) {
        let mut copy: Vec<u8> = Vec::new();
        let first = self.start as usize;
        let mut index: usize = 0;
        while index < self.length as usize {
            copy.push(self.allocation.bytes[first + index].get());
            index += 1;
        }
        sink.write(copy.as_slice())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> (written: Result<usize, Source::Error>) {
        let mut copy: Vec<u8> = Vec::new();
        while copy.len() < self.length as usize {
            copy.push(0);
        }
        let read = source.read(copy.as_mut_slice());
        if let Ok(count) = read {
            let first = self.start as usize;
            let mut index: usize = 0;
            while index < count {
                self.allocation.bytes[first + index].set(copy[index]);
                index += 1;
            }
        }
        read
    }
}

} // verus!
