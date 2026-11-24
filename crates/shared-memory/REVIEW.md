# The guest-memory primitive, for review

Marks: MEASURED (a run, its log in `checks/`), READ (file:line or URL), INFERRED.

## What it is

`SharedMemory` (interface.rs) is a safe trait: copy out, copy in, load, store, swap and compare-and-exchange of a `u32` or
`u64` word, futex wait and wake (Linux), and a sub-range. Every method checks its offsets at run time and returns an
error outside them; no signature holds a pointer or an address. `HostMapping` and, under Miri and Kani only, `HeapMapping`
implement it through one private `Region` (foreign_implementation/region/), which holds all the unsafe code.

The mapping is treated as memory OUTSIDE every Rust allocation: no reference and no slice over it is ever formed. Copies
are one assembly routine per architecture, used in both directions (x86-64 `rep movsb`; ARM64 `ldp`/`stp` of vector
registers, then 32, 16, 8, 4, 2, 1 bytes). Words are `read_volatile`/`write_volatile`, with an acquire fence after a load
and a release fence before a store. Swap and compare-and-exchange are the processor's own (`xchg`, `lock cmpxchg`;
`ldaxr`/`stlxr` pairs, which every ARM64 has). Anything else, and Miri and Kani, use a volatile fallback, where swap and
compare-and-exchange are not atomic.

## The sentence it rests on

"Volatile operations, however, may also be used to access memory that is _outside_ of any Rust allocation." (READ:
`core::ptr::read_volatile`, Rust 1.100.0, library/core/src/ptr/mod.rs:2078.) The assembly reads and writes the region as
a run of such volatile accesses would.

NOT SETTLED: whether a program may treat memory it mapped itself as outside every allocation. Two open discussions:
https://github.com/rust-lang/unsafe-code-guidelines/issues/607 (the language team lead's "never make that memory part of
the AM ... raw pointers ... volatile") and https://github.com/rust-lang/unsafe-code-guidelines/issues/543. Neither is
in the Reference.

## What the mapping must guarantee (`Region::new`'s `# Safety`)

- The bytes stay mapped, readable and writable, and reaching them never traps, while the value lives; they never shrink.
  For a memory file: the shrink seal is seen BEFORE the size is read (`ShrinkSealedFile`), the file is long enough, and
  not on the huge-page file system. Anonymous memory needs nothing more.
- The start is aligned to 8 (asserted; memmap2 returns the offset's own address, memmap2-0.9.12 src/unix.rs:99-100).
- No reference or slice over them is ever formed: `MmapRaw` gives only an address.
- NOT KEPT under strict overcommit (`vm.overcommit_memory=2`): writing a file's pages can end the process with SIGBUS
  (MEASURED earlier, checks/overcommit.log). A crash, not a stray write.

## What is the hardware's business, and what is not covered

- The other party's accesses (a guest, another process, the kernel) are its own, on the hardware's terms; they cannot
  cause undefined behavior in our code, because no Rust rule about races applies outside an allocation. That includes
  the single-byte writes the old 8-byte atomics design could not allow.
- NOT COVERED: an engine whose own Rust code makes slices over the same memory (wasmtime's linear memory). There the
  memory is a Rust allocation already; such an engine must use its own safe API, not this primitive.
- The fences give processor order only (INFERRED that LLVM keeps each volatile access on its side of the fence; the
  listing shows `ldr` then `dmb ishld`). In Rust's model nothing here synchronizes with anything.

## The TODO

One line in region/implementation.rs: remove the assembly once Rust has volatile atomic operations
(https://github.com/rust-lang/rust/issues/158947, unstable `load_volatile`/`store_volatile` only, no read-modify-write)
and a copy that tolerates races (https://github.com/rust-lang/rfcs/pull/3301, https://github.com/rust-lang/rfcs/pull/2728).

## The unsafe

14 marked items in a build for x86-64 or ARM64: `Region::new`'s promise; the region's copy out, copy in, load, store,
swap, compare-and-exchange and futex call; that architecture's 5 routines (copy, swap and compare-and-exchange of 4 and 8
bytes); the host mapping's call of `Region::new`. Not built there: the other architecture's 5, the fallback's 3, the
heap mapping's 1. The futex goes through `libc::syscall`, because rustix takes the word only as an `&AtomicU32`.

Each assembly block states its contract: registers clobbered (declared), flags (`preserves_flags` only on `rep movsb`,
`xchg` and the ARM64 swap, which change none), the x86-64 direction flag (clear on entry to every asm block, Reference
`asm.rules.x86-df`; `rep movsb` relies on it and leaves it clear), the stack (`nostack`), and none of `pure`, `nomem`,
`readonly`, which would be false.

How the copy never touches a byte outside: `rep movsb` moves exactly `rcx` bytes. The ARM64 loop moves 64 bytes only
while 64 remain, then the tail goes by the bits of what is left; no load overlaps another or passes the last byte.

## What the checks cover (see the report for the runs)

- Native suite on both machines: every offset within 17 bytes of each word, cache-line and page boundary and of the end,
  at 24 lengths up to 129, in both directions, from buffers at every alignment, checking every byte around; the guard
  test (a memory with no page mapped before or after it); reads while another thread, and another process, write single
  bytes, 4-byte and 8-byte words; atomicity of swap and compare-and-exchange under four threads; the seal tests.
- Wrong changes the suite must catch, on both machines: a copy one byte past the end or before the start (the guard test
  dies with SIGSEGV), a swap or compare-and-exchange that is not atomic, the range and alignment checks off by one, and
  the earlier seal, size and base mistakes.
- Miri: the safe code around the assembly, with the volatile fallback, single-threaded (the heap's bytes are a Rust
  allocation, so a race there is a race by definition). It cannot see inside assembly.
- Kani: the range arithmetic of every method, at 1 to 4 words, any offset, length and content, with the fallback.
- Only the native tests cover the assembly itself; the listing in the report shows each byte loaded once.

## Measurements (ns per copy, median of 11 alternating rounds, ratio to plain `copy_nonoverlapping`; MEASURED, checks/bench-*.log)

- x86-64 quiet (load 0.4): out 8 B 1.37x, 64 B 1.80x, 4 KiB 1.00x, 64 KiB 0.92x, 2 MiB 1.00x; in 1.29x, 1.50x, 0.97x,
  1.00x, 1.00x. Old atomics: 1.45x, 2.39x, 2.48x, 1.15x, 1.05x out. `pread`: 74x, 95x, 7.0x, 1.8x, 1.5x.
- ARM64 quiet (load 5): out 1.99x, 1.29x, 0.95x, 1.03x, 1.02x; in 0.78x, 0.81x, 0.97x, 1.01x, 1.02x. Old atomics:
  2.57x, 1.89x, 3.98x, 4.17x, 2.17x out. `pread`: 172x, 100x, 6.8x, 3.9x, 3.7x.
- With a thread storing 8-byte words over the same bytes: ARM64 ours 0.99-1.05x of plain from 64 B up. x86-64 copying
  out at 4 KiB and 64 KiB, both `rep movsb` (ours) and plain (glibc) are much slower than 8-byte loads (2146 and 24375 ns
  against 1177 and 4196 for the old atomics); copying in, ours is 1.00x and atomics 5.7-10.9x.
