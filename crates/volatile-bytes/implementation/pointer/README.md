# wcontainers-volatile-bytes-pointer

`PointerBytes` is bytes at a pointer for a length that another party (a guest, a C caller, another thread or process)
may change at any moment. It implements `VolatileBytes`. Every copy checks its range against the view, goes through
the C library's `memcpy`, and never forms a reference or a slice over the bytes. Its `Config` is `PointerAndLength`,
made only by the `unsafe fn PointerAndLength::new`, so the promise is made once and `new` stays safe. `part` makes a
view of a sub-range, checked (PROVISIONAL). It does NOT keep the memory alive, map it, or free it: the caller does.

## The unsafe items (4)

1. implementation.rs:153, `unsafe fn copy`: the one call to `memcpy`, through a pointer read by a volatile load so the
   compiler cannot swap in its own copy. Rests on: the caller gives a readable source and a writable destination that
   do not overlap.
2. implementation.rs:96, the copy out in `read`. Rests on: `bytes` checked that the count lies inside the view.
3. implementation.rs:110, the copy in in `write`. Rests on the same check.
4. implementation.rs:33, `unsafe fn PointerAndLength::new`: the promise. Rests on its caller: while the value and every
   view made from it live, the bytes are valid for reads and writes from any thread, never trap, and no one forms a
   reference or a slice over them.

## What is assumed, not proved

- `memcpy` writes only the destination's bytes, and each byte it copies from a source that changes meanwhile is some
  value that byte held during the call. It may read or write a byte more than once.
- The pointer is kept in an `AtomicPtr` only so the type is `Send` and `Sync` with no `unsafe impl`; the promise
  covers every thread.

## Running

- Tests: `cargo test -p wcontainers-volatile-bytes-pointer`
- Miri: `cargo +nightly miri test -p wcontainers-volatile-bytes-pointer`
- Kani (the range arithmetic, proofs.rs): `cargo kani -p wcontainers-volatile-bytes-pointer`

## Open questions

- The constructor: this branch adds `StandaloneVolatileBytes` (`type Config; fn new(config: Self::Config) -> Self;`)
  to crates/volatile-bytes/interface.rs:32-36, following the `StandaloneClock` idiom.
- `part`: should it stay each type's own method, or should `VolatileBytes` gain
  `fn part(&self, offsets: Range<u64>) -> Option<Self> where Self: Sized`?
