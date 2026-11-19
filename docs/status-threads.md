# Status of the parallel-threads plan

2026-11-18. The plan is docs/threads.md. Nothing is built. It stops here until the owner has ruled on its
section 16 and an independent agent has checked it.

## What was read

- The brief and the interface it points to: wcontainers-scratch/orchestrator/brief-threads.md to its end,
  and next-embedder-interface.md with its later sections taken over its earlier ones.
- The results the owner has seen: orchestrator/dsm-survey/results.json (the 2008 system's cost of a
  recorded page handover), orchestrator/optimistic-replay-results.json (what ordering only atomics and
  kernel entries misses, and what a single sequence costs), wcontainers-scratch/ordering-cost/result.json
  (atomic against plain accesses, shared against unshared memories).
- This branch at 77e50ddc: docs/runtime.md whole; the kernel's entry points, scheduler, processes and
  signals (kernel lib.rs, task.rs, schedule.rs, process.rs, signal.rs, parts of memory.rs, shared.rs,
  loader.rs, container.rs, syscall/system.rs, syscall/rseq.rs, fs/proc.rs); the abi; the runtime crate
  whole; the WebAssembly executor's types, `run` and outline (wasmtime lib.rs); the translator's contract,
  its memory imports, stop check, permission check and lookup; ARM64's atomics and exclusives
  (arm64/memory.rs).
- Other branches: docs/native.md at origin/phase-2-native ab448e58, sections 5.1, 5.3, 5.5 and 11;
  docs/phase-2-guest-cpu-and-x86.md at origin/phase-2-x86 0de30f18, its lines on lock prefixes, on
  protecting code pages and its section 8.
- wasmtime 50.0.1's source in the cargo registry, for how a shared memory is made
  (src/runtime/vm/memory/shared_memory.rs:34-50, src/runtime/vm/memory.rs:251-256,
  src/runtime/instance.rs:554).

## What the earlier agent left

No docs/threads.md was in the checkout, committed or not. In wcontainers-scratch/threads it left a clone of
this branch (`exp/`) with an experiment half made: an environment variable `WC_SHARED_GUEST=1` that makes
the translator import the guest memory shared and the translator's test runner supply a wasmtime shared
memory, a test `tests/spin_hoist.rs`, a 774 MB build, and copies of the native and x86 plans. The
experiment code was sound and its build was complete, so it was used as it was, with two small additions
to the spin test (it writes its modules as files; the declared maximum can be set).

## What was measured

All on this Mac: Apple M5 Max, 18 cores, macOS 27.2, wasmtime 50.0.1, Node 24.14.1, release builds, with
other tracks running. Load is `sysctl -n vm.loadavg`, the 1 minute figure. Everything is under
wcontainers-scratch/threads; build output was deleted afterwards (the `target` folder and the compiled
`.cwasm` files). The x86 laptop was not used.

**1. A shared guest memory against an unshared one, single-threaded.** `shared-cost.sh`, run inside `nix
develop <this checkout> -c`. It runs the translator's benchmark test (`measure_benchmarks`,
crates/wcontainers-translator-test/tests/benchmarks.rs, `programs/bench.c` at -O2, full look-ahead, every
check, wasmtime only) once per workload per setting under `/usr/bin/time -l`, alternating the two
settings. Every run checked the program's output against the native run's.
- Three rounds with one run in each process: logs/shared-cost.log, load 49 to 64.
- Two rounds with three runs in each process: logs/shared-cost-runs3.log, load 72 to 96.
- The whole process's instructions include fixed work that is not the translated run (9.3 to 9.4 thousand
  million instructions in every case). The difference between the three-run and one-run processes, halved,
  is one translation, compile and run. In thousand millions of instructions, unshared then shared: fib
  81.602 and 81.580 (-0.03%); sieve 14.038 and 14.049 (+0.07%); hash 7.959 and 7.967 (+0.10%); sort 31.710
  and 31.733 (+0.07%); interpret 22.896 and 22.871 (-0.11%); matrix 7.967 and 7.960 (-0.09%); walk 16.790
  and 16.763 (-0.16%); search 6.947 and 6.969 (+0.33%).
- The two rounds of one setting differed by up to 0.39% (search, shared: 6.956 and 6.983) and 0.19% (walk,
  unshared). So no difference is shown.
- Whole-process counts from the one-run rounds, mean of three, shared against unshared: fib -0.02%, sieve
  -0.23%, hash -0.12%, sort -0.10%, interpret +0.16%, matrix -0.04%, walk +0.04%, search -0.23%.
- Best time of three, unshared then shared, in seconds: fib 2.806 and 2.476; sieve 0.706 and 0.681; hash
  0.591 and 0.589; sort 1.322 and 1.308; interpret 0.623 and 0.653; matrix 0.274 and 0.302; walk 2.733 and
  2.576; search 0.258 and 0.260. Times at this load vary more than these differences.
- What this does and does not cover: the import's type and a wasmtime `SharedMemory` made by wasmtime
  itself, in the translator's runner. It does not cover the runtime's host-supplied mapping as a shared
  memory, one store per thread, or anything with two threads.

**2. A loop of plain loads sees another thread's store.** `tests/spin_hoist.rs` in the scratch clone, and
`spin-node.mjs`. One thread runs a loop that counts and loads a 64 bit word from a shared 64 bit memory
until it is not zero. Another thread stores 1 to the word after 200 ms (wasmtime) or 300 ms (Node). Three
loops: a bare plain load; a plain load with the translator's stop check in the loop (a plain load of a
flag in a second shared memory and a call when raised); an atomic load.
- wasmtime 50.0.1 (logs/spin-wasmtime.log, load 69): all three saw the store, after 755 million, 439
  million and 465 million rounds, at 201 to 205 ms.
- Node 24.14.1 (logs/spin-node.log, load 62 to 64): all three saw it with the default compilers, with
  `--no-liftoff` (the optimising compiler alone) and with `--liftoff-only` (the baseline compiler alone).
- This shows what these versions do. WebAssembly's rules do not promise it, so the plan makes it a test
  of every engine version (docs/threads.md section 12).

**3. V8 refuses a declared maximum above its limit.** The first Node run of experiment 2 failed to
compile: "maximum memory size (281474976710656 pages) is larger than implementation limit (262144 pages)".
The module imported a shared 64 bit memory with a declared maximum of 2^48 pages. With a declared maximum
of 262,144 pages it compiled and ran. wasmtime accepted both.

## What was not measured

Listed in docs/threads.md section 15. The ones that matter most to the plan's decisions:
- how long processors would wait for the kernel's one lock in a real build: it needs T1 built;
- the runtime's host-supplied mapping as a wasmtime shared memory: read in wasmtime's source, not run;
- no ordering test was run, so that an x86 guest's plain accesses would misbehave on this Mac rests on the
  architecture's manual;
- a group stop with several threads running, and a page handover through per-thread watches, are both
  estimated from parts measured apart (the stop flag's latency in Chrome from docs/phase-1-translator.md,
  leaving `run` and coming back from docs/runtime.md section 10).

## What the plan asks of other tracks

- The x86 track: lock prefixes and `xchg` with memory as WebAssembly atomic operations, with a hook for
  misaligned ones and for `cmpxchg16b`, measured on `Serial` runs before it lands (docs/threads.md 7.3).
- The native track: nothing before its own thread milestone; an interrupt from one virtual CPU to another
  inside the machine measured there (section 10).
- The translator track: the progress count, which exact positions at full speed already wait for.
- The embedder interface: `Start` carrying the container's ordering; memory watches before T4.
