use core::ops::Range;

use thiserror::Error;
use vstd::prelude::*;
use vstd::resource::Loc;
use vstd::resource::ghost_var::GhostVar;
use wcontainers_abi::Permissions;
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

/// The number of bytes in a page.
pub const PAGE_SIZE: u64 = 4096;

// ---------------------------------------------------------------- the model

/// One page of a space, as the contract sees it.
pub ghost struct Page {
    /// The frame that holds the page's bytes: pages that hold the same frame, in one space or two, share their bytes.
    pub frame: int,
    /// Whether the page was mapped from another space, so that an unmap ends it and a copy shares it.
    pub mapped: bool,
    /// What guest code may do on the page.
    pub allowed: Permissions,
}

/// A space, as the contract sees it: each of its pages, by the page's first address.
pub ghost struct Space {
    pub pages: spec_fn(u64) -> Page,
}

impl Space {
    /// The page that starts at the address.
    pub open spec fn page(self, start: u64) -> Page {
        (self.pages)(start)
    }
}

/// What a page allows when it allows nothing.
pub uninterp spec fn nothing() -> Permissions;

/// Whether the address is the first of a page.
pub open spec fn starts_page(address: u64) -> bool {
    address % PAGE_SIZE == 0
}

/// Whether the page that starts at the address lies among the addresses.
pub open spec fn page_among(start: u64, addresses: Range<u64>) -> bool {
    starts_page(start) && addresses.start <= start < addresses.end
}

/// Whether the addresses run forward and lie inside the bounds.
pub open spec fn inside(addresses: Range<u64>, bounds: Range<u64>) -> bool {
    bounds.start <= addresses.start <= addresses.end <= bounds.end
}

/// Whether the addresses lie inside the bounds and start and end on a page boundary.
pub open spec fn whole_pages(addresses: Range<u64>, bounds: Range<u64>) -> bool {
    inside(addresses, bounds) && starts_page(addresses.start) && starts_page(addresses.end)
}

/// Whether the other space's addresses, as many as these and from its start, are whole pages inside the bounds.
pub open spec fn source_fits(addresses: Range<u64>, other_start: u64, bounds: Range<u64>) -> bool {
    let end = other_start + (addresses.end - addresses.start);
    end <= u64::MAX && whole_pages(Range { start: other_start, end: end as u64 }, bounds)
}

/// The page of the other space that a page among the addresses is mapped or copied from.
pub open spec fn source(start: u64, addresses: Range<u64>, other_start: u64) -> u64 {
    (start - addresses.start + other_start) as u64
}

/// Whether the space holds the frame on one of its pages.
pub open spec fn holds(space: Space, frame: int) -> bool {
    exists|start: u64| starts_page(start) && #[trigger] space.page(start).frame == frame
}

/// Whether the frame is a new one: neither space held it.
pub open spec fn new_frame(frame: int, this: Space, other: Space) -> bool {
    !holds(this, frame) && !holds(other, frame)
}

// ---------------------------------------------------------------- the rules

/// A new space: no page is mapped, and no page allows anything.
pub open spec fn new_space(space: Space) -> bool {
    forall|start: u64| !(#[trigger] space.page(start)).mapped && space.page(start).allowed == nothing()
}

/// Every page outside the addresses is as it was.
pub open spec fn unchanged_outside(before: Space, after: Space, addresses: Range<u64>) -> bool {
    forall|start: u64| !page_among(start, addresses) ==> #[trigger] after.page(start) == before.page(start)
}

/// Each page among the addresses allows what was set, and keeps its frame and mapping.
pub open spec fn permissions_set(before: Space, after: Space, addresses: Range<u64>, allowed: Permissions) -> bool {
    &&& unchanged_outside(before, after, addresses)
    &&& forall|start: u64| page_among(start, addresses) ==>
            #[trigger] after.page(start) == Page { allowed, ..before.page(start) }
}

/// Each page among the addresses holds the frame of its source page in the other space, and keeps what it allows.
pub open spec fn mapped(before: Space, after: Space, other: Space, addresses: Range<u64>, other_start: u64) -> bool {
    &&& unchanged_outside(before, after, addresses)
    &&& forall|start: u64| #![trigger after.page(start)] page_among(start, addresses) ==> {
            let from = other.page(source(start, addresses, other_start));
            after.page(start) == Page { frame: from.frame, mapped: true, ..before.page(start) }
        }
}

/// Each mapped page among the addresses holds a new frame of its own and is no longer mapped; every other page is as it was.
pub open spec fn unmapped(before: Space, after: Space, addresses: Range<u64>) -> bool {
    &&& forall|start: u64| #![trigger after.page(start)] !(page_among(start, addresses) && before.page(start).mapped) ==>
            after.page(start) == before.page(start)
    &&& forall|start: u64| #![trigger after.page(start)] page_among(start, addresses) && before.page(start).mapped ==> {
            let page = after.page(start);
            &&& !page.mapped
            &&& page.allowed == before.page(start).allowed
            &&& new_frame(page.frame, before, before)
        }
    &&& forall|start: u64, other_start: u64|
            #![trigger after.page(start), after.page(other_start)]
            page_among(start, addresses) && before.page(start).mapped
            && page_among(other_start, addresses) && before.page(other_start).mapped
            && start != other_start ==> after.page(start).frame != after.page(other_start).frame
}

/// Whether a page among the addresses is copied from a page of the other space that is not mapped.
pub open spec fn copied_own(start: u64, other: Space, addresses: Range<u64>, other_start: u64) -> bool {
    page_among(start, addresses) && !other.page(source(start, addresses, other_start)).mapped
}

/// Each page among the addresses allows what its source page allows and shares its frame if that was mapped, else holds a new frame of its own.
pub open spec fn copied(before: Space, after: Space, other: Space, addresses: Range<u64>, other_start: u64) -> bool {
    &&& unchanged_outside(before, after, addresses)
    &&& forall|start: u64| #![trigger after.page(start)] page_among(start, addresses) ==> {
            let from = other.page(source(start, addresses, other_start));
            let page = after.page(start);
            &&& page.allowed == from.allowed
            &&& page.mapped == from.mapped
            &&& from.mapped ==> page.frame == from.frame
            &&& !from.mapped ==> new_frame(page.frame, before, other)
        }
    &&& forall|start: u64, other_page: u64|
            #![trigger after.page(start), after.page(other_page)]
            copied_own(start, other, addresses, other_start) && copied_own(other_page, other, addresses, other_start)
            && start != other_page ==> after.page(start).frame != after.page(other_page).frame
}

// ---------------------------------------------------------------- the interface

#[allow(
    reviewable::explicit_shared_error::InvalidAddressError,
    reason = "an address outside the space is the one way each of these methods fails"
)]
/// An address space, held with full authority over it: every address is all zeros and allows nothing until it is written or given permissions.
///
/// Bytes borrowed from the space ignore the permissions, which bind only the code executed on it.
/// The page methods take the token that holds the space's model: its holder is the one party that changes the space's pages.
pub trait MemorySpace: Sized {
    /// A borrowed part of the space, which its guest may be changing while it is held.
    type Bytes<'space>: VolatileBytes
    where
        Self: 'space;

    /// The addresses a program can use in this space.
    spec fn addressable() -> Range<u64>;

    /// Which token holds this space's model.
    spec fn token(&self) -> Loc;

    fn borrow(&self, addresses: Range<u64>) -> (result: Result<Self::Bytes<'_>, InvalidAddressError>)
        ensures
            result is Ok <==> inside(addresses, Self::addressable()),
    ;

    /// Sets the bytes at the addresses to zero.
    fn zero(&self, addresses: Range<u64>) -> (result: Result<(), InvalidAddressError>)
        ensures
            result is Ok <==> addresses.end <= addresses.start || inside(addresses, Self::addressable()),
    {
        let zeros: [u8; PAGE_SIZE as usize] = [0; PAGE_SIZE as usize];
        let mut start = addresses.start;
        while start < addresses.end
            invariant
                addresses.start <= start,
                addresses.start < start ==> inside(Range { start: addresses.start, end: start }, Self::addressable()),
                start <= addresses.end || addresses.end <= addresses.start,
            decreases addresses.end - start,
        {
            let end = addresses.end.min(start.saturating_add(PAGE_SIZE));
            let bytes = match self.borrow(start..end) {
                Ok(bytes) => bytes,
                Err(error) => return Err(error),
            };
            bytes.write(&zeros[..(end - start) as usize]);
            start = end;
        }
        Ok(())
    }

    fn set_permissions(
        &self,
        addresses: Range<u64>,
        allowed: Permissions,
        Tracked(space): Tracked<&mut GhostVar<Space>>,
    ) -> (result: Result<(), InvalidAddressError>)
        requires
            old(space).id() == self.token(),
        ensures
            ({
                let before = old(space)@;
                let after = final(space)@;
                &&& final(space).id() == old(space).id()
                &&& result is Ok <==> whole_pages(addresses, Self::addressable())
                &&& result is Ok ==> permissions_set(before, after, addresses, allowed)
                &&& result is Err ==> after == before
            }),
    ;

    /// Maps the pages of the other space that start at the given address into the addresses in this space.
    fn map_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
        Tracked(space): Tracked<&mut GhostVar<Space>>,
        Tracked(other_space): Tracked<&GhostVar<Space>>,
    ) -> (result: Result<(), MapMemoryError>)
        requires
            old(space).id() == self.token(),
            other_space.id() == other.token(),
        ensures
            ({
                let before = old(space)@;
                let after = final(space)@;
                let fits = whole_pages(addresses, Self::addressable())
                    && source_fits(addresses, other_start, Self::addressable());
                &&& final(space).id() == old(space).id()
                &&& (result matches Err(MapMemoryError::InvalidAddress(_))) <==> !fits
                &&& result is Ok ==> mapped(before, after, other_space@, addresses, other_start)
                &&& result is Err ==> after == before
            }),
    ;

    /// Ends the mapping of the addresses, which become zeroed out wherever they were previously mapped from.
    fn unmap_from(
        &self,
        addresses: Range<u64>,
        Tracked(space): Tracked<&mut GhostVar<Space>>,
    ) -> (result: Result<(), InvalidAddressError>)
        requires
            old(space).id() == self.token(),
        ensures
            ({
                let before = old(space)@;
                let after = final(space)@;
                &&& final(space).id() == old(space).id()
                &&& result is Ok <==> whole_pages(addresses, Self::addressable())
                &&& result is Ok ==> unmapped(before, after, addresses)
                &&& result is Err ==> after == before
            }),
    ;

    /// Copies the bytes, permissions and mappings of the other space that start at the given address into the addresses of this space.
    fn copy_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
        Tracked(space): Tracked<&mut GhostVar<Space>>,
        Tracked(other_space): Tracked<&GhostVar<Space>>,
    ) -> (result: Result<(), CopyMemoryError>)
        requires
            old(space).id() == self.token(),
            other_space.id() == other.token(),
        ensures
            ({
                let before = old(space)@;
                let after = final(space)@;
                let fits = whole_pages(addresses, Self::addressable())
                    && source_fits(addresses, other_start, Self::addressable());
                &&& final(space).id() == old(space).id()
                &&& (result matches Err(CopyMemoryError::InvalidAddress(_))) <==> !fits
                &&& result is Ok ==> copied(before, after, other_space@, addresses, other_start)
                &&& result is Err ==> after == before
            }),
    ;
}

// ---------------------------------------------------------------- trusted: plain values the contract names

/// Permissions is a plain value.
#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExPermissions(Permissions);

/// Returns what a page allows when it allows nothing.
#[verifier::external_body]
pub fn no_permissions() -> (allowed: Permissions)
    ensures
        allowed == nothing(),
{
    Permissions::empty()
}

/// The bytes a borrow lends: the contract promises nothing about them yet.
#[verifier::external_trait_specification]
pub trait ExVolatileBytes {
    type ExternalTraitSpecificationFor: VolatileBytes;

    fn write(&self, bytes: &[u8]) -> usize;
}

/// The errors are plain values.
#[verifier::external_type_specification]
pub struct ExInvalidAddressError(InvalidAddressError);

#[verifier::external_type_specification]
pub struct ExMapMemoryError(MapMemoryError);

#[verifier::external_type_specification]
pub struct ExCopyMemoryError(CopyMemoryError);

} // verus!

/// The size of a page, as the contract counts it: the same as the ABI's.
const _: () = assert!(PAGE_SIZE == wcontainers_abi::PAGE_SIZE);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("the addresses do not all lie inside the space")]
pub struct InvalidAddressError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum MapMemoryError {
    #[error(transparent)]
    InvalidAddress(InvalidAddressError),
    #[error("the host has no memory left for the mapping")]
    OutOfMemory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CopyMemoryError {
    #[error(transparent)]
    InvalidAddress(InvalidAddressError),
    #[error("the host has no memory left for the copy")]
    OutOfMemory,
}
