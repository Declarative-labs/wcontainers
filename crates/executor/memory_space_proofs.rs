//! Caller proofs: what any `MemorySpace` guarantees, proven from the contract alone. Built only by Creusot.
//!
//! They also make Creusot 0.15.0 record each method's contract in this crate, which an implementation in another
//! crate needs: without a call here it crashes ("DefId::expect_local ... isn't local").

use core::ops::Range;

use creusot_std::prelude::{
    Ghost, Int, Snapshot, ensures, ghost, proof_assert, requires, snapshot,
};
use wcontainers_abi::Permissions;

use crate::memory_space::MemorySpace;

/// After a space sets the permissions of whole pages, each of those pages allows exactly them.
#[requires(space.held(**holder))]
#[requires(S::covers(addresses, *page))]
#[ensures(S::whole_pages(addresses.start@, addresses.end@) ==> space.page(^*holder, *page).permissions == allowed)]
pub fn permissions_set_are_found<S: MemorySpace>(
    space: &S,
    addresses: Range<u64>,
    allowed: Permissions,
    page: Snapshot<Int>,
    holder: Ghost<&mut S::Holder>,
) {
    let _ = space.set_permissions(addresses, allowed, holder);
}

/// After `map_from`, a page and the other space's page show one frame, so each sees the other's writes; after
/// `unmap_from`, the page shows no frame, so it no longer sees them, while the other space keeps its frame.
#[requires(space.held(**holder) && other.held(**holder) && *space != *other)]
#[requires(S::covers(addresses, *page))]
pub fn map_shares_and_unmap_ends<S: MemorySpace>(
    space: &S,
    addresses: Range<u64>,
    other: &S,
    other_start: u64,
    page: Snapshot<Int>,
    mut holder: Ghost<&mut S::Holder>,
) {
    let other_page = snapshot!(*page - addresses.start@ + other_start@);
    let unmapped = addresses.start..addresses.end;
    if space
        .map_from(addresses, other, other_start, ghost!(&mut **holder))
        .is_err()
    {
        return;
    }
    let shared = snapshot!(space.page(**holder, *page).frame);
    proof_assert!(*shared != None && *shared == other.page(**holder, *other_page).frame);
    let ended = space.unmap_from(unmapped, ghost!(&mut **holder));
    proof_assert!(ended == Ok(()));
    proof_assert!(space.page(**holder, *page).frame == None);
    proof_assert!(other.page(**holder, *other_page).frame == *shared);
}

/// A page copied from a page that was not mapped shows no frame that any other page shows: a copy does not share.
#[requires(space.held(**holder) && other.held(**holder))]
#[requires(S::covers(addresses, *page))]
#[requires(!other.page(**holder, *page - addresses.start@ + other_start@).mapped)]
#[ensures(result ==> S::alone(^*holder, *space, *page) && !space.page(^*holder, *page).mapped)]
pub fn copy_does_not_share<S: MemorySpace>(
    space: &S,
    addresses: Range<u64>,
    other: &S,
    other_start: u64,
    page: Snapshot<Int>,
    holder: Ghost<&mut S::Holder>,
) -> bool {
    space
        .copy_from(addresses, other, other_start, holder)
        .is_ok()
}

/// Addresses that end past the space are refused by every method, and the holder is left as it was.
#[requires(space.held(**holder) && other.held(**holder))]
#[requires(addresses.start@ <= addresses.end@ && addresses.end@ > S::addressable().end)]
#[ensures(^*holder == **holder)]
pub fn outside_is_refused<S: MemorySpace>(
    space: &S,
    addresses: Range<u64>,
    other: &S,
    allowed: Permissions,
    mut holder: Ghost<&mut S::Holder>,
) {
    let start = addresses.start;
    let end = addresses.end;
    let borrowed = space.borrow(start..end);
    proof_assert!(match borrowed {
        Ok(_) => false,
        Err(_) => true,
    });
    let zeroed = space.zero(start..end);
    proof_assert!(start@ < end@ ==> zeroed != Ok(()));
    let set = space.set_permissions(start..end, allowed, ghost!(&mut **holder));
    proof_assert!(set != Ok(()));
    let mapped = space.map_from(start..end, other, start, ghost!(&mut **holder));
    proof_assert!(mapped != Ok(()));
    let unmapped = space.unmap_from(start..end, ghost!(&mut **holder));
    proof_assert!(unmapped != Ok(()));
    let copied = space.copy_from(start..end, other, start, ghost!(&mut **holder));
    proof_assert!(copied != Ok(()));
}
