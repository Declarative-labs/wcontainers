//! The base alone, over memory this test binary owns: its conformance.

use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use wcontainers_volatile_bytes::StandaloneVolatileBytes;
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};

/// The bytes each call of `owned` hands out, and the number of calls the tests make at most.
const CHUNK: usize = 1 << 16;
const CHUNKS: usize = 16;

/// Memory no code here forms a reference over; each call of `owned` takes the next chunk of it.
static mut MEMORY: [u8; CHUNK * CHUNKS] = [0; CHUNK * CHUNKS];
static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Returns the bytes of a chunk of `MEMORY` no other call returned, zero, the length long.
fn owned(length: usize) -> PointerBytes {
    let chunk = NEXT.fetch_add(1, Relaxed);
    assert!(chunk < CHUNKS && length <= CHUNK, "the tests take more memory than MEMORY holds");
    let pointer = (&raw mut MEMORY).cast::<u8>().wrapping_add(chunk * CHUNK);
    #[expect(unsafe_code, reason = "the promise of the test's own memory")]
    // SAFETY: the chunk lies inside `MEMORY`, a static that lives for the whole program, so it is valid for reads and
    // writes and never traps; no other call takes it, and no code here forms a reference over `MEMORY`.
    let config = unsafe { PointerAndLength::new(pointer, length) };
    PointerBytes::new(config)
}

wcontainers_volatile_bytes_conformance::tests!(owned(u16::MAX as usize));
