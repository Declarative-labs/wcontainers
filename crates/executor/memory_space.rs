use core::ops::Range;

use creusot_std::prelude::{DeepModel, Ghost, Int, ensures, extern_spec, invariant, logic, requires};
use thiserror::Error;
use wcontainers_abi::{PAGE_SIZE, Permissions};
use wcontainers_volatile_bytes::VolatileBytes;

/// One page of a space as the contract sees it. Only proofs use it.
///
/// The bytes are not part of it: the guest may change them at any time, so no contract here promises them.
pub struct Page<Frame> {
    /// The frame whose bytes the page shows, which other pages may show too; none while the page's bytes are its own.
    pub frame: Option<Frame>,
    /// Whether the frame came from another space by `map_from`.
    pub mapped: bool,
    /// What guest code may do on the page.
    pub permissions: Permissions,
}

#[allow(
    reviewable::explicit_shared_error::InvalidAddressError,
    reason = "an address outside the space is the one way each of these methods fails"
)]
/// An address space, held with full authority over it: every address is all zeros and allows nothing until it is written or given permissions.
///
/// Bytes borrowed from the space ignore the permissions, which bind only the code executed on it.
///
/// In the contracts, `*holder` is the holder when the call starts and `^holder` the holder when it returns.
pub trait MemorySpace: Sized {
    /// A borrowed part of the space, which its guest may be changing while it is held.
    type Bytes<'space>: VolatileBytes
    where
        Self: 'space;

    /// The right to change the spaces it holds. It exists only in proofs: when the program runs it is nothing.
    type Holder;

    /// What tells one frame from another. It exists only in proofs.
    type Frame;

    // The model. Each implementation says what these are for it; everything else is fixed here.

    /// Whether the holder holds this space.
    #[logic]
    fn held(self, holder: Self::Holder) -> bool;

    /// The page of this space that starts at the address, as the holder knows it.
    #[logic]
    fn page(self, holder: Self::Holder, start: Int) -> Page<Self::Frame>;

    /// The addresses a program may use, the same in every space.
    #[logic]
    fn addressable() -> Range<Int>;

    // The rules. Sealed: no implementation can change them.

    /// Whether the addresses run forward and lie inside the space.
    #[logic(open, sealed, inline)]
    fn inside(start: Int, end: Int) -> bool {
        pearlite! { Self::addressable().start <= start && start <= end && end <= Self::addressable().end }
    }

    /// Whether the addresses lie inside the space and start and end at page boundaries.
    #[logic(open, sealed, inline)]
    fn whole_pages(start: Int, end: Int) -> bool {
        pearlite! { Self::inside(start, end) && start % PAGE_SIZE@ == 0 && end % PAGE_SIZE@ == 0 }
    }

    /// Whether the page that starts at `page` is one of the addresses' pages.
    #[logic(open, sealed, inline)]
    fn covers(addresses: Range<u64>, page: Int) -> bool {
        pearlite! { addresses.start@ <= page && page < addresses.end@ && page % PAGE_SIZE@ == 0 }
    }

    /// Whether no page of a held space showed the frame before the call: the call made it.
    #[logic(open, sealed, inline)]
    fn unseen(before: Self::Holder, frame: Self::Frame) -> bool {
        pearlite! { forall<space: Self, page: Int> space.held(before) ==> space.page(before, page).frame != Some(frame) }
    }

    /// Whether the page shows no frame, or one that no other page of a held space shows.
    #[logic(open, sealed, inline)]
    fn alone(after: Self::Holder, space: Self, page: Int) -> bool {
        pearlite! {
            space.page(after, page).frame == None
            || forall<other: Self, at: Int> other.held(after) && (other != space || at != page)
                ==> other.page(after, at).frame != space.page(after, page).frame
        }
    }

    /// Whether the page is as it was, except that a write since may have given a page with no frame a frame of its own.
    #[logic(open, sealed, inline)]
    fn kept(before: Self::Holder, after: Self::Holder, space: Self, page: Int) -> bool {
        pearlite! {
            space.page(after, page) == space.page(before, page)
            || space.page(before, page).frame == None
                && space.page(after, page).mapped == space.page(before, page).mapped
                && space.page(after, page).permissions == space.page(before, page).permissions
                && Self::alone(after, space, page)
        }
    }

    /// Whether the holder still holds every space it held.
    #[logic(open, sealed, inline)]
    fn still_held(before: Self::Holder, after: Self::Holder) -> bool {
        pearlite! { forall<space: Self> space.held(before) ==> space.held(after) }
    }

    /// Whether the pages the addresses cover allow what is given, and nothing else changed.
    #[logic(open, sealed, inline)]
    fn permissions_set(
        before: Self::Holder,
        after: Self::Holder,
        space: Self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> bool {
        pearlite! {
            forall<other: Self, page: Int> if other == space && Self::covers(addresses, page) {
                space.page(after, page).permissions == allowed
                && space.page(after, page).mapped == space.page(before, page).mapped
                && (space.page(after, page).frame == space.page(before, page).frame
                    || space.page(before, page).frame == None && Self::alone(after, space, page))
            } else {
                Self::kept(before, after, other, page)
            }
        }
    }

    /// Whether the mapped pages the addresses cover no longer show their frame, keeping their permissions, and
    /// nothing else changed: there is no way back to the frames they had been given.
    #[logic(open, sealed, inline)]
    fn mappings_ended(before: Self::Holder, after: Self::Holder, space: Self, addresses: Range<u64>) -> bool {
        pearlite! {
            forall<other: Self, page: Int> if other == space && Self::covers(addresses, page) && space.page(before, page).mapped {
                space.page(after, page) == Page { frame: None, mapped: false, permissions: space.page(before, page).permissions }
            } else {
                Self::kept(before, after, other, page)
            }
        }
    }

    /// Whether the frame is the one the other space's page showed, or one made for it that it now shows too.
    #[logic(open, sealed, inline)]
    fn given(
        before: Self::Holder,
        after: Self::Holder,
        space: Self,
        addresses: Range<u64>,
        other: Self,
        other_page: Int,
        frame: Self::Frame,
    ) -> bool {
        pearlite! {
            (other.page(before, other_page).frame == Some(frame)
                || other.page(before, other_page).frame == None && Self::unseen(before, frame))
            && (other == space && Self::covers(addresses, other_page) || other.page(after, other_page).frame == Some(frame))
        }
    }

    /// Whether each page the addresses cover shows the frame of the other space's page at the same place from
    /// `other_start`, keeping its permissions, and nothing else changed but those frames being made.
    #[logic(open, sealed, inline)]
    fn frames_shared(
        before: Self::Holder,
        after: Self::Holder,
        space: Self,
        addresses: Range<u64>,
        other: Self,
        other_start: u64,
    ) -> bool {
        pearlite! {
            forall<which: Self, page: Int> if which == space && Self::covers(addresses, page) {
                space.page(after, page).mapped
                && space.page(after, page).permissions == space.page(before, page).permissions
                && match space.page(after, page).frame {
                    None => false,
                    Some(frame) => Self::given(before, after, space, addresses, other,
                        page - addresses.start@ + other_start@, frame),
                }
            } else if which == other && other_start@ <= page && page < other_start@ + addresses.end@ - addresses.start@
                && page % PAGE_SIZE@ == 0 {
                other.page(after, page).mapped == other.page(before, page).mapped
                && other.page(after, page).permissions == other.page(before, page).permissions
                && match other.page(before, page).frame {
                    Some(frame) => other.page(after, page).frame == Some(frame),
                    None => match other.page(after, page).frame {
                        None => false,
                        Some(frame) => Self::unseen(before, frame),
                    },
                }
            } else {
                Self::kept(before, after, which, page)
            }
        }
    }

    /// Whether each page the addresses cover is the other space's page at the same place from `other_start` as it
    /// was: shared if it was mapped, otherwise with bytes of its own; and nothing else changed.
    #[logic(open, sealed, inline)]
    fn pages_copied(
        before: Self::Holder,
        after: Self::Holder,
        space: Self,
        addresses: Range<u64>,
        other: Self,
        other_start: u64,
    ) -> bool {
        pearlite! {
            forall<which: Self, page: Int> if which == space && Self::covers(addresses, page) {
                if other.page(before, page - addresses.start@ + other_start@).mapped {
                    space.page(after, page) == other.page(before, page - addresses.start@ + other_start@)
                } else {
                    !space.page(after, page).mapped
                    && space.page(after, page).permissions
                        == other.page(before, page - addresses.start@ + other_start@).permissions
                    && Self::alone(after, space, page)
                }
            } else {
                Self::kept(before, after, which, page)
            }
        }
    }

    /// Fails exactly when the addresses do not lie inside the space.
    #[ensures(match result { Ok(_) => Self::inside(addresses.start@, addresses.end@), Err(_) => !Self::inside(addresses.start@, addresses.end@) })]
    fn borrow(&self, addresses: Range<u64>) -> Result<Self::Bytes<'_>, InvalidAddressError>;

    /// Sets the bytes at the addresses to zero.
    ///
    /// Fails exactly when there are addresses and they do not lie inside the space; the bytes are the guest's and the
    /// contract says nothing of them.
    #[ensures(match result {
        Ok(()) => addresses.start@ >= addresses.end@ || Self::inside(addresses.start@, addresses.end@),
        Err(_) => addresses.start@ < addresses.end@ && !Self::inside(addresses.start@, addresses.end@),
    })]
    fn zero(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        const ZEROS: [u8; PAGE_SIZE as usize] = [0; PAGE_SIZE as usize];
        let mut start = addresses.start;
        #[invariant(addresses.start@ <= start@)]
        #[invariant(start@ == addresses.start@ || start@ <= addresses.end@ && Self::inside(addresses.start@, start@))]
        while start < addresses.end {
            let end = addresses.end.min(start.saturating_add(PAGE_SIZE));
            let bytes = self.borrow(start..end)?;
            // Changed: Creusot has no contract for a range of an array, but has one for a range of a slice.
            let zeros: &[u8] = &ZEROS;
            bytes.write(&zeros[..(end - start) as usize]);
            start = end;
        }
        Ok(())
    }

    /// Sets what the pages any of the addresses lie on allow. Fails exactly when they are not whole pages inside the space.
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
        holder: Ghost<&mut Self::Holder>,
    ) -> Result<(), InvalidAddressError>;

    /// Maps the pages of the other space that start at the given address into the addresses in this space.
    ///
    /// Fails with an invalid address exactly when the addresses here or there are not whole pages inside the space.
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
        holder: Ghost<&mut Self::Holder>,
    ) -> Result<(), MapMemoryError>;

    /// Ends the mapping of the addresses, which become zeroed out wherever they were previously mapped from.
    ///
    /// Fails exactly when they are not whole pages inside the space.
    #[requires(self.held(**holder))]
    #[ensures(Self::still_held(**holder, ^*holder))]
    #[ensures(match result {
        Ok(()) => Self::whole_pages(addresses.start@, addresses.end@)
            && Self::mappings_ended(**holder, ^*holder, *self, addresses),
        Err(_) => !Self::whole_pages(addresses.start@, addresses.end@) && ^*holder == **holder,
    })]
    fn unmap_from(&self, addresses: Range<u64>, holder: Ghost<&mut Self::Holder>) -> Result<(), InvalidAddressError>;

    /// Copies the bytes, permissions and mappings of the other space that start at the given address into the addresses of this space.
    ///
    /// Fails with an invalid address exactly when the addresses here or there are not whole pages inside the space.
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
        holder: Ghost<&mut Self::Holder>,
    ) -> Result<(), CopyMemoryError>;
}

// Trusted: `VolatileBytes` has no contract yet, and Creusot gives a method without one a precondition no caller can
// meet. This says `write` may be called with any bytes, and promises nothing of what it does.
extern_spec! {
    mod wcontainers_volatile_bytes {
        trait VolatileBytes {
            #[ensures(true)]
            fn write(&self, bytes: &[u8]) -> usize;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error, DeepModel)]
#[error("the addresses do not all lie inside the space")]
pub struct InvalidAddressError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error, DeepModel)]
pub enum MapMemoryError {
    #[error(transparent)]
    InvalidAddress(InvalidAddressError),
    #[error("the host has no memory left for the mapping")]
    OutOfMemory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error, DeepModel)]
pub enum CopyMemoryError {
    #[error(transparent)]
    InvalidAddress(InvalidAddressError),
    #[error("the host has no memory left for the copy")]
    OutOfMemory,
}
