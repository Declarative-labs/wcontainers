//! A memory table that keeps a task's shards apart, over any memory table.

use alloc::vec::Vec;
use core::marker::PhantomData;

use vstd::prelude::*;
use wcontainers_capability_table::{CapabilityConfig, CapabilityTable, CreateError, DuplicateError};
use wcontainers_kernel_core::memory::{
    MemoryCapabilityTable, MemoryError, SetPermissionsError, SplitError,
};
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};

verus! {

/// Whether a range of addresses shares no address with the shard at another start.
pub open spec fn lies_apart<Shard: MemoryShard>(
    shards: Map<u64, Shard>,
    start: u64,
    length: u64,
    other: u64,
) -> bool {
    start + length <= other || other + shards.index(other).spec_len() <= start
}

pub open spec fn misses_every_shard<Shard: MemoryShard>(
    shards: Map<u64, Shard>,
    start: u64,
    length: u64,
) -> bool {
    forall|other: u64| #[trigger]
        shards.contains_key(other) ==> lies_apart(shards, start, length, other)
}

/// Whether every shard ends inside the address space, so no split or address past it wraps.
pub open spec fn fit_in_address_space<Shard: MemoryShard>(shards: Map<u64, Shard>) -> bool {
    forall|start: u64| #[trigger]
        shards.contains_key(start) ==> start + shards.index(start).spec_len() <= u64::MAX
}

pub open spec fn contains_address<Shard: MemoryShard>(
    shards: Map<u64, Shard>,
    start: u64,
    address: u64,
) -> bool {
    start <= address < start + shards.index(start).spec_len()
}

pub open spec fn follows_create<Shard: MemoryShard>(
    before: Map<u64, Shard>,
    after: Map<u64, Shard>,
    identifier: u64,
    config: MemoryShardConfig,
    created: Result<(), CreateError<MemoryError>>,
) -> bool {
    match created {
        Ok(_) => !before.contains_key(identifier) && after.contains_key(identifier) && after.remove(
            identifier,
        ) == before && config.describes(after.index(identifier)),
        Err(CreateError::IdentifierTaken) => before.contains_key(identifier) && after == before,
        Err(CreateError::Failed(_)) => !before.contains_key(identifier) && after == before,
    }
}

pub open spec fn follows_duplicate<Shard: MemoryShard>(
    before: Map<u64, Shard>,
    after: Map<u64, Shard>,
    source: Map<u64, Shard>,
    identifier: u64,
    new_identifier: u64,
    duplicated: Result<(), DuplicateError<MemoryError>>,
) -> bool {
    match duplicated {
        Ok(_) => source.contains_key(identifier) && !before.contains_key(new_identifier) && after
            == before.insert(new_identifier, source.index(identifier)),
        Err(DuplicateError::NothingHeld) => !source.contains_key(identifier) && after == before,
        Err(DuplicateError::IdentifierTaken) => source.contains_key(identifier)
            && before.contains_key(new_identifier) && after == before,
        Err(DuplicateError::Failed(_)) => source.contains_key(identifier) && !before.contains_key(
            new_identifier,
        ) && after == before,
    }
}

pub open spec fn follows_set_permissions<Shard: MemoryShard>(
    before: Map<u64, Shard>,
    after: Map<u64, Shard>,
    address: u64,
    permissions: MemoryPermissions,
    set: Result<(), SetPermissionsError>,
) -> bool {
    match set {
        Ok(_) => {
            &&& before.contains_key(address)
            &&& after.contains_key(address)
            &&& after.remove(address) == before.remove(address)
            &&& after.index(address).spec_len() == before.index(address).spec_len()
            &&& after.index(address).spec_permissions() == permissions
        },
        Err(SetPermissionsError::NothingHeld) => !before.contains_key(address) && after == before,
        Err(SetPermissionsError::Failed(_)) => before.contains_key(address) && after == before,
    }
}

pub open spec fn follows_split<Shard: MemoryShard>(
    before: Map<u64, Shard>,
    after: Map<u64, Shard>,
    address: u64,
    offset: u64,
    split: Result<(), SplitError>,
) -> bool {
    match split {
        Ok(_) => {
            let shard = before.index(address);
            let second = (address + offset) as u64;
            &&& before.contains_key(address)
            &&& 0 < offset < shard.spec_len()
            &&& after.contains_key(address)
            &&& after.contains_key(second)
            &&& after.remove(address).remove(second) == before.remove(address)
            &&& after.index(address).spec_len() == offset
            &&& after.index(second).spec_len() == shard.spec_len() - offset
            &&& after.index(address).spec_permissions() == shard.spec_permissions()
            &&& after.index(second).spec_permissions() == shard.spec_permissions()
        },
        Err(SplitError::NothingHeld) => !before.contains_key(address) && after == before,
        Err(SplitError::OutsideShard) => before.contains_key(address) && !(0 < offset
            < before.index(address).spec_len()) && after == before,
        Err(SplitError::Failed(_)) => before.contains_key(address) && after == before,
    }
}

proof fn lemma_insert_keeps_apart<Shard: MemoryShard>(
    before: Map<u64, Shard>,
    after: Map<u64, Shard>,
    start: u64,
)
    requires
        wcontainers_kernel_core::memory::shards_are_disjoint(before),
        fit_in_address_space(before),
        !before.contains_key(start),
        after == before.insert(start, after.index(start)),
        misses_every_shard(before, start, after.index(start).spec_len()),
        start + after.index(start).spec_len() <= u64::MAX,
    ensures
        wcontainers_kernel_core::memory::shards_are_disjoint(after),
        fit_in_address_space(after),
{
    assert forall|first: u64, second: u64|
        #![trigger after.contains_key(first), after.contains_key(second)]
        after.contains_key(first) && after.contains_key(second) && first != second implies first
        + after.index(first).spec_len() <= second || second + after.index(second).spec_len()
        <= first by {
        if first == start {
            assert(before.contains_key(second));
            assert(lies_apart(before, start, after.index(start).spec_len(), second));
        } else if second == start {
            assert(before.contains_key(first));
            assert(lies_apart(before, start, after.index(start).spec_len(), first));
        } else {
            assert(before.contains_key(first) && before.contains_key(second));
        }
    }
    assert forall|other: u64| #[trigger]
        after.contains_key(other) implies other + after.index(other).spec_len() <= u64::MAX by {
        if other != start {
            assert(before.contains_key(other));
        }
    }
}

proof fn lemma_split_keeps_apart<Shard: MemoryShard>(
    before: Map<u64, Shard>,
    after: Map<u64, Shard>,
    address: u64,
    offset: u64,
)
    requires
        wcontainers_kernel_core::memory::shards_are_disjoint(before),
        fit_in_address_space(before),
        follows_split(before, after, address, offset, Ok(())),
    ensures
        !before.contains_key((address + offset) as u64),
        wcontainers_kernel_core::memory::shards_are_disjoint(after),
        fit_in_address_space(after),
        forall|start: u64| #[trigger]
            after.contains_key(start) ==> before.contains_key(start) || start == (address
                + offset) as u64,
{
    let length = before.index(address).spec_len();
    let second = (address + offset) as u64;
    assert(address + length <= u64::MAX);
    assert(second == address + offset);
    if before.contains_key(second) {
        assert(second != address);
        assert(!(address + before.index(address).spec_len() <= second || second + before.index(
            second,
        ).spec_len() <= address));
    }
    assert forall|other: u64|
        other != address && other != second implies #[trigger] after.contains_key(other)
        == before.contains_key(other) && (before.contains_key(other) ==> after.index(other)
        == before.index(other)) by {
        assert(after.remove(address).remove(second).contains_key(other) == after.contains_key(
            other,
        ));
        assert(before.remove(address).contains_key(other) == before.contains_key(other));
        if before.contains_key(other) {
            assert(after.remove(address).remove(second).index(other) == after.index(other));
            assert(before.remove(address).index(other) == before.index(other));
        }
    }
    assert forall|first: u64, second_start: u64|
        #![trigger after.contains_key(first), after.contains_key(second_start)]
        after.contains_key(first) && after.contains_key(second_start) && first
            != second_start implies first + after.index(first).spec_len() <= second_start
        || second_start + after.index(second_start).spec_len() <= first by {
        if first != address && first != second {
            assert(before.contains_key(first));
            assert(before.contains_key(address));
        }
        if second_start != address && second_start != second {
            assert(before.contains_key(second_start));
            assert(before.contains_key(address));
        }
    }
    assert forall|other: u64| #[trigger]
        after.contains_key(other) implies other + after.index(other).spec_len() <= u64::MAX by {
        if other != address && other != second {
            assert(before.contains_key(other));
        }
    }
}

/// The table and a list of every start it holds a shard at, perhaps with others.
struct Parts<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>> {
    table: Table,
    starts: Vec<u64>,
    shard: PhantomData<Shard>,
}

impl<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>> Parts<Shard, Table> {
    spec fn is_consistent(&self) -> bool {
        &&& forall|start: u64| #[trigger]
            self.table.state().contains_key(start) ==> self.starts@.contains(start)
        &&& wcontainers_kernel_core::memory::shards_are_disjoint(self.table.state())
        &&& fit_in_address_space(self.table.state())
    }

    fn misses_shards(&self, start: u64, length: u64) -> (missed: bool)
        requires
            self.is_consistent(),
            start + length <= u64::MAX,
        ensures
            missed == misses_every_shard(self.table.state(), start, length),
    {
        let mut index = usize::MIN;
        while index < self.starts.len()
            invariant
                index <= self.starts.len(),
                self.is_consistent(),
                start + length <= u64::MAX,
                forall|position: int|
                    0 <= position < index && self.table.state().contains_key(
                        #[trigger] self.starts@[position],
                    ) ==> lies_apart(self.table.state(), start, length, self.starts@[position]),
            decreases self.starts.len() - index,
        {
            let other = self.starts[index];
            if let Some(shard) = self.table.get(&other) {
                let other_length = shard.len();
                if !(start + length <= other || other + other_length <= start) {
                    assert(!lies_apart(self.table.state(), start, length, other));
                    return false;
                }
            }
            index += 1;
        }
        assert forall|other: u64| #[trigger]
            self.table.state().contains_key(other) implies lies_apart(
            self.table.state(),
            start,
            length,
            other,
        ) by {
            assert(self.starts@.contains(other));
            let position = choose|position: int|
                0 <= position < self.starts@.len() && self.starts@[position] == other;
            assert(self.table.state().contains_key(self.starts@[position]));
        }
        true
    }

    fn find_start(&self, address: u64) -> (found: Option<u64>)
        requires
            self.is_consistent(),
        ensures
            match found {
                Some(start) => self.table.state().contains_key(start) && contains_address(
                    self.table.state(),
                    start,
                    address,
                ),
                None => forall|start: u64| #[trigger]
                    self.table.state().contains_key(start) ==> !contains_address(
                        self.table.state(),
                        start,
                        address,
                    ),
            },
    {
        let mut index = usize::MIN;
        while index < self.starts.len()
            invariant
                index <= self.starts.len(),
                self.is_consistent(),
                forall|position: int|
                    0 <= position < index && self.table.state().contains_key(
                        #[trigger] self.starts@[position],
                    ) ==> !contains_address(self.table.state(), self.starts@[position], address),
            decreases self.starts.len() - index,
        {
            let start = self.starts[index];
            if let Some(shard) = self.table.get(&start) {
                let length = shard.len();
                if start <= address && address - start < length {
                    return Some(start);
                }
            }
            index += 1;
        }
        assert forall|start: u64| #[trigger]
            self.table.state().contains_key(start) implies !contains_address(
            self.table.state(),
            start,
            address,
        ) by {
            assert(self.starts@.contains(start));
            let position = choose|position: int|
                0 <= position < self.starts@.len() && self.starts@[position] == start;
            assert(self.table.state().contains_key(self.starts@[position]));
        }
        None
    }

    fn remove_start(&mut self, removed: u64)
        requires
            forall|start: u64| #[trigger]
                old(self).table.state().contains_key(start) && start != removed ==> old(self).starts@.contains(start),
        ensures
            final(self).table == old(self).table,
            forall|start: u64| #[trigger]
                final(self).table.state().contains_key(start) && start != removed ==> final(self).starts@.contains(start),
    {
        let mut kept: Vec<u64> = Vec::new();
        let mut index = usize::MIN;
        while index < self.starts.len()
            invariant
                index <= self.starts.len(),
                forall|position: int|
                    0 <= position < index && #[trigger] self.starts@[position] != removed
                        ==> kept@.contains(self.starts@[position]),
            decreases self.starts.len() - index,
        {
            let start = self.starts[index];
            if start != removed {
                let ghost before = kept@;
                kept.push(start);
                assert(kept@[before.len() as int] == start);
                assert forall|position: int|
                    0 <= position < index && #[trigger] self.starts@[position]
                        != removed implies kept@.contains(self.starts@[position]) by {
                    let earlier = choose|earlier: int|
                        0 <= earlier < before.len() && before[earlier] == self.starts@[position];
                    assert(kept@[earlier] == self.starts@[position]);
                }
            }
            index += 1;
        }
        assert forall|start: u64| #[trigger]
            self.table.state().contains_key(start) && start != removed implies kept@.contains(
            start,
        ) by {
            assert(self.starts@.contains(start));
            let position = choose|position: int|
                0 <= position < self.starts@.len() && self.starts@[position] == start;
            assert(self.starts@[position] != removed);
        }
        self.starts = kept;
    }

    fn add_start(&mut self, start: u64)
        ensures
            final(self).table == old(self).table,
            final(self).starts@.contains(start),
            forall|other: u64| old(self).starts@.contains(other) ==> #[trigger] final(self).starts@.contains(other),
    {
        let ghost before = self.starts@;
        self.starts.push(start);
        assert(self.starts@[before.len() as int] == start);
        assert forall|other: u64| before.contains(other) implies #[trigger] self.starts@.contains(
            other,
        ) by {
            let position = choose|position: int|
                0 <= position < before.len() && before[position] == other;
            assert(self.starts@[position] == other);
        }
    }

    fn create_shard(&mut self, identifier: u64, config: MemoryShardConfig) -> (created: Result<
        (),
        CreateError<MemoryError>,
    >)
        requires
            old(self).is_consistent(),
        ensures
            final(self).is_consistent(),
            follows_create(
                old(self).table.state(),
                final(self).table.state(),
                identifier,
                config,
                created,
            ),
    {
        if self.table.get(&identifier).is_some() {
            return Err(CreateError::IdentifierTaken);
        }
        if config.length > u64::MAX - identifier {
            return Err(CreateError::Failed(MemoryError::OutOfMemory));
        }
        if !self.misses_shards(identifier, config.length) {
            return Err(CreateError::Failed(MemoryError::Overlap));
        }
        let ghost before = self.table.state();
        let ghost wanted = config;
        let created = self.table.create(identifier, config);
        if created.is_ok() {
            self.add_start(identifier);
            proof {
                let after = self.table.state();
                assert(after =~= before.insert(identifier, after.index(identifier)));
                lemma_insert_keeps_apart(before, after, identifier);
            }
        }
        proof {
            assert(follows_create(before, self.table.state(), identifier, wanted, created));
        }
        created
    }

    fn duplicate_shard(
        &mut self,
        length: u64,
        identifier: &u64,
        new_identifier: u64,
        other: Option<&Self>,
    ) -> (duplicated: Result<(), DuplicateError<MemoryError>>)
        requires
            old(self).is_consistent(),
            match other {
                Some(other) => other.table.state().contains_key(*identifier)
                    && other.table.state().index(*identifier).spec_len() == length,
                None => old(self).table.state().contains_key(*identifier) && old(self).table.state().index(*identifier).spec_len() == length,
            },
        ensures
            final(self).is_consistent(),
            follows_duplicate(
                old(self).table.state(),
                final(self).table.state(),
                match other {
                    Some(other) => other.table.state(),
                    None => old(self).table.state(),
                },
                *identifier,
                new_identifier,
                duplicated,
            ),
    {
        if self.table.get(&new_identifier).is_some() {
            return Err(DuplicateError::IdentifierTaken);
        }
        if length > u64::MAX - new_identifier {
            return Err(DuplicateError::Failed(MemoryError::OutOfMemory));
        }
        if !self.misses_shards(new_identifier, length) {
            return Err(DuplicateError::Failed(MemoryError::Overlap));
        }
        let ghost before = self.table.state();
        let duplicated = match other {
            Some(other) => self.table.duplicate_from(&other.table, identifier, new_identifier),
            None => self.table.duplicate_within(identifier, new_identifier),
        };
        if duplicated.is_ok() {
            self.add_start(new_identifier);
            proof {
                let after = self.table.state();
                assert(after =~= before.insert(new_identifier, after.index(new_identifier)));
                lemma_insert_keeps_apart(before, after, new_identifier);
            }
        }
        duplicated
    }

    fn drop_shard(&mut self, identifier: &u64)
        requires
            old(self).is_consistent(),
        ensures
            final(self).is_consistent(),
            final(self).table.state() == old(self).table.state().remove(*identifier),
    {
        let ghost before = self.table.state();
        self.table.drop(identifier);
        proof {
            let after = self.table.state();
            assert forall|first: u64, second: u64|
        #![trigger after.contains_key(first), after.contains_key(second)]
                after.contains_key(first) && after.contains_key(second) && first
                    != second implies first + after.index(first).spec_len() <= second || second
                + after.index(second).spec_len() <= first by {
                assert(before.contains_key(first) && before.contains_key(second));
            }
            assert forall|start: u64| #[trigger]
                after.contains_key(start) implies start + after.index(start).spec_len()
                <= u64::MAX by {
                assert(before.contains_key(start));
            }
            assert forall|start: u64| #[trigger]
                after.contains_key(start) && start != *identifier implies self.starts@.contains(
                start,
            ) by {
                assert(before.contains_key(start));
            }
        }
        self.remove_start(*identifier);
        proof {
            let after = self.table.state();
            assert(!after.contains_key(*identifier));
            assert forall|start: u64| #[trigger]
                after.contains_key(start) implies self.starts@.contains(start) by {
                assert(start != *identifier);
            }
        }
    }

    fn set_shard_permissions(&mut self, address: &u64, permissions: MemoryPermissions) -> (set:
        Result<(), SetPermissionsError>)
        requires
            old(self).is_consistent(),
        ensures
            final(self).is_consistent(),
            follows_set_permissions(
                old(self).table.state(),
                final(self).table.state(),
                *address,
                permissions,
                set,
            ),
    {
        let ghost before = self.table.state();
        let set = self.table.set_permissions(address, permissions);
        proof {
            let after = self.table.state();
            if set.is_ok() {
                assert forall|other: u64|
                    other != *address implies #[trigger] after.contains_key(other)
                    == before.contains_key(other) && (before.contains_key(other) ==> after.index(
                    other,
                ) == before.index(other)) by {
                    assert(after.remove(*address).contains_key(other) == after.contains_key(
                        other,
                    ));
                    assert(before.remove(*address).contains_key(other) == before.contains_key(
                        other,
                    ));
                    if before.contains_key(other) {
                        assert(after.remove(*address).index(other) == after.index(other));
                        assert(before.remove(*address).index(other) == before.index(other));
                    }
                }
                assert forall|first: u64, second: u64|
        #![trigger after.contains_key(first), after.contains_key(second)]
                    after.contains_key(first) && after.contains_key(second) && first
                        != second implies first + after.index(first).spec_len() <= second || second
                    + after.index(second).spec_len() <= first by {
                    assert(before.contains_key(first) && before.contains_key(second));
                }
                assert forall|start: u64| #[trigger]
                    after.contains_key(start) implies start + after.index(start).spec_len()
                    <= u64::MAX by {
                    assert(before.contains_key(start));
                }
                assert forall|start: u64| #[trigger]
                    after.contains_key(start) implies self.starts@.contains(start) by {
                    assert(before.contains_key(start));
                }
            }
        }
        set
    }

    fn split_shard(&mut self, address: &u64, offset: u64) -> (split: Result<(), SplitError>)
        requires
            old(self).is_consistent(),
        ensures
            final(self).is_consistent(),
            follows_split(old(self).table.state(), final(self).table.state(), *address, offset, split),
    {
        let ghost before = self.table.state();
        let split = self.table.split(address, offset);
        if split.is_ok() {
            proof {
                lemma_split_keeps_apart(before, self.table.state(), *address, offset);
                assert(*address + offset <= u64::MAX);
            }
            self.add_start(*address + offset);
            proof {
                let after = self.table.state();
                assert forall|start: u64| #[trigger]
                    after.contains_key(start) implies self.starts@.contains(start) by {
                    if start != (*address + offset) as u64 {
                        assert(before.contains_key(start));
                    }
                }
            }
        }
        split
    }
}

/// A task's memory: shards under their starts, which never share an address.
///
/// Its parts are taken out while a call changes them, so a call that unwinds leaves it holding
/// nothing rather than something inconsistent.
pub struct DisjointMemory<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>> {
    parts: Option<Parts<Shard, Table>>,
}

impl<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>> DisjointMemory<Shard, Table> {
    #[verifier::type_invariant]
    spec fn is_consistent(self) -> bool {
        self.parts matches Some(parts) ==> parts.is_consistent()
    }

    pub fn new(table: Table) -> (memory: Self)
        requires
            table.state() == Map::<u64, Shard>::empty(),
        ensures
            memory.state() == Map::<u64, Shard>::empty(),
    {
        DisjointMemory { parts: Some(Parts { table, starts: Vec::new(), shard: PhantomData }) }
    }

    pub proof fn lemma_shards_are_disjoint(memory: Self)
        ensures
            wcontainers_kernel_core::memory::shards_are_disjoint(memory.state()),
            fit_in_address_space(memory.state()),
    {
    }

    /// Returns the shard holding an address, with its start.
    pub fn find_shard(&self, address: u64) -> (found: Option<(u64, &Shard)>)
        ensures
            match found {
                Some((start, shard)) => self.state().contains_key(start) && *shard
                    == self.state().index(start) && contains_address(self.state(), start, address),
                None => forall|start: u64| #[trigger]
                    self.state().contains_key(start) ==> !contains_address(
                        self.state(),
                        start,
                        address,
                    ),
            },
    {
        proof {
            use_type_invariant(self);
        }
        match &self.parts {
            Some(parts) => match parts.find_start(address) {
                Some(start) => match parts.table.get(&start) {
                    Some(shard) => Some((start, shard)),
                    None => None,
                },
                None => None,
            },
            None => None,
        }
    }

    /// Returns a list that names every start a shard is held at, perhaps with others.
    pub fn list_starts(&self) -> (starts: &[u64])
        ensures
            forall|start: u64| #[trigger]
                self.state().contains_key(start) ==> starts@.contains(start),
    {
        proof {
            use_type_invariant(self);
        }
        match &self.parts {
            Some(parts) => parts.starts.as_slice(),
            None => &[],
        }
    }
}

impl<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>> CapabilityTable<
    u64,
    Shard,
    MemoryShardConfig,
    MemoryError,
> for DisjointMemory<Shard, Table> {
    closed spec fn state(&self) -> Map<u64, Shard> {
        match self.parts {
            Some(parts) => if wcontainers_kernel_core::memory::shards_are_disjoint(parts.table.state()) && fit_in_address_space(
                parts.table.state(),
            ) {
                parts.table.state()
            } else {
                Map::empty()
            },
            None => Map::empty(),
        }
    }

    fn create(&mut self, identifier: u64, config: MemoryShardConfig) -> (created: Result<
        (),
        CreateError<MemoryError>,
    >) {
        proof {
            use_type_invariant(&*self);
        }
        match self.parts.take() {
            Some(mut parts) => {
                let created = parts.create_shard(identifier, config);
                self.parts = Some(parts);
                created
            },
            None => Err(CreateError::Failed(MemoryError::OutOfMemory)),
        }
    }

    fn get(&self, identifier: &u64) -> (capability: Option<&Shard>) {
        proof {
            use_type_invariant(self);
        }
        match &self.parts {
            Some(parts) => parts.table.get(identifier),
            None => None,
        }
    }

    fn drop(&mut self, identifier: &u64) {
        proof {
            use_type_invariant(&*self);
        }
        if let Some(mut parts) = self.parts.take() {
            parts.drop_shard(identifier);
            self.parts = Some(parts);
        }
    }

    fn duplicate_within(&mut self, identifier: &u64, new_identifier: u64) -> (duplicated: Result<
        (),
        DuplicateError<MemoryError>,
    >) {
        proof {
            use_type_invariant(&*self);
        }
        match self.parts.take() {
            Some(mut parts) => {
                let duplicated = match parts.table.get(identifier) {
                    Some(shard) => {
                        let length = shard.len();
                        parts.duplicate_shard(length, identifier, new_identifier, None)
                    },
                    None => Err(DuplicateError::NothingHeld),
                };
                self.parts = Some(parts);
                duplicated
            },
            None => Err(DuplicateError::NothingHeld),
        }
    }

    fn duplicate_from(
        &mut self,
        other: &Self,
        identifier: &u64,
        new_identifier: u64,
    ) -> (duplicated: Result<(), DuplicateError<MemoryError>>) {
        proof {
            use_type_invariant(&*self);
            use_type_invariant(other);
        }
        let other_parts = match &other.parts {
            Some(other_parts) => other_parts,
            None => {
                return Err(DuplicateError::NothingHeld);
            },
        };
        let length = match other_parts.table.get(identifier) {
            Some(shard) => shard.len(),
            None => {
                return Err(DuplicateError::NothingHeld);
            },
        };
        match self.parts.take() {
            Some(mut parts) => {
                let duplicated = parts.duplicate_shard(
                    length,
                    identifier,
                    new_identifier,
                    Some(other_parts),
                );
                self.parts = Some(parts);
                duplicated
            },
            None => Err(DuplicateError::Failed(MemoryError::OutOfMemory)),
        }
    }
}

impl<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>> MemoryCapabilityTable<
    Shard,
> for DisjointMemory<Shard, Table> {
    fn set_permissions(&mut self, address: &u64, permissions: MemoryPermissions) -> (set: Result<
        (),
        SetPermissionsError,
    >) {
        proof {
            use_type_invariant(&*self);
        }
        match self.parts.take() {
            Some(mut parts) => {
                let set = parts.set_shard_permissions(address, permissions);
                self.parts = Some(parts);
                set
            },
            None => Err(SetPermissionsError::NothingHeld),
        }
    }

    fn split(&mut self, address: &u64, offset: u64) -> (split: Result<(), SplitError>) {
        proof {
            use_type_invariant(&*self);
        }
        match self.parts.take() {
            Some(mut parts) => {
                let split = parts.split_shard(address, offset);
                self.parts = Some(parts);
                split
            },
            None => Err(SplitError::NothingHeld),
        }
    }
}

} // verus!
