//! Guest memory as owned pages, shared between spaces only where a mapping shares them.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use vstd::prelude::*;
#[cfg(verus_only)]
use vstd::resource::Loc;
use vstd::resource::ghost_var::{GhostVar, GhostVarAuth};
#[cfg(verus_only)]
use vstd::std_specs::btree::{btree_map_inv, group_btree_axioms};
use wcontainers_abi::{PAGE_MASK, PAGE_SIZE, Permissions};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_executor::memory_space::{
    self as contract, CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace,
};
use wcontainers_processor_architecture::{MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, InstructionFetch, PagePermission,
};
use wcontainers_volatile_bytes::VolatileBytes;

const PAGE_BYTES: usize = PAGE_SIZE as usize;

/// Why a space stops the program: a page method stopped while it held the pages, so they are gone.
const LOST: &str = "a page method stopped while it held the space's pages";

// The proof's copies of the architecture's bounds and the ABI's page mask, checked equal here.
const _: () =
    assert!(Arm64::ADDRESSABLE.start == u64::MIN && Arm64::ADDRESSABLE.end == ADDRESSABLE_END);
const _: () = assert!(OFFSET_MASK == PAGE_MASK);

/// The number of changes made to any space, counted after each change is in place.
///
/// One count serves every space, since a mapping shares pages between spaces: a copy taken at a count is current while
/// the count stays.
static CHANGES: AtomicU64 = AtomicU64::new(u64::MIN);

verus! {

broadcast use {group_btree_axioms, vstd::laws_cmp::group_laws_cmp};

/// The first address past those an ARM64 program can use.
pub const ADDRESSABLE_END: u64 = 1 << 48;

/// The bits of an address that are its offset within its page.
const OFFSET_MASK: u64 = 4095;

// ---------------------------------------------------------------- trusted: frames and the lock

/// The bytes of one page, held by every space that maps it.
#[verifier::external_body]
pub(crate) struct Frame(Arc<Mutex<Box<[u8]>>>);

impl Frame {
    /// The name the contract gives the frame: every handle to the same bytes has the same name.
    pub uninterp spec fn name(&self) -> contract::FrameName;

    /// Returns a new frame of zeros, with the given name.
    #[verifier::external_body]
    fn zeros(Ghost(name): Ghost<contract::FrameName>) -> (frame: Frame)
        ensures
            frame.name() == name,
    {
        Frame(Arc::new(Mutex::new(vec![u8::MIN; PAGE_BYTES].into_boxed_slice())))
    }

    /// Returns another handle to the same bytes.
    #[verifier::external_body]
    pub(crate) fn share(&self) -> (frame: Frame)
        ensures
            frame.name() == self.name(),
    {
        Frame(Arc::clone(&self.0))
    }

    /// Returns a new frame holding a copy of the bytes, with the given name.
    #[verifier::external_body]
    fn copy(&self, Ghost(name): Ghost<contract::FrameName>) -> (frame: Frame)
        ensures
            frame.name() == name,
    {
        Frame(Arc::new(Mutex::new(lock_frame(self).clone())))
    }
}

/// A space's pages behind a lock, which keeps them well formed for the token it was made for.
#[verifier::external_body]
struct Locked {
    /// None only while a page method holds the table, or after one stopped without putting it back.
    mutex: Mutex<Option<Table>>,
}

/// The lock held, until the table is put back.
#[verifier::external_body]
struct Held<'lock> {
    guard: MutexGuard<'lock, Option<Table>>,
}

impl Locked {
    /// The token whose model the table is.
    uninterp spec fn token(&self) -> Loc;

    #[verifier::external_body]
    fn new(table: Table) -> (locked: Locked)
        requires
            table.wf(table.model@.id()),
        ensures
            locked.token() == table.model@.id(),
    {
        Locked { mutex: Mutex::new(Some(table)) }
    }

    /// Takes the lock and the table out of it; stops the program if a page method lost the table.
    #[verifier::external_body]
    fn acquire(&self) -> (taken: (Table, Held<'_>))
        ensures
            taken.0.wf(self.token()),
            taken.1.token() == self.token(),
    {
        let mut guard = self.mutex.lock().unwrap_or_else(PoisonError::into_inner);
        let table = guard.take().expect(LOST);
        (table, Held { guard })
    }
}

impl<'lock> Held<'lock> {
    uninterp spec fn token(&self) -> Loc;

    /// Puts the table back and releases the lock.
    #[verifier::external_body]
    fn release(self, table: Table)
        requires
            table.wf(self.token()),
    {
        let mut guard = self.guard;
        *guard = Some(table);
    }
}

/// Changes no proof: counts a change now in place, and returns the count of changes before it.
#[verifier::external_body]
fn count_change() -> u64 {
    CHANGES.fetch_add(1, Ordering::AcqRel)
}

/// The name of the new frame a change gives the page at the start: numbered past those the space had used.
spec fn new_name(token: Loc, before: contract::Space, start: u64) -> contract::FrameName {
    contract::FrameName { space: token, number: before.made + start }
}

/// The count of frame numbers a space has used once a change has made new frames.
spec fn made_after(before: contract::Space) -> int {
    before.made + u64::MAX + 1
}

// ---------------------------------------------------------------- the pages

/// One page of a space; a page with no entry reads as zeros and allows nothing.
pub(crate) struct Page {
    /// None while the page reads as zeros and no space shares it.
    pub(crate) frame: Option<Frame>,
    /// Whether this space mapped the frame from another, so that a copy of the page shares it too and an unmap ends it.
    pub(crate) mapped: bool,
    pub(crate) permissions: Permissions,
    /// The name of the page's frame, given before the frame is made.
    name: Ghost<contract::FrameName>,
}

impl Page {
    /// The page as the contract sees it.
    spec fn view(&self) -> contract::Page {
        contract::Page { frame: self.name@, mapped: self.mapped, allowed: self.permissions }
    }

    /// Whether the page's frame, if it has one, has the page's name.
    spec fn wf(&self) -> bool {
        self.frame matches Some(frame) ==> frame.name() == self.name@
    }
}

/// Returns a page that reads as zeros, is not mapped and allows nothing.
fn empty_page(Ghost(name): Ghost<contract::FrameName>) -> (page: Page)
    ensures
        page.wf(),
        page@ == (contract::Page { frame: name, mapped: false, allowed: contract::nothing() }),
{
    Page { frame: None, mapped: false, permissions: contract::no_permissions(), name: Ghost(name) }
}

/// The pages of a space, with the ghost state its proof needs.
pub(crate) struct Table {
    /// The pages that have a frame, a mapping or permissions, by their first address.
    pub(crate) pages: BTreeMap<u64, Page>,
    /// The name of the frame of each page with no entry.
    unmade: Ghost<spec_fn(u64) -> contract::FrameName>,
    /// How many frame numbers the space has used.
    made: Ghost<int>,
    /// The other half of the token that holds the space's model.
    #[allow(dead_code, reason = "ghost state, erased from the built code")]
    model: Tracked<GhostVarAuth<contract::Space>>,
}

/// The page that starts at the address, as the contract sees it.
spec fn page_of(pages: Map<u64, Page>, unmade: spec_fn(u64) -> contract::FrameName, start: u64) -> contract::Page {
    if pages.contains_key(start) {
        pages[start]@
    } else {
        contract::Page { frame: unmade(start), mapped: false, allowed: contract::nothing() }
    }
}

/// The space, as the contract sees it.
spec fn space_of(pages: Map<u64, Page>, unmade: spec_fn(u64) -> contract::FrameName, made: int) -> contract::Space {
    contract::Space { pages: |start: u64| page_of(pages, unmade, start), made }
}

impl Table {
    spec fn space(&self) -> contract::Space {
        space_of(self.pages@, self.unmade@, self.made@)
    }

    /// Whether the table is well formed and is the model held by the token.
    spec fn wf(&self, token: Loc) -> bool {
        &&& btree_map_inv(&self.pages)
        &&& self.model@.id() == token
        &&& self.model@@ == self.space()
        &&& forall|start: u64| #[trigger] self.pages@.contains_key(start) ==> {
            &&& self.pages@[start].wf()
            &&& self.pages@[start].mapped ==> contract::starts_page(start)
        }
    }
}

// ---------------------------------------------------------------- the space

/// A guest's address space, every page of it owned or shared in host memory.
///
/// The page methods (`set_permissions`, `map_from`, `unmap_from` and `copy_from`) take whole pages: addresses that
/// do not start and end at a page boundary are refused. A copy copies the bytes at once rather than on a write.
pub struct SimulatedMemory {
    /// The pages that have bytes, a mapping or permissions, by their first address.
    pages: Locked,
}

/// Addresses of a [`SimulatedMemory`], which another thread's guest may be changing while they are held.
pub struct SimulatedBytes<'space> {
    space: &'space SimulatedMemory,
    addresses: Range<u64>,
}

/// The addresses an ARM64 program can use.
pub open spec fn addressable() -> Range<u64> {
    Range { start: 0, end: ADDRESSABLE_END }
}

impl SimulatedMemory {
    /// Returns a new space, every page of it zeros that allow nothing, and the token that holds its model.
    pub fn new() -> (made: (SimulatedMemory, Tracked<GhostVar<contract::Space>>))
        ensures
            made.1@.id() == made.0.token(),
            contract::new_space(made.1@@, made.0.token()),
    {
        let pages = BTreeMap::new();
        let ghost first: contract::Space = arbitrary();
        let tracked (mut auth, mut var) = GhostVarAuth::<contract::Space>::new(first);
        let ghost token = auth.id();
        let ghost unmade = |start: u64| contract::FrameName { space: token, number: start as int };
        let ghost made = u64::MAX + 1;
        proof {
            auth.update(&mut var, space_of(Map::empty(), unmade, made));
        }
        let table = Table { pages, unmade: Ghost(unmade), made: Ghost(made), model: Tracked(auth) };
        assert(table.space().pages =~= space_of(Map::empty(), unmade, made).pages);
        (SimulatedMemory { pages: Locked::new(table) }, Tracked(var))
    }

    /// Returns the frame of the page, given one of zeros if it had none: the frame the known model names.
    pub(crate) fn make_frame(&self, page: u64, Tracked(known): Tracked<&GhostVar<contract::Space>>) -> (frame: Frame)
        requires
            known.id() == self.token(),
        ensures
            frame.name() == known@.page(page).frame,
    {
        let (mut table, held) = self.pages.acquire();
        proof {
            table.model.borrow().agree(known);
        }
        let ghost before = table.space();
        let page_entry = match table.pages.remove(&page) {
            Some(found) => found,
            None => empty_page(Ghost((table.unmade@)(page))),
        };
        let (frame, kept) = match page_entry.frame {
            Some(frame) => {
                let shared = frame.share();
                (shared, Page { frame: Some(frame), ..page_entry })
            },
            None => {
                let made = Frame::zeros(page_entry.name);
                let shared = made.share();
                (shared, Page { frame: Some(made), ..page_entry })
            },
        };
        table.pages.insert(page, kept);
        assert(table.space().pages =~= before.pages);
        held.release(table);
        frame
    }

    /// Gives each page from the start of the addresses, a page at a time, what an instruction allowed.
    pub(crate) fn assign_permissions(
        &self,
        addresses: Range<u64>,
        allowed: Permissions,
        Tracked(space): Tracked<&mut GhostVar<contract::Space>>,
    )
        requires
            old(space).id() == self.token(),
        ensures
            final(space).id() == old(space).id(),
    {
        let (mut table, held) = self.pages.acquire();
        let mut start = addresses.start;
        while start < addresses.end
            invariant
                btree_map_inv(&table.pages),
                table.model@.id() == self.pages.token(),
                table.model@.id() == space.id(),
                forall|key: u64| #[trigger] table.pages@.contains_key(key) ==> {
                    &&& table.pages@[key].wf()
                    &&& table.pages@[key].mapped ==> contract::starts_page(key)
                },
            decreases u64::MAX - start,
        {
            let page = match table.pages.remove(&start) {
                Some(found) => Page { permissions: allowed, ..found },
                None => Page { permissions: allowed, ..empty_page(Ghost((table.unmade@)(start))) },
            };
            table.pages.insert(start, page);
            start = match start.checked_add(contract::PAGE_SIZE) {
                Some(next) => next,
                None => break,
            };
        }
        proof {
            let after = table.space();
            table.model.borrow_mut().update(space, after);
        }
        held.release(table);
    }
}

/// Returns whether the addresses run forward and lie inside the addressable ones.
fn check_inside(addresses: &Range<u64>) -> (result: Result<(), InvalidAddressError>)
    ensures
        result is Ok <==> contract::inside(*addresses, addressable()),
{
    let inside = addresses.start <= ADDRESSABLE_END && addresses.end <= ADDRESSABLE_END
        && addresses.start <= addresses.end;
    if inside {
        Ok(())
    } else {
        Err(InvalidAddressError)
    }
}

/// Returns whether the addresses are whole pages inside the addressable ones.
fn check_pages(addresses: &Range<u64>) -> (result: Result<(), InvalidAddressError>)
    ensures
        result is Ok <==> contract::whole_pages(*addresses, addressable()),
{
    check_inside(addresses)?;
    let ghost start = addresses.start;
    let ghost end = addresses.end;
    proof {
        assert(start & 4095 == 0 <==> start % 4096 == 0) by (bit_vector);
        assert(end & 4095 == 0 <==> end % 4096 == 0) by (bit_vector);
    }
    if addresses.start & OFFSET_MASK == u64::MIN && addresses.end & OFFSET_MASK == u64::MIN {
        Ok(())
    } else {
        Err(InvalidAddressError)
    }
}

/// Returns the addresses as long as the given ones that start at the start, refused unless they are whole pages.
fn shift(addresses: &Range<u64>, start: u64) -> (result: Result<Range<u64>, InvalidAddressError>)
    requires
        addresses.start <= addresses.end,
    ensures
        result is Ok <==> contract::source_fits(*addresses, start, addressable()),
        result matches Ok(shifted) ==> shifted.start == start
            && shifted.end == start + (addresses.end - addresses.start)
            && contract::whole_pages(shifted, addressable()),
{
    let end = match start.checked_add(addresses.end - addresses.start) {
        Some(end) => end,
        None => return Err(InvalidAddressError),
    };
    check_pages(&(start..end))?;
    Ok(start..end)
}

/// The page among the addresses that starts the given number of pages in.
spec fn nth_page(addresses: Range<u64>, index: int) -> u64 {
    (addresses.start + index * contract::PAGE_SIZE) as u64
}

/// Putting an entry at a start changes that page of the space alone.
proof fn lemma_put(pages: Map<u64, Page>, unmade: spec_fn(u64) -> contract::FrameName, made: int, start: u64, entry: Page)
    ensures
        forall|page: u64| #[trigger] space_of(pages.insert(start, entry), unmade, made).page(page)
            == if page == start { entry@ } else { space_of(pages, unmade, made).page(page) },
{
}

/// The names of the frames of pages with no entry, with the page at the start named anew.
spec fn renamed(
    unmade: spec_fn(u64) -> contract::FrameName,
    start: u64,
    name: contract::FrameName,
) -> spec_fn(u64) -> contract::FrameName {
    |page: u64| if page == start { name } else { unmade(page) }
}

/// Taking the entry at a start out, and naming that page's frame anew, changes that page of the space alone.
proof fn lemma_take(
    pages: Map<u64, Page>,
    unmade: spec_fn(u64) -> contract::FrameName,
    made: int,
    start: u64,
    name: contract::FrameName,
)
    ensures
        ({
            let renamed = renamed(unmade, start, name);
            forall|page: u64| #[trigger] space_of(pages.remove(start), renamed, made).page(page)
                == if page == start {
                    contract::Page { frame: name, mapped: false, allowed: contract::nothing() }
                } else {
                    space_of(pages, unmade, made).page(page)
                }
        }),
{
}

/// A page that starts after another starts at least a page later.
proof fn lemma_pages_apart(start: u64, page: u64)
    requires
        contract::starts_page(start),
        contract::starts_page(page),
        start < page,
    ensures
        start + contract::PAGE_SIZE <= page,
{
    assert(start + 4096 <= page) by (nonlinear_arith)
        requires
            start % 4096 == 0,
            page % 4096 == 0,
            start < page,
    ;
}

/// The pages among the addresses before the next page are those before this one, and this one.
proof fn lemma_next(addresses: Range<u64>, start: u64)
    requires
        contract::starts_page(start),
        addresses.start <= start < addresses.end,
        contract::starts_page(addresses.start),
        contract::starts_page(addresses.end),
    ensures
        start + contract::PAGE_SIZE <= addresses.end,
        contract::starts_page((start + contract::PAGE_SIZE) as u64),
        forall|page: u64| #[trigger] contract::page_among(page, addresses) ==>
            ((page < start + contract::PAGE_SIZE) <==> (page < start || page == start)),
        contract::page_among(start, addresses),
{
    lemma_pages_apart(start, addresses.end);
    assert forall|page: u64| #[trigger] contract::page_among(page, addresses) implies
        ((page < start + contract::PAGE_SIZE) <==> (page < start || page == start)) by {
        if start < page {
            lemma_pages_apart(start, page);
        }
    }
}

impl MemorySpace for SimulatedMemory {
    type Bytes<'space> = SimulatedBytes<'space>;

    open spec fn addressable() -> Range<u64> {
        addressable()
    }

    closed spec fn token(&self) -> Loc {
        self.pages.token()
    }

    open spec fn can_run_out() -> bool {
        false
    }

    fn borrow(&self, addresses: Range<u64>) -> (result: Result<SimulatedBytes<'_>, InvalidAddressError>) {
        check_inside(&addresses)?;
        Ok(SimulatedBytes { space: self, addresses })
    }

    fn permissions(&self, address: u64, Tracked(space): Tracked<&GhostVar<contract::Space>>) -> (allowed: Permissions) {
        let (table, held) = self.pages.acquire();
        proof {
            table.model.borrow().agree(space);
        }
        let start = address - address % contract::PAGE_SIZE;
        let allowed = match table.pages.get(&start) {
            Some(page) => page.permissions,
            None => contract::no_permissions(),
        };
        held.release(table);
        allowed
    }

    fn set_permissions(
        &self,
        addresses: Range<u64>,
        allowed: Permissions,
        Tracked(space): Tracked<&mut GhostVar<contract::Space>>,
    ) -> (result: Result<(), InvalidAddressError>) {
        check_pages(&addresses)?;
        let (mut table, held) = self.pages.acquire();
        proof {
            table.model.borrow().agree(space);
        }
        let ghost before = table.space();
        let mut start = addresses.start;
        while start < addresses.end
            invariant
                contract::whole_pages(addresses, addressable()),
                addresses.start <= start <= addresses.end,
                contract::starts_page(start),
                btree_map_inv(&table.pages),
                table.model@.id() == self.pages.token(),
                table.model@@ == before,
                forall|key: u64| #[trigger] table.pages@.contains_key(key) ==> {
                    &&& table.pages@[key].wf()
                    &&& table.pages@[key].mapped ==> contract::starts_page(key)
                },
                forall|page: u64| #[trigger] table.space().page(page) == if contract::page_among(page, addresses) && page < start {
                    contract::Page { allowed, ..before.page(page) }
                } else {
                    before.page(page)
                },
            decreases addresses.end - start,
        {
            let ghost pages_before = table.pages@;
            proof {
                assert(table.space().page(start) == before.page(start));
            }
            let removed = table.pages.remove(&start);
            proof {
                assert(removed matches Some(found) ==> pages_before.contains_key(start) && found == pages_before[start]);
                assert(removed is None ==> !pages_before.contains_key(start));
            }
            let page = match removed {
                Some(found) => Page { permissions: allowed, ..found },
                None => Page { permissions: allowed, ..empty_page(Ghost((table.unmade@)(start))) },
            };
            let ghost entry = page;
            table.pages.insert(start, page);
            proof {
                lemma_put(pages_before, table.unmade@, table.made@, start, entry);
                assert(table.pages@ =~= pages_before.insert(start, entry));
                assert(entry@ == contract::Page { allowed, ..before.page(start) });
                assert(forall|page: u64| #[trigger] table.space().page(page)
                    == if page == start { entry@ } else { space_of(pages_before, table.unmade@, table.made@).page(page) });
                lemma_next(addresses, start);
            }
            start += contract::PAGE_SIZE;
        }
        proof {
            let after = table.space();
            assert(contract::permissions_set(before, after, addresses, allowed));
            table.model.borrow_mut().update(space, after);
        }
        held.release(table);
        count_change();
        Ok(())
    }

    fn map_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
        Tracked(space): Tracked<&mut GhostVar<contract::Space>>,
        Tracked(other_space): Tracked<&GhostVar<contract::Space>>,
    ) -> (result: Result<(), MapMemoryError>) {
        if let Err(error) = check_pages(&addresses) {
            return Err(MapMemoryError::InvalidAddress(error));
        }
        let other_addresses = match shift(&addresses, other_start) {
            Ok(shifted) => shifted,
            Err(error) => return Err(MapMemoryError::InvalidAddress(error)),
        };
        // The frames in reverse order, so that each is taken off the end in the order of the pages.
        let mut frames: Vec<Frame> = Vec::new();
        let mut end = other_addresses.end;
        while end > other_addresses.start
            invariant
                contract::whole_pages(other_addresses, addressable()),
                other_addresses.start <= end <= other_addresses.end,
                contract::starts_page(end),
                frames@.len() * contract::PAGE_SIZE == other_addresses.end - end,
                other_space.id() == other.token(),
                forall|index: int| 0 <= index < frames@.len() ==> #[trigger] frames@[index].name()
                    == other_space@.page((other_addresses.end - (index + 1) * contract::PAGE_SIZE) as u64).frame,
            decreases end,
        {
            end -= contract::PAGE_SIZE;
            frames.push(other.make_frame(end, Tracked(other_space)));
        }
        let (mut table, held) = self.pages.acquire();
        proof {
            table.model.borrow().agree(space);
        }
        let ghost before = table.space();
        let mut start = addresses.start;
        while start < addresses.end
            invariant
                contract::whole_pages(addresses, addressable()),
                addresses.start <= start <= addresses.end,
                contract::starts_page(start),
                frames@.len() * contract::PAGE_SIZE == addresses.end - start,
                other_addresses.start == other_start,
                other_addresses.end == other_start + (addresses.end - addresses.start),
                forall|index: int| 0 <= index < frames@.len() ==> #[trigger] frames@[index].name()
                    == other_space@.page((other_addresses.end - (index + 1) * contract::PAGE_SIZE) as u64).frame,
                btree_map_inv(&table.pages),
                table.model@.id() == self.pages.token(),
                table.model@@ == before,
                forall|key: u64| #[trigger] table.pages@.contains_key(key) ==> {
                    &&& table.pages@[key].wf()
                    &&& table.pages@[key].mapped ==> contract::starts_page(key)
                },
                forall|page: u64| #[trigger] table.space().page(page) == if contract::page_among(page, addresses) && page < start {
                    contract::Page {
                        frame: other_space@.page(contract::source(page, addresses, other_start)).frame,
                        mapped: true,
                        ..before.page(page)
                    }
                } else {
                    before.page(page)
                },
            decreases addresses.end - start,
        {
            let frame = frames.pop().unwrap();
            let ghost pages_before = table.pages@;
            proof {
                assert(table.space().page(start) == before.page(start));
                assert(frame.name() == other_space@.page(contract::source(start, addresses, other_start)).frame);
            }
            let removed = table.pages.remove(&start);
            proof {
                assert(removed matches Some(found) ==> pages_before.contains_key(start) && found == pages_before[start]);
                assert(removed is None ==> !pages_before.contains_key(start));
            }
            let found = match removed {
                Some(found) => found,
                None => empty_page(Ghost((table.unmade@)(start))),
            };
            let name = Ghost(frame.name());
            let page = Page { frame: Some(frame), mapped: true, name, ..found };
            let ghost entry = page;
            table.pages.insert(start, page);
            proof {
                lemma_put(pages_before, table.unmade@, table.made@, start, entry);
                assert(table.pages@ =~= pages_before.insert(start, entry));
                assert(forall|page: u64| #[trigger] table.space().page(page)
                    == if page == start { entry@ } else { space_of(pages_before, table.unmade@, table.made@).page(page) });
                lemma_next(addresses, start);
            }
            start += contract::PAGE_SIZE;
        }
        proof {
            let after = table.space();
            assert(contract::mapped(before, after, other_space@, addresses, other_start));
            table.model.borrow_mut().update(space, after);
        }
        held.release(table);
        count_change();
        Ok(())
    }

    fn unmap_from(
        &self,
        addresses: Range<u64>,
        Tracked(space): Tracked<&mut GhostVar<contract::Space>>,
    ) -> (result: Result<(), InvalidAddressError>) {
        check_pages(&addresses)?;
        let (mut table, held) = self.pages.acquire();
        proof {
            table.model.borrow().agree(space);
        }
        let ghost before = table.space();
        let ghost token = space.id();
        let keys = keys_among(&table.pages, &addresses);
        let ghost before_pages = table.pages@;
        proof {
            assert forall|key: u64| #![trigger before.page(key)] before_pages.contains_key(key) && before.page(key).mapped implies contract::starts_page(key) by {
                assert(table.space().page(key) == before.page(key));
            }
        }
        let mut index: usize = 0;
        let ghost mut done = Set::<u64>::empty();
        while index < keys.len()
            invariant
                contract::whole_pages(addresses, addressable()),
                index <= keys@.len(),
                keys@.no_duplicates(),
                forall|key: u64| #[trigger] keys@.contains(key) ==> addresses.start <= key < addresses.end,
                btree_map_inv(&table.pages),
                table.model@.id() == self.pages.token(),
                table.model@@ == before,
                table.made@ == before.made,
                token == space.id(),
                forall|key: u64| #[trigger] table.pages@.contains_key(key) ==> {
                    &&& table.pages@[key].wf()
                    &&& table.pages@[key].mapped ==> contract::starts_page(key)
                },
                forall|page: u64| #[trigger] done.contains(page) ==> exists|at: int| 0 <= at < index && keys@[at] == page,
                forall|at: int| 0 <= at < index ==> #[trigger] done.contains(keys@[at]),
                forall|page: u64| #[trigger] table.space().page(page) == if done.contains(page) && before.page(page).mapped {
                    contract::Page { frame: new_name(token, before, page), mapped: false, allowed: before.page(page).allowed }
                } else {
                    before.page(page)
                },
                forall|key: u64| #[trigger] before_pages.contains_key(key) <==> table.pages@.contains_key(key),
                forall|key: u64| before_pages.contains_key(key) && #[trigger] before.page(key).mapped ==> contract::starts_page(key),
            decreases keys@.len() - index,
        {
            let key = keys[index];
            let ghost pages_before = table.pages@;
            proof {
                assert(!done.contains(key)) by {
                    if done.contains(key) {
                        let at = choose|at: int| 0 <= at < index && keys@[at] == key;
                        assert(keys@[at] == keys@[index as int]);
                    }
                }
                assert(keys@.contains(key));
                assert(table.space().page(key) == before.page(key));
            }
            let removed = table.pages.remove(&key);
            proof {
                assert(removed matches Some(found) ==> pages_before.contains_key(key) && found == pages_before[key]);
                assert(removed is None ==> !pages_before.contains_key(key));
            }
            if let Some(found) = removed {
                if found.mapped {
                    let fresh_name = Ghost(new_name(token, before, key));
                    let page = Page { frame: None, mapped: false, name: fresh_name, ..found };
                    let ghost entry = page;
                    table.pages.insert(key, page);
                    proof {
                        lemma_put(pages_before, table.unmade@, table.made@, key, entry);
                        assert(table.pages@ =~= pages_before.insert(key, entry));
                    }
                } else {
                    let ghost entry = found;
                    table.pages.insert(key, found);
                    proof {
                        assert(table.pages@ =~= pages_before);
                    }
                }
            } else {
                proof {
                    assert(table.pages@ =~= pages_before);
                }
            }
            proof {
                done = done.insert(key);
            }
            index += 1;
        }
        let ghost unmapped_pages = table.space();
        table.made = Ghost(made_after(before));
        proof {
            let after = table.space();
            assert(forall|page: u64| #[trigger] after.page(page) == unmapped_pages.page(page));
            assert forall|page: u64| keys@.contains(page) implies done.contains(page) by {
                let at = choose|at: int| 0 <= at < keys@.len() && keys@[at] == page;
                assert(done.contains(keys@[at]));
            }
            assert forall|page: u64| contract::page_among(page, addresses) && before.page(page).mapped implies
                #[trigger] keys@.contains(page) by {
                assert(before_pages.contains_key(page));
            }
            assert forall|start: u64| !(contract::page_among(start, addresses) && before.page(start).mapped) implies
                #[trigger] after.page(start) == before.page(start) by {
                if done.contains(start) && before.page(start).mapped {
                    assert(keys@.contains(start));
                    assert(before_pages.contains_key(start));
                }
            }
            assert forall|start: u64| contract::page_among(start, addresses) && before.page(start).mapped implies {
                let page = #[trigger] after.page(start);
                &&& !page.mapped
                &&& page.allowed == before.page(start).allowed
                &&& contract::new_frame(page.frame, token, before, after)
            } by {
                assert(keys@.contains(start));
                assert(done.contains(start));
            }
            assert(contract::unmapped(before, after, addresses, token));
            table.model.borrow_mut().update(space, after);
        }
        held.release(table);
        count_change();
        Ok(())
    }

    fn copy_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
        Tracked(space): Tracked<&mut GhostVar<contract::Space>>,
        Tracked(other_space): Tracked<&GhostVar<contract::Space>>,
    ) -> (result: Result<(), CopyMemoryError>) {
        if let Err(error) = check_pages(&addresses) {
            return Err(CopyMemoryError::InvalidAddress(error));
        }
        let other_addresses = match shift(&addresses, other_start) {
            Ok(shifted) => shifted,
            Err(error) => return Err(CopyMemoryError::InvalidAddress(error)),
        };
        let ghost before = space@;
        let ghost from = other_space@;
        let ghost token = space.id();
        // The copied pages in reverse order, so that each is taken off the end in the order of the pages.
        let mut copied: Vec<Option<Page>> = Vec::new();
        {
            let (other_table, other_held) = other.pages.acquire();
            proof {
                other_table.model.borrow().agree(other_space);
            }
            let mut end = other_addresses.end;
            while end > other_addresses.start
                invariant
                    contract::whole_pages(other_addresses, addressable()),
                    other_addresses.start <= end <= other_addresses.end,
                    contract::starts_page(end),
                    copied@.len() * contract::PAGE_SIZE == other_addresses.end - end,
                    other_table.wf(other.pages.token()),
                    other_table.space() == from,
                    other_held.token() == other.pages.token(),
                    forall|index: int| 0 <= index < copied@.len() ==> {
                        let page = (other_addresses.end - (index + 1) * contract::PAGE_SIZE) as u64;
                        let start = (addresses.end - (index + 1) * contract::PAGE_SIZE) as u64;
                        copied_as(#[trigger] copied@[index], other_table.pages@, page, token, before, start)
                    },
                    other_addresses.start == other_start,
                    other_addresses.end == other_start + (addresses.end - addresses.start),
                    contract::whole_pages(addresses, addressable()),
                decreases end,
            {
                end -= contract::PAGE_SIZE;
                let ghost start = (addresses.start + (end - other_start)) as u64;
                #[allow(clippy::manual_map, reason = "Verus would need a specification for the closure `map` takes")]
                let page = match other_table.pages.get(&end) {
                    Some(found) => Some(copy_page(found, Ghost(new_name(token, before, start)))),
                    None => None,
                };
                copied.push(page);
            }
            other_held.release(other_table);
        }
        let (mut table, held) = self.pages.acquire();
        proof {
            table.model.borrow().agree(space);
        }
        let mut start = addresses.start;
        while start < addresses.end
            invariant
                contract::whole_pages(addresses, addressable()),
                addresses.start <= start <= addresses.end,
                contract::starts_page(start),
                copied@.len() * contract::PAGE_SIZE == addresses.end - start,
                other_addresses.start == other_start,
                other_addresses.end == other_start + (addresses.end - addresses.start),
                forall|index: int| 0 <= index < copied@.len() ==> {
                    let start = (addresses.end - (index + 1) * contract::PAGE_SIZE) as u64;
                    copied_into(#[trigger] copied@[index], token, before, from, addresses, other_start, start)
                },
                btree_map_inv(&table.pages),
                table.model@.id() == self.pages.token(),
                table.model@@ == before,
                table.made@ == before.made,
                token == space.id(),
                forall|key: u64| #[trigger] table.pages@.contains_key(key) ==> {
                    &&& table.pages@[key].wf()
                    &&& table.pages@[key].mapped ==> contract::starts_page(key)
                },
                forall|page: u64| #[trigger] table.space().page(page) == if contract::page_among(page, addresses) && page < start {
                    copy_target(token, before, from, addresses, other_start, page)
                } else {
                    before.page(page)
                },
            decreases addresses.end - start,
        {
            let ghost pages_before = table.pages@;
            let ghost unmade = table.unmade@;
            let ghost made = table.made@;
            let ghost space_before = table.space();
            proof {
                assert(addresses.end - copied@.len() * contract::PAGE_SIZE == start);
                assert(forall|page: u64| #[trigger] space_before.page(page) == space_of(pages_before, unmade, made).page(page));
            }
            let page = copied.pop().unwrap();
            match page {
                Some(page) => {
                    let ghost entry = page;
                    table.pages.insert(start, page);
                    proof {
                        lemma_put(pages_before, unmade, made, start, entry);
                        assert(entry@ == copy_target(token, before, from, addresses, other_start, start));
                        assert(table.space().page(start) == copy_target(token, before, from, addresses, other_start, start));
                    }
                },
                None => {
                    table.pages.remove(&start);
                    table.unmade = Ghost(renamed(unmade, start, new_name(token, before, start)));
                    proof {
                        lemma_take(pages_before, unmade, made, start, new_name(token, before, start));
                        assert(table.space().page(start) == copy_target(token, before, from, addresses, other_start, start));
                    }
                },
            };
            proof {
                lemma_next(addresses, start);
                assert(forall|page: u64| #[trigger] table.space().page(page) == if page == start {
                    copy_target(token, before, from, addresses, other_start, start)
                } else {
                    space_of(pages_before, unmade, made).page(page)
                });
                assert(forall|page: u64| page != start ==> #[trigger] space_before.page(page) == if contract::page_among(page, addresses) && page < start {
                    copy_target(token, before, from, addresses, other_start, page)
                } else {
                    before.page(page)
                });
            }
            start += contract::PAGE_SIZE;
        }
        let ghost copied_pages = table.space();
        table.made = Ghost(made_after(before));
        proof {
            let after = table.space();
            assert(forall|page: u64| #[trigger] after.page(page) == copied_pages.page(page));
            assert(contract::copied(before, after, from, addresses, other_start, token));
            table.model.borrow_mut().update(space, after);
        }
        held.release(table);
        count_change();
        Ok(())
    }
}

/// The page a copy gives the page at the start: its source's frame if that was mapped, else a new one.
spec fn copy_target(
    token: Loc,
    before: contract::Space,
    from: contract::Space,
    addresses: Range<u64>,
    other_start: u64,
    start: u64,
) -> contract::Page {
    let source = from.page(contract::source(start, addresses, other_start));
    if source.mapped {
        source
    } else {
        contract::Page { frame: new_name(token, before, start), ..source }
    }
}

/// Whether the copied entry is the copy of the other table's entry for the page, for the page at the start.
spec fn copied_as(
    copied: Option<Page>,
    other_pages: Map<u64, Page>,
    page: u64,
    token: Loc,
    before: contract::Space,
    start: u64,
) -> bool {
    match copied {
        Some(copy) => other_pages.contains_key(page) && copy.wf() && copy@ == if other_pages[page].mapped {
            other_pages[page]@
        } else {
            contract::Page { frame: new_name(token, before, start), ..other_pages[page]@ }
        },
        None => !other_pages.contains_key(page),
    }
}

/// Whether the copied entry, put at the start, gives the page its copy target.
spec fn copied_into(
    copied: Option<Page>,
    token: Loc,
    before: contract::Space,
    from: contract::Space,
    addresses: Range<u64>,
    other_start: u64,
    start: u64,
) -> bool {
    match copied {
        Some(copy) => copy.wf() && (copy.mapped ==> contract::starts_page(start))
            && copy@ == copy_target(token, before, from, addresses, other_start, start),
        None => {
            let target = copy_target(token, before, from, addresses, other_start, start);
            &&& target == (contract::Page { frame: new_name(token, before, start), mapped: false, allowed: contract::nothing() })
        },
    }
}

/// Returns the page as a copy carries it: a mapped frame shared, an owned one copied under the given name.
fn copy_page(page: &Page, Ghost(name): Ghost<contract::FrameName>) -> (copy: Page)
    requires
        page.wf(),
    ensures
        copy.wf(),
        copy@ == if page.mapped { page@ } else { contract::Page { frame: name, ..page@ } },
{
    let frame = match &page.frame {
        Some(frame) => if page.mapped {
            Some(frame.share())
        } else {
            Some(frame.copy(Ghost(name)))
        },
        None => None,
    };
    Page {
        frame,
        mapped: page.mapped,
        permissions: page.permissions,
        name: if page.mapped { page.name } else { Ghost(name) },
    }
}

/// Trusted: returns the keys of the table's entries among the addresses.
#[verifier::external_body]
fn keys_among(pages: &BTreeMap<u64, Page>, addresses: &Range<u64>) -> (keys: Vec<u64>)
    ensures
        keys@.no_duplicates(),
        forall|key: u64| #[trigger] keys@.contains(key) <==> pages@.contains_key(key) && addresses.start <= key < addresses.end,
{
    pages.range(addresses.clone()).map(|(&key, _)| key).collect()
}

/// The ARM64 permission types are plain values.
#[verifier::external_type_specification]
#[allow(dead_code, reason = "a declaration to Verus, never built")]
pub struct ExPagePermission(PagePermission);

#[verifier::external_type_specification]
#[allow(dead_code, reason = "a declaration to Verus, never built")]
pub struct ExDataAccess(DataAccess);

#[verifier::external_type_specification]
#[allow(dead_code, reason = "a declaration to Verus, never built")]
pub struct ExInstructionFetch(InstructionFetch);

/// Returns what the page allows as ARM64 sees it; a page that allows nothing is held unmapped.
fn to_page_permission(permissions: Permissions) -> (page: Option<PagePermission>)
    ensures
        page is None <==> permissions == contract::nothing(),
        !(page matches Some(PagePermission::Unmapped)),
        page matches Some(PagePermission::Mapped { data_access, instruction_fetch }) ==> {
            &&& (instruction_fetch == InstructionFetch::Allowed) == contract::executes(permissions)
            &&& (data_access == DataAccess::ReadWrite) == contract::writes(permissions)
            &&& (data_access == DataAccess::ReadOnly) == (contract::reads(permissions) && !contract::writes(permissions))
        },
{
    if contract::allows_nothing(permissions) {
        return None;
    }
    // ARM64 has no page that can be written and not read; Linux gives such a page read and write.
    let data_access = if contract::allows_writing(permissions) {
        DataAccess::ReadWrite
    } else if contract::allows_reading(permissions) {
        DataAccess::ReadOnly
    } else {
        DataAccess::Forbidden
    };
    let instruction_fetch = if contract::allows_executing(permissions) {
        InstructionFetch::Allowed
    } else {
        InstructionFetch::Forbidden
    };
    Some(PagePermission::Mapped { data_access, instruction_fetch })
}

} // verus!

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

impl Default for SimulatedMemory {
    fn default() -> Self {
        SimulatedMemory::new().0
    }
}

impl SimulatedMemory {
    /// Takes the copy anew unless no space changed since it was taken.
    pub(crate) fn refresh(&self, copy: &mut SpaceCopy) {
        let changes = CHANGES.load(Ordering::Acquire);
        if copy.changes != Some(changes) {
            *copy = self.take_copy(Some(changes));
        }
    }

    /// Applies a change an instruction made, whatever the permissions, to the space and to the copy it ran on.
    ///
    /// The copy stays current only if no other change came between: otherwise it is taken anew at the next refresh.
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
                // The guest's change goes through the holder of the space's token, which the executor stands for.
                self.assign_permissions(
                    addresses,
                    to_permissions(permission),
                    Tracked::assume_new(),
                );
                count_change();
                copy.changes = None;
            }
        }
    }

    /// Returns the copy of every page, current at the count of changes given (read before the pages are).
    fn take_copy(&self, changes: Option<u64>) -> SpaceCopy {
        let guard = self.lock_pages();
        let table = guard.as_ref().expect(LOST);
        let mut copy = SpaceCopy {
            changes,
            ..SpaceCopy::default()
        };
        for (&start, page) in table.pages.iter() {
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

    /// Takes the lock to read the pages: only verified code changes them.
    fn lock_pages(&self) -> MutexGuard<'_, Option<Table>> {
        self.pages
            .mutex
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn find_frame(&self, page: u64) -> Option<Frame> {
        self.lock_pages()
            .as_ref()
            .expect(LOST)
            .pages
            .get(&page)
            .and_then(|found| found.frame.as_ref().map(Frame::share))
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
            lock_frame(&self.make_frame(page, Tracked::assume_new()))[within]
                .copy_from_slice(&bytes[buffer]);
        }
        count_change()
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

fn lock_frame(frame: &Frame) -> MutexGuard<'_, Box<[u8]>> {
    frame.0.lock().unwrap_or_else(PoisonError::into_inner)
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
