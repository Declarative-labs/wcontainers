//! What a caller can count on, proven from the trait's contract alone: these hold for every implementation.

use core::ops::Range;

use vstd::prelude::*;
use vstd::resource::Loc;
use vstd::resource::ghost_var::GhostVar;
use wcontainers_abi::Permissions;

use crate::memory_space::*;

verus! {

// ---------------------------------------------------------------- isolation: what no guest input can break

/// Whether the frame was made by one of the spaces, with a number that space had already used.
pub open spec fn made_among(spaces: Map<Loc, Space>, frame: FrameName) -> bool {
    spaces.contains_key(frame.space) && frame.number < spaces[frame.space].made
}

/// Whether every frame the spaces hold was made by one of them, with a number its maker had already used.
pub open spec fn isolated(spaces: Map<Loc, Space>) -> bool {
    forall|token: Loc, start: u64| #![trigger spaces[token].page(start)]
        spaces.contains_key(token) ==> made_among(spaces, spaces[token].page(start).frame)
}

/// Two spaces, by their tokens.
pub open spec fn two(token: Loc, space: Space, other_token: Loc, other: Space) -> Map<Loc, Space> {
    Map::empty().insert(token, space).insert(other_token, other)
}

/// A new space joins isolated spaces and keeps them isolated.
pub proof fn new_space_keeps_isolation(spaces: Map<Loc, Space>, token: Loc, space: Space)
    requires
        isolated(spaces),
        !spaces.contains_key(token),
        new_space(space, token),
    ensures
        isolated(spaces.insert(token, space)),
{
    let after = spaces.insert(token, space);
    assert forall|held: Loc, start: u64| #![trigger after[held].page(start)]
        after.contains_key(held) implies made_among(after, after[held].page(start).frame) by {
        if held == token {
            assert(space.page(start).frame.number == start as int);
        } else {
            assert(made_among(spaces, spaces[held].page(start).frame));
        }
    }
}

/// A change to one space keeps the spaces isolated when each of its pages holds a frame one of them made, or a new frame of its own.
pub proof fn change_keeps_isolation(spaces: Map<Loc, Space>, token: Loc, after: Space)
    requires
        isolated(spaces),
        spaces.contains_key(token),
        spaces[token].made <= after.made,
        forall|start: u64| #![trigger after.page(start)]
            made_among(spaces, after.page(start).frame) || new_frame(after.page(start).frame, token, spaces[token], after),
    ensures
        isolated(spaces.insert(token, after)),
{
    let changed = spaces.insert(token, after);
    assert forall|held: Loc, start: u64| #![trigger changed[held].page(start)]
        changed.contains_key(held) implies made_among(changed, changed[held].page(start).frame) by {
        if held != token {
            assert(made_among(spaces, spaces[held].page(start).frame));
        }
    }
}

/// Setting permissions, mapping, unmapping or copying, as the contract allows, keeps isolated spaces isolated.
pub proof fn page_methods_keep_isolation(
    spaces: Map<Loc, Space>,
    token: Loc,
    other_token: Loc,
    after: Space,
    addresses: Range<u64>,
    other_start: u64,
    allowed: Permissions,
)
    requires
        isolated(spaces),
        spaces.contains_key(token),
        spaces.contains_key(other_token),
        ({
            let before = spaces[token];
            let other = spaces[other_token];
            ||| permissions_set(before, after, addresses, allowed)
            ||| mapped(before, after, other, addresses, other_start)
            ||| unmapped(before, after, addresses, token)
            ||| copied(before, after, other, addresses, other_start, token)
        }),
    ensures
        isolated(spaces.insert(token, after)),
{
    let before = spaces[token];
    let other = spaces[other_token];
    assert forall|start: u64| #![trigger after.page(start)]
        made_among(spaces, after.page(start).frame) || new_frame(after.page(start).frame, token, before, after) by {
        assert(made_among(spaces, before.page(start).frame));
        assert(made_among(spaces, other.page(source(start, addresses, other_start)).frame));
    }
    change_keeps_isolation(spaces, token, after);
}

/// A frame a change makes is held by no page of any isolated space before the change: no guest can reach it through another space.
pub proof fn new_frames_are_unshared(spaces: Map<Loc, Space>, token: Loc, after: Space, frame: FrameName)
    requires
        isolated(spaces),
        spaces.contains_key(token),
        new_frame(frame, token, spaces[token], after),
    ensures
        forall|held: Loc, start: u64| #![trigger spaces[held].page(start)]
            spaces.contains_key(held) ==> spaces[held].page(start).frame != frame,
{
}

// ---------------------------------------------------------------- calls a verified caller makes

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
        old(mine).id() != theirs.id(),
        isolated(two(old(mine).id(), old(mine)@, theirs.id(), theirs@)),
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
        let ghost spaces = two(mine.id(), mapped, theirs.id(), theirs@);
        proof {
            let first = two(mine.id(), old(mine)@, theirs.id(), theirs@);
            page_methods_keep_isolation(first, mine.id(), theirs.id(), mapped, one_page(start), start, allowed);
            assert(first.insert(mine.id(), mapped) =~= spaces);
        }
        let range = start..start + PAGE_SIZE;
        let unmapped = this.unmap_from(range, Tracked(mine));
        assert(unmapped is Ok);
        proof {
            assert(spaces[mine.id()].page(start).frame == shared);
            new_frames_are_unshared(spaces, mine.id(), mine@, mine@.page(start).frame);
        }
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
        old(mine).id() != theirs.id(),
        isolated(two(old(mine).id(), old(mine)@, theirs.id(), theirs@)),
        start + PAGE_SIZE <= u64::MAX,
        other_start + PAGE_SIZE <= u64::MAX,
        whole_pages(one_page(start), S::addressable()),
        whole_pages(one_page(other_start), S::addressable()),
        !theirs@.page(other_start).mapped,
{
    let ghost spaces = two(mine.id(), mine@, theirs.id(), theirs@);
    let range = start..start + PAGE_SIZE;
    if this.copy_from(range, other, other_start, Tracked(mine), Tracked(theirs)).is_ok() {
        assert(page_among(start, one_page(start)));
        assert(source(start, one_page(start), other_start) == other_start);
        proof {
            new_frames_are_unshared(spaces, mine.id(), mine@, mine@.page(start).frame);
            assert(spaces[theirs.id()].page(other_start) == theirs@.page(other_start));
        }
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
    let (start, end) = (addresses.start, addresses.end);
    let result = this.set_permissions(start..end, allowed, Tracked(mine));
    assert(result is Ok);
    assert(forall|start: u64| page_among(start, addresses) ==> (#[trigger] mine@.page(start)).allowed == allowed);
    assert(forall|start: u64| !page_among(start, addresses) ==> #[trigger] mine@.page(start) == before.page(start));
    if start < end {
        let found = this.permissions(start, Tracked(&*mine));
        assert(page_start(start) == start);
        assert(page_among(start, addresses));
        assert(found == allowed);
    }
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
        new_space(old(mine)@, old(mine).id()),
    ensures
        forall|start: u64| (#[trigger] final(mine)@.page(start)).allowed == nothing(),
{
    let (start, end) = (addresses.start, addresses.end);
    let _ = this.map_from(start..end, other, other_start, Tracked(mine), Tracked(theirs));
    assert(forall|start: u64| (#[trigger] mine@.page(start)).allowed == nothing());
    let _ = this.unmap_from(start..end, Tracked(mine));
}

} // verus!
