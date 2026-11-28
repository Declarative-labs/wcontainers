# wcontainers-volatile-bytes-shared-mapping

`SharedMapping` is a view of a mapping that another party (a guest, another thread, another process) may change at any
moment. It owns the mapping, keeps it mapped while any view lives (an `Arc`), and reaches its bytes only through
`PointerBytes` (crates/volatile-bytes/implementation/pointer), to which its `VolatileBytes` forwards. Its `Config` is
`Mapped`: fresh anonymous memory (`Mapped::anonymous`) or a memory file sealed against shrinking (`Mapped::file`,
Linux), so the steps that can fail happen when the `Mapped` is made and `new` cannot fail. `read_at` and `write_at`
forward to the base, which checks the range.

## The unsafe item (1)

1. implementation.rs:93, in `new`, the one call to `PointerAndLength::new` with the map's address and length. Rests
   on: the `Arc` keeps the bytes mapped while any view lives; a `Mapped` is made only by `anonymous` or by `file`,
   both readable and writable, with a private field so no reference over it is formed; `file`'s checks run in this
   order: length and offset, the shrink seal, not huge pages, then the size.

## What is assumed, not proved

- The kernel: anonymous memory never traps; a shrink seal cannot be undone, so a file seen sealed and long enough
  stays long enough. Not kept: under strict overcommit a file's page can fail to be charged, which ends the process
  with SIGBUS.
- memmap2: `MmapRaw` keeps the range mapped until it drops, hands out only an address, and returns the address of the
  offset asked.
- No code in this process breaks I/O safety between `file`'s checks and the mapping.

## Running

- Tests: `cargo test -p wcontainers-volatile-bytes-shared-mapping`
- Miri (the anonymous tests): `cargo +nightly miri test -p wcontainers-volatile-bytes-shared-mapping`
