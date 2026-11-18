# Runtime status

Plan: docs/runtime.md. Branch: phase-1-runtime. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-runtime.md.

## Where things stand

- J0 (the plan) is done: docs/runtime.md. J1 is being built (run 1 of J1, 2026-11-17).
- First light, 2026-11-17 23:2x: a program ran through both halves for the first time. The static hello
  passes against the reference with the kernel deciding and the translator's WebAssembly, run by wasmtime,
  running its instructions, at full, none and bounded look-ahead:
  `nix develop -c cargo test -j 4 -p wcontainers-kernel-test --features runtime --test hello -- --test-threads 3`
  (and with `LOOK_AHEAD=none`, `LOOK_AHEAD=bounded` in front). Pushed at that point.
- Busybox's single programs pass at full look-ahead (`--test busybox`: text, files, system, one_cpu; the
  known failure root_file_system stays ignored as with the stand-in), each command against the reference with
  the root in memory and on the backend: 4 passed, 1 ignored, 66.9 s (load 10.5 at the start, 26.2 at the end;
  scratch j1/busybox-full-1.log).
- Built in J1:
  - crates/wcontainers-runtime: `Stepper::step` and `drive` (the one loop), `Execution` (with `Failure` and
    `restored`), `Outside` (with `End`; `turn`, `ran`, `arrived`, `stopped`, `idle`, `cpu_warning`),
    `Answer`, `Carry`, `Turn` (counted turns only; free turns come with J3's timer), `Step`, `Ending`.
  - crates/wcontainers-wasmtime: `Compiler` (engine with host-supplied memories and Unix signal traps, the
    thread module, the back end's own rayon pool, the probe, the in-memory cache) and `CpuSide<G>` (every
    `Cpu` method but `memory_duplicate`, `run` with carrying on in place and leaving, the hooks, translation
    at first use from the kernel's starts and the image's own, shared blocks by copying with the "shared
    pages" write lock).
  - The kernel test crate (the plan's one change outside the runtime's crates, its own commits): bdf240e1
    moves the driver onto `drive` (the stand-in implements `Execution`, `TestHost` implements `Outside`
    holding the driver's per-turn work); 2c219b7f adds the `runtime` feature and `LOOK_AHEAD`; later commits
    add `LOOK_AHEAD_SEED` and the back end's report on the standard error.
- Not yet in J1 (next, in order): the whole-suite verdict diff of the driver move (step 1's check); hello and
  busybox at bounded and none (running); the recorded figures of J1 (CPU warnings per command, run-time
  pieces per command, second-container time); the whole suite of what this track owns before the milestone
  is reported complete.

## Findings of J0's last check, and how each was settled

- Shared block calls in J1 (blocking): moved into J1 and built (plan sections 8 and 16). Building them showed
  this Mac's pages are 16 KiB against the guest's 4 KiB, so blocks are kept by the copying method with a write
  lock, not by host mappings (plan section 8, and Departures below). Section 8's sentence about guest memory
  holding everything the kernel loaded is corrected: file mappings, the ELF images among them, show blocks and
  the fork copy skips them.
- The user surface (blocking): plan section 14 now takes `oci_spec::runtime::Spec`, the image-spec
  conversion, every field honoured or refused by name with the list of refused fields and what each would
  take, how we run (`Tuning`, suggested) and what machine it sees (`Machine`, suggested, with one `Offer`
  field per architecture) apart; names as approved; the command line program as J3's last, thin part;
  section 20 asks only what is still open (the two names, the program's name, the OCI image format).
- Progress with two meanings in J1 (minor, ruled interim): recorded under Departures; `Turn` and progress
  kept; no test compares progress between back ends or look-aheads; the loop's count of the kernel's
  changes of pc moves to J2 with the stand-in's own count, since both change the stand-in's unit.
- RELR measured on 5 of 9 binaries: plan sections 7 and 18 now give java's and node's `RELA` (from the
  check's log), bun's neither, go not read; the design reads both.
- Load of the system call figures: section 18 now names measure-translator.log, load 39.78.
- Exec and the count: section 9 says exec is not counted and the count does not survive it (a new thread's
  block starts at zero); a new thread's first `FromRegisters` is not counted.
- How a generic outside rebuilds the back end after a save: `Execution::restored()` (plan section 5); the
  test outside's `stopped` calls it. Built for the stand-in; for `CpuSide` it panics naming J2, where it is
  built with the fork copy (no J1 test saves).
- The cache key leaving out the abi and guest interface crates: section 7's build identity now covers the
  translator, `wcontainers-guest-cpu`, the guest crate, `wcontainers-abi` and wasm-encoder's locked version;
  built with the disk level (J2), since the in-memory level lives inside one build.
- Data words read two ways: an image's `RELR` words are now read one way for the whole image (relocated
  when more of them, less the load offset, fall in the image's span); `RELA` starts come from addends. Built
  in crates/wcontainers-wasmtime/src/starts.rs.
- Line offsets: RNDR at decode_forms.rs:253 and the clock hook at guest.rs:434-441, in both places.

## How to run

The one build directory of this track is the checkout's own `target/` (CARGO_TARGET_DIR unset). Scratch is
pyro-parked/wcontainers-scratch/runtime; J1's logs are in its `j1/`.
- The bar, first step: `nix develop -c cargo test -j 4 -p wcontainers-kernel-test --features runtime --test
  hello -- --test-threads 3`, again with `LOOK_AHEAD=none` and with `LOOK_AHEAD=bounded` in front.
- The bar: `nix develop -c cargo test -j 4 -p wcontainers-kernel-test --features runtime --test hello --test
  busybox -- --test-threads 3` (full; the same with `LOOK_AHEAD=bounded` and `LOOK_AHEAD=none` also pass).
- `LOOK_AHEAD` is `full` (also when unset), `none` or `bounded`; at `bounded` the container's N is drawn from
  1 to 64 from the run's seed and `LOOK_AHEAD_SEED` (0 if unset) and printed ("look-ahead bounded N"). hello
  and busybox run with seed 0, so `LOOK_AHEAD_SEED` is the knob that changes N for them.
- With the feature, each run prints the back end's report on the standard error ("cpu side: ..."): its
  translations, how many were made at run time, translating and compiling time, cache hits, the run's time,
  and per range its starts by source. `--nocapture` shows it.
- The stand-in unchanged: `nix develop -c cargo test -j 4 -p wcontainers-kernel-test --no-fail-fast --
  --test-threads 3`, its `test ... ok/FAILED/ignored` lines sorted and compared with the merge commit's
  (scratch j1/baseline/verdicts-before.txt). The seeded runs' check is the scratch test
  j1/standin-check/zz_standin_check.rs, copied into the crate's tests/ for the run and removed after; it prints
  hello (seeds 1 and 5), turns (seed 1) and the shell's four jobs (seeds 7, 8, 9): output, warnings,
  counters, turn counts.
- J0's measurements (scratch, not in the repository) are in pyro-parked/wcontainers-scratch/runtime:
  - `base/` is the trial merge's tree (main 36d20013 with 84e9a937) plus three scratch files:
    `base/crates/wcontainers-translator-test/programs/syscall_loop.S`,
    `base/crates/wcontainers-translator-test/tests/runtime_measure.rs` (tests a to g) and
    `base/crates/wcontainers-kernel-test/tests/runtime_measure.rs` (the kernel's own share). Its target
    directory is deleted; a rerun rebuilds it.
  - `measure.sh`, `measure2.sh` and `measure3.sh` run them; logs `measure-*.log`, `forksize/sizes.txt`,
    `retired-gzip.log`; `forkcopy/forkcopy.c` and its log; `check-j0-run2/relr.py` and `relr.log`;
    `ceiling/reserve.c` and `ceiling/reference-maps.log`. measure2.out's first host memory run exited 101;
    its logs were overwritten by later runs started by hand (plain-3, plain-4, pooling-1, plain-5, pooling-2,
    plain-6, then measure-hostmemory.log); `measure-none.log` is measure3.sh's.

## J1 measurements (2026-11-17, this Mac: Apple M5, 18 cores; wasmtime 50.0.1; the tests' build, opt-level 3 for dependencies and 1 for the workspace)

Load is `sysctl vm.loadavg` at the start and end of each run; other tracks were running. Logs in scratch j1/.
- hello at full, none and bounded (N 60 from seed 0): passes; one translation of its one page of code from 2
  starts (entry 1, unwind 1, ELF data words 3 before merging), 1 piece at full and bounded, 18 at none; made in
  17 to 22 ms translating and 1.2 to 2.3 ms compiling (hello-*.log; load 13.2 to 17.1).
- busybox, full, one command at a time (busybox-full-record.log and its summary; load 31.9 at the start, 33.2
  at the end; 178 runs, two per command, the root in memory then on the backend):
  - first use of each range, translated and compiled while the first command ran: musl's code (0xa2000 bytes)
    0.169 s translating and 0.787 s compiling to 2219 pieces from 1538 starts; busybox's (0xd2000 bytes) 0.324 s
    and 1.764 s to 4368 pieces from 559 starts. M5's runner made 1824 and 3520 pieces from the files.
  - starts by source before merging (`Sources`): musl entry 1, init and fini 2, dynamic symbols 1611, unwind
    table 22, relocations 45; busybox entry 1, init and fini 2, symbols 7, unwind 4, relocations 607 (RELR read
    one way, as relocated), init arrays 2.
  - a second container in the same process (cache hit for every range): run time median 5.6 ms (min 4.6, max
    247.9, 112 runs that made nothing), from the kernel's start to its finish.
  - pieces translated at run time per run: median 5, max 51 (0 for the runs where every address already had
    one).
  - the CPU side's warnings, by reason, in how many of the 178 runs and their counts per run (min, median,
    max): progress counted by stop checks 178 (1, 6, 893: runs of `run`, warned once each); loop not proven
    177 (506, 559, 1647); jump through a register not recognised 177 (13, 39, 1,838,650, the most in `cmp
    /bin/busybox /bin/sh` on the backend, 1075 ms); continued after stack loss 175 (1, 1, 1777); translated at
    run time 177 (2, 5, 51); memory fault 177 (2, 5, 10); left to an address 2 (1). First write to a shared
    page, code changed after it ran and code outside an announced range: none. Per command: the summary file
    (j1/busybox-full-record-summary.txt), made by j1/scripts/summarise.py from the log.
- busybox at bounded (N 60) and at none, three tests at a time (busybox-bounded-none-1.log): pass, 37.0 s
  (bounded; load 19.7) and 55.8 s (none; load 24.9 to 32.6). At none the first two runs, side by side, each
  made the whole of musl and busybox: 1.05 and 1.07 s translating, 18.7 and 19.5 s compiling.
- Not measured, and why: what counting every stop check costs against full speed needs a turn that counts
  nothing, which comes with J3's free turns; making a per-process 4 GiB state memory alone (it is inside the
  4.6 ms of the fastest cache-hit run).

## J0 measurements (2026-11-17, this Mac: Apple M5, 18 cores; wasmtime 50.0.1; release)

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
- Blocks of shared pages are kept by the copying method on every host, not by host mappings as plan section 8
  first said: this Mac's pages are 16 KiB (`sysctl hw.pagesize`) and the guest's 4 KiB, and the kernel maps
  and unmaps blocks a 4 KiB page at a time. Each memory holds a copy; pages that show a block carry the
  "shared pages" write lock; the guest's first write faults to the back end (counted "first write to a
  shared page"), and written pages are copied into the block and every other memory before the kernel sees
  anything. Exact while one guest thread runs at a time (plan section 8).
- Progress has two meanings in J1, accepted by the orchestrator until the owner rules on the count: the
  stand-in counts its instructions and the WebAssembly back end counts stop checks passed (named, counted and
  warned once per run: "progress counted by stop checks"). The translator is not changed for it. No test
  compares progress between the two back ends or between look-aheads; that waits for the translator's
  guest-defined count. The loop's count of the kernel's changes of pc moves to J2 with the stand-in's own
  count, since either alone changes the stand-in's results.
- At bounded look-ahead N is drawn once per container from the run's seed (and `LOOK_AHEAD_SEED`), not per
  range from the range's start as plan section 15 first said: the look-ahead is a setting of the container,
  and drawing it per range would put a test's behaviour in the shipped back end (plan section 15 corrected).
- The private words of a thread are zeroed at every `FromRegisters`, the contract's state of a new thread, so
  the back end never names ARM64's words; the plan first said the monitor is cleared at every exit and float
  tracking set to "tracked" (plan sections 4 and 9 corrected).
- A dropped thread's block is reused by the process's next thread, but its instances are not; they stay in
  the process's store until it is dropped. J1's programs start no threads; reusing instances comes with J4's
  threads.
- `Outside` has six methods, not five: `cpu_warning` carries the back end's slow paths apart from the
  kernel's `Host::warn` (plan section 5 corrected). `Execution` has `restored()` (finding of J0's check 4).
- `restored()` of the WebAssembly back end panics naming J2: it is the fork copy's body, built there.
- J1's step 1 was checked by the seeded runs before the commit was made and by the whole-suite diff after it,
  before the push of the milestone (the whole suite takes about 20 minutes at this load).


## Dependencies

Added in J1, licences read from each crate's manifest in the registry on 2026-11-17:
- wcontainers-runtime: none beyond the workspace's own crates.
- wcontainers-wasmtime: wasmtime 50.0.1 (Apache-2.0 WITH LLVM-exception); wasm-encoder 0.263.0 (Apache-2.0
  WITH LLVM-exception OR Apache-2.0 OR MIT) for the hook types the guest interface names; rustix 1.1.6
  (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT) for mappings and the host page size; blake3 1.8.7
  (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception) for the cache key; object 0.41.0 (Apache-2.0 OR
  MIT) for ELF layouts; gimli 0.34.0 (MIT OR Apache-2.0) for the unwind table index; rayon 1.12.0 (MIT OR
  Apache-2.0) for the back end's pool; zerocopy (the workspace's).
- wcontainers-kernel-test: wcontainers-runtime, and, behind the `runtime` feature, wcontainers-wasmtime and
  wcontainers-arm64. The crate stays GPL-2.0-only and unshipped; nothing shipped depends on it.
- Not added yet: mach2 (the macOS fork copy, J2), tar (J3), oci-spec (J3).
