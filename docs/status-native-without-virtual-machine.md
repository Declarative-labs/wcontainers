# Native without a virtual machine: status

Plan: docs/native-without-virtual-machine.md. Branch: agents (shared worktree). Scratch:
pyro-parked/wcontainers-scratch/native-without-virtual-machine.

## Where things stand

- W0 (the plan for both forms): written in run 1, revised in run 2 (2026-11-23) to answer every finding of its first
  independent check; it waits for its next check. How each finding was answered:
  - Form I's stack pointer could leave its slot: plan 3.2 item 1 now keeps the guest's sp inside its slot at every
    instruction boundary (unchanged immediate accesses inside 64 KiB bands no one maps, writebacks covered by their
    own access, a one-instruction probe after each small `add`/`sub sp`, a full check on every other write, and a
    named one-instruction-at-a-time slow path when a guest really moves sp outside). Its cost is measured (plan 11)
    and is in W2's speed bar and the crossover's f.
  - Slot count: 255 slots at r = 36 (was 4096); the kernel's views moved out of the guest area; r chosen from the
    container's pids limit (plan 3.1).
  - The ARM64 unpinned round trip did not reproduce: re-measured (plan 11): unpinned runs stall for milliseconds per
    round trip on both machines, idle or not, so P now pins each runner and the guest thread it drives to one CPU
    (plan 4.3), whose figure does reproduce (2.29 µs Graviton4, 1.89 to 1.96 µs Xeon). W1's bar is now against the
    bare mechanism measured in the same run.
  - Fork while a sibling runs: the parent's write protection is applied synchronously before `memory_duplicate`
    returns, and an owner/borrower rule with one copy per event settles chains (plan 3.5, 4.2, 4.4).
  - Slot-table pages in P: mapped page by page by a stub command that the stub's filter allows in exactly that range
    and only for the slot table's fd (plan 3.1, 3.7).
  - Runners and the kernel's executor: `Executor::runner_handle` plus `run_thread` with `slice_nanoseconds`
    (plan 8.1); the kernel runs one thread at a time today, which W1 and W2 accept (plan 3.9).
  - x86-64: plan 3.10 takes the x86 plan's decisions (two-level mirror, GS/FS, 2^32 window, `membarrier`); built at
    W3; recorded below as a departure.
  - memfd refused: data in /dev/shm, code in an `O_TMPFILE` file in an exec-allowed directory found by the probe;
    measured in Docker (plan 3.4, 11).
  - Signals: `SA_NODEFER` with only the stop signal masked, no handler returns through a guest-writable frame, every
    window named (plan 3.6); I's handling of the embedder's signals (plan 3.7).
  - process_vm_readv claim corrected; `stop.rs`'s `MIRROR` asked as a parameter (plan 3.3, 10); f labelled by
    layout with its omissions named (plan 1, 7); every figure now from one clean run (plan 11); x86 vector state
    returned by `XRSTOR` from an initial image (plan 3.8); P saves the whole register file at every exit (plan 3.3,
    4.5); the stub is forked, not exec'd, with the zygote removing the embedder's memory and `PR_SET_PDEATHSIG` for
    orphans (plan 4.1); per-thread watches left open for P by a process per thread (plan 8); W1's busybox item has a
    bar against a model, and a fork child's code area is decided (plan 3.4, 13).
- No code yet. The two crates (crates/executor/implementation/host-process and in-process) are not created: W1
  creates host-process, W2 in-process. So the root Cargo.toml's unsafe list has no lines of ours yet.

## Next

- After the check: W1 (form P on ARM64 Linux), plan section 13. It needs the translator's `HostProcess` form and the
  `stop` crate with the slot table's base as a parameter first (below, "Asked of other tracks"); until they exist,
  W1 can build everything of plan sections 3 and 4 that does not run translated code (layout and reservations,
  memfds and views, the zygote and stubs, the channel, placement and runners, copy-on-write and written marks,
  stops, the seccomp filter) and test it with hand-written ARM64 pieces.

## How to run the measurements

- Scratch w0/: `run2.sh <tag>` on a rented machine in /work/nwvm-w0 (sent there as a tar through `arm-run --ssh` /
  `x86-run --ssh`; `SPCOST=1` adds the stack-guard campaign, ARM64 only; it needs the Nix store's clang 21 linked
  as /work/nwvm-w0/bin/clang). It builds probe.c and fallback.c with gcc, picks the least busy CPUs, and writes
  <tag>-probe.out, <tag>-futex.out, <tag>-fallback.out and spcost/results/. `./probe 2000 <cpu> <cpu>` (-1 for
  unpinned) runs the futex slot alone; `CPU_A`/`CPU_B` choose the two CPUs of the full probe.
- spcost/ is the cloud ARM64 harness (costs-cloud/arm-recheck/native) with two stack modes added to
  src/instrument2elf.py ("probe", "check") and two variants (X32-p, X32-c) in src/buildlx.sh.

## Measurements

All in plan section 11, with machine and load, from run 2 only (w0/run2/ in scratch). In short, Graviton4 at load
14 to 27 / Xeon 8488C at load 0.05 to 0.13, both bare metal: futex slot on one CPU 2.29 to 2.37 / 1.89 to 1.96 µs,
on two CPUs 4.58 to 4.83 / 4.02 to 4.16 µs, unpinned 4.6 / 4.4 to 4.6 µs when it finishes but over 2 ms per round
trip in 3 of 4 / 1 of 4 runs; zygote child 80 / 95 µs; stop signal to handler 1.68 to 1.78 / 1.75 to 1.90 µs; a
fault handled in process 1.31 / 1.33 µs; code area homes in Docker as plan 3.4. Form I's check on Graviton4 (load
5.4 to 13.7, 18 pairs): X32-e +21.8%, X32-p (with the stack pointer's guard) +21.4%, X32-c +21.7% wall, so f =
0.214; the crossover 90,000 to 94,000 kernel entries per second on Graviton4 with P's one-CPU placement. Run 1's outputs (probe-*.out, probe2-*.out,
probe3-*.out, probe4-arm.out) are kept in w0/ but no figure comes from them: in run 1 a spinning thread of the stop
test was left on CPU 3 and slowed the futex figures, the second run hung on a test bug, and its "unpinned 2.40 µs on
Graviton4" did not reproduce.

## Asked of other tracks

- Translator and ARM64 (crates/wcontainers-translator, crates/wcontainers-arm64): a form value given to
  `translate_native` and part of the cache key, with variants `Vm`, `HostProcess`, `InProcess`, that changes four
  places only (plan 3.2): `carry.rs` `access` and the instructions that read or write sp (InProcess: `ubfx u, a,
  #r, #(56 - r); cbnz u, fault` then the access against a base register; the stack pointer's rules (a) to (e),
  including one-instruction pieces with sp in the register block); the jump through a register (slot table at 2^46
  instead of `mirror::MIRROR`, reach 2^44, and InProcess's base add); `Arm64::call` (`str x30, [page, #saved_link];
  bl entry_<code>` instead of `brk`); the replay check's stop word from the register page instead of `TPIDRRO_EL0`.
  Two more withheld registers for InProcess.
- Native (VM) track: `stop::resolve`, `stop::guest_state` and the slot arithmetic in
  crates/executor/implementation/hardware-virtualization-inside/stop.rs (which uses `mirror::MIRROR` at lines 10
  and 296) taking the slot table's base as a parameter, and `stop` and `translation` in a crate that does not pull
  aarch64-paging, arm-gic, smccc, the kernel or the runtime; the monitor's kept translations (`Translator`,
  hardware-virtualization/translate.rs) usable from such a crate.
- Kernel: K2 (placement low first, memory size at start) as docs/native.md asks; several threads running at once
  for `ThreadOrdering::Parallel`; `ProcessInfo::kernel_entries` if the owner takes it.
- x86 track: the x86-64 native output with its `Way` (XN1, XN2), which plan 3.10 takes as it stands.

## For the owner

- Two interface additions, as code in plan section 8: `Executor::runner_handle` with `Executor::run_thread` (today
  `KernelRunner::run` is `unimplemented!` for want of it, for every executor; or a handle type in `Runner::run`
  instead) and `ProcessInfo::kernel_entries` (the evaluator's input).
- Decisions in plan section 12: names; I's wrapping check (about 10 points faster, but an access outside the slot
  lands in the process's own memory where Linux faults); P's guest can read and write its stub's own pages where
  Linux faults (no harm to us, plan 3.7, but a difference); PAuth and MTE not offered under P and I; a clock the
  system publishes; changing the form at an exec; per-thread watches under P by a process per guest thread; I's
  runner threads holding off the embedder's SIGCANCEL and SIGSETXID while they run.

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
