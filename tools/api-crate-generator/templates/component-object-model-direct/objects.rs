//! Our objects as the C side sees them, with no `unsafe` but `Table::new`. Each pointer the C side holds is the
//! address of one word of a table we never free, and that word holds a table of entry points for life. A pointer
//! comes back as an index by arithmetic and a range check, and the object is found in the index's slot, checked for
//! its type: a wrong, stale or misaligned pointer finds nothing, and a call that re-enters finds the object again.
//!
//! The glue trusts neither side here:
//! - No implementation code runs under any lock of this file: a slot's lock guards an index, a generation and an
//!   `Arc`, cloned or moved out before the lock drops; the value of an object is dropped after every lock, and its
//!   drop is caught if it panics, so that an implementation's `Drop` can neither hang nor abort the glue.
//! - The glue never asks an implementation's `PartialEq`, `Hash` or `Clone`: each object served is new, and an object
//!   handed out again (a child's device, the immediate context, a back buffer) comes from what its record holds.
//! - Every reference the glue itself holds is a `Counted`, tied to the generation of the word it counts, so that a
//!   program that over-releases cannot make the glue give back a reference of a later object in the same word.
//! - Everything that changes the table takes a `GlueKey`, which only generated glue holds.
//! - The table grows in chunks that double, none ever moved; a full table makes `serve` answer none.
//! - A word given back waits until `QUARANTINE` later words of its table were given back.

use core::any::Any;
use core::ops::Deref;
use core::panic::AssertUnwindSafe;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::collections::VecDeque;
use std::panic::catch_unwind;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock, PoisonError};

use wcontainers_component_object_model::structures::Guid;

use crate::IID_IUNKNOWN;
use crate::key::GlueKey;

/// The first chunk's words; each later chunk holds twice its predecessor's.
const FIRST_CHUNK: usize = 1 << 12;
/// How many chunks the table may have: 12 hold 16,773,120 objects at once, 2 hold 12,288.
#[cfg(not(feature = "small-table"))]
const CHUNKS: usize = 12;
#[cfg(feature = "small-table")]
const CHUNKS: usize = 2;
/// How many later words of the same table are given back before a word is used again.
#[cfg(not(feature = "small-table"))]
const QUARANTINE: usize = 1 << 16;
#[cfg(feature = "small-table")]
const QUARANTINE: usize = 1 << 10;

/// A table of entry points that the C side may call through, made only by the glue that defines it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Table(usize);

impl Table {
    /// Returns the table's handle for the object table's words.
    ///
    /// # Safety
    /// `T` is `#[repr(C)]` and lays out a COM interface's table exactly, and each of its entries checks the type of
    /// the object it finds, as the generated entries do.
    #[expect(unsafe_code, reason = "a table the C side calls through, whose layout is the caller's promise")]
    pub unsafe fn new<T>(entry_points: &'static T) -> Table {
        Table(core::ptr::from_ref(entry_points).expose_provenance())
    }
}

/// Returns the table that answers an interface for one type of value, none for an interface it does not answer;
/// IUnknown's table is the value's first word.
pub type Interfaces = fn(&Guid) -> Option<Table>;

/// What the glue records about an object when it first crosses, one part per API crate that reads it back.
pub type Record = Vec<Box<dyn Any + Send + Sync>>;

/// The address of one of our words, which the C side holds as an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Address(usize);

impl Address {
    pub fn get(self) -> usize {
        self.0
    }
}

/// Runs `body`, answering `failed` for a panic, whose payload is never dropped, since its drop could panic again.
pub fn guarded<R>(failed: R, body: impl FnOnce() -> R) -> R {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(answer) => answer,
        Err(payload) => {
            core::mem::forget(payload);
            failed
        }
    }
}

/// An implementation's value held by the glue outside the table, dropped as `Served` drops its value: caught.
pub struct Shield<T>(Option<T>);

impl<T> Shield<T> {
    pub fn new(value: T) -> Shield<T> {
        Shield(Some(value))
    }

    pub fn get(&self) -> &T {
        self.0.as_ref().expect("a shielded value lives until taken")
    }

    pub fn take(mut self) -> T {
        self.0.take().expect("a shielded value lives until taken")
    }
}

impl<T> Drop for Shield<T> {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            guarded((), move || drop(value));
        }
    }
}

/// A value of ours that the C side holds by pointer, the references it counts, and what the glue recorded.
pub struct Served<V> {
    /// Some until the drop, which takes it out to drop it under `guarded`.
    value: Option<V>,
    references: AtomicU32,
    record: Record,
    interfaces: Interfaces,
    /// IUnknown's word, which identity answers, and its table.
    first: (usize, Table),
    /// The value's other words, one per other table, made when the C side first asks for that interface.
    others: Mutex<Vec<(usize, Table)>>,
}

impl<V> Served<V> {
    pub fn value(&self) -> &V {
        self.value.as_ref().expect("a served value lives until its drop")
    }

    /// Returns the part of what the glue recorded that is of type `R`, if any.
    pub fn part<R: Any>(&self) -> Option<&R> {
        self.record.iter().find_map(|part| part.downcast_ref())
    }
}

impl<V> Drop for Served<V> {
    fn drop(&mut self) {
        // The value before the record, which may hold the object's device: a child dies before its device.
        let value = self.value.take();
        guarded((), move || drop(value));
        let record = core::mem::take(&mut self.record);
        guarded((), move || drop(record));
    }
}

type Entry = Arc<dyn Any + Send + Sync>;

/// Our object found for one call, which stays alive until this drops.
pub struct Found<V>(Arc<Served<V>>);

impl<V> Deref for Found<V> {
    type Target = Served<V>;

    fn deref(&self) -> &Served<V> {
        &self.0
    }
}

impl<V: Send + Sync + 'static> Found<V> {
    /// Returns a handle to the object's first word (IUnknown's), which finds the object again while any reference to it
    /// lives, as `Counted::weak` does, with no search of the table; none if the word no longer holds this object.
    pub fn weak(&self) -> Option<Weak> {
        let index = self.0.first.0;
        let slot = lock(OBJECTS.slot(index)?);
        let entry = slot.entry.as_ref()?;
        // The word holds this very object (the same allocation), so its generation is this object's.
        let same = core::ptr::addr_eq(Arc::as_ptr(entry), Arc::as_ptr(&self.0));
        same.then_some(Weak { index, generation: slot.generation, add: add_exact::<V>, release: release_exact::<V> })
    }
}

/// One word's object, and how many objects the word has held, which tells a later object from an earlier one.
#[derive(Default)]
struct Slot {
    entry: Option<Entry>,
    generation: u32,
}

/// One chunk: the words the C side sees as objects, and the slot of each.
struct Chunk {
    words: Box<[AtomicUsize]>,
    slots: Box<[Mutex<Slot>]>,
}

impl Chunk {
    fn new(length: usize) -> Chunk {
        Chunk { words: (0..length).map(|_| AtomicUsize::new(0)).collect(), slots: (0..length).map(|_| Mutex::default()).collect() }
    }
}

struct ObjectTable {
    chunks: [OnceLock<Chunk>; CHUNKS],
    /// The first index never used.
    unused: AtomicUsize,
    /// Indices given back, oldest first, by the table their word holds for life.
    released: Mutex<Vec<(Table, VecDeque<usize>)>>,
}

/// Our objects alive now, which a test reads.
static ALIVE: AtomicUsize = AtomicUsize::new(0);

/// Returns how many of our objects are alive now.
pub fn alive() -> usize {
    ALIVE.load(Ordering::Relaxed)
}

static OBJECTS: LazyLock<ObjectTable> = LazyLock::new(|| ObjectTable {
    chunks: core::array::from_fn(|_| OnceLock::new()),
    unused: AtomicUsize::new(0),
    released: Mutex::default(),
});

/// Returns the chunk and the offset in it of an index, or none past the last chunk.
fn locate(index: usize) -> Option<(usize, usize)> {
    let doubling = index / FIRST_CHUNK + 1;
    let chunk = (usize::BITS - 1 - doubling.leading_zeros()) as usize;
    let start = FIRST_CHUNK * ((1 << chunk) - 1);
    (chunk < CHUNKS).then_some((chunk, index - start))
}

impl ObjectTable {
    /// Returns the index of the word at this address, if it is one of ours.
    fn index(&self, address: usize) -> Option<usize> {
        self.chunks.iter().enumerate().find_map(|(at, chunk)| {
            let words = &chunk.get()?.words;
            let offset = address.checked_sub(words.as_ptr().addr())?;
            let word = offset / size_of::<AtomicUsize>();
            let inside = offset % size_of::<AtomicUsize>() == 0 && word < words.len();
            inside.then(|| FIRST_CHUNK * ((1 << at) - 1) + word)
        })
    }

    fn slot(&self, index: usize) -> Option<&Mutex<Slot>> {
        let (chunk, offset) = locate(index)?;
        Some(&self.chunks[chunk].get()?.slots[offset])
    }

    /// Returns the address the C side holds for the word, its provenance exposed for C to use.
    fn address(&self, index: usize) -> Option<Address> {
        let (chunk, offset) = locate(index)?;
        let word = &self.chunks[chunk].get()?.words[offset];
        Some(Address(core::ptr::from_ref(word).expose_provenance()))
    }

    /// Returns a word holding this table: the oldest given back once its quarantine is over, else a new one; none
    /// when the table of objects is full.
    fn claim(&self, table: Table) -> Option<usize> {
        let reused = {
            let mut released = lock(&self.released);
            let queue = released.iter_mut().find(|(other, _)| *other == table).map(|(_, queue)| queue);
            queue.filter(|queue| queue.len() > QUARANTINE).and_then(VecDeque::pop_front)
        };
        if reused.is_some() {
            return reused;
        }
        let index = self.unused.fetch_add(1, Ordering::Relaxed);
        let (chunk, offset) = locate(index)?;
        let chunk = self.chunks[chunk].get_or_init(|| Chunk::new(FIRST_CHUNK << chunk));
        chunk.words[offset].store(table.0, Ordering::Release);
        Some(index)
    }

    /// Gives a word back, to wait in its table's quarantine.
    fn give_back(&self, index: usize, table: Table) {
        let mut released = lock(&self.released);
        match released.iter_mut().find(|(other, _)| *other == table) {
            Some((_, queue)) => queue.push_back(index),
            None => released.push((table, VecDeque::from([index]))),
        }
    }

    /// Puts an object in a free word and returns its generation, or gives the entry back if the word is taken.
    fn install(&self, index: usize, entry: Entry) -> Result<u32, Entry> {
        let Some(slot) = self.slot(index) else { return Err(entry) };
        let mut slot = lock(slot);
        if slot.entry.is_some() {
            return Err(entry);
        }
        slot.generation = slot.generation.wrapping_add(1);
        slot.entry = Some(entry);
        Ok(slot.generation)
    }

    fn find<V: Send + Sync + 'static>(&self, index: usize) -> Option<Found<V>> {
        let entry = lock(self.slot(index)?).entry.clone()?;
        entry.downcast::<Served<V>>().ok().map(Found)
    }

    fn take(&self, index: usize) -> Option<Entry> {
        lock(self.slot(index)?).entry.take()
    }
}

/// Adds a reference to an object the C side still holds, and returns the new count, or zero when it holds none.
fn add<V>(served: &Served<V>) -> u32 {
    let added = served.references.try_update(Ordering::AcqRel, Ordering::Acquire, |n| (n > 0).then(|| n.checked_add(1)).flatten());
    added.map_or(0, |n| n + 1)
}

/// Adds a reference to the object of type `V` in word `index`, if the word still holds generation `generation`.
fn add_exact<V: Send + Sync + 'static>(index: usize, generation: u32) -> u32 {
    let Some(slot) = OBJECTS.slot(index) else { return 0 };
    let slot = lock(slot);
    let served = slot.entry.as_ref().filter(|_| slot.generation == generation).and_then(|entry| entry.downcast_ref::<Served<V>>());
    served.map_or(0, add)
}

/// Gives back a reference to the object of type `V` in word `index`, if the word holds `generation` (or any, for
/// none), and returns the count left; the last one gives back every word of the object and drops it after every lock.
fn release_exact<V: Send + Sync + 'static>(index: usize, generation: Option<u32>) -> u32 {
    let table: &ObjectTable = &OBJECTS;
    let Some(slot) = table.slot(index) else { return 0 };
    let last = {
        let mut slot = lock(slot);
        if generation.is_some_and(|generation| generation != slot.generation) {
            return 0;
        }
        let Some(served) = slot.entry.as_ref().and_then(|entry| entry.downcast_ref::<Served<V>>()) else { return 0 };
        match served.references.try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1)) {
            Err(_) => return 0,
            Ok(before) if before > 1 => return before - 1,
            Ok(_) => {}
        }
        slot.entry.take()
    };
    let Some(served) = last.and_then(|entry| entry.downcast::<Served<V>>().ok()) else { return 0 };
    // The first word's entry was taken above (when `index` is it); the other words' entries now, with no allocation for
    // an object that answered no other interface.
    let others = core::mem::take(&mut *lock(&served.others));
    let first_taken = table.take(served.first.0);
    let taken: Vec<Option<Entry>> = others.iter().map(|&(word, _)| table.take(word)).collect();
    table.give_back(served.first.0, served.first.1);
    others.into_iter().for_each(|(word, word_table)| table.give_back(word, word_table));
    // Outside every lock; `Served`'s drop catches a panic of the value's.
    drop((first_taken, taken));
    drop(served);
    ALIVE.fetch_sub(1, Ordering::Relaxed);
    0
}

/// A reference the glue holds to one of our objects: given back when this drops, unless handed to the C side.
pub struct Counted {
    index: usize,
    generation: u32,
    add: fn(usize, u32) -> u32,
    release: fn(usize, Option<u32>) -> u32,
}

impl Counted {
    /// Returns the address of the word this counts.
    pub fn address(&self) -> usize {
        OBJECTS.address(self.index).map_or(0, Address::get)
    }

    /// Returns another reference the glue holds to the same object.
    pub fn again(&self) -> Option<Counted> {
        self.weak().upgrade()
    }

    /// Returns a handle that finds the object again while any reference to it lives.
    pub fn weak(&self) -> Weak {
        Weak { index: self.index, generation: self.generation, add: self.add, release: self.release }
    }

    /// Hands this reference to the C side, which gives it back by Release.
    pub fn into_address(self) -> Address {
        let address = Address(self.address());
        core::mem::forget(self);
        address
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        (self.release)(self.index, Some(self.generation));
    }
}

/// A handle to one of our objects that holds no reference: it finds the object while it lives, never a later one.
#[derive(Clone, Copy)]
pub struct Weak {
    index: usize,
    generation: u32,
    add: fn(usize, u32) -> u32,
    release: fn(usize, Option<u32>) -> u32,
}

impl Weak {
    /// Returns a new reference to the object, or none once it has died.
    pub fn upgrade(&self) -> Option<Counted> {
        let Weak { index, generation, add, release } = *self;
        (add(index, generation) > 0).then_some(Counted { index, generation, add, release })
    }
}

/// Returns our object of type `V` at this address, or none for a wrong or stale address or another type.
pub fn object<V: Send + Sync + 'static>(_key: GlueKey, address: usize) -> Option<Found<V>> {
    let table: &ObjectTable = &OBJECTS;
    table.find(table.index(address)?)
}

/// Reads a part of what the glue recorded for our object of type `V` at this address, under the word's lock and with
/// no reference taken; none for a wrong or stale address, another type or no such part. `read` runs no code but the
/// glue's own: a record's parts are the glue's types.
pub fn peek<V: Send + Sync + 'static, P: Any, T>(_key: GlueKey, address: usize, read: impl FnOnce(&P) -> Option<T>) -> Option<T> {
    let table: &ObjectTable = &OBJECTS;
    let index = table.index(address)?;
    let slot = lock(table.slot(index)?);
    let served = slot.entry.as_ref()?.downcast_ref::<Served<V>>()?;
    read(served.part::<P>()?)
}

/// Serves a new object in a free word, with one reference the glue holds, recording `record` and answering by
/// `interfaces`; none when the table is full (the value is dropped, caught).
pub fn serve<V: Send + Sync + 'static>(_key: GlueKey, value: V, record: Record, interfaces: Interfaces) -> Option<Counted> {
    let table: &ObjectTable = &OBJECTS;
    let Some(first_table) = interfaces(&IID_IUNKNOWN) else {
        guarded((), move || drop((value, record)));
        return None;
    };
    let Some(index) = table.claim(first_table) else {
        guarded((), move || drop((value, record)));
        return None;
    };
    let served = Served {
        value: Some(value),
        references: AtomicU32::new(1),
        record,
        interfaces,
        first: (index, first_table),
        others: Mutex::default(),
    };
    let generation = table.install(index, Arc::new(served)).ok()?;
    ALIVE.fetch_add(1, Ordering::Relaxed);
    Some(Counted { index, generation, add: add_exact::<V>, release: release_exact::<V> })
}

/// Returns a reference the glue holds to our object of type `V` at this address, or none.
pub fn counted<V: Send + Sync + 'static>(_key: GlueKey, address: usize) -> Option<Counted> {
    let table: &ObjectTable = &OBJECTS;
    let index = table.index(address)?;
    let slot = lock(table.slot(index)?);
    let served = slot.entry.as_ref()?.downcast_ref::<Served<V>>()?;
    (add(served) > 0).then(|| Counted { index, generation: slot.generation, add: add_exact::<V>, release: release_exact::<V> })
}

/// Returns a reference to the word answering `interface` for our object of type `V` at this address, making the word
/// on first use; none for an interface it does not answer or a full table.
pub fn query<V: Send + Sync + 'static>(key: GlueKey, address: usize, interface: &Guid) -> Option<Counted> {
    let table: &ObjectTable = &OBJECTS;
    let found = object::<V>(key, address)?;
    let wanted = (found.interfaces)(interface)?;
    let (first, first_table) = found.first;
    if wanted == first_table {
        return counted::<V>(key, table.address(first)?.get());
    }
    let index = {
        let mut others = lock(&found.others);
        match others.iter().find(|&&(_, other)| other == wanted) {
            Some(&(index, _)) => index,
            None => {
                let index = table.claim(wanted)?;
                let entry: Entry = Arc::clone(&found.0) as Entry;
                if let Err(entry) = table.install(index, entry) {
                    drop(others);
                    drop(entry);
                    return None;
                }
                others.push((index, wanted));
                index
            }
        }
    };
    drop(found);
    counted::<V>(key, table.address(index)?.get())
}

/// Adds a reference for the C side to our object of type `V`, and returns the new count, zero for a wrong or stale
/// address.
pub fn add_reference<V: Send + Sync + 'static>(_key: GlueKey, address: usize) -> u32 {
    let table: &ObjectTable = &OBJECTS;
    let Some(slot) = table.index(address).and_then(|index| table.slot(index)) else { return 0 };
    let slot = lock(slot);
    slot.entry.as_ref().and_then(|entry| entry.downcast_ref::<Served<V>>()).map_or(0, add)
}

/// Gives back a reference the C side held to our object of type `V`, and returns the count left.
pub fn release<V: Send + Sync + 'static>(_key: GlueKey, address: usize) -> u32 {
    OBJECTS.index(address).map_or(0, |index| release_exact::<V>(index, None))
}

/// Returns the guard of a lock, taking it over from a thread that panicked while holding it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
