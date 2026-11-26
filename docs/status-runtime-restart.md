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

2026-11-26 (afternoon): his main FETCH_HEAD ac696ea19 already merged (it holds today's named iterator types,
`MemoryReader` and `Executor::Execution`); merged origin/kernel-port d176769a7 (KS) and origin/executor-restart 9aaaf77a3
(the stand-in that is `Send`), both without conflict. Every file of his interface folders that exists on his main is
byte-identical to it (MEASURED, `git diff --quiet FETCH_HEAD HEAD` per file; the only other files in those folders,
crates/kvm and crates/linux-socket-options, are not on his main). origin/executor-wasmtime does not exist yet (MEASURED,
`git fetch origin`). The merged kernel does not compile: running.rs:197 builds `Processor` without the two fields
executor-restart's arm64 merge added (MEASURED, E0063); that file is the kernel track's, so every run below applied a
throwaway fill in the machine's copy only, never committed. Later the same day origin/kernel-port 36bf10eed (which had
merged this branch at 93637d42e and fills those fields) merged here without conflict (deba6c650): the tip builds as
committed, and the final bar below ran with no fill. After it the 77 top-level files of his interface folders are
byte-identical to his main (MEASURED). origin/executor-wasmtime appeared (ee26ae9f9) but its status still reads "X2: not
started", so it is not merged.

2026-11-26 (evening): his main FETCH_HEAD ac696ea19 already merged. Merged origin/executor-restart 914b4e9d1 (dd899860a, no
conflict), origin/host-sockets 572e37e70 (4c9d4f318, no conflict) and origin/executor-restart c4113f822 (b07c27008, no
conflict: the stand-in's own slice, and kernel-port 406fba566, which that branch had merged), then origin/executor-restart
20d552cdf (08028d218, no conflict). After them the 91 files of his interface folders are
byte-identical to his main (MEASURED, `git diff --quiet FETCH_HEAD HEAD` per file).

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
| number_constant_without_link | 17 -> 0, really 13 (below) | 4 -> 0, really 0 | 0 -> 0 |
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

CORRECTION (2026-11-26): the independent check did not hold for `number_constant_without_link`. Eight were silenced
with links that do not fix the value: the suite's EXIT_CODE, BEFORE_INTERRUPTING, SIDE_BY_SIDE, ROUNDS and single-kernel's
IDLE_POLL and LIMITS (links to our own docs/runtime.md), CHANGES_PER_WAIT (mio's `with_capacity` page) and
BLOCKS_AT_A_TIME (ruzstd's enum page). So that round cleared 13 of single-kernel's 17 and none of the suite's 4. That was
a way round the owner's rule; each is now fixed in the code (section RT2), with no link.

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

## RT2: containers run through the runners on the stand-in; the suite passes at 1 and 4 runners on both machines

Built (orchestrator rulings 1 and 2):
- `wcontainers_runtime::Runtime` is implemented by `Runtime<E, S>` (container.rs). Its `Config` is `RuntimeConfig<E>`:
  `make_executor` (the kernel's executor from the translator's configuration), `make_runner_executor` (each runner's,
  from the kernel's: a runner's executor must run on the memories the kernel's makes; the stand-in's `restored` shares
  its engine), `runners` (the kernel's parallelism) and `idle_poll`. `ImageSource` is a plain tar or an OCI layout; the
  image must be for `arm64`, the only guest the kernel runs (`GUEST_ARCHITECTURE`). `make_container` refuses a thread
  ordering other than `Serial` and a replay other than `Exact` with the interface's errors, maps a field refused, a
  program not found and a file the kernel refused (`Error::FileRefused`, new) to `ContainerError`; the image implements
  `wcontainers_runtime::Image`.
- The standard endpoints come from the caller (`ContainerConfig`): pipes, or with `process.terminal` one terminal
  endpoint, refused unless output, error and any input name the same one. The local system's endpoints and its input
  reader moved behind them: `LocalSystem::output_end`, `error_end`, `give_input(reader)`, `give_terminal(reader?)`; the
  reader starts when the container is made. The runtime's own `System` lost `standard` and `on_stop_flag_made`; the
  arrivals no longer raise a stop flag.
- Fitted to kernel-port 36bf10eed (7d6d0e11e): the kernel now bounds its spaces only by the start's ceiling, so the runtime
  gives `ceiling: Some(end of the executor's space)` (with `None`, every `fork` failed: "sh: can't fork: Bad address",
  MEASURED); and its runner now goes on to the next turn after an execution's interrupt, leaving `run` only while one of
  the kernel's calls waits for its lock, so ending a run also calls the kernel (`suspend_tasks(&[])`, which takes no task)
  until no runner is inside `run` (without it the interrupt test hung, MEASURED). Asked as owner/orchestrator line 5.
- Interrupts go through the executor's own `ExecutionInterrupter` (brief question 1, the new path): every executor is
  wrapped in `Interruptible<E>` (interruptible.rs: an `Executor`, `BehaviorSource`, `Cpu` and `PerThreadWatch` passing
  every call to `E`), whose `prepare_execution` keeps a share of each execution's interrupter in a slot. The container's
  interrupter, and a runner that finishes or panics, raise every runner's slot: the execution running is interrupted, and
  one prepared later is interrupted at once, until the run is over. `StopFlag` is on no runtime path now; it stays only
  for the old executors (RT3).
- The runtime's own `Executor` trait (`shared`, `new`, `stop_flag`, `ARCHITECTURE`, `Offer`) and `GuestMachine` are gone:
  the caller gives the executors through the configuration (line 2 below).
- conformance_run.rs runs the runtime suite on `LocalSystem` with the unicorn stand-in (dev-dependency without its
  harness). Its parameters come from the environment: `WCONTAINERS_RUNTIME_SUITE_IMAGE` (a plain tar holding `sh`,
  `cat`, `cmp`), `WCONTAINERS_RUNTIME_SUITE_RUNNERS`, `WCONTAINERS_RUNTIME_SUITE_IDLE_POLL_MICROSECONDS`.

The numbers that were linked falsely, each now derived or a parameter (none has a link):
- suite EXIT_CODE: `u8::MAX`, the largest status Linux keeps; a shell's own are 0 to 2, 126, 127 and 128 plus a signal.
- suite BEFORE_INTERRUPTING: measured in the test, as long as a container that only exits took to make and run.
- suite SIDE_BY_SIDE and ROUNDS: gone; the test copies the shell's own program through a pipeline of one more `cat`
  stage than the runtime has runners (`Subject::runners`, new) and compares it with `cmp`.
- IDLE_POLL: `RuntimeConfig::idle_poll`, a parameter of the configuration (and of the suite run).
- LIMITS: derived from the kernel's own `DOCKER_LIMITS`; the specification's names of Linux's 16 resources are paired
  with the kernel's `linux::general` numbers (`RESOURCE_NAMES`, linked to runtime-spec v1.2.1 config.md, which names them).
- CHANGES_PER_WAIT: the poller's events hold one change per source it was given (its waker and every socket made).
- BLOCKS_AT_A_TIME: each decoding step is `BlockDecodingStrategy::UptoBytes(buffer.len())`, the caller's buffer.
- RUNNERS (a constant `NonZeroUsize::MIN`): `RuntimeConfig::runners`.

Bar at 7d6d0e11e, with no fill (MEASURED, rustc 1.99.0, `-j 12`, jobs runtime-restart/20261126-123615-2816384 ARM64
load 25, 20261126-123610-775590 x86-64 load 49-54; logs SCRATCH/runtime-restart/rt2/final4-*.log; the same table held at
e7f605981 before the kernel merge, with the fill, final2-*.log). `cargo metadata --locked` exits 0 on both.

| | ARM64 | x86-64 |
|---|---|---|
| rustfmt `--check`, three packages | 0 | 0 |
| check `--all-targets`, three packages | 0 | 0 |
| check and clippy `--no-deps -D warnings`, single-kernel `--no-default-features --all-targets` | 0 / 0 | 0 / 0 |
| clippy `--no-deps -D warnings --all-targets`, three packages | 0 | 0 |
| single-kernel unit tests | 15 of 15 | 15 of 15 |
| policy tests (`--all-targets`) | 13 of 13 | 13 of 13 |
| runtime suite, 1 runner, three rounds | 7 of 7 each | 7 of 7 each |
| runtime suite, 4 runners, three rounds | 7 of 7 each | 7 of 7 each |

The seven tests: a container runs to its end; a finished container stays finished; a program nowhere is refused; a
guest in `while :; do :; done` ended by `Interrupter::interrupt` from another thread, twice; an interrupt before a run
ends that run alone; a pipeline of 2 or 5 processes; a host thread panicking inside a run (the system's standard output
panics on its first write) ends the container as `Panicked` for every later run. Planted (MEASURED, ARM64): with the
container's interrupter raising no executor's slot (before the kernel merge), and again at 7d6d0e11e with it ending no
run, `an_interrupt_from_another_thread_stops_a_run` runs past the timeout at 1 and at 4 runners (exit 124). The image: alpine:3.22 for linux/arm64 from `docker export`.

RT2 IS NOT CLAIMED: its bar also asks the suite and the interrupt test on wasmtime from a merged commit, the kernel
harness at full, none and bounded against j5f's lists, and LTP.

### The time slice (orchestrator ruling of 2026-11-26: an executor bounds its own run)

The independent check found a container with a busy background process never finished on one runner. MEASURED at
2b50e4a23 before the executor's slice (ARM64, job 20261126-190222-1663300, slice/base2-arm.log): the check's test,
added to the suite, ran past the timeout at 1 runner (exit 124) and passed at 4.
- The suite has an eighth test, `a_busy_background_process_does_not_keep_its_container_running`: `sh -c "(while :; do
  :; done) & sleep T; exit 255"`, T being as long as a container that only exits took (measured in the test, no number);
  the first process sleeps so the busy one holds the runner first.
- The stand-in now ends a slice of its own configuration (`StandInConfig::slice_instructions`, default 1,000,000 retired
  instructions) with `TrapHandler::handle_interrupt` (executor-restart 30ca724a8, merged). The runtime needs nothing
  else: the kernel takes that interrupt as a slice's end.
- The slice timer is gone (`SliceTimer`, timer.rs and its two tests; nothing used it). `StopFlag` stays for RT3.
- `ceiling: Some(executor end)` as before; the end is now found through the executor interface (`space_end`: a bisection
  over `MemorySpace::borrow`, which fails only outside the space), so the runtime asks no abi `Cpu` of an executor
  (wasmtime has none). New unit test: the probed end equals the stand-in's `address_space().end`.
- conformance_run.rs runs the suite in a module per executor (`on_the_stand_in`), generic over the executor.

Bar at a839372ff (pushed; the runs were of the working tree equal to it, MEASURED, rustc 1.99.0, `-j 12`, jobs
runtime-restart/20261126-195620-1777559 ARM64 load 25-42 and 20261126-195615-782681 x86-64 load 42; logs
SCRATCH/runtime-restart/slice/bar3-*.log):

| | ARM64 | x86-64 |
|---|---|---|
| `cargo metadata --locked` | 0 | 0 |
| rustfmt `--check`, three packages | 0 | 0 |
| check `--all-targets`, three packages; single-kernel `--no-default-features` | 0 / 0 | 0 / 0 |
| clippy `--no-deps -D warnings --all-targets`, the same two | 0 / 0 | 0 / 0 |
| single-kernel unit tests | 14 of 14 | 14 of 14 |
| policy tests | 13 of 13 | 13 of 13 |
| runtime suite, 1 runner, three rounds | 8 of 8 each | 8 of 8 each |
| runtime suite, 4 runners, three rounds | 8 of 8 each | 8 of 8 each |

The same table held again at 08028d218 (pushed, after merging executor-restart 20d552cdf; MEASURED, jobs
runtime-restart/20261126-201750-2030376 ARM64 load 20 and 20261126-201741-1488821 x86-64 load 22-46; slice/bar4-*.log),
with the plant below repeated there.

Planted (MEASURED, ARM64, the machine's copy only): the stand-in configured with `slice_instructions: u64::MAX` makes
the busy test run past the timeout at 1 runner (exit 124) and pass at 4 (slice/bar3-arm.log, plant2-arm.log).

Linter at a839372ff (ARM64, reviewable-rust da55b9d7, no model key): single-kernel 396, suite 43, policy 68; the only
changes from the table below are the model's unanswered questions for the new and changed items (time_to_exit, the busy
test, space_end, the two subjects), and 5 fewer public_outside_interface (`SliceTimer` gone). No magic_literal,
number_constant_without_link or summary rule.

### The WebAssembly half, prepared as one command (not merged: its status names no commit for containers yet)

SCRATCH/runtime-restart/wasmtime/rt2-wasmtime.sh <commit>: a throwaway worktree of this branch merged with that commit of
origin/executor-wasmtime, wasmtime-suite.patch (wasmtime as a dev-dependency and a module `on_wasmtime` running the same
suite, the executors made with `WasmtimeExecutor::new`), then the suite on the stand-in and on wasmtime at 1 and 4
runners, three rounds, on both machines. MEASURED with b547d187f (origin/executor-wasmtime's tip; its status still reads
4bae8d25 for RT2), FILL=1: merge without conflict; clippy 0; on wasmtime 8 of 8 in every round at 1 and 4 runners on
both machines (ARM64 load 38, x86-64 load 46-49); planted `slice: None` on wasmtime makes the busy test run past the
timeout at 1 runner (exit 124). FILL=1 is needed: that branch brings arm64's new `Processor::process_state_controls`
(58791651d), which the kernel's running.rs and the stand-in's standin.rs do not fill (E0063 in each); fill.cmd adds
`process_state_controls: 0` in the machine's copy only. Again with 6e358a5c3 (the tip later, on this branch at
08028d218): 8 of 8 on wasmtime and on the stand-in in every round at 1 and 4 runners on both machines (jobs
runtime-restart-wasmtime/20261126-201821-2058265 ARM64 load 20, 20261126-201838-1519618 x86-64 load 14-44); that tip
fills the stand-in itself, so only running.rs still needs the fill.

Linter (`cargo reviewable check`, ARM64, reviewable-rust da55b9d7, no model key; MEASURED at the tree before the last
two small fixes): single-kernel 403, suite 39, policy 68. `magic_literal` 0 and `number_constant_without_link` 0 in all
three. Structural: public_outside_interface 127 / 12 / 41; implementation_folder 34 (the local system's 29, the five
`Interruptible` impls) / 0 / 14; conformance_not_run 8 / 7 / 1 (the suite takes a `Subject`; conformance_run.rs is more
than one `tests!` call and names unicorn; the test target needs `std`; and the linter reads `Runtime`, `Container`,
`Image`, `RunInterrupter` as not exported, being re-exported from private modules); trait_outside_interface 3 / 0 / 0
(`Answer`, `Execution`, the runtime's `System`: the runtime's `Executor` is gone); trait_not_implemented 1 / 0 / 1;
unparsed 0 / 0 / 1 (owner line 2). Three `literal_restates_constant` (two "arm64" in image tests, `-c` in the pipeline
script) are fixed in the commit, not relinted. The model's: summary_unanswered 169 / 10 / 9, name_unanswered 41 / 9 / 1,
grammar_unclear 10, summary_unclear 8.

## Next

- RT2: waits on (a) the executor track's status naming a commit of origin/executor-wasmtime a container runs on, then
  rt2-wasmtime.sh with it and the merge; (b) the kernel track filling `process_state_controls` in running.rs, without which
  that merge does not build; (c) the harness `--features runtime` on wasmtime (the
  executor track's kernel-harness-on-wasmtime.patch touches their files) at full, none and bounded against j5f's lists,
  and LTP.
- RT3: waits on KP2 and X4.

## For the orchestrator (one line each)

1. Kernel track: `KernelConfig` holds `Start<'static>`, so every container leaks its start (a few kilobytes); owning its
   bytes, or `KernelConfig<'a, ..>`, would end it.
2. Executor track (X2): the runtime asks of an executor only `wcontainers_executor::Executor<TranslatorConfig: Send
   + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>> + Send + 'static` (no abi `Cpu` since 286f5cc60); wasmtime is given through `RuntimeConfig
   { make_executor, make_runner_executor, runners, idle_poll }` and run through `Runtime::make_container`; wcon takes its
   endpoints from `LocalSystem::output_end`, `error_end`, `give_input`, `give_terminal`.
3. Kernel track: the 21 errors measured in single-kernel were main's old copy of the crate; kernel-port has since merged
   this branch and this branch merged kernel-port 36bf10eed back; both build (MEASURED above).
4. Kernel track: done on kernel-port (789862313): running.rs fills the processor's predicate fields.
5. Kernel track (and question 3 of the brief): the runner leaves `run` only while one of the kernel's calls waits for its
   lock, so an embedder's interrupt needs a call that does nothing (`suspend_tasks(&[])`); an execution interrupt that
   ends `run` as `Interrupted` when the embedder asks (as at d176769a7) would let the runtime drop that loop.
6. Kernel track: origin/executor-wasmtime carries arm64-instructions 58791651d, whose `Processor::process_state_controls`
   running.rs:210 does not fill (E0063, MEASURED in the trial merge; the stand-in fills it from 6e358a5c3).

For the owner (his files unchanged):
1. A system has no constructor. Before (crates/system/interface.rs:26):
   ```rust
   pub trait System:
       ProcessTracer + ProcessorInputs + MachineDescription + TaskIdAllocator + StreamTransport + Clock
       + RandomSource + FallbackObserver<<<Self as ProcessTracer>::Kernel as Kernel>::Fallback> + FuseServer
       + TcpSockets + UdpSockets + IcmpSockets + RawSockets + RouteResolver + DomainNameResolver + Sync
   {
   }
   ```
   After:
   ```rust
   pub trait System:
       ProcessTracer + ProcessorInputs + MachineDescription + TaskIdAllocator + StreamTransport + Clock
       + RandomSource + FallbackObserver<<<Self as ProcessTracer>::Kernel as Kernel>::Fallback> + FuseServer
       + TcpSockets + UdpSockets + IcmpSockets + RawSockets + RouteResolver + DomainNameResolver + Sync
   {
       type Config;
       fn new(config: Self::Config) -> Self;
   }
   ```
   It would let me delete `LocalSystem::new` and `LocalSystem::serving` (public outside an interface) and the suite's
   `Parts::system` with the `parts` and `failing_parts` closures that hand over a made system (the suite makes it from a
   `Config`), and move the local system to crates/system/implementation/local with the system suite run on it, which
   ends the 29 implementation_folder findings it has in single-kernel.
2. The linter does not parse return type notation: `S: Measured<measure(..): Send>` gives `unparsed: expected ,`.
   Minimal reproduction: SCRATCH/runtime/linter-faults/return-type-notation (with-notation and without-notation:
   reviewable-rust's own examples/conforming plus one async trait; MEASURED: the finding with the notation, none
   without). Seeded-test-policy implementation.rs:336 and the kernel's own `System` need the notation.

## SECURITY

- Lines with `unsafe` (`git grep -w unsafe` per folder), before (main / origin/agents) and now: single-kernel 7 / 0 -> 0,
  runtime suite 0 / 0 -> 0, seeded-test-policy 0 / 0 -> 0. No marked exception; every crate keeps
  `[lints] workspace = true`; no lint lowered, no linter exclusion.
- New dependencies: none from outside the workspace. Single-kernel adds workspace crates behavior-source and
  processor-architecture (for `Interruptible`) and, as dev-dependencies, the runtime suite and the unicorn stand-in
  (GPL-2.0-only, test-only, `publish = false`, without its harness). Against main's copy, single-kernel no longer uses
  `libc` or `polling`.
- Hostile-guest suite: none exists; none run (the thread-test design review is owed to the owner first).
