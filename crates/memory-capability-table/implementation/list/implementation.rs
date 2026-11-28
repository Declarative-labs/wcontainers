//! A memory capability table as a list of shards under their starting addresses, none of which overlap.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use vstd::prelude::*;
use wcontainers_capability_table::{CapabilityTable, CreateError, DuplicateError};
use wcontainers_memory_capability_table::{MemoryCapabilityTable, MemoryError, SetPermissionsError, SplitError};
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_memory_shard_shared_allocation::SharedAllocationMemoryShard;

verus! {

/// The shards a task holds, each under the virtual address it starts at.
pub struct ListMemoryCapabilityTable {
    entries: Vec<Entry>,
}

// One shard and the address it starts at.
struct Entry {
    address: u64,
    shard: SharedAllocationMemoryShard,
}

// Returns whether the bytes from one address and those from another overlap.
fn overlaps(address: u64, length: u64, other_address: u64, other_length: u64) -> (overlap: bool) {
    !((address as u128) + (length as u128) <= other_address as u128 || (other_address as u128)
        + (other_length as u128) <= address as u128)
}

impl ListMemoryCapabilityTable {
    /// Returns a table that holds no memory.
    pub fn new() -> (table: Self) {
        ListMemoryCapabilityTable { entries: Vec::new() }
    }

    // Returns where in the list the shard at the address is.
    fn find(&self, address: u64) -> (found: Option<usize>) {
        let mut index: usize = 0;
        while index < self.entries.len() {
            if self.entries[index].address == address {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    // Returns whether bytes from the address would overlap a shard the table holds.
    fn overlaps_any(&self, address: u64, length: u64) -> (overlap: bool) {
        let mut index: usize = 0;
        while index < self.entries.len() {
            let entry = &self.entries[index];
            if overlaps(address, length, entry.address, entry.shard.len()) {
                return true;
            }
            index += 1;
        }
        false
    }

    // Holds the shard at the address, unless it would overlap a held shard or run past the last address.
    fn insert(&mut self, address: u64, shard: SharedAllocationMemoryShard) -> (inserted: Result<(), MemoryError>) {
        if shard.len() as u128 > (u64::MAX as u128) + 1 - (address as u128) {
            return Err(MemoryError::OutOfMemory);
        }
        if self.overlaps_any(address, shard.len()) {
            return Err(MemoryError::Overlap);
        }
        self.entries.push(Entry { address, shard });
        Ok(())
    }
}

impl CapabilityTable<u64, SharedAllocationMemoryShard, MemoryShardConfig, MemoryError> for ListMemoryCapabilityTable {
    fn create(&mut self, identifier: u64, config: MemoryShardConfig) -> (created: Result<(), CreateError<MemoryError>>) {
        if self.find(identifier).is_some() {
            return Err(CreateError::IdentifierTaken);
        }
        let shard = match SharedAllocationMemoryShard::allocate(config.length, config.permissions) {
            Some(shard) => shard,
            None => return Err(CreateError::Failed(MemoryError::OutOfMemory)),
        };
        match self.insert(identifier, shard) {
            Ok(()) => Ok(()),
            Err(error) => Err(CreateError::Failed(error)),
        }
    }

    fn get(&self, identifier: &u64) -> (capability: Option<&SharedAllocationMemoryShard>) {
        match self.find(*identifier) {
            Some(index) => Some(&self.entries[index].shard),
            None => None,
        }
    }

    fn drop(&mut self, identifier: &u64) {
        if let Some(index) = self.find(*identifier) {
            self.entries.remove(index);
        }
    }

    fn duplicate_within(&mut self, identifier: &u64, new_identifier: u64) -> (duplicated: Result<(), DuplicateError<MemoryError>>) {
        let duplicate = match self.find(*identifier) {
            Some(index) => self.entries[index].shard.duplicate(),
            None => return Err(DuplicateError::NothingHeld),
        };
        if self.find(new_identifier).is_some() {
            return Err(DuplicateError::IdentifierTaken);
        }
        match self.insert(new_identifier, duplicate) {
            Ok(()) => Ok(()),
            Err(error) => Err(DuplicateError::Failed(error)),
        }
    }

    fn duplicate_from(&mut self, other: &Self, identifier: &u64, new_identifier: u64) -> (duplicated: Result<(), DuplicateError<MemoryError>>) {
        let duplicate = match other.find(*identifier) {
            Some(index) => other.entries[index].shard.duplicate(),
            None => return Err(DuplicateError::NothingHeld),
        };
        if self.find(new_identifier).is_some() {
            return Err(DuplicateError::IdentifierTaken);
        }
        match self.insert(new_identifier, duplicate) {
            Ok(()) => Ok(()),
            Err(error) => Err(DuplicateError::Failed(error)),
        }
    }
}

impl MemoryCapabilityTable<SharedAllocationMemoryShard> for ListMemoryCapabilityTable {
    fn set_permissions(&mut self, address: &u64, permissions: MemoryPermissions) -> (set: Result<(), SetPermissionsError>) {
        let index = match self.find(*address) {
            Some(index) => index,
            None => return Err(SetPermissionsError::NothingHeld),
        };
        let protected = self.entries[index].shard.protect(permissions);
        self.entries[index] = Entry { address: *address, shard: protected };
        Ok(())
    }

    fn split(&mut self, address: &u64, offset: u64) -> (split: Result<(), SplitError>) {
        let index = match self.find(*address) {
            Some(index) => index,
            None => return Err(SplitError::NothingHeld),
        };
        let (first, second) = match self.entries[index].shard.split_at(offset) {
            Some(halves) => halves,
            None => return Err(SplitError::OutsideShard),
        };
        self.entries[index] = Entry { address: *address, shard: first };
        self.entries.push(Entry { address: *address + offset, shard: second });
        Ok(())
    }
}

} // verus!
