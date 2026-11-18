# Runtime status

Plan: docs/runtime.md. Branch: phase-1-runtime. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-runtime.md.

## Where things stand

- J0 (the plan) is done: docs/runtime.md. J1 is being built (run 1 of J1, 2026-11-17).
- First light, 2026-11-17 23:2x: a program ran through both halves for the first time. The static hello
  passes against the reference with the kernel deciding and the translator's WebAssembly, run by wasmtime,
  running its instructions, at full, none and bounded look-ahead:
  `nix develop -c cargo test -j 4 -p wcontainers-kernel-test --features runtime --test hello -- --test-threads 3`
  (and with `LOOK_AHEAD=none`, `LOOK_AHEAD=bounded` in front). Logs: scratch j1/hello-full-2.log,
  hello-none.log, hello-bounded.log (load 17.1, 13.2, 13.4).
- Done in J1 so far: the plan's carried findings fixed (see "Findings of J0's last check" below); the merge
  of 84e9a937; crates/wcontainers-runtime (`Stepper::step`, `drive`, `Execution`, `Outside`, `Answer`,
  `Turn`, `Carry`, `Step`, `Ending`); crates/wcontainers-wasmtime (`Compiler`, `CpuSide`); the kernel test
  crate's driver on `drive` (commit bdf240e1) and its `runtime` feature with `LOOK_AHEAD` (commit 2c219b7f).
- Next: busybox at full; the whole-suite verdict diff of the driver move; the recorded figures of J1.

## How to run

Nothing to run in the repository yet. The J0 measurements (scratch, not in the repository) are in
pyro-parked/wcontainers-scratch/runtime:
- `base/` is the trial merge's tree (main 36d20013 with 84e9a937) plus three scratch files:
  `base/crates/wcontainers-translator-test/programs/syscall_loop.S`,
  `base/crates/wcontainers-translator-test/tests/runtime_measure.rs` (tests a to g) and
  `base/crates/wcontainers-kernel-test/tests/runtime_measure.rs` (the kernel's own share). The scratch
  runner.rs there also has an optional pooling allocator (environment `WC_POOLING`), and its Cargo.toml the
  `pooling-allocator` feature. `base/target/binaries` holds copies of busybox, musl and python from the
  translator's list.
- `measure.sh`, `measure2.sh` and `measure3.sh` run them (`nix develop -c cargo test --release -j 4 ... --test
  runtime_measure -- --nocapture --test-threads 1`, with `CARGO_TARGET_DIR` in scratch). Logs:
  `measure-*.log`, `forksize/sizes.txt` for the reference's memory sizes, `retired-gzip.log`. The build
  output is deleted at the end of each run; a rerun rebuilds it (about 2 minutes at load 35).
- `forkcopy/forkcopy.c` (`cc -O2 -o forkcopy forkcopy.c && ./forkcopy`): the macOS fork copy; log
  `forkcopy/forkcopy.log`.
- `python3 -I check-j0-run2/relr.py <binaries>`: relocation kinds; log `relr.log`.
- What the logs hold: `measure2.out` shows its first host memory run exited 101 and its two rounds ended
  within seconds; those logs were overwritten by later runs started by hand: plain-3, plain-4, pooling-1,
  plain-5, pooling-2, plain-6 in that order, then `measure-hostmemory.log` (tests d and e together).
  plain-1 and plain-2 are measure2.sh's. `measure-none.log` is measure3.sh's (test g, and test b, whose name
  also matched).
- `runner-84e9a937.rs` is the translator runner at X1, copied for reading line numbers.
- `ceiling/reserve.c` (`cc -O2 -o reserve reserve.c && ./reserve`): address room; log `ceiling/reserve.log`.
  `ceiling/reference-maps.log`: the reference's layout (`docker run --rm --platform linux/arm64 alpine:3.22
  cat /proc/self/maps`).

## Measurements (2026-11-17, this Mac: Apple M5, 18 cores; wasmtime 50.0.1; release)

Load is `sysctl vm.loadavg` (1, 5, 15 minutes) at the time; other tracks were running. Plan section 18 has the
same figures with the log of each.
- Reference resident and anonymous memory (`smaps_rollup`, Docker ARM64 Linux; load 29): busybox sh 892 /
  48 kB; python 3.13 8592 / 3364 kB; node 22 48204 / 7268 kB; java 21 44656 / 25064 kB.
- New 4 GiB wasmtime memory plus a copy into it (median of 15; load 39.8): 48 KB 6.4 + 4.0 µs; 3364 KB 6.3 +
  167.8 µs; 7268 KB 9.0 + 387.6 µs; 25064 KB 11.6 + 1587.7 µs; 48204 KB 15.6 + 3229.6 µs.
- Fork copy on macOS (median of 15; load 27.7): `mach_vm_copy` against `memcpy` of the written bytes: busybox
  sh layout (11,200 KB mapped, 1,784 KB written) 19.1 against 115.6 µs; python-sized 197.9 against 610.6 µs;
  java-sized 1292.4 against 3354.6 µs; node-sized 483.0 against 3521.2 µs; exact in all 60 rounds. The parent
  rewriting its written pages after the copy: 266.3 µs, 1575.6 µs, 8916.5 µs, 9576.1 µs, more than `memcpy` of
  the written set in every case. 2000 copies in a row: mean of the first ten 20.3 µs, of the last ten 18.0 µs,
  exact.
- System call in the translator's runner, full look-ahead (load 39.8): in the hook 10.5 ns (median of 7
  alternating pairs of 1,000,000 calls; min 9.4); leaving `run` and back 300.8 ns (median of 5 of 100,000; min
  285.7). Again at load 26.5 (measure-none.log): leaving 286.3 ns (min 283.5).
- Kernel's own share (layer 1 buffer CPU; load 64.8): `getppid` 116.3 to 120.0 ns (5 x 1,000,000); fork, exit
  and wait 2.42 to 2.65 µs (5 x 20,000).
- Real binaries, full, speed, default allocator, 8 runs (load 41.5 to 60.6; instantiation median of 3 and
  bringing back median of 4 in each run): busybox translate 0.15 to 0.35 s, compile 1.12 to 2.08 s,
  instantiate 0.43 to 0.60 ms, from a serialised file 2.77 to 3.59 ms; musl 0.06 to 0.16 s, 0.51 to 0.99 s,
  0.27 to 0.42 ms, 1.19 to 1.67 ms; libpython (1 run, load 57.1) 0.46 s, 6.60 s, 1.19 ms, 12.56 ms.
- Look-ahead none, whole binary from M5's starts (load 27.1 to 26.2): busybox 135,972 pieces, compile 8.45 s,
  peak 3,120 MB; musl 85,954, 5.09 s; libpython 703,235, 113.92 s, peak 16,918 MB. Reading M5's starts from
  the file: 0.07, 0.08 and 1.37 ms.
- Relocations: busybox, musl, git and rg keep all relative relocations in `RELR` (1074, 80, 4253, 14,146
  places); python in `RELA` (12,063).
- Pooling allocator, 2 runs interleaved with 4 default runs (load 54.2 to 63.7): busybox 0.60 and 0.55 ms
  against 0.44 to 0.60 ms; musl 0.36 and 0.35 ms against 0.33 to 0.42 ms: no difference.
- BLAKE3 of code (median of 21; load 40.3): busybox 0.300 ms, musl 0.159 ms, libpython 1.275 ms.
- Host-supplied memory (macOS; load 59.2): one 1 MiB shared memory object mapped into two wasmtime guest
  memories; 1000 of 1000 stores seen across; mapping 0.6 µs (median of 50).
- Instructions retired against time, `gzip -c` of one 20 MB file, 8 runs (load 88.45, read once after them): 2,716,771,743 to
  2,721,493,991 instructions (0.17% apart); 0.25 to 0.29 s.

- Address room (macOS 27.2; load 38.34): one process can reserve 2^47 bytes in all (largest single
  reservation 130,578 GiB). The reference (Docker ARM64, `alpine:3.22`; load 41.28) puts its stack at
  0xffff_d4a3_b000: 48 bit addresses.

Expected only (not measured): a system call answered at once about 130 ns end to end (10.5 + 116 to 120); a
shell's fork about 0.75 ms, nearly all of it instantiating busybox's and musl's modules; rebuilding a child's
array of slots in microseconds.

## Departures from the phase-one plans and the brief

- The container's address-space size is the guest's Linux size (2^48 for ARM64), not the kernel plan's 4 GiB,
  as the brief's late section directs; the memory size told is a separate value given in `Start` (proposed
  4 GiB). Each back end has its own address ceiling, computed from the output's widths (0xFF00_0000 for ARM64
  today). Until the kernel holds the size and places low first, it lays processes out inside
  `Cpu::address_space` as today, so the stand-in (4 GiB) and the WebAssembly back end (0xFF00_0000) lay out
  and tell sizes differently in J1. Listed for the owner in plan section 20.
- Today's translator contract keeps the permission table and the array of slots in a 32 bit state memory,
  so its output does not yet give a process the largest address space the host offers (translator plan
  section 6). The plan asks the translator track for a wider form; nothing in the runtime changes but its
  layout code.
- Fork copies exactly with the host's own copy, which on macOS copies each page on first write (the kernel
  plan's expectation for wasmtime) and on Linux copies the present and swapped pages at once.
- Starts are M5's list read from the image in memory at first use, not from the file: no `.symtab`, and the
  address first reached is not a start (plan section 7).
- The kernel test driver's own loop is replaced by the runtime's `step` and `drive`; its per-turn work moves
  into the test host's `Outside`. One loop instead of two (plan section 5).
- The brief's layer 1 bar ("with the translator behind the boundary") is read as the WebAssembly back end's
  memories, tables and copies in place of the buffer CPU, since layer 1 runs no instructions (plan section 1).
- Until the translator emits the guest-defined progress count, the WebAssembly back end counts stop checks in
  its place (named, counted and warned); J4 waits for the count (plan section 9).
- Look-ahead none is not used for the K6b runtimes (J6): libpython at none took 113.9 s to compile and 16.9 GB.
- The benchmark margin is checked on instructions retired, with times reported beside it (plan section 15;
  listed for the owner).
- wasmtime's engine uses Unix signals for traps on macOS, not its default Mach ports, so a process copy (the
  tests' saves) can still run WebAssembly (plan section 11).

## Dependencies

None added yet. Proposed, licences read from their manifests on 2026-11-17: wasmtime 50.0.1 (Apache-2.0 WITH
LLVM-exception, already used by the translator tests); rustix 1.1 (Apache-2.0 WITH LLVM-exception OR
Apache-2.0 OR MIT) for mappings and shared memory objects; mach2 0.6.0 (BSD-2-Clause OR MIT OR Apache-2.0,
already in the lock file through wasmtime) for `mach_vm_copy` on macOS; blake3 1.8 (CC0-1.0 OR Apache-2.0 OR
Apache-2.0 WITH LLVM-exception, already a dependency of the shipped translator crate); gimli 0.34 (MIT OR
Apache-2.0, already used by the translator test crate) for unwind tables; rayon 1.12 (MIT OR Apache-2.0,
already a dependency of the shipped translator crate) for the back end's own pool; object 0.41 (Apache-2.0
OR MIT, the kernel's version, so one build holds one); tar 0.4.46 (MIT OR Apache-2.0, already used by the
kernel test crate) for J3. Linux's `pagemap` is read with the standard library.
