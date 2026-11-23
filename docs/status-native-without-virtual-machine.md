# Native without a virtual machine: status

Plan: docs/native-without-virtual-machine.md. Branch: agents (shared worktree). Scratch:
pyro-parked/wcontainers-scratch/native-without-virtual-machine.

## Where things stand

- W0 (the plan for both forms) is accepted (runs 1 to 6, four independent checks; its history is in git, commit
  544c6658 and before).
- W1 (form P), run 1 (2026-11-23):
  - The fourth check's eight findings are answered in the plan (commit dec511ab): the zygote's inherited
    restartable-sequences registration (4.1, 6), K2 named as W1's need with `differential` and
    `count_at_every_stop` waiting on it by name (3.1, 13), the SVE ABI's promise (3.8), the zygote's own end (4.1,
    7), I's checked address and no second check of pairs (3.2, 7), the probe fault's counter (3.2), the wait window
    from the request word on (3.6), aliasing instruction caches (3.4).
  - Built (crates/executor/implementation/host-process): the start-up probe (page size, the user-space top by
    fixed mappings, ARM64 `CTR_EL0`, SVE after a system call on its own short-lived thread) and the zygote: forked
    with every signal blocked, its code copied to a memfd and mapped at 2^45 over a reservation, assembly for
    ARM64 and x86-64 that unregisters the inherited rseq area and proves none is left (registering its own), clears
    the robust list and exit-clear word, closes every fd but its socket, unmaps everything else, and answers ping,
    fork, kill and reap over a SOCK_SEQPACKET socket. A forked child closes the zygote's socket, sets
    PR_SET_PDEATHSIG, checks its parent and waits on a futex word: the stub's start, with no body yet.
  - Tests (tests/zygote.rs, 8): the zygote moved 200 times between two CPUs answering after each (the fourth
    check's case); a stub moved, stopped and continued 200 times then killed and reaped as SIGKILL; zygote and stub
    map only the zygote's pages (x86's vsyscall page aside) and hold at most one fd (read from /proc by the test
    only); 64 stubs forked, killed, reaped and gone; stubs die with a killed zygote; 16 zygotes from an embedder
    with 4 busy allocating threads; the probe's values. Passed on Graviton4 (load 17 to 36) and the Xeon (load 0.1
    to 0.6), 5 runs each with /dev/kvm denied (`sudo setpriv --clear-groups`, checked by an open that fails), and
    3 more on x86 with it readable. The tests see the fault: with the unregistration and proof skipped on the
    remote copy, 5 of 8 fail on Graviton4 and 3 of 8 on the Xeon (the migration test on both); the stub test alone
    does not catch it on the Xeon, which is why the zygote's own proof matters (a stub inherits the zygote's state).
    The SVE probe reports Dropped on Graviton4 and Kept when its `svc` is replaced by a `nop`.
  - Probe values: Graviton4 user-space top 2^48, `CTR_EL0` 0xb444c004 (DIC and IDC set, L1Ip PIPT); Xeon 8488C
    top 0xfffffffffff000 (five-level paging).
- W1, run 2 (2026-11-23), built and tested on both machines:
  - The stub's start (zygote/*.rs, stub.rs, pages.rs): a fork request carries the stub's socket, its thread-pages
    memfd and its memory's memfd (`SCM_RIGHTS`; the zygote closes its copies after the fork). The stub puts the
    socket and memory at fixed numbers 8 and 9 (`dup3`), maps all 2^15 control pages read-only and register pages
    read-write (at 2^45 + 2^32) and every signal stack (`MAP_NORESERVE`), closes the pages' fd, names thread 0's
    exit-clear word (`set_tid_address`) and writes its id, then installs its filter. Its start is turn 1 of thread 0:
    the filter waits in that control page, and `fork_stub` returns only once the stub has reported it installed (a
    stub that ends first is reaped and its exit status named, `Unavailable::Zygote(FilterRefused)` and so on).
  - Turns: the embedder writes commands and the resume into the control page, stores the answer word (release) and
    wakes it; the stub applies `Map` (memory memfd, shared, fixed), `Protect` and `Unmap` in order, then reports
    `Done` with the count, or `CommandRefused` with the index and error, in its register page, stores the request
    word and wakes it. The runner waits on the request word with the futex's expected value, 100 ms at a time, and
    on each time-out polls the socket for a hang-up. `end` ends the last thread (`exit`) and waits for the hang-up.
  - The filter (filter.rs, classic BPF written here: no filter crate compares two arguments): the architecture,
    then `futex`, `exit`, `exit_group`; `mmap` only of fd 9, shared and fixed, inside [64 KiB, 2^44) with a 64-bit
    end computed with its carry, never executable, at the offset equal to the address (both words); `mprotect`
    inside the guest area, never executable; `munmap` inside the guest area; anything else kills the process
    (`SECCOMP_RET_KILL_PROCESS`). The rest of section 6's list (clone, recvmsg, close, sched_setaffinity,
    rt_sigprocmask, sigaltstack) joins as the routines that make those calls are built.
  - The window cache (windows.rs): 2 MiB windows of any stub's memfd mapped shared at first touch into one
    reservation, 16,384 at most, evicted by the clock algorithm, copies in and out through raw pointers (a stub may
    write the same bytes meanwhile), `forget` for a memfd about to close.
  - Tests: tests/stub.rs (5 and one measurement): 10,000 turns and thread 0's reported id; maps at the guest's own
    addresses at both edges of the guest area seen through /proc/<pid>/mem with the embedder's bytes, protect and
    unmap seen in /proc/<pid>/maps; a refused command reported after those before it; ten host calls outside the
    filter each kill the stub by SIGSYS (control page made writable, register page unmapped, signal stacks
    replaced, executable protect, executable map, offset not the address, offset equal only in its low word, a map
    across 2^44, a map below 64 KiB, a wrapping length); a stub stopped then killed from outside is seen ended
    within 150 ms. tests/windows.rs (3): writes across window edges survive eviction and are the stub's bytes at the
    guest address; three memories in a cache of two never mix; the plan's case of 64 memories of 2^43 bytes, 512
    seeded touches each (`WCHP_SEED=1`, knobs `WCHP_MEMORIES`, `WCHP_TOUCHES`, 32,768 touches run): 16,384
    windows mapped, 16,383 evicted, at most 16,490 host mappings against 105 before the cache, every word read back
    through the cache and every 61st through /proc. tests/zygote.rs (8) as run 1, the stub now started for real.
  - Verdicts: Graviton4, all 16 tests passed 3 times with /dev/kvm denied (load 9.9 to 15.9), clippy with warnings
    denied clean. Xeon: the stub and zygote tests (13) passed with KVM readable at 13:37 UTC (load 0.2); the machine
    stopped answering SSH at 13:39 UTC, so the KVM-denied runs and the window tests are not yet run there.
- Not built yet (W1's bar is not met): everything that runs guest code. The translator's `HostProcess` form and the
  `stop` crate with the slot table's base as a parameter do not exist (asked below), so no executor suite, kernel
  test program, register-isolation test or speed figure can run yet.

## Next

- First: rerun on the Xeon, with /dev/kvm denied, everything run 2 built (the machine was unreachable at the end of
  run 2): the command under "How to run".
- W1, in this order, each tested on both rented machines with /dev/kvm denied: threads (a `Clone` command on the
  control page, the new thread's start on its own signal stack, `exit` through its exit-clear word, index reuse only
  after the word is zero, `THREADS_PER_PROCESS` + 1 refused), with `clone` and the thread's calls added to the
  filter; the stop signal, its handler and its windows (3.6: `sigaltstack`, `rt_sigaction` with `SA_RESTORER` on
  x86-64, the stop blocked around commands, a stop landing in the wait returns to the wait); copy-on-write and the
  adversarial fork tests (3.5); placement (4.3); the zygote's end and re-fork (4.1), in the process-wide host's
  private stand-in. Hand-written ARM64 pieces
  stand in for translated code until the translator's form exists. Then the `Cpu` and `Executor` traits with the
  `with_parts` loop (4.6), the suites, the register-isolation tests and the speed figures.
- I's reservation (W2) must leave out the zygote's pages [2^45, 2^45 + 128 KiB), or the zygote's start is refused
  ("reservation refused") in an embedder that made an in-process host first.

## How to run

- W1's tests, on either rented machine: `arm-run <checkout> --name nwvm -- 'cargo test -p
  wcontainers-executor-host-process -j 12 -- --test-threads 3'` (or `x86-run`). With /dev/kvm denied: build with
  `--no-run`, then `sudo setpriv --reuid=$(id -u) --regid=$(id -g) --clear-groups -- <test binary> --test-threads 3`
  for each test binary: scratch w1/run2/nokvm.sh does it all, each run first proving /dev/kvm cannot be opened
  (send it inline as rt.sh is sent, then `bash nokvm.sh 3`). Knobs: `WCHP_TURNS`, `WCHP_SEED`, `WCHP_MEMORIES`,
  `WCHP_TOUCHES`.
- The fault-visibility runs: scratch keep-rseq.py and keep-rseq-x86.py patch the remote copy (at
  /work/nwvm-w1 on each machine), then the same test command; the next sync restores the tree.

## How to run the measurements

- Scratch w1/run2/: `rt.sh <cpu> <rounds>`, sent inline through `arm-run` / `x86-run --name nwvm` (it writes itself
  to /work/target/nwvm/rt.sh; see arm-rt.out for the command), alternates W0's `/work/nwvm-w0r3/slot futex 10000 21
  <cpu> <cpu> -` with the ignored test `turn_round_trip_on_one_cpu` (knobs `WCHP_CPU`, `WCHP_PER_BATCH`,
  `WCHP_BATCHES`). Outputs arm-rt.out, x86-rt.out, and the KVM-denied verdicts arm-run-nokvm.out.

- Scratch w0/run5/: `run5.sh <tag>` in /work/nwvm-w0r5 (sent as a tar through `arm-run --ssh` / `x86-run --ssh`,
  started detached); builds check5.c with gcc and writes <tag>-check5.out. `./check5 rt <ro|rw> <cpu> <work>
  <per batch> <batches>`, `./check5 alone <cpu> <work> <per batch> <batches>`, `./check5 filter`. The filter lines
  inside the round-trip section of arm5-check5.out and x865-check5.out come from a build with a wrong jump; the
  section "filter (fixed build)" at their end is the valid one. `spcost5.sh` reruns I's check campaign in
  /work/nwvm-w0r5/spcost (needs /work/nwvm-w0/bin/clang; send w0/spcost/src and ported; the fault program needs
  round2-copy too and was not built in run 5).

- Scratch w0/run4/: `run4.sh <tag>` in /work/nwvm-w0r4 (sent as a tar through `arm-run --ssh` / `x86-run --ssh`,
  started detached); builds place.c (and sve.c on ARM64) with gcc and writes <tag>-place.out. `./place fixed
  <kcpu> <gcpu> <work> <calls>`, `./place alone <cpu> <work> <calls>`, `./place rule <cpu,cpu,...> <work> <calls>`.
- Scratch w0/run3/: `run3.sh <tag>` in /work/nwvm-w0r3 on a rented machine (sent as a tar through `arm-run --ssh` /
  `x86-run --ssh`); builds slot.c with gcc and writes <tag>-slot.out. `./slot futex <per batch> <batches> <cpu_k>
  <cpu_g> <spin cpus or ->` (-1: unpinned), `./slot sigmask`, `./slot window`.
- Scratch w0/: `run2.sh <tag>` in /work/nwvm-w0 (probe.c, fallback.c; `SPCOST=1` adds the stack-guard campaign,
  ARM64 only, with the Nix store's clang 21 linked as /work/nwvm-w0/bin/clang). probe.c's futex and zygote waits
  have the lost-wakeup fault: its unpinned runs can hang; use slot.c for the futex slot.
- spcost/ is the cloud ARM64 harness (costs-cloud/arm-recheck/native) with two stack modes added to
  src/instrument2elf.py ("probe", "check") and two variants (X32-p, X32-c) in src/buildlx.sh.

## Measurements

All in plan section 11, with machine and load. Run 2 (w0/run2/; Graviton4 at load 14 to 27, Xeon 8488C at 0.05 to
0.13, both bare metal): futex slot on one CPU 2.29 to 2.37 / 1.89 to 1.96 µs, two CPUs 4.58 to 4.83 / 4.02 to
4.16 µs; zygote child 80 / 95 µs; stop signal to handler 1.68 to 1.78 / 1.75 to 1.90 µs; a fault handled in process
1.31 / 1.33 µs; code area homes in Docker as plan 3.4. Form I's check on Graviton4 (load 5.4 to 13.7, 18 pairs):
X32-e +21.8%, X32-p +21.4%, X32-c +21.7% wall, so f = 0.214. Run 3 (w0/run3/; Graviton4 at load 3.8 to 7.4, Xeon at
0.7 to 1.6): idle one CPU 2.41 to 2.46 / 1.98 to 1.99 µs (two clock reads per round trip included), two CPUs 4.6
to 4.8 / 4.2 to 4.5 µs, unpinned 4.5 to 4.9 / 4.4 to 4.9 µs with every run finished; busy one CPU median 3.7 to
3.9 / 2.7 to 3.1 µs, mean 6.0 to 7.0 / 4.7 to 5.7 µs; busy two CPUs about 2 ms on both; rt_sigprocmask 125 /
87 to 89 ns; 2 MiB window map, touch, unmap 8.5 to 8.6 / 4.8 to 5.2 µs. Crossover with one-CPU placement:
90,000 to 94,000 kernel entries per second on Graviton4 idle. Run 3's busy crossover (31,000 to 36,000) is
withdrawn: it left out that I's work slows on a busy CPU too. Run 4 (w0/run4/; 2026-11-23 11:52 UTC, Graviton4 at
load 7.4 to 8.2, Xeon at 2.7 to 10.4): with 10.8 µs of work between calls c = 2.60 to 2.63 µs on a free CPU
(Graviton4), c_busy 6.11 to 6.23 µs beyond the work alone on a CPU shared with a spinner (work alone slowed 2.00
times); Xeon c 2.9 to 4.5 µs (clock speed varies), c_busy 4.3 to 5.1 µs; CPU share 1.000 free, 0.505 to 0.559
contended; the placement rule's one move 21 to 22 / 118 to 137 µs, then 13.37 to 13.39 / 18.10 to 18.29 µs per
call; SVE p0 and FFR zero after `getppid`. Crossover with work between calls 81,000 to 82,000 (Graviton4, free),
69,000 to 70,000 (every CPU contended); Xeon 25,000 to 52,000 and 38,000 to 66,000. Run 1's outputs and run 2's
unpinned "did not finish" runs give no figure. Run 5 (w0/run5/; 2026-11-23 12:07 to 12:20 UTC, Graviton4 at load
16 to 21, then 5.8 to 6.7 for the campaign, Xeon at 0.1 to 0.4): the stub's answer word on a read-only page, back
to back 2.36 to 2.40 against 2.33 to 2.36 µs read-write (Graviton4), 1.93 to 1.98 against 1.88 to 1.94 µs (Xeon);
with work between calls c = 2.50 to 2.53 / 2.02 to 2.18 µs, no read-only difference; the filter's two-argument
`mmap` rule works on both; I's check X32-p +21.1% wall, X32-e +21.8% (3 rounds of 6 pairs, 9 programs, outputs equal
the plain build's). Crossover with work between calls now 81,000 to 86,000 (Graviton4), 50,000 to 75,000 (Xeon,
idle).

W1 run 2 (2026-11-23 13:38 to 13:39 UTC, w1/run2/): the stub's turn with no commands, embedder thread and stub
pinned to CPU 41, alternating with W0's slot in the same run, three rounds of 21 batches of 10,000, median of batch
means: Graviton4 2,703 to 2,718 ns against the slot's 2,354 to 2,365 ns (load 7.2), Xeon 2,235 to 2,253 ns against
2,019 to 2,040 ns (load 0.3); 1.15 and 1.10 times the slot. (The slot's figure includes two clock reads per round
trip; the turn's does not.) This is the transport alone; the bar's figure is the guest's getppid, which needs
translated code.

W1 run 1 (2026-11-23 12:55 to 13:20 UTC): no timings yet, only the probe's values and the test verdicts above
(Graviton4 at load 13 to 36 while other tracks ran, Xeon at 0.1 to 0.6).

## Asked of other tracks

- Translator and ARM64 (crates/wcontainers-translator, crates/wcontainers-arm64): a form value given to
  `translate_native` and part of the cache key, with variants `Vm`, `HostProcess`, `InProcess`, that changes four
  places only (plan 3.2):
  - `carry.rs` `access` and the instructions that read or write sp. InProcess emits `ubfx u, a, #32, #24; cbnz u,
    fault` and then the access `[base, wA, uxtw]` (or `add t, base, wA, uxtw` first, for an immediate or a pair),
    plus the stack pointer's rules (a) to (f), including one-instruction pieces with sp in the register block.
  - The jump through a register: the slot table at 2^46 instead of `mirror::MIRROR`, reach 2^44, and InProcess's
    `add t, base, wN, uxtw`.
  - `Arm64::call`: `str x30, [page, #saved_link]; bl entry_<code>` instead of `brk`.
  - The replay check: its stop word from the register page instead of `TPIDRRO_EL0`.
  InProcess also needs two more withheld registers.
- Native (VM) track: `stop::resolve`, `stop::guest_state` and the slot arithmetic in
  crates/executor/implementation/hardware-virtualization-inside/stop.rs (which uses `mirror::MIRROR` at lines 10
  and 296) taking the slot table's base as a parameter, and `stop` and `translation` in a crate that does not pull
  aarch64-paging, arm-gic, smccc, the kernel or the runtime; the monitor's kept translations (`Translator`,
  hardware-virtualization/translate.rs) usable from such a crate.
- Kernel: K2 (placement low first, memory size at start) as docs/native.md asks, now W1's need too: until it
  lands W1's bar is equal verdicts, and `differential` and `count_at_every_stop` wait on it by name; several threads running at once
  for `ThreadOrdering::Parallel`; calling the stopper (plan 8 item 3) from `suspend_tasks`, `suspend_threads`,
  signals to a running thread and group stops, if the owner takes it; `ProcessInfo::kernel_entries` if he takes it.
- Every executor track, if the owner takes plan 8 item 3: a `ThreadStopper` of its own.
- x86 track: the x86-64 native output with its `Way` (XN1, XN2), which plan 3.10 takes as it stands.

## For the owner

- Interface changes, as code in plan section 8:
  - `Executor::runner_handle` with `Executor::run_thread`. Today `KernelRunner::run` is `unimplemented!` for want
    of it, for every executor. The alternative is a handle type in `Runner::run`.
  - `ProcessInfo::kernel_entries`, the evaluator's input.
  - `Executor::Stopper` with `ThreadStopper::stop(thread)`. Without it nothing can stop a thread that is running
    on another runner; `Interrupter` has no implementation for the same reason.
  - `StandaloneExecutorHost`: the process-wide part both executors need, made once, making one executor per
    container.
- Decisions in plan section 12:
  - names;
  - I's wrapping check: about 10 points faster, but an access outside the window lands in the process's own
    memory where Linux faults;
  - P's guest can read and write its stub's own pages where Linux faults;
  - PAuth and MTE not offered under P and I;
  - a clock the system publishes;
  - changing the form at an exec;
  - per-thread watches under P, by a process per guest thread;
  - I's runner threads holding off the embedder's SIGCANCEL and SIGSETXID while they run;
  - I's 4 GiB per guest process, against larger windows at one more instruction per access;
  - I's guests sharing one process with no line between them against branch target injection and cache timing;
  - P's defence in depth behind translation: an escaped stub can write pages it lends to fork children before they
    copy them, and shared blocks; close it with a sealed file per fork, or accept;
  - hosts with pages larger than 4 KiB: the ruling of 2026-11-18 followed by split host pages, built at W4 after
    both forms work on 4 KiB hosts.

- Filled in by W1 run 1, where the plan was silent (the fourth check's minor point): when the zygote itself ends,
  every P guest process of that embedder ends as killed by SIGKILL (its stubs die with it), counted "zygote
  ended", and a new zygote is forked from the embedder on the next `memory_new`; if that fork fails, P is
  unavailable from then on. The alternative was to fail every later P container of the process. Plan 4.1.

## Departures from the plan

- W1 run 2: the window cache evicts by the clock algorithm, an approximation of least recently used whose touch is
  one store (plan 4.2 says so now); a stub's start is a turn the embedder waits for (plan 4.1 says so now); the
  filter allows only the calls the stub's routines make so far, and grows with them.
- W1 run 2: until the stop handler is built, a stub keeps the zygote's mask (every signal blocked); SIGKILL and the
  filter's SIGSYS end it regardless.

- The zygote is forked with every signal already blocked by the forking thread (and its mask restored after),
  rather than blocking them as its first act, so no handler of the embedder can run in the child (plan 4.1 says
  so now).

- The brief's "Linux hosts on ARM64 and x86-64 first" is built in two steps: ARM64 at W1 and W2, x86-64 at W3,
  because x86's native output (the x86 track's XN1, XN2) does not exist yet and both forms run only translated code.
  x86's side of both forms is decided now (plan 3.10, from docs/x86-native-output.md).
- The owner's 2026-11-18 ruling that a back end gives the guest 4 KiB pages on whatever the host has is followed
  (plan section 6, split host pages), but built last, at W4: until then both forms refuse to start on a host whose
  pages are larger than 4 KiB (Apple silicon, 64 KiB-page ARM64 kernels), named. Cloud ARM64 Linux machines run
  4 KiB pages by default, so W1 to W3 cover the case the ruling of 2026-11-23 is about first.

## Unsafe code

The crate denies `unsafe_code`; each use carries its own `#[expect(unsafe_code, reason)]`. 25 in the library (23
build on ARM64, 21 on x86-64), 1 in its tests:
- zygote.rs: the zygote's assembly, one module per architecture (it runs with none of the embedder's memory, so
  it refers to nothing outside itself); glibc's `__rseq_offset` and `__rseq_size` (no crate has them); the two
  labels around the assembly; the slice of its code between them; the reservation's `mmap` and `munmap`;
  `sigfillset` with `pthread_sigmask`, then `fork`, then `pthread_sigmask` again (rustix has no signal mask call);
  the jump into the zygote's code in the child, one per architecture.
- probe.rs: `mrs ctr_el0` (no crate wraps it); the fixed `mmap` and `munmap` of the user-space search; the SVE
  probe's assembly (set p0 and FFR, `svc`, store them).
- pages.rs: the thread pages' `mmap` where the host chooses and its `munmap`; the two views of that mapping as
  32-bit and 64-bit atomics (the stub writes the same pages from its own process).
- windows.rs: the reservation's `mmap` and `munmap`; mapping a window, or inaccessible memory, over one slot of
  it; the copy out of a window and the copy into one (raw copies, since a stub may touch the bytes meanwhile).
- tests/zygote.rs: glibc's `__rseq_size`, to show the test has a registration to drop.
Constants no crate defines, named with their source: glibc's rseq signatures (`RSEQ_SIG`), Linux's
`RSEQ_FLAG_UNREGISTER` and the original 32-byte `struct rseq`, `CTR_EL0`'s fields.

## Dependencies and licences

host-process, Linux only: rustix 1.1.6 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT), libc 0.2.191 (MIT OR
Apache-2.0) for the signal mask, `fork` and the wait-status macros, linux-raw-sys 0.12.1 (Apache-2.0 WITH
LLVM-exception OR Apache-2.0 OR MIT) for Linux's numbers in the assembly, wcontainers-abi. All were already in the
lock file. Measurement tools only, never dependencies: gcc 13.3 and clang 21 on the rented
machines.
