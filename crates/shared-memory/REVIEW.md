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
    /// Stores the value in the 8-byte word and returns the value the word held before; while it runs, any other access
    /// to the word but a swap, including this program's own `read`, `write`, `load` or `store` from another thread, is
    /// a data race by Rust's rules (REVIEW.md §5).
    fn swap(&self, at: u64, value: u64) -> Result<u64, WordError>;
    /// Stores `new` in the 4-byte word if it holds `current`, and returns the value the word held before; while it runs,
    /// any other access to the word but a compare-and-exchange is a data race by Rust's rules, as for `swap`.
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
   never traps, aligned to 8, no reference formed but one word for one swap or compare-and-exchange). One exception
   the host does not keep: with strict overcommit (`vm.overcommit_memory=2`) a memory file's page can fail to be
   charged when first touched, and the access ends the process with SIGBUS; a crash, not a write anywhere else
   (MEASURED, checks/overcommit.log, in a 512 MiB guest: the file mapping died with signal 7 after 45 824 of 91 648
   pages; anonymous memory was refused when mapped instead; with `overcommit_memory=0` both finished).
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
10. host/implementation.rs: `unsafe { Region::new(base, map.len()) }` over memmap2's `MmapRaw`. For a memory file it
    rests on three things done in order on one borrowed descriptor: the seals are read and hold `F_SEAL_SHRINK`, then
    the size is read, then the file is mapped. Two limits:
    - The type does not enforce the order. `ShrinkSealedFile::new` reads the seals and `map` reads the size, but code
      in that module could read the size in `new` first and keep it. What catches that order is the test
      `a_shrink_then_seal_racing_the_checks_never_maps_past_the_end` (the wrong change `size-read-before-the-seals`
      fails it, checks/x86.log and arm.log).
    - It holds only while no code in the process breaks I/O safety. An `unsafe` `dup2` over the borrowed descriptor,
      between the checks and the mapping, maps a file never seen sealed; shrinking it then ends the process with
      SIGBUS (MEASURED on x86-64 by the check of the older branch `unsafe-primitives`, scratch
      executor-unsafe-primitives/report4-check.md: mapped after 4 100 to 8 232 tries, SIGBUS 3 of 3). A safe `dup2`
      over a borrowed descriptor does not compile (E0502, same check).

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

What (a) assumes, and where it conflicts with the rest:
- `from_ptr` asks for a pointer "[valid] for both reads and writes for the whole lifetime" and forbids mixing
  "conflicting atomic and non-atomic accesses, or atomic accesses of different sizes, without synchronization" (READ,
  1.99.0 library/core/src/sync/atomic.rs:2655-2658). "Valid" means inside one Rust allocation (READ, ptr/mod.rs:21-24).
  Every other item here relies on the opposite: the region lies OUTSIDE every allocation, which only volatile accesses
  may reach (READ, ptr/mod.rs:2024). So items 6 and 7 stand on a premise the copies deny. Neither is proved; both are
  the stated assumption that the mapping is ordinary memory to the hardware.
- Every other access to the word while the swap or compare-and-exchange runs is an atomic access of the same size.
  The futex protocol requires that of a well-behaved guest. Three things break it, each a data race by Rust's rules
  (READ, atomic.rs:44, "data races are undefined behavior"): a hostile guest's 1-, 2- or 4-byte stores into the word;
  our own `read`, `write`, `load` or `store` of a range covering the word, from another thread; and a swap and a
  compare-and-exchange on overlapping words. The trait's doc of `swap` and `compare_exchange` now says so. No caller
  does the second or third today (INFERRED: no crate calls the primitive yet, READ, grep over crates/).
- It is the one place a reference (`&AtomicU64`, `&AtomicU32`) to the mapping is formed: one word, for one expression.

What the hardware does there: one locked or exclusive access, INFERRED from the listings (MEASURED, checks/x86.log and
arm.log, the listings of `swap` and `compare_exchange`): x86 `xchgq` and `lock cmpxchgl` inline; ARM64 calls
compiler-rt's `__aarch64_swp8_acq_rel` and `__aarch64_cas4_acq_rel`, which use `swpal` and `casal` when the processor
has them and an exclusive load and store loop when not (INFERRED, not read in this tree). MEASURED, both machines: while another thread stores single bytes into the last byte of the very
words they change, 4 threads counting with compare-and-exchange lose no count and every token swapped through comes out
once (`counts_and_swaps_without_losing_a_value_while_another_thread_writes_bytes_of_the_same_words`, checks/x86.log
and arm.log). That is evidence about these two processors, not a guarantee of Rust's.

Wait and wake: no safe library call takes a raw address (rustix needs `&AtomicU32`), so `libc::syscall(SYS_futex)`.

Where there is no C library (the floor, `no_std`): another implementation of the trait over the linear map would use
8-byte `read_volatile`/`write_volatile` loops, or one ARM64 copy routine since the floor runs only there.

## 6. What the tests cover

Every log names the code it ran on: `checks/tree.sh` hashes every file of the crate but the logs and this page, and
native.sh, miri.sh and kani.sh print it first. `checks/review.sh` (run on the Mac before each commit) fails when a log
names other code, when this page's unsafe count differs from the code's, when a cited `checks/` file is missing, or
when a comment cites this page for a measurement. Final run: code e8a20ce18811b34d, both machines, in one job each; the host suite has 51 tests
(MEASURED, checks/x86.log, arm.log; review.sh: "10 unsafe items, 0 failed").

MEASURED, both machines, debug and release:
- Copies in both directions at every start within 17 bytes of each word, cache-line, page and 2-page boundary and of
  the end, at 42 lengths up to 8193, from buffers at every alignment, checking every byte around.
- Copies of 2 MiB±1 and 9 MiB-3 at four alignments (glibc's large-copy paths).
- A memory with no page mapped before or after: every length to 257 and 15 longer ones to 8192, at both edges, plus
  words at both edges.
- While another thread, and another process, write the same bytes with 1-, 4- and 8-byte stores: reads see only values
  held (13 146 800 bytes against the process); words loaded, swapped and exchanged see only values held; copies in
  never touch the other party's 8-, 4- and 1-byte neighbors (203 947 256 bytes against the process).
- Swap and compare-and-exchange lose nothing across 8 threads and across 2 processes, and lose nothing across 4
  threads while a fifth stores single bytes into the same two words (section 5).
- The process test's other process ends when the test thread that started it ends (`PR_SET_PDEATHSIG`, SIGKILL; it
  also checks its parent is still the one named). Before, a failing or crashing process test left its writers spinning
  and holding the output pipe, so the run hung. A test kills the process that started a writer and checks the writer
  ends within 30 s. A planted one-byte-long copy fails the process test on both machines; 3 s later, without the
  signal, 4 (x86) and 2 (ARM) other processes were still running; with it, 0.
- Wrong changes caught on both, each in checks/x86.log and arm.log: memcpy one byte long or one early (guard
  SIGSEGV); swap, or compare-and-exchange, as a load then a store (both counting tests fail); the range, seal, length,
  offset and base changes below; and the parent-death signal unset ("the writer ... outlived the process that started it").
- The seal check, `fcntl_get_seals(file).is_ok_and(...)`: a file whose seals cannot be read must be refused. The test
  `refuses_a_file_that_cannot_be_sealed` used an ordinary file in the temporary folder; with `TMPDIR` on tmpfs that
  file's seals ARE readable, so the wrong change `.map_or(true, ...)` passed every test there and then mapped an ext4
  file that shrank and raised SIGBUS (MEASURED, both machines, at 80f6725968215b3f; scratch guest-memory-copy/seal-and-lengths/run1-x86.log and
  run1-arm.log). The test now uses a pipe, whose
  seals no host reads (the kernel keeps seals only for memory files), asserts that they cannot be read, and so fails
  rather than pass untried. The wrong change fails it with `TMPDIR` on tmpfs and on ext4, both machines.
- Lengths and offsets: the file mapping refuses a length of zero, an offset that is not a multiple of 8, and bytes
  past the file's end, and maps every other length, whole words or not (the anonymous one refuses only zero). Each
  check fails a test when removed, weakened or made stricter, both machines: anonymous and file length zero accepted,
  anonymous and file mapping one byte more, file offset unchecked or checked to 4 or to 16, file lengths limited to
  whole words, the end allowed one byte past the file or refused at it, the end wrapping, the file mapped from one word
  on, the region one byte long or short. Two checks sit behind others and no input reaches them, so their removal
  passes every test: the fallback that reads a negative size as 0 (`fstat` gives none), and the alignment assert in
  `new` (the offset check already refuses every offset it would catch).
- The 8 compile refusals each fail to compile.
- Miri, both machines, both borrow models, on the final code: host 13, heap 12, attacks 5 pass (the 7 tests with
  threads in each suite are ignored there). Miri runs its own memcpy for the foreign call and checks both ranges lie in their allocations;
  the two planted errors are reported ("attempting to access 33 bytes ... only 32", and a dangling pointer for a
  zero-length region). Miri cannot see the real C memcpy, races with the other party (there the memory is a Rust
  allocation), or the futex.
- Kani, both machines: the range arithmetic of every method, 32 harnesses at 1 to 4 words, "32 successful, 0 failed";
  a one-byte-long range check fails it ("2 of 616 failed").
- Only native tests cover the real memcpy, the listings, the guard pages, threads and processes.

## 7. Measurements (ns per copy, median of 11 alternating rounds, ratio to plain `copy_nonoverlapping`; MEASURED,
checks/bench-x86.log and bench-arm.log)

Sizes 8 B, 64 B, 4 KiB, 64 KiB, 2 MiB.

x86-64, load 5.0 of 96:
- out 1.30x, 1.42x, 1.07x, 0.96x, 1.00x; in 1.29x, 1.25x, 1.01x, 1.01x, 1.00x.
- old 8-byte atomics out 1.41x, 2.36x, 2.51x, 1.14x, 1.01x. `pread` out 72x, 93x, 7.1x, 1.8x, 1.4x.

ARM64, load 18 of 96 (shared, noisy):
- out 1.75x, 1.71x, 0.95x, 1.01x, 1.00x; in 0.70x, 0.71x, 0.97x, 0.99x, 1.02x.
- old atomics out 2.52x, 2.01x, 3.75x, 4.15x, 2.24x. `pread` out 172x, 99x, 6.7x, 3.9x, 3.4x.

With a thread storing 8-byte words over the same bytes: ours 0.96-1.03x of plain from 4 KiB up on both. On x86,
copying out 4 and 64 KiB, both ours and plain are far slower than the old 8-byte atomics (2239 and 24036 ns against
1329 and 3874).

The opacity probes in the same logs: the direct call becomes one load (x86 `mov (%rdi),%rax`, ARM `ldr x0, [x0]`) and
the volatile-loaded pointer stays a call (`call *%rax`, `blr x8`) without and with fat LTO; the primitive's 8-byte read
under fat LTO keeps its range check and ends in `jmp *%rcx` and `br x3`.

## 8. The TODO

One line above `MEMCPY`: replace the call with Rust's own copy when one tolerates a changing source; links
https://github.com/rust-lang/rust/issues/158947, https://github.com/rust-lang/rfcs/pull/3301,
https://github.com/rust-lang/rfcs/pull/2728.
