use core::ops::Range;

use thiserror::Error;
use wcontainers_abi::{PAGE_SIZE, Permissions};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;

#[allow(
    reviewable::explicit_shared_error::InvalidAddressError,
    reason = "an address outside the space is the one way each of these methods fails"
)]
/// An address space, held with full authority over it: every address is all zeros and allows nothing until it is written or given permissions.
///
/// Bytes borrowed from the space ignore the permissions, which bind only the code executed on it.
pub trait MemorySpace: Sized {
    /// A borrowed part of the space, which its guest may be changing while it is held.
    type Bytes<'space>: VolatileBytes
    where
        Self: 'space;

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

    fn set_permissions(
        &self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), InvalidAddressError>;

    /// Maps the pages of the other space that start at the given address into the addresses in this space.
    fn map_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError>;

    /// Ends the mapping of the addresses, which become zeroed out wherever they were previously mapped from.
    fn unmap_from(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError>;

    /// Copies the bytes, permissions and mappings of the other space that start at the given address into the addresses of this space.
    fn copy_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError>;
}

/// Bytes that something else may be changing while they are held.
pub trait VolatileBytes {
    fn len(&self) -> u64;

    fn is_empty(&self) -> bool {
        self.len() == u64::MIN
    }

    /// Copies bytes from the start into the buffer, as many as the shorter of the two holds, and returns the number of bytes read.
    fn read(&self, into: &mut [u8]) -> usize;

    /// Copies bytes from the buffer to the start, as many as the shorter of the two holds, and returns the number of bytes written.
    fn write(&self, bytes: &[u8]) -> usize;

    /// Reads bytes from the start to the sink, once, and returns the number of bytes read.
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut buffer = [u8::MIN; PAGE_SIZE as usize];
        let count = self.read(&mut buffer);
        sink.write(&buffer[..count])
    }

    /// Writes bytes from the source to the start, once, and returns the number of bytes written.
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut buffer = [u8::MIN; PAGE_SIZE as usize];
        let room = usize::try_from(self.len()).unwrap_or(usize::MAX).min(buffer.len());
        let count = source.read(&mut buffer[..room])?;
        Ok(self.write(&buffer[..count]))
    }
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
