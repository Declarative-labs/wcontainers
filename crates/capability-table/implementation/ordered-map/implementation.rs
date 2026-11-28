//! A capability table in the standard library's ordered map from each identifier to a shared reference to a thing.
//!
//! A thing is made from its config by `TryFrom` straight into the map, and a duplicate is a second reference to it, so
//! no number stands for a thing apart from its entries and none can come to name a thing made later.

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use thiserror::Error;
use vstd::cell::invcell::InvCell;
use vstd::predicate::Predicate;
use vstd::prelude::*;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_capability_table::{CapabilityTable, CreateError, DuplicateError};
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

broadcast use vstd::std_specs::btree::group_btree_axioms;

/// A table of capabilities in an ordered map from each identifier to a shared thing made from a config.
pub struct OrderedMapCapabilityTable<Identifier: Ord, Config, Thing> {
    entries: BTreeMap<Identifier, Shared<Thing>>,
    configs: PhantomData<Config>,
}

/// A capability to a thing, which only its table can duplicate, so that none outlives every entry holding it.
pub struct Shared<Thing> {
    thing: Rc<Thing>,
}

// Returns a second capability to the same thing.
fn duplicate<Thing>(capability: &Shared<Thing>) -> (duplicate: Shared<Thing>)
    ensures
        duplicate == *capability,
{
    Shared { thing: Rc::clone(&capability.thing) }
}

impl<Identifier: Ord, Config, Thing> OrderedMapCapabilityTable<Identifier, Config, Thing> {
    // The order laws under which the standard library's map is specified, which identifiers obey, and the map's own.
    #[verifier::type_invariant]
    spec fn ordered(self) -> bool {
        &&& vstd::laws_cmp::obeys_cmp::<Identifier>()
        &&& vstd::laws_cmp::obeys_cmp_concrete_eq::<Identifier>()
        &&& vstd::std_specs::btree::btree_map_inv(&self.entries)
    }
}

// Returns the table's map, leaving an empty one in its place, so that the map can change under no borrow of a field.
fn take<Identifier: Ord, Config, Thing>(table: &mut OrderedMapCapabilityTable<Identifier, Config, Thing>) -> (entries:
    BTreeMap<Identifier, Shared<Thing>>)
    ensures
        entries@ == old(table).entries@,
        vstd::laws_cmp::obeys_cmp::<Identifier>(),
        vstd::laws_cmp::obeys_cmp_concrete_eq::<Identifier>(),
        vstd::std_specs::btree::btree_map_inv(&entries),
{
    proof {
        use_type_invariant(&*table);
    }
    let mut taken = OrderedMapCapabilityTable { entries: BTreeMap::new(), configs: PhantomData };
    core::mem::swap(table, &mut taken);
    proof {
        use_type_invariant(&taken);
    }
    taken.entries
}

// Returns the table of a map whose keys obey the order laws.
fn wrap<Identifier: Ord, Config, Thing>(entries: BTreeMap<Identifier, Shared<Thing>>) -> (table:
    OrderedMapCapabilityTable<Identifier, Config, Thing>)
    requires
        vstd::laws_cmp::obeys_cmp::<Identifier>(),
        vstd::laws_cmp::obeys_cmp_concrete_eq::<Identifier>(),
        vstd::std_specs::btree::btree_map_inv(&entries),
    ensures
        table.entries@ == entries@,
{
    OrderedMapCapabilityTable { entries, configs: PhantomData }
}

impl<Config, Thing> Default for OrderedMapCapabilityTable<u64, Config, Thing> {
    // Returns a table holding nothing; only u64 identifiers, whose order laws vstd proves, can make one.
    fn default() -> (table: Self) {
        OrderedMapCapabilityTable { entries: BTreeMap::new(), configs: PhantomData }
    }
}

impl<Identifier: Ord, Config, Thing: TryFrom<Config>> CapabilityTable<Identifier, Shared<Thing>> for
    OrderedMapCapabilityTable<Identifier, Config, Thing> {
    type Config = Config;

    type Error = <Thing as TryFrom<Config>>::Error;

    closed spec fn state(&self) -> Map<Identifier, Shared<Thing>> {
        self.entries@
    }

    fn create(&mut self, identifier: Identifier, config: Config) -> (created: Result<
        (),
        CreateError<Self::Error>,
    >) {
        proof {
            use_type_invariant(&*self);
        }
        if self.entries.contains_key(&identifier) {
            return Err(CreateError::IdentifierTaken);
        }
        let thing = match Thing::try_from(config) {
            Ok(thing) => thing,
            Err(error) => return Err(CreateError::Failed(error)),
        };
        let ghost key = identifier;
        let mut entries = take(self);
        let _ = entries.insert(identifier, Shared { thing: Rc::new(thing) });
        proof {
            assert(entries@.remove(key) =~= old(self).entries@);
        }
        *self = wrap(entries);
        Ok(())
    }

    fn get(&self, identifier: &Identifier) -> (capability: Option<&Shared<Thing>>) {
        proof {
            use_type_invariant(self);
        }
        self.entries.get(identifier)
    }

    fn drop(&mut self, identifier: &Identifier) {
        let mut entries = take(self);
        let _ = entries.remove(identifier);
        *self = wrap(entries);
    }

    fn duplicate_within(&mut self, identifier: &Identifier, new_identifier: Identifier) -> (duplicated:
        Result<(), DuplicateError<Self::Error>>) {
        proof {
            use_type_invariant(&*self);
        }
        let duplicate = match self.entries.get(identifier) {
            Some(capability) => duplicate(capability),
            None => return Err(DuplicateError::NothingHeld),
        };
        if self.entries.contains_key(&new_identifier) {
            return Err(DuplicateError::IdentifierTaken);
        }
        let mut entries = take(self);
        let _ = entries.insert(new_identifier, duplicate);
        *self = wrap(entries);
        Ok(())
    }

    fn duplicate_from(
        &mut self,
        other: &Self,
        identifier: &Identifier,
        new_identifier: Identifier,
    ) -> (duplicated: Result<(), DuplicateError<Self::Error>>) {
        proof {
            use_type_invariant(&*self);
            use_type_invariant(other);
        }
        let duplicate = match other.entries.get(identifier) {
            Some(capability) => duplicate(capability),
            None => return Err(DuplicateError::NothingHeld),
        };
        if self.entries.contains_key(&new_identifier) {
            return Err(DuplicateError::IdentifierTaken);
        }
        let mut entries = take(self);
        let _ = entries.insert(new_identifier, duplicate);
        *self = wrap(entries);
        Ok(())
    }
}

impl<Thing: VolatileBytes> VolatileBytes for Shared<Thing> {
    closed spec fn spec_len(&self) -> nat {
        self.thing.spec_len()
    }

    fn len(&self) -> (length: u64) {
        self.thing.len()
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> (count: usize) {
        self.thing.read_at(offset, into)
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> (count: usize) {
        self.thing.write_at(offset, bytes)
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> (read: Result<usize, Sink::Error>) {
        self.thing.read_to(sink)
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> (written: Result<usize, Source::Error>) {
        self.thing.write_from(source)
    }
}

/// The length of a WebAssembly page, the most bytes a page holds.
///
/// https://webassembly.github.io/spec/core/exec/runtime.html#page-size
const WEBASSEMBLY_PAGE_LENGTH: u64 = 65536;

/// A page of bytes, each in a cell that a shared borrow may write, made from its length.
pub struct Page {
    bytes: Vec<InvCell<u8, AnyByte>>,
}

// The rule every byte of a page meets, which any byte does.
struct AnyByte;

impl Predicate<u8> for AnyByte {
    open spec fn predicate(&self, byte: u8) -> bool {
        true
    }
}

/// Why a page could not be made.
#[derive(Debug, Error)]
#[verifier::external_derive]
pub enum PageError {
    #[error("the length is more than a WebAssembly page holds")]
    TooLong,
}

// Returns a page of the length, every byte zero, or `TooLong` past a WebAssembly page.
fn make(length: u64) -> (made: Result<Page, PageError>)
    ensures
        made matches Ok(page) ==> page.spec_len() == length,
        made is Err <==> length > WEBASSEMBLY_PAGE_LENGTH,
{
    if length > WEBASSEMBLY_PAGE_LENGTH {
        return Err(PageError::TooLong);
    }
    let mut bytes: Vec<InvCell<u8, AnyByte>> = Vec::new();
    let mut made: u64 = u64::MIN;
    while made < length
        invariant
            made <= length,
            length <= WEBASSEMBLY_PAGE_LENGTH,
            bytes@.len() == made,
        decreases length - made,
    {
        bytes.push(InvCell::new(u8::MIN, Ghost(AnyByte)));
        made += 1;
    }
    Ok(Page { bytes })
}

// Returns where in the page a copy at the offset starts and how many bytes it moves for a buffer of the length.
fn range(page: &Page, offset: u64, buffer: usize) -> (range: (usize, usize))
    ensures
        range.1 == wcontainers_volatile_bytes::copied(page.spec_len(), offset, buffer as nat),
        range.0 + range.1 <= page.bytes@.len(),
        range.1 > 0 ==> range.0 == offset,
        page.bytes@.len() <= usize::MAX,
{
    let length = page.bytes.len();
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

impl VolatileBytes for Page {
    closed spec fn spec_len(&self) -> nat {
        self.bytes@.len()
    }

    fn len(&self) -> (length: u64) {
        self.bytes.len() as u64
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> (count: usize) {
        let (start, count) = range(self, offset, into.len());
        let mut index: usize = usize::MIN;
        while index < count
            invariant
                count == wcontainers_volatile_bytes::copied(self.spec_len(), offset, old(into)@.len()),
                start + count <= self.bytes@.len(),
                self.bytes@.len() <= usize::MAX,
                into@.len() == old(into)@.len(),
                index <= count,
                forall|later: int| count <= later < old(into)@.len() ==> into@[later] == old(into)@[later],
            decreases count - index,
        {
            into[index] = self.bytes[start + index].get();
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
                start + count <= self.bytes@.len(),
                self.bytes@.len() <= usize::MAX,
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
        let mut index: usize = usize::MIN;
        while index < self.bytes.len()
            invariant
                index <= self.bytes@.len(),
                copy@.len() == index,
            decreases self.bytes@.len() - index,
        {
            copy.push(self.bytes[index].get());
            index += 1;
        }
        sink.write(copy.as_slice())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> (written: Result<usize, Source::Error>) {
        let mut copy: Vec<u8> = Vec::new();
        while copy.len() < self.bytes.len()
            invariant
                copy@.len() <= self.bytes@.len(),
            decreases self.bytes@.len() - copy@.len(),
        {
            copy.push(u8::MIN);
        }
        let read = source.read(copy.as_mut_slice());
        if let Ok(count) = read {
            let mut index: usize = usize::MIN;
            while index < count
                invariant
                    count <= copy@.len(),
                    copy@.len() == self.bytes@.len(),
                    index <= count,
                decreases count - index,
            {
                self.bytes[index].set(copy[index]);
                index += 1;
            }
        }
        read
    }
}

} // verus!

// TODO: prove it. Verus proves an impl of `TryFrom` only beside one of `TryFromSpecImpl`, which vstd declares only when
// Verus compiles, so a crate that also builds without Verus cannot write it; this impl is one call of the proven `make`.
impl TryFrom<u64> for Page {
    type Error = PageError;

    fn try_from(length: u64) -> Result<Page, PageError> {
        make(length)
    }
}
