//! A memory shard as a view of part of an allocation of bytes that every shard viewing the allocation shares.

#![no_std]

extern crate alloc;

use alloc::rc::Rc;
use alloc::vec::Vec;

use vstd::cell::invcell::InvCell;
use vstd::predicate::Predicate;
use vstd::prelude::*;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_memory_shard::{
    BorrowError, MemoryPermissions, MemoryShard, MemoryShardConfig,
};
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

/// The most bytes one allocation holds.
pub const MOST_BYTES: u64 = 1 << 30;

/// One byte of an allocation, in a cell that a shared borrow may write.
pub type Byte = InvCell<u8, AnyByte>;

/// The rule every byte of an allocation meets, which any byte does.
pub struct AnyByte;

impl Predicate<u8> for AnyByte {
    open spec fn predicate(&self, byte: u8) -> bool {
        true
    }
}

// Returns `length` bytes, every one zero.
fn allocate(length: u64) -> (bytes: Vec<Byte>)
    requires
        length <= MOST_BYTES,
    ensures
        bytes@.len() == length,
{
    let mut bytes: Vec<Byte> = Vec::new();
    let mut made: u64 = u64::MIN;
    while made < length
        invariant
            made <= length,
            bytes@.len() == made,
        decreases length - made,
    {
        bytes.push(InvCell::new(u8::MIN, Ghost(AnyByte)));
        made += 1;
    }
    bytes
}

/// A view of `length` bytes of an allocation, `start` bytes in, with what a task may do with them.
pub struct SharedBytesMemoryShard {
    bytes: Rc<Vec<Byte>>,
    start: u64,
    length: u64,
    permissions: MemoryPermissions,
    allocation: Ghost<int>,
}

impl SharedBytesMemoryShard {
    #[verifier::type_invariant]
    spec fn fits(self) -> bool {
        self.start + self.length <= self.bytes.len()
    }

    /// Returns a shard of the config's length and permissions over a new allocation of zeros, or nothing past `MOST_BYTES`.
    pub fn new(config: MemoryShardConfig, Ghost(allocation): Ghost<int>) -> (made: Option<Self>)
        ensures
            made is None <==> config.length > MOST_BYTES,
            made matches Some(shard) ==> {
                &&& shard.spec_len() == config.length
                &&& shard.spec_permissions() == config.permissions
                &&& shard.spec_viewed_allocation() == wcontainers_memory_shard::MemoryAllocationIdentity(allocation)
                &&& shard.spec_view_start() == 0
            },
    {
        if config.length > MOST_BYTES {
            return None;
        }
        let bytes = Rc::new(allocate(config.length));
        Some(
            SharedBytesMemoryShard {
                bytes,
                start: u64::MIN,
                length: config.length,
                permissions: config.permissions,
                allocation: Ghost(allocation),
            },
        )
    }

    /// Returns a second shard that views the same bytes with the same permissions.
    pub fn duplicate(&self) -> (duplicate: Self)
        ensures
            duplicate == *self,
    {
        proof {
            use_type_invariant(self);
        }
        SharedBytesMemoryShard {
            bytes: Rc::clone(&self.bytes),
            start: self.start,
            length: self.length,
            permissions: self.permissions,
            allocation: Ghost(self.allocation@),
        }
    }

    /// Returns a shard that views the same bytes with the given permissions.
    pub fn with_permissions(&self, permissions: MemoryPermissions) -> (changed: Self)
        ensures
            changed.spec_len() == self.spec_len(),
            changed.spec_viewed_allocation() == self.spec_viewed_allocation(),
            changed.spec_view_start() == self.spec_view_start(),
            changed.spec_permissions() == permissions,
    {
        proof {
            use_type_invariant(self);
        }
        SharedBytesMemoryShard {
            bytes: Rc::clone(&self.bytes),
            start: self.start,
            length: self.length,
            permissions,
            allocation: Ghost(self.allocation@),
        }
    }

    /// Returns the shards that view the bytes before and from the offset, with this shard's permissions.
    pub fn split_at(&self, offset: u64) -> (halves: (Self, Self))
        requires
            0 < offset < self.spec_len(),
        ensures
            halves.0.spec_len() == offset,
            halves.0.spec_viewed_allocation() == self.spec_viewed_allocation(),
            halves.0.spec_view_start() == self.spec_view_start(),
            halves.0.spec_permissions() == self.spec_permissions(),
            halves.1.spec_len() == self.spec_len() - offset,
            halves.1.spec_viewed_allocation() == self.spec_viewed_allocation(),
            halves.1.spec_view_start() == self.spec_view_start() + offset,
            halves.1.spec_permissions() == self.spec_permissions(),
    {
        proof {
            use_type_invariant(self);
        }
        let first = SharedBytesMemoryShard {
            bytes: Rc::clone(&self.bytes),
            start: self.start,
            length: offset,
            permissions: self.permissions,
            allocation: Ghost(self.allocation@),
        };
        let second = SharedBytesMemoryShard {
            bytes: Rc::clone(&self.bytes),
            start: self.start + offset,
            length: self.length - offset,
            permissions: self.permissions,
            allocation: Ghost(self.allocation@),
        };
        (first, second)
    }
}

impl MemoryShard for SharedBytesMemoryShard {
    type ByteView<'shard> = SharedBytesView<'shard>;

    closed spec fn spec_len(&self) -> u64 {
        self.length
    }

    fn len(&self) -> (length: u64) {
        self.length
    }

    closed spec fn spec_viewed_allocation(&self) -> wcontainers_memory_shard::MemoryAllocationIdentity {
        wcontainers_memory_shard::MemoryAllocationIdentity(self.allocation@)
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

    fn borrow(&self, offset: u64, length: u64) -> (borrowed: Result<SharedBytesView<'_>, BorrowError>) {
        proof {
            use_type_invariant(self);
        }
        if offset > self.length || length > self.length - offset {
            return Err(BorrowError::OutsideShard);
        }
        Ok(SharedBytesView { bytes: &*self.bytes, start: self.start + offset, length })
    }
}

/// Bytes of an allocation borrowed from a shard.
pub struct SharedBytesView<'shard> {
    bytes: &'shard Vec<Byte>,
    start: u64,
    length: u64,
}

impl<'shard> SharedBytesView<'shard> {
    #[verifier::type_invariant]
    spec fn fits(self) -> bool {
        self.start + self.length <= self.bytes.len()
    }
}

// Returns where in the allocation a copy at the offset starts and how many bytes it moves for a buffer of the length.
fn locate(view: &SharedBytesView, offset: u64, buffer: usize) -> (range: (usize, usize))
    ensures
        range.1 == wcontainers_volatile_bytes::copied(view.spec_len(), offset, buffer as nat),
        range.0 + range.1 <= view.bytes.len(),
        range.1 <= buffer,
{
    proof {
        use_type_invariant(view);
    }
    if offset >= view.length {
        return (usize::MIN, usize::MIN);
    }
    let start = (view.start + offset) as usize;
    let rest = view.length - offset;
    if buffer as u64 <= rest {
        (start, buffer)
    } else {
        (start, rest as usize)
    }
}

impl<'shard> VolatileBytes for SharedBytesView<'shard> {
    closed spec fn spec_len(&self) -> nat {
        self.length as nat
    }

    fn len(&self) -> (length: u64) {
        self.length
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> (count: usize) {
        let (start, count) = locate(self, offset, into.len());
        let mut index: usize = usize::MIN;
        while index < count
            invariant
                count == wcontainers_volatile_bytes::copied(self.spec_len(), offset, old(into)@.len()),
                start + count <= self.bytes.len(),
                into@.len() == old(into)@.len(),
                index <= count,
                count <= into@.len(),
                forall|later: int| count <= later < old(into)@.len() ==> into@[later] == old(into)@[later],
            decreases count - index,
        {
            into[index] = self.bytes[start + index].get();
            index += 1;
        }
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> (count: usize) {
        let (start, count) = locate(self, offset, bytes.len());
        let mut index: usize = usize::MIN;
        while index < count
            invariant
                count == wcontainers_volatile_bytes::copied(self.spec_len(), offset, bytes@.len()),
                start + count <= self.bytes.len(),
                count <= bytes@.len(),
                index <= count,
            decreases count - index,
        {
            self.bytes[start + index].set(bytes[index]);
            index += 1;
        }
        count
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> (read: Result<usize, Sink::Error>) {
        let mut copy: Vec<u8> = Vec::new();
        let mut index: u64 = u64::MIN;
        proof {
            use_type_invariant(self);
        }
        while index < self.length
            invariant
                self.start + self.length <= self.bytes.len(),
                index <= self.length,
                copy@.len() == index,
            decreases self.length - index,
        {
            copy.push(self.bytes[(self.start + index) as usize].get());
            index += 1;
        }
        sink.write(copy.as_slice())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> (written: Result<usize, Source::Error>) {
        let mut copy: Vec<u8> = Vec::new();
        let mut made: u64 = u64::MIN;
        while made < self.length
            invariant
                made <= self.length,
                copy@.len() == made,
            decreases self.length - made,
        {
            copy.push(u8::MIN);
            made += 1;
        }
        let read = source.read(copy.as_mut_slice());
        if let Ok(count) = read {
            proof {
                use_type_invariant(self);
            }
            let mut index: usize = usize::MIN;
            while index < count
                invariant
                    count <= copy@.len(),
                    copy@.len() == self.length,
                    self.start + self.length <= self.bytes.len(),
                    index <= count,
                decreases count - index,
            {
                self.bytes[(self.start as usize) + index].set(copy[index]);
                index += 1;
            }
        }
        read
    }
}

} // verus!
