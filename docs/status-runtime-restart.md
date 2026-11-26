# Runtime track: status (restart of 2026-11-25)

Brief: orchestrator/restart-2026-11-25/brief-runtime.md. Plan: docs/runtime.md. The old status (2,698 lines) stays on
origin/agents. Facts are MEASURED (a command ran on both rented machines unless one is named), READ or INFERRED.

## Branch

`runtime-restart`, cut from his main ac696ea19 (FETCH_HEAD of his checkout, merged first: already up to date), then
merged: kernel-port e177511f5 (KP1 with the kernel's own part of KP2), translator-port 15968cb89, host-sockets 1ea5c793e
(HS1 and HS2 done by that track's own status; one conflict, Cargo.lock, both sides' new packages kept) and
streams-restart 1adeb3f37 (the system suite the policy's stream tests run). After the merges every owner interface file
(100 files in the 29 interface folders) is byte-identical to his main (MEASURED: `git diff --quiet main HEAD` on each).
Brought by path from origin/agents 198bb11d6: single-kernel, the runtime suite, seeded-test-policy, docs/runtime.md.

## RT0: done (results in SCRATCH/runtime-restart/results-main, results-agents)

## RT1: built; bar met but for the linter

The kernel's old drive is gone (its `next` and `exit` are the runner's alone, `event` split into the runner's
`fire_timers` and `resize_terminal`), so the old loop could not be kept: a container now runs through
`KernelWithExecutor` (`Kernel::new` with one runner, so threads still run one at a time on one host thread) and
the runners' driver `KernelContainer` built for JR. What changed:
- implementation.rs: `Stepper`, `drive`, `Step`, `Ending` and the loop's answer are gone; the own `System` keeps
  `standard`, `stop_flag`, `serve_directory` and a hook `on_container_made`. `Execution`, `Answer`, `Carry`,
  `Leaving`, `Turn` and `StopFlag` stay, unused by the runtime, for the executors that still name them (RT3).
- container.rs: `Executor` asks for the committed `Executor` (with the bounds the kernel needs) and the abi's `Cpu`
  (the kernel reads `address_space` once, owner line 3 of the kernel track) instead of `Execution`; it loses its own
  `TranslatorConfig`. `Container::run` returns `StopReason`; `interrupter()` gives the `RunInterrupter`; `step`,
  `idle`, `start_failure` (now `ProcessEnd::NotStarted`) and `executor` are gone.
- The kernel's configuration holds its `Start` as `'static`, so a container's arguments, environment and paths are
  kept for the life of the process (`kept`, a few kilobytes per container).
- LocalSystem: arrivals (standard input, network readiness) are handed over by a thread of the system's own as they
  come, since no loop between turns asks; it ends when the system is dropped. `Host` is no longer implemented; the
  kernel's fallbacks come through `FallbackObserver`. The slice timer is no longer armed (nothing calls a turn
  start; owner question 3); its two tests moved to timer.rs and test the timer itself.
- seeded-test-policy: its loop-only hooks are inherent methods (`turn`, `ran`, `count_turn`, `skip_to`,
  `cpu_warning`) for whoever drives it; `Host` became `FallbackObserver`.
- The policy's stream loop (conformance_run.rs) had the three faults the streams track read; fixed, and the
  streams track's 13 stream tests are taken.

Bar (MEASURED, rustc 1.99.0, `-j 12`):

| | ARM64 | x86-64 |
|---|---|---|
| check and clippy `-D warnings`, single-kernel `--all-targets` | 0 / 0 | 0 / 0 |
| the same with `--no-default-features` | 0 / 0 | 0 / 0 |
| check and clippy, runtime suite and seeded-test-policy `--all-targets` | 0 / 0 | 0 / 0 |
| rustfmt `--check`, the three packages | 0 | 0 |
| single-kernel unit tests | 15 passed | 15 passed |
| policy stream tests, fixed loop | 13 passed | 13 passed |
| policy stream tests, agents' loop (x86-64) | | 7 passed, 6 failed (the three faults) |

Lines with `unsafe` (root manifest's count): single-kernel 0, suite 0, policy 0 (main had 7 in single-kernel). Every
crate inherits `forbid`. No `libc` or `polling` in single-kernel (its lock entry lost both).

Linter (`cargo reviewable check`, ARM64, built from reviewable-rust da55b9d7, no model key): single-kernel 873 (908 at
agents), suite 45 (45), policy 69 (58). NOT at the bar. By cause:
- the four internal traits, waiting on RT3: container.rs `Executor`, implementation.rs `Answer`, `Execution`, `System`
  (trait_outside_interface 4);
- the model's answer, whose key may not go to a rented machine: summary_unanswered, name_unanswered, grammar_unclear,
  summary_unclear (smallest: conformance_run.rs:40 `wake`, summary_unanswered);
- rewriting the literals and comments of 8,000 lines: magic_literal 306, summary shape rules about 270, number
  constant links 17 (smallest: files.rs:132, `0` in `kind`);
- the crate's surface other tracks' crates use: public_outside_interface 163 + 11 + 41;
- the suite and the policy not yet run as the linter's `tests!` shape: conformance_not_run (the runtime suite's
  functions take a `Subject`; the policy runs `stream_tests!`), trait_not_implemented (nothing implements
  `wcontainers_runtime::Runtime` before RT2; the policy implements `FallbackObserver` without the system interface's
  `System`);
- return type notation the linter cannot parse: seeded-test-policy implementation.rs:336 `unparsed` (the kernel
  track's owner line 2 is the same).

Dependents (MEASURED, `cargo check --lib`, both machines): wasmtime fails (60 errors: its own `unsafe` and the
port, and 2 because `CpuSide` is not yet the committed `Executor`), so does unicorn through it; hardware-virtualization
-inside fails (12 errors, one of them its call of the removed `System::started`), so does hardware-virtualization
through it. All four are X2's or the native track's to move.

## Next

- RT2 (containers through the runners, the suite at parallelism 1 and 4, interrupts): waits on KS, X2 and X3 on this
  branch. Built ahead of it here: the container path itself.
- The cores measured at a container's first start (docs/runtime.md section 16): after RT2; waits on a complete sample
  (the collector stops on both machines today), the collector implementation on his main or this branch, executors
  whose behavior source is made from a measured set, and his word on where samples are kept and on a failed collection.
- RT3: waits on KP2 and X4.

## For the orchestrator (one line each)

1. Kernel track: `KernelConfig` holds `Start<'static>`, so every container leaks its start; owning its bytes
   (`struct StartData { path: Vec<u8>, arguments: Vec<Vec<u8>>, ... }`) would end it.
2. Executor track: X2's executors meet the runtime as `Executor: Cpu + wcontainers_executor::Executor<TranslatorConfig:
   Clone + Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>> + Send + 'static` plus `shared`, `new`,
   `stop_flag`, `ARCHITECTURE`, `Offer`; `wcon` reads `Container::run() -> StopReason`.

## SECURITY

- Lines with `unsafe`, before (main / origin/agents) and after: single-kernel 7 / 0 -> 0, runtime suite 0 / 0 -> 0,
  seeded-test-policy 0 / 0 -> 0. No marked exception; every crate keeps `[lints] workspace = true`.
- New dependencies: none from outside the workspace. Against main's copy, single-kernel no longer uses `libc` or
  `polling`.
- Hostile-guest suite: none exists; none run (the thread-test design review is owed to the owner first).
