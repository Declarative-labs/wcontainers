# The guest-memory primitive, for review

Marks: MEASURED (a run, its log in `checks/`), READ (file:line), INFERRED. Lines are as of the commit that adds the section on order.

## The trait and the struct

```rust
mod seal { pub trait Sealed {} }   // private: no other crate can name it, so none can implement `Mapped`

/// # Safety
/// While the value lives: the bytes from `base` for `len` stay mapped readable and writable as ordinary memory, and
/// reaching them never traps; `base` is aligned to 8 and `len` is a multiple of 8, and neither changes; this program
/// reaches them at `base` only through [`Words`], and any other address of the same bytes (another mapping of their
/// file) and any system call on them counts as an outside agent; no unsafe code of this program trusts their contents;
/// whatever else writes them changes no other memory of this program.
pub unsafe trait Mapped: Send + Sync + seal::Sealed {
    fn base(&self) -> NonNull<u64>;
    fn len(&self) -> usize;
}

pub struct Words { mapping: Arc<dyn Mapped>, span: read_once::Span }   // Span: the base and count, read once, no store
// new, count, load, store, swap, read, write, wait, wake (Linux), and part, which gives a `Part` borrowing the words.
```

The two mappings: `HostMapping` (feature `host`: memmap2 anonymous memory, or a memory file sealed against shrinking) and `HeapMapping` (a `Box<[AtomicU64]>`), for Miri and Kani. It is compiled only with `--cfg miri` or `--cfg kani`, and that is a convention, not a guarantee: any build can set either flag (`RUSTFLAGS="--cfg miri" cargo build` compiles a crate that names it, MEASURED). It is sound, so a crate that reached it would get words on its own heap, not undefined behavior.

## The five unsafe lines

- `pub unsafe trait Mapped` (interface.rs:63): the promise above; only this crate can implement it.
- `slice::from_raw_parts(base, count)` (interface.rs:282): `base` and `count` (`len / 8`) were read from the mapping once, in `new`, and `Span` cannot change them; the promise keeps the range mapped, aligned and reached only atomically while `self` holds the `Arc`, and every index goes through the slice's own `get`, so wrong offset arithmetic gives an error or wrong bytes, never an access outside.
- `AtomicU32::from_ptr(half)` (interface.rs:300): the 4 bytes lie inside a word of that slice; only the kernel reads the address.
- `unsafe impl Mapped for HostMapping` (host/implementation.rs:145): memmap2 keeps the range mapped until drop; anonymous memory never traps; a file must carry the shrink seal (which nothing can remove), seen BEFORE its size is read, be long enough, and not be on hugetlbfs. Under strict overcommit a file mapping does trap: see below.
- `unsafe impl Mapped for HeapMapping` (heap/implementation.rs:29): a box this value owns, aligned to 8, freed only on drop.

## What is assumed about the kernel, the hardware and memmap2

- Linux keeps an `mmap` range mapped until `munmap`, which only memmap2's drop calls.
- A memory file sealed with `F_SEAL_SHRINK` never becomes shorter, through any descriptor, and the seal cannot be removed (MEASURED: `ftruncate` and `O_TRUNC` get `EPERM`).
- The seal is seen BEFORE the size is read; the other way, another descriptor can shrink and seal the file between the reads, and the mapping faults (SIGBUS). Built in: only `ShrinkSealedFile::new` (host/implementation.rs:100) makes one, after reading the seals, and only its `map` (:112) reads the size. Tested by a 4-second race.
- The checks and the mapping see one file: one `BorrowedFd`, which I/O safety keeps on the same file (a safe `dup2` over it is E0502, refused.rs `redirect`).
- memmap2 returns the address of the offset asked, not of its page (memmap2-0.9.12 src/unix.rs:99-100, :246, READ). `aligned` asserts it; the SAFETY comment relies on it for "the bytes from `base` are the file's from the offset". Tested: `maps_the_files_bytes_from_the_offset_asked_up_to_its_last_byte` fails with the base one word late or at the page start.
- Pages are always supplied on a fault. FALSE under strict overcommit (`vm.overcommit_memory=2`), MEASURED in a KVM guest (checks/overcommit.log): writing a mapped memory file's pages ends the process with SIGBUS once Committed_AS reaches CommitLimit; anonymous memory is refused at `mmap` instead. A crash, not a stray write, but "never traps" fails there, and this crate cannot prevent it: a hole punched through another descriptor is charged again when faulted back (INFERRED). Untested: memory-cgroup limits.
- Mappings of one file page at two addresses see each other's writes (MEASURED by test).
- `futex` wait and wake read the 4 bytes and never write them.
- Aligned 8-byte accesses by a guest, the stub or the kernel are single-copy atomic (Arm ARM; Intel SDM vol. 3A; INFERRED).
- A relaxed `AtomicU64` load or store is one plain load or store (not checked).

## Where correctness rests on an order, or on a check staying true

- The seal before the size (above): by construction, a race test, the SAFETY comment and this page.
- The size check staying true from the check to the mapping and for the mapping's life: the seal is permanent, and the size is read inside `map`, just before `mmap`, on the same descriptor. Tests: a second descriptor's shrink and `O_TRUNC` fail, and a grow-only seal is refused.
- The checks and the mapping on one file: one `BorrowedFd` (above).
- The start's alignment, checked once in `aligned`: the `MmapRaw` is private (E0616, `field`) and cannot be replaced under live words (E0507, `replace`).
- The base and count `Words::new` reads once, which the slice trusts at every use: `Span` has private fields and no method that stores, and `Words` no method that takes `&mut self`. No test can show more than that the code has no such method (READ).
- Not order-dependent, though they check first and use after: `read`, `write` and `word` check offsets through `plan` and then index the slice with `get`, so a wrong plan gives an error or wrong bytes, never an access outside; a `Part` checks its range when made and `Words` checks it again on every access; the futex address lives only as long as the borrow of `self` the call holds.

## What each check covers, and what it does not

- The suite (native, x86-64 and ARM64): 39 host tests, among them a 4-second race of a shrink and seal against `HostMapping::file`, and a check that the bytes mapped are the file's from the offset; both mappings' kinds, sizes that are not whole pages, all 32 sets of seals, a file shrunk, truncated, grown and punched through a second descriptor. Not covered: memory pressure (only the strict-overcommit probe in a guest, run by hand), other kernels.
- Miri: the host's anonymous mapping, the heap mapping and the checker's six attacks, with Stacked and Tree Borrows and 16 seeds of every threaded test. Not covered: memory files (Miri has no `memfd_create`) and the futex calls (Miri refuses rustix's call before our code).
- Kani: 7 harnesses at 1 to 4 words, every 64-bit offset and index, every buffer up to 33 bytes, any contents; every cover is reached. Not covered: threads, the real futex calls (stubbed), the host mapping, larger counts.
- Verus: the offset arithmetic for all sizes, but on proof/arithmetic.rs, a copy of arithmetic.rs; proof/agreement.rs checks the two give the same answers at the edges and on 4 million random inputs. Not covered: any unsafe line, and `read`'s promise to load no other word.
- The compiler: eight safe or outside-crate callers refused (attack/refused.rs), among them `E0603 module seal is private` and `E0277 the trait bound Mine: Sealed is not satisfied`, each with `unsafe_code` allowed.

## Measurements (nanoseconds per copy, median of 11, sealed memory file; MEASURED)

- x86-64, 4 KiB read: memcpy 30, Words 72, `pread` 210. 2 MiB read: 120.5 µs, 123.8 µs, 154.5 µs.
- ARM64, 4 KiB read: memcpy 49, Words 196, `pread` 366. 2 MiB read: 47.4 µs, 100.2 µs, 153.5 µs.
- Writes are within a few percent of reads on both.

## Found and fixed in two rounds of checking

- Round one (the builder's own wrong changes): an off-by-one offset check, an overflow in offset plus length, a part past its parent, and a slice one word too long were each caught by the suite, Miri or Kani.
- Round two (an independent checker): `Sealed` was `pub`, so any crate could write `unsafe impl Mapped`; it is now in a private module, and both mappings moved into this crate.
- The host mapping had no check beyond 4096-byte tests: `len()` rounded up to a page, and a seal check that accepted a grow-only seal (SIGBUS after a safe `ftruncate`), both passed; both now fail the suite, and the host suite runs under Miri.
- The promise said this program reaches the bytes only through `Words`, which a second mapping of the same file breaks; other addresses and system calls now count as outside agents.
- `read` did not forbid loads outside its range; its documentation now does.
- The futex on a word's high half passes the suite and Miri; only Kani refuses it (`1 of 1001 failed`), and that is still so.
- The Kani log predated the last commit and showed unreached covers; the log in `checks/` is of this tree.
- The Verus proof was of an unlabeled hand copy; the code's arithmetic now sits in its own file, and the copy is labeled and tested against it.
- `HeapMapping` was an ordinary crate that production code could depend on; it now exists only under `cfg(miri)` or `cfg(kani)`, so a normal build that names it fails with `E0433`, though any build may set those flags.
- Round three (the second independent check): the size read before the seals passed every test, Miri and Kani, and ended in SIGBUS under a race. The order is now built into `ShrinkSealedFile`, and the race test fails that wrong order within 1.6 s in each of 120 runs (MEASURED). refused.rs named E0432 where the build gives E0433. Strict overcommit was admitted and untested; it is now measured, and it does break "never traps" for memory files.
