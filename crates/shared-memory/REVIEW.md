# The guest-memory primitive, for review

Marks: MEASURED (a run; its log in `checks/`), READ (file:line or URL), INFERRED.

## 1. The trait

```rust
/// Memory that another party, such as a guest or another process, may read and write at any moment.
///
/// While the other party writes, each byte a read copies is some value that byte held during the call; the bytes of
/// one call need not come from one moment, and a byte may be read more than once. A write stores only the bytes of its
/// range, each its new value, possibly more than once. Every method checks its offsets and, when they lie outside,
/// returns an error and touches nothing. Nothing here synchronizes with anything: no method orders this program's other
/// memory, atomics or locks; on the processor alone, a load comes before this thread's later accesses to this memory, a
/// store after its earlier ones, and a swap, or a compare-and-exchange that stores, both.
pub trait SharedMemory {
    /// Returns the number of bytes of the memory.
    fn len(&self) -> u64;
    /// Copies the bytes from the offset into the whole buffer.
    fn read(&self, at: u64, into: &mut [u8]) -> Result<(), OutsideError>;
    /// Copies the whole buffer to the bytes from the offset.
    fn write(&self, at: u64, from: &[u8]) -> Result<(), OutsideError>;
    fn load<W: Word>(&self, at: u64) -> Result<W, WordError>;
    fn store<W: Word>(&self, at: u64, value: W) -> Result<(), WordError>;
    /// Stores the value in the 8-byte word and returns the value the word held before.
    fn swap(&self, at: u64, value: u64) -> Result<u64, WordError>;
    /// Stores `new` in the 4-byte word if it holds `current`, and returns the value the word held before.
    fn compare_exchange(&self, at: u64, current: u32, new: u32) -> Result<u32, WordError>;
    /// Waits, until woken or the timeout passes, while the 4-byte word at the offset holds `expected`.
    #[cfg(target_os = "linux")]
    fn wait(&self, at: u64, expected: u32, timeout: Option<Duration>) -> Result<(), WaitError>;
    /// Wakes at most `limit` threads waiting on the 4-byte word at the offset, and returns the number of threads woken.
    #[cfg(target_os = "linux")]
    fn wake(&self, at: u64, limit: u32) -> Result<u32, WaitError>;
    /// Borrows the bytes at the offsets.
    fn borrow(&self, offsets: Range<u64>) -> Result<Part<'_, Self>, OutsideError> { /* range check, then a Part */ }
}

/// An unsigned integer the processor loads and stores in one access, at an offset that is a multiple of its size.
pub trait Word: Copy + Eq + Debug + Into<u64> + TryFrom<u64> + seal::Sealed {} // u32 and u64 only
```

`Part` implements `VolatileBytes` (sockets, `MemorySpace::Bytes`). Narrowed to the real callers: `swap` is 8 bytes only
(stub.rs:783, the one swap); `compare_exchange` is 4 bytes only (the guest kernel's futex words: futex.rs:731-760
`FUTEX_WAKE_OP`, :800-835 the PI lock, both read-then-write today); wait and wake are 4 bytes (stub.rs:223, :652,
:699). No caller compares and exchanges 8 bytes or swaps 4 (READ, grep over crates/).

## 2. Every unsafe item: 10 in a host build on Linux

All in foreign_implementation/region/implementation.rs unless named.

1. `pub(crate) unsafe fn new(base: *mut u8, length: usize) -> Region`: the mapping's promise (mapped, writable,
   never traps, aligned to 8, no reference formed but one word for one swap or compare-and-exchange).
2. `unsafe { copy(into.as_mut_ptr(), from, into.len()); }` in `read`: range checked just before.
3. `unsafe { copy(to, from.as_ptr(), from.len()); }` in `write`: the same.
4. `unsafe { word.read_volatile() }` in `load`, then `fence(Acquire)`.
5. `unsafe { word.write_volatile(value); }` in `store`, after `fence(Release)`.
6. `unsafe { AtomicU64::from_ptr(word) }.swap(value, AcqRel)`: see section 5.
7. `unsafe { AtomicU32::from_ptr(word) }.compare_exchange(current, new, AcqRel, Acquire)`: see section 5.
8. The one foreign call:
   ```rust
   pub(crate) unsafe fn copy(to: *mut u8, from: *const u8, length: usize) {
       if length == 0 { return; }
       unsafe {
           let memcpy = (&raw const MEMCPY).read_volatile();
           memcpy(to.cast(), from.cast(), length);
       }
   }
   ```
9. `libc::syscall(SYS_futex, word, ...)` in `futex`: rustix takes the word only as `&AtomicU32`.
10. host/implementation.rs: `unsafe { Region::new(base, map.len()) }` over memmap2's `MmapRaw`.

Built only under Miri and Kani: the heap mapping's `Region::new` and Kani's stand-in for `copy` (heap/proofs.rs, on
`copy_nonoverlapping`). The assembly files are gone: no `asm!` in the crate.

## 3. How the call is kept opaque

`static MEMCPY: unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> *mut c_void = libc::memcpy;` is read by a
volatile load before each call. LLVM's rule (READ LangRef 9e3b1fa, "Volatile Memory Accesses"): a volatile operation
"can read and/or modify state which is not accessible via a regular load", so the optimizer cannot know which function
it loaded, and an indirect call to an unknown function is never its `memcpy` builtin.

Four ways compared, copying a constant 8 bytes, opt-level 3, one codegen unit, without and with fat LTO, both machines
(MEASURED, checks/bench-*.log; sources in scratch `guest-memory-copy/bench-tree/probe`):
- `libc::memcpy` called directly: one load. x86 `mov (%rdi),%rax; ret`, ARM `ldr x0, [x0]; ret`. The forbidden plain copy.
- An `extern` under another name with `link_name = "memcpy"`, and a direct call in a `#![no_builtins]` crate: the same
  address as the direct one in `nm`, so the same single load.
- `black_box(libc::memcpy)`: keeps `call *0x10(%rsp)` / `blr x8`. But std calls `black_box` "best effort", not to be
  relied on for correctness.
- The volatile load (built): keeps the call, both levels, both machines:
  ```
  x86  lto fat: mov 0x37705(%rip),%rax  # <memcpy@GLIBC_2.14>; mov $0x8,%edx; call *%rax
  ARM  lto fat: ldr x8, [x8, #1568]; mov w2, #0x8; blr x8
  ```
The primitive itself in the bench binary with fat LTO, a constant 8-byte read, after inlining:
```
x86: cmpq $0x17,0x8(%rdi); jbe ...; mov (%rdi),%rax; add $0x10,%rax; mov 0x43c2b(%rip),%rcx  # MEMCPY; mov $0x8,%edx; ...; jmp *%rcx
ARM: ldr x8, [x0, #8]; cmp x8, #0x17; b.ls ...; ldr x9, [x0]; ...; ldr x3, [x10, #2528]; mov w2, #0x8; ...; br x3
```
The range check stays, and the copy is a jump through the loaded pointer.

## 4. Stated assumptions about the C library (trusted, not proved)

What C promises (READ, N1570 7.24.2.1p2): "The memcpy function copies n characters from the object pointed to by s2
into the object pointed to by s1. If copying takes place between objects that overlap, the behavior is undefined." And
7.24.1p2: for n zero the pointers "shall still have valid values" (so the primitive makes no call for zero bytes).
5.1.2.4p28's note: transformations "that introduce assignments to a potentially shared memory location that would not
be modified by the abstract machine are generally precluded", so no byte outside the destination is written.
Microsoft's page says the same about overlap and nothing more (READ learn.microsoft.com memcpy-wmemcpy, 2024-03-28).

What C does not promise: how many times each source byte is read, in what widths or order; anything when the source
changes during the call, which is a data race and undefined in C too (5.1.2.4p25). So we assume, for every C library
this runs on:
1. It copies exactly n bytes and writes no byte outside the destination.
2. Its path depends only on the pointers and n, never on the bytes, so a changing source yields mixed bytes and
   nothing worse.
3. Each destination byte ends as a value its source byte held at some read during the call.

What they do (READ): musl x86-64 `rep movsq` plus byte loops, each byte read once (src/string/x86_64/memcpy.s); musl
ARM64 (Arm's routine) loads overlapping ranges, e.g. `[src]` and `[srcend, -16]` for 16-32 bytes; glibc x86-64
memmove-vec-unaligned-erms.S "Use overlapping load and store", reloads the last vectors, `rep movsb` and non-temporal
stores for large sizes. So bytes ARE read twice; assumption 3 still holds, and the trait now says "a byte may be read
more than once" (it said "once" before). None reads or writes outside its ranges. macOS: libplatform's public tree has
only src/string/generic/memmove.c (word and byte loops); the optimized ARM64 and x86-64 routines macOS ships are not
published there, NOT READ. Windows: the CRT's memcpy source is not public, NOT READ.

## 5. The word operations

Load and store: `read_volatile`/`write_volatile` of an aligned `u32` or `u64`, with an acquire fence after a load and
a release fence before a store (processor order only, INFERRED).

Swap and compare-and-exchange, choice (a): `AtomicU64::from_ptr(word).swap` and `AtomicU32::from_ptr(word)
.compare_exchange`, AcqRel. Why not (b): no C library exports one (`nm -D libc.so.6`: 0 symbols naming atomic, both
machines); libatomic exists on both machines but is a separate library not every host has, and its functions are C11
atomics, so they carry the same assumption with a link dependency added; ARM64 binaries already hold compiler-rt's
`__aarch64_cas8_acq_rel`, which is what (a) compiles to (MEASURED). Why not (c): the owner's ruling.

What (a) assumes: the word stays mapped while borrowed (one expression), and every other access to it meanwhile is an
atomic access of the same size. The futex protocol requires that of a well-behaved guest. A hostile guest's 1-byte
writes to the word, and our own volatile `store` to the same word from another thread, are outside Rust's model; the
hardware still does one locked or exclusive access. It is the one place a reference (`&AtomicU64`, `&AtomicU32`) to the
mapping is formed.

Wait and wake: no safe library call takes a raw address (rustix needs `&AtomicU32`), so `libc::syscall(SYS_futex)`.

Where there is no C library (the floor, `no_std`): another implementation of the trait over the linear map would use
8-byte `read_volatile`/`write_volatile` loops, or one ARM64 copy routine since the floor runs only there.

## 6. What the tests cover

MEASURED, both machines, debug and release (checks/x86.log, arm.log):
- Copies in both directions at every start within 17 bytes of each word, cache-line, page and 2-page boundary and of
  the end, at 42 lengths up to 8193, from buffers at every alignment, checking every byte around.
- Copies of 2 MiB±1 and 9 MiB-3 at four alignments (glibc's large-copy paths).
- A memory with no page mapped before or after: every length to 257 and 15 longer ones to 8192, at both edges, plus
  words at both edges.
- While another thread, and another process, write the same bytes with 1-, 4- and 8-byte stores: reads see only values
  held (13 146 800 bytes against the process); words loaded, swapped and exchanged see only values held; copies in
  never touch the other party's 8-, 4- and 1-byte neighbors (203 947 256 bytes against the process).
- Swap and compare-and-exchange lose nothing across 8 threads and across 2 processes.
- Wrong changes caught on both: memcpy one byte long or one early (guard SIGSEGV), swap or compare-and-exchange as load
  then store, and the ten range, seal and base changes.
- Miri (x86 and ARM, both borrow models): all single-threaded tests. Miri runs its own memcpy for the foreign call and
  checks both ranges lie in their allocations (a region one byte long: "attempting to access 33 bytes ... only 32").
  Miri cannot see the real C memcpy, races with the other party (here the memory is a Rust allocation), or the futex.
- Kani: the range arithmetic of every method, 32 harnesses, 1 to 4 words, all successful on both; a one-byte-long
  range check fails it.
- Only native tests cover the real memcpy, the listings, the guard pages, threads and processes.

## 7. Measurements (ns per copy, median of 11 alternating rounds, ratio to plain `copy_nonoverlapping`; MEASURED)

x86-64, load 5.3-5.5 of 96, quiet:
- out 8 B 1.26x, 64 B 1.44x, 4 KiB 1.06x, 64 KiB 0.93x, 2 MiB 1.00x; in 1.30x, 1.24x, 1.02x, 1.00x, 1.01x.
- old atomics out 1.36x, 2.45x, 2.56x, 1.15x, 1.04x. `pread` 72x, 98x, 7.5x, 1.8x, 1.5x.

ARM64, load 40 of 96 (shared, noisy), quiet:
- out 1.75x, 1.55x, 0.94x, 1.01x, 1.02x; in 0.70x, 0.85x, 0.97x, 0.99x, 0.99x.
- old atomics out 2.50x, 2.01x, 3.84x, 4.15x, 2.11x. `pread` 171x, 99x, 6.8x, 3.8x, 4.0x.

With a thread storing 8-byte words over the same bytes: ours 0.98-1.05x of plain from 4 KiB up on both. On x86 copying
out at 4 and 64 KiB both ours and plain are far slower than 8-byte loads (2114 and 25014 ns against 1418 and 4454).

## 8. The TODO

One line above `MEMCPY`: replace the call with Rust's own copy when one tolerates a changing source; links
https://github.com/rust-lang/rust/issues/158947, https://github.com/rust-lang/rfcs/pull/3301,
https://github.com/rust-lang/rfcs/pull/2728.
