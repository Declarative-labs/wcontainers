//! Memory shards whose bytes are cells shared by every entry that stands for the same memory, and a task's table of them.
//!
//! The shard and its table are one crate because splitting a shard makes two shards over the cells of one. Each table
//! entry carries its own permissions, so two tables holding the same memory may hold it with different permissions.

#![no_std]

extern crate alloc;

use alloc::rc::Rc;
use alloc::vec::Vec;

use vstd::cell::invcell::InvCell;
use vstd::predicate::Predicate;
use vstd::prelude::*;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_capability_table::{CapabilityTable, CreateError, DuplicateError};
use wcontainers_kernel_core::memory::{MemoryCapabilityTable, MemoryError, SetPermissionsError, SplitError};
use wcontainers_memory_shard::{BorrowError, MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

/// A shard of memory: a run of shared cells and the permissions its entry holds them with.
pub struct SharedCellsMemoryShard {
    run: Run,
    permissions: MemoryPermissions,
}

/// The bytes of a shard lent by one borrow, which another holder of the same memory may change at any time.
pub struct SharedCellsBytes<'shard> {
    window: Window<'shard>,
}

/// The memory a task holds, as shards under their starting addresses, no two of which overlap.
pub struct SharedCellsMemoryCapabilityTable {
    entries: Entries,
}

// The rule every byte meets, which any byte does.
struct AnyByte;

impl Predicate<u8> for AnyByte {
    open spec fn predicate(&self, byte: u8) -> bool {
        true
    }
}

// The cells of the memory one create made, which every shard cut from it or duplicated from it shares.
type Cells = Vec<InvCell<u8, AnyByte>>;

// A run of cells, inside the cells it names.
struct Run {
    cells: Rc<Cells>,
    start: usize,
    length: u64,
}

impl Run {
    #[verifier::type_invariant]
    spec fn inside(self) -> bool {
        self.start + self.length <= (*self.cells)@.len()
    }
}

// A borrowed run of cells, inside the cells it names.
struct Window<'shard> {
    cells: &'shard Cells,
    start: usize,
    length: usize,
}

impl<'shard> Window<'shard> {
    #[verifier::type_invariant]
    spec fn inside(self) -> bool {
        self.start + self.length <= self.cells@.len() <= usize::MAX
    }
}

// A shard held at an address.
struct Entry {
    address: u64,
    shard: SharedCellsMemoryShard,
}

// The entries of a table and the map they stand for, which never disagree.
struct Entries {
    list: Vec<Entry>,
    held: Ghost<Map<u64, SharedCellsMemoryShard>>,
}

impl Entries {
    #[verifier::type_invariant]
    spec fn consistent(self) -> bool {
        holds(self.list@, self.held@)
    }
}

// The length of a shard, which the trait's clauses call `spec_len`.
spec fn length_of(shard: SharedCellsMemoryShard) -> u64 {
    shard.run.length
}

// The permissions a shard is held with, which the trait's clauses call `spec_permissions`.
spec fn permissions_of(shard: SharedCellsMemoryShard) -> MemoryPermissions {
    shard.permissions
}

// The shards a table holds under their addresses, which the trait's clauses call `state`.
spec fn state_of(table: SharedCellsMemoryCapabilityTable) -> Map<u64, SharedCellsMemoryShard> {
    if holds(table.entries.list@, table.entries.held@) {
        table.entries.held@
    } else {
        Map::empty()
    }
}

// Whether no two shards share an address: kernel-core's `shards_are_disjoint` for this shard.
spec fn disjoint(shards: Map<u64, SharedCellsMemoryShard>) -> bool {
    forall|first: u64, second: u64|
        #![trigger shards.contains_key(first), shards.contains_key(second)]
        shards.contains_key(first) && shards.contains_key(second) && first != second ==> first
            + length_of(shards.index(first)) <= second || second + length_of(shards.index(second))
            <= first
}

// Whether two runs of addresses share no address.
spec fn apart(first: u64, first_length: u64, second: u64, second_length: u64) -> bool {
    first + first_length <= second || second + second_length <= first
}

// Whether a list of entries stands for the map: one entry for each address, no two shards overlapping, each ending
// inside the address space.
spec fn holds(list: Seq<Entry>, held: Map<u64, SharedCellsMemoryShard>) -> bool {
    &&& forall|index: int|
        #![trigger list[index]]
        0 <= index < list.len() ==> held.contains_key(list[index].address) && held.index(
            list[index].address,
        ) == list[index].shard
    &&& forall|address: u64|
        #![trigger held.contains_key(address)]
        held.contains_key(address) ==> exists|index: int|
            #![trigger list[index]]
            0 <= index < list.len() && list[index].address == address
    &&& forall|first: int, second: int|
        #![trigger list[first], list[second]]
        0 <= first < list.len() && 0 <= second < list.len() && first != second
            ==> list[first].address != list[second].address
    &&& disjoint(held)
    &&& forall|address: u64|
        #![trigger held.contains_key(address)]
        held.contains_key(address) ==> address + length_of(held.index(address)) <= u64::MAX
}

// Proves that the shards of any table never overlap, the fact a task's memory gives its callers.
proof fn lemma_shards_are_disjoint(table: SharedCellsMemoryCapabilityTable)
    ensures
        disjoint(state_of(table)),
{
}

// Proves that a new entry apart from every other keeps a list standing for its map.
proof fn lemma_push(list: Seq<Entry>, held: Map<u64, SharedCellsMemoryShard>, entry: Entry)
    requires
        holds(list, held),
        !held.contains_key(entry.address),
        entry.address + length_of(entry.shard) <= u64::MAX,
        forall|index: int|
            #![trigger list[index]]
            0 <= index < list.len() ==> apart(
                list[index].address,
                length_of(list[index].shard),
                entry.address,
                length_of(entry.shard),
            ),
    ensures
        holds(list.push(entry), held.insert(entry.address, entry.shard)),
{
    let pushed = list.push(entry);
    let map = held.insert(entry.address, entry.shard);
    assert forall|index: int| #![trigger pushed[index]] 0 <= index < pushed.len() implies map.contains_key(
        pushed[index].address,
    ) && map.index(pushed[index].address) == pushed[index].shard by {
        if index < list.len() {
            assert(pushed[index] == list[index]);
            assert(held.contains_key(list[index].address));
        }
    }
    assert forall|address: u64| #![trigger map.contains_key(address)] map.contains_key(address) implies exists|index: int|
        #![trigger pushed[index]]
        0 <= index < pushed.len() && pushed[index].address == address by {
        if address == entry.address {
            assert(pushed[list.len() as int] == entry);
        } else {
            assert(held.contains_key(address));
            let index = choose|index: int| #![trigger list[index]] 0 <= index < list.len() && list[index].address == address;
            assert(pushed[index] == list[index]);
        }
    }
    assert forall|first: int, second: int| #![trigger pushed[first], pushed[second]]
        0 <= first < pushed.len() && 0 <= second < pushed.len() && first != second implies pushed[first].address
        != pushed[second].address by {
        if first < list.len() {
            assert(pushed[first] == list[first]);
            assert(held.contains_key(list[first].address));
        }
        if second < list.len() {
            assert(pushed[second] == list[second]);
            assert(held.contains_key(list[second].address));
        }
    }
    assert forall|first: u64, second: u64| #![trigger map.contains_key(first), map.contains_key(second)]
        map.contains_key(first) && map.contains_key(second) && first != second implies first
        + length_of(map.index(first)) <= second || second + length_of(map.index(second))
        <= first by {
        if first == entry.address {
            assert(held.contains_key(second));
            let index = choose|index: int| #![trigger list[index]] 0 <= index < list.len() && list[index].address == second;
            assert(apart(list[index].address, length_of(list[index].shard), entry.address, length_of(entry.shard)));
        } else if second == entry.address {
            assert(held.contains_key(first));
            let index = choose|index: int| #![trigger list[index]] 0 <= index < list.len() && list[index].address == first;
            assert(apart(list[index].address, length_of(list[index].shard), entry.address, length_of(entry.shard)));
        } else {
            assert(held.contains_key(first) && held.contains_key(second));
        }
    }
    assert forall|address: u64| #![trigger map.contains_key(address)] map.contains_key(address) implies address + length_of(map.index(
        address,
    )) <= u64::MAX by {
        if address != entry.address {
            assert(held.contains_key(address));
        }
    }
}

// Proves that taking an entry out keeps a list standing for its map without the entry's address.
proof fn lemma_remove(list: Seq<Entry>, held: Map<u64, SharedCellsMemoryShard>, removed: int)
    requires
        holds(list, held),
        0 <= removed < list.len(),
    ensures
        holds(list.remove(removed), held.remove(list[removed].address)),
{
    let left = list.remove(removed);
    let gone = list[removed].address;
    let map = held.remove(gone);
    assert forall|index: int| #![trigger left[index]] 0 <= index < left.len() implies map.contains_key(left[index].address)
        && map.index(left[index].address) == left[index].shard by {
        let before = if index < removed { index } else { index + 1 };
        assert(left[index] == list[before]);
        assert(list[before].address != list[removed].address);
    }
    assert forall|address: u64| #![trigger map.contains_key(address)] map.contains_key(address) implies exists|index: int|
        #![trigger left[index]]
        0 <= index < left.len() && left[index].address == address by {
        assert(held.contains_key(address));
        let before = choose|index: int| #![trigger list[index]] 0 <= index < list.len() && list[index].address == address;
        assert(before != removed);
        let after = if before < removed { before } else { before - 1 };
        assert(left[after] == list[before]);
    }
    assert forall|first: int, second: int| #![trigger left[first], left[second]]
        0 <= first < left.len() && 0 <= second < left.len() && first != second implies left[first].address
        != left[second].address by {
        let first_before = if first < removed { first } else { first + 1 };
        let second_before = if second < removed { second } else { second + 1 };
        assert(left[first] == list[first_before]);
        assert(left[second] == list[second_before]);
    }
    assert forall|first: u64, second: u64| #![trigger map.contains_key(first), map.contains_key(second)]
        map.contains_key(first) && map.contains_key(second) && first != second implies first
        + length_of(map.index(first)) <= second || second + length_of(map.index(second))
        <= first by {
        assert(held.contains_key(first) && held.contains_key(second));
    }
    assert forall|address: u64| #![trigger map.contains_key(address)] map.contains_key(address) implies address + length_of(map.index(
        address,
    )) <= u64::MAX by {
        assert(held.contains_key(address));
    }
}

// Proves that putting a shard no longer than the one it replaces under the same address keeps a list standing for its map.
proof fn lemma_replace(
    list: Seq<Entry>,
    held: Map<u64, SharedCellsMemoryShard>,
    replaced: int,
    shard: SharedCellsMemoryShard,
)
    requires
        holds(list, held),
        0 <= replaced < list.len(),
        length_of(shard) <= length_of(list[replaced].shard),
    ensures
        holds(
            list.update(replaced, Entry { address: list[replaced].address, shard }),
            held.insert(list[replaced].address, shard),
        ),
{
    let address = list[replaced].address;
    let changed = list.update(replaced, Entry { address, shard });
    let map = held.insert(address, shard);
    assert(held.contains_key(address));
    assert forall|index: int| #![trigger changed[index]] 0 <= index < changed.len() implies map.contains_key(
        changed[index].address,
    ) && map.index(changed[index].address) == changed[index].shard by {
        if index != replaced {
            assert(changed[index] == list[index]);
            assert(list[index].address != list[replaced].address);
        }
    }
    assert forall|key: u64| #![trigger map.contains_key(key)] map.contains_key(key) implies exists|index: int|
        #![trigger changed[index]]
        0 <= index < changed.len() && changed[index].address == key by {
        if key == address {
            assert(changed[replaced].address == address);
        } else {
            assert(held.contains_key(key));
            let index = choose|index: int| #![trigger list[index]] 0 <= index < list.len() && list[index].address == key;
            assert(changed[index] == list[index]);
        }
    }
    assert forall|first: int, second: int| #![trigger changed[first], changed[second]]
        0 <= first < changed.len() && 0 <= second < changed.len() && first != second implies changed[first].address
        != changed[second].address by {
        assert(changed[first].address == list[first].address);
        assert(changed[second].address == list[second].address);
    }
    assert forall|first: u64, second: u64| #![trigger map.contains_key(first), map.contains_key(second)]
        map.contains_key(first) && map.contains_key(second) && first != second implies first
        + length_of(map.index(first)) <= second || second + length_of(map.index(second))
        <= first by {
        assert(held.contains_key(first) && held.contains_key(second));
    }
    assert forall|key: u64| #![trigger map.contains_key(key)] map.contains_key(key) implies key + length_of(map.index(key))
        <= u64::MAX by {
        assert(held.contains_key(key));
    }
}

// Proves that once a shard is cut to its first `offset` bytes, the rest of it is free for the second shard.
proof fn lemma_room_after_cut(
    list: Seq<Entry>,
    held: Map<u64, SharedCellsMemoryShard>,
    cut: int,
    offset: u64,
    first: SharedCellsMemoryShard,
)
    requires
        holds(list, held),
        0 <= cut < list.len(),
        0 < offset < length_of(list[cut].shard),
        length_of(first) == offset,
    ensures
        ({
            let address = list[cut].address;
            let second = (address + offset) as u64;
            let length = (length_of(list[cut].shard) - offset) as u64;
            let changed = list.update(cut, Entry { address, shard: first });
            &&& address + length_of(list[cut].shard) <= u64::MAX
            &&& second == address + offset
            &&& second + length <= u64::MAX
            &&& !held.contains_key(second)
            &&& !held.insert(address, first).contains_key(second)
            &&& forall|index: int|
                #![trigger changed[index]]
                0 <= index < changed.len() ==> apart(
                    changed[index].address,
                    length_of(changed[index].shard),
                    second,
                    length,
                )
        }),
{
    let address = list[cut].address;
    let whole = length_of(list[cut].shard);
    let second = (address + offset) as u64;
    let length = (whole - offset) as u64;
    let changed = list.update(cut, Entry { address, shard: first });
    assert(held.contains_key(address));
    if held.contains_key(second) {
        assert(second != address);
        assert(address + length_of(held.index(address)) <= second || second + length_of(held.index(
            second,
        )) <= address);
    }
    assert forall|index: int| #![trigger changed[index]] 0 <= index < changed.len() implies apart(
        changed[index].address,
        length_of(changed[index].shard),
        second,
        length,
    ) by {
        if index != cut {
            assert(changed[index] == list[index]);
            let other = list[index].address;
            assert(other != address);
            assert(held.contains_key(other));
            assert(other + length_of(held.index(other)) <= address || address + length_of(held.index(
                address,
            )) <= other);
        }
    }
}

// Returns where in the list the address is held, if it is.
fn find(list: &[Entry], address: u64) -> (found: Option<usize>)
    ensures
        match found {
            Some(index) => index < list@.len() && list@[index as int].address == address,
            None => forall|index: int|
                #![trigger list@[index]]
                0 <= index < list@.len() ==> list@[index].address != address,
        },
{
    let mut index = usize::MIN;
    while index < list.len()
        invariant
            index <= list@.len(),
            forall|earlier: int|
                #![trigger list@[earlier]]
                0 <= earlier < index ==> list@[earlier].address != address,
        decreases list@.len() - index,
    {
        if list[index].address == address {
            return Some(index);
        }
        index += 1;
    }
    None
}

// Returns whether two runs of addresses share no address, without computing an end that may not fit.
fn is_apart(first: u64, first_length: u64, second: u64, second_length: u64) -> (is_apart: bool)
    ensures
        is_apart == apart(first, first_length, second, second_length),
{
    (first <= second && second - first >= first_length) || (second <= first && first - second
        >= second_length)
}

// Returns why a shard of the length cannot be held at the address beside the shards of the list, if it cannot.
fn refusal(list: &[Entry], address: u64, length: u64) -> (refused: Option<MemoryError>)
    ensures
        refused is None ==> address + length <= u64::MAX && forall|index: int|
            #![trigger list@[index]]
            0 <= index < list@.len() ==> apart(
                list@[index].address,
                length_of(list@[index].shard),
                address,
                length,
            ),
{
    if length > u64::MAX - address {
        return Some(MemoryError::OutOfMemory);
    }
    let mut index = usize::MIN;
    while index < list.len()
        invariant
            index <= list@.len(),
            address + length <= u64::MAX,
            forall|earlier: int|
                #![trigger list@[earlier]]
                0 <= earlier < index ==> apart(
                    list@[earlier].address,
                    length_of(list@[earlier].shard),
                    address,
                    length,
                ),
        decreases list@.len() - index,
    {
        let entry = &list[index];
        if !is_apart(entry.address, entry.shard.run.length, address, length) {
            return Some(MemoryError::Overlap);
        }
        index += 1;
    }
    None
}

// Returns a run of new cells, every byte zero, of the length the config asks for, or nothing when no memory is left.
fn make(config: MemoryShardConfig) -> (made: Option<SharedCellsMemoryShard>)
    ensures
        made matches Some(shard) ==> length_of(shard) == config.length && permissions_of(shard)
            == config.permissions,
{
    if config.length > usize::MAX as u64 {
        return None;
    }
    let length = config.length as usize;
    let mut cells: Cells = Vec::new();
    if cells.try_reserve_exact(length).is_err() {
        return None;
    }
    let mut made = usize::MIN;
    while made < length
        invariant
            made <= length,
            cells@.len() == made,
        decreases length - made,
    {
        cells.push(InvCell::new(u8::MIN, Ghost(AnyByte)));
        made += 1;
    }
    Some(
        SharedCellsMemoryShard {
            run: Run { cells: Rc::new(cells), start: usize::MIN, length: config.length },
            permissions: config.permissions,
        },
    )
}

// Returns a second shard over the same cells as the first, held with the permissions.
fn reprotect(shard: &SharedCellsMemoryShard, permissions: MemoryPermissions) -> (reprotected:
    SharedCellsMemoryShard)
    ensures
        reprotected.run == shard.run,
        reprotected.permissions == permissions,
{
    proof {
        use_type_invariant(&shard.run);
    }
    SharedCellsMemoryShard {
        run: Run { cells: Rc::clone(&shard.run.cells), start: shard.run.start, length: shard.run.length },
        permissions,
    }
}

// Returns a second shard over the same cells as the first, held with the same permissions.
fn duplicate(shard: &SharedCellsMemoryShard) -> (duplicate: SharedCellsMemoryShard)
    ensures
        duplicate == *shard,
{
    reprotect(shard, shard.permissions)
}

// Returns the first `offset` bytes of a shard and the rest, as two shards over its cells with its permissions.
fn cut(shard: &SharedCellsMemoryShard, offset: u64) -> (cut: (SharedCellsMemoryShard, SharedCellsMemoryShard))
    requires
        0 < offset < length_of(*shard),
    ensures
        length_of(cut.0) == offset,
        length_of(cut.1) == length_of(*shard) - offset,
        permissions_of(cut.0) == permissions_of(*shard),
        permissions_of(cut.1) == permissions_of(*shard),
{
    proof {
        use_type_invariant(&shard.run);
    }
    let cells: &Cells = &shard.run.cells;
    let _ = cells.len();
    let first = Run { cells: Rc::clone(&shard.run.cells), start: shard.run.start, length: offset };
    let second = Run {
        cells: Rc::clone(&shard.run.cells),
        start: shard.run.start + offset as usize,
        length: shard.run.length - offset,
    };
    (
        SharedCellsMemoryShard { run: first, permissions: shard.permissions },
        SharedCellsMemoryShard { run: second, permissions: shard.permissions },
    )
}

// Returns the list, which stands for the map, leaving no entries in its place.
fn take(entries: &mut Entries) -> (list: Vec<Entry>)
    ensures
        list@ == old(entries).list@,
        holds(list@, old(entries).held@),
{
    proof {
        use_type_invariant(&*entries);
    }
    let mut taken = Entries { list: Vec::new(), held: Ghost(Map::empty()) };
    core::mem::swap(entries, &mut taken);
    proof {
        use_type_invariant(&taken);
    }
    let (list, _held) = (taken.list, taken.held);
    list
}

// Holds the entry beside shards it is apart from.
fn insert(entries: &mut Entries, entry: Entry)
    requires
        !old(entries).held@.contains_key(entry.address),
        entry.address + length_of(entry.shard) <= u64::MAX,
        forall|index: int|
            #![trigger old(entries).list@[index]]
            0 <= index < old(entries).list@.len() ==> apart(
                old(entries).list@[index].address,
                length_of(old(entries).list@[index].shard),
                entry.address,
                length_of(entry.shard),
            ),
    ensures
        final(entries).held@ == old(entries).held@.insert(entry.address, entry.shard),
{
    let ghost held = entries.held@;
    let mut list = take(entries);
    let ghost pushed = entry;
    proof {
        lemma_push(list@, held, entry);
    }
    list.push(entry);
    *entries = Entries { list, held: Ghost(held.insert(pushed.address, pushed.shard)) };
}

// Stops holding the entry at the index.
fn remove(entries: &mut Entries, index: usize)
    requires
        index < old(entries).list@.len(),
    ensures
        final(entries).held@ == old(entries).held@.remove(old(entries).list@[index as int].address),
{
    let ghost held = entries.held@;
    let mut list = take(entries);
    let ghost address = list@[index as int].address;
    proof {
        lemma_remove(list@, held, index as int);
    }
    let _ = list.remove(index);
    *entries = Entries { list, held: Ghost(held.remove(address)) };
}

// Holds a shard no longer than the one at the index in its place.
fn replace(entries: &mut Entries, index: usize, shard: SharedCellsMemoryShard)
    requires
        index < old(entries).list@.len(),
        length_of(shard) <= length_of(old(entries).list@[index as int].shard),
    ensures
        final(entries).list@ == old(entries).list@.update(
            index as int,
            Entry { address: old(entries).list@[index as int].address, shard },
        ),
        final(entries).held@ == old(entries).held@.insert(
            old(entries).list@[index as int].address,
            shard,
        ),
{
    let ghost held = entries.held@;
    let mut list = take(entries);
    let address = list[index].address;
    let ghost placed = shard;
    proof {
        lemma_replace(list@, held, index as int, shard);
    }
    list[index] = Entry { address, shard };
    *entries = Entries { list, held: Ghost(held.insert(address, placed)) };
}


// Creates a shard as `CapabilityTable::create` promises, its clause written over this crate's own view of the table.
fn create_shard(
    table: &mut SharedCellsMemoryCapabilityTable,
    identifier: u64,
    config: MemoryShardConfig,
) -> (created: Result<(), CreateError<MemoryError>>)
    ensures
        match created {
            Ok(_) => !state_of(*old(table)).contains_key(identifier) && state_of(
                *final(table),
            ).contains_key(identifier) && state_of(*final(table)).remove(identifier) == state_of(
                *old(table),
            ) && length_of(state_of(*final(table)).index(identifier)) == config.length
                && permissions_of(state_of(*final(table)).index(identifier)) == config.permissions,
            Err(CreateError::IdentifierTaken) => state_of(*old(table)).contains_key(identifier)
                && state_of(*final(table)) == state_of(*old(table)),
            Err(CreateError::Failed(_)) => !state_of(*old(table)).contains_key(identifier)
                && state_of(*final(table)) == state_of(*old(table)),
        },
{
    proof {
        use_type_invariant(&table.entries);
    }
    if find(table.entries.list.as_slice(), identifier).is_some() {
        return Err(CreateError::IdentifierTaken);
    }
    if let Some(refused) = refusal(table.entries.list.as_slice(), identifier, config.length) {
        return Err(CreateError::Failed(refused));
    }
    let shard = match make(config) {
        Some(shard) => shard,
        None => return Err(CreateError::Failed(MemoryError::OutOfMemory)),
    };
    insert(&mut table.entries, Entry { address: identifier, shard });
    proof {
        use_type_invariant(&table.entries);
        assert(state_of(*table).remove(identifier) =~= state_of(*old(table)));
    }
    Ok(())
}

// Returns the shard at the identifier as `CapabilityTable::get` promises.
fn get_shard<'table>(table: &'table SharedCellsMemoryCapabilityTable, identifier: &u64) -> (capability:
    Option<&'table SharedCellsMemoryShard>)
    ensures
        match capability {
            Some(capability) => state_of(*table).contains_key(*identifier) && *capability == state_of(
                *table,
            ).index(*identifier),
            None => !state_of(*table).contains_key(*identifier),
        },
{
    proof {
        use_type_invariant(&table.entries);
    }
    match find(table.entries.list.as_slice(), *identifier) {
        Some(index) => Some(&table.entries.list[index].shard),
        None => None,
    }
}

// Drops the entry at the identifier as `CapabilityTable::drop` promises.
fn drop_shard(table: &mut SharedCellsMemoryCapabilityTable, identifier: &u64)
    ensures
        state_of(*final(table)) == state_of(*old(table)).remove(*identifier),
{
    proof {
        use_type_invariant(&table.entries);
    }
    if let Some(index) = find(table.entries.list.as_slice(), *identifier) {
        remove(&mut table.entries, index);
        proof {
            use_type_invariant(&table.entries);
        }
    } else {
        proof {
            assert(state_of(*old(table)).remove(*identifier) =~= state_of(*old(table)));
        }
    }
}

// Holds a duplicate of a shard at the new identifier, as both duplicates promise once the shard is found.
fn hold_duplicate(table: &mut SharedCellsMemoryCapabilityTable, shard: SharedCellsMemoryShard, new_identifier: u64) -> (held:
    Result<(), DuplicateError<MemoryError>>)
    ensures
        match held {
            Ok(_) => !state_of(*old(table)).contains_key(new_identifier) && state_of(*final(table))
                == state_of(*old(table)).insert(new_identifier, shard),
            Err(DuplicateError::IdentifierTaken) => state_of(*old(table)).contains_key(new_identifier)
                && state_of(*final(table)) == state_of(*old(table)),
            Err(DuplicateError::Failed(_)) => !state_of(*old(table)).contains_key(new_identifier)
                && state_of(*final(table)) == state_of(*old(table)),
            Err(DuplicateError::NothingHeld) => false,
        },
{
    proof {
        use_type_invariant(&table.entries);
    }
    if find(table.entries.list.as_slice(), new_identifier).is_some() {
        return Err(DuplicateError::IdentifierTaken);
    }
    if let Some(refused) = refusal(table.entries.list.as_slice(), new_identifier, shard.run.length) {
        return Err(DuplicateError::Failed(refused));
    }
    insert(&mut table.entries, Entry { address: new_identifier, shard });
    proof {
        use_type_invariant(&table.entries);
    }
    Ok(())
}

// Duplicates a shard of the table into it as `CapabilityTable::duplicate_within` promises.
fn duplicate_shard_within(
    table: &mut SharedCellsMemoryCapabilityTable,
    identifier: &u64,
    new_identifier: u64,
) -> (duplicated: Result<(), DuplicateError<MemoryError>>)
    ensures
        match duplicated {
            Ok(_) => state_of(*old(table)).contains_key(*identifier) && !state_of(
                *old(table),
            ).contains_key(new_identifier) && state_of(*final(table)) == state_of(
                *old(table),
            ).insert(new_identifier, state_of(*old(table)).index(*identifier)),
            Err(DuplicateError::NothingHeld) => !state_of(*old(table)).contains_key(*identifier)
                && state_of(*final(table)) == state_of(*old(table)),
            Err(DuplicateError::IdentifierTaken) => state_of(*old(table)).contains_key(*identifier)
                && state_of(*old(table)).contains_key(new_identifier) && state_of(*final(table))
                == state_of(*old(table)),
            Err(DuplicateError::Failed(_)) => state_of(*old(table)).contains_key(*identifier)
                && !state_of(*old(table)).contains_key(new_identifier) && state_of(*final(table))
                == state_of(*old(table)),
        },
{
    let shard = match get_shard(table, identifier) {
        Some(shard) => duplicate(shard),
        None => return Err(DuplicateError::NothingHeld),
    };
    hold_duplicate(table, shard, new_identifier)
}

// Duplicates a shard of another table into this one as `CapabilityTable::duplicate_from` promises.
fn duplicate_shard_from(
    table: &mut SharedCellsMemoryCapabilityTable,
    other: &SharedCellsMemoryCapabilityTable,
    identifier: &u64,
    new_identifier: u64,
) -> (duplicated: Result<(), DuplicateError<MemoryError>>)
    ensures
        match duplicated {
            Ok(_) => state_of(*other).contains_key(*identifier) && !state_of(
                *old(table),
            ).contains_key(new_identifier) && state_of(*final(table)) == state_of(
                *old(table),
            ).insert(new_identifier, state_of(*other).index(*identifier)),
            Err(DuplicateError::NothingHeld) => !state_of(*other).contains_key(*identifier)
                && state_of(*final(table)) == state_of(*old(table)),
            Err(DuplicateError::IdentifierTaken) => state_of(*other).contains_key(*identifier)
                && state_of(*old(table)).contains_key(new_identifier) && state_of(*final(table))
                == state_of(*old(table)),
            Err(DuplicateError::Failed(_)) => state_of(*other).contains_key(*identifier)
                && !state_of(*old(table)).contains_key(new_identifier) && state_of(*final(table))
                == state_of(*old(table)),
        },
{
    let shard = match get_shard(other, identifier) {
        Some(shard) => duplicate(shard),
        None => return Err(DuplicateError::NothingHeld),
    };
    hold_duplicate(table, shard, new_identifier)
}

// Holds the shard at the address with new permissions as `MemoryCapabilityTable::set_permissions` promises.
fn set_shard_permissions(
    table: &mut SharedCellsMemoryCapabilityTable,
    address: &u64,
    permissions: MemoryPermissions,
) -> (set: Result<(), SetPermissionsError>)
    ensures
        match set {
            Ok(_) => {
                &&& state_of(*old(table)).contains_key(*address)
                &&& state_of(*final(table)).contains_key(*address)
                &&& state_of(*final(table)).remove(*address) == state_of(*old(table)).remove(*address)
                &&& length_of(state_of(*final(table)).index(*address)) == length_of(
                    state_of(*old(table)).index(*address),
                )
                &&& permissions_of(state_of(*final(table)).index(*address)) == permissions
            },
            Err(SetPermissionsError::NothingHeld) => !state_of(*old(table)).contains_key(*address)
                && state_of(*final(table)) == state_of(*old(table)),
            Err(SetPermissionsError::Failed(_)) => state_of(*old(table)).contains_key(*address)
                && state_of(*final(table)) == state_of(*old(table)),
        },
{
    proof {
        use_type_invariant(&table.entries);
    }
    let index = match find(table.entries.list.as_slice(), *address) {
        Some(index) => index,
        None => return Err(SetPermissionsError::NothingHeld),
    };
    let shard = reprotect(&table.entries.list[index].shard, permissions);
    replace(&mut table.entries, index, shard);
    proof {
        use_type_invariant(&table.entries);
        assert(state_of(*table).remove(*address) =~= state_of(*old(table)).remove(*address));
    }
    Ok(())
}

// Splits the shard at the address in two as `MemoryCapabilityTable::split` promises.
fn split_shard(table: &mut SharedCellsMemoryCapabilityTable, address: &u64, offset: u64) -> (split:
    Result<(), SplitError>)
    ensures
        match split {
            Ok(_) => {
                let shard = state_of(*old(table)).index(*address);
                let second = (*address + offset) as u64;
                &&& state_of(*old(table)).contains_key(*address)
                &&& 0 < offset < length_of(shard)
                &&& state_of(*final(table)).contains_key(*address)
                &&& state_of(*final(table)).contains_key(second)
                &&& state_of(*final(table)).remove(*address).remove(second) == state_of(
                    *old(table),
                ).remove(*address)
                &&& length_of(state_of(*final(table)).index(*address)) == offset
                &&& length_of(state_of(*final(table)).index(second)) == length_of(shard) - offset
                &&& permissions_of(state_of(*final(table)).index(*address)) == permissions_of(shard)
                &&& permissions_of(state_of(*final(table)).index(second)) == permissions_of(shard)
            },
            Err(SplitError::NothingHeld) => !state_of(*old(table)).contains_key(*address)
                && state_of(*final(table)) == state_of(*old(table)),
            Err(SplitError::OutsideShard) => state_of(*old(table)).contains_key(*address) && !(0
                < offset < length_of(state_of(*old(table)).index(*address))) && state_of(*final(table))
                == state_of(*old(table)),
            Err(SplitError::Failed(_)) => state_of(*old(table)).contains_key(*address)
                && state_of(*final(table)) == state_of(*old(table)),
        },
{
    proof {
        use_type_invariant(&table.entries);
    }
    let index = match find(table.entries.list.as_slice(), *address) {
        Some(index) => index,
        None => return Err(SplitError::NothingHeld),
    };
    if offset == u64::MIN || offset >= table.entries.list[index].shard.run.length {
        return Err(SplitError::OutsideShard);
    }
    let (first, second) = cut(&table.entries.list[index].shard, offset);
    let ghost list = table.entries.list@;
    let ghost held = table.entries.held@;
    proof {
        lemma_room_after_cut(list, held, index as int, offset, first);
    }
    replace(&mut table.entries, index, first);
    let second_address = *address + offset;
    insert(&mut table.entries, Entry { address: second_address, shard: second });
    proof {
        use_type_invariant(&table.entries);
        assert(state_of(*table).remove(*address).remove(second_address) =~= state_of(
            *old(table),
        ).remove(*address));
    }
    Ok(())
}

// Lends the bytes of a shard as `MemoryShard::borrow` promises.
fn borrow_bytes(shard: &SharedCellsMemoryShard, offset: u64, length: u64) -> (borrowed: Result<
    SharedCellsBytes<'_>,
    BorrowError,
>)
    ensures
        match borrowed {
            Ok(bytes) => offset + length <= length_of(*shard) && bytes.spec_len() == length,
            Err(_) => offset + length > length_of(*shard),
        },
{
    proof {
        use_type_invariant(&shard.run);
    }
    if offset > shard.run.length || length > shard.run.length - offset {
        return Err(BorrowError::OutsideShard);
    }
    let cells: &Cells = &shard.run.cells;
    let _ = cells.len();
    Ok(
        SharedCellsBytes {
            window: Window { cells, start: shard.run.start + offset as usize, length: length as usize },
        },
    )
}

// Returns where in the cells a copy at the offset starts and how many bytes it moves for a buffer of the length.
fn range(window: &Window, offset: u64, buffer: usize) -> (range: (usize, usize))
    ensures
        range.1 == wcontainers_volatile_bytes::copied(window.length as nat, offset, buffer as nat),
        range.0 + range.1 <= window.cells@.len(),
{
    proof {
        use_type_invariant(window);
    }
    if offset >= window.length as u64 {
        return (window.start, usize::MIN);
    }
    let inside = offset as usize;
    let rest = window.length - inside;
    if buffer <= rest {
        (window.start + inside, buffer)
    } else {
        (window.start + inside, rest)
    }
}

impl Default for SharedCellsMemoryCapabilityTable {
    // Returns a table holding no memory.
    fn default() -> (table: Self) {
        SharedCellsMemoryCapabilityTable { entries: Entries { list: Vec::new(), held: Ghost(Map::empty()) } }
    }
}

impl<'shard> VolatileBytes for SharedCellsBytes<'shard> {
    closed spec fn spec_len(&self) -> nat {
        self.window.length as nat
    }

    fn len(&self) -> (length: u64) {
        self.window.length as u64
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> (count: usize) {
        proof {
            use_type_invariant(&self.window);
        }
        let (first, count) = range(&self.window, offset, into.len());
        let mut index = usize::MIN;
        while index < count
            invariant
                count == wcontainers_volatile_bytes::copied(self.spec_len(), offset, old(into)@.len()),
                first + count <= self.window.cells@.len() <= usize::MAX,
                into@.len() == old(into)@.len(),
                index <= count,
                forall|later: int| count <= later < old(into)@.len() ==> into@[later] == old(into)@[later],
            decreases count - index,
        {
            into[index] = self.window.cells[first + index].get();
            index += 1;
        }
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> (count: usize) {
        proof {
            use_type_invariant(&self.window);
        }
        let (first, count) = range(&self.window, offset, bytes.len());
        let mut index = usize::MIN;
        while index < count
            invariant
                count == wcontainers_volatile_bytes::copied(self.spec_len(), offset, bytes@.len()),
                first + count <= self.window.cells@.len() <= usize::MAX,
                count <= bytes@.len(),
                index <= count,
            decreases count - index,
        {
            self.window.cells[first + index].set(bytes[index]);
            index += 1;
        }
        count
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> (read: Result<usize, Sink::Error>) {
        proof {
            use_type_invariant(&self.window);
        }
        let mut copy: Vec<u8> = Vec::new();
        let mut index = usize::MIN;
        while index < self.window.length
            invariant
                index <= self.window.length,
                self.window.start + self.window.length <= self.window.cells@.len() <= usize::MAX,
                copy@.len() == index,
            decreases self.window.length - index,
        {
            copy.push(self.window.cells[self.window.start + index].get());
            index += 1;
        }
        sink.write(copy.as_slice())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> (written: Result<usize, Source::Error>) {
        proof {
            use_type_invariant(&self.window);
        }
        let mut copy: Vec<u8> = Vec::new();
        while copy.len() < self.window.length
            invariant
                copy@.len() <= self.window.length,
            decreases self.window.length - copy@.len(),
        {
            copy.push(u8::MIN);
        }
        let read = source.read(copy.as_mut_slice());
        if let Ok(count) = read {
            let mut index = usize::MIN;
            while index < count
                invariant
                    count <= copy@.len(),
                    copy@.len() == self.window.length,
                    self.window.start + self.window.length <= self.window.cells@.len() <= usize::MAX,
                    index <= count,
                decreases count - index,
            {
                self.window.cells[self.window.start + index].set(copy[index]);
                index += 1;
            }
        }
        read
    }
}

/// A shard as the trait names it; each method is one call of a proven function of this crate.
#[allow(
    reviewable::implementation_not_verified,
    reason = "TODO: prove it: Verus 0.2026.11.22 refuses a generic associated type in an impl, and `ByteView<'shard>` is one"
)]
#[verifier::external]
impl MemoryShard for SharedCellsMemoryShard {
    type ByteView<'shard> = SharedCellsBytes<'shard> where Self: 'shard;

    spec fn spec_len(&self) -> u64 {
        length_of(*self)
    }

    fn len(&self) -> u64 {
        self.run.length
    }

    spec fn spec_permissions(&self) -> MemoryPermissions {
        permissions_of(*self)
    }

    fn permissions(&self) -> MemoryPermissions {
        self.permissions
    }

    fn borrow(&self, offset: u64, length: u64) -> Result<SharedCellsBytes<'_>, BorrowError> {
        borrow_bytes(self, offset, length)
    }
}

/// The table as the trait names it; each method is one call of a proven function of this crate.
#[allow(
    reviewable::implementation_not_verified,
    reason = "TODO: prove it: its clauses read the shard's `spec_len`, which Verus cannot see while `impl MemoryShard` must be external"
)]
#[verifier::external]
impl CapabilityTable<u64, SharedCellsMemoryShard, MemoryShardConfig, MemoryError> for SharedCellsMemoryCapabilityTable {
    spec fn state(&self) -> Map<u64, SharedCellsMemoryShard> {
        state_of(*self)
    }

    fn create(&mut self, identifier: u64, config: MemoryShardConfig) -> Result<(), CreateError<MemoryError>> {
        create_shard(self, identifier, config)
    }

    fn get(&self, identifier: &u64) -> Option<&SharedCellsMemoryShard> {
        get_shard(self, identifier)
    }

    fn drop(&mut self, identifier: &u64) {
        drop_shard(self, identifier)
    }

    fn duplicate_within(&mut self, identifier: &u64, new_identifier: u64) -> Result<(), DuplicateError<MemoryError>> {
        duplicate_shard_within(self, identifier, new_identifier)
    }

    fn duplicate_from(
        &mut self,
        other: &Self,
        identifier: &u64,
        new_identifier: u64,
    ) -> Result<(), DuplicateError<MemoryError>> {
        duplicate_shard_from(self, other, identifier, new_identifier)
    }
}

/// The table as kernel-core names it; each method is one call of a proven function of this crate.
#[allow(
    reviewable::implementation_not_verified,
    reason = "TODO: prove it: its clauses read the shard's `spec_len`, which Verus cannot see while `impl MemoryShard` must be external"
)]
#[verifier::external]
impl MemoryCapabilityTable<SharedCellsMemoryShard> for SharedCellsMemoryCapabilityTable {
    fn set_permissions(&mut self, address: &u64, permissions: MemoryPermissions) -> Result<(), SetPermissionsError> {
        set_shard_permissions(self, address, permissions)
    }

    fn split(&mut self, address: &u64, offset: u64) -> Result<(), SplitError> {
        split_shard(self, address, offset)
    }
}

} // verus!
