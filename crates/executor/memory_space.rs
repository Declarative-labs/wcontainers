use core::ops::Range;

use thiserror::Error;
use wcontainers_abi::{PAGE_SIZE, Permissions};
use wcontainers_volatile_bytes::VolatileBytes;

#[allow(
    reviewable::explicit_shared_error::InvalidAddressError,
    reason = "an address outside the space is the one way each of these methods fails"
)]
/// An address space, shared by everyone who runs on it: every address is all zeros and allows nothing until it is written or given permissions.
///
/// Bytes borrowed from the space ignore the permissions, which bind only the code executed on it.
#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait MemorySpace: Sized {
    /// A borrowed part of the space, which its guest may be changing while it is held.
    type Bytes<'space>: VolatileBytes
    where
        Self: 'space;

    /// The one holder of the space's pages, made with the space.
    type PageTable: PageTable;

    fn borrow(&self, addresses: Range<u64>) -> Result<Self::Bytes<'_>, InvalidAddressError>;

    /// Sets the bytes at the addresses to zero.
    fn zero(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        const ZEROS: [u8; PAGE_SIZE as usize] = [0; PAGE_SIZE as usize];
        let mut start = addresses.start;
        while start < addresses.end {
            let end = addresses.end.min(start.saturating_add(PAGE_SIZE));
            let bytes = self.borrow(start..end)?;
            bytes.write(&ZEROS[..(end - start) as usize]);
            start = end;
        }
        Ok(())
    }
}

#[allow(
    reviewable::explicit_shared_error::InvalidAddressError,
    reason = "an address outside the space is the one way each of these methods fails"
)]
/// The pages of one address space, held by the one party that may change them.
#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait PageTable: Sized {
    fn set_permissions(
        &mut self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), InvalidAddressError>;

    /// Maps the pages of the other table's space that start at the given address into the addresses in this space.
    fn map_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError>;

    /// Ends the mapping of the addresses, which become zeroed out wherever they were previously mapped from.
    fn unmap_from(&mut self, addresses: Range<u64>) -> Result<(), InvalidAddressError>;

    /// Copies the bytes, permissions and mappings of the other table's space that start at the given address into the addresses of this space.
    fn copy_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError>;
}

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
