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
use wcontainers_memory_shard::{BorrowError, MemoryAllocationIdentity, MemoryPermissions, MemoryShard};
use wcontainers_volatile_bytes::{VolatileBytes, copied};

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

// The bytes of one allocation, each in a cell that a shared borrow may write, and the number that names it in proofs.
struct Allocation {
    bytes: Vec<InvCell<u8, AnyByte>>,
    identity: Ghost<int>,
}

// The rule every byte meets, which any byte does.
struct AnyByte;

impl Predicate<u8> for AnyByte {
    open spec fn predicate(&self, byte: u8) -> bool {
        true
    }
}

// Whether a view of `length` bytes from `start` lies inside an allocation no longer than the most bytes.
spec fn lies_inside(allocation: Allocation, start: u64, length: u64) -> bool {
    &&& start + length <= allocation.bytes@.len()
    &&& allocation.bytes@.len() <= MOST_BYTES
}

impl SharedAllocationMemoryShard {
    // The view lies inside its allocation.
    #[verifier::type_invariant]
    spec fn fits(self) -> bool {
        lies_inside(*self.allocation, self.start, self.length)
    }

    /// Returns a view of all of a new allocation of zero bytes, or `None` past the most bytes an allocation holds.
    pub fn allocate(length: u64, permissions: MemoryPermissions) -> (allocated: Option<Self>)
        ensures
            allocated is None <==> length > MOST_BYTES,
            allocated matches Some(shard) ==> shard.spec_len() == length && shard.spec_permissions()
                == permissions,
    {
        if length > MOST_BYTES {
            return None;
        }
        let mut bytes: Vec<InvCell<u8, AnyByte>> = Vec::new();
        let mut made: u64 = 0;
        while made < length
            invariant
                made <= length,
                length <= MOST_BYTES,
                bytes@.len() == made,
            decreases length - made,
        {
            bytes.push(InvCell::new(0, Ghost(AnyByte)));
            made += 1;
        }
        Some(SharedAllocationMemoryShard {
            allocation: Rc::new(Allocation { bytes, identity: Ghost(arbitrary()) }),
            start: 0,
            length,
            permissions,
        })
    }

    /// Returns a second view of the same bytes with the same permissions.
    pub fn duplicate(&self) -> (duplicate: Self)
        ensures
            duplicate == *self,
    {
        proof {
            use_type_invariant(self);
        }
        SharedAllocationMemoryShard {
            allocation: Rc::clone(&self.allocation),
            start: self.start,
            length: self.length,
            permissions: self.permissions,
        }
    }

    /// Returns a view of the same bytes with other permissions.
    pub fn protect(&self, permissions: MemoryPermissions) -> (protected: Self)
        ensures
            protected.spec_len() == self.spec_len(),
            protected.spec_viewed_allocation() == self.spec_viewed_allocation(),
            protected.spec_view_start() == self.spec_view_start(),
            protected.spec_permissions() == permissions,
    {
        proof {
            use_type_invariant(self);
        }
        SharedAllocationMemoryShard {
            allocation: Rc::clone(&self.allocation),
            start: self.start,
            length: self.length,
            permissions,
        }
    }

    /// Returns the views of the bytes before and from the offset, or `None` unless the offset lies strictly inside.
    pub fn split_at(&self, offset: u64) -> (halves: Option<(Self, Self)>)
        ensures
            halves is None <==> !(0 < offset < self.spec_len()),
            halves matches Some((first, second)) ==> {
                &&& first.spec_len() == offset
                &&& second.spec_len() == self.spec_len() - offset
                &&& first.spec_viewed_allocation() == self.spec_viewed_allocation()
                &&& second.spec_viewed_allocation() == self.spec_viewed_allocation()
                &&& first.spec_view_start() == self.spec_view_start()
                &&& second.spec_view_start() == self.spec_view_start() + offset
                &&& first.spec_permissions() == self.spec_permissions()
                &&& second.spec_permissions() == self.spec_permissions()
            },
    {
        proof {
            use_type_invariant(self);
        }
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

    closed spec fn spec_len(&self) -> u64 {
        self.length
    }

    fn len(&self) -> (length: u64) {
        self.length
    }

    closed spec fn spec_viewed_allocation(&self) -> MemoryAllocationIdentity {
        MemoryAllocationIdentity(self.allocation.identity@)
    }

    closed spec fn spec_view_start(&self) -> nat {
        self.start as nat
    }

    closed spec fn spec_permissions(&self) -> MemoryPermissions {
        self.permissions
    }

    fn permissions(&self) -> (permissions: MemoryPermissions) {
        self.permissions
    }

    fn borrow(&self, offset: u64, length: u64) -> (borrowed: Result<SharedBytes<'_>, BorrowError>) {
        proof {
            use_type_invariant(self);
        }
        if offset > self.length || length > self.length - offset {
            return Err(BorrowError::OutsideShard);
        }
        Ok(SharedBytes { allocation: &*self.allocation, start: self.start + offset, length })
    }
}

/// Borrowed bytes of an allocation, which another view may be changing while they are held.
pub struct SharedBytes<'shard> {
    allocation: &'shard Allocation,
    start: u64,
    length: u64,
}

impl<'shard> SharedBytes<'shard> {
    // The bytes lie inside their allocation.
    #[verifier::type_invariant]
    spec fn fits(self) -> bool {
        lies_inside(*self.allocation, self.start, self.length)
    }
}

// Returns how many bytes a copy at the offset moves for a buffer of the length.
fn count_copied(length: u64, offset: u64, buffer: usize) -> (count: usize)
    ensures
        count == copied(length as nat, offset, buffer as nat),
        count > 0 ==> offset + count <= length,
{
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
    closed spec fn spec_len(&self) -> nat {
        self.length as nat
    }

    fn len(&self) -> (length: u64) {
        self.length
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> (count: usize) {
        proof {
            use_type_invariant(self);
        }
        let count = count_copied(self.length, offset, into.len());
        if count == 0 {
            return 0;
        }
        let _ = self.allocation.bytes.len();
        let first = (self.start + offset) as usize;
        let mut index: usize = 0;
        while index < count
            invariant
                count == copied(self.spec_len(), offset, old(into)@.len()),
                first + count <= self.allocation.bytes@.len(),
                self.allocation.bytes@.len() <= usize::MAX,
                count <= old(into)@.len(),
                into@.len() == old(into)@.len(),
                index <= count,
                forall|later: int| count <= later < old(into)@.len() ==> into@[later] == old(into)@[later],
            decreases count - index,
        {
            into[index] = self.allocation.bytes[first + index].get();
            index += 1;
        }
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> (count: usize) {
        proof {
            use_type_invariant(self);
        }
        let count = count_copied(self.length, offset, bytes.len());
        if count == 0 {
            return 0;
        }
        let _ = self.allocation.bytes.len();
        let first = (self.start + offset) as usize;
        let mut index: usize = 0;
        while index < count
            invariant
                first + count <= self.allocation.bytes@.len(),
                self.allocation.bytes@.len() <= usize::MAX,
                count <= bytes@.len(),
                index <= count,
            decreases count - index,
        {
            self.allocation.bytes[first + index].set(bytes[index]);
            index += 1;
        }
        count
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> (read: Result<usize, Sink::Error>) {
        proof {
            use_type_invariant(self);
        }
        let mut copy: Vec<u8> = Vec::new();
        let _ = self.allocation.bytes.len();
        let first = self.start as usize;
        let length = self.length as usize;
        let mut index: usize = 0;
        while index < length
            invariant
                first + length <= self.allocation.bytes@.len(),
                self.allocation.bytes@.len() <= usize::MAX,
                length == self.spec_len(),
                index <= length,
                copy@.len() == index,
            decreases length - index,
        {
            copy.push(self.allocation.bytes[first + index].get());
            index += 1;
        }
        sink.write(copy.as_slice())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> (written: Result<usize, Source::Error>) {
        proof {
            use_type_invariant(self);
        }
        let _ = self.allocation.bytes.len();
        let length = self.length as usize;
        let mut copy: Vec<u8> = Vec::new();
        while copy.len() < length
            invariant
                copy@.len() <= length,
            decreases length - copy@.len(),
        {
            copy.push(0);
        }
        let read = source.read(copy.as_mut_slice());
        if let Ok(count) = read {
            let first = self.start as usize;
            let mut index: usize = 0;
            while index < count
                invariant
                    first + length <= self.allocation.bytes@.len(),
                    self.allocation.bytes@.len() <= usize::MAX,
                    count <= copy@.len(),
                    copy@.len() == length,
                    index <= count,
                decreases count - index,
            {
                self.allocation.bytes[first + index].set(copy[index]);
                index += 1;
            }
        }
        read
    }
}

} // verus!
