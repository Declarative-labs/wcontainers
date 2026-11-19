# Status of the parallel-threads plan

2026-11-18. The plan is docs/threads.md. Nothing is built. The plan was written, checked by an independent
agent, corrected for that check's nine findings, checked again, and corrected for the second check's ten
findings (both lists below). It stops here until the owner has ruled on its section 16.

## The second check's findings, and how each was settled

All ten were right, and each was checked against the code before it was settled. None is argued
against. Nothing new was measured for this run: the second checker's two experiments are recorded below
as items 8 and 9, and its sources and logs are in wcontainers-scratch/threads/check2.

1. **A request to stop a thread could be lost** (blocking). Confirmed in the code: the WebAssembly
   executor stores the whole stop word each time round its loop and in the stop hook (wt/lib.rs:2283,
   :1786), and only `StopFlag`'s latch puts a request back (rt/lib.rs:64-74). Settled in 4.5 and section
   5. The stop word becomes a set of bits, each written only by its owner with an atomic or and an
   atomic and. The asked-in bit is taken off only by the thread itself, just before a kernel entry of its
   own, so a request holds until that entry. The held bit of a group stop is a different bit, so neither
   uses the other up. Translated code does not change: it tests the word against zero. The promise is
   part of the stopping handle's interface for every executor, and section 10 says how the native
   executor keeps it. Section 12 has a test that pauses the worker at each place the old loop wrote over
   the word. Cost on a system call: a load and a branch.
2. **A group stop counted threads whose hooks were still touching memory or installing code**
   (blocking). Settled in 4.5, 7.3 and 7.5.
   (a) One stop lock per memory, taken exclusively before any held bit is set; every hook's and every
   `Cpu` call's access to the memory's bytes, table, slots or code lists has it shared; and "held" is
   defined exactly, with a mark per thread. Two 128 bit atomics then go one after the other, and no
   write of the kernel's lands inside one.
   (b) Each code range keeps a count of its drops. A translation reads the count with its bytes and
   installs only if the count has not moved; otherwise it is thrown away and made again.
   Different from the suggested fix in one point: the lock is per memory, not per group. Every change a
   group stop guards is to one memory's own table, code or bytes, and on the WebAssembly executor a
   group in force `Parallel` shows every writable shared page in one place.
   Found while settling it, three more of the same family:
   - the kernel itself makes read, compute, write sequences on guest words that guest code changes too:
     `FUTEX_WAKE_OP`, the owner word of a priority-inheritance lock, the owner-died mark
     (kernel/syscall/futex.rs:682-711, :760-786, :1020, :1163). In a `Parallel` group another thread's
     change in between would be lost. They become a compare and exchange asked of the executor (4.5);
   - `ic ivau` today drops the same line in every memory that shows the block (wt/lib.rs:2008-2010),
     which would change another group's code from a hook. A drop now reaches the caller's group only
     (7.5), with the reason why nothing more is needed;
   - `block_write` goes through every process's records to find who shows a block
     (wt/lib.rs:1413-1414). Each block keeps its own list instead (7.2).
3. **The plan understated what T1 adds to a `Serial` system call.** Settled in 4.2, 4.6, 7.2, section
   12, T1 and decision 7. The lock's cost is now given with both measurements and as depending on the
   work around it. Each other cost is designed away and listed with how: the system is asked about
   entries only if it said at the start that it orders them (the shipped one does not); a `Serial`
   thread takes its process's executor lock once for its whole stay in `Execution::run`, not in every
   hook; the shipped system's per-call state is already atomic (rt/system.rs:256-275); the stop word
   costs a load and a branch. T1's bar is now three builds, one of them with a lock that does nothing,
   so "nothing but the lock was added" is checked and the lock's real cost is measured in place.
   Found while settling it: 4.5 let a page of a `Serial` group gain a permission from another group's
   entry with nobody called in (a file that grows). That broke 4.4 item 5 and the argument for the
   executor's lock above. It is now a call-in.
4. **Names.** `ExecutionConfig`, `RuntimeConfig`, `Process::execution_config()`, `run`,
   `run_until_finished` and `StopReason` are used throughout, and "engine" is gone (the plan says
   "WebAssembly compiler"). This was made as its own commit before the other corrections. The
   executor's own call is always written `Execution::run`, and decision 11 points out that it now shares
   a word with the container's `run`.
5. **`suspend` could not stop a process wherever it may be called.** Settled in sections 5 and 6 and
   decisions 1 and 12. `suspend` never calls threads in. On a handle from `kernel.process(id)` no
   worker is inside `run`; in a hook of a `Serial` group nobody else runs (new item 7 of 4.4, which
   follows from item 5); in a hook of a `Parallel` process it uses the executor's group stop, with
   registers saved in place and a thread that had not yet handed its exit to the kernel put back to the
   instruction that made it. Where `kernel.process(id)` may be called is now a question for the owner
   (decision 12).
6. **Per-thread watches left out the contract's exception for stack frames.** Settled in section 8 and
   T4. A watch that takes a permission from a page at or above a thread's stack pointer makes that
   thread give up its WebAssembly frames, by the x86 plan's own rule for code on a live stack. Checked in
   the translator: a piece entered from the bottom loop knows nothing about its frame
   (tr/full.rs:851-864). T4's program now shares objects on stacks.
7. **The copy of a written shared page must go one way.** Settled in 7.4: in a `Parallel` group it is
   its own copy from the memory to the block's bytes, never `block_write`.
8. **The test clock does not repeat with several workers.** Settled in section 11 and section 12. With
   one worker a run repeats, with any number of processors. With several it repeats only from its
   recording, so stress runs are recorded.
9. **`sched_setaffinity` and one `membarrier` command.** Settled in 4.3, 4.5, section 5 and section 12.
   Each thread keeps a mask; `next` and the idle wake respect it; a thread on a processor its new mask
   leaves out is asked in, and the caller parks until it has come in. `membarrier`'s
   restartable-sequence command asks each running thread in, because a sequence is restarted only in an
   entry of its own thread.
10. **The second checker's wasmtime experiment** is recorded: 7.6, section 14, section 15, and item 9
    below.

## The first check's findings, and how each was settled

All nine were right. None is argued against. Two were settled a little differently from the fix the
check suggested, and two more problems turned up while settling them; both are said below.

1. **Barriers were translated to nothing, and the single translation was unproven for them** (blocking).
   Settled in 7.1, 7.2, 7.3, 7.6, section 9 and section 10. ARM64's `dmb` and `dsb` and x86's `mfence`
   become `atomic.fence`, behind one byte per process that says whether another thread of the group may
   be running. The same byte keeps 128 bit atomics inline in a `Serial` run. Measured: the byte's test
   costs 0.12 to 0.8 ns a barrier, the fence without it 0.7 to 19 ns. A `Serial` run does get slower by
   that test, so it is put to the owner (decision 7), and section 12 has a bar for it.
   Found while settling it: the native output drops barriers too (native.md lines 350-362), and its
   exclusive load has a fix-up that assumes one thread (lines 492-496). Section 10 said "atomics need
   nothing" there, which was wrong for both. Section 10 now asks both of the native track.
2. **Groups that are not joined still reached each other's memory** (blocking). Settled in section 3, 4.4
   item 5, 4.5 and 4.6. (a) Every memory that shows a block which some shared mapping may write is joined,
   private mappings included. (b) Two rules: a thread of a `Serial` group stops only at an entry of its
   own, and a thread ends only in an entry of its own. So the kernel calls a `Serial` group in before it
   touches its bytes, permissions or code from outside. (c) "May write" is what `mprotect` may give
   (`Mapping::allowed`).
   Different from the suggested fix in one point: for a group in force `Parallel` reached from outside, a
   permission lowering or code drop uses the executor's group stop, not `thread_stop` and parking. Such a
   group promises nothing about where its threads stop, and the group stop is cheaper.
   Found while settling it: a fatal signal from outside ended the target on the spot
   (kernel/signal.rs:481-485), which writes into the target's memory under its running thread. It now
   waits for that thread's entry (4.5).
3. **`next` was not a sequenced entry, and a host thread was a processor** (blocking). Settled in 4.3,
   4.6 and section 5. A processor is the kernel's; a worker names the processor it asks for. `next` is an
   entry when it hands a thread out. The kernel no longer hands out numbers: it asks the system whether
   an entry may take effect now, and an entry told "later" never holds a host thread (a returned thread
   is kept aside and its worker freed; a hand-out answers `Idle`). A replay calls `run` in the log's
   order, with one worker or many. 4.6 says why it cannot get stuck.
4. **Decision 8 reopened a ruled question.** Removed. Section 2 states the ruling (Linux's
   `ptrace_may_access` line; `cmdline` is not inspection, `environ`, `maps`, `auxv` and the address fields
   of `stat` are). 4.5 keeps only what this plan adds: `cmdline` is still a read of another group's
   memory, so a `Serial` group is called in for it.
5. **Form B was weighed in replaced words.** Section 6 is rewritten around `suspend`, `resume` and
   `SuspendedProcess`. The two points that assumed a conversion are withdrawn. A group of several
   processes has its own paragraph. The strictest-wins rule is now decision 2, with what
   `Process::thread_ordering()` answers.
6. **The interface list was incomplete and idle could lose a wake.** Section 5 and decision 9. The list
   now has the admission service, `thread_stop`, the hook that wakes an idle processor, the processor and
   thread told to `turn`, `ran`, `arrived` and `idle`, and threads made ready by any entry. The idle mark
   is made under the lock and the wake is kept if it comes early. A thread now always ends in its own
   entry, so none leaves `Execution::run` without its exit.
7. **No bar kept `Serial` from getting slower in T1.** Section 12 has three `Serial` measurements and T1's
   bar names them. Measured for it: an uncontended lock is not visible on this Mac and costs 7 to 9 ns on
   the x86 laptop. So the loop takes the lock once a system call, not twice (4.2), and the rest is put to
   the owner (decision 7).
8. **Two costs of shared-page locks and watches were left out.** (1) 7.4: in a `Parallel` group a written
   shared page is no longer locked again at each turn's end; it is copied when the block's bytes are
   asked for. The group stop at every turn end is gone. (2) Section 8: kernel accesses for a thread are
   checked against that thread's watches; a kernel write from outside the process needs the hook to name
   no thread (decision 10).
9. **Small accuracy problems.** Fixed: 155 lines in 29 files; the "3.2 ms for 48 MB" figure is gone with
   the argument it served; the kernel as a WebAssembly module with several callers (4.2, 7.6);
   `Config::shared_memory` named in 7.2 and T2's first task.

## What was read

- The brief and the interface it points to: wcontainers-scratch/orchestrator/brief-threads.md to its end,
  and next-embedder-interface.md with its later sections taken over its earlier ones.
- The results the owner has seen: orchestrator/dsm-survey/results.json (the 2008 system's cost of a
  recorded page hand-over), orchestrator/optimistic-replay-results.json (what ordering only atomics and
  kernel entries misses, what a single sequence costs, and which programs rely on barriers),
  wcontainers-scratch/ordering-cost/result.json (atomic against plain accesses, shared against unshared
  memories).
- This branch at 77e50ddc: docs/runtime.md whole; the kernel's entry points, scheduler, processes and
  signals (kernel lib.rs, task.rs, schedule.rs, process.rs, signal.rs, parts of memory.rs, shared.rs,
  fs/mod.rs, loader.rs, container.rs, syscall/system.rs, syscall/rseq.rs, fs/proc.rs); the abi; the
  runtime crate whole; the WebAssembly executor's types, `Execution::run`, `flush`, `block_write`,
  `settle` and outline (wasmtime lib.rs); the translator's contract, its memory imports, stop check,
  permission check and lookup; ARM64's atomics, exclusives and how barriers are decoded (arm64/memory.rs,
  decode.rs, instruction.rs).
- Other branches: docs/native.md at origin/phase-2-native ab448e58, sections 3.4 (the routes for nothing,
  aligned and exclusive forms), 5.1, 5.3, 5.5 and 11; docs/phase-2-guest-cpu-and-x86.md at
  origin/phase-2-x86 0de30f18, its lines on lock prefixes, on protecting code pages and its section 8. It
  does not name `mfence`.
- wasmtime 50.0.1's source in the cargo registry, for how a shared memory is made and switched on
  (src/runtime/vm/memory/shared_memory.rs:34-62, src/runtime/vm/memory.rs:251-256,
  src/runtime/instance.rs:554, src/config.rs above `shared_memory`), and its compiler's rule for
  `atomic.fence` (cranelift-codegen 0.137.1, src/isa/aarch64/inst/emit.rs:1809-1811,
  src/isa/x64/lower.isle:3425-3426).

- For the second check's findings, on this branch: the WebAssembly executor's loop, stop hook,
  `carry_on` and `leave`, `load_code`, `flush` and `block_write` (wt/lib.rs:2235-2340, :1780-1820,
  :1719-1740, :1885-1981, :1341-1433); `StopFlag` and the loop (rt/lib.rs:33-74, :190-300); the local
  system (rt/system.rs); `pick`, `came_back` and the tick (kernel/task.rs); `sched_setaffinity`
  (kernel/syscall/system.rs:161-190); `membarrier` (kernel/syscall/rseq.rs); `rseq_preempted`
  (kernel/signal.rs:648-665); the futex calls that change a guest word (kernel/syscall/futex.rs); the
  translator's contract and how a piece learns about its frame (tr/contract.rs, tr/full.rs:730-864,
  docs/phase-1-translator.md "Checks the translator drops"); the x86 plan's lines 1875-1915. The brief
  and the interface notes were read to their ends again: the brief's last section corrects `Settings`
  to `Config` and names `run`, and the notes' last sections give `run_until_finished` and `StopReason`.

## What the first agent left

No docs/threads.md was in the checkout, committed or not. In wcontainers-scratch/threads it left a clone of
this branch (`exp/`) with an experiment half made: an environment variable `WC_SHARED_GUEST=1` that makes
the translator import the guest memory shared and the translator's test runner supply a wasmtime shared
memory, a test `tests/spin_hoist.rs`, a 774 MB build, and copies of the native and x86 plans. The
experiment code was sound and its build was complete, so it was used as it was, with two small additions
to the spin test (it writes its modules as files; the declared maximum can be set).

## What was measured

Machines:
- this Mac: Apple M5 Max, 18 cores, macOS 27.2, with other tracks running. Load is `sysctl -n vm.loadavg`,
  the 1 minute figure;
- the x86 laptop: Intel Core i7-8750H, 12 threads, Linux 7.3.4, nearly idle. Load is `/proc/loadavg`, the
  1 minute figure. Everything ran under `taskset` on cores 4, 5, 6, 10 and 11; cores 2 and 8 were not
  used. Its files are in ~/threads-plan. No system package was installed: Node 24.22.3 and wasmtime
  49.0.2 were already in the home folder from earlier measurements.

Everything is under wcontainers-scratch/threads. Build output was deleted afterwards on both machines.

### First run

**1. A shared guest memory against an unshared one, single-threaded.** `shared-cost.sh`, run inside `nix
develop <this checkout> -c`. It runs the translator's benchmark test (`measure_benchmarks`,
crates/wcontainers-translator-test/tests/benchmarks.rs, `programs/bench.c` at -O2, full look-ahead, every
check, wasmtime 50.0.1 only) once per workload per setting under `/usr/bin/time -l`, alternating the two
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
  memory, one store per thread, anything with two threads, or any program with a barrier or a 128 bit
  atomic in it.

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
  of every version of each WebAssembly compiler (docs/threads.md section 12).

**3. V8 refuses a declared maximum above its limit.** The first Node run of experiment 2 failed to
compile: "maximum memory size (281474976710656 pages) is larger than implementation limit (262144 pages)".
The module imported a shared 64 bit memory with a declared maximum of 2^48 pages. With a declared maximum
of 262,144 pages it compiled and ran. wasmtime accepted both.

### By the plan's checker

**4. The message-passing test.** check1/mp.mjs, logs mp-node.log and mp-node-2.log; this Mac, Node 24.14.1,
load 99 to 107. Two workers, plain 32 bit accesses on a shared memory. The flag was seen with stale data
84 times in 1,500,000 rounds and 64 times in 2,000,000. With `atomic.fence` between the two stores and
between the two loads: 0 times in 3,500,000. In one thread, a fence between two plain stores took a round
from about 0.35 ns to about 1.6 ns.

### Second run, for the corrections

**5. A barrier in a single-threaded run, in three forms.** barrier/bench.wat, assembled with `wasm-tools
parse` from the checkout's `nix develop`. A module with a shared 64 bit guest memory and a shared 32 bit
state memory, as the translator's contract has them. A round stores a word, has the barrier, and loads
another word. Four exported loops: `none` (the barrier is nothing, as today); `fence` (`atomic.fence`);
`flag0` and `flag1` (a byte of the state memory is loaded, and `atomic.fence` runs when it is not zero;
the byte is 0, then 1). Nanoseconds a round, best of five.
- This Mac, Node 24.14.1, `node bench.mjs bench.wasm 200000000 5` (barrier/bench-node-mac.log, load 90 to
  91): none 0.463, fence 6.590, flag0 0.585, flag1 6.907.
- This Mac, wasmtime 49.0.2's command line program from `nix develop`, `bench-wasmtime.sh`, 500,000,000
  rounds, each time less the time of a run of no rounds (barrier/bench-wasmtime-mac.log, load 90 to 91):
  none 0.975, fence 1.672, flag0 1.146, flag1 1.739.
- The laptop, Node 24.22.3, core 4 (barrier/bench-node-x86.log, load 0.2 to 0.9): none 1.051, fence
  17.047, flag0 1.233, flag1 17.577.
- The laptop, wasmtime 49.0.2, core 4 (barrier/bench-wasmtime-x86.log, load 0.9 to 1.2): none 1.763,
  fence 20.343, flag0 2.563, flag1 20.121.
- So the fence costs 0.7 and 6.1 ns a barrier on this Mac and 18.6 and 16.0 ns on the laptop. The byte's
  test costs 0.17 and 0.12 ns on this Mac and 0.80 and 0.18 ns on the laptop.
- wasmtime and Node differ on this Mac for the fence (0.7 against 6.1 ns). wasmtime's loop is slower to
  begin with (it calls a function for the address and checks bounds); why the fence then costs less there
  was not looked into.
- wasmtime's compiled output on this Mac was disassembled (`wasmtime compile`, then `objdump -d`): the
  fence is `dmb ish`, and the tested form is `ldrb`, `cbz` around `dmb ish`.
- Not covered: wasmtime 50.0.1 (the nix shell has 49.0.2; the compiler's rule for the fence was read in
  50.0.1's own compiler source instead); Chrome; the translator's real output, where the byte's place and
  the register holding its base may differ.

**6. The store-then-load test.** barrier/litmus.wat and litmus.mjs, in Node with two workers on one shared
memory. Each round has its own two words. The threads meet at an atomic counter, then thread 0 stores to
x and loads y, and thread 1 stores to y and loads x. "Both loads saw 0" is forbidden when a full barrier
stands between each thread's store and its load. Four forms of the store: plain; plain then
`atomic.fence`; an atomic store; an atomic exchange. `node litmus.mjs litmus.wasm 200000 10`, so 2,000,000
rounds of each.
- This Mac, Node 24.14.1 (barrier/litmus-node-mac.log, load 89 to 91): 371,914; 0; 0; 0.
- The laptop, Node 24.22.3, cores 4, 5, 6, 10, 11 (barrier/litmus-node-x86.log, load 1.2): 134,938; 0; 0;
  0.
- So on both hosts plain accesses show the outcome, which is why a guest's barrier cannot be nothing, and
  `atomic.fence` forbids it. An atomic store or exchange in the barrier's place forbade it too, in V8.
- Not covered: wasmtime with two threads (its command line program runs one); a test through the
  translator.

**7. An uncontended lock.** lock/src/main.rs, a small Rust program with `parking_lot` 0.12.5 through
`lock_api` 0.4.14, release build. One thread; a round is two small pieces of work; a `parking_lot` raw
lock is taken and let go around none, one or both. Nanoseconds a round, best of five, 300,000,000 rounds.
- This Mac, built with `cargo build --release --offline -j 3` under `nix develop` (lock/lock-mac.log, load
  83 to 96): 4.863, 5.141, 4.977. The lock cannot be told from the noise.
- The laptop, core 4 (lock/lock-x86.log, load 0.6 to 1.7): 5.514, 12.596, 21.540. So 7.1 and 8.9 ns a
  lock.
- Not covered: the lock inside a real system call, where the work around it is different.

### By the plan's second checker

**8. An uncontended lock, with two kinds of work around it.** check2/lock/lock.c, a C program: the lock is
`parking_lot`'s fast path written out, a compare and swap to take it (acquire) and one to let it go
(release). "Register work" is a dependent multiply that touches no memory. "Memory work" walks a chain
of eight loads in a 64 KiB table and stores once. Best of five, 100,000,000 rounds, nanoseconds a round
without the lock and with it.
- This Mac (check2/lock/lock-mac.log, load 53 to 71): register work 0.950 and 4.523; memory work 8.421
  and 8.414.
- The laptop, core 4 (check2/lock/lock-x86.log, load 5.5 to 5.7): register work 2.667 and 14.041; memory
  work 16.852 and 17.485.
- So a lock costs 0 to 3.6 ns on this Mac and 0.6 to 11.4 ns on the laptop, by the work around it. Item
  7's figure for this Mac was the loop's, not the lock's. What it costs inside a real system call is
  measured only by T1.

**9. The store-then-load test in wasmtime with two host threads.** check2/host (a Rust program on
wasmtime 50.0.1 as the repository pins it, `Config::wasm_threads` and `Config::shared_memory`), with
check2/host/src/sb.wat. Two host threads, each with its own `Store` and `Instance`, on one wasmtime
`SharedMemory`. Twenty runs of 100,000 rounds for each form; the count is rounds in which both loads saw
0.
- This Mac (check2/sb-mac.log, load 102 to 106): plain accesses 30,199; `atomic.fence` 0; the byte
  tested and zero 30,425; the byte tested and set 0; a compare and exchange then a plain load 0; an
  atomic store then an atomic load 0.
- The laptop, cores 4 and 10 (check2/sb-x86.log, load 8.8 to 11.3): 143,587; 0; 39,671; 0; 0; 0.
- So item 6's "not covered: wasmtime with two threads" is now covered for this test. Build output was
  deleted on both machines.

## What was not measured

Listed in docs/threads.md section 15. The ones that matter most to the plan's decisions:
- how long processors would wait for the kernel's one lock in a real build: it needs T1 built;
- what T1 adds to a system call end to end; only the lock by itself was measured (items 7 and 8), and
  it costs between nothing and 11.4 ns by the work around it;
- how often real programs run barriers and 128 bit atomics: the cost of each is measured, their number
  is not;
- the runtime's host-supplied mapping as a wasmtime shared memory: read in wasmtime's source, not run;
- the ordering tests as guest programs through the translator: only hand-written tests were run (items
  4, 6 and 9);
- what the stop lock costs the kernel's accesses to a `Parallel` memory, and how often a translation is
  thrown away because its code was dropped meanwhile;
- a group stop with several threads running, and a page hand-over through per-thread watches, are both
  estimated from parts measured apart (the stop flag's latency in Chrome from docs/phase-1-translator.md,
  leaving `Execution::run` and coming back from docs/runtime.md section 10).

## What the plan asks of other tracks

- The x86 track: lock prefixes and `xchg` with memory as WebAssembly atomic operations, with a hook for
  misaligned ones; `cmpxchg16b` as a 128 bit pair behind the byte; `mfence` as the tested fence; all
  measured on `Serial` runs before they land (docs/threads.md 7.3). Its rule for code on a live stack
  frame (its lines 1898-1905) is used again for watches (section 8), so it is built once.
- The native track: `dmb`, `dsb` and `mfence` copied instead of dropped; the exclusive load's fix-up made
  safe for threads in parallel; an interrupt from one virtual CPU to another inside the machine measured;
  a request to stop kept until the thread's next entry; a stop lock per memory and the count of drops
  (section 10). Nothing before its own thread milestone.
- The translator track: the progress count, which exact positions at full speed already wait for; the
  byte's place in the contract. The stop word's meaning to translated code does not change.
- The kernel track's K7: the kernel as a WebAssembly module built for a shared memory, for more than one
  caller (4.2).
- The embedder interface: `ExecutionConfig` at the container's start; `suspend` and `resume`; memory
  watches before T4; who shares memory with whom; the hook's thread made optional (decision 10); where
  `kernel.process(id)` may be called and how `suspend` stops a process (sections 5 and 6, decision 12).
- The runtime track: the renames to `RuntimeConfig`, `TranslatorConfig`, `run` and `run_until_finished`
  in the code, which this plan already uses.
