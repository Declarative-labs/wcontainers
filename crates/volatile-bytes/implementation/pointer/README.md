# wcontainers-volatile-bytes-pointer

`PointerBytes` is bytes at a pointer for a length that another party (a guest, a C caller, another thread or process)
may change at any moment. It implements `VolatileBytes`, and Verus proves it against the interface's contract in strict
mode (`--no-cheating`): `range` gives exactly the interface's `copied`, `read_at` leaves the buffer beyond the count as it
was, `write_at` counts exactly `copied`, and every copy stays inside the view. Every copy goes through the C library's
`memcpy` and never forms a reference or a slice over the bytes. Its `Config` is `PointerAndLength`, made only by the
`unsafe fn PointerAndLength::new`, so the promise is made once and `new` stays safe. It does NOT keep the memory alive,
map it, or free it: the caller does.

## The unsafe items (7)

1. implementation.rs:28, `unsafe fn PointerAndLength::new`: the promise. Rests on its caller: while the value and every
   view made from it live, the bytes are valid for reads and writes from any thread, never trap, and no one forms a
   reference or a slice over them.
2. implementation.rs:75, the copy out in `read_at`. Rests on: the promise, and `at + count <= length`, which Verus
   proves at the call (the copy's `requires`).
3. implementation.rs:89, the copy in in `write_at`. Rests on the same.
4. implementation.rs:179, `unsafe fn copy_out`: a call to `memcpy`, through a pointer read by a volatile load so the
   compiler cannot swap in its own copy. Rests on: the view's pointer and length, `at + into.len() <= length`, and
   `into` not over the view.
5. implementation.rs:202, `unsafe fn copy_in`: the same, writing the view from `bytes`.
6. and 7. implementation.rs:149 and 158, the two `assume_specification`s of the assumption below. They exist only when
   Verus builds the crate: Verus turns each into an unsafe function that calls the function it specifies. The ordinary
   build drops them.

## What is assumed, not proved

One assumption, in `mod memcpy_assumption`, in Verus's form for the specification of a function it does not verify
(`assume_specification`), marked `#[verus::trusted]` so that strict mode accepts it: each copy reaches only the `count`
bytes at `at` of the view, the copy out writes only the buffer it is handed (keeping its length), and the copy in writes
no buffer. Nothing is said about the values of the bytes: `memcpy` may read or write a byte more than once, and each byte
it copies from a source that changes meanwhile is some value that byte held during the call. The pointer is kept in an
`AtomicPtr` only so the type is `Send` and `Sync` with no `unsafe impl`; the promise covers every thread.

## Running

- Tests: `cargo test -p wcontainers-volatile-bytes-pointer`
- Miri: `cargo +nightly miri test -p wcontainers-volatile-bytes-pointer`
- Verus, strict, as the linter's proof run: `cargo verus focus --package wcontainers-byte-sink --package
  wcontainers-byte-source --package wcontainers-volatile-bytes --package wcontainers-volatile-bytes-pointer --package
  wcontainers-volatile-bytes-shared-mapping -- --no-cheating -Aduplicate_features` (every package that opts in must be
  named, or the workspace's `feature(register_tool)` meets Verus's own in a dependency)
- Kani (the range arithmetic, proofs.rs): `RUSTFLAGS="-Zcrate-attr=register_tool(reviewable)" cargo kani -p
  wcontainers-volatile-bytes-pointer`
- Plants (a wrong form of each bound and copy, each caught by a named test): `crates/volatile-bytes/implementation/pointer/plants.sh`
  from the workspace root

## Open questions

- The constructor: this branch adds `StandaloneVolatileBytes` (`type Config; fn new(config: Self::Config) -> Self;`)
  to crates/volatile-bytes/interface.rs, following the `StandaloneClock` idiom.
- The promise of `PointerAndLength::new` still names "every part" of a view, from the `part` method the offset methods
  replaced; no part is made any more.
