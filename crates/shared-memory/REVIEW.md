# The guest-memory primitive, for review

Marks: MEASURED (a run, its log in `checks/`), READ (file:line), INFERRED. Lines are as of commit 673309d7a.

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

pub struct Words { mapping: Arc<dyn Mapped>, base: AtomicPtr<u64>, count: usize }
// new, count, load, store, swap, read, write, wait, wake (Linux), and part, which gives a `Part` borrowing the words.
```

The two mappings: `HostMapping` (feature `host`: memmap2 anonymous memory, or a memory file sealed against shrinking) and `HeapMapping` (a `Box<[AtomicU64]>`), which is compiled only under Miri and Kani.

## The five unsafe lines

- `pub unsafe trait Mapped` (interface.rs:63): the promise above; only this crate can implement it.
- `slice::from_raw_parts(base, self.count)` (interface.rs:257): `count` is `len / 8`, the promise keeps the range mapped, aligned and reached only atomically while `self` holds the `Arc`, and every index goes through the slice's own `get`, so wrong offset arithmetic gives an error or wrong bytes, never an access outside.
- `AtomicU32::from_ptr(half)` (interface.rs:275): the 4 bytes lie inside a word of that slice; only the kernel reads the address.
- `unsafe impl Mapped for HostMapping` (host/implementation.rs:105): memmap2 keeps the range mapped until drop; anonymous memory never traps; a file must carry the shrink seal (which nothing can remove), be long enough, and not be on hugetlbfs.
- `unsafe impl Mapped for HeapMapping` (heap/implementation.rs:28): a box this value owns, aligned to 8, freed only on drop.

## What is assumed about the kernel and the hardware

- Linux keeps an `mmap` range mapped until `munmap`, which only memmap2's drop calls.
- A memory file sealed with `F_SEAL_SHRINK` can never become shorter, through any descriptor (MEASURED: `ftruncate` and `O_TRUNC` get `EPERM`).
- Pages of a memory file or of anonymous memory are always supplied on a fault (not true under `vm.overcommit_memory=2` or a full hugetlbfs: then the fault is SIGBUS, a crash; INFERRED, not tested).
- Mappings of the same file page at two addresses see each other's writes coherently (MEASURED by test, not proven).
- `futex` wait and wake read the 4 bytes and never write them.
- An aligned 8-byte access by a guest, the stub process or the kernel is single-copy atomic (Arm ARM; Intel SDM vol. 3A; INFERRED).
- A relaxed `AtomicU64` load or store is one plain load or store (checkable in the assembly; not checked).

## What each check covers, and what it does not

- The suite (native, x86-64 and ARM64): 37 host tests, both mappings' kinds, sizes that are not whole pages, all 32 sets of seals, a file shrunk, truncated, grown and punched through a second descriptor. Not covered: memory pressure, other kernels.
- Miri: the host's anonymous mapping, the heap mapping and the checker's six attacks, with Stacked and Tree Borrows and 16 seeds of every threaded test. Not covered: memory files (Miri has no `memfd_create`) and the futex calls (Miri refuses rustix's call before our code).
- Kani: 7 harnesses at 1 to 4 words, every 64-bit offset and index, every buffer up to 33 bytes, any contents; every cover is reached. Not covered: threads, the real futex calls (stubbed), the host mapping, larger counts.
- Verus: the offset arithmetic for all sizes, but on proof/arithmetic.rs, a copy of arithmetic.rs; proof/agreement.rs checks the two give the same answers at the edges and on 4 million random inputs. Not covered: any unsafe line, and `read`'s promise to load no other word.
- The compiler: seven safe or outside-crate callers refused (attack/refused.rs), among them `E0603 module seal is private` and `E0277 the trait bound Mine: Sealed is not satisfied`, each with `unsafe_code` allowed.

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
- `HeapMapping` was an ordinary crate that production code could depend on; it now exists only under `cfg(miri)` or `cfg(kani)`, so a normal build that names it fails with `E0433`.
