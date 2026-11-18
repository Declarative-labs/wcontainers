# Runtime status

Plan: docs/runtime.md. Branch: phase-1-runtime. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-runtime.md.

## Where things stand

- J0 (the plan) is written: docs/runtime.md. No crate is built yet.
- Next is J1 (plan section 14). Its steps, in order:
  1. Merge `origin/phase-2-x86` at 84e9a937 into this branch (the J1 task names the merge). A trial with
     `git merge-tree --write-tree 36d20013 84e9a937` on 2026-11-17 gave no conflicts.
  2. Create `crates/wcontainers-runtime`: host-supplied guest memories (rustix), one wasmtime store and one
     4 GiB state memory per process (16 MiB of permission table and thread blocks, then the array of slots
     from 64 KiB to 0xFF00_0000), `CpuSide<G>` with `impl Cpu for CpuSide<Arm64>`, translation at first use
     in the load code hook with an in-memory cache, and the run loop answering system calls and stops inside
     the hooks. `memory_duplicate` answers `NotSupported` until J2.
  3. Make the kernel test driver (crates/wcontainers-kernel-test/src/driver.rs) generic over its CPU side and
     add the crate feature `runtime`, as its own commit; say so here, since the kernel track also edits that
     crate.
  4. Bar: `cargo test -p wcontainers-kernel-test --features runtime --test hello --test busybox --
     --test-threads 3`, plus hello at look-ahead none; and the same tests without the feature.
- The scratch measurement code from J0 can be reused for J2's and J7's measurements (below).

## How to run

Nothing to run in the repository yet. The J0 measurements (scratch, not in the repository) are in
pyro-parked/wcontainers-scratch/runtime:
- `base/` is the trial merge's tree (main 36d20013 with 84e9a937) plus three scratch files:
  `base/crates/wcontainers-translator-test/programs/syscall_loop.S`,
  `base/crates/wcontainers-translator-test/tests/runtime_measure.rs` (tests a to f) and
  `base/crates/wcontainers-kernel-test/tests/runtime_measure.rs` (the kernel's own share). The scratch
  runner.rs there also has an optional pooling allocator (environment `WC_POOLING`), and its Cargo.toml the
  `pooling-allocator` feature.
- `measure.sh` and `measure2.sh` run them (`nix develop -c cargo test --release -j 4 ... --test
  runtime_measure -- --nocapture --test-threads 1`, with `CARGO_TARGET_DIR` in scratch). Logs:
  `measure-*.log`, and `forksize/sizes.txt` for the reference's memory sizes.

## Measurements (2026-11-17, this Mac: Apple M5, 18 cores; wasmtime 50.0.1; release)

Load is `sysctl vm.loadavg` (1, 5, 15 minutes) at the time; other tracks were running.
- Reference resident and anonymous memory (`smaps_rollup`, Docker ARM64 Linux; load 29): busybox sh 892 /
  48 kB; python 3.13 8592 / 3364 kB; node 22 48204 / 7268 kB; java 21 44656 / 25064 kB.
- New 4 GiB wasmtime memory plus a copy into it (median of 15; load 40 to 28): 48 KB 6.4 + 4.0 µs; 3364 KB
  6.3 + 168 µs; 7268 KB 9.0 + 388 µs; 25064 KB 11.6 µs + 1.59 ms; 48204 KB 15.6 µs + 3.23 ms. `mincore` over
  4 GiB: 2.77 ms.
- System call in the translator's runner, full look-ahead: in the hook 10.5 ns (median of 7 alternating
  pairs of 1,000,000 calls; min 9.4); leaving `run` and back 301 ns (median of 5 of 100,000; min 286); load
  40 to 28.
- Kernel's own share (layer 1 buffer CPU; load 65 to 34): `getppid` 116 to 120 ns (5 x 1,000,000); fork,
  exit and wait 2.42 to 2.65 µs (5 x 20,000).
- Real binaries, full, speed (load 41 to 63): busybox translate 0.15 to 0.35 s, compile 1.12 to 2.08 s,
  instantiate 0.43 to 0.60 ms, from a serialised file 2.65 to 3.26 ms; musl 0.06 to 0.16 s, 0.51 to 0.95 s,
  0.27 to 0.42 ms, 1.19 to 1.56 ms; libpython 0.46 s, 6.60 s, 1.10 to 1.19 ms, 12.6 ms. Pooling allocator
  alternated with the default, two rounds (load 55 to 63): busybox 0.44 to 0.60 against 0.43 to 0.56 ms,
  musl 0.28 to 0.36 against 0.26 to 0.34 ms: no difference.
- BLAKE3 of code (median of 21; load 40): busybox 0.30 ms, musl 0.16 ms, libpython 1.28 ms.
- Host-supplied memory (macOS; load 59): one 1 MiB shared memory object mapped into two wasmtime guest
  memories; 1000 of 1000 stores seen across; mapping 0.6 µs; unsharing by fresh pages and copy 114 µs.

Expected only (not measured): a system call answered at once about 130 ns end to end (10.5 + 117); a
shell's fork about 0.75 ms, nearly all of it instantiating busybox's and musl's modules.

## Departures from the phase-one plans

- The first form's address space ends at 0xFF00_0000, not 4 GiB: the translator's array of slots must cover
  the whole address space and share one 4 GiB state memory with the permission table and thread blocks.
  Listed for the owner in plan section 18.
- Fork copies the pages in use instead of copying on first write (kernel plan section 7 expected the latter
  for wasmtime): measured small next to instantiation; can come later behind `memory_duplicate`.

## Dependencies

None added yet. Proposed for J1, licences read from their manifests on 2026-11-17: wasmtime 50.0.1
(Apache-2.0 WITH LLVM-exception, already used by the translator tests); rustix 1.1 (Apache-2.0 WITH
LLVM-exception OR Apache-2.0 OR MIT); blake3 1.8 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception,
already a dev-dependency of the translator tests).
