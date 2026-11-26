//! The promises data pointers from the C side make, in the forms the serving glue needs; each call site states which
//! metadata fact gives the promise. Every structure crosses as bytes, copied once by the C library's memcpy, and is
//! read or written field by field at the offsets C's layout gives (the generated layout check asserts each against
//! the system's headers). Input bytes are only ever copied out, never viewed as writable: the owner's
//! `PointerAndLength::new` promises readable and writable memory, which C's `const` memory is not.
//!
//! Every `unsafe fn` here and in the generated glue has one shape: the mark on the function covers its declaration
//! only, and the body is a closure under `#[deny(unsafe_code)]`, so that each `unsafe` block inside needs its own.

use core::ffi::c_void;

use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};

/// The C library's `memcpy`, read back by a volatile load so that the compiler cannot know which function it calls
/// and replace the call with a copy that assumes no other party changes the source (as the owner's pointer crate).
static MEMCPY: unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> *mut c_void = libc::memcpy;

/// Copies `into.len()` bytes from `pointer` into `into`, and returns false for null.
///
/// # Safety
/// The metadata promises `into.len()` readable bytes at `pointer` for the call (a `const` pointer, `In`, a
/// `NativeArrayInfo` or `MemorySize` count), which another party may change meanwhile.
#[expect(unsafe_code, reason = "the promise of a read-only pointer and a length from the C side")]
pub unsafe fn read_bytes(pointer: *const u8, into: &mut [u8]) -> bool {
    #[deny(unsafe_code)]
    let mut body = move || {
        if pointer.is_null() {
            return false;
        }
        if into.is_empty() {
            return true;
        }
        #[expect(unsafe_code, reason = "the static holding the C library's memcpy")]
        // SAFETY: `MEMCPY` is a static that lives for the whole program and holds a valid function pointer.
        let memcpy = unsafe { (&raw const MEMCPY).read_volatile() };
        #[expect(unsafe_code, reason = "the copy out of C's read-only bytes")]
        // SAFETY: the caller's promise of `into.len()` readable bytes; no reference covers them, and `into` is ours,
        // so the ranges do not overlap.
        unsafe {
            memcpy(into.as_mut_ptr().cast(), pointer.cast(), into.len());
        }
        true
    };
    body()
}

/// Returns a view of `length` readable bytes, the same as `read_bytes`, copied into a new vector; none for null or
/// a length the allocator cannot give.
///
/// # Safety
/// As `read_bytes`, for `length` bytes.
#[expect(unsafe_code, reason = "the promise of a read-only pointer and a length from the C side")]
pub unsafe fn read_only_at(pointer: *const u8, length: usize) -> Option<Vec<u8>> {
    #[deny(unsafe_code)]
    let body = move || {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).ok()?;
        bytes.resize(length, 0_u8);
        #[expect(unsafe_code, reason = "bytes the C side passes in")]
        // SAFETY: the caller's promise.
        let read = unsafe { read_bytes(pointer, &mut bytes) };
        read.then_some(bytes)
    };
    body()
}

/// Returns writable bytes of `length` bytes at `pointer`, or none for null.
///
/// # Safety
/// The metadata promises `length` writable bytes at `pointer` (`Out`, a `MemorySize` count), which are also
/// readable, from any thread while the view lives, and the view does not outlive the call.
#[expect(unsafe_code, reason = "the promise of an output pointer and a length from the C side")]
pub unsafe fn writable_at(pointer: *mut u8, length: usize) -> Option<PointerBytes> {
    #[deny(unsafe_code)]
    let body = move || {
        if pointer.is_null() {
            return None;
        }
        #[expect(unsafe_code, reason = "an output pointer and a length from the C side")]
        // SAFETY: the caller's promise, which is the promise `PointerAndLength::new` asks for.
        let config = unsafe { PointerAndLength::new(pointer, length) };
        Some(PointerBytes::new(config))
    };
    body()
}

/// Writes `bytes` to `pointer` and returns false for null.
///
/// # Safety
/// As `writable_at`, for `bytes.len()` bytes.
#[expect(unsafe_code, reason = "an out-parameter the C side passes in")]
pub unsafe fn write_bytes(pointer: *mut u8, bytes: &[u8]) -> bool {
    #[deny(unsafe_code)]
    let body = move || {
        #[expect(unsafe_code, reason = "an out-parameter the C side passes in")]
        // SAFETY: the caller's promise.
        let Some(view) = (unsafe { writable_at(pointer, bytes.len()) }) else {
            return false;
        };
        view.write(bytes) == bytes.len()
    };
    body()
}
