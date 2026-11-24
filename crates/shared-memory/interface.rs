#![no_std]

extern crate alloc;
#[cfg(any(target_os = "linux", all(feature = "host", unix)))]
extern crate std;

#[cfg(any(all(feature = "host", unix), miri, kani))]
mod arithmetic;
mod foreign_implementation {
    #[cfg(any(miri, kani))]
    #[path = "heap/implementation.rs"]
    pub(crate) mod heap;
    #[cfg(all(feature = "host", unix))]
    #[path = "host/implementation.rs"]
    pub(crate) mod host;
    #[cfg(kani)]
    #[path = "heap/proofs.rs"]
    mod proofs;
    // Only the host's and the heap's mappings use it; it calls the C library, which a build without them may not have.
    #[cfg(any(all(feature = "host", unix), miri, kani))]
    #[path = "region/implementation.rs"]
    pub(crate) mod region;
}
/// Holds the trait no other crate can name, so that only `u32` and `u64` are words.
mod seal {
    pub trait Sealed {}

    impl Sealed for u32 {}

    impl Sealed for u64 {}
}

use alloc::vec;
use core::fmt::Debug;
use core::ops::Range;
#[cfg(target_os = "linux")]
use core::time::Duration;

use thiserror::Error;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::VolatileBytes;

#[cfg(any(miri, kani))]
pub use crate::foreign_implementation::heap::HeapMapping;
#[cfg(all(feature = "host", target_os = "linux"))]
pub use crate::foreign_implementation::host::FileError;
#[cfg(all(feature = "host", unix))]
pub use crate::foreign_implementation::host::{AnonymousError, HostMapping};

/// Memory that another party, such as a guest or another process, may read and write at any moment.
///
/// While the other party writes, each byte a read copies is some value that byte held during the call; the bytes of
/// one call need not come from one moment, and a byte may be read more than once. A write stores only the bytes of its
/// range, each its new value, possibly more than once. Every method checks its offsets and, when they lie outside,
/// returns an error and touches nothing. Nothing here synchronizes with anything: no method orders this program's other
/// memory, atomics or locks; on the processor alone, a load comes before this thread's later accesses to this memory, a
/// store after its earlier ones, and a swap, or a compare-and-exchange that stores, both.
#[expect(
    clippy::len_without_is_empty,
    reason = "no caller asks whether the memory is empty"
)]
pub trait SharedMemory {
    /// Returns the number of bytes of the memory.
    fn len(&self) -> u64;

    /// Copies the bytes from the offset into the whole buffer.
    fn read(&self, at: u64, into: &mut [u8]) -> Result<(), OutsideError>;

    /// Copies the whole buffer to the bytes from the offset.
    fn write(&self, at: u64, from: &[u8]) -> Result<(), OutsideError>;

    fn load<W: Word>(&self, at: u64) -> Result<W, WordError>;

    fn store<W: Word>(&self, at: u64, value: W) -> Result<(), WordError>;

    /// Stores the value in the 8-byte word and returns the value the word held before; while it runs, any other access
    /// to the word but a swap, including this program's own `read`, `write`, `load` or `store` from another thread, is
    /// a data race by Rust's rules (REVIEW.md §5).
    fn swap(&self, at: u64, value: u64) -> Result<u64, WordError>;

    /// Stores `new` in the 4-byte word if it holds `current`, and returns the value the word held before; while it runs,
    /// any other access to the word but a compare-and-exchange is a data race by Rust's rules, as for `swap`.
    fn compare_exchange(&self, at: u64, current: u32, new: u32) -> Result<u32, WordError>;

    /// Waits, until woken or the timeout passes, while the 4-byte word at the offset holds `expected`.
    #[cfg(target_os = "linux")]
    fn wait(&self, at: u64, expected: u32, timeout: Option<Duration>) -> Result<(), WaitError>;

    /// Wakes at most `limit` threads waiting on the 4-byte word at the offset, and returns the number of threads woken.
    #[cfg(target_os = "linux")]
    fn wake(&self, at: u64, limit: u32) -> Result<u32, WaitError>;

    /// Borrows the bytes at the offsets.
    fn borrow(&self, offsets: Range<u64>) -> Result<Part<'_, Self>, OutsideError> {
        if offsets.start > offsets.end || offsets.end > self.len() {
            return Err(OutsideError);
        }
        Ok(Part {
            memory: self,
            start: offsets.start,
            length: offsets.end - offsets.start,
        })
    }
}

/// An unsigned integer the processor loads and stores in one access, at an offset that is a multiple of its size.
pub trait Word: Copy + Eq + Debug + Into<u64> + TryFrom<u64> + seal::Sealed {}

impl Word for u32 {}

impl Word for u64 {}

/// Some bytes of a shared memory, whose offsets every read and write checks again.
pub struct Part<'memory, Memory: SharedMemory + ?Sized> {
    memory: &'memory Memory,
    start: u64,
    length: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("the bytes do not all lie inside the memory")]
pub struct OutsideError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum WordError {
    #[error(transparent)]
    Outside(#[from] OutsideError),
    #[error("the word's offset is not a multiple of its size")]
    Misaligned,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum WaitError {
    #[error(transparent)]
    Word(#[from] WordError),
    #[error("the host refused: {0}")]
    Refused(rustix::io::Errno),
}

impl<Memory: SharedMemory + ?Sized> Part<'_, Memory> {
    /// Returns the number of bytes a buffer of the length shares with the part.
    fn shared(&self, buffer: usize) -> usize {
        usize::try_from(self.length).map_or(buffer, |length| length.min(buffer))
    }
}

/// Reads and writes from the part's start, as many bytes as the shorter of the part and the buffer holds.
impl<Memory: SharedMemory + ?Sized> VolatileBytes for Part<'_, Memory> {
    fn len(&self) -> u64 {
        self.length
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = self.shared(into.len());
        match self.memory.read(self.start, &mut into[..count]) {
            Ok(()) => count,
            Err(OutsideError) => 0,
        }
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let count = self.shared(bytes.len());
        match self.memory.write(self.start, &bytes[..count]) {
            Ok(()) => count,
            Err(OutsideError) => 0,
        }
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut bytes = vec![0; self.shared(usize::MAX)];
        let count = VolatileBytes::read(self, &mut bytes);
        sink.write(&bytes[..count])
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut bytes = vec![0; self.shared(usize::MAX)];
        let count = source.read(&mut bytes)?;
        Ok(VolatileBytes::write(self, &bytes[..count]))
    }
}
