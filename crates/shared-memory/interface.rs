#![no_std]

extern crate alloc;
#[cfg(all(feature = "host", unix))]
extern crate std;

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
}
/// Holds the trait no other crate can name, so that only this crate's mappings implement [`Mapped`].
mod seal {
    pub trait Sealed {}
}

use alloc::sync::Arc;
use alloc::vec;
use core::ops::Range;
use core::ptr::NonNull;
#[cfg(target_os = "linux")]
use core::sync::atomic::AtomicU32;
use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_os = "linux")]
use core::time::Duration;

use thiserror::Error;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::VolatileBytes;

use crate::arithmetic::{WORD, plan};
#[cfg(any(miri, kani))]
pub use crate::foreign_implementation::heap::HeapMapping;
#[cfg(all(feature = "host", target_os = "linux"))]
pub use crate::foreign_implementation::host::FileError;
#[cfg(all(feature = "host", unix))]
pub use crate::foreign_implementation::host::{AnonymousError, HostMapping};

/// Memory that something outside this program may change at any moment.
///
/// # Safety
/// While the value lives: the bytes from `base` for `len` stay mapped readable and writable as ordinary memory, and
/// reaching them never traps; `base` is aligned to 8 and `len` is a multiple of 8, and neither changes; this program
/// reaches them at `base` only through [`Words`], and any other address of the same bytes (another mapping of their
/// file) and any system call on them counts as an outside agent; no unsafe code of this program trusts their contents;
/// whatever else writes them changes no other memory of this program.
#[expect(
    unsafe_code,
    reason = "the promise each mapping makes, on which `Words` reaches its bytes"
)]
#[expect(
    clippy::len_without_is_empty,
    reason = "`Words` asks a mapping its length once and never whether it is empty"
)]
pub unsafe trait Mapped: Send + Sync + seal::Sealed {
    fn base(&self) -> NonNull<u64>;

    /// Returns the number of bytes mapped.
    fn len(&self) -> usize;
}

/// The only way to reach a [`Mapped`]: aligned 8-byte atomic accesses, and no reference or pointer into it given out.
pub struct Words {
    #[expect(
        dead_code,
        reason = "held so that the mapping lives as long as the words; only `base` reaches it"
    )]
    mapping: Arc<dyn Mapped>,
    span: read_once::Span,
}

/// Holds what `Words::new` asks a mapping once, which the slice of its words trusts at every later use.
mod read_once {
    use core::ptr::NonNull;
    use core::sync::atomic::{AtomicPtr, Ordering};

    // Nothing here can change a `Span` after `new`: its fields are private to this module, which has no method that
    // stores, and `Words` has no method that takes `&mut self`. So the base and count stay the mapping's own, which
    // `Mapped` promises never change.

    /// A mapping's base and its length in words; the base is an `AtomicPtr` only because a raw pointer is neither `Send`
    /// nor `Sync`.
    pub(crate) struct Span {
        base: AtomicPtr<u64>,
        count: usize,
    }

    impl Span {
        pub(crate) fn new(base: NonNull<u64>, count: usize) -> Self {
            Self {
                base: AtomicPtr::new(base.as_ptr()),
                count,
            }
        }

        pub(crate) fn base(&self) -> *mut u64 {
            self.base.load(Ordering::Relaxed)
        }

        pub(crate) fn count(&self) -> usize {
            self.count
        }
    }
}

/// A part of a mapping, whose offsets every read and write checks again.
pub struct Part<'words> {
    words: &'words Words,
    start: u64,
    length: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("the offsets do not all lie inside the mapping")]
pub struct OutsideError;

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum WaitError {
    #[error(transparent)]
    Outside(#[from] OutsideError),
    #[error("the host refused: {0}")]
    Refused(rustix::io::Errno),
}

impl Words {
    pub fn new(mapping: Arc<dyn Mapped>) -> Self {
        let span = read_once::Span::new(mapping.base(), mapping.len() / WORD);
        Self { mapping, span }
    }

    /// Returns the number of 8-byte words.
    pub fn count(&self) -> u64 {
        self.span.count() as u64
    }

    /// Loads the word with `Acquire`.
    pub fn load(&self, index: u64) -> Result<u64, OutsideError> {
        Ok(self.word(index)?.load(Ordering::Acquire))
    }

    /// Stores the word with `Release`.
    pub fn store(&self, index: u64, value: u64) -> Result<(), OutsideError> {
        self.word(index)?.store(value, Ordering::Release);
        Ok(())
    }

    /// Stores the word with `AcqRel` and returns the value it replaced.
    pub fn swap(&self, index: u64, value: u64) -> Result<u64, OutsideError> {
        Ok(self.word(index)?.swap(value, Ordering::AcqRel))
    }

    /// Copies the bytes from the byte offset into the whole buffer, one `Relaxed` load of each word they touch and no
    /// load of any other word, or, if any of them lies outside, touches neither the words nor the buffer.
    pub fn read(&self, at: u64, into: &mut [u8]) -> Result<(), OutsideError> {
        let plan = plan(self.span.count(), at, into.len()).ok_or(OutsideError)?;
        let mut index = plan.index;
        let all = self.all();
        let (head, body) = into.split_at_mut(plan.head);
        if !head.is_empty() {
            let value = all
                .get(index)
                .ok_or(OutsideError)?
                .load(Ordering::Relaxed)
                .to_ne_bytes();
            head.copy_from_slice(&value[plan.low..plan.low + head.len()]);
            index += 1;
        }
        let (whole, tail) = body.as_chunks_mut::<WORD>();
        let middle = all.get(index..index + whole.len()).ok_or(OutsideError)?;
        for (chunk, word) in whole.iter_mut().zip(middle) {
            *chunk = word.load(Ordering::Relaxed).to_ne_bytes();
        }
        index += middle.len();
        if !tail.is_empty() {
            let value = all
                .get(index)
                .ok_or(OutsideError)?
                .load(Ordering::Relaxed)
                .to_ne_bytes();
            tail.copy_from_slice(&value[..tail.len()]);
        }
        Ok(())
    }

    /// Copies the whole buffer to the bytes from the byte offset, touching no other word, and a word only partly covered
    /// by a compare-and-exchange that keeps its other bytes; or, if any byte lies outside, touches no word.
    pub fn write(&self, at: u64, bytes: &[u8]) -> Result<(), OutsideError> {
        let plan = plan(self.span.count(), at, bytes.len()).ok_or(OutsideError)?;
        let mut index = plan.index;
        let all = self.all();
        let (head, body) = bytes.split_at(plan.head);
        if !head.is_empty() {
            merge(all.get(index).ok_or(OutsideError)?, plan.low, head);
            index += 1;
        }
        let (whole, tail) = body.as_chunks::<WORD>();
        let middle = all.get(index..index + whole.len()).ok_or(OutsideError)?;
        for (chunk, word) in whole.iter().zip(middle) {
            word.store(u64::from_ne_bytes(*chunk), Ordering::Relaxed);
        }
        index += middle.len();
        if !tail.is_empty() {
            merge(all.get(index).ok_or(OutsideError)?, 0, tail);
        }
        Ok(())
    }

    /// Waits, until woken or the timeout passes, while the word's low 4 bytes hold the value.
    #[cfg(target_os = "linux")]
    pub fn wait(
        &self,
        index: u64,
        expected: u32,
        timeout: Option<Duration>,
    ) -> Result<(), WaitError> {
        use rustix::io::Errno;
        use rustix::thread::futex;

        let timeout = timeout.and_then(|timeout| futex::Timespec::try_from(timeout).ok());
        match futex::wait(
            self.low_half(index)?,
            futex::Flags::empty(),
            expected,
            timeout.as_ref(),
        ) {
            Ok(()) | Err(Errno::AGAIN | Errno::INTR | Errno::TIMEDOUT) => Ok(()),
            Err(errno) => Err(WaitError::Refused(errno)),
        }
    }

    /// Wakes at most the count of threads waiting on the word, and returns the number woken.
    #[cfg(target_os = "linux")]
    pub fn wake(&self, index: u64, count: u32) -> Result<u32, WaitError> {
        use rustix::thread::futex;

        let woken = futex::wake(self.low_half(index)?, futex::Flags::empty(), count)
            .map_err(WaitError::Refused)?;
        Ok(u32::try_from(woken).unwrap_or(u32::MAX))
    }

    /// Returns the bytes at the offsets.
    pub fn part(&self, offsets: Range<u64>) -> Result<Part<'_>, OutsideError> {
        let (start, length) =
            arithmetic::part(self.span.count(), offsets.start, offsets.end).ok_or(OutsideError)?;
        Ok(Part {
            words: self,
            start,
            length,
        })
    }

    /// Returns the word at the index, if it lies inside the mapping.
    fn word(&self, index: u64) -> Result<&AtomicU64, OutsideError> {
        self.all()
            .get(usize::try_from(index).map_err(|_| OutsideError)?)
            .ok_or(OutsideError)
    }

    /// Returns every word of the mapping: the one place its words become reachable.
    fn all(&self) -> &[AtomicU64] {
        let base = self.span.base().cast_const().cast::<AtomicU64>();
        let count = self.span.count();
        #[expect(
            unsafe_code,
            reason = "the one place the words of a mapping become atomics"
        )]
        // SAFETY: `Mapped` promises that the `count` words from `base` stay mapped readable and writable as ordinary
        // memory, aligned to 8, while the mapping lives, and that this program reaches them at `base` only through
        // `Words`, everything else being an outside agent; `self` holds the mapping for as long as the slice lives. The
        // base and count were read from the mapping once, in `new`, and `Span` cannot change them after, so they are
        // still the mapping's. An `AtomicU64` has the size and alignment of a `u64`, and every access through this
        // slice is an 8-byte atomic.
        let all = unsafe { core::slice::from_raw_parts(base, count) };
        all
    }

    /// Returns the low 4 bytes of the word at the index, for the kernel's futex calls only.
    #[cfg(target_os = "linux")]
    fn low_half(&self, index: u64) -> Result<&AtomicU32, OutsideError> {
        // The low half is a word's first 4 bytes on a little-endian host and its last 4 on a big-endian one.
        let half = core::ptr::from_ref(self.word(index)?)
            .cast::<u32>()
            .cast_mut();
        let half = half.wrapping_add(usize::from(cfg!(target_endian = "big")));
        #[expect(
            unsafe_code,
            reason = "the address of a word's low half, which only the kernel reads"
        )]
        // SAFETY: the 4 bytes lie inside a word of the slice `all` gives, aligned to 4, for as long as `self` lives. No
        // code of this program loads or stores through this reference: the futex calls hand its address to the kernel.
        let low = unsafe { AtomicU32::from_ptr(half) };
        Ok(low)
    }
}

/// Writes the bytes into the word from its byte `low`, keeping its other bytes as they are at the exchange.
fn merge(word: &AtomicU64, low: usize, bytes: &[u8]) {
    let mut current = word.load(Ordering::Relaxed);
    loop {
        let mut value = current.to_ne_bytes();
        value[low..low + bytes.len()].copy_from_slice(bytes);
        match word.compare_exchange_weak(
            current,
            u64::from_ne_bytes(value),
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return,
            Err(seen) => current = seen,
        }
    }
}

impl Part<'_> {
    /// Returns the number of bytes a buffer of the length shares with the part.
    fn shared(&self, buffer: usize) -> usize {
        usize::try_from(self.length).map_or(buffer, |length| length.min(buffer))
    }
}

/// Reads and writes from the part's start, as many bytes as the shorter of the part and the buffer holds.
impl VolatileBytes for Part<'_> {
    fn len(&self) -> u64 {
        self.length
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = self.shared(into.len());
        match self.words.read(self.start, &mut into[..count]) {
            Ok(()) => count,
            Err(OutsideError) => 0,
        }
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let count = self.shared(bytes.len());
        match self.words.write(self.start, &bytes[..count]) {
            Ok(()) => count,
            Err(OutsideError) => 0,
        }
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut bytes = vec![0; self.shared(usize::MAX)];
        let count = self.read(&mut bytes);
        sink.write(&bytes[..count])
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut bytes = vec![0; self.shared(usize::MAX)];
        let count = source.read(&mut bytes)?;
        Ok(self.write(&bytes[..count]))
    }
}
