//! Guest memory as owned pages, shared between spaces only where a mapping shares them.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut, Range};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use creusot_std::logic::FMap;
use creusot_std::prelude::{
    Ghost, Int, Resolve, Seq, Snapshot, check, ensures, ghost, invariant, logic, opaque, proof_assert, requires, snapshot,
    trusted,
};
use wcontainers_abi::{PAGE_MASK, PAGE_SIZE, Permissions};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_executor::memory_space::{
    self, CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace,
};
use wcontainers_processor_architecture::{MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, InstructionFetch, PagePermission,
};
use wcontainers_volatile_bytes::VolatileBytes;

const PAGE_BYTES: usize = PAGE_SIZE as usize;

/// The number of changes made to any space, counted after each change is in place.
///
/// One count serves every space, since a mapping shares pages between spaces: a copy taken at a count is current while
/// the count stays.
// Trusted: Creusot cannot translate a `static`; only the trusted `count_change` and `refresh` use it.
#[trusted]
static CHANGES: AtomicU64 = AtomicU64::new(u64::MIN);

/// A space as the architecture's memory changes, kept current between instructions without copying it again.
#[derive(Default)]
pub(crate) struct SpaceCopy {
    /// The count of changes the copy is current at; None until taken, or once a change it cannot follow is made.
    changes: Option<u64>,
    deltas: Vec<MemoryDelta<'static, Arm64>>,
    /// The place in the deltas of each page's bytes, by the page's first address.
    page_bytes: BTreeMap<u64, usize>,
}

impl SpaceCopy {
    pub(crate) fn deltas(&self) -> &[MemoryDelta<'static, Arm64>] {
        &self.deltas
    }

    /// Writes the bytes into the copy's pages, adding a page of zeros where the copy had no bytes.
    fn write_at(&mut self, start: u64, bytes: &[u8]) {
        for (page, within, buffer) in split_at_pages(start, bytes.len()) {
            let deltas = &mut self.deltas;
            let place = *self.page_bytes.entry(page).or_insert_with(|| {
                deltas.push(MemoryDelta::ValueAssignment {
                    start: page,
                    bytes: Cow::Owned(vec![u8::MIN; PAGE_BYTES]),
                });
                deltas.len() - 1
            });
            if let Some(MemoryDelta::ValueAssignment {
                bytes: page_bytes, ..
            }) = self.deltas.get_mut(place)
            {
                page_bytes.to_mut()[within].copy_from_slice(&bytes[buffer]);
            }
        }
    }
}

/// The bytes of one page, held by every space that maps it.
type Frame = Arc<Mutex<Box<[u8]>>>;

/// One page of a space; a page with no entry reads as zeros and allows nothing.
#[derive(Clone)]
struct Page {
    /// None while the page reads as zeros and no space shares it.
    frame: Option<Frame>,
    /// Whether this space mapped the frame from another, so that a copy of the page shares it too and an unmap ends it.
    mapped: bool,
    permissions: Permissions,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            frame: None,
            mapped: false,
            permissions: Permissions::empty(),
        }
    }
}

/// A guest's address space, every page of it owned or shared in host memory.
///
/// The page methods (`set_permissions`, `map_from`, `unmap_from` and `copy_from`) take whole pages: addresses that
/// do not start and end at a page boundary are refused. A copy copies the bytes at once rather than on a write.
#[derive(Default)]
pub struct SimulatedMemory {
    /// The pages that have bytes, a mapping or permissions, by their first address.
    pages: Mutex<BTreeMap<u64, Page>>,
}

/// Addresses of a [`SimulatedMemory`], which another thread's guest may be changing while they are held.
pub struct SimulatedBytes<'space> {
    space: &'space SimulatedMemory,
    addresses: Range<u64>,
}

impl SimulatedMemory {
    /// Takes the copy anew unless no space changed since it was taken.
    // Trusted: Creusot cannot translate a `static`; this belongs to the executor, outside the memory contract.
    #[trusted]
    pub(crate) fn refresh(&self, copy: &mut SpaceCopy) {
        let changes = CHANGES.load(Ordering::Acquire);
        if copy.changes != Some(changes) {
            *copy = self.take_copy(Some(changes));
        }
    }

    /// Applies a change an instruction made, whatever the permissions, to the space and to the copy it ran on.
    ///
    /// The copy stays current only if no other change came between: otherwise it is taken anew at the next refresh.
    // Trusted: Creusot has no iterator contract for `step_by`; this belongs to the executor, outside the memory contract.
    #[trusted]
    pub(crate) fn apply(&self, delta: MemoryDelta<'_, Arm64>, copy: &mut SpaceCopy) {
        match delta {
            MemoryDelta::ValueAssignment { start, bytes } => {
                let before = self.write_counted(start, &bytes);
                if copy.changes == Some(before) {
                    copy.write_at(start, &bytes);
                    copy.changes = Some(before.wrapping_add(1));
                } else {
                    copy.changes = None;
                }
            }
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => {
                let mut pages = self.lock_pages();
                for start in page_starts(&addresses) {
                    pages.entry(start).or_default().permissions = to_permissions(permission);
                }
                count_change();
                copy.changes = None;
            }
        }
    }

    /// Returns the copy of every page, current at the count of changes given (read before the pages are).
    // Trusted: Creusot has no iterator contract for a `BTreeMap`; this belongs to the executor, outside the memory contract.
    #[trusted]
    fn take_copy(&self, changes: Option<u64>) -> SpaceCopy {
        let pages = self.lock_pages();
        let mut copy = SpaceCopy {
            changes,
            ..SpaceCopy::default()
        };
        for (&start, page) in pages.iter() {
            if let Some(permission) = to_page_permission(page.permissions) {
                copy.deltas.push(MemoryDelta::PermissionAssignment {
                    addresses: start..start + PAGE_SIZE,
                    permission,
                });
            }
            if let Some(frame) = &page.frame {
                copy.page_bytes.insert(start, copy.deltas.len());
                copy.deltas.push(MemoryDelta::ValueAssignment {
                    start,
                    bytes: Cow::Owned(lock_frame(frame).to_vec()),
                });
            }
        }
        copy
    }

    fn lock_pages(&self) -> MutexGuard<'_, BTreeMap<u64, Page>> {
        self.pages.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn find_frame(&self, page: u64) -> Option<Frame> {
        self.lock_pages()
            .get(&page)
            .and_then(|found| found.frame.clone())
    }

    /// Returns the frame of the page, given one of zeros if it had none.
    fn make_frame(&self, page: u64) -> Frame {
        self.lock_pages()
            .entry(page)
            .or_default()
            .frame
            .get_or_insert_with(new_frame)
            .clone()
    }

    /// Returns the number of bytes read, starting at the address.
    fn read_at(&self, start: u64, into: &mut [u8]) -> usize {
        for (page, within, buffer) in split_at_pages(start, into.len()) {
            match self.find_frame(page) {
                Some(frame) => into[buffer].copy_from_slice(&lock_frame(&frame)[within]),
                None => into[buffer].fill(u8::MIN),
            }
        }
        into.len()
    }

    /// Returns the number of bytes written, starting at the address.
    fn write_at(&self, start: u64, bytes: &[u8]) -> usize {
        self.write_counted(start, bytes);
        bytes.len()
    }

    /// Writes the bytes, starting at the address, and returns the count of changes before the write.
    fn write_counted(&self, start: u64, bytes: &[u8]) -> u64 {
        for (page, within, buffer) in split_at_pages(start, bytes.len()) {
            lock_frame(&self.make_frame(page))[within].copy_from_slice(&bytes[buffer]);
        }
        count_change()
    }
}

impl MemorySpace for SimulatedMemory {
    type Bytes<'space> = SimulatedBytes<'space>;
    type Holder = Tables;
    type Frame = Frame;

    #[logic]
    fn held(self, holder: Tables) -> bool {
        pearlite! { holder.holds(self) && holder.sound() }
    }

    #[logic]
    fn page(self, holder: Tables, start: Int) -> memory_space::Page<Frame> {
        pearlite! {
            if start % PAGE_SIZE@ == 0 && 0 <= start { model(page_in(holder.table(self), start)) } else { model(blank()) }
        }
    }

    #[logic]
    fn addressable() -> Range<Int> {
        pearlite! { Range { start: Arm64::ADDRESSABLE.start@, end: Arm64::ADDRESSABLE.end@ } }
    }

    #[ensures(match result { Ok(_) => Self::inside(addresses.start@, addresses.end@), Err(_) => !Self::inside(addresses.start@, addresses.end@) })]
    fn borrow(&self, addresses: Range<u64>) -> Result<SimulatedBytes<'_>, InvalidAddressError> {
        check_inside(&addresses)?;
        Ok(SimulatedBytes {
            space: self,
            addresses,
        })
    }

    #[requires(self.held(**holder))]
    #[ensures(Self::still_held(**holder, ^*holder))]
    #[ensures(match result {
        Ok(()) => Self::whole_pages(addresses.start@, addresses.end@)
            && Self::permissions_set(**holder, ^*holder, *self, addresses, allowed),
        Err(_) => !Self::whole_pages(addresses.start@, addresses.end@) && ^*holder == **holder,
    })]
    fn set_permissions(
        &self,
        addresses: Range<u64>,
        allowed: Permissions,
        holder: Ghost<&mut Tables>,
    ) -> Result<(), InvalidAddressError> {
        check_pages(&addresses)?;
        let before = snapshot!(**holder);
        proof_assert!(table_sound((*before).table(*self)));
        let mut pages = self.lock_held(holder, snapshot!(Seq::empty()));
        // Changed: one borrow of the table for the whole loop, so that Creusot keeps what the table will hold when unlocked.
        let table = &mut *pages;
        let found = snapshot!(entries(*table));
        proof_assert!(written_since((*before).table(*self), *found, *before, Seq::empty()));
        proof_assert!(table_sound(*found));
        // Changed: a loop over the page starts, for which Creusot needs no iterator contract.
        let mut start = addresses.start;
        #[invariant(addresses.start@ <= start@ && start@ <= addresses.end@ && start@ % PAGE_SIZE@ == 0)]
        #[invariant(forall<key: Int> page_in(entries(*table), key) == if addresses.start@ <= key && key < start@ && key % PAGE_SIZE@ == 0 {
            Page { frame: page_in(*found, key).frame, mapped: page_in(*found, key).mapped, permissions: allowed }
        } else {
            page_in(*found, key)
        })]
        while start < addresses.end {
            page_at(table, start).permissions = allowed;
            start += PAGE_SIZE;
        }
        proof_assert!(table_sound(entries(*table)));
        count_change();
        Ok(())
    }

    #[requires(self.held(**holder) && other.held(**holder))]
    #[ensures(Self::still_held(**holder, ^*holder))]
    #[ensures(match result {
        Ok(()) => Self::whole_pages(addresses.start@, addresses.end@)
            && Self::whole_pages(other_start@, other_start@ + addresses.end@ - addresses.start@)
            && Self::frames_shared(**holder, ^*holder, *self, addresses, *other, other_start),
        Err(MapMemoryError::InvalidAddress(_)) => ^*holder == **holder
            && !(Self::whole_pages(addresses.start@, addresses.end@)
                && Self::whole_pages(other_start@, other_start@ + addresses.end@ - addresses.start@)),
        Err(MapMemoryError::OutOfMemory) => ^*holder == **holder,
    })]
    fn map_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
        mut holder: Ghost<&mut Tables>,
    ) -> Result<(), MapMemoryError> {
        // Changed: matches rather than `map_err` with the variant as a function, whose result Creusot does not know.
        if let Err(error) = check_pages(&addresses) {
            return Err(MapMemoryError::InvalidAddress(error));
        }
        let other_addresses = match shift(&addresses, other_start) {
            Ok(other_addresses) => other_addresses,
            Err(error) => return Err(MapMemoryError::InvalidAddress(error)),
        };
        let before = snapshot!(**holder);
        // Changed: the other space's frames are made in one locked loop rather than one lock per page.
        let mut frames: Vec<Frame> = Vec::new();
        let mut left = snapshot!(FMap::empty());
        {
            let mut other_pages = other.lock_held(ghost!(&mut **holder), snapshot!(Seq::empty()));
            // Changed: one borrow of the table for the whole loop, so that Creusot keeps what the table will hold when unlocked.
            let table = &mut *other_pages;
            let found = snapshot!(entries(*table));
            proof_assert!(table_sound(*found));
            let mut start = other_addresses.start;
            #[invariant(other_addresses.start@ <= start@ && start@ <= other_addresses.end@ && start@ % PAGE_SIZE@ == 0)]
            #[invariant(frames@.len() * PAGE_SIZE@ == start@ - other_addresses.start@)]
            #[invariant(forall<i: Int> 0 <= i && i < frames@.len() ==>
                page_in(entries(*table), other_addresses.start@ + i * PAGE_SIZE@).frame == Some(frames@[i])
                && (page_in(*found, other_addresses.start@ + i * PAGE_SIZE@).frame == Some(frames@[i])
                    || page_in(*found, other_addresses.start@ + i * PAGE_SIZE@).frame == None && new_to(*before, frames@[i])
                        && !shows(*found, frames@[i])))]
            #[invariant(forall<key: Int> if other_addresses.start@ <= key && key < start@ && key % PAGE_SIZE@ == 0 {
                page_in(entries(*table), key).mapped == page_in(*found, key).mapped
                && page_in(entries(*table), key).permissions == page_in(*found, key).permissions
                && (page_in(*found, key).frame != None ==> page_in(entries(*table), key).frame == page_in(*found, key).frame)
                && page_in(entries(*table), key).frame != None
            } else {
                page_in(entries(*table), key) == page_in(*found, key)
            })]
            while start < other_addresses.end {
                let now = snapshot!(entries(*table));
                let page = page_at(table, start);
                let frame = match &page.frame {
                    Some(frame) => Arc::clone(frame),
                    None => {
                        let frame = new_frame_for(before, now, snapshot!(frames@));
                        page.frame = Some(Arc::clone(&frame));
                        frame
                    }
                };
                frames.push(frame);
                start += PAGE_SIZE;
            }
            proof_assert!(written_since((*before).table(*other), *found, *before, Seq::empty()));
            proof_assert!(forall<i: Int> 0 <= i && i < frames@.len() ==>
                page_in((*before).table(*other), other_addresses.start@ + i * PAGE_SIZE@).frame == Some(frames@[i])
                || page_in((*before).table(*other), other_addresses.start@ + i * PAGE_SIZE@).frame == None
                    && new_to(*before, frames@[i]));
            left = snapshot!(entries(*table));
            proof_assert!(forall<key: Int> other_addresses.start@ <= key && key < other_addresses.end@ && key % PAGE_SIZE@ == 0 ==>
                match page_in(*left, key).frame {
                    None => false,
                    Some(frame) => page_in((*before).table(*other), key).frame == Some(frame)
                        || page_in((*before).table(*other), key).frame == None && new_to(*before, frame),
                });
        }
        let middle = snapshot!(**holder);
        proof_assert!((*middle).table(*other) == *left);
        proof_assert!(forall<space: SimulatedMemory> space != *other ==> (*middle).table(space) == (*before).table(space));
        let mut pages = self.lock_held(holder, snapshot!(Seq::empty()));
        // Changed: one borrow of the table for the whole loop, so that Creusot keeps what the table will hold when unlocked.
        let table = &mut *pages;
        let found = snapshot!(entries(*table));
        proof_assert!(table_sound(*found));
        let all = snapshot!(frames@);
        proof_assert!(forall<key: Int> addresses.start@ <= key && key % PAGE_SIZE@ == 0
            ==> key - addresses.start@ == (key - addresses.start@) / PAGE_SIZE@ * PAGE_SIZE@);
        let mut frames = frames.into_iter();
        let mut start = addresses.start;
        let mut index = snapshot!(0);
        #[invariant(0 <= *index && start@ == addresses.start@ + *index * PAGE_SIZE@ && start@ <= addresses.end@)]
        #[invariant((*all).len() * PAGE_SIZE@ == addresses.end@ - addresses.start@)]
        #[invariant(frames@.len() == (*all).len() - *index)]
        #[invariant(forall<j: Int> 0 <= j && j < frames@.len() ==> frames@[j] == (*all)[*index + j])]
        #[invariant(forall<i: Int> 0 <= i && i < *index ==> page_in(entries(*table), addresses.start@ + i * PAGE_SIZE@)
            == Page { frame: Some((*all)[i]), mapped: true, permissions: page_in(*found, addresses.start@ + i * PAGE_SIZE@).permissions })]
        #[invariant(forall<key: Int> key < addresses.start@ || start@ <= key || (key - addresses.start@) % PAGE_SIZE@ != 0
            ==> page_in(entries(*table), key) == page_in(*found, key))]
        while start < addresses.end {
            let rest = snapshot!(frames@);
            if let Some(frame) = frames.next() {
                proof_assert!(frame == (*rest)[0] && forall<j: Int> 0 <= j && j < frames@.len() ==> frames@[j] == (*rest)[j + 1]);
                let page = page_at(table, start);
                page.frame = Some(frame);
                page.mapped = true;
            }
            start += PAGE_SIZE;
            index = snapshot!(*index + 1);
        }
        proof_assert!(forall<key: Int> addresses.start@ <= key && key < addresses.end@ && key % PAGE_SIZE@ == 0
            ==> page_in(entries(*table), key) == Page {
                frame: Some((*all)[(key - addresses.start@) / PAGE_SIZE@]), mapped: true, permissions: page_in(*found, key).permissions
            });
        proof_assert!(forall<key: Int> !(addresses.start@ <= key && key < addresses.end@ && key % PAGE_SIZE@ == 0)
            ==> page_in(entries(*table), key) == page_in(*found, key));

        proof_assert!(other_addresses.start == other_start);
        proof_assert!(forall<key: Int> addresses.start@ <= key && key < addresses.end@ && key % PAGE_SIZE@ == 0 ==>
            page_in((*before).table(*other), key - addresses.start@ + other_start@).frame == Some((*all)[(key - addresses.start@) / PAGE_SIZE@])
            || page_in((*before).table(*other), key - addresses.start@ + other_start@).frame == None
                && new_to(*before, (*all)[(key - addresses.start@) / PAGE_SIZE@]));
        proof_assert!(forall<key: Int> addresses.start@ <= key && key < addresses.end@ && key % PAGE_SIZE@ == 0 ==>
            page_in(*left, key - addresses.start@ + other_start@).frame == Some((*all)[(key - addresses.start@) / PAGE_SIZE@]));
        proof_assert!(*other == *self ==> forall<key: Int> addresses.start@ <= key && key < addresses.end@ && key % PAGE_SIZE@ == 0
            && !(addresses.start@ <= key - addresses.start@ + other_start@ && key - addresses.start@ + other_start@ < addresses.end@)
            ==> page_in(entries(*table), key - addresses.start@ + other_start@).frame == Some((*all)[(key - addresses.start@) / PAGE_SIZE@]));
        proof_assert!(forall<key: Int> !(addresses.start@ <= key && key < addresses.end@ && key % PAGE_SIZE@ == 0) ==>
            match page_in(entries(*table), key).frame {
                None => true,
                Some(frame) => page_in((*middle).table(*self), key).frame == Some(frame)
                    || new_to(*middle, frame) && forall<other_key: Int> other_key != key ==> page_in(entries(*table), other_key).frame != Some(frame),
            });
        proof_assert!(table_sound(entries(*table)));
        count_change();
        Ok(())
    }

    #[requires(self.held(**holder))]
    #[ensures(Self::still_held(**holder, ^*holder))]
    #[ensures(match result {
        Ok(()) => Self::whole_pages(addresses.start@, addresses.end@)
            && Self::mappings_ended(**holder, ^*holder, *self, addresses),
        Err(_) => !Self::whole_pages(addresses.start@, addresses.end@) && ^*holder == **holder,
    })]
    fn unmap_from(&self, addresses: Range<u64>, holder: Ghost<&mut Tables>) -> Result<(), InvalidAddressError> {
        check_pages(&addresses)?;
        let before = snapshot!(**holder);
        proof_assert!(table_sound((*before).table(*self)));
        let mut pages = self.lock_held(holder, snapshot!(Seq::empty()));
        // Changed: one borrow of the table for the whole loop, so that Creusot keeps what the table will hold when unlocked.
        let table = &mut *pages;
        let found = snapshot!(entries(*table));
        proof_assert!(written_since((*before).table(*self), *found, *before, Seq::empty()));
        proof_assert!(table_sound(*found));
        // Changed: each page with an entry is found by a lookup from the last one, for which Creusot needs no
        // iterator contract; the pages visited are the same.
        let mut cursor = addresses.start;
        #[invariant(addresses.start@ <= cursor@ && cursor@ <= addresses.end@)]
        #[invariant(forall<key: Int> page_in(entries(*table), key) == if addresses.start@ <= key && key < cursor@ && page_in(*found, key).mapped {
            Page { frame: None, mapped: false, permissions: page_in(*found, key).permissions }
        } else {
            page_in(*found, key)
        })]
        while let Some(start) = first_entry(table, cursor..addresses.end) {
            let page = page_at(table, start);
            if page.mapped {
                page.frame = None;
                page.mapped = false;
            }
            cursor = start + 1;
        }
        proof_assert!(forall<key: Int> !(addresses.start@ <= key && key < addresses.end@ && page_in((*before).table(*self), key).mapped)
            ==> page_in(entries(*table), key) == page_in((*before).table(*self), key)
                || page_in((*before).table(*self), key).frame == None
                    && page_in(entries(*table), key).mapped == page_in((*before).table(*self), key).mapped
                    && page_in(entries(*table), key).permissions == page_in((*before).table(*self), key).permissions);
        proof_assert!(written_alone((*before).table(*self), entries(*table), *before));
        proof_assert!(table_sound(entries(*table)));
        count_change();
        Ok(())
    }

    #[requires(self.held(**holder) && other.held(**holder))]
    #[ensures(Self::still_held(**holder, ^*holder))]
    #[ensures(match result {
        Ok(()) => Self::whole_pages(addresses.start@, addresses.end@)
            && Self::whole_pages(other_start@, other_start@ + addresses.end@ - addresses.start@)
            && Self::pages_copied(**holder, ^*holder, *self, addresses, *other, other_start),
        Err(CopyMemoryError::InvalidAddress(_)) => ^*holder == **holder
            && !(Self::whole_pages(addresses.start@, addresses.end@)
                && Self::whole_pages(other_start@, other_start@ + addresses.end@ - addresses.start@)),
        Err(CopyMemoryError::OutOfMemory) => ^*holder == **holder,
    })]
    fn copy_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
        mut holder: Ghost<&mut Tables>,
    ) -> Result<(), CopyMemoryError> {
        // Changed: matches rather than `map_err` with the variant as a function, whose result Creusot does not know.
        if let Err(error) = check_pages(&addresses) {
            return Err(CopyMemoryError::InvalidAddress(error));
        }
        let other_addresses = match shift(&addresses, other_start) {
            Ok(other_addresses) => other_addresses,
            Err(error) => return Err(CopyMemoryError::InvalidAddress(error)),
        };
        let before = snapshot!(**holder);
        let mut made = snapshot!(Seq::<Frame>::empty());
        // Changed: loops over the page starts, for which Creusot needs no iterator contract.
        let mut copied: Vec<Option<Page>> = Vec::new();
        let mut left = snapshot!(FMap::empty());
        {
            let other_pages = other.lock_held(ghost!(&mut **holder), snapshot!(Seq::empty()));
            let table = &*other_pages;
            let found = snapshot!(entries(*table));
            proof_assert!(table_sound(*found));
            let mut start = other_addresses.start;
            #[invariant(other_addresses.start@ <= start@ && start@ <= other_addresses.end@ && start@ % PAGE_SIZE@ == 0)]
            #[invariant(entries(*table) == *found)]
            #[invariant(copied@.len() * PAGE_SIZE@ == start@ - other_addresses.start@)]
            #[invariant(forall<i: Int> 0 <= i && i < copied@.len() ==> copied_as(page_in(*found, other_addresses.start@ + i * PAGE_SIZE@), copied@[i], *made))]
            #[invariant(forall<f: Frame> (*made).contains(f) ==> new_to(*before, f) && !shows(*found, f))]
            #[invariant(forall<i: Int, j: Int> 0 <= i && i < j && j < (*made).len() ==> (*made)[i] != (*made)[j])]
            #[invariant(forall<i: Int, j: Int> 0 <= i && i < j && j < copied@.len() ==> match (copied@[i], copied@[j]) {
                (Some(first), Some(second)) => !first.mapped && first.frame != None ==> first.frame != second.frame,
                _ => true,
            })]
            #[invariant(forall<i: Int, f: Frame> 0 <= i && i < copied@.len() && (*made).contains(f) ==> match copied@[i] {
                Some(copy) => copy.mapped ==> copy.frame != Some(f),
                None => true,
            })]
            while start < other_addresses.end {
                let page = match page_of(table, start) {
                    Some(page) => {
                        let copy = copy_page(page, before, found, made);
                        made = snapshot!(match copy.frame {
                            Some(frame) => if copy.mapped { *made } else { (*made).concat(Seq::singleton(frame)) },
                            None => *made,
                        });
                        Some(copy)
                    }
                    None => None,
                };
                copied.push(page);
                start += PAGE_SIZE;
            }
            left = snapshot!(*found);
        }
        let middle = snapshot!(**holder);
        proof_assert!((*middle).table(*other) == *left);
        proof_assert!(forall<f: Frame> (*made).contains(f) ==> new_to(*before, f) && !shows(*left, f));
        proof_assert!(forall<space: SimulatedMemory> space != *other ==> (*middle).table(space) == (*before).table(space));
        let mut pages = self.lock_held(holder, made);
        // Changed: one borrow of the table for the whole loop, so that Creusot keeps what the table will hold when unlocked.
        let table = &mut *pages;
        let found = snapshot!(entries(*table));
        proof_assert!(table_sound(*found));
        let all = snapshot!(copied@);
        proof_assert!(forall<key: Int> addresses.start@ <= key && key % PAGE_SIZE@ == 0
            ==> key - addresses.start@ == (key - addresses.start@) / PAGE_SIZE@ * PAGE_SIZE@);
        let mut copied = copied.into_iter();
        let mut start = addresses.start;
        let mut index = snapshot!(0);
        #[invariant(0 <= *index && start@ == addresses.start@ + *index * PAGE_SIZE@ && start@ <= addresses.end@)]
        #[invariant((*all).len() * PAGE_SIZE@ == addresses.end@ - addresses.start@)]
        #[invariant(copied@.len() == (*all).len() - *index)]
        #[invariant(forall<j: Int> 0 <= j && j < copied@.len() ==> copied@[j] == (*all)[*index + j])]
        #[invariant(forall<i: Int> 0 <= i && i < *index ==> page_in(entries(*table), addresses.start@ + i * PAGE_SIZE@)
            == match (*all)[i] { Some(page) => page, None => blank() })]
        #[invariant(forall<key: Int> key < addresses.start@ || start@ <= key || (key - addresses.start@) % PAGE_SIZE@ != 0
            ==> page_in(entries(*table), key) == page_in(*found, key))]
        while start < addresses.end {
            let rest = snapshot!(copied@);
            if let Some(page) = copied.next() {
                proof_assert!(page == (*rest)[0] && forall<j: Int> 0 <= j && j < copied@.len() ==> copied@[j] == (*rest)[j + 1]);
                match page {
                    Some(page) => put(table, start, page),
                    None => clear(table, start),
                }
            }
            start += PAGE_SIZE;
            index = snapshot!(*index + 1);
        }
        proof_assert!(forall<key: Int> addresses.start@ <= key && key < addresses.end@ && key % PAGE_SIZE@ == 0
            ==> page_in(entries(*table), key) == match (*all)[(key - addresses.start@) / PAGE_SIZE@] { Some(page) => page, None => blank() });
        proof_assert!(forall<key: Int> !(addresses.start@ <= key && key < addresses.end@ && key % PAGE_SIZE@ == 0)
            ==> page_in(entries(*table), key) == page_in(*found, key));
        proof_assert!(table_sound(entries(*table)));
        count_change();
        Ok(())
    }
}

impl VolatileBytes for SimulatedBytes<'_> {
    fn len(&self) -> u64 {
        self.addresses.end - self.addresses.start
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = into.len().min(to_count(self.len()));
        self.space.read_at(self.addresses.start, &mut into[..count])
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let count = bytes.len().min(to_count(self.len()));
        self.space.write_at(self.addresses.start, &bytes[..count])
    }

    /// Copies all the bytes out and hands them to the sink, since a slice of the guest's bytes may not be lent.
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut copied = vec![u8::MIN; to_count(self.len())];
        self.read(&mut copied);
        sink.write(&copied)
    }

    /// Has the source fill a buffer as long as the bytes and writes what it gave, for the reason `read_to` copies.
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut buffer = vec![u8::MIN; to_count(self.len())];
        let count = source.read(&mut buffer)?;
        Ok(self.write(&buffer[..count]))
    }
}

/// Counts a change now in place, and returns the count of changes before it.
///
/// Trusted: Creusot has no contract for the standard atomic integers; this one promises nothing.
#[trusted]
fn count_change() -> u64 {
    CHANGES.fetch_add(1, Ordering::AcqRel)
}

fn new_frame() -> Frame {
    Arc::new(Mutex::new(vec![u8::MIN; PAGE_BYTES].into_boxed_slice()))
}

fn lock_frame(frame: &Frame) -> MutexGuard<'_, Box<[u8]>> {
    frame.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Returns the page as a copy carries it: a mapped frame shared, an owned one copied.
#[ensures(page.mapped ==> result == *page)]
#[ensures(!page.mapped ==> !result.mapped && result.permissions == page.permissions
    && match result.frame { None => page.frame == None, Some(frame) => fresh(*before, *found, *made, frame) })]
fn copy_page(page: &Page, before: Snapshot<Tables>, found: Snapshot<FMap<Int, Page>>, made: Snapshot<Seq<Frame>>) -> Page {
    let frame = match &page.frame {
        Some(frame) if page.mapped => Some(Arc::clone(frame)),
        // Changed: the copy of the bytes is made by a function whose contract says it is new.
        Some(frame) => Some(copy_frame(frame, before, found, made)),
        None => None,
    };
    Page {
        frame,
        mapped: page.mapped,
        permissions: page.permissions,
    }
}

#[ensures(match result { Ok(()) => SimulatedMemory::inside(addresses.start@, addresses.end@), Err(_) => !SimulatedMemory::inside(addresses.start@, addresses.end@) })]
fn check_inside(addresses: &Range<u64>) -> Result<(), InvalidAddressError> {
    // Changed: Creusot has no contract for `RangeInclusive::contains`; these comparisons say the same.
    let bounds = Arm64::ADDRESSABLE;
    let inside = bounds.start <= addresses.start
        && addresses.end <= bounds.end
        && addresses.start <= addresses.end;
    if inside {
        Ok(())
    } else {
        Err(InvalidAddressError)
    }
}

#[ensures(match result { Ok(()) => SimulatedMemory::whole_pages(addresses.start@, addresses.end@), Err(_) => !SimulatedMemory::whole_pages(addresses.start@, addresses.end@) })]
fn check_pages(addresses: &Range<u64>) -> Result<(), InvalidAddressError> {
    check_inside(addresses)?;
    // Changed: the remainder rather than the mask, the same test, which Creusot relates to the contract's `%`.
    if addresses.start % PAGE_SIZE == u64::MIN && addresses.end % PAGE_SIZE == u64::MIN {
        Ok(())
    } else {
        Err(InvalidAddressError)
    }
}

/// Returns the addresses as long as the given ones that start at the start, refused unless they are whole pages.
#[requires(addresses.start@ <= addresses.end@)]
#[ensures(match result {
    Ok(shifted) => shifted.start == start && shifted.end@ == start@ + addresses.end@ - addresses.start@
        && SimulatedMemory::whole_pages(shifted.start@, shifted.end@),
    Err(_) => !SimulatedMemory::whole_pages(start@, start@ + addresses.end@ - addresses.start@),
})]
fn shift(addresses: &Range<u64>, start: u64) -> Result<Range<u64>, InvalidAddressError> {
    let end = start
        .checked_add(addresses.end - addresses.start)
        .ok_or(InvalidAddressError)?;
    check_pages(&(start..end))?;
    Ok(start..end)
}

/// Returns the first address of each page of addresses that start a page.
fn page_starts(addresses: &Range<u64>) -> impl Iterator<Item = u64> + use<> {
    addresses.clone().step_by(PAGE_BYTES)
}

/// Returns each page the bytes from the start touch, with the bytes' place in the page and in the buffer.
fn split_at_pages(start: u64, length: usize) -> Vec<(u64, Range<usize>, Range<usize>)> {
    let mut parts = Vec::new();
    let mut done = usize::MIN;
    while done < length {
        let address = start + done as u64;
        let within = to_count(address & PAGE_MASK);
        let count = (PAGE_BYTES - within).min(length - done);
        parts.push((
            address & !PAGE_MASK,
            within..within + count,
            done..done + count,
        ));
        done += count;
    }
    parts
}

/// Returns the length as a count of host bytes, at most the most the host can count.
fn to_count(length: u64) -> usize {
    usize::try_from(length).unwrap_or(usize::MAX)
}

/// Returns what the page allows as ARM64 sees it; a page that allows nothing is held unmapped.
fn to_page_permission(permissions: Permissions) -> Option<PagePermission> {
    if permissions.is_empty() {
        return None;
    }
    // ARM64 has no page that can be written and not read; Linux gives such a page read and write.
    let data_access = if permissions.contains(Permissions::WRITE) {
        DataAccess::ReadWrite
    } else if permissions.contains(Permissions::READ) {
        DataAccess::ReadOnly
    } else {
        DataAccess::Forbidden
    };
    let instruction_fetch = if permissions.contains(Permissions::EXECUTE) {
        InstructionFetch::Allowed
    } else {
        InstructionFetch::Forbidden
    };
    Some(PagePermission::Mapped {
        data_access,
        instruction_fetch,
    })
}

fn to_permissions(permission: PagePermission) -> Permissions {
    match permission {
        PagePermission::Unmapped => Permissions::empty(),
        PagePermission::Mapped {
            data_access,
            instruction_fetch,
        } => {
            let data = match data_access {
                DataAccess::Forbidden => Permissions::empty(),
                DataAccess::ReadOnly => Permissions::READ,
                DataAccess::ReadWrite => Permissions::READ | Permissions::WRITE,
            };
            match instruction_fetch {
                InstructionFetch::Forbidden => data,
                InstructionFetch::Allowed => data | Permissions::EXECUTE,
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The model of a page table, for the proofs only.

/// What a page that was never given permissions allows. Only its name matters to the proofs.
#[logic(opaque)]
fn nothing() -> Permissions {
    dead
}

/// The page a key with no entry stands for: no frame, not mapped, allowing nothing.
#[logic(inline)]
fn blank() -> Page {
    pearlite! { Page { frame: None, mapped: false, permissions: nothing() } }
}

/// The page that starts at the address in a table.
#[logic(inline)]
fn page_in(table: FMap<Int, Page>, start: Int) -> Page {
    pearlite! { match table.lookup(start) { Some(page) => page, None => blank() } }
}

/// A page of a table as the contract sees it.
#[logic(inline)]
fn model(page: Page) -> memory_space::Page<Frame> {
    pearlite! { memory_space::Page { frame: page.frame, mapped: page.mapped, permissions: page.permissions } }
}

/// Whether some page of the table shows the frame.
#[logic(inline)]
fn shows(table: FMap<Int, Page>, frame: Frame) -> bool {
    pearlite! { exists<start: Int> page_in(table, start).frame == Some(frame) }
}

/// Whether no page of a space the holder holds shows the frame.
#[logic(inline)]
fn new_to(holder: Tables, frame: Frame) -> bool {
    pearlite! { forall<space: SimulatedMemory> holder.holds(space) ==> !shows(holder.table(space), frame) }
}

/// Whether each page that gained a frame since the holder saw the table shows a frame new to the holder that no
/// other page of the table shows.
#[logic(inline)]
fn written_alone(seen: FMap<Int, Page>, table: FMap<Int, Page>, holder: Tables) -> bool {
    pearlite! {
        forall<key: Int> page_in(seen, key).frame == None ==> match page_in(table, key).frame {
            None => true,
            Some(frame) => new_to(holder, frame) && forall<other: Int> other != key ==> page_in(table, other).frame != Some(frame),
        }
    }
}

/// Whether a frame is new to the holder, to the table and to the frames already made.
#[logic(inline)]
fn fresh(holder: Tables, table: FMap<Int, Page>, made: Seq<Frame>, frame: Frame) -> bool {
    pearlite! { new_to(holder, frame) && !shows(table, frame) && !made.contains(frame) }
}

/// Whether a copy of the page carries it: the same page if mapped, otherwise its permissions with a frame of its own
/// among those made.
#[logic(inline)]
fn copied_as(page: Page, copy: Option<Page>, made: Seq<Frame>) -> bool {
    pearlite! {
        match copy {
            None => page == blank(),
            Some(copy) => if page.mapped { copy == page } else {
                !copy.mapped && copy.permissions == page.permissions
                && match copy.frame { None => true, Some(frame) => made.contains(frame) }
            },
        }
    }
}

/// Whether a table locked now is the table as the holder last saw it, but for pages with no frame that a write since
/// gave one: a new frame, shown by no other page.
#[logic(inline)]
fn written_since(seen: FMap<Int, Page>, now: FMap<Int, Page>, holder: Tables, alive: Seq<Frame>) -> bool {
    pearlite! {
        forall<start: Int> page_in(now, start) == page_in(seen, start)
        || page_in(seen, start).frame == None
            && page_in(now, start).mapped == page_in(seen, start).mapped
            && page_in(now, start).permissions == page_in(seen, start).permissions
            && match page_in(now, start).frame {
                None => false,
                Some(frame) => new_to(holder, frame) && !alive.contains(frame)
                    && forall<other: Int> other != start ==> page_in(now, other).frame != Some(frame),
            }
    }
}

/// Whether every mapped page of the table has a frame.
#[logic(inline)]
fn table_sound(table: FMap<Int, Page>) -> bool {
    pearlite! { forall<start: Int> page_in(table, start).mapped ==> page_in(table, start).frame != None }
}

impl Tables {
    /// Whether the table of each space held is sound.
    #[logic]
    pub(crate) fn sound(self) -> bool {
        pearlite! { forall<space: SimulatedMemory> self.holds(space) ==> table_sound(self.table(space)) }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Trusted: what Creusot cannot see into, with what each assumes. Every body here is the original standard call.

/// The right to change spaces, as `MemorySpace::Holder`: each held space's page table as the holder last saw it.
/// It exists only in proofs and is never made when the program runs. Opaque: without it every holder would be equal.
#[opaque]
pub struct Tables(());

impl Tables {
    /// The page table of the space as the holder last saw it, by the first address of each page.
    #[logic(opaque)]
    pub(crate) fn table(self, space: SimulatedMemory) -> FMap<Int, Page> {
        dead
    }

    /// Whether the holder holds the space.
    #[logic(opaque)]
    pub(crate) fn holds(self, space: SimulatedMemory) -> bool {
        dead
    }

    /// A holder of no spaces. Assumes nothing: holding no space promises nothing.
    #[trusted]
    #[check(ghost)]
    #[ensures(forall<space: SimulatedMemory> !result.holds(space))]
    pub fn new() -> Self {
        Tables(())
    }
}

impl SimulatedMemory {
    /// Returns a new space with no pages, held by the holder.
    ///
    /// Trusted: assumes a new space is none of those the holder holds, as a new value is.
    #[trusted]
    #[ensures(!(**holder).holds(result) && (^*holder).holds(result) && (^*holder).table(result) == FMap::empty())]
    #[ensures(forall<other: SimulatedMemory> other != result ==> (^*holder).holds(other) == (**holder).holds(other) && (^*holder).table(other) == (**holder).table(other))]
    pub fn new_held(holder: Ghost<&mut Tables>) -> Self {
        let _ = holder;
        Self::default()
    }

    /// Locks the page table of a space the holder holds.
    ///
    /// Trusted: assumes that, between holder calls, the only change to the table is a write giving a page with no
    /// frame a new frame (`make_frame`, through `VolatileBytes::write`), new to every space and to the frames alive;
    /// and that when the lock ends the holder sees the table as it is left.
    #[trusted]
    #[requires((**holder).holds(*self))]
    #[ensures(written_since((**holder).table(*self), entries(result.now()), **holder, *alive))]
    #[ensures((^*holder).table(*self) == entries(result.last()))]
    #[ensures(forall<other: SimulatedMemory> other != *self ==> (^*holder).table(other) == (**holder).table(other))]
    #[ensures(forall<other: SimulatedMemory> (^*holder).holds(other) == (**holder).holds(other))]
    fn lock_held<'a>(&'a self, holder: Ghost<&'a mut Tables>, alive: Snapshot<Seq<Frame>>) -> Held<'a> {
        let _ = (holder, alive);
        Held(self.lock_pages())
    }
}

/// A locked page table, with what a proof knows of it: what it holds now, and what it will hold when unlocked.
struct Held<'a>(MutexGuard<'a, BTreeMap<u64, Page>>);

impl Held<'_> {
    #[logic(opaque)]
    fn now(self) -> BTreeMap<u64, Page> {
        dead
    }

    #[logic(opaque, prophetic)]
    fn last(self) -> BTreeMap<u64, Page> {
        dead
    }
}

impl Resolve for Held<'_> {
    #[logic(prophetic)]
    fn resolve(self) -> bool {
        pearlite! { self.now() == self.last() }
    }

    /// Trusted: what the table holds when the lock ends is what it holds last.
    #[trusted]
    #[logic(prophetic)]
    #[requires(creusot_std::invariant::inv(self))]
    #[requires(creusot_std::resolve::structural_resolve(self))]
    #[ensures(self.resolve())]
    fn resolve_coherence(self) {}
}

impl Deref for Held<'_> {
    type Target = BTreeMap<u64, Page>;

    #[trusted]
    #[check(ghost)]
    #[ensures(*result == self.now())]
    fn deref(&self) -> &BTreeMap<u64, Page> {
        &self.0
    }
}

impl DerefMut for Held<'_> {
    #[trusted]
    #[check(ghost)]
    #[ensures(*result == (*self).now() && ^result == (^self).now() && (^self).last() == (*self).last())]
    fn deref_mut(&mut self) -> &mut BTreeMap<u64, Page> {
        &mut self.0
    }
}

/// What a page table holds, by the first address of each page. Opaque: std's `BTreeMap` has no model in Creusot.
#[logic(opaque)]
fn entries(table: BTreeMap<u64, Page>) -> FMap<Int, Page> {
    dead
}

/// Returns the page that starts at the address, made with nothing in it if there was none.
#[trusted]
#[ensures(*result == page_in(entries(*pages), start@))]
#[ensures(entries(^pages) == entries(*pages).add(start@, ^result))]
fn page_at(pages: &mut BTreeMap<u64, Page>, start: u64) -> &mut Page {
    pages.entry(start).or_default()
}

/// Returns the page that starts at the address, if it has an entry.
#[trusted]
#[ensures(match result { Some(page) => entries(*pages).lookup(start@) == Some(*page), None => entries(*pages).lookup(start@) == None })]
fn page_of(pages: &BTreeMap<u64, Page>, start: u64) -> Option<&Page> {
    pages.get(&start)
}

/// Sets the page that starts at the address.
#[trusted]
#[ensures(entries(^pages) == entries(*pages).add(start@, page))]
fn put(pages: &mut BTreeMap<u64, Page>, start: u64, page: Page) {
    pages.insert(start, page);
}

/// Removes the page that starts at the address.
#[trusted]
#[ensures(entries(^pages) == entries(*pages).remove(start@))]
fn clear(pages: &mut BTreeMap<u64, Page>, start: u64) {
    pages.remove(&start);
}

/// Returns the first address of the addresses that has an entry.
#[trusted]
#[ensures(match result {
    Some(start) => addresses.start@ <= start@ && start@ < addresses.end@ && entries(*pages).lookup(start@) != None
        && forall<key: Int> addresses.start@ <= key && key < start@ ==> entries(*pages).lookup(key) == None,
    None => forall<key: Int> addresses.start@ <= key && key < addresses.end@ ==> entries(*pages).lookup(key) == None,
})]
fn first_entry(pages: &BTreeMap<u64, Page>, addresses: Range<u64>) -> Option<u64> {
    pages.range(addresses).next().map(|(&start, _)| start)
}

/// Returns a new frame of zeros.
///
/// Trusted: assumes a new frame is none of the frames alive: those of the spaces held, of the table, and made.
#[trusted]
#[ensures(fresh(*holder, *table, *made, result))]
fn new_frame_for(holder: Snapshot<Tables>, table: Snapshot<FMap<Int, Page>>, made: Snapshot<Seq<Frame>>) -> Frame {
    let _ = (holder, table, made);
    new_frame()
}

/// Returns a new frame holding a copy of the frame's bytes.
///
/// Trusted: as `new_frame_for`.
#[trusted]
#[ensures(fresh(*holder, *table, *made, result))]
fn copy_frame(frame: &Frame, holder: Snapshot<Tables>, table: Snapshot<FMap<Int, Page>>, made: Snapshot<Seq<Frame>>) -> Frame {
    let _ = (holder, table, made);
    Arc::new(Mutex::new(lock_frame(frame).clone()))
}
