//! Bytes at a pointer for a length, which another party may change at any moment, reached only by the C library's
//! `memcpy`.

#[cfg(kani)]
mod proofs;

use core::ffi::c_void;
use core::ptr::addr_of;
use core::sync::atomic::{AtomicPtr, Ordering::Relaxed};

use vstd::prelude::*;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};

// TODO: the copies call the C library's memcpy because Rust has no copy that tolerates a source another party changes. Check on https://github.com/rust-lang/rust/issues/158947 (volatile atomics), https://github.com/rust-lang/rfcs/pull/3301 (a per-byte atomic copy) and https://github.com/rust-lang/rfcs/pull/2728 (volatile atomics); replace the call with Rust's own copy when one is stable.

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

impl StandaloneVolatileBytes for PointerBytes {
    type Config = PointerAndLength;

    fn new(config: PointerAndLength) -> PointerBytes {
        PointerBytes { pointer: AtomicPtr::new(config.pointer), length: config.length }
    }
}

verus! {

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

/// Reads and writes from an offset in the view, as many bytes as the shorter of the buffer and the rest of the view.
impl VolatileBytes for PointerBytes {
    closed spec fn spec_len(&self) -> nat {
        self.length as nat
    }

    fn len(&self) -> u64 {
        self.length as u64
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        let Some((at, count)) = range(self.length, offset, into.len()) else {
            return usize::MIN;
        };
        #[expect(unsafe_code, reason = "the copy out of the bytes")]
        // SAFETY: `pointer` and `length` are this view's, which `PointerAndLength::new` promised; Verus proves
        // `at + count <= length` here (the copy's `requires`, from `range`), and the promise keeps any reference, so
        // `into`, off the view.
        unsafe {
            copy_out(&self.pointer, self.length, at, &mut into[..count]);
        }
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        let Some((at, count)) = range(self.length, offset, bytes.len()) else {
            return usize::MIN;
        };
        #[expect(unsafe_code, reason = "the copy into the bytes")]
        // SAFETY: `pointer` and `length` are this view's, which `PointerAndLength::new` promised; Verus proves
        // `at + count <= length` here (the copy's `requires`, from `range`), and the promise keeps any reference, so
        // `bytes`, off the view.
        unsafe {
            copy_in(&self.pointer, self.length, at, &bytes[..count]);
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

/// Returns where the offset lies in the `length` bytes and how many bytes from it a buffer of `buffer` bytes takes: the
/// shorter of the buffer and the bytes from the offset to the end, which is `copied`. None from an offset past the end,
/// which takes no bytes. Every step is checked, so nothing wraps.
fn range(length: usize, offset: u64, buffer: usize) -> (found: Option<(usize, usize)>)
    ensures
        match found {
            Some((at, count)) => {
                &&& at == offset
                &&& at + count <= length
                &&& count <= buffer
                &&& count == wcontainers_volatile_bytes::copied(
                    length as nat,
                    offset,
                    buffer as nat,
                )
            },
            None => offset > length && wcontainers_volatile_bytes::copied(
                length as nat,
                offset,
                buffer as nat,
            ) == 0,
        },
{
    let at = usize::try_from(offset).ok()?;
    let left = length.checked_sub(at)?;
    Some((at, buffer.min(left)))
}

} // verus!

// The one assumption of this crate, which nothing proves: what the C library's `memcpy` does when it copies between the
// view and a buffer. Verus can neither see into a call to C nor reason about memory another party changes during the
// copy, so it is given, for each direction, by `assume_specification` (Verus's form for the specification of a function
// it does not verify), marked `verus::trusted` so that strict mode (`--no-cheating`) takes it and nothing else here.
// Each copy reaches only the `count` bytes at `at` of the view (`at + count <= length`, which Verus checks at every
// call); the copy out writes only the buffer it is handed, keeping its length, and the copy in writes no buffer. Nothing
// is said about the bytes' values, which another party may be changing.
verus! {

#[verus::trusted]
#[expect(unsafe_code, reason = "Verus makes each specification an unsafe function that calls the function it specifies")]
assume_specification[ copy_out ](pointer: &AtomicPtr<u8>, length: usize, at: usize, into: &mut [u8])
    requires
        at + old(into)@.len() <= length,
    ensures
        final(into)@.len() == old(into)@.len(),
;

#[verus::trusted]
#[expect(unsafe_code, reason = "Verus makes each specification an unsafe function that calls the function it specifies")]
assume_specification[ copy_in ](pointer: &AtomicPtr<u8>, length: usize, at: usize, bytes: &[u8])
    requires
        at + bytes@.len() <= length,
;

/// The C library's `memcpy`, which the copies read back by a volatile load so that the compiler cannot know which
/// function they call and so cannot replace the call with its own copy.
#[verus::trusted]
#[verifier::external]
static MEMCPY: unsafe extern "C" fn (*mut c_void, *const c_void, usize) -> *mut c_void =
    libc::memcpy;

/// Copies into `into` as many bytes as it holds from the byte at `at` of the view, with the C library's `memcpy`;
/// makes no call for no bytes, so that C's rules for the pointers of an empty copy never arise.
///
/// # Safety
/// `pointer` and `length` are those of a view `PointerAndLength::new` promised, `at + into.len() <= length`, and
/// `into` is not over the view. `length` is used only to check the bound in a debug build.
#[verus::trusted]
#[verifier::external]
#[expect(unsafe_code, reason = "the copy out, whose caller promises the view, the bound and no overlap")]
unsafe fn copy_out(pointer: &AtomicPtr<u8>, length: usize, at: usize, into: &mut [u8]) {
    debug_assert!(at + into.len() <= length, "the copy out stays inside the view");
    if into.is_empty() {
        return;
    }
    let from = pointer.load(Relaxed).wrapping_add(at);
    #[expect(unsafe_code, reason = "the volatile load of the C library's memcpy")]
    // SAFETY: `MEMCPY` is a static that lives for the whole program and holds a valid function pointer.
    let memcpy = unsafe { addr_of!(MEMCPY).read_volatile() };
    #[expect(unsafe_code, reason = "the call to the C library's memcpy")]
    // SAFETY: `at + into.len() <= length`, so `from` lies in the view and is readable for `into.len()` bytes by the
    // promise, and does not wrap; `into` is writable for as many and not over the view. What `memcpy` does with a
    // source another party changes meanwhile is the assumption above.
    unsafe {
        memcpy(into.as_mut_ptr().cast(), from.cast_const().cast(), into.len());
    }
}

/// Copies all of `bytes` to the byte at `at` of the view, with the C library's `memcpy`; makes no call for no bytes.
///
/// # Safety
/// `pointer` and `length` are those of a view `PointerAndLength::new` promised, `at + bytes.len() <= length`, and
/// `bytes` is not over the view. `length` is used only to check the bound in a debug build.
#[verus::trusted]
#[verifier::external]
#[expect(unsafe_code, reason = "the copy in, whose caller promises the view, the bound and no overlap")]
unsafe fn copy_in(pointer: &AtomicPtr<u8>, length: usize, at: usize, bytes: &[u8]) {
    debug_assert!(at + bytes.len() <= length, "the copy in stays inside the view");
    if bytes.is_empty() {
        return;
    }
    let to = pointer.load(Relaxed).wrapping_add(at);
    #[expect(unsafe_code, reason = "the volatile load of the C library's memcpy")]
    // SAFETY: `MEMCPY` is a static that lives for the whole program and holds a valid function pointer.
    let memcpy = unsafe { addr_of!(MEMCPY).read_volatile() };
    #[expect(unsafe_code, reason = "the call to the C library's memcpy")]
    // SAFETY: `at + bytes.len() <= length`, so `to` lies in the view and is writable for `bytes.len()` bytes by the
    // promise, and does not wrap; `bytes` is readable for as many and not over the view.
    unsafe {
        memcpy(to.cast(), bytes.as_ptr().cast(), bytes.len());
    }
}

} // verus!
