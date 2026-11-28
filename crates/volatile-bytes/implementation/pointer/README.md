# wcontainers-volatile-bytes-pointer

`PointerBytes` is bytes at a pointer for a length that another party (a guest, a C caller, another thread or process)
may change at any moment. It implements `VolatileBytes`. Every copy (`read_at`, `write_at`; `read` and `write` are the
interface's, from offset 0) takes its range from `range`, whose checked arithmetic cannot wrap, goes through the C
library's `memcpy`, and never forms a reference or a slice over the bytes. Its `Config` is `PointerAndLength`, made only
by the `unsafe fn PointerAndLength::new`, so the promise is made once and `new` stays safe. It does NOT keep the memory
alive, map it, or free it: the caller does.

## The unsafe items (4)

1. implementation.rs:134, `unsafe fn copy`: the one call to `memcpy`, through a pointer read by a volatile load so the
   compiler cannot swap in its own copy. Rests on: the caller gives a readable source and a writable destination that
   do not overlap.
2. implementation.rs:83, the copy out in `read_at`. Rests on: `range` checked that the count bytes from the offset lie
   inside the view and that the count is at most the buffer's length.
3. implementation.rs:97, the copy in in `write_at`. Rests on the same check.
4. implementation.rs:32, `unsafe fn PointerAndLength::new`: the promise. Rests on its caller: while the value and every
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
- Plants (a wrong form of each bound and copy, each caught by a named test): `crates/volatile-bytes/implementation/pointer/plants.sh`
  from the workspace root

## Open questions

- The constructor: this branch adds `StandaloneVolatileBytes` (`type Config; fn new(config: Self::Config) -> Self;`)
  to crates/volatile-bytes/interface.rs, following the `StandaloneClock` idiom.
- The promise of `PointerAndLength::new` still names "every part" of a view, from the `part` method the offset methods
  replaced; no part is made any more.
