# Native without a virtual machine: status

Plan: docs/native-without-virtual-machine.md. Branch: agents (shared worktree). Scratch:
pyro-parked/wcontainers-scratch/native-without-virtual-machine.

## Where things stand

- W0 (the plan for both forms): written in run 1, revised in run 2 for the first independent check, revised in
  run 3 (2026-11-23) for the second check's eleven findings; it waits for its next check. How run 3 answered them:
  - The "unpinned stall" was a lost-wakeup hang in run 2's probe (the guest's futex wait re-read the word instead
    of passing the value it compared; its zygote test had the same fault). Re-measured with a fixed probe
    (w0/run3/slot.c), idle and with spinning programs on the CPUs used. Unpinned does not stall when idle (1 of
    504,000 round trips over 1 ms), but one CPU still halves the round trip, and on busy CPUs one CPU holds a
    3.6 to 3.9 µs median (Graviton4) while two CPUs or the scheduler's own placement wait about 2 ms per round trip.
    So the placement rule stands with new backing, the "move to another CPU" rule is gone (nothing measured needs
    it), and "P is for hosts with idle cores" is withdrawn (plan 4.3, 7, 11). The channel's waits now say they
    pass the compared value.
  - P's slot tables (2^46 bytes each, only three fit in the embedder): every embedder view of a stub's memfds is
    now a 2 MiB window of a bounded cache (16,384 windows, least recently used evicted), measured at 8.5 / 4.8 to
    5.2 µs to map, touch and unmap (plan 4.2, 3.5). This also answers the minor point on large guest
    reservations across many stubs.
  - sp-based register offsets in I: new rule (f) computes the address from sp less the base plus the offset and
    checks it like any other; a structure access post-indexed by a register is split into (a) and (d). SVE and SME
    are not offered to guests (the translator decodes neither), so no SVE form reaches the emitter (plan 3.2).
  - Speculation (Spectre v1): I now uses 4 GiB windows in 8 GiB slots, and the access after the check is
    `[base, wA, uxtw]`, cut to the window by its addressing mode. That is exactly the measured X32 sequence, so f
    is unchanged and needs no barrier. sp's speculative drift is bounded by the 4 GiB unmapped on both sides, and
    jumps through a register are cut the same way. What I does not protect against (branch target injection,
    cache timing between guests on one core) is named in the plan's sections 3.7 and 9 and is an evaluator input.
  - Stopping a running thread: plan 8 item 3 now asks for `Executor::Stopper` / `ThreadStopper::stop(thread)`. It
    used to say "no change", which was wrong.
  - Several I containers in one embedder process: one layout per process, made by an in-process host that makes
    each container's executor and gives out slots from one pool of 2047. P needs no fixed range in the embedder,
    so P and I mix. Construction is asked as `StandaloneExecutorHost` (plan 3.1, 8 item 5).
  - Minor points:
    - The pids limit no longer plays any part: r is gone.
    - memfd refused: data goes to the directory with the most free space, the kernel's side does `fallocate`
      before it writes, and a guest SIGBUS ends the process as an out-of-memory kill would (plan 3.4).
    - Zen 4's one-CPU figure is now its median, 5.14 µs.
    - The mask-change cost is now measured: 125 / 87 to 89 ns.
    - The 1.4 ms figure is gone.
    - Pages a fork child touches after a fork now count in the crossover (plan 3.5, 7).
    - Stub death is learned one way, from the socket hanging up (polled at the runner's 100 ms futex time-out),
      and the zygote always reaps.
    - x86 stubs may now use `arch_prctl(ARCH_SET_GS)`.
- No code yet. The two crates (crates/executor/implementation/host-process and in-process) are not created: W1
  creates host-process, W2 in-process. So the root Cargo.toml's unsafe list has no lines of ours yet.

## Next

- After the check: W1 (form P on ARM64 Linux), plan section 13. It needs the translator's `HostProcess` form and the
  `stop` crate with the slot table's base as a parameter first (below, "Asked of other tracks"); until they exist,
  W1 can build everything of plan sections 3 and 4 that does not run translated code (layout and reservations,
  memfds and the window cache, the zygote and stubs, the channel, placement and runners, copy-on-write and written
  marks, stops, the seccomp filter) and test it with hand-written ARM64 pieces.

## How to run the measurements

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
90,000 to 94,000 kernel entries per second on Graviton4 idle, 31,000 to 36,000 on a busy CPU. Run 1's outputs and
run 2's unpinned "did not finish" runs give no figure.

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
- Kernel: K2 (placement low first, memory size at start) as docs/native.md asks; several threads running at once
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
  - I's guests sharing one process with no line between them against branch target injection and cache timing.

## Departures from the plan

- The brief's "Linux hosts on ARM64 and x86-64 first" is built in two steps: ARM64 at W1 and W2, x86-64 at W3,
  because x86's native output (the x86 track's XN1, XN2) does not exist yet and both forms run only translated code.
  x86's side of both forms is decided now (plan 3.10, from docs/x86-native-output.md).

## Unsafe code

None yet (no crate). Expected uses are listed in plan section 2.

## Dependencies and licences

None added yet. Planned: rustix (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT), and libc (MIT OR Apache-2.0)
only where rustix has no call. Measurement tools only, never dependencies: gcc 13.3 and clang 21 on the rented
machines.
