# Runtime status

Plan: docs/runtime.md. Branch: phase-1-runtime. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-runtime.md.

## Where things stand

- J0 (the plan) is written: docs/runtime.md. No crate is built yet.
- Run 2 of J0 fixed what the independent check found: one loop (`drive`) for the shipped runtime and the
  kernel tests, with `Execution` (back end) and `Outside` (host and arriving events) as its two compile-time
  interfaces (plan section 5); the rules for the kernel calling back into the back end from inside a hook
  (section 6); counted and free turns, so the repeatable test clock moves with stop checks (section 9);
  saving by process copy with wasmtime (section 11); J1 widened to what busybox's tests actually need
  (exec, waits, `Idle`, the file backend), fork left to J2; figures matched to the kept logs. It also added
  section 12 for the brief's later section on the owner's other project (back ends, guest state, shareable
  cache files, dirty pages, repeatable runs and every source of non-repeatability).
- Next is J1 (plan section 16). Its steps, in order:
  1. Merge `origin/phase-2-x86` at 84e9a937 into this branch (the J1 task names the merge). A trial with
     `git merge-tree --write-tree 36d20013 84e9a937` on 2026-11-17 gave no conflicts.
  2. Create `crates/wcontainers-runtime` (`drive`, `Execution`, `Outside`, `Turn`) and
     `crates/wcontainers-wasmtime` (`CpuSide<G>`, `impl Cpu for CpuSide<Arm64>`, host-supplied guest memories,
     one store and one state memory per process, translation at first use, the in-memory cache, counted
     turns, Unix signal traps). `memory_duplicate` answers `NotSupported` until J2.
  3. In crates/wcontainers-kernel-test, as their own commits: the stand-in implements `Execution`, `TestHost`
     implements `Outside` with the driver loop's per-turn work, `run_prepared` calls `drive`, and the feature
     `runtime`. Check first that a seeded hello run's output, counters and turn count do not change. Say so
     here, since the kernel track also edits that crate.
  4. Bar: `cargo test -p wcontainers-kernel-test --features runtime --test hello -- --test-threads 3` at full
     and none, then with `--test busybox`; and the same tests without the feature.

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
  `measure-*.log`, `forksize/sizes.txt` for the reference's memory sizes, `retired-gzip.log`. The build
  output was deleted; a rerun rebuilds it (about 2 minutes at load 35).
- What the logs hold: `measure2.out` shows its first host memory run exited 101 and its two rounds ended
  within seconds; those logs were overwritten by later runs started by hand: plain-3, plain-4, pooling-1,
  plain-5, pooling-2, plain-6 in that order, then `measure-hostmemory.log` (tests d and e together).
  plain-1 and plain-2 are measure2.sh's.
- `runner-84e9a937.rs` is the translator runner at X1, copied for reading line numbers.

## Measurements (2026-11-17, this Mac: Apple M5, 18 cores; wasmtime 50.0.1; release)

Load is `sysctl vm.loadavg` (1, 5, 15 minutes) at the time; other tracks were running. Plan section 18 has the
same figures with the log of each.
- Reference resident and anonymous memory (`smaps_rollup`, Docker ARM64 Linux; load 29): busybox sh 892 /
  48 kB; python 3.13 8592 / 3364 kB; node 22 48204 / 7268 kB; java 21 44656 / 25064 kB.
- New 4 GiB wasmtime memory plus a copy into it (median of 15; load 39.8): 48 KB 6.4 + 4.0 µs; 3364 KB 6.3 +
  167.8 µs; 7268 KB 9.0 + 387.6 µs; 25064 KB 11.6 + 1587.7 µs; 48204 KB 15.6 + 3229.6 µs. `mincore` over 4 GiB:
  2773.8 µs.
- System call in the translator's runner, full look-ahead (load 39.8): in the hook 10.5 ns (median of 7
  alternating pairs of 1,000,000 calls; min 9.4); leaving `run` and back 300.8 ns (median of 5 of 100,000; min
  285.7).
- Kernel's own share (layer 1 buffer CPU; load 64.8): `getppid` 116.3 to 120.0 ns (5 x 1,000,000); fork, exit
  and wait 2.42 to 2.65 µs (5 x 20,000).
- Real binaries, full, speed, default allocator, 8 runs (load 41.5 to 60.6; instantiation median of 3 and
  bringing back median of 4 in each run): busybox translate 0.15 to 0.35 s, compile 1.12 to 2.08 s,
  instantiate 0.43 to 0.60 ms, from a serialised file 2.77 to 3.59 ms; musl 0.06 to 0.16 s, 0.51 to 0.99 s,
  0.27 to 0.42 ms, 1.19 to 1.67 ms; libpython (1 run, load 57.1) 0.46 s, 6.60 s, 1.19 ms, 12.56 ms.
- Pooling allocator, 2 runs interleaved with 4 default runs (load 54.2 to 63.7): busybox 0.60 and 0.55 ms
  against 0.44 to 0.60 ms; musl 0.36 and 0.35 ms against 0.33 to 0.42 ms: no difference.
- BLAKE3 of code (median of 21; load 40.3): busybox 0.300 ms, musl 0.159 ms, libpython 1.275 ms.
- Host-supplied memory (macOS; load 59.2): one 1 MiB shared memory object mapped into two wasmtime guest
  memories; 1000 of 1000 stores seen across; mapping 0.6 µs (median of 50).
- Instructions retired against time, `gzip -c` of one 20 MB file, 8 runs (load 88 to 102): 2,716,771,743 to
  2,721,493,991 instructions (0.17% apart); 0.25 to 0.29 s.

Expected only (not measured): a system call answered at once about 130 ns end to end (10.5 + 116 to 120); a
shell's fork about 0.75 ms, nearly all of it instantiating busybox's and musl's modules.

## Departures from the phase-one plans

- The first form's address space ends at 0xFF00_0000, not 4 GiB: the translator's array of slots must cover
  the whole address space and share one 4 GiB state memory with the permission table and thread blocks.
  Listed for the owner in plan section 20.
- Fork copies the pages in use instead of copying on first write (kernel plan section 7 expected the latter
  for wasmtime): measured small next to instantiation; can come later behind `memory_duplicate`.
- Starts read from an ELF image in memory are narrower than the translator plan's list (section 5 there):
  no `RELR` relocations and no data words of an image that is not moved, because they sit in writable data
  that may have changed by first use, and the cache key needs the same bytes to give the same starts. More
  pieces are made at run time than M5 counted; J1 measures how many.
- The kernel test driver's own loop is replaced by the runtime's `drive`; its per-turn work moves into the
  test host's `Outside`. One loop instead of two (plan section 5).
- The benchmark margin is checked on instructions retired, with times reported beside it (plan section 15;
  listed for the owner).
- wasmtime's engine uses Unix signals for traps on macOS, not its default Mach ports, so a process copy (the
  tests' saves) can still run WebAssembly (plan section 11).

## Dependencies

None added yet. Proposed, licences read from their manifests on 2026-11-17: wasmtime 50.0.1 (Apache-2.0 WITH
LLVM-exception, already used by the translator tests); rustix 1.1 (Apache-2.0 WITH LLVM-exception OR
Apache-2.0 OR MIT); blake3 1.8 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception, already a
dev-dependency of the translator tests); object 0.40 (Apache-2.0 OR MIT, already used by the translator
tests); tar 0.4.46 (MIT OR Apache-2.0, already used by the kernel test crate) for J3.
