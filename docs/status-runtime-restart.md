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

2026-11-27: his main FETCH_HEAD ac696ea19 already merged. Merged origin/executor-wasmtime 6221e6d57 (041526ebf; its status
names 9802b9416 as the commit hello and busybox run on, and 6221e6d57 holds it) and origin/kernel-port 353987bad (74f09aff0:
the kernel fills `process_state_controls`, answers the trapped counter reads, and the harness's `runtime` feature runs the
kernel on `WasmtimeExecutor`), both without conflict; so the tip builds with no throwaway fill. Not merged: executor-wasmtime
52406c3bc (newer, its status still names 9802b9416) and executor-restart 922f20713 (status only). After the merges the 84
top-level files of his 29 interface folders are byte-identical to his main (MEASURED, `git diff --quiet FETCH_HEAD HEAD`
per file).

2026-11-27 (afternoon): his main FETCH_HEAD ac696ea19 already merged. Merged origin/kernel-port f02018c89 (ee11386da) and
origin/executor-wasmtime ddbcf4ab3 (b13147f46), both without conflict; the lock file then kept one dependency cargo drops
(`wcontainers-instruction-input-domain-arm64` under behavior-source-recording), removed in 5ddc8da51 so `cargo metadata
--locked` exits 0. After the merges no `interface.rs` or `memory_space.rs` that exists on his main differs from it
(MEASURED, `git diff --name-only ac696ea19 HEAD`: only crates/kvm, crates/linux-socket-options and crates/wasmtime
interface.rs differ, and none of the three is on his main).

2026-11-27 (evening): his main FETCH_HEAD ac696ea19 already merged (`git merge-base --is-ancestor`, MEASURED); no other
branch merged this run.

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

## RT2: containers run through the runners on the stand-in and on wasmtime; the suite passes at 1 and 4 runners on both machines

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

RT2 IS NOT CLAIMED IN FULL (2026-11-27, evening): the five plants now each fail a test against a kernel double (section
"The runtime's own end of a run" below), the whole LTP at full and the bounded targets alone have run; what differs from
j5f is named with one cause and owner each (section "RT2's remainder, measured"), and closing on those waits on the
orchestrator.

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

### The WebAssembly half: containers run on the WebAssembly executor (merged, no fill)

The suite runs on wasmtime from the branch itself (4d868b54c: wasmtime a dev-dependency, a module `on_wasmtime` running the
same suite with `WasmtimeExecutor::new(WasmtimeTranslatorConfig::default())`, which bounds its own slice). A ninth test,
`a_sleeping_process_ends_its_container_after_its_sleep` (d37ea6ad1): a container whose only process sleeps (every runner
idle) ends with its own exit, not earlier. A tenth, `a_host_thread_dying_while_another_computes_ends_the_container`
(d49efb22f): a host thread dies (the system's standard output panics) while a background process computes on another
runner; the container ends as `Panicked`.

Bar at d49efb22f (pushed; MEASURED, rustc 1.99.0, `-j 12`, SCRATCH/runtime-restart/wasmtime/rt2-detached.sh d49efb22f with
no fill; jobs runtime-restart-wasmtime/20261127-065311-372810 ARM64 load 46-61 and 20261127-065342-1318018 x86-64 load
21-45; the same table with 9 tests held at 40f441eb8, jobs 20261127-060711-2502168 and 20261127-060739-3467430):

| | ARM64 | x86-64 |
|---|---|---|
| `cargo metadata --locked`; rustfmt `--check`, three packages | 0; 0 | 0; 0 |
| check and clippy `--no-deps -D warnings --all-targets`, three packages; single-kernel `--no-default-features` | 0 / 0 | 0 / 0 |
| single-kernel unit tests; policy tests | 14 of 14; 13 of 13 | 14 of 14; 13 of 13 |
| runtime suite on the stand-in, 1 runner, three rounds | 10 of 10 each | 10 of 10 each |
| runtime suite on wasmtime, 1 runner, three rounds | 10 of 10 each | 10 of 10 each |
| runtime suite on the stand-in, 4 runners, three rounds | 10 of 10 each | 10 of 10 each |
| runtime suite on wasmtime, 4 runners, three rounds | 10 of 10 each | 10 of 10 each |

The interrupt test (`an_interrupt_from_another_thread_stops_a_run`: `while :; do :; done` ended by
`Interrupter::interrupt` from another thread, twice) passes in each of those 12 runs per machine. Planted, each in the
machine's copy only (MEASURED, both machines, at d49efb22f): `Idle` read as `Finished` fails the sleeping test at 1 runner
(exit 101 on both executors) and 9 of 10 at 4 runners; a runner left running when another finishes or dies (the runtime's
`end_runs` taken out of `stop`, and also the stop left out of `leaving`) fails no test at 1 or 4 runners (exit 0): the
kernel's runner itself returns `Finished` to every runner and returns `Interrupted` at least once per tick of its clock
(READ executing.rs:556-583), so a runner the runtime leaves running comes back within a tick by itself, and the runtime's
own end on a finish or a panic is never the one that acts (INFERRED); no test can tell it from outside without timing;
wasmtime with `slice: None` makes the busy test run past the timeout at 1 runner
and also at 4 (exit 124; at b547d187f it was 1 runner only, cause not looked at: INFERRED the executor's or the kernel's,
for the executor track).

The kernel's harness with `--features runtime` (the kernel on `WasmtimeExecutor`), every target at parallelism 1
(`--test-threads 4` within a target), each bounded at 1800 s (MEASURED, ARM64 only, at 74f09aff0, pushed, which differs
from 40f441eb8 only in the runtime suite; job runtime-restart-harness/20261127-042751-773937, 6,276 s, load 16-93 with the
four modes side by side; throwaway la-switch.sh in the machine's copy so LOOK_AHEAD=full|none reaches wasmtime; bounded is
the executor's default). Logs quiet (`-q`), so the comparison with j5f is per target: the failed tests by name and the
passed and ignored counts (SCRATCH/runtime-restart/harness/compare.py, hv/). Targets that differ from j5f's lists:

| Cause (owner) | Targets and tests | full | none | bounded |
|---|---|---|---|---|
| the test needs the host's kernel on the stand-in: driver.rs:486 "this host's kernel runs on the stand-in" (kernel track, harness) | away 12, edges 7, taskend 7, outside 1 (`a_look_hears_a_reset_the_system_has_not_told_yet`) | 27 | 27 | 27 |
| does not build with `runtime`: embedder (3 x E0308), round_trips (2 x E0308, `ExecutionConfig<WasmtimeExecutor>` vs `StandIn`), repeats (E0432 `wcontainers_executor_wasmtime::reason`) (kernel track, harness tests) | 47 tests not run | 47 | 47 | 47 |
| code written through one shared view runs stale through the other (executor track, wasmtime's translations) | sharedviews `code_written_through_one_view_runs_through_the_other` ("parent after 7", reference 8) | 1 | 1 | 1 |
| restartable sequence preemption differs (executor's slice positions; executor track) | rseqabort `preempted_sections_match_reference` | 1 | 1 | 1 |
| order of wake-ups under computing threads (kernel's time model; kernel track, brief question 3) | deadlines `deadlines_end_waits_while_others_compute` ("child done" before "parent woke") | 1 | timeout | 1 |
| pipe capacity 65536 where the reference has 8192 (kernel track; also on the stand-in, write) | rwmodes `modes_match_reference`, saving `saved_and_brought_back_match_reference` | 0 | 0 | 2 |
| address ceiling, 4 GiB space (brief question 5) | python `system_test_modules_match_reference` (test_mmap around 4 GB); java both ("Error occurred during initialization of VM"); j5f had java threads and python system as FAILED at full too | 0 | 3 | 0 |
| bounded at 1800 s with four modes side by side (no verdict) | backend (also on the stand-in), cpulimit, turns; none also deadlines, threads | 3 | 5 | 3 |
| better than j5f | counterclock 2 of 2 (j5f ignored one); outside `a_full_connection_hears_a_reset` passes at bounded | | | |

Every other target equals j5f's list (the stand-in run equals j5f's stand-in list but for the four bounded targets
backend, layer1, node, python, and write's `faulted_pipe_writes_match_reference`, the pipe capacity above). go and node at
none pass (j5f did not run them there).

LTP on wasmtime at full, the kernel track's 30 (MEASURED, same job): 29 of 30 verdicts equal the committed reference;
timerfd01 fails ("got 15 tick(s) expected 3": the guest's clock ran on while the machine was loaded; kernel's time model,
brief question 4). The whole suite at full: 173 tests run before it was stopped for the
machine's load (job runtime-restart-harness/20261127-063131-2258723), all 173 equal to the committed reference.

Linter (`cargo reviewable check`, ARM64, reviewable-rust da55b9d7, no model key; MEASURED at 40c5888ed, job
runtime-restart-lint/20261127-041348-3394686): single-kernel 234, suite 43, policy 13; `magic_literal` and
`number_constant_without_link` 0 in all three. The structural rules did not run for single-kernel and the policy
(`compiler_view_missing`: the harness tests above do not build with `runtime` in the workspace), so the numbers of the
earlier table stand for those rules. What ran: summary_unanswered 169 / 12 / 9, name_unanswered 42 / 11 / 1,
grammar_unclear 10, summary_unclear 8, conformance_not_run 4 / 8 / 1, public_outside_interface - / 12 / -, unparsed
- / - / 1 (owner line 2).

The shape of `Executor`: the runtime calls no `execute`; `Interruptible<E>` passes `prepare_execution` and `Execution`
through to `E` and keeps a share of the interrupter, so a synchronous `run_until_trap` would change that one forwarding
method (READ interruptible.rs).

### Independent check of the stand-in half (finished 2026-11-27, MEASURED)

A fresh clone of origin/runtime-restart at 40f441eb8 (SCRATCH/check-runtime-1127: clone, scripts/check.sh and plants.sh,
check-arm.log, check-x86.log; jobs rt-check-1127/20261127-060603-2057359 ARM64 load 18-190 and 20261127-060619-3392378
x86-64 load 0-80), on both machines with the same results: `cargo metadata --locked` 0; rustfmt 0; check and clippy
`-D warnings` of the three packages `--all-targets` and of single-kernel `--no-default-features` 0, 0, 0, 0; unit tests 14
of 14; policy 13 of 13; the runtime suite (both executors, 18 tests) 18 of 18 in each of 3 rounds at 1 runner and 3 at 4.
Planted faults: an interrupt dropped fails `a_run_goes_on_after_an_interrupt` on both executors and runs past the
timeout (124) at 1 and 4 runners; a runner's panic read as `Interrupted` runs past the timeout (124) at 1 and 4; `Idle`
read as `Finished` fails 2 tests at 1 runner and 16 at 4 (101); the stand-in without its slice runs past the timeout (124)
at 1 and 4; a runner left running when another finishes (two forms) fails nothing (exit 0 at 1 and 4), the gap above.

### The runtime's own end of a run, ruled by the kernel interface (2026-11-27)

The check found five plants no test caught (two forms of a runner left running when another finishes; `interrupt()` not
calling `end_runs`; `wait` ignoring that the run ended; a panic not outranking an earlier finish), because this kernel
returns `Finished` to every runner and `Interrupted` once per tick by itself. The orchestrator's ruling: decide by what
the interface promises. READ, word by word, crates/kernel/interface.rs and crates/executor/interface.rs at his main
ac696ea19 (byte-identical here):
- `Runner::run`: "Runs what the kernel decides is next, using the given executor." No time, no word on another runner.
- `Kernel::new`: "Makes a kernel that executes at most the given number of threads at the same time, and one runner for
  each." `Kernel` has no method that ends a run; `suspend_tasks` "Takes the given tasks out of this kernel instance
  atomically, ... returning each one that is still executing on it", nothing about runners.
- `StopReason::Interrupted`, `Idle`, `Finished(ProcessEnd)` have no documentation; `Panicked`: "A host thread ended inside
  a run, so the kernel can no longer be run."
- `Execution::run`: "Runs until a trap terminates the execution or the executor must wait." `ExecutionInterrupter::interrupt`
  has no documentation; `TrapHandler`: "The guest stays stopped at the trap while a method runs."; `InterruptResponse`'s
  `Continue` and `Terminate` have none.

No sentence promises that a run returns within a bounded time once one runner finished or an interrupt was asked: that is
a property of this one kernel (executing.rs's tick). So the runtime keeps its own end of a run and is tested against a
kernel that does only what the interface promises: runners.rs's test module (b84c44861) has a `ScriptedKernel`, a
test-only `Kernel` whose runners have no clock: each runs until the execution it runs is ended through the runner stops
the runtime was given (as an execution ends on its executor's interrupter), then returns what its script says
(`Interrupted`, `Idle { until: None }`, or its host thread panics); one runner returns `Finished` once the other runs.
The one thing it assumes beyond the interface (INFERRED, the runtime's whole means of ending a runner): a runner whose
execution is interrupted returns from `run`. Four tests, each bounded at 30 s:
`a_finish_ends_a_runner_that_runs_until_its_execution_is_ended`,
`a_finish_ends_a_runner_that_answers_idle_once_its_execution_is_ended`,
`an_interrupt_ends_runners_that_run_until_their_executions_are_ended`,
`a_panic_after_a_finish_ends_the_container_as_panicked` (the last is the runtime's alone, as ruled).

Bar at 5ddc8da51 (pushed; MEASURED, both machines, jobs runtime-restart-bar2/20261127-163200-4064781 ARM64 load 28-53 and
20261127-163220-3088372 x86-64 load 33-40; the same at b84c44861 on x86-64, 20261127-125502-2769369):

| | ARM64 | x86-64 |
|---|---|---|
| `cargo metadata --locked`; rustfmt `--check`, three packages | 0; 0 | 0; 0 |
| check, clippy `-D warnings --all-targets`, three packages; clippy single-kernel `--no-default-features` | 0, 0; 0 | 0, 0; 0 |
| single-kernel unit tests; policy tests | 18 of 18; 13 of 13 | 18 of 18; 13 of 13 |
| the four double-kernel tests, three rounds | 4 of 4 each | 4 of 4 each |
| runtime suite on the stand-in and on wasmtime, 1 runner and 4, three rounds each | 10 of 10 every time | 10 of 10 every time |

Planted in the machine's copy, one sed line each (MEASURED, both machines, exit 101 every time, after the 30 s bound where
a runner is left):

| Plant | Fails |
|---|---|
| `end_runs` taken out of `stop` (a runner left running, first form) | both finish tests and the panic test |
| `stopped` taken out of `leaving` (a runner left running, second form) | both finish tests |
| `interrupt()` not calling `end_runs` | the interrupt test |
| `wait` not returning once the run ended | the idle finish test and the interrupt test |
| a panic not outranking an earlier finish | the panic test |

Linter (`cargo reviewable check`, reviewable-rust da55b9d7, x86-64, no model key; MEASURED): the new test module adds only
`summary_unanswered` findings (11, one per new summary: no key, so nothing was asked); no `magic_literal` or other rule.

### The check's six surviving plants, and the kernel call taken out (2026-11-27, evening)

The check of 6011f8560 found six plants in the stop code that no test caught. Each is now either tested against the
interface-only double or removed:
- A (`stop` without `notify_all`: a runner waiting idle when another finishes) and B (`interrupt` without `notify_all`: two
  idle runners and an interrupt): `a_finish_wakes_a_runner_that_waits_idle` and `an_interrupt_wakes_runners_that_wait_idle`.
  The idle poll is an hour, so a runner left asleep is seen at the 30 s bound.
- F (a finish after a panic replaces it): `a_finish_after_a_panic_ends_the_container_as_panicked`. The right value is
  `Panicked`: its documentation says "A host thread ended inside a run, so the kernel can no longer be run".
- C and D (`end_runs` without its kernel call, or without its loop): REMOVED. Nothing in the kernel interface makes the
  call do anything a caller can see (READ, crates/kernel/interface.rs at his main ac696ea19): `suspend_tasks` "Takes the
  given tasks out of this kernel instance atomically, ... returning each one that is still executing on it". With no
  task that is nothing, and no sentence says a call ends a runner's `run`. That was a property of this one kernel's
  lock. `Interrupter::interrupt` promises only that it "interrupts a container's run, idempotently", not that the run has
  ended when it returns. So the runtime now ends a run only through the executions (`RunnerStops::stop`, which stays in
  force until every runner is back) and its own condition variable. The `inside` count, `call_kernel` and the kernel held
  in an `Arc` went with it. `drive` keeps the `leaving` check: an end asked after that check still ends the run, because
  the executions stay ended until `resume`.

The check's non-blocking points, fixed in the same commit:
- O (a later finish replaces an earlier one): `stop` now says "the first finish stands", tested by `the_first_finish_stands`.
- P (`Suspended` ends the runner): `a_suspended_runner_is_run_again`.
- M (the idle wait ignores `until`): the double's system has a clock that stands at zero and keeps the warnings it is told
  of. Four tests check the idle wait against `IDLE_WITHOUT_WAKE`'s documentation ("run again before the time the kernel
  gave"; its detail is now documented as the idle poll in nanoseconds). The tests are
  `an_idle_runner_is_run_again_at_the_time_it_was_given`,
  `an_idle_runner_run_again_at_the_idle_poll_itself_is_not_early` (no warning),
  `an_idle_runner_with_no_time_is_run_again_after_the_idle_poll_and_told` (count 1) and
  `an_idle_runner_whose_time_is_past_the_idle_poll_is_run_again_early_and_told` (count 2).
- Also new: `a_run_after_an_interrupt_runs_the_runners_again`, `a_container_that_panicked_is_not_run_again` (one run in
  all), and `the_idle_polls_are_told_again_only_when_they_grew`.
- The merge message that names b2594b6d9 for kernel-port's f02018c89 stays: changing it would need a force push.

The double's runners now take one step per run (finish behind a gate, run until ended then answer, idle or suspend at
once, panic). The interrupting thread waits at most the run bound and says whether it interrupted, so a runner that
never runs fails the test instead of hanging it.

Bar at 04b3b47e2 (pushed; MEASURED, jobs rt3-bar/20261127-211509-2545588 ARM64 load 98-188 and 20261127-211448-2705185
x86-64 load 15-22; the product code is the same as at 455d9d2cf, where the same bar ran on both machines first, jobs
20261127-191429-2052753 and 20261127-191447-279957):

| | ARM64 | x86-64 |
|---|---|---|
| `cargo metadata --locked`; rustfmt `--check`, three packages | 0; 0 | 0; 0 |
| check, clippy `-D warnings --all-targets`, three packages; clippy single-kernel `--no-default-features` | 0, 0; 0 | 0, 0; 0 |
| single-kernel unit tests; policy tests | 30 of 30; 13 of 13 | 30 of 30; 13 of 13 |
| the 16 double-kernel tests, three rounds | 16 of 16 each | 16 of 16 each |
| runtime suite on the stand-in and on wasmtime, 1 runner and 4, three rounds each | 10 of 10 every time | 10 of 10 every time |

Mutation testing (MEASURED): cargo-mutants 27.1.0, installed on the machine only (not in any lock file), over runners.rs,
`--in-place`, tests `cargo test -p wcontainers-runtime-single-kernel --lib`. Results:
- At 04b3b47e2 on x86-64 (job rt3-plants/20261127-210442-856298) and at 204b70c60 on ARM64 (rt3-mut/20261127-200534-3370248):
  "21 mutants tested: 14 caught, 7 unviable", 0 missed, 0 timeouts.
- The 7 unviable ones replace `system`, both `run`s and `lock` with values that do not compile (no `Default` for
  `StopReason`, `&S`, `MutexGuard`).
- The first round at 455d9d2cf had two timeouts (`drive` and `leaving` made to do nothing hung the interrupting thread);
  the bounded interrupting thread turned both into caught.
- Outcomes: SCRATCH/runtime-restart/rt3-results/mutants.out.

The check's six plants plus O, P and M, as sed lines in the machine's copy (MEASURED, x86-64, same job at 04b3b47e2):

| Plant | Exit | Fails |
|---|---|---|
| A: `stop` without `notify_all` | 101 | `a_finish_wakes_a_runner_that_waits_idle` (30 s bound) |
| B: `interrupt` without `notify_all` | 101 | `an_interrupt_wakes_runners_that_wait_idle` (30 s bound) |
| F: `Some(_) => true` | 101 | `a_finish_after_a_panic_ends_the_container_as_panicked` |
| O: a later finish replaces an earlier one | 101 | `the_first_finish_stands` |
| P: `Suspended => return` | 101 | `a_suspended_runner_is_run_again` |
| M: the idle wait ignores `until` | 101 | the two tests of a runner given a time |
| C, D | | the code is gone |

F and O survived once at 204b70c60, because the second runner could see the end before it ran. Both tests now start the
second runner before the first ends.

Linter (`cargo reviewable check`, reviewable-rust da55b9d7, x86-64, no model key; MEASURED, job rt3-plants/20261127-204532-3596183):
in single-kernel's runners.rs, beyond `summary_unanswered` and `name_unanswered` (no key, so nothing was asked), there are
only the three findings it had before: `public_outside_interface` (`RunInterrupter`) and `conformance_not_run` ×2.

### RT2's remainder, measured (2026-11-27)

The look-ahead switch is committed: tools/runtime-modes/la-switch.sh (852859ad5). In a throwaway copy it makes the harness's
`runtime` feature read `LOOK_AHEAD` (full, none, bounded; bounded when unset, the WebAssembly executor's own default), as
the feature's comment in the stand-in's Cargo.toml says; driver.rs's `make_back` is the kernel track's, so the script
changes the copy only.

LTP's whole suite on wasmtime at full (MEASURED, ARM64: the x86-64 machine cannot run it, since every test's file backend
and the reference are linux/arm64 containers and that machine runs none: `exec /bin/uname: exec format error`; job
runtime-restart-ltp/20261127-124227-1970472 at 852859ad5, release build, `LTP_JOBS=12 LTP_LIMIT=600`, 11,162 s, load
27-193; results and log SCRATCH/runtime-restart/rt2b/ltpres; the translation folder and all its build 2.3 GB): 4,226 tests,
4,164 equal to the committed reference, 62 differ. Against j5f's stand-in (whole.tsv with redo.tsv) 17 differ:
- better (7): dio30, ADSP037, ADSP038, ADSP073, ADSP074, fsx22 pass where the stand-in timed out.
- math (5): float_bessel, float_exp_log, float_iperb, float_power, float_trigo time out at 600 s; rerun at j5f's 900 s
  (job 20261127-154914-3741884, `LTP_JOBS=4`, load 30-95): iperb passes in 695 s and power in 834 s, the other three time
  out (the stand-in passes them in 46 to 212 s): floating point runs about 10 times slower on wasmtime at full (INFERRED:
  translator and executor tracks).
- sched_football: timeout at 600 s, broken at 900 s as the stand-in and the reference.
- the clock (4, kernel track's time model and the executor's pauses; brief question 4): clock_gettime04 "Difference
  between successive readings greater than 5 ms (1): 19", epoll_wait04 "waited for 17957us with a timeout equal to zero",
  timerfd01 "got 14 tick(s) expected 3", getrusage04 "utime increased > 2000us"; each fails again at 900 s alone.
- read_all_dev: "Worker timeout is too short; restarts take >3051045us" (process start on wasmtime; executor track).
fcntl12, fcntl12_64 and fork09 time out as on the stand-in.

The targets bounded at 1800 s, rerun one at a time (MEASURED, ARM64, job runtime-restart-alone/20261127-125559-1820800 at
b84c44861, `--test-threads 4`, load 35-168):

| Target | bounded | full | none |
|---|---|---|---|
| backend | 19 of 21, j5f's two (file_capabilities, special_files), 63 s | the same, 198 s | the same, 23 s |
| cpulimit | past 1800 s | past 1800 s | past 1800 s |
| turns | past 1800 s | past 1800 s | past 1800 s |
| deadlines | | | past 1800 s |
| threads | | | past 1800 s |

cpulimit and turns run past the bound alone, in every mode, so load is not the cause (deadlines and threads at none run
past it alone too: look-ahead none translates one instruction per piece; INFERRED slowness, not a verdict). INFERRED (READ driver.rs:160-171):
with `runtime` the harness's repeatable clock moves by the instructions the executor ran, and `last_instructions` is 0 for
the WebAssembly executor, so a guest that computes never uses CPU time: RLIMIT_CPU's signal never comes (cpulimit) and a
thread's share never ends (turns). Kernel track (harness) with the executor track (an instruction count from wasmtime).

rwmodes and saving at bounded, three times each alone (same job, load 41-56): rwmodes 1 of 1 every time (2 to 3 s), saving 1
of 1 every time (95 to 97 s). Why they failed in the four-mode run
and passed in the check's: neither pipe capacity of ours nor load as such. rwmodes prints how much a 70,000-byte
`RWF_NOWAIT` write puts into a new pipe; ours is 65,536 (Linux's default of 16 pages of 4 KiB), and the reference said
8,192 in the failing run. READ (Linux fs/pipe.c, `alloc_pipe_info`): a user without CAP_SYS_RESOURCE whose pipes already
hold more than `fs.pipe-user-pages-soft` pages (16,384 on the ARM64 machine, MEASURED) gets 2 pages (8,192 bytes) per new
pipe. Every Docker container on the shared machine is root without that capability, so the reference's answer depends on
how many pipe pages all containers of all agents hold at that moment: four modes side by side (each with its file backends
and references) reached it, the check's single run did not. A probe (a container's pipe filled by a writer that stops when
it blocks) gave 65,536 bytes at each of 18 moments of this run (MEASURED). Saving's five programs compare whole outputs
with the reference too (INFERRED: the same cause).

## Next

- RT2, what is left (2026-11-27 evening): every measurement of the bar has now run. Not equal to j5f and the stand-in,
  each with one cause and owner: the 27 driver.rs:486 tests and the three targets that do not build with `runtime`
  (kernel track, harness); cpulimit and turns, which never end with `runtime` because the WebAssembly executor counts no
  instructions (kernel track's harness clock with the executor track); sharedviews and rseqabort (executor track);
  LTP's 10 worse verdicts (math floating point about 10 times slower: translator and executor tracks; four clock tests:
  kernel track's time model, brief question 4; read_all_dev's process start: executor track). The orchestrator decides
  whether these named tests close RT2; then RT2's independent check.
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
5. Kernel track: the runtime no longer calls `suspend_tasks(&[])` to make runners leave `run` (2026-11-27 evening; the
   interface promises nothing of it). A runner must return once its execution is ended through the executor's
   interrupter, and today's kernel does so within a tick.
6. Kernel track: done on kernel-port 353987bad (merged here): running.rs fills `process_state_controls`.
7. Kernel track (harness): with `--features runtime`, 27 tests panic at driver.rs:486 "this host's kernel runs on the
   stand-in" (away 12, edges 7, taskend 7, outside 1), and embedder, round_trips and repeats do not build (E0308 x5,
   E0432 `wcontainers_executor_wasmtime::reason`); either run them on wasmtime or mark them stand-in only.
8. Executor track (wasmtime): sharedviews `code_written_through_one_view_runs_through_the_other` runs the old code through
   the second view ("parent after 7", reference 8); rseqabort `preempted_sections_match_reference` differs; with
   `slice: None` a container with a busy background process does not end at 4 runners either.
9. Kernel track: the pipe-capacity line here before was wrong: ours is Linux's default, and the reference gives 8192 only
   when all of the machine's containers hold more than `fs.pipe-user-pages-soft` pages of pipes (Linux's per-user limit);
   rwmodes and saving pass alone (section "RT2's remainder"). LTP timerfd01, clock_gettime04, epoll_wait04, getrusage04
   fail on wasmtime (the time model, brief question 4).
10. Kernel track (harness): with `runtime` the repeatable clock moves by `last_instructions`, which is 0 for wasmtime
   (driver.rs:160-171), so cpulimit and turns never end; and `make_back` ignores `LOOK_AHEAD`, which the feature's comment
   promises (tools/runtime-modes/la-switch.sh is the change, for a throwaway copy).
11. Translator and executor tracks: LTP's math floating-point tests run about 10 times slower on wasmtime at full than on
   the stand-in (float_iperb 695 s against 46 s); read_all_dev's worker restarts take over 3 s.

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
  runtime suite 0 / 0 -> 0, seeded-test-policy 0 / 0 -> 0.
  2026-11-27 evening, db8b17712 -> 5ddc8da51 (MEASURED, `git grep -w unsafe <commit> -- <folder>`): single-kernel 0 -> 0,
  runtime suite 0 -> 0, seeded-test-policy 0 -> 0, tools/runtime-modes (new, one shell script) 0. No marked exception; every crate keeps
  `[lints] workspace = true`; no lint lowered, no linter exclusion.
- New dependencies: none from outside the workspace. Single-kernel adds workspace crates behavior-source and
  processor-architecture (for `Interruptible`) and, as dev-dependencies, the runtime suite and the unicorn stand-in
  (GPL-2.0-only, test-only, `publish = false`, without its harness), and since 4d868b54c the WebAssembly executor
  (workspace crate, test-only; it brings wasmtime, Apache-2.0 WITH LLVM-exception, already in the lock file). Against main's copy, single-kernel no longer uses
  `libc` or `polling`.
- Hostile-guest suite: none exists; none run (the thread-test design review is owed to the owner first).
