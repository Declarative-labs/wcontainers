//! The base alone, over memory this test binary owns: its conformance, and the ranges of its parts.

use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
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

mod whole {
    wcontainers_volatile_bytes_conformance::tests!(super::owned(u16::MAX as usize));
}

mod part {
    wcontainers_volatile_bytes_conformance::tests!(
        super::owned(u16::MAX as usize).part(3..u16::MAX as u64 - 1).expect("inside")
    );
}

#[test]
fn refuses_parts_outside_the_view() {
    let view = owned(64).part(8..40).expect("inside");
    #[expect(clippy::reversed_empty_ranges, reason = "a start after the end is one of the cases refused")]
    for offsets in [0..33, 33..33, 9..8, 0..u64::MAX, u64::MAX..u64::MAX, 1 << 63..1 << 63] {
        assert!(view.part(offsets.clone()).is_none(), "{offsets:?}");
    }
    assert!(view.part(32..32).is_some_and(|part| part.is_empty()));
}

#[test]
fn reads_and_writes_only_the_bytes_of_a_part() {
    let whole = owned(64);
    let part = whole.part(8..40).and_then(|view| view.part(8..16)).expect("inside");
    assert_eq!(part.write(&[u8::MAX; 64]), 8);
    let mut all = [0; 64];
    assert_eq!(whole.read(&mut all), 64);
    for (index, byte) in all.iter().enumerate() {
        assert_eq!(*byte, if (16..24).contains(&index) { u8::MAX } else { 0 }, "{index}");
    }
    let empty = whole.part(64..64).expect("inside");
    assert_eq!((empty.write(&[1]), empty.read(&mut all)), (0, 0));
}

#[test]
fn two_views_of_the_same_bytes_see_each_others_writes() {
    let first = owned(64);
    let second = first.part(0..64).expect("inside");
    assert_eq!(first.write(&[7; 64]), 64);
    let mut read = [0; 64];
    assert_eq!(second.read(&mut read), 64);
    assert_eq!(read, [7; 64]);
}
