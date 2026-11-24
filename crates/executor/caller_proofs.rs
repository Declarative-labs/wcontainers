//! What a caller can count on, proven from the trait's contract alone: these hold for every implementation.

use core::ops::Range;

use vstd::prelude::*;
use vstd::resource::ghost_var::GhostVar;
use wcontainers_abi::Permissions;

use crate::memory_space::*;

verus! {

/// The one page that starts at the address.
pub open spec fn one_page(start: u64) -> Range<u64> {
    Range { start, end: (start + PAGE_SIZE) as u64 }
}

/// A page another space maps shares that space's frame; once unmapped, it holds a frame of its own, and keeps what it allowed.
pub fn map_then_unmap<S: MemorySpace>(
    this: &S,
    other: &S,
    start: u64,
    Tracked(mine): Tracked<&mut GhostVar<Space>>,
    Tracked(theirs): Tracked<&GhostVar<Space>>,
)
    requires
        old(mine).id() == this.token(),
        theirs.id() == other.token(),
        start + PAGE_SIZE <= u64::MAX,
        whole_pages(one_page(start), S::addressable()),
{
    let ghost allowed = mine@.page(start).allowed;
    let range = start..start + PAGE_SIZE;
    if this.map_from(range, other, start, Tracked(mine), Tracked(theirs)).is_ok() {
        assert(page_among(start, one_page(start)));
        let ghost shared = theirs@.page(start).frame;
        assert(mine@.page(start).frame == shared);
        assert(mine@.page(start).mapped);
        let ghost mapped = mine@;
        let range = start..start + PAGE_SIZE;
        let unmapped = this.unmap_from(range, Tracked(mine));
        assert(unmapped is Ok);
        assert(holds(mapped, shared));
        assert(mine@.page(start).frame != shared);
        assert(!mine@.page(start).mapped);
        assert(mine@.page(start).allowed == allowed);
    }
}

/// A copy of a page that was not mapped holds a frame the other space does not hold, and allows what the other page allows.
pub fn copy_does_not_share<S: MemorySpace>(
    this: &S,
    other: &S,
    start: u64,
    other_start: u64,
    Tracked(mine): Tracked<&mut GhostVar<Space>>,
    Tracked(theirs): Tracked<&GhostVar<Space>>,
)
    requires
        old(mine).id() == this.token(),
        theirs.id() == other.token(),
        start + PAGE_SIZE <= u64::MAX,
        other_start + PAGE_SIZE <= u64::MAX,
        whole_pages(one_page(start), S::addressable()),
        whole_pages(one_page(other_start), S::addressable()),
        !theirs@.page(other_start).mapped,
{
    let range = start..start + PAGE_SIZE;
    if this.copy_from(range, other, other_start, Tracked(mine), Tracked(theirs)).is_ok() {
        assert(page_among(start, one_page(start)));
        assert(source(start, one_page(start), other_start) == other_start);
        assert(!holds(theirs@, mine@.page(start).frame));
        assert(mine@.page(start).frame != theirs@.page(other_start).frame);
        assert(mine@.page(start).allowed == theirs@.page(other_start).allowed);
    }
}

/// The permissions set on whole pages inside the space are the permissions found there, and nowhere else changes.
pub fn permissions_set_are_found<S: MemorySpace>(
    this: &S,
    addresses: Range<u64>,
    allowed: Permissions,
    Tracked(mine): Tracked<&mut GhostVar<Space>>,
)
    requires
        old(mine).id() == this.token(),
        whole_pages(addresses, S::addressable()),
{
    let ghost before = mine@;
    let result = this.set_permissions(addresses, allowed, Tracked(mine));
    assert(result is Ok);
    assert(forall|start: u64| page_among(start, addresses) ==> (#[trigger] mine@.page(start)).allowed == allowed);
    assert(forall|start: u64| !page_among(start, addresses) ==> #[trigger] mine@.page(start) == before.page(start));
}

/// A refused call changes nothing: addresses that do not start on a page are refused by every page method.
pub fn refused_changes_nothing<S: MemorySpace>(
    this: &S,
    other: &S,
    addresses: Range<u64>,
    allowed: Permissions,
    Tracked(mine): Tracked<&mut GhostVar<Space>>,
    Tracked(theirs): Tracked<&GhostVar<Space>>,
)
    requires
        old(mine).id() == this.token(),
        theirs.id() == other.token(),
        !starts_page(addresses.start),
{
    let ghost before = mine@;
    let (start, end) = (addresses.start, addresses.end);
    let a = this.set_permissions(start..end, allowed, Tracked(mine));
    let b = this.map_from(start..end, other, 0, Tracked(mine), Tracked(theirs));
    let c = this.unmap_from(start..end, Tracked(mine));
    let d = this.copy_from(start..end, other, 0, Tracked(mine), Tracked(theirs));
    assert(a is Err && b is Err && c is Err && d is Err);
    assert(mine@ == before);
}

/// Mapping and unmapping never grant permissions: a new space still allows nothing after any of them.
pub fn mapping_grants_nothing<S: MemorySpace>(
    this: &S,
    other: &S,
    addresses: Range<u64>,
    other_start: u64,
    Tracked(mine): Tracked<&mut GhostVar<Space>>,
    Tracked(theirs): Tracked<&GhostVar<Space>>,
)
    requires
        old(mine).id() == this.token(),
        theirs.id() == other.token(),
        new_space(old(mine)@),
    ensures
        forall|start: u64| (#[trigger] final(mine)@.page(start)).allowed == nothing(),
{
    let (start, end) = (addresses.start, addresses.end);
    let _ = this.map_from(start..end, other, other_start, Tracked(mine), Tracked(theirs));
    assert(forall|start: u64| (#[trigger] mine@.page(start)).allowed == nothing());
    let _ = this.unmap_from(start..end, Tracked(mine));
}

} // verus!
