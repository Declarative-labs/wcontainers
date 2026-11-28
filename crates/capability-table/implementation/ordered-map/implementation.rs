//! A capability table in the standard library's ordered map, and a kind of memory shard for it to hold.
//!
//! The table makes and duplicates capabilities by the two functions it holds, whose specifications say that a made
//! capability is what its config asks for and a duplicate is the same capability.

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use vstd::cell::invcell::InvCell;
use vstd::predicate::Predicate;
use vstd::prelude::*;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_capability_table::{
    CapabilityConfig, CapabilityTable, CreateError, DuplicateError,
};
use wcontainers_memory_capability_table::MemoryError;
use wcontainers_memory_shard::{BorrowError, MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

broadcast use vstd::std_specs::btree::group_btree_axioms;

/// A table of capabilities in an ordered map from each identifier, which makes and duplicates them by its two functions.
#[verifier::reject_recursive_types(Capability)]
#[verifier::reject_recursive_types(Config)]
#[verifier::reject_recursive_types(Error)]
pub struct OrderedMapCapabilityTable<
    Identifier: Ord,
    Capability,
    Config: CapabilityConfig<Capability>,
    Error,
    Make: Fn(Config) -> Result<Capability, Error>,
    Duplicate: Fn(&Capability) -> Capability,
> {
    entries: Entries<Identifier, Capability>,
    makers: Makers<Capability, Config, Error, Make, Duplicate>,
}

// The standard library's ordered map from identifiers to capabilities, kept with the order laws its specification needs.
struct Entries<Identifier: Ord, Capability> {
    map: BTreeMap<Identifier, Capability>,
}

impl<Identifier: Ord, Capability> Entries<Identifier, Capability> {
    // The order laws under which the standard library's map is specified, which identifiers obey, and the map's own.
    #[verifier::type_invariant]
    spec fn ordered(self) -> bool {
        &&& vstd::laws_cmp::obeys_cmp::<Identifier>()
        &&& vstd::laws_cmp::obeys_cmp_concrete_eq::<Identifier>()
        &&& vstd::std_specs::btree::btree_map_inv(&self.map)
    }
}

// The two functions a table makes and duplicates capabilities by, kept with the specifications the table relies on.
#[verifier::reject_recursive_types(Capability)]
#[verifier::reject_recursive_types(Config)]
#[verifier::reject_recursive_types(Error)]
struct Makers<
    Capability,
    Config: CapabilityConfig<Capability>,
    Error,
    Make: Fn(Config) -> Result<Capability, Error>,
    Duplicate: Fn(&Capability) -> Capability,
> {
    make: Make,
    duplicate: Duplicate,
    kinds: PhantomData<(Capability, Config, Error)>,
}

impl<
    Capability,
    Config: CapabilityConfig<Capability>,
    Error,
    Make: Fn(Config) -> Result<Capability, Error>,
    Duplicate: Fn(&Capability) -> Capability,
> Makers<Capability, Config, Error, Make, Duplicate> {
    // Every config may be made, what is made is what its config asks for, and a duplicate equals its original.
    #[verifier::type_invariant]
    spec fn specified(self) -> bool {
        &&& makes_what_is_asked(self.make)
        &&& duplicates_the_same(self.duplicate)
    }
}

// Whether a function makes, from every config, either an error or a capability the config describes.
spec fn makes_what_is_asked<
    Capability,
    Config: CapabilityConfig<Capability>,
    Error,
    Make: Fn(Config) -> Result<Capability, Error>,
>(make: Make) -> bool {
    &&& forall|config: Config| #[trigger] make.requires((config,))
    &&& forall|config: Config, made: Result<Capability, Error>| #[trigger]
        make.ensures((config,), made) ==> (made matches Ok(capability) ==> config.describes(capability))
}

// Whether a function returns, for every capability, one equal to it.
spec fn duplicates_the_same<Capability, Duplicate: Fn(&Capability) -> Capability>(
    duplicate: Duplicate,
) -> bool {
    &&& forall|capability: &Capability| #[trigger] duplicate.requires((capability,))
    &&& forall|capability: &Capability, copy: Capability| #[trigger]
        duplicate.ensures((capability,), copy) ==> copy == *capability
}

// Returns the map, leaving an empty one in its place, so that the map can change under no borrow of a field.
fn take<Identifier: Ord, Capability>(entries: &mut Entries<Identifier, Capability>) -> (map: BTreeMap<
    Identifier,
    Capability,
>)
    ensures
        map@ == old(entries).map@,
        vstd::laws_cmp::obeys_cmp::<Identifier>(),
        vstd::laws_cmp::obeys_cmp_concrete_eq::<Identifier>(),
        vstd::std_specs::btree::btree_map_inv(&map),
{
    proof {
        use_type_invariant(&*entries);
    }
    let mut taken = Entries { map: BTreeMap::new() };
    core::mem::swap(entries, &mut taken);
    proof {
        use_type_invariant(&taken);
    }
    taken.map
}

// Returns the entries of a map whose keys obey the order laws.
fn wrap<Identifier: Ord, Capability>(map: BTreeMap<Identifier, Capability>) -> (entries: Entries<Identifier, Capability>)
    requires
        vstd::laws_cmp::obeys_cmp::<Identifier>(),
        vstd::laws_cmp::obeys_cmp_concrete_eq::<Identifier>(),
        vstd::std_specs::btree::btree_map_inv(&map),
    ensures
        entries.map@ == map@,
{
    Entries { map }
}

// Returns a table holding nothing, which makes and duplicates by the two functions.
fn make_table<
    Identifier: Ord,
    Capability,
    Config: CapabilityConfig<Capability>,
    Error,
    Make: Fn(Config) -> Result<Capability, Error>,
    Duplicate: Fn(&Capability) -> Capability,
>(make: Make, duplicate: Duplicate) -> (table: OrderedMapCapabilityTable<
    Identifier,
    Capability,
    Config,
    Error,
    Make,
    Duplicate,
>)
    requires
        vstd::laws_cmp::obeys_cmp::<Identifier>(),
        vstd::laws_cmp::obeys_cmp_concrete_eq::<Identifier>(),
        makes_what_is_asked(make),
        duplicates_the_same(duplicate),
    ensures
        table.state() == Map::<Identifier, Capability>::empty(),
{
    OrderedMapCapabilityTable {
        entries: wrap(BTreeMap::new()),
        makers: Makers { make, duplicate, kinds: PhantomData },
    }
}

impl<
    Identifier: Ord,
    Capability,
    Config: CapabilityConfig<Capability>,
    Error,
    Make: Fn(Config) -> Result<Capability, Error>,
    Duplicate: Fn(&Capability) -> Capability,
> CapabilityTable<Identifier, Capability, Config, Error> for OrderedMapCapabilityTable<
    Identifier,
    Capability,
    Config,
    Error,
    Make,
    Duplicate,
> {
    closed spec fn state(&self) -> Map<Identifier, Capability> {
        self.entries.map@
    }

    fn create(&mut self, identifier: Identifier, config: Config) -> (created: Result<
        (),
        CreateError<Error>,
    >) {
        proof {
            use_type_invariant(&self.entries);
            use_type_invariant(&self.makers);
        }
        if self.entries.map.contains_key(&identifier) {
            return Err(CreateError::IdentifierTaken);
        }
        let ghost asked = config;
        let made = (self.makers.make)(config);
        let capability = match made {
            Ok(capability) => capability,
            Err(error) => return Err(CreateError::Failed(error)),
        };
        proof {
            assert(self.makers.make.ensures((asked,), made));
        }
        let ghost key = identifier;
        let mut map = take(&mut self.entries);
        let _ = map.insert(identifier, capability);
        proof {
            assert(map@.remove(key) =~= old(self).entries.map@);
        }
        self.entries = wrap(map);
        Ok(())
    }

    fn get(&self, identifier: &Identifier) -> (capability: Option<&Capability>) {
        proof {
            use_type_invariant(&self.entries);
        }
        self.entries.map.get(identifier)
    }

    fn drop(&mut self, identifier: &Identifier) {
        let mut map = take(&mut self.entries);
        let _ = map.remove(identifier);
        self.entries = wrap(map);
    }

    fn duplicate_within(&mut self, identifier: &Identifier, new_identifier: Identifier) -> (duplicated:
        Result<(), DuplicateError<Error>>) {
        proof {
            use_type_invariant(&self.entries);
            use_type_invariant(&self.makers);
        }
        let duplicate = match self.entries.map.get(identifier) {
            Some(capability) => (self.makers.duplicate)(capability),
            None => return Err(DuplicateError::NothingHeld),
        };
        if self.entries.map.contains_key(&new_identifier) {
            return Err(DuplicateError::IdentifierTaken);
        }
        let mut map = take(&mut self.entries);
        let _ = map.insert(new_identifier, duplicate);
        self.entries = wrap(map);
        Ok(())
    }

    fn duplicate_from(
        &mut self,
        other: &Self,
        identifier: &Identifier,
        new_identifier: Identifier,
    ) -> (duplicated: Result<(), DuplicateError<Error>>) {
        proof {
            use_type_invariant(&self.entries);
            use_type_invariant(&self.makers);
            use_type_invariant(&other.entries);
        }
        let duplicate = match other.entries.map.get(identifier) {
            Some(capability) => (self.makers.duplicate)(capability),
            None => return Err(DuplicateError::NothingHeld),
        };
        if self.entries.map.contains_key(&new_identifier) {
            return Err(DuplicateError::IdentifierTaken);
        }
        let mut map = take(&mut self.entries);
        let _ = map.insert(new_identifier, duplicate);
        self.entries = wrap(map);
        Ok(())
    }
}

/// The length of a WebAssembly page, the most bytes a shard holds.
///
/// https://webassembly.github.io/spec/core/exec/runtime.html#page-size
const WEBASSEMBLY_PAGE_LENGTH: u64 = 65536;

/// A shard of memory whose bytes every duplicate shares, held with the permissions it was made with.
pub struct Shard {
    cells: Rc<Vec<InvCell<u8, AnyByte>>>,
    permissions: MemoryPermissions,
}

/// A run of a shard's bytes that a borrow lends.
pub struct Window<'shard> {
    cells: &'shard [InvCell<u8, AnyByte>],
}

// The rule every byte of a shard meets, which any byte does.
struct AnyByte;

impl Predicate<u8> for AnyByte {
    open spec fn predicate(&self, byte: u8) -> bool {
        true
    }
}

// Returns a shard of the config's length and permissions, every byte zero, or `OutOfMemory` past a WebAssembly page.
fn make_shard(config: MemoryShardConfig) -> (made: Result<Shard, MemoryError>)
    ensures
        made matches Ok(shard) ==> config.describes(shard),
        made is Err <==> config.length > WEBASSEMBLY_PAGE_LENGTH,
{
    if config.length > WEBASSEMBLY_PAGE_LENGTH {
        return Err(MemoryError::OutOfMemory);
    }
    let mut cells: Vec<InvCell<u8, AnyByte>> = Vec::new();
    let mut made: u64 = u64::MIN;
    while made < config.length
        invariant
            made <= config.length,
            config.length <= WEBASSEMBLY_PAGE_LENGTH,
            cells@.len() == made,
        decreases config.length - made,
    {
        cells.push(InvCell::new(u8::MIN, Ghost(AnyByte)));
        made += 1;
    }
    Ok(Shard { cells: Rc::new(cells), permissions: config.permissions })
}

// Returns a second capability to the same bytes with the same permissions.
fn duplicate_shard(shard: &Shard) -> (duplicate: Shard)
    ensures
        duplicate == *shard,
{
    Shard { cells: Rc::clone(&shard.cells), permissions: shard.permissions }
}

/// Returns a table of shards under their starting addresses, holding nothing.
pub fn make_shard_table() -> (table: OrderedMapCapabilityTable<
    u64,
    Shard,
    MemoryShardConfig,
    MemoryError,
    impl Fn(MemoryShardConfig) -> Result<Shard, MemoryError>,
    impl Fn(&Shard) -> Shard,
>)
    ensures
        table.state() == Map::<u64, Shard>::empty(),
{
    make_table(make_shard, duplicate_shard)
}

impl MemoryShard for Shard {
    type ByteView<'shard> = Window<'shard>;

    closed spec fn spec_len(&self) -> u64 {
        self.cells@.len() as u64
    }

    fn len(&self) -> (length: u64) {
        self.cells.len() as u64
    }

    // One identity for every allocation, since nothing in Verus tells two apart without an assumption.
    closed spec fn spec_viewed_allocation(&self) -> wcontainers_memory_shard::MemoryAllocationIdentity {
        wcontainers_memory_shard::MemoryAllocationIdentity(0)
    }

    // Every shard views its bytes from the first one, since this table neither splits nor offsets them.
    closed spec fn spec_view_start(&self) -> nat {
        0
    }

    closed spec fn spec_permissions(&self) -> MemoryPermissions {
        self.permissions
    }

    fn permissions(&self) -> (permissions: MemoryPermissions) {
        self.permissions
    }

    fn borrow(&self, offset: u64, length: u64) -> (borrowed: Result<Window<'_>, BorrowError>) {
        let cells = self.cells.as_slice();
        let size = cells.len() as u64;
        if offset > size || length > size - offset {
            return Err(BorrowError::OutsideShard);
        }
        let (_, rest) = cells.split_at(offset as usize);
        let (window, _) = rest.split_at(length as usize);
        Ok(Window { cells: window })
    }
}

// Returns where in the window a copy at the offset starts and how many bytes it moves for a buffer of the length.
fn range(window: &Window, offset: u64, buffer: usize) -> (range: (usize, usize))
    ensures
        range.1 == wcontainers_volatile_bytes::copied(window.spec_len(), offset, buffer as nat),
        range.0 + range.1 <= window.cells@.len(),
        window.cells@.len() <= usize::MAX,
{
    let length = window.cells.len();
    if offset >= length as u64 {
        return (usize::MIN, usize::MIN);
    }
    let start = offset as usize;
    let rest = length - start;
    if buffer <= rest {
        (start, buffer)
    } else {
        (start, rest)
    }
}

impl<'shard> VolatileBytes for Window<'shard> {
    closed spec fn spec_len(&self) -> nat {
        self.cells@.len()
    }

    fn len(&self) -> (length: u64) {
        self.cells.len() as u64
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> (count: usize) {
        let (start, count) = range(self, offset, into.len());
        let mut index: usize = usize::MIN;
        while index < count
            invariant
                count == wcontainers_volatile_bytes::copied(self.spec_len(), offset, old(into)@.len()),
                start + count <= self.cells@.len(),
                self.cells@.len() <= usize::MAX,
                into@.len() == old(into)@.len(),
                index <= count,
                forall|later: int| count <= later < old(into)@.len() ==> into@[later] == old(into)@[later],
            decreases count - index,
        {
            into[index] = self.cells[start + index].get();
            index += 1;
        }
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> (count: usize) {
        let (start, count) = range(self, offset, bytes.len());
        let mut index: usize = usize::MIN;
        while index < count
            invariant
                count == wcontainers_volatile_bytes::copied(self.spec_len(), offset, bytes@.len()),
                start + count <= self.cells@.len(),
                self.cells@.len() <= usize::MAX,
                count <= bytes@.len(),
                index <= count,
            decreases count - index,
        {
            self.cells[start + index].set(bytes[index]);
            index += 1;
        }
        count
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> (read: Result<usize, Sink::Error>) {
        let mut copy: Vec<u8> = Vec::new();
        while copy.len() < self.cells.len()
            invariant
                copy@.len() <= self.cells@.len(),
            decreases self.cells@.len() - copy@.len(),
        {
            copy.push(self.cells[copy.len()].get());
        }
        sink.write(copy.as_slice())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> (written: Result<usize, Source::Error>) {
        let mut copy: Vec<u8> = Vec::new();
        while copy.len() < self.cells.len()
            invariant
                copy@.len() <= self.cells@.len(),
            decreases self.cells@.len() - copy@.len(),
        {
            copy.push(u8::MIN);
        }
        let read = source.read(copy.as_mut_slice());
        if let Ok(count) = read {
            let mut index: usize = usize::MIN;
            while index < count
                invariant
                    count <= copy@.len(),
                    copy@.len() == self.cells@.len(),
                    index <= count,
                decreases count - index,
            {
                self.cells[index].set(copy[index]);
                index += 1;
            }
        }
        read
    }
}

} // verus!
