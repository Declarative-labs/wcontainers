//! Bytes at a pointer for a length, which another party may change at any moment, reached only by the C library's
//! `memcpy`.

#[cfg(kani)]
mod proofs;

use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, Ordering::Relaxed};

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};

// TODO: the copies call the C library's memcpy because Rust has no copy that tolerates a source another party changes. Check on https://github.com/rust-lang/rust/issues/158947 (volatile atomics), https://github.com/rust-lang/rfcs/pull/3301 (a per-byte atomic copy) and https://github.com/rust-lang/rfcs/pull/2728 (volatile atomics); replace the call with Rust's own copy when one is stable.

/// The C library's `memcpy`, which `copy` reads back by a volatile load so that the compiler cannot know which function
/// it calls and so cannot replace the call with its own copy.
static MEMCPY: unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> *mut c_void = libc::memcpy;

/// A pointer and a length, made only by `PointerAndLength::new`, whose promise every copy relies on.
pub struct PointerAndLength {
    pointer: *mut u8,
    length: usize,
}

impl PointerAndLength {
    /// # Safety
    /// For as long as this value, the `PointerBytes` made from it and every part of that live: the `length` bytes from
    /// `pointer` are valid for reads and writes from any thread, reaching them never traps, and no one forms a reference
    /// or a slice over any of them.
    #[expect(unsafe_code, reason = "the promise each pointer and length makes, on which every copy relies")]
    pub unsafe fn new(pointer: *mut u8, length: usize) -> PointerAndLength {
        PointerAndLength { pointer, length }
    }
}

/// Bytes at a pointer for a length, which another party, such as a guest, a C caller or another process, may read and
/// write at any moment.
///
/// While the other party writes, each byte a read copies is some value that byte held during the call; the bytes of one
/// call need not come from one moment, and a byte may be read more than once. A write stores only the bytes of the view,
/// each its new value, possibly more than once. Nothing here orders this program's other memory, atomics or locks. It
/// does not keep the memory alive.
pub struct PointerBytes {
    /// An `AtomicPtr` only so that the type is `Send` and `Sync` without an `unsafe impl`, which the promise of
    /// `PointerAndLength::new` covers: the pointer never changes.
    pointer: AtomicPtr<u8>,
    length: usize,
}

impl PointerBytes {
    /// Returns the address of the byte at the offset and how many bytes from it a buffer of `buffer` bytes takes, if
    /// the offset lies inside the view or at its end.
    fn at(&self, offset: u64, buffer: usize) -> Option<(*mut u8, usize)> {
        let pointer = self.pointer.load(Relaxed);
        let (address, count) = range(pointer.addr(), self.length, offset, buffer)?;
        Some((pointer.with_addr(address), count))
    }
}

impl StandaloneVolatileBytes for PointerBytes {
    type Config = PointerAndLength;

    fn new(config: PointerAndLength) -> PointerBytes {
        PointerBytes { pointer: AtomicPtr::new(config.pointer), length: config.length }
    }
}

/// Reads and writes from an offset in the view, as many bytes as the shorter of the buffer and the rest of the view.
impl VolatileBytes for PointerBytes {
    fn len(&self) -> u64 {
        self.length as u64
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        let Some((from, count)) = self.at(offset, into.len()) else {
            return usize::MIN;
        };
        #[expect(unsafe_code, reason = "the copy out of the bytes")]
        // SAFETY: the `count` bytes from `from` lie inside the view and `count` is at most `into`'s length (both checked
        // by `range`); the promise of `PointerAndLength::new` keeps the view readable with no reference over it, so
        // `into`, a reference, is not over them.
        unsafe {
            copy(into.as_mut_ptr(), from, count);
        }
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        let Some((to, count)) = self.at(offset, bytes.len()) else {
            return usize::MIN;
        };
        #[expect(unsafe_code, reason = "the copy into the bytes")]
        // SAFETY: the `count` bytes from `to` lie inside the view and `count` is at most `bytes`'s length (both checked
        // by `range`); the promise of `PointerAndLength::new` keeps the view writable with no reference over it, so
        // `bytes`, a reference, is not over them.
        unsafe {
            copy(to, bytes.as_ptr(), count);
        }
        count
    }

    // The sink and the source each take all the bytes as one slice, and no slice over the bytes may be formed, so both
    // copy through a buffer as long as the view.

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut bytes = vec![0; self.length];
        let count = self.read(&mut bytes);
        sink.write(&bytes[..count])
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut bytes = vec![0; self.length];
        let count = source.read(&mut bytes)?;
        Ok(self.write(&bytes[..count]))
    }
}

/// Returns the address of the byte at `offset` in the `length` bytes from `address`, and the count of bytes from it a
/// buffer of `buffer` bytes takes: the shorter of the buffer and the bytes from the offset to the end. None from an
/// offset past the end, which takes no bytes. Every step is checked, so nothing wraps.
fn range(address: usize, length: usize, offset: u64, buffer: usize) -> Option<(usize, usize)> {
    let at = usize::try_from(offset).ok()?;
    let left = length.checked_sub(at)?;
    Some((address.checked_add(at)?, buffer.min(left)))
}

/// Copies `length` bytes from `from` to `to` with the C library's `memcpy`, called through a pointer the compiler cannot
/// see into; makes no call for no bytes, so that C's rules for the pointers of an empty copy never arise.
///
/// # Safety
/// `from` is readable and `to` writable for `length` bytes, and the two ranges do not overlap.
#[expect(unsafe_code, reason = "the one foreign call, to the C library's memcpy, that both directions use")]
unsafe fn copy(to: *mut u8, from: *const u8, length: usize) {
    if length == 0 {
        return;
    }
    // SAFETY: `MEMCPY` is a static that lives for the whole program and holds a valid function pointer, so the volatile
    // load reads a valid value; the call's ranges are the caller's promise. What `memcpy` does with a source another
    // party changes meanwhile is the C library's, and the stated assumption of README.md.
    unsafe {
        let memcpy = (&raw const MEMCPY).read_volatile();
        memcpy(to.cast(), from.cast(), length);
    }
}
