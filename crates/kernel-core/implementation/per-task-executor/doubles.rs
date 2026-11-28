//! A test table of sparse shards, which stands in for the proven ones being built on another branch.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_capability_table::{CapabilityTable, CreateError, DuplicateError};
use wcontainers_memory_capability_table::{
    MemoryCapabilityTable, MemoryError, SetPermissionsError, SplitError,
};
use wcontainers_memory_shard::{BorrowError, MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

type Bytes = Rc<RefCell<BTreeMap<u64, u8>>>;

/// Shared bytes that hold zero wherever nothing was written, so a shard may be as long as the address space.
#[derive(Clone)]
pub struct SparseShard {
    bytes: Bytes,
    base: u64,
    length: u64,
    permissions: MemoryPermissions,
}

pub struct SparseBytes<'shard> {
    bytes: &'shard RefCell<BTreeMap<u64, u8>>,
    base: u64,
    length: u64,
}

impl SparseBytes<'_> {
    fn count_copied(&self, offset: u64, buffer: usize) -> usize {
        let left = self.length.saturating_sub(offset);
        usize::try_from(left).map_or(buffer, |left| left.min(buffer))
    }
}

impl VolatileBytes for SparseBytes<'_> {
    fn len(&self) -> u64 {
        self.length
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        let count = self.count_copied(offset, into.len());
        let bytes = self.bytes.borrow();
        for (address, byte) in (self.base + offset..).zip(into[..count].iter_mut()) {
            *byte = bytes.get(&address).copied().unwrap_or(u8::MIN);
        }
        count
    }

    fn write_at(&self, offset: u64, from: &[u8]) -> usize {
        let count = self.count_copied(offset, from.len());
        let mut bytes = self.bytes.borrow_mut();
        for (address, byte) in (self.base + offset..).zip(from[..count].iter()) {
            bytes.insert(address, *byte);
        }
        count
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut all = vec![u8::MIN; self.count_copied(u64::MIN, usize::MAX)];
        self.read_at(u64::MIN, &mut all);
        sink.write(&all)
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut all = vec![u8::MIN; self.count_copied(u64::MIN, usize::MAX)];
        let count = source.read(&mut all)?;
        self.write_at(u64::MIN, &all[..count]);
        Ok(count)
    }
}

impl MemoryShard for SparseShard {
    type ByteView<'shard> = SparseBytes<'shard>;

    fn len(&self) -> u64 {
        self.length
    }

    fn permissions(&self) -> MemoryPermissions {
        self.permissions
    }

    fn borrow(&self, offset: u64, length: u64) -> Result<SparseBytes<'_>, BorrowError> {
        match offset.checked_add(length) {
            Some(end) if end <= self.length => Ok(SparseBytes {
                bytes: &self.bytes,
                base: self.base + offset,
                length,
            }),
            _ => Err(BorrowError::OutsideShard),
        }
    }
}

#[derive(Default)]
pub struct SparseTable {
    shards: BTreeMap<u64, SparseShard>,
}

impl CapabilityTable<u64, SparseShard, MemoryShardConfig, MemoryError> for SparseTable {
    fn create(
        &mut self,
        identifier: u64,
        config: MemoryShardConfig,
    ) -> Result<(), CreateError<MemoryError>> {
        if self.shards.contains_key(&identifier) {
            return Err(CreateError::IdentifierTaken);
        }
        let shard = SparseShard {
            bytes: Bytes::default(),
            base: u64::MIN,
            length: config.length,
            permissions: config.permissions,
        };
        self.shards.insert(identifier, shard);
        Ok(())
    }

    fn get(&self, identifier: &u64) -> Option<&SparseShard> {
        self.shards.get(identifier)
    }

    fn drop(&mut self, identifier: &u64) {
        self.shards.remove(identifier);
    }

    fn duplicate_within(
        &mut self,
        identifier: &u64,
        new_identifier: u64,
    ) -> Result<(), DuplicateError<MemoryError>> {
        let shard = self
            .shards
            .get(identifier)
            .cloned()
            .ok_or(DuplicateError::NothingHeld)?;
        self.insert_duplicate(shard, new_identifier)
    }

    fn duplicate_from(
        &mut self,
        other: &Self,
        identifier: &u64,
        new_identifier: u64,
    ) -> Result<(), DuplicateError<MemoryError>> {
        let shard = other
            .shards
            .get(identifier)
            .cloned()
            .ok_or(DuplicateError::NothingHeld)?;
        self.insert_duplicate(shard, new_identifier)
    }
}

impl SparseTable {
    fn insert_duplicate(
        &mut self,
        shard: SparseShard,
        new_identifier: u64,
    ) -> Result<(), DuplicateError<MemoryError>> {
        if self.shards.contains_key(&new_identifier) {
            return Err(DuplicateError::IdentifierTaken);
        }
        self.shards.insert(new_identifier, shard);
        Ok(())
    }
}

impl MemoryCapabilityTable<SparseShard> for SparseTable {
    fn set_permissions(
        &mut self,
        address: &u64,
        permissions: MemoryPermissions,
    ) -> Result<(), SetPermissionsError> {
        let shard = self
            .shards
            .get_mut(address)
            .ok_or(SetPermissionsError::NothingHeld)?;
        shard.permissions = permissions;
        Ok(())
    }

    fn split(&mut self, address: &u64, offset: u64) -> Result<(), SplitError> {
        let shard = self
            .shards
            .get_mut(address)
            .ok_or(SplitError::NothingHeld)?;
        if offset == u64::MIN || offset >= shard.length {
            return Err(SplitError::OutsideShard);
        }
        let second = SparseShard {
            bytes: Rc::clone(&shard.bytes),
            base: shard.base + offset,
            length: shard.length - offset,
            permissions: shard.permissions,
        };
        shard.length = offset;
        self.shards.insert(address.wrapping_add(offset), second);
        Ok(())
    }
}
