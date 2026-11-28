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
    held: Held,
}

// The list of entries, kept with the rule that no two share an address or overlap.
struct Held {
    entries: Vec<Entry>,
}

// One shard and the address it starts at.
struct Entry {
    address: u64,
    shard: SharedAllocationMemoryShard,
}

/// Whether the bytes from one address lie wholly before or wholly after those from another.
pub open spec fn apart(address: u64, length: u64, other_address: u64, other_length: u64) -> bool {
    address + length <= other_address || other_address + other_length <= address
}

// Whether no two entries share an address or overlap, and each ends inside the address space.
spec fn well_formed(entries: Seq<Entry>) -> bool {
    &&& forall|index: int|
        #![trigger entries[index]]
        0 <= index < entries.len() ==> entries[index].address + entries[index].shard.spec_len()
            <= u64::MAX + 1
    &&& forall|first: int, second: int|
        #![trigger entries[first], entries[second]]
        0 <= first < entries.len() && 0 <= second < entries.len() && first != second ==> {
            &&& entries[first].address != entries[second].address
            &&& apart(
                entries[first].address,
                entries[first].shard.spec_len(),
                entries[second].address,
                entries[second].shard.spec_len(),
            )
        }
}

// Whether an entry is held at the address.
spec fn holds(entries: Seq<Entry>, address: u64) -> bool {
    exists|index: int| 0 <= index < entries.len() && #[trigger] entries[index].address == address
}

// The shards of the entries under their addresses.
spec fn map_of(entries: Seq<Entry>) -> Map<u64, SharedAllocationMemoryShard> {
    Map::new(
        |address: u64| holds(entries, address),
        |address: u64|
            entries[choose|index: int|
                0 <= index < entries.len() && #[trigger] entries[index].address == address].shard,
    )
}

// The shards the entries hold, or none for entries that break the rule.
spec fn state_of(entries: Seq<Entry>) -> Map<u64, SharedAllocationMemoryShard> {
    if well_formed(entries) {
        map_of(entries)
    } else {
        Map::empty()
    }
}

// The entry at an index is the one the map holds under its address.
proof fn lemma_map_of_index(entries: Seq<Entry>, at: int)
    requires
        well_formed(entries),
        0 <= at < entries.len(),
    ensures
        map_of(entries).contains_key(entries[at].address),
        map_of(entries).index(entries[at].address) == entries[at].shard,
{
    let address = entries[at].address;
    assert(holds(entries, address));
    let index = choose|index: int|
        0 <= index < entries.len() && #[trigger] entries[index].address == address;
    if index != at {
        assert(entries[index].address != entries[at].address);
    }
}

// The map of the entries holds an address only where an entry starts.
proof fn lemma_map_of_holds(entries: Seq<Entry>, address: u64)
    requires
        well_formed(entries),
        map_of(entries).contains_key(address),
    ensures
        exists|index: int|
            0 <= index < entries.len() && #[trigger] entries[index].address == address
                && map_of(entries).index(address) == entries[index].shard,
{
    let index = choose|index: int| 0 <= index < entries.len() && #[trigger] entries[index].address == address;
    lemma_map_of_index(entries, index);
}

// Adding an entry that is apart from all others holds its shard under its address too.
proof fn lemma_push(entries: Seq<Entry>, entry: Entry)
    requires
        well_formed(entries),
        !holds(entries, entry.address),
        entry.address + entry.shard.spec_len() <= u64::MAX + 1,
        forall|index: int|
            0 <= index < entries.len() ==> apart(
                entry.address,
                entry.shard.spec_len(),
                #[trigger] entries[index].address,
                entries[index].shard.spec_len(),
            ),
    ensures
        well_formed(entries.push(entry)),
        map_of(entries.push(entry)) == map_of(entries).insert(entry.address, entry.shard),
{
    let pushed = entries.push(entry);
    assert forall|first: int, second: int|
        #![trigger pushed[first], pushed[second]]
        0 <= first < pushed.len() && 0 <= second < pushed.len() && first != second implies {
            &&& pushed[first].address != pushed[second].address
            &&& apart(
                pushed[first].address,
                pushed[first].shard.spec_len(),
                pushed[second].address,
                pushed[second].shard.spec_len(),
            )
        } by {
        if first < entries.len() && second < entries.len() {
            assert(entries[first] == pushed[first] && entries[second] == pushed[second]);
        } else if first < entries.len() {
            assert(entries[first] == pushed[first]);
            assert(entries[first].address != entry.address);
        } else {
            assert(entries[second] == pushed[second]);
            assert(entries[second].address != entry.address);
        }
    }
    assert(well_formed(pushed)) by {
        assert forall|index: int|
            #![trigger pushed[index]]
            0 <= index < pushed.len() implies pushed[index].address + pushed[index].shard.spec_len()
                <= u64::MAX + 1 by {
            if index < entries.len() {
                assert(entries[index] == pushed[index]);
            }
        }
    }
    let expected = map_of(entries).insert(entry.address, entry.shard);
    assert forall|address: u64| #[trigger] map_of(pushed).contains_key(address) <==> expected.contains_key(address) by {
        if holds(pushed, address) {
            let index = choose|index: int| 0 <= index < pushed.len() && #[trigger] pushed[index].address == address;
            if index < entries.len() {
                assert(entries[index] == pushed[index]);
                assert(entries[index].address == address);
                assert(holds(entries, address));
            }
        }
        if holds(entries, address) {
            let index = choose|index: int| 0 <= index < entries.len() && #[trigger] entries[index].address == address;
            assert(pushed[index] == entries[index]);
            assert(pushed[index].address == address);
        }
        if address == entry.address {
            assert(pushed[entries.len() as int] == entry);
            assert(pushed[entries.len() as int].address == address);
        }
    }
    assert forall|address: u64| #[trigger] map_of(pushed).contains_key(address) implies map_of(pushed).index(address)
        == expected.index(address) by {
        lemma_map_of_holds(pushed, address);
        let index = choose|index: int|
            0 <= index < pushed.len() && #[trigger] pushed[index].address == address
                && map_of(pushed).index(address) == pushed[index].shard;
        if index < entries.len() {
            assert(entries[index] == pushed[index]);
            lemma_map_of_index(entries, index);
        } else {
            assert(pushed[index] == entry);
        }
    }
    assert(map_of(pushed) =~= expected);
}

// Replacing an entry by one at the same address and no longer keeps the rule and holds the new shard there.
proof fn lemma_update(entries: Seq<Entry>, index: int, entry: Entry)
    requires
        well_formed(entries),
        0 <= index < entries.len(),
        entry.address == entries[index].address,
        entry.shard.spec_len() <= entries[index].shard.spec_len(),
    ensures
        well_formed(entries.update(index, entry)),
        map_of(entries.update(index, entry)) == map_of(entries).insert(entry.address, entry.shard),
{
    let updated = entries.update(index, entry);
    assert forall|first: int, second: int|
        #![trigger updated[first], updated[second]]
        0 <= first < updated.len() && 0 <= second < updated.len() && first != second implies {
            &&& updated[first].address != updated[second].address
            &&& apart(
                updated[first].address,
                updated[first].shard.spec_len(),
                updated[second].address,
                updated[second].shard.spec_len(),
            )
        } by {
        assert(entries[first].address != entries[second].address);
        assert(apart(
            entries[first].address,
            entries[first].shard.spec_len(),
            entries[second].address,
            entries[second].shard.spec_len(),
        ));
    }
    assert(well_formed(updated)) by {
        assert forall|later: int|
            #![trigger updated[later]]
            0 <= later < updated.len() implies updated[later].address + updated[later].shard.spec_len()
                <= u64::MAX + 1 by {
            assert(entries[later].address + entries[later].shard.spec_len() <= u64::MAX + 1);
        }
    }
    let expected = map_of(entries).insert(entry.address, entry.shard);
    assert forall|address: u64| #[trigger] map_of(updated).contains_key(address) <==> expected.contains_key(address) by {
        if holds(updated, address) {
            let found = choose|found: int| 0 <= found < updated.len() && #[trigger] updated[found].address == address;
            assert(entries[found].address == address);
            assert(holds(entries, address));
        }
        if holds(entries, address) {
            let found = choose|found: int| 0 <= found < entries.len() && #[trigger] entries[found].address == address;
            assert(updated[found].address == address);
            assert(holds(updated, address));
        }
        lemma_map_of_index(entries, index);
    }
    assert forall|address: u64| #[trigger] map_of(updated).contains_key(address) implies map_of(updated).index(address)
        == expected.index(address) by {
        lemma_map_of_holds(updated, address);
        let found = choose|found: int|
            0 <= found < updated.len() && #[trigger] updated[found].address == address
                && map_of(updated).index(address) == updated[found].shard;
        if found != index {
            assert(entries[found] == updated[found]);
            lemma_map_of_index(entries, found);
        }
    }
    assert(map_of(updated) =~= expected);
}

// Removing the entry at an index keeps the rule and holds nothing under its address.
proof fn lemma_remove(entries: Seq<Entry>, index: int)
    requires
        well_formed(entries),
        0 <= index < entries.len(),
    ensures
        well_formed(entries.remove(index)),
        map_of(entries.remove(index)) == map_of(entries).remove(entries[index].address),
{
    let removed = entries.remove(index);
    let gone = entries[index].address;
    // The index in the old list of each index in the new one.
    let before = |later: int| if later < index { later } else { later + 1 };
    assert forall|later: int| 0 <= later < removed.len() implies #[trigger] removed[later] == entries[before(later)] by {}
    assert forall|first: int, second: int|
        #![trigger removed[first], removed[second]]
        0 <= first < removed.len() && 0 <= second < removed.len() && first != second implies {
            &&& removed[first].address != removed[second].address
            &&& apart(
                removed[first].address,
                removed[first].shard.spec_len(),
                removed[second].address,
                removed[second].shard.spec_len(),
            )
        } by {
        assert(removed[first] == entries[before(first)]);
        assert(removed[second] == entries[before(second)]);
        assert(entries[before(first)].address != entries[before(second)].address);
    }
    assert(well_formed(removed)) by {
        assert forall|later: int|
            #![trigger removed[later]]
            0 <= later < removed.len() implies removed[later].address + removed[later].shard.spec_len()
                <= u64::MAX + 1 by {
            assert(removed[later] == entries[before(later)]);
            assert(entries[before(later)].address + entries[before(later)].shard.spec_len() <= u64::MAX + 1);
        }
    }
    let expected = map_of(entries).remove(gone);
    assert forall|address: u64| #[trigger] map_of(removed).contains_key(address) <==> expected.contains_key(address) by {
        if holds(removed, address) {
            let found = choose|found: int| 0 <= found < removed.len() && #[trigger] removed[found].address == address;
            assert(removed[found] == entries[before(found)]);
            assert(holds(entries, address));
            assert(entries[before(found)].address != entries[index].address);
        }
        if holds(entries, address) && address != gone {
            let found = choose|found: int| 0 <= found < entries.len() && #[trigger] entries[found].address == address;
            assert(found != index);
            let later = if found < index { found } else { found - 1 };
            assert(removed[later] == entries[before(later)]);
            assert(removed[later].address == address);
            assert(holds(removed, address));
        }
    }
    assert forall|address: u64| #[trigger] map_of(removed).contains_key(address) implies map_of(removed).index(address)
        == expected.index(address) by {
        lemma_map_of_holds(removed, address);
        let found = choose|found: int|
            0 <= found < removed.len() && #[trigger] removed[found].address == address
                && map_of(removed).index(address) == removed[found].shard;
        assert(removed[found] == entries[before(found)]);
        lemma_map_of_index(entries, before(found));
    }
    assert(map_of(removed) =~= expected);
}

impl Held {
    // No two entries share an address or overlap.
    #[verifier::type_invariant]
    spec fn ruled(self) -> bool {
        well_formed(self.entries@)
    }
}

// Returns the entries, leaving none in their place, so that the list can change under no borrow of a field.
fn take(held: &mut Held) -> (entries: Vec<Entry>)
    ensures
        entries@ == old(held).entries@,
        well_formed(entries@),
{
    proof {
        use_type_invariant(&*held);
    }
    let mut taken = Held { entries: Vec::new() };
    core::mem::swap(held, &mut taken);
    proof {
        use_type_invariant(&taken);
    }
    taken.entries
}

// Returns the held entries of a list that keeps the rule.
fn wrap(entries: Vec<Entry>) -> (held: Held)
    requires
        well_formed(entries@),
    ensures
        held.entries@ == entries@,
{
    Held { entries }
}

// Returns whether the bytes from one address and those from another overlap.
fn overlaps(address: u64, length: u64, other_address: u64, other_length: u64) -> (overlap: bool)
    ensures
        overlap == !apart(address, length, other_address, other_length),
{
    !((address as u128) + (length as u128) <= other_address as u128 || (other_address as u128)
        + (other_length as u128) <= address as u128)
}

/// Every two shards a table holds lie apart.
pub proof fn lemma_shards_are_disjoint(table: ListMemoryCapabilityTable)
    ensures
        ({
            let shards = table.state();
            forall|first: u64, second: u64|
                #![trigger shards.contains_key(first), shards.contains_key(second)]
                shards.contains_key(first) && shards.contains_key(second) && first != second ==> first
                    + shards.index(first).spec_len() <= second || second + shards.index(second).spec_len()
                    <= first
        }),
{
    let entries = table.held.entries@;
    if well_formed(entries) {
        let shards = map_of(entries);
        assert forall|first: u64, second: u64|
            #![trigger shards.contains_key(first), shards.contains_key(second)]
            shards.contains_key(first) && shards.contains_key(second) && first != second implies first
                + shards.index(first).spec_len() <= second || second + shards.index(second).spec_len()
                <= first by {
            lemma_map_of_holds(entries, first);
            lemma_map_of_holds(entries, second);
            let one = choose|index: int|
                0 <= index < entries.len() && #[trigger] entries[index].address == first
                    && shards.index(first) == entries[index].shard;
            let other = choose|index: int|
                0 <= index < entries.len() && #[trigger] entries[index].address == second
                    && shards.index(second) == entries[index].shard;
            assert(apart(
                entries[one].address,
                entries[one].shard.spec_len(),
                entries[other].address,
                entries[other].shard.spec_len(),
            ));
        }
    }
}

impl ListMemoryCapabilityTable {
    /// Returns a table that holds no memory.
    pub fn new() -> (table: Self)
        ensures
            table.state() == Map::<u64, SharedAllocationMemoryShard>::empty(),
    {
        let table = ListMemoryCapabilityTable { held: wrap(Vec::new()) };
        assert(map_of(Seq::<Entry>::empty()) =~= Map::<u64, SharedAllocationMemoryShard>::empty());
        table
    }

    // Returns where in the list the shard at the address is.
    fn find(&self, address: u64) -> (found: Option<usize>)
        ensures
            match found {
                Some(index) => index < self.held.entries@.len() && self.held.entries@[index as int].address
                    == address,
                None => !holds(self.held.entries@, address),
            },
    {
        let entries = &self.held.entries;
        let mut index: usize = 0;
        while index < entries.len()
            invariant
                index <= entries@.len(),
                entries@ == self.held.entries@,
                forall|earlier: int| 0 <= earlier < index ==> #[trigger] entries@[earlier].address != address,
            decreases entries@.len() - index,
        {
            if entries[index].address == address {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    // Returns whether bytes from the address would overlap a shard the table holds.
    fn overlaps_any(&self, address: u64, length: u64) -> (overlap: bool)
        ensures
            !overlap ==> forall|index: int|
                0 <= index < self.held.entries@.len() ==> apart(
                    address,
                    length,
                    #[trigger] self.held.entries@[index].address,
                    self.held.entries@[index].shard.spec_len(),
                ),
    {
        let entries = &self.held.entries;
        let mut index: usize = 0;
        while index < entries.len()
            invariant
                index <= entries@.len(),
                entries@ == self.held.entries@,
                forall|earlier: int|
                    0 <= earlier < index ==> apart(
                        address,
                        length,
                        #[trigger] entries@[earlier].address,
                        entries@[earlier].shard.spec_len(),
                    ),
            decreases entries@.len() - index,
        {
            let entry = &entries[index];
            if overlaps(address, length, entry.address, entry.shard.len()) {
                return true;
            }
            index += 1;
        }
        false
    }

    // Holds the shard at a free address, unless it would overlap a held shard or run past the last address.
    fn insert(&mut self, address: u64, shard: SharedAllocationMemoryShard) -> (inserted: Result<(), MemoryError>)
        requires
            !old(self).state().contains_key(address),
        ensures
            match inserted {
                Ok(()) => final(self).state() == old(self).state().insert(address, shard),
                Err(_) => final(self).state() == old(self).state(),
            },
    {
        proof {
            use_type_invariant(&self.held);
        }
        if shard.len() as u128 > (u64::MAX as u128) + 1 - (address as u128) {
            return Err(MemoryError::OutOfMemory);
        }
        if self.overlaps_any(address, shard.len()) {
            return Err(MemoryError::Overlap);
        }
        let ghost before = self.held.entries@;
        let mut entries = take(&mut self.held);
        let entry = Entry { address, shard };
        proof {
            lemma_push(before, entry);
        }
        entries.push(entry);
        self.held = wrap(entries);
        Ok(())
    }
}

impl CapabilityTable<u64, SharedAllocationMemoryShard, MemoryShardConfig, MemoryError> for ListMemoryCapabilityTable {
    closed spec fn state(&self) -> Map<u64, SharedAllocationMemoryShard> {
        state_of(self.held.entries@)
    }

    fn create(&mut self, identifier: u64, config: MemoryShardConfig) -> (created: Result<(), CreateError<MemoryError>>) {
        proof {
            use_type_invariant(&self.held);
        }
        if let Some(index) = self.find(identifier) {
            proof {
                lemma_map_of_index(self.held.entries@, index as int);
            }
            return Err(CreateError::IdentifierTaken);
        }
        let shard = match SharedAllocationMemoryShard::allocate(config.length, config.permissions) {
            Some(shard) => shard,
            None => return Err(CreateError::Failed(MemoryError::OutOfMemory)),
        };
        match self.insert(identifier, shard) {
            Ok(()) => {
                assert(self.state().remove(identifier) =~= old(self).state());
                Ok(())
            },
            Err(error) => Err(CreateError::Failed(error)),
        }
    }

    fn get(&self, identifier: &u64) -> (capability: Option<&SharedAllocationMemoryShard>) {
        proof {
            use_type_invariant(&self.held);
        }
        match self.find(*identifier) {
            Some(index) => {
                proof {
                    lemma_map_of_index(self.held.entries@, index as int);
                }
                Some(&self.held.entries[index].shard)
            },
            None => None,
        }
    }

    fn drop(&mut self, identifier: &u64) {
        proof {
            use_type_invariant(&self.held);
        }
        if let Some(index) = self.find(*identifier) {
            let ghost before = self.held.entries@;
            let mut entries = take(&mut self.held);
            proof {
                lemma_remove(before, index as int);
            }
            entries.remove(index);
            self.held = wrap(entries);
        } else {
            assert(old(self).state().remove(*identifier) =~= old(self).state());
        }
    }

    fn duplicate_within(&mut self, identifier: &u64, new_identifier: u64) -> (duplicated: Result<(), DuplicateError<MemoryError>>) {
        proof {
            use_type_invariant(&self.held);
        }
        let duplicate = match self.find(*identifier) {
            Some(index) => {
                proof {
                    lemma_map_of_index(self.held.entries@, index as int);
                }
                self.held.entries[index].shard.duplicate()
            },
            None => return Err(DuplicateError::NothingHeld),
        };
        if let Some(index) = self.find(new_identifier) {
            proof {
                lemma_map_of_index(self.held.entries@, index as int);
            }
            return Err(DuplicateError::IdentifierTaken);
        }
        match self.insert(new_identifier, duplicate) {
            Ok(()) => Ok(()),
            Err(error) => Err(DuplicateError::Failed(error)),
        }
    }

    fn duplicate_from(&mut self, other: &Self, identifier: &u64, new_identifier: u64) -> (duplicated: Result<(), DuplicateError<MemoryError>>) {
        proof {
            use_type_invariant(&self.held);
            use_type_invariant(&other.held);
        }
        let duplicate = match other.find(*identifier) {
            Some(index) => {
                proof {
                    lemma_map_of_index(other.held.entries@, index as int);
                }
                other.held.entries[index].shard.duplicate()
            },
            None => return Err(DuplicateError::NothingHeld),
        };
        if let Some(index) = self.find(new_identifier) {
            proof {
                lemma_map_of_index(self.held.entries@, index as int);
            }
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
        proof {
            use_type_invariant(&self.held);
        }
        let index = match self.find(*address) {
            Some(index) => index,
            None => return Err(SetPermissionsError::NothingHeld),
        };
        proof {
            lemma_map_of_index(self.held.entries@, index as int);
        }
        let protected = self.held.entries[index].shard.protect(permissions);
        let ghost before = self.held.entries@;
        let mut entries = take(&mut self.held);
        let entry = Entry { address: *address, shard: protected };
        proof {
            lemma_update(before, index as int, entry);
        }
        entries[index] = entry;
        self.held = wrap(entries);
        assert(self.state().remove(*address) =~= old(self).state().remove(*address));
        Ok(())
    }

    fn split(&mut self, address: &u64, offset: u64) -> (split: Result<(), SplitError>) {
        proof {
            use_type_invariant(&self.held);
        }
        let index = match self.find(*address) {
            Some(index) => index,
            None => return Err(SplitError::NothingHeld),
        };
        proof {
            lemma_map_of_index(self.held.entries@, index as int);
        }
        let (first, second) = match self.held.entries[index].shard.split_at(offset) {
            Some(halves) => halves,
            None => return Err(SplitError::OutsideShard),
        };
        let ghost before = self.held.entries@;
        let mut entries = take(&mut self.held);
        let first_entry = Entry { address: *address, shard: first };
        proof {
            lemma_update(before, index as int, first_entry);
        }
        entries[index] = first_entry;
        let ghost updated = entries@;
        let second_address = *address + offset;
        let second_entry = Entry { address: second_address, shard: second };
        proof {
            assert forall|later: int| 0 <= later < updated.len() implies apart(
                second_entry.address,
                second_entry.shard.spec_len(),
                #[trigger] updated[later].address,
                updated[later].shard.spec_len(),
            ) && updated[later].address != second_address by {
                if later != index as int {
                    assert(updated[later] == before[later]);
                    assert(apart(
                        before[index as int].address,
                        before[index as int].shard.spec_len(),
                        before[later].address,
                        before[later].shard.spec_len(),
                    ));
                }
            }
            lemma_push(updated, second_entry);
        }
        entries.push(second_entry);
        self.held = wrap(entries);
        assert(self.state().remove(*address).remove(second_address) =~= old(self).state().remove(*address));
        Ok(())
    }
}

} // verus!
