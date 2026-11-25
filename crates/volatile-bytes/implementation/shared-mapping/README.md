# wcontainers-volatile-bytes-shared-mapping

`SharedMapping` is one type: a view of a mapping that another party (a guest, another thread, another process) may
change at any moment. It implements `VolatileBytes` directly. Every copy checks its range against the mapping, goes
through the C library's `memcpy`, and never forms a reference or a slice over the mapped bytes. The mapping is fresh
anonymous memory (`Mapped::anonymous`) or a memory file sealed against shrinking (`Mapped::file`, Linux).

## The unsafe items (5)

1. implementation.rs:205, `unsafe fn copy`: the one call to `memcpy`, through a pointer read by a volatile load so the
   compiler cannot swap in its own copy. Rests on: the caller gives a readable source and a writable destination that
   do not overlap.
2. implementation.rs:148, the copy out in `read`. Rests on: `bytes` checked that the count of bytes lies inside the mapping.
3. implementation.rs:162, the copy in in `write`. Rests on the same check.
4. implementation.rs:98, `unsafe fn whole`: the mapping's promise. Rests on: the map is readable and writable, never
   traps while it lives, and no reference over it is ever formed.
5. implementation.rs:130, the one call to `whole`, in `new`. Rests on: a `Mapped` is made only by `anonymous` or by
   `file`, whose checks run in this order: length and offset, the shrink seal, not huge pages, then the size.

## What is assumed, not proved

- `memcpy` writes only the destination's bytes, and each byte it copies from a source that changes meanwhile is some
  value that byte held during the call. It may read or write a byte more than once.
- The kernel: anonymous memory never traps; a shrink seal cannot be undone, so a file seen sealed and long enough
  stays long enough. Not kept: under strict overcommit a file's page can fail to be charged, which ends the process
  with SIGBUS.
- memmap2: `MmapRaw` keeps the range mapped until it drops, hands out only an address, and returns the address of the
  offset asked.

## Running

- Tests: `cargo test -p wcontainers-volatile-bytes-shared-mapping`
- Miri (the anonymous tests; Miri uses its own `memcpy`): `cargo +nightly miri test -p wcontainers-volatile-bytes-shared-mapping`
- Kani (the range arithmetic, proofs.rs): `cargo kani -p wcontainers-volatile-bytes-shared-mapping`

## Proposal: the constructor

`VolatileBytes` has no way to make a value. Following the `StandaloneClock` idiom, this branch adds to
crates/volatile-bytes/interface.rs:32-36:

```rust
pub trait StandaloneVolatileBytes: VolatileBytes {
    type Config;

    fn new(config: Self::Config) -> Self;
}
```

Here `Config` is `Mapped`, so the steps that can fail happen when the `Mapped` is made, and `new` cannot fail.

A view of a sub-range: the executor's `MemorySpace::borrow` returns `Bytes<'_>`, and no interface says how to make
one from a range. This type has its own `part(&self, offsets: Range<u64>) -> Option<SharedMapping>`, marked
PROVISIONAL, which shares the mapping and checks the range.

Question: should (a) `part` stay this type's own method, which the executor's memory space calls on the concrete type,
or (b) `VolatileBytes` gain `fn part(&self, offsets: Range<u64>) -> Option<Self> where Self: Sized`?
