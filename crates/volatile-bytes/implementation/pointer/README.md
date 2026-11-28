# wcontainers-volatile-bytes-pointer

`PointerBytes` is bytes at a pointer for a length that another party (a guest, a C caller, another thread or process)
may change at any moment. It implements `VolatileBytes`, and Verus proves it against the interface's contract in strict
mode (`--no-cheating`): `range` gives exactly the interface's `copied`, `read_at` leaves the buffer beyond the count as it
was, `write_at` counts exactly `copied`, and every copy stays inside the view. Every copy goes through the C library's
`memcpy` and never forms a reference or a slice over the bytes. Its `Config` is `PointerAndLength`, made only by the
`unsafe fn PointerAndLength::new`, so the promise is made once and `new` stays safe. It does NOT keep the memory alive,
map it, or free it: the caller does.

## The unsafe items (11)

Counted on this tree: implementation.rs has 11 `#[expect(unsafe_code)]` marks, one per item below, and 22 lines holding
the word `unsafe` (21 outside comments; the other lines are the `unsafe` blocks and functions the marks cover and the
`unsafe extern "C" fn` type of `MEMCPY`, which is no operation). The test tests/pointer.rs has one more mark, line 21,
for the promise over its own memory.

1. implementation.rs:30, `unsafe fn PointerAndLength::new`: the promise. Rests on its caller: while the value and every
   view made from it live, the bytes are valid for reads and writes from any thread, never trap, and no one forms a
   reference or a slice over them.
2. implementation.rs:82, the copy out in `read_at`, handed the view itself. Rests on: the promise, and
   `at + count <= self.length`, which Verus proves at the call (the copy's `requires`, over the view's own length).
3. implementation.rs:96, the copy in in `write_at`. Rests on the same.
4. implementation.rs:188, `unsafe fn copy_out`. Rests on: `at + into.len() <= view.length` and `into` not over the
   view. It takes the view, so the bound is the view's own length and no caller can hand it another; a debug build
   also checks the bound.
5. implementation.rs:196, in `copy_out`, the volatile load of `memcpy`'s pointer, so the compiler cannot swap in its
   own copy. Rests on `MEMCPY` being a static holding a valid function pointer.
6. implementation.rs:201, in `copy_out`, the call to `memcpy`. Rests on what item 4 rests on.
7. to 9. implementation.rs:213, 221 and 225: the same three for `copy_in`, writing the view from `bytes`.
10. and 11. implementation.rs:159 and 168, the two `assume_specification`s of the assumption below. They exist only
   when Verus builds the crate: Verus turns each into an unsafe function that calls the function it specifies. The
   ordinary build drops them.

## What is assumed, not proved

One assumption, in the last `verus!` block of implementation.rs, in Verus's form for the specification of a function it does not verify
(`assume_specification`), marked `#[verus::trusted]` so that strict mode accepts it: each copy takes the view and reaches only the `count`
bytes at `at` of it, bounded by the view's own length (`at + count <= view.length`), the copy out writes only the buffer
it is handed (keeping its length), and the copy in writes no buffer. Since trusted code may name only trusted items of
its crate, `PointerBytes` itself is marked `#[verus::trusted]`; Verus checks nothing about a struct beyond its fields'
types, and what this trusts is that a view's `length` is the length its pointer was promised with, which only the
unverified `new` sets, from `PointerAndLength`. Nothing is said about the values of the bytes: `memcpy` may read or write a byte more than once, and each byte
it copies from a source that changes meanwhile is some value that byte held during the call. The pointer is kept in an
`AtomicPtr` only so the type is `Send` and `Sync` with no `unsafe impl`; the promise covers every thread.

## Running

- Tests: `cargo test -p wcontainers-volatile-bytes-pointer`
- Miri: `cargo +nightly miri test -p wcontainers-volatile-bytes-pointer`
- Verus, strict, as the linter's proof run: `cargo verus focus --package wcontainers-byte-sink --package
  wcontainers-byte-source --package wcontainers-volatile-bytes --package wcontainers-volatile-bytes-pointer --package
  wcontainers-volatile-bytes-exclusive-slice --package wcontainers-volatile-bytes-shared-mapping -- --no-cheating
  -Aduplicate_features` (every package that opts in must be
  named, or the workspace's `feature(register_tool)` meets Verus's own in a dependency)
- Kani (the range arithmetic, proofs.rs): `RUSTFLAGS="-Zcrate-attr=register_tool(reviewable)" cargo kani -p
  wcontainers-volatile-bytes-pointer`

## Open questions

- The constructor: this branch adds `StandaloneVolatileBytes` (`type Config; fn new(config: Self::Config) -> Self;`)
  to crates/volatile-bytes/interface.rs, following the `StandaloneClock` idiom.
- The promise of `PointerAndLength::new` still names "every part" of a view, from the `part` method the offset methods
  replaced; no part is made any more.
