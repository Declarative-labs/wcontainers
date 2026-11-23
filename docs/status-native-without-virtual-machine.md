# Native without a virtual machine: status

Plan: docs/native-without-virtual-machine.md. Branch: agents (shared worktree). Scratch:
pyro-parked/wcontainers-scratch/native-without-virtual-machine.

## Where things stand

- W0 (the plan for both forms) is written, run 1, 2026-11-23: docs/native-without-virtual-machine.md. It answers
  every item of the milestone; its decisive figures not in the orchestrator's files were measured on both rented
  machines (plan section 11). It waits for its independent check.
- No code yet. The two crates (crates/executor/implementation/host-process and in-process) are not created: W1
  creates host-process, W2 in-process. So the root Cargo.toml's unsafe list has no lines of ours yet.

## Next

- After the check: W1 (form P on ARM64 Linux), plan section 13. It needs the translator's `HostProcess` form first
  (below, "Asked of other tracks"); until that exists, W1 can build everything of plan sections 3 and 4 that does
  not run translated code (layout and reservations, memfds and views, the zygote and stubs, the channel and runners,
  copy-on-write and written marks, stops, the seccomp filter) and test it with hand-written ARM64 pieces.

## How to run the measurements

- `w0/probe.c` in scratch: `gcc -O2 -static -o probe probe.c -lpthread && ./probe 20000` on a Linux host
  (`./probe 1 as` prints only the address-space part). It was sent to the machines with
  `arm-run --ssh` / `x86-run --ssh` (base64 through the command line, built in /work/nwvm-w0 there).

## Measurements

All in plan section 11, with machine and load. Discarded: probe-arm.out and probe-x86.out (a spinning thread of the
stop test was left running on CPU 3 and slowed the futex figures), probe2-*.out (the stop test looked for its target
thread among the next 64 thread ids, which the busy machine had passed, and hung). probe3-*.out are the results.

## Asked of other tracks

- Translator and ARM64 (crates/wcontainers-translator, crates/wcontainers-arm64): a form value given to
  `translate_native` and part of the cache key, with variants `Vm`, `HostProcess`, `InProcess`, that changes four
  places only (plan 3.2): `carry.rs` `access` (InProcess: `ubfx u, a, #r, #(56 - r); cbnz u, fault` then the access
  against a base register); the jump through a register (slot table at 2^46 instead of `mirror::MIRROR`, reach 2^44,
  and InProcess's base add); `Arm64::call` (`str x30, [page, #saved_link]; bl entry_<code>` instead of `brk`); the
  replay check's stop word from the register page instead of `TPIDRRO_EL0`. Two more withheld registers for
  InProcess.
- Native (VM) track: the monitor's kept translations (`Translator`, crates/executor/implementation/
  hardware-virtualization/translate.rs) in a crate that does not pull KVM or Hypervisor.framework.
- Kernel: K2 (placement low first, memory size at start) as docs/native.md asks; `ProcessInfo::kernel_entries` if
  the owner takes it.

## For the owner

- Two interface additions, as code in plan section 8: `Executor::run_thread` (today `KernelRunner::run` is
  `unimplemented!` for want of it, for every executor) and `ProcessInfo::kernel_entries` (the evaluator's input).
- Decisions in plan section 12: names; I's wrapping check (about 10 points faster, but an access outside the slot
  lands in the process's own memory where Linux faults); P's guest can read and write its stub's own pages where
  Linux faults (no harm to us, plan 3.7, but a difference); PAuth and MTE not offered under P and I; a clock the
  system publishes; changing the form at an exec.
- Neither form can give per-thread watches (`PerThreadWatch`): threads of one host process share its page tables.

## Departures from the plan

None yet.

## Unsafe code

None yet (no crate). Expected uses are listed in plan section 2.

## Dependencies and licences

None added yet. Planned: rustix (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT), and libc (MIT OR Apache-2.0)
only where rustix has no call.
