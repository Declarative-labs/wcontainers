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
2026-11-26: his main again (FETCH_HEAD ac696ea19, already merged); origin/kernel-port still at e177511f5 (already
merged); f94a514ef merges origin/executor-restart 62d137472 (one conflict, Cargo.lock: their hermit-abi and hex entries
kept). After it the 67 files of his interface folders (`interface.rs` and the data files beside them) are byte-identical
to his main (MEASURED, `git diff --quiet FETCH_HEAD` on each).

## RT0: done (results in SCRATCH/runtime-restart/results-main, results-agents)

## RT1: built; bar NOT met: the linter (the earlier line here that called it met but for the linter was wrong)

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

Linter (`cargo reviewable check`, ARM64, reviewable-rust da55b9d7, no model key; MEASURED 2026-11-26 before at
322758f61, after at this commit). Every finding that needs neither the model nor a change of structure is cleared, in the
code or the comment, with no exclusion: magic literals named or taken from a type, summaries one third-person sentence on
one line, named numbers linked, functions named verb first (`open_udp`, `check_linux`, `run_timer`, ...), the hook
renamed `on_stop_flag_made`, duplicate constants shared or renamed, items no other crate names made `pub(crate)`.

| rule | single-kernel | suite | policy |
|---|---|---|---|
| magic_literal | 306 -> 0 | 4 -> 0 | 0 -> 0 |
| function_summary_not_third_person | 91 -> 0 | 7 -> 0 | 1 -> 0 |
| summary_not_one_line | 81 -> 0 | 0 -> 0 | 0 -> 0 |
| summary_too_long | 44 -> 0 | 0 -> 0 | 0 -> 0 |
| verb_not_first | 25 -> 0 | 0 -> 0 | 1 -> 0 |
| number_constant_without_link | 17 -> 0 | 4 -> 0 | 0 -> 0 |
| summary_not_one_sentence | 14 -> 0 | 0 -> 0 | 0 -> 0 |
| literal_restates_constant | 5 -> 0 | 0 -> 0 | 0 -> 0 |
| summary_starts_with_name | 4 -> 0 | 0 -> 0 | 0 -> 0 |
| comment_too_long | 4 -> 0 | 0 -> 0 | 0 -> 0 |
| constant_defined_twice | 2 -> 0 | 0 -> 0 | 0 -> 0 |
| comment_budget_exceeded, hook_name | 1, 1 -> 0, 0 | 0 -> 0 | 0 -> 0 |
| public_outside_interface | 163 -> 115 | 11 -> 11 | 41 -> 41 |
| implementation_folder | 29 -> 29 | 0 -> 0 | 14 -> 14 |
| conformance_not_run | 3 -> 3 | 7 -> 7 | 1 -> 1 |
| trait_outside_interface (the four old-loop traits) | 4 -> 4 | 0 -> 0 | 0 -> 0 |
| trait_not_implemented | 1 -> 1 | 0 -> 0 | 1 -> 1 |
| unparsed | 0 -> 0 | 0 -> 0 | 1 -> 1 |
| model: summary_unanswered | 33 -> 150 | 3 -> 10 | 8 -> 9 |
| model: name_unanswered | 12 -> 37 | 9 -> 9 | 0 -> 1 |
| model: grammar_unclear, summary_unclear | 12, 14 -> 12, 11 | 0 -> 0 | 0 -> 0 |
| model: summary_restates_declaration, type_summary_not_noun_phrase, hook_without_participle | 4, 1, 1 -> 0 | 0 -> 0 | 0 -> 0 |
| total | 872 -> 362 | 45 -> 37 | 68 -> 68 |

The model's rules grow because every rewritten summary and renamed function needs a fresh answer, which a run without
the key cannot record (all are listed in SCRATCH/runtime-restart/lint/after-findings.tsv). What is left that needs no
model is structural, and each part waits on something named:
- public_outside_interface (167): items another track's crate names stay public (wasmtime's `wcon` and tests: `Runtime`,
  `RuntimeConfig`, `GuestMachine`, `Executor`, `Image`, `LocalSystem`, `Warned`, `spec_from_json`, `base_spec`,
  `bind_mount`, the two path fields; unicorn's harness: `TerminalSize`, `SliceTimer`, `names`, `udp`, the image layout
  and account table constants; the old loop's `Answer`, `Carry`, `Execution`, `Leaving`, `StopFlag`, `Turn`; the
  policy's whole surface, which the harness and the native track drive). They go once the runtime implements
  `wcontainers_runtime::Runtime` (RT2) and those crates move onto it; a system has no constructor in his interface (owner
  line 1), so `LocalSystem::new` cannot go at all.
- implementation_folder (43): `LocalSystem` implements the system interface inside the runtime crate; it belongs in
  `crates/system/implementation/<name>`, which needs owner line 1 for its constructor. Of the policy's 14, those on
  `Clock`, `RandomSource` and the sockets go once it implements `wcontainers_system::System` (it lacks ICMP, raw sockets
  and routes); those on the kernel's own `wcontainers_kernel_linux::System` and on the runtime's own `System` stay until
  KP2 and RT3 end those traits.
- conformance_not_run (11): the suite's functions take a `Subject` because `tests!` hands one value and nothing in his
  interfaces makes a `System` from it (owner line 1); the runners' `Container` and `Interrupter` impls are not on an
  exported type until RT2.
- trait_not_implemented (2): nothing implements `wcontainers_runtime::Runtime` before RT2; the policy implements
  `FallbackObserver` without `System`.
- unparsed (1): seeded-test-policy implementation.rs:336, `Readiness<u32, wait_until_ready(..): Send>`; the linter does
  not parse return type notation (owner line 2), and the kernel's own `System` asks for it.

Bar after the linter round and the executor merge (MEASURED both machines, rustc 1.99.0, `-j 12`, load ARM 22-30, x86
10-17): check and clippy `-D warnings` of the three packages (all targets, and single-kernel `--no-default-features`)
exit 0 with 0 warnings; rustfmt `--check` 0 diffs; single-kernel unit tests 15 of 15; the policy's stream tests 13 of
13. Behavior: the one change is that the runners' idle polls, counted before and read by nothing, now reach the system as
a fallback (`IDLE_WITHOUT_WAKE`, count and the poll's nanoseconds) when a run ends, the counted slow path the brief's
question 2 asks for.

Dependents (MEASURED, `cargo check --lib`, both machines): wasmtime fails (75 errors: its own `unsafe` and the
port, and 2 because `CpuSide` is not yet the committed `Executor`), so does unicorn through it; hardware-virtualization
-inside fails (12 errors, one of them its call of the removed `System::started`), so does hardware-virtualization
through it. All four are X2's or the native track's to move.

## RT2: started; no container ran

What RT2 asks of this branch first, the kernel's public drive and `Host` gone, was done at RT1: containers run only
through `Kernel::new`, its runners and `Runner::run`, and the kernel's fallbacks come through `FallbackObserver`.
origin/kernel-port has nothing newer than e177511f5.

A container on the stand-in: NO CONTAINER RAN. The stand-in from origin/executor-restart is an `Executor` of his
interface (MEASURED, a probe test in a throwaway copy, `cargo test --test standin_probe`, ARM64: `is_executor::<StandIn>()`
compiles) but it is not `Send`, and nothing else stands between it and the kernel: 14 errors, all E0277, all `Send`
(`Rc<UnsafeCell<UnicornInner>>` and `*mut uc_context` of unicorn-engine itself, `Rc<SharedPages>`,
`Rc<RefCell<Memory>>` and `Weak<..>` of `UnicornSpace`). His interface asks `Kernel: Sync` and `Runner: Send`, and
`KernelWithExecutor` holds an executor behind its lock, so its executor must be `Send` (and the kernel's impl asks
`GuestMemory: Send`). So `KernelWithExecutor<StandIn, (), LocalSystem<StandIn>>` is no `Kernel`, and
`LocalSystem<StandIn>` no `System`. The executor track made the stand-in thread-bound at X3 when it dropped its
`unsafe impl Send`; making it `Send` again without `unsafe` means keeping no unicorn handle between executions (an engine
made per execution on the running thread, the memories as owned bytes), or the kernel holding no executor between
calls. That is the executor track's or the kernel track's to choose (line 3 below).

## Next

- RT2 (containers through the runners, the suite at parallelism 1 and 4, interrupts): waits on X3 (a `Send` stand-in,
  line 3) and X2 (wasmtime on the committed `Executor`), then KS, on this branch. Ready to build here meanwhile, after a
  word on the terminal: `wcontainers_runtime::Runtime` on the runtime (it takes the standard endpoints from its caller,
  so the local system's terminal and input reader move behind them).
- The cores measured at a container's first start (docs/runtime.md section 16): after RT2; waits on a complete sample
  (the collector stops on both machines today), the collector implementation on his main or this branch, executors
  whose behavior source is made from a measured set, and his word on where samples are kept and on a failed collection.
- RT3: waits on KP2 and X4.

## For the orchestrator (one line each)

1. Kernel track: `KernelConfig` holds `Start<'static>`, so every container leaks its start (path, arguments,
   environment, working directory, host name, groups, backend path and mount list, a few kilobytes each); owning its
   bytes (`struct StartData { path: Vec<u8>, arguments: Vec<Vec<u8>>, environment: Vec<Vec<u8>>, ... }`, or
   `Start<'a>` with `KernelConfig<'a, ..>`) would end it.
2. Executor track: X2's executors meet the runtime as `Executor: Cpu + wcontainers_executor::Executor<TranslatorConfig:
   Clone + Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>> + Send + 'static` plus `shared`, `new`,
   `stop_flag`, `ARCHITECTURE`, `Offer`; `wcon` reads `Container::run() -> StopReason`.
3. Executor track (and kernel track): the stand-in is not `Send` (unicorn's own handle, `Rc<SharedPages>`,
   `UnicornSpace`'s `Rc<RefCell<Memory>>`), and `Kernel: Sync` makes the executor `KernelWithExecutor` holds need it,
   so no container runs on the stand-in (14 E0277 errors, all `Send`).

For the owner (one line each, with the code):
1. A system cannot be made through his interfaces: `pub trait StandaloneSystem: System { type Config; fn new(config:
   Self::Config) -> Self; }` (or a `System` the runtime's `Config` makes) would let the runtime suite run as `tests!`
   and let the local system leave the runtime crate.
2. The linter does not parse return type notation, which the kernel's own `System` asks of a generic system:
   `+ Readiness<u32, wait_until_ready(..): Send>` (seeded-test-policy implementation.rs:336, finding `unparsed`).

## SECURITY

- Lines with `unsafe`, before (main / origin/agents) and after: single-kernel 7 / 0 -> 0, runtime suite 0 / 0 -> 0,
  seeded-test-policy 0 / 0 -> 0. No marked exception; every crate keeps `[lints] workspace = true`.
- New dependencies: none from outside the workspace. Against main's copy, single-kernel no longer uses `libc` or
  `polling`.
- 2026-11-26, after the linter round and the executor merge (MEASURED, `git grep -w unsafe` per folder): single-kernel 0,
  runtime suite 0, seeded-test-policy 0; each keeps `[lints] workspace = true`. No dependency added (the stand-in
  probe's dev-dependency on unicorn lived only in a throwaway copy on the rented machine).
- Hostile-guest suite: none exists; none run (the thread-test design review is owed to the owner first).
