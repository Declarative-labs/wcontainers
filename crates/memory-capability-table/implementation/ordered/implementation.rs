//! A memory capability table in the standard library's ordered map from each starting address to a shard, beside a list of the addresses that it searches for overlaps.

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use vstd::prelude::*;
use wcontainers_capability_table::{CapabilityTable, CreateError, DuplicateError};
use wcontainers_memory_capability_table::{
    MemoryCapabilityTable, MemoryError, SetPermissionsError, SplitError,
};
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_memory_shard_shared_bytes::SharedBytesMemoryShard;

verus! {

broadcast use vstd::std_specs::btree::group_btree_axioms;

/// Whether no two shards held under different addresses share an address.
pub open spec fn disjoint(shards: Map<u64, SharedBytesMemoryShard>) -> bool {
    forall|first: u64, second: u64|
        #![trigger shards.contains_key(first), shards.contains_key(second)]
        shards.contains_key(first) && shards.contains_key(second) && first != second ==> first
            + shards.index(first).spec_len() <= second || second + shards.index(second).spec_len()
            <= first
}

/// Whether every shard ends at or before the last address.
pub open spec fn addressable(shards: Map<u64, SharedBytesMemoryShard>) -> bool {
    forall|address: u64| #[trigger]
        shards.contains_key(address) ==> address + shards.index(address).spec_len() <= u64::MAX
}

// Whether a shard of the length at the address shares no address with a held shard.
spec fn free(shards: Map<u64, SharedBytesMemoryShard>, address: u64, length: u64) -> bool {
    forall|held: u64| #[trigger]
        shards.contains_key(held) ==> held + shards.index(held).spec_len() <= address || address
            + length <= held
}

// The map of shards and the list of its addresses, kept with the laws the map's specification needs.
struct Entries {
    map: BTreeMap<u64, SharedBytesMemoryShard>,
    addresses: Vec<u64>,
}

impl Entries {
    // The map's order laws, the list holding each of its addresses once, and the shards apart and addressable.
    #[verifier::type_invariant]
    spec fn kept(self) -> bool {
        &&& vstd::laws_cmp::obeys_cmp::<u64>()
        &&& vstd::laws_cmp::obeys_cmp_concrete_eq::<u64>()
        &&& vstd::std_specs::btree::btree_map_inv(&self.map)
        &&& self.addresses@.no_duplicates()
        &&& forall|address: u64| #[trigger]
            self.addresses@.contains(address) <==> self.map@.contains_key(address)
        &&& disjoint(self.map@)
        &&& addressable(self.map@)
    }
}

// Returns the map and the list, leaving empty ones in their place.
fn take(entries: &mut Entries) -> (taken: (BTreeMap<u64, SharedBytesMemoryShard>, Vec<u64>))
    ensures
        taken.0@ == old(entries).map@,
        taken.1@ == old(entries).addresses@,
        old(entries).kept(),
        vstd::laws_cmp::obeys_cmp::<u64>(),
        vstd::laws_cmp::obeys_cmp_concrete_eq::<u64>(),
        vstd::std_specs::btree::btree_map_inv(&taken.0),
{
    proof {
        use_type_invariant(&*entries);
    }
    let mut taken = Entries { map: BTreeMap::new(), addresses: Vec::new() };
    core::mem::swap(entries, &mut taken);
    proof {
        use_type_invariant(&taken);
    }
    (taken.map, taken.addresses)
}

// Returns the entries of a map and its list of addresses.
fn wrap(map: BTreeMap<u64, SharedBytesMemoryShard>, addresses: Vec<u64>) -> (entries: Entries)
    requires
        vstd::laws_cmp::obeys_cmp::<u64>(),
        vstd::laws_cmp::obeys_cmp_concrete_eq::<u64>(),
        vstd::std_specs::btree::btree_map_inv(&map),
        addresses@.no_duplicates(),
        forall|address: u64| #[trigger] addresses@.contains(address) <==> map@.contains_key(address),
        disjoint(map@),
        addressable(map@),
    ensures
        entries.map@ == map@,
{
    Entries { map, addresses }
}

// Returns whether a shard of the length at the address would share no address with a held shard.
fn is_free(entries: &Entries, address: u64, length: u64) -> (is_free: bool)
    ensures
        is_free ==> free(entries.map@, address, length),
{
    proof {
        use_type_invariant(entries);
    }
    let mut index: usize = usize::MIN;
    while index < entries.addresses.len()
        invariant
            index <= entries.addresses@.len(),
            entries.kept(),
            forall|seen: int|
                0 <= seen < index ==> {
                    let held = #[trigger] entries.addresses@[seen];
                    held + entries.map@.index(held).spec_len() <= address || address + length
                        <= held
                },
        decreases entries.addresses@.len() - index,
    {
        let held = entries.addresses[index];
        let shard = match entries.map.get(&held) {
            Some(shard) => shard,
            None => return false,
        };
        let end = held as u128 + shard.len() as u128;
        if !(end <= address as u128 || address as u128 + length as u128 <= held as u128) {
            return false;
        }
        index += 1;
    }
    proof {
        assert forall|held: u64| #[trigger] entries.map@.contains_key(held) implies held
            + entries.map@.index(held).spec_len() <= address || address + length <= held by {
            assert(entries.addresses@.contains(held));
            let seen = choose|seen: int|
                0 <= seen < entries.addresses@.len() && entries.addresses@[seen] == held;
            assert(entries.addresses@[seen] == held);
        }
    }
    true
}

// Holds the shard at an address that no held shard shares.
fn insert_new(entries: &mut Entries, address: u64, shard: SharedBytesMemoryShard)
    requires
        !old(entries).map@.contains_key(address),
        free(old(entries).map@, address, shard.spec_len()),
        address + shard.spec_len() <= u64::MAX,
    ensures
        final(entries).map@ == old(entries).map@.insert(address, shard),
{
    let (mut map, mut addresses) = take(entries);
    let ghost before = map@;
    let ghost listed = addresses@;
    let _ = map.insert(address, shard);
    addresses.push(address);
    proof {
        assert(map@ == before.insert(address, shard));
        assert(addresses@ == listed.push(address));
        assert forall|held: u64| #[trigger] addresses@.contains(held) <==> map@.contains_key(held) by {
            if held != address {
                if listed.contains(held) {
                    let seen = choose|seen: int| 0 <= seen < listed.len() && listed[seen] == held;
                    assert(addresses@[seen] == held);
                }
                if addresses@.contains(held) {
                    let seen = choose|seen: int|
                        0 <= seen < addresses@.len() && addresses@[seen] == held;
                    assert(listed[seen] == held);
                }
            } else {
                assert(addresses@[listed.len() as int] == held);
            }
        }
        assert(addresses@.no_duplicates()) by {
            assert forall|first: int, second: int|
                0 <= first < second < addresses@.len() implies addresses@[first]
                != addresses@[second] by {
                if second == listed.len() {
                    assert(listed.contains(addresses@[first]));
                }
            }
        }
    }
    *entries = wrap(map, addresses);
}

// Stops holding the shard at the address, if one is held there.
fn remove(entries: &mut Entries, address: u64)
    ensures
        final(entries).map@ == old(entries).map@.remove(address),
{
    let (mut map, mut addresses) = take(entries);
    let ghost before = map@;
    let ghost listed = addresses@;
    let _ = map.remove(&address);
    let mut index: usize = usize::MIN;
    while index < addresses.len()
        invariant
            index <= addresses@.len(),
            addresses@ == listed,
            forall|seen: int| 0 <= seen < index ==> listed[seen] != address,
        ensures
            index <= addresses@.len(),
            addresses@ == listed,
            forall|seen: int| 0 <= seen < index ==> listed[seen] != address,
            index < addresses@.len() ==> listed[index as int] == address,
        decreases addresses@.len() - index,
    {
        if addresses[index] == address {
            break;
        }
        index += 1;
    }
    if index < addresses.len() {
        let _ = addresses.remove(index);
    }
    proof {
        assert(map@ == before.remove(address));
        assert forall|held: u64| #[trigger] addresses@.contains(held) <==> map@.contains_key(held) by {
            if index < listed.len() {
                assert(addresses@ == listed.remove(index as int));
                if listed.contains(held) && held != address {
                    let seen = choose|seen: int| 0 <= seen < listed.len() && listed[seen] == held;
                    if seen < index {
                        assert(addresses@[seen] == held);
                    } else {
                        assert(addresses@[seen - 1] == held);
                    }
                }
                if addresses@.contains(held) {
                    let seen = choose|seen: int|
                        0 <= seen < addresses@.len() && addresses@[seen] == held;
                    if seen < index {
                        assert(listed[seen] == held);
                    } else {
                        assert(listed[seen + 1] == held);
                    }
                }
            } else {
                if listed.contains(held) {
                    let seen = choose|seen: int| 0 <= seen < listed.len() && listed[seen] == held;
                    assert(listed[seen] != address);
                }
            }
        }
        if index < listed.len() {
            assert(addresses@.no_duplicates()) by {
                assert forall|first: int, second: int|
                    0 <= first < second < addresses@.len() implies addresses@[first]
                    != addresses@[second] by {
                    let a = if first < index { first } else { first + 1 };
                    let b = if second < index { second } else { second + 1 };
                    assert(listed[a] != listed[b]);
                }
            }
        }
    }
    *entries = wrap(map, addresses);
}

// Holds the shard in place of the one of the same length at the address.
fn replace(entries: &mut Entries, address: u64, shard: SharedBytesMemoryShard)
    requires
        old(entries).map@.contains_key(address),
        old(entries).map@.index(address).spec_len() == shard.spec_len(),
    ensures
        final(entries).map@ == old(entries).map@.insert(address, shard),
{
    let (mut map, addresses) = take(entries);
    let ghost before = map@;
    let _ = map.insert(address, shard);
    proof {
        assert(map@ == before.insert(address, shard));
        assert forall|held: u64| #[trigger] addresses@.contains(held) <==> map@.contains_key(held) by {
            assert(map@.contains_key(held) == before.contains_key(held));
        }
    }
    *entries = wrap(map, addresses);
}

/// The memory a task holds, as shards that never overlap under their starting virtual addresses.
pub struct OrderedMemoryCapabilityTable {
    entries: Entries,
    next_allocation: Ghost<int>,
}

impl OrderedMemoryCapabilityTable {
    /// Returns a table that holds no memory.
    pub fn new() -> (table: Self)
        ensures
            table.held() == Map::<u64, SharedBytesMemoryShard>::empty(),
    {
        let map = BTreeMap::new();
        let addresses = Vec::new();
        proof {
            assert(map@ =~= Map::empty());
        }
        let entries = wrap(map, addresses);
        OrderedMemoryCapabilityTable { entries, next_allocation: Ghost(0) }
    }

    /// What the table holds: the map's shards while its entries are kept, as every table's are.
    pub closed spec fn held(&self) -> Map<u64, SharedBytesMemoryShard> {
        if self.entries.kept() {
            self.entries.map@
        } else {
            Map::empty()
        }
    }

    /// No two shards a table holds share an address.
    pub proof fn lemma_disjoint(table: Self)
        ensures
            disjoint(table.held()),
    {
    }
}

impl CapabilityTable<u64, SharedBytesMemoryShard, MemoryShardConfig, MemoryError> for
    OrderedMemoryCapabilityTable {
    open spec fn state(&self) -> Map<u64, SharedBytesMemoryShard> {
        self.held()
    }

    fn create(&mut self, identifier: u64, config: MemoryShardConfig) -> (created: Result<
        (),
        CreateError<MemoryError>,
    >) {
        proof {
            use_type_invariant(&self.entries);
        }
        if self.entries.map.contains_key(&identifier) {
            return Err(CreateError::IdentifierTaken);
        }
        if config.length > u64::MAX - identifier {
            return Err(CreateError::Failed(MemoryError::OutOfMemory));
        }
        if !is_free(&self.entries, identifier, config.length) {
            return Err(CreateError::Failed(MemoryError::Overlap));
        }
        let shard = match SharedBytesMemoryShard::new(config, Ghost(self.next_allocation@)) {
            Some(shard) => shard,
            None => return Err(CreateError::Failed(MemoryError::OutOfMemory)),
        };
        insert_new(&mut self.entries, identifier, shard);
        self.next_allocation = Ghost(self.next_allocation@ + 1);
        proof {
            use_type_invariant(&self.entries);
            assert(self.state().remove(identifier) =~= old(self).state());
        }
        Ok(())
    }

    fn get(&self, identifier: &u64) -> (capability: Option<&SharedBytesMemoryShard>) {
        proof {
            use_type_invariant(&self.entries);
        }
        self.entries.map.get(identifier)
    }

    fn drop(&mut self, identifier: &u64) {
        proof {
            use_type_invariant(&self.entries);
        }
        remove(&mut self.entries, *identifier);
        proof {
            use_type_invariant(&self.entries);
        }
    }

    fn duplicate_within(&mut self, identifier: &u64, new_identifier: u64) -> (duplicated: Result<
        (),
        DuplicateError<MemoryError>,
    >) {
        proof {
            use_type_invariant(&self.entries);
        }
        let duplicate = match self.entries.map.get(identifier) {
            Some(shard) => shard.duplicate(),
            None => return Err(DuplicateError::NothingHeld),
        };
        if self.entries.map.contains_key(&new_identifier) {
            return Err(DuplicateError::IdentifierTaken);
        }
        let length = duplicate.len();
        if length > u64::MAX - new_identifier {
            return Err(DuplicateError::Failed(MemoryError::OutOfMemory));
        }
        if !is_free(&self.entries, new_identifier, length) {
            return Err(DuplicateError::Failed(MemoryError::Overlap));
        }
        insert_new(&mut self.entries, new_identifier, duplicate);
        proof {
            use_type_invariant(&self.entries);
        }
        Ok(())
    }

    fn duplicate_from(
        &mut self,
        other: &Self,
        identifier: &u64,
        new_identifier: u64,
    ) -> (duplicated: Result<(), DuplicateError<MemoryError>>) {
        proof {
            use_type_invariant(&self.entries);
            use_type_invariant(&other.entries);
        }
        let duplicate = match other.entries.map.get(identifier) {
            Some(shard) => shard.duplicate(),
            None => return Err(DuplicateError::NothingHeld),
        };
        if self.entries.map.contains_key(&new_identifier) {
            return Err(DuplicateError::IdentifierTaken);
        }
        let length = duplicate.len();
        if length > u64::MAX - new_identifier {
            return Err(DuplicateError::Failed(MemoryError::OutOfMemory));
        }
        if !is_free(&self.entries, new_identifier, length) {
            return Err(DuplicateError::Failed(MemoryError::Overlap));
        }
        insert_new(&mut self.entries, new_identifier, duplicate);
        proof {
            use_type_invariant(&self.entries);
        }
        Ok(())
    }
}

impl MemoryCapabilityTable<SharedBytesMemoryShard> for OrderedMemoryCapabilityTable {
    fn set_permissions(&mut self, address: &u64, permissions: MemoryPermissions) -> (set: Result<
        (),
        SetPermissionsError,
    >) {
        proof {
            use_type_invariant(&self.entries);
        }
        let changed = match self.entries.map.get(address) {
            Some(shard) => shard.with_permissions(permissions),
            None => return Err(SetPermissionsError::NothingHeld),
        };
        replace(&mut self.entries, *address, changed);
        proof {
            use_type_invariant(&self.entries);
            assert(self.state().remove(*address) =~= old(self).state().remove(*address));
        }
        Ok(())
    }

    fn split(&mut self, address: &u64, offset: u64) -> (split: Result<(), SplitError>) {
        proof {
            use_type_invariant(&self.entries);
        }
        let (first, second) = match self.entries.map.get(address) {
            Some(shard) => {
                if offset == u64::MIN || offset >= shard.len() {
                    return Err(SplitError::OutsideShard);
                }
                shard.split_at(offset)
            },
            None => return Err(SplitError::NothingHeld),
        };
        let ghost before = self.entries.map@;
        let ghost length = before.index(*address).spec_len();
        let second_address = *address + offset;
        remove(&mut self.entries, *address);
        proof {
            use_type_invariant(&self.entries);
            assert(free(self.entries.map@, *address, offset)) by {
                assert forall|held: u64| #[trigger]
                    self.entries.map@.contains_key(held) implies held
                    + self.entries.map@.index(held).spec_len() <= *address || *address + offset
                    <= held by {
                    assert(before.contains_key(held) && before.contains_key(*address));
                }
            }
        }
        insert_new(&mut self.entries, *address, first);
        proof {
            use_type_invariant(&self.entries);
            assert(!self.entries.map@.contains_key(second_address)) by {
                if self.entries.map@.contains_key(second_address) {
                    assert(before.contains_key(second_address) && before.contains_key(*address));
                }
            }
            assert(free(self.entries.map@, second_address, (length - offset) as u64)) by {
                assert forall|held: u64| #[trigger]
                    self.entries.map@.contains_key(held) implies held
                    + self.entries.map@.index(held).spec_len() <= second_address || second_address
                    + (length - offset) <= held by {
                    if held != *address {
                        assert(before.contains_key(held) && before.contains_key(*address));
                    }
                }
            }
        }
        insert_new(&mut self.entries, second_address, second);
        proof {
            use_type_invariant(&self.entries);
            assert(self.state().remove(*address).remove(second_address) =~= before.remove(
                *address,
            ));
        }
        Ok(())
    }
}

} // verus!
