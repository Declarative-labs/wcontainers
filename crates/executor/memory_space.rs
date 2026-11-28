use core::ops::Range;

use vstd::prelude::*;
use wcontainers_abi::{PAGE_SIZE, Permissions};
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

#[allow(
    reviewable::explicit_shared_error::InvalidAddressError,
    reason = "an address outside the space is the one way each of these methods fails"
)]
/// An address space, shared by everyone who runs on it: every address is all zeros and allows nothing until it is written or given permissions.
///
/// Bytes borrowed from the space ignore the permissions, which bind only the code executed on it.
pub trait MemorySpace: Sized {
    /// A borrowed part of the space, which its guest may be changing while it is held.
    type Bytes<'space>: VolatileBytes
    where
        Self: 'space;

    /// The one holder of the space's pages, made with the space.
    type PageTable: PageTable;

    fn borrow(&self, addresses: Range<u64>) -> Result<Self::Bytes<'_>, InvalidAddressError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Sets the bytes at the addresses to zero.
    fn zero(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
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
pub trait PageTable: Sized {
    fn set_permissions(
        &mut self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), InvalidAddressError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Maps the pages of the other table's space that start at the given address into the addresses in this space.
    fn map_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Ends the mapping of the addresses, which become zeroed out wherever they were previously mapped from.
    fn unmap_from(&mut self, addresses: Range<u64>) -> Result<(), InvalidAddressError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Copies the bytes, permissions and mappings of the other table's space that start at the given address into the addresses of this space.
    fn copy_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidAddressError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapMemoryError {
    InvalidAddress(InvalidAddressError),
    OutOfMemory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyMemoryError {
    InvalidAddress(InvalidAddressError),
    OutOfMemory,
}

} // verus!

impl core::fmt::Display for InvalidAddressError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("the addresses do not all lie inside the space")
    }
}

impl core::error::Error for InvalidAddressError {}

impl core::fmt::Display for MapMemoryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidAddress(inner) => core::fmt::Display::fmt(inner, formatter),
            Self::OutOfMemory => formatter.write_str("the host has no memory left for the mapping"),
        }
    }
}

impl core::error::Error for MapMemoryError {}

impl core::fmt::Display for CopyMemoryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidAddress(inner) => core::fmt::Display::fmt(inner, formatter),
            Self::OutOfMemory => formatter.write_str("the host has no memory left for the copy"),
        }
    }
}

impl core::error::Error for CopyMemoryError {}
