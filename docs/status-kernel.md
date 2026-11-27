# Kernel status

Plan: docs/phase-1-kernel.md (section 14). Branch `kernel-port`, worktree
wcontainers-worktrees/kernel-port, cut from `origin/instruction-port` at b69a2d39f. The old status
(8,831 lines) stays on origin/agents. Scratch: pyro-parked/wcontainers-scratch/kernel-port.

## KP1, the kernel on the committed interfaces (2026-11-25, first run)

Done:
- The kernel and its suite came back from origin/agents (c86ddeb60's code) as they were, in a commit
  of their own. The port changed nothing in those folders (`git diff main b69a2d39f` on them is
  empty), so no port hunk needed re-applying.
- Fit, in the next commit: each guest space's `PageTable` is held beside its memory by the guest
  machine alone and taken `&mut`; the turn reads the port's `Processor<()>` and `Exception` enum
  (an exception kept whole as a syndrome gives the exit the old syndrome match gave); the unit-test
  executor is a `BehaviorSource` and `Executor` with an absent `PageTable`; the base's abi still
  has `Event::BackendNotice`, which is applied as main's kernel applied it.
- Three private traits went: siginfo.rs `Code` (one number function per code type), ipc.rs
  `Object` (each kind's `Ids` keeps where its objects hold their `Perm`), touch.rs `Memory` (the
  walk takes `CallMemory` itself).

Measured (rented machines, `-j 12` except the x86 library check, which ran at 96):
- `cargo check` and `cargo clippy --no-deps -- -D warnings` of `-p wcontainers-kernel-linux --lib`:
  exit 0 on ARM64 and on x86-64.
- The unit tests, in a copy without the stand-in dev-dependency: 26 passed on ARM64 and x86-64;
  the same 26 pass at c86ddeb60 on ARM64.
- No line of code with `unsafe` (one comment line in the manifest, as at origin/agents).

Linter, repair after the failed check (2026-11-25): 31,840 findings became 31,285 (scratch
kp1-repair/lint/summary.txt). 404 items no other crate can reach went from `pub` to `pub(crate)`
(rustc's `unreachable_pub` suggestions, 538 findings), and 8 constants that restated a binding now
name it (`SIGNALS`, `ROBUST_LIST_LIMIT`, `MAX_LINKS`, the four `SS_*`). The rest cannot be
cleared in KP1. Each group, with its smallest example:

| Findings | Cause | Smallest example |
|---|---|---|
| 20,898 | generated bindings: a rule question | `linux/general.rs:1832 pub const ROBUST_LIST_LIMIT: u32 = 2048;` is pub, has no link, and is also in another binding file |
| 1,083 `public_outside_interface` | the crate's surface, which other tracks' crates use | `implementation.rs:60 pub use cgroup::MOUNT as CGROUP_MOUNT;` |
| 666 | the model's answer is needed, and its key may not go to a rented machine | `conformance/suite.rs:64` `taken_tasks_are_resumed`, name_unanswered |
| 8,619 | rewriting the comments and literals of 74,000 lines | `build.rs:11 "cargo::rerun-if-changed={source}"`, magic_literal |
| 3 | the `System` trait's `Send` bounds | `system.rs:63 wait_until_ready(..): Send` |
| 14 | later milestones: the `Cpu` adapter (KP2), the suite (KS) | `guest_machine.rs:164` implements abi `Cpu` |
| 1 | a rule question: the executor's interface asks its caller to implement `TrapHandler` | `running.rs:31` implements `TrapHandler` |
| 1 | the same fact is in a crate the kernel does not depend on | `running.rs:229 ALIGNMENT_FAULT = 33`, also hardware-virtualization-inside syndrome.rs:7 |

The 666 are grammar_unclear, verb_not_first, name_unanswered, summary_unclear,
summary_unanswered, summary_restates_declaration, type_summary_not_noun_phrase and hook_name.
The 8,619 are magic_literal, number_constant_without_link, the summary shape rules,
comment_too_long and comment_budget_exceeded. Every rewritten summary of a function or type
also needs the model's answer.

## For the owner

1. The kernel's own `System` trait (system.rs:46) can go only when the hooks are the interface's
   `ProcessTracer` and that trait is generic over the kernel, so a hook's process can be suspended
   and kept:

```rust
pub trait ProcessTracer<K: Kernel> {
    type ProcessState: Clone + Send;

    fn intercept_system_call_request<P: ProcessOf<Self::ProcessState, K>>(
        &self,
        process: P,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        ControlFlow::Continue(request)
    }
    // ... every hook the same, with `K` for `Self::Kernel`
}

pub trait System<K: Kernel>: ProcessTracer<K> + FallbackObserver<K::Fallback> + /* ... */ Sync {}
```

2. The kernel's `System` bounds its waits' futures as `Send` with return type notation
   (system.rs:63, `#![feature(return_type_notation)]` at implementation.rs:13), which the linter
   cannot parse and calls an exclusion. Without it, the interfaces would say `Send` themselves:

```rust
pub trait Readiness<Handle> {
    fn wait_until_ready(
        &self,
        handle: &Handle,
        interest: ReadyFor,
    ) -> impl Future<Output = Result<ReadinessReport, HostError>> + Send;
}

pub trait FuseServer {
    fn send_request(&self, request: &[u8]) -> impl Future<Output = Result<Self::Reply, HostError>> + Send;
    fn receive_notice(
        &self,
        applied: Option<Result<(), Errno>>,
    ) -> impl Future<Output = Result<Self::Notice, HostError>> + Send;
    // ...
}

pub trait DomainNameResolver {
    fn resolve_domain(
        &self,
        question: Question<&Name<[u8]>>,
    ) -> impl Future<Output = Result<Message<Self::AnswerBytes>, HostError>> + Send;
    // ...
}
```

## 2026-11-25, evening: his main `95bceadf0`, and the embedder calls built

- Merge `64013e048` of his main: no conflict in the kernel. Four conflicts outside it (two suite
  manifests, the architecture suite's manifest, the vector sink's file) were taken as the port has
  them, the vector file as his main has it; his `processor-architecture/interface.rs` is restored to
  his main's bytes (the merge had put his `foreign_implementation` module in twice).
- The library does not build on his main yet: `wcontainers-processor-architecture-arm64` (the
  port's) has no `ProcessorStateExpectation`, `meets_expectations` or `Simulator::simulate` (6
  errors, MEASURED). The kernel waits on the port for that; its own code was checked in a copy at
  the pre-merge commit `190b9e1e3`, whose interfaces the library uses unchanged.
- The unit-test executor `Plain` takes the `MemorySnapshot` view.
- `Process::watch_thread_memory` is built (it was `unimplemented!`): each memory's watches keep,
  beside those of every thread, each thread's own; the CPU side holds the own watches of the thread
  readied for its turn last (the runner readies each turn), and the kernel's copies, the touches
  of a call and the faults meet the watches of the thread they are for. A thread's own watches go
  with their pages on `mremap`, end on unmap and with the thread, and a fork's child has none.
- `on_inspection_starting` and `on_inspection_ended` are announced (they were never fired): when
  the kernel settles, the processes another process holds open through `/proc/<pid>/mem` are
  compared with those last told, once such a file has been opened.
- Measured in the copy (ARM64 and x86-64): library clippy `--no-deps -D warnings` exit 0, rustfmt
  clean, unit tests 30 passed (26 before, 4 new on the per-thread watches).
- Every other call of `Kernel` and `Process` was already built; `KernelWithExecutor` is shared
  across threads through one `spin::Mutex` and `atomic_refcell` records, with no `unsafe`.

## 2026-11-25, night: the arm64 crates of today, and KP1 on his main `1b6b5d6dc`

- Merge `6a0f9ca5d` of `origin/arm64-instructions` `d708c4a55` and merge `3502499e8` of his main
  `1b6b5d6dc` (the processor core interface): no conflict. Every `interface.rs` of his main and every
  file beside one is byte-identical to his main (MEASURED, 29 of 29; the branch's 30th,
  crates/kvm/interface.rs, is the native track's outside-API crate, not on his main).
- The kernel's fit to today's arm64 crate: a turn's processor state starts with its exclusive
  monitor open (the registers keep none), and the three new exceptions (`SupervisorCall`,
  `WaitTrapped`, `SystemAccessTrapped`) end a turn as the old crate's syndromes of them did: with
  the panic for an exception no exit names.
- KP1 holds on today's main (MEASURED, ARM64 and x86-64, `-j 12`): `cargo metadata --locked`,
  `cargo check` and `cargo clippy --no-deps -- -D warnings` of `--lib` of both kernel crates, and
  rustfmt, exit 0; 0 `.rs` lines with `unsafe` in crates/kernel. Unit tests in the copy without
  the stand-in: 30 passed on both.
- New suite cases: `thread_watched_memory_is_read_and_written`, `thread_watch_leaves_the_process`
  (both need an executor with `PerThreadWatch`) and `embedder_access_is_not_an_inspection`, in the
  process group as the linter's fixed shape of `tests!` requires. So the stand-in of X3 must
  implement `PerThreadWatch` for the kernel's suite to build.
- The harness (built only with X3): its `Hooks` has the two inspection hooks and the driver passes
  them on; the inspection test checks the order of yields and hooks; embedder.rs's
  `watch_thread_memory` call takes today's unit return.
- Linter, kernel crates: 31,321 findings (31,293 at the level run). New in this run's lines: 3
  `name_unanswered` and 3 `summary_unanswered` in suite.rs, which need the model's key. Logs:
  SCRATCH/kernel-port/merge-arm64/.

### KP2's first step waits on the size of a space

The kernel's executor must still implement the abi's `Cpu`, for one call: the guest machine reads
the executor's `address_space()` to copy a whole space on fork, bound a shared block, and cap the
kernel's ceiling. Neither `Executor` nor `MemorySpace` tells a space's size, and the kernel's own
`Start` is built by other tracks' crates. A way the interface could tell it:

```rust
pub trait MemorySpace: Sized {
    /// Returns the addresses the space holds, from 0.
    fn addresses(&self) -> Range<u64>;
    // ...
}
```

## 2026-11-26: KP1 on his main `e6672b3eb` (named iterators, `MemoryReader`, `Execution`)

- Merge `5146e48b3` of his main: no conflict. Every `interface.rs` of his main and every file beside
  one is byte-identical to his main (MEASURED, 100 files in the 29 interface folders). His main's
  lock has no entry for the two observation-collector crates; the branch's lock adds just those two
  (`cargo metadata --offline`).
- Fit (`67c7225a4`): `Kernel::Runners`, `SuspendedTasks<'_>`, `UnresumedTasks<'_>` and
  `Process::SuspendedTasks` are `vec::IntoIter`s of what was collected before; `read_memory` hands
  the range to the reader as one part; the unit-test executor's `Execution` is `Ready<Processor>`.
  The suite reads through a `MemoryReader` that joins the parts. The turn (running.rs) was already
  one "run until the next trap, then the kernel answers" step: `execute` with a handler that ends
  at the first trap, polled to its end, then the trap answered outside the execution.
- MEASURED, ARM64 and x86-64, `--jobs 12`: `cargo metadata --locked`, rustfmt `--check`, and
  `cargo check` and `cargo clippy --no-deps -- -D warnings` of `--lib` of both kernel crates exit 0;
  0 `.rs` lines with `unsafe` in crates/kernel; unit tests in the copy without the stand-in: 30
  passed on both. Logs: SCRATCH/kernel-port/kp1-main-1125/.
- Linter: the kernel's own crates 31,316 findings (crates/kernel with its interface 31,326; 31,321
  at the last run). New in this run's lines: one `summary_unanswered` (suite.rs, the reader that
  joins the parts), which needs the model's key.

## For the owner (one line each)

3. The kernel calls the legacy abi's `Cpu` on its executor only for `address_space()` (the size of a
   space: copy a whole space on fork, bound a shared block, cap the ceiling), and no interface of
   his gives it: `MemorySpace` would return the addresses the space holds
   (`fn addresses(&self) -> Range<u64>;`).
4. The suite cases with a `PerThreadWatch` bound (`thread_watched_memory_is_read_and_written`,
   `thread_watch_leaves_the_process`) must sit in the one `tests!` macro the linter fixes, so every
   kernel that runs the suite needs an executor with per-thread watches.

## 2026-11-26: KP2, the kernel's own part (`f39e9cec8`, `9e3b1d4a1`)

- The internal `Cpu` adapter is gone: `GuestMachine<C>` no longer implements the abi's `Cpu`; its
  calls are its own methods (verb first: `make_memory`, `duplicate_memory`, `map_shared`, ...), and
  the call context, the call memory, the guest copy and the hooks' process handle carry the
  executor type `C` instead of `&mut dyn Cpu`. The size of a space is read once, in
  `GuestMachine::new`, with the one legacy call `executor.address_space()` (owner line 3), so
  `C: Cpu` stays only where the kernel is made (`start`, `Kernel` and the `Process` impls).
- `Host` is gone: the kernel's `System` asks for the system interface's
  `FallbackObserver<Warning<'static>>`, and each counted fallback is told through
  `on_fallback_used`. An embedder's `Host::warn` becomes `on_fallback_used`.
- The old drive is gone from the kernel's surface: `next` and `exit` are the runner's alone
  (crate-private), on the kernel's own `Next` and `Exit`; `event` is split into the runner's
  `fire_timers` and a public `resize_terminal` (no interface carries a resize from outside); the
  abi's backend notice event went, as the kernel waits for notices itself. `Exit` has no fault
  without registers: no executor of the interface makes one.
- The runner's turn stays one clear step: run until the next trap (`execute`, a handler that
  ends at the first trap, polled to its end), then the kernel answers the trap.
- MEASURED, ARM64 and x86-64, `--jobs 12`: `cargo metadata --locked`, rustfmt `--check`, and
  `cargo check` and `cargo clippy --no-deps -- -D warnings` of `--lib` of both kernel crates exit 0;
  0 `.rs` lines with `unsafe` in crates/kernel; unit tests in the copy without the stand-in: 30
  passed on both. `git grep` of the abi's `Cpu`, `Host`, `Event`, `Next`, `Exit` in crates/kernel:
  only `Cpu`, in executing.rs (the bounds above) and guest_machine.rs (the one call).
- Linter, the kernel's own crates: 31,322 findings (31,316 before): 12 more `name_unanswered` and
  8 more `summary_unanswered` (the renamed methods and new types need the model's key), every other
  rule the same or fewer; the `Cpu` implementation_folder finding is gone. Logs:
  SCRATCH/kernel-port/kp2/.
- Waiting: the harness (driver.rs, replay.rs, streams.rs, ltp.rs and the tests) still drives the
  kernel through `next`, `exit`, `event` and `&mut dyn Cpu`; it moves to `Runner::run` when it can
  build, with the stand-in of the executor track's X3. Other tracks' crates that call the old drive
  or implement `Host` for the kernel: the runtime (single-kernel), seeded-test-policy, and the
  hardware-virtualization executors.

## 2026-11-26: his main `ac696ea19` (`prepare_execution`, `Execution::run`)

- Merge `b60df9441`: no conflict; 100 files in the interface folders byte-identical to his main.
- Fit (`4bea40c2f`): the turn is a loop of "prepare the execution, run it until the next trap,
  which the handler ends it at"; an `ExecutionStop::Waiting` is polled to its end and the execution
  prepared again from the processor state it stopped at (INFERRED from the interface: `run`
  consumes the execution), each new interrupter handed to the kernel's lock. The unit-test
  executor's execution is the uninhabited `Absent`.
- MEASURED, ARM64 and x86-64, `--jobs 12`: metadata `--locked`, rustfmt `--check`, `--lib` check and
  clippy `--no-deps -D warnings` of both kernel crates exit 0; 0 `unsafe` lines; unit tests 30
  passed on both (then one test-only doc comment shortened, rechecked on ARM64). Linter, the
  kernel's own crates: 31,319 (31,322 before this fit). Logs: SCRATCH/kernel-port/exec-1126/.

## 2026-11-26: KS, the harness on the kernel's runner (first guest programs end to end)

- Fast-forward to `origin/executor-restart` 62d137472 (the stand-in on today's interfaces); every
  file of his main's interface folders byte-identical to his main ac696ea19 (MEASURED, 90 files).
- Kernel (`61a003c87`, `49f659c08`): `KernelWithExecutor::with_runners` and `runner`, and
  `KernelRunner::run_on`, need no `Send` executor (the interface's `new` and `run` call them), so a
  stand-in stays on the host thread that made it; `Process::on_executor` names a hook's executor;
  the runner settles before it reports the end; a system call's program counter is taken as the
  executor leaves it (taken after its instruction: the turn added 4 again, MEASURED as hello's
  second write skipped); a start that waits gets its first process's data once it exists.
- Harness (`90b0fec53`, `38f65e53a`): driver.rs runs every container through `run_on` with the
  outside's work between runs; the abi's `Cpu`, `Host`, `Event`, `Next`, `Exit` and the runtime's
  `drive`, `Execution`, `Turn` are gone from driver, streams, replay, hooks and ltp; the four traits
  (`Hooks` stays as the tests' own boxed hooks, `BetweenSteps`, `DigestMemories` use, `StreamHost`)
  are down to `Hooks` and `DigestMemories` (the latter is the stand-in's file); the LTP runner
  starts each test as this program again (`std::process::Command`), no `fork`/`_exit`. Saving (a
  copy made by `fork`) panics with the reason; layer 1's old-drive calls panic with the reason.
- MEASURED (ARM64, job kernel-port/20261126-071122-3199251, 07:11 to 09:32 UTC, load 8 to 35, three
  targets at a time, 4 test threads, 2,400 s per target): kernel unit tests 30 passed; kernel clippy
  `--lib -D warnings` 0; harness tests 257 passed, 92 failed, 8 ignored, 8 targets timed out
  (cpulimit, cputime, deadlines, edges, processes, repeats, threads, turns). Of fails16's 35, 34
  fail; the 35th (proc_task) is in the timed-out threads target. Rerun with the start fix (job
  kernel-port-b): the 22 start failures are gone but the 5 on the known list. New failures left:
  saves 26 (fork), layer 1 1, and 11 others: sharedmap 2, sharedviews 3, privatemap 1 (views of
  one block disagree in the stand-in), rseqabort, aio poll, procstatus fdinfo, embedder
  compute-used (no time passes in a turn: the runner keeps no slice), streams
  input_arrives_while_the_reader_waits (epoll does not hear the input closed). Logs:
  SCRATCH/kernel-port/ks/.
- LTP: the whole job's 30 all crashed (before the start fix). With the fix, a run stopped after 12:
  mprotect01, sendto01, recvfrom01, listen01 pass as the reference; fork01, clone04, vfork01,
  mmap01, munmap01, pipe01, pipe2_01, socket01 hang (TPASS lines, then LTP's own timeout: the
  parent never sees its child end; INFERRED: the shared memory LTP's library keeps its results and
  checkpoints in, as the sharedviews failures show).
- Not building: tests round_trips, runners and the kernel's conformance (the interface's `Kernel`
  asks a `Send` executor; the conformance also `PerThreadWatch`, which the stand-in lacks);
  guest_edges (the old executor surface). Clippy of the harness: one finding in standin.rs
  (`refused_in_a_copy` unused, the executor track's file).

## 2026-11-26, afternoon: the check's repair, the rulings, KS at a pushed tip

### Repair (the check did not hold: the pushed tip built no test)
- Merges, normal, no conflict: `7d7baa5fa` of `origin/runtime-restart` 93637d42e, `5540e025b` of
  `origin/executor-restart` 9aaaf77a3 (its tip; it carries the move of wasmtime behind the
  `runtime` feature, `5bfd6d5ad`, which was the local change). His main `ac696ea19` was already in
  (`git fetch` of his checkout: FETCH_HEAD is an ancestor). The 91 files of his interface folders
  are byte-identical to his main (MEASURED, `git diff --quiet` on each). The two `interface.rs` not
  on his main (crates/kvm, crates/linux-socket-options) are other tracks'.
- Every number below comes from a fresh clone of the pushed tip (`36bf10eed`), run as job
  kernel-port-ks2 (scratch kernel-port/ks2/clone, `ks-whole.sh`, `ltp30.sh`).
- The stand-in now has predicate registers and FFR in its processor: the kernel keeps them per
  thread across interrupts and faults and drops them at a system call, as Linux discards them
  there (`789862313`).

### The check's findings
1. Fixed above.
2. `wcontainers_abi::Cpu` is gone from the harness and from the kernel: the kernel no longer reads a
   space's size from the executor. The start's `ceiling` (its doc already says: the highest address
   the executor can give) bounds the guest machine's spaces; the harness gives the stand-in's end as
   the ceiling. `git grep -n '\bCpu\b'` in crates/kernel and in driver.rs: none (MEASURED).
   FOR THE RUNTIME TRACK: `container.rs:202` passes `ceiling: None`; with this change that means
   the executor holds the whole 48-bit space. Pass the executor's end (4 GiB for wasmtime's
   32-bit memories) as `ceiling: Some(..)`.
3. `Hooks` stays, for the first of his reasons: several implementations exist. 33 test scripts
   implement it (embedder.rs 24, edges.rs 5, taskend.rs 2, away.rs 1, outside.rs 1), and the
   driver holds whichever one a test hands it as `Box<dyn Hooks>`. Its linter finding stays.

### Rulings
(a) SEND. The stand-in of `origin/executor-restart` is `Send` (its engine on a thread of its own).
`with_runners`, `runner` and `run_on` are gone: `Kernel::new` makes the runners and
`Runner::run` runs them. The harness runs each runner with a stand-in made by `restored()` from
the kernel's own, so both are on one engine (`prepare_execution` asserts it). `round_trips`,
`runners` and the kernel's conformance build and run (results below).

(b) TIME SLICE: superseded by the orchestrator's ruling of 2026-11-26 evening (below): no interface
change; the executor bounds its own run by a slice of retired instructions set in its own
configuration and calls `TrapHandler::handle_interrupt`. The question once put here is withdrawn,
and `keep_time` is gone.

(c) SAVING. Built without a copy of the process (`save_and_bring_back`, driver.rs): at each save
moment, between two runs, the kernel's `restore` copies every memory into a new space of its
executor (`create_guest_memory` and `PageTable::copy_from`, so nothing the executor translated is
kept), the runner gets a new stand-in (`restored()`, whose refusals it carries), and the host moves
on to a new seed and clocks. Not the replay in a second process I had named: replaying cannot
rebuild what 28 of the tests carry across a save outside the kernel (the backend server's state,
the outside sockets: backend.rs, outside.rs, the language runtimes), which a copy of the process
kept; and the copy's only extra was the waiting original. MEASURED: the saving and language tests'
outputs match the reference, but their saves are never reached (`saves made: 0`, `ended before its
first save`): the turn a save is drawn at counts runner returns, which only a time slice gives
(the stand-in's own slice, below).

(d) LAYER 1. It tests what layers 2 and 3 do not: random sequences of system calls with random and
edge arguments (proptest), every return value and the arena compared after every call with the
same sequence on Linux, then the play area; layers 2 and 3 run fixed programs. Ported without an
executor of its own: our side now runs the reference's own runner (guest/layer1.c) as the
container's program on the stand-in, the sequence given on its standard input, and parses the
report it writes, exactly as the reference side is run (`8dfdcde9f`). The replay variant compares
the results and uses up the record; it no longer compares guest memory digests (the stand-in's
digests are not wired in since the drive changed). MEASURED (job kernel-port 20261126-152412, 16,
8, 8 and 8 cases): on the backend, on the cached backend and the replay pass;
`sequences_match_reference` fails on a real difference (seed 0, a `setsockopt` after
`finit_module`), and is on fails16's list.

### Waiting on nothing
- Epoll and the closed input (`82ffab441`): the kernel let go of an end while `serve_ends` still
  held its own copy of the end's file, so the release did not see the last reference and woke
  nothing; `poll` passed only because any object's wake wakes a poll. MEASURED: streams 18 of 18
  (was 17), and streams, terminal (but the known `making_pairs`), shell, epoll, fifo, splice,
  pollmask, sigio, taskend, orphans, zombie, hello as before.
- `round_trips` 7 of 7; `guest_edges` 9 of 9 (ported to `prepare_execution`, `PageTable`, the
  arm64 crate's `Exception`); `runners` 3 of 4 (below).

### KS at `36bf10eed` (MEASURED: fresh clone, job kernel-port-ks2/20261126-113931, 11:39 to
14:38 UTC, load 20 to 47, three targets at a time, 4 test threads, 2,400 s per target)
- clippy `--all-targets --no-deps -D warnings` of the kernel and the harness: 0; rustfmt `--check`:
  0; kernel unit tests 30 passed; the kernel's conformance suite on the stand-in 22 passed (it built
  for the first time since the port).
- Harness: 114 targets: 68 pass, 34 fail, 12 reach the bound. Tests: 287 passed, 53 failed, 8
  ignored, plus 10 seen failing inside bounded targets. Logs and the failing list:
  SCRATCH/kernel-port/ks2/ks-suite, ks2/failed.txt.
- fails16's 35: 31 fail; the other 4 fail inside bounded targets (file_capabilities,
  special_files in backend; timers in timers; proc_task in threads). So the known list is met.
- Bounded (no time slice then): backend (`arguments_taken_once`, saves), cpulimit,
  cputime, deadlines, edges, java, node, processes, repeats, threads, timers, turns. The 8 of the
  last run are all still bounded; backend, java, node and timers join them now that saves no longer
  panic at once.
- New failures beyond fails16, each with its cause:
  - waits on the time slice: saving, go, python language and system test modules' "saves made 0",
    outside (2, the restore is never reached), embedder compute-used (no time passes in a turn),
    rseqabort (no preemption inside a section).
  - stand-in (executor track): counterclock 2 (`TrapOrUndefined`, an MRS of the counter is
    refused since the stand-in runs at EL0), sharedmap 2, sharedviews 3, privatemap 1 (views of
    one block disagree), runners `code_the_kernel_writes_over_code_that_ran_is_what_runs_next`
    (exit 1 where 2: code the kernel wrote is not what runs; INFERRED a translation kept), python
    system program and system test modules (SIGSEGV, exit 139; INFERRED the shared views).
  - mine, open: aio `poll_requests_match_reference` ("events 5" where 1), procstatus fdinfo,
    layer 1 replay (fixed since, above).
- LTP, the 30 run to the end (job kernel-port-ks2/20261126-144004, 2,622 s): 6 verdicts match
  (listen01, mprotect01, recvfrom01, rt_sigprocmask01, sendto01, sigaction01), 24 hang (LTP's own
  timeout; every one with a child or LTP's shared result page; the stand-in's shared-page fault is
  the executor track's).
- KS IS NOT REACHED: the bounded targets waited on a time slice; the stand-in's
  failures and the LTP hangs on the executor track.

### Linter (MEASURED, the tip with the two fixes, no key): the kernel's own crates 31,350 (31,319
at the last run): `constant_defined_twice` +11 (merged crates' constants that a generated binding
also defines), and +4 `name_unanswered`, +2 `summary_unanswered`, +7 `summary_not_one_line`, +4
`summary_too_long`, +2 `summary_not_one_sentence`, +1 third person, all in this run's new
comments (the model's key is needed to answer them). Logs: SCRATCH/kernel-port/ks2/lint.

### Next
- (Superseded below.) KS waited on a time slice, and on the executor track
  for the shared views, the counter's MRS at EL0, the code written over and LTP's shared page.
- Mine meanwhile: aio poll, procstatus fdinfo, saving checked once a slice exists.

## 2026-11-26, evening: the orchestrator's two rulings, the time model, KS again

### Rulings applied (no interface change; the question to the owner is withdrawn)
- TIME SLICE. The executor bounds its own run: the stand-in's `StandInConfig::slice_instructions`
  (executor track, `30ca724a8`, default 1,000,000) calls `TrapHandler::handle_interrupt`. The kernel's
  `keep_time`, the per-turn slice it computed (`Next::Run::slice_nanoseconds`) and `cpu_slice` are
  gone (`dc510b1b0`). A run now ends (`Interrupted`) at three points, so the embedder does the
  outside's work between them (`ed29033b5`, `2a7df943c`, `56218d2c1`): the end of an executor slice;
  a tick (1 ms, Linux's `CONFIG_HZ=1000`) passed on the kernel's clock in turns that all end in
  traps, which an executor slice never reaches as it starts again at each execution; and a thread
  giving way (`sched_yield`). MEASURED (below): backendrace, which spins between system calls,
  went from no runner return in 240 s to finished in 18 s.
- GUEST'S COUNTER. A trapped `mrs` of CNTFRQ_EL0, CNTPCT_EL0, CNTVCT_EL0 or their self-synchronized
  forms (`SystemAccessTrapped`, or kept whole as an EC 0x18 syndrome) is a new exit, `CounterRead`:
  the kernel writes the value into the register (none for XZR) and steps past the instruction. The
  count is the kernel's own monotonic clock (host clock plus the restore offset) at 1 GHz, so it
  never goes backwards across a restore and a replay reads it from the record like every other time.
  Any other trapped system access ends as an undefined instruction (SIGILL), where it panicked before.
  The stand-in makes the read trap (executor track, `c4113f822`). The instruction list already lists
  the three counter registers `ReadTrapped` ("CNTKCTL_EL1 gives EL0 no timer",
  processor-architecture/arm64/system_register.rs:163, READ). MEASURED: counterclock 2 of 2 (was
  0 of 2); unit test `trapped_counter_reads_are_answered_and_other_system_accesses_are_undefined`.

### The harness's time and turns
- The repeatable clock moves by the stand-in's instructions at each runner return (the seeded
  `Schedule::turn_nanoseconds`, as before the port) and by 100 ns at each reading, so two readings
  in one run differ, as on Linux (procstatus `fdinfo`'s timer had read exactly what was set).
- A run's turns for its injected moments (saves, signals, notices) count each system call as well
  as each runner return, as when every call was a turn; every moment reached is acted on at the
  next return (`523babf51`). An injected run gives the stand-in a seeded slice of 5 to 50
  instructions per turn of its save gap (`2f7809a1c`).

### Mine, fixed
- aio poll: a fork's child mapped the same shared pages as the parent for a `MADV_DONTFORK` range
  (the aio ring) and cleared them, so the parent's ring read zeros (`events 5` of zeros). The child
  now unmaps the shared views first (`a834b3c38`). MEASURED: aio 3 of 3.
- procstatus fdinfo: the clock above. MEASURED: procstatus 4 of 5, the fifth `limits` on fails16.
- The runtime's `ceiling`: done by the runtime track, `7d6d0e11e`, `ceiling: Some(memory_bytes)`
  where `memory_bytes` is the executor's space end (READ); the kernel bounds its spaces by it
  (container.rs:1012).

### MEASURED in pieces before the full run (ARM64, `-j 12`, 3 or 4 targets at a time)
- Of the 12 bounded targets: cpulimit, cputime, deadlines, edges, processes, repeats, turns pass;
  threads 12 of 13 (proc_task, fails16); timers 4 of 5 (timers_match_reference, fails16); go 1 of 1;
  saving 1 of 1; outside 9 of 9; java 0 of 2 (saves now reached; the result is in the full run);
  node, backend's `arguments_taken_once_match_reference`: still past 1,500 s (open, below).
- Also: embedder 36 of 36 (compute-used fixed), rseqabort 2 of 2, streams 18 of 18, sharedviews
  3 of 3, round_trips 7 of 7, hello, shell.

### KS at `b7406836f` (MEASURED: fresh clone of the pushed tip, job kernel-port-ks3/20261126-213754-1463701, 21:37 to 00:04 UTC, load 8 to 38, three targets at a time, 4 test threads, 2,400 s per target)
- clippy `--all-targets --no-deps -D warnings` of the kernel and the harness 0; rustfmt `--check` 0;
  kernel unit tests 31 passed; the kernel's conformance suite on the stand-in 22 passed.
- Harness: 114 targets: 87 pass, 25 fail, 2 reach the bound (was 68, 34, 12). Tests: 350 passed,
  37 failed, 8 ignored (was 287, 53, 8). Logs: SCRATCH/kernel-port/ks3/ks-suite, ks3/whole.log.
- fails16's 35: all 35 fail, each in a finished target (`proc_task` and `timers` now inside
  finished threads and timers targets; `file_capabilities`, `special_files` in backend's log).
- Failing beyond fails16, each with its cause:
  - python, all three: the guest ends by SIGSEGV (exit 139); language modules after test_bool,
    with 2 saves made. Cause not found yet (open; the system program crashed before this run too).
  - sharedmap `remapped_file_pages_match_reference`: pages a `remap_file_pages` maps at another
    offset of the block read zeros (INFERRED: the stand-in's `map_from` at an offset; the kernel
    calls `PageTable::map_from` with the offset, guest_machine.rs `map_shared`).
- Bounded: backend (`arguments_taken_once_match_reference` past 2,400 s with saves; its program
  alone finishes in 18 s per seed, MEASURED with a probe, so INFERRED a later seed or the reference
  side; open), node (`start_up` bounded, `system_program` failed, no detail before the bound).
- Fixed since the last KS: aio, procstatus fdinfo, counterclock (2), embedder compute-used,
  rseqabort, saving, go, java (2), outside (2), threads saving (2), timers wall clock, runners
  code written over (executor `af5897634`), privatemap and sharedviews (executor), layer1 replay,
  streams input closed; and cpulimit, cputime, deadlines, edges, processes, repeats, turns finish.
- The stand-in's own suite (`--test conformance`, executor track's): `behavior_source::
  next_state_after_an_error_executes_as_on_a_fresh_source` fails; `execution_is_deterministic`,
  `execute_agrees_with_execute_in_place`, `measured_behavior_agrees_with_the_source` did not
  finish in 1,500 s in `--release` (job 20261127-000521). Not measured as holding.
- LTP 30 to the end (2,623 s): 6 verdicts match (listen01, mprotect01, recvfrom01,
  rt_sigprocmask01, sendto01, sigaction01), 24 hang at LTP's own timeout, as at the last KS.
  Logs: ks3/ltp30.log, ks3/ltp30/.
- KS IS NOT REACHED: python (3), sharedmap remapped, backend's arguments_taken_once, node and the
  24 LTP hangs.

### Linter (MEASURED at `dc510b1b0`'s tree plus the fork fix, no key): the kernel's own crates
31,381 (31,350 at the last run): magic_literal +16 (the syndrome's field shifts and the tests'
encodings), name_unanswered +3, the summary shape rules +8, comment rules +2,
constant_defined_twice +1, summary_unanswered +1. Logs: SCRATCH/kernel-port/ks3/lint.

### Next (superseded below)
- python's SIGSEGV, the 24 LTP hangs, backend's arguments_taken_once, node, sharedmap remapped.

## 2026-11-27: KS re-measured from fresh clones; the WebAssembly executor's container; the fixes

His main is still `ac696ea19` (MEASURED: `git fetch` of his checkout, FETCH_HEAD an ancestor of the
branch). All 91 files at the top of his interface folders are byte-identical to his main (MEASURED,
`git diff --quiet` each); the three `interface.rs` not on his main (crates/kvm, linux-socket-options,
wasmtime) are other tracks'.

### Commits of this run
- `c40255f01` merge of `origin/executor-wasmtime` 9802b9416 (normal, no conflict).
- `517923bf4` the stand-in track's `AddressMap::insert` fix, judged right and taken: a mapping
  placed over a file mapping unmaps the old mapping's shared view first, as `unmap` already did
  (memory.rs). Without it `clear_used` zeroed through the view into the file's page cache and the
  new anonymous page stayed a view of the file: ld.so's bss over libc's text tail made one page
  shared by every process (glibc's locks, malloc state).
- `d77525c9d` THE ORCHESTRATOR'S RULING on trapped system registers: a trapped `mrs` of the ID space
  (op0 3, op1 0, CRn 0) is answered as Linux's emulation does (identification.rs: MIDR, MPIDR
  bit 31, REVIDR 0, the ID_AA64* registers of the processor the kernel presents, RAZ for the
  rest of the space it answers); the counter as before; every other trapped access is SIGILL,
  ILL_ILLOPC. Also the WebAssembly executor's line: `to_processor` names the new
  `process_state_controls` field (SSBS, DIT, TCO kept per thread).
- `353987bad` THE HARNESS ON THE WEBASSEMBLY EXECUTOR, done properly: the `runtime` feature makes
  `Back` the `WasmtimeExecutor` (driver.rs); every place only one executor has something is one
  small function pair (`make_back`, `back_alongside`, `carry_refusals`, `last_instructions`,
  `space_end`, the latter capping spaces at 4 GiB); counterclock loses its `runtime` ignore.
  FOR THE RUNTIME TRACK: merge `kernel-port` at `b2594b6d9` (or `353987bad` and later); the
  kernel line and the harness change are both in.
- `b2594b6d9` backend's `arguments_taken_once`: `dup2`/`dup3` over a descriptor of a backend file
  sent its FLUSH unwaited, so a save between send and answer dropped the request
  (`backend_restored`) while the harness's server still owed the answer; every later answer then
  belonged to a lost request and the call that waited never woke (MEASURED with a scratch trace:
  seed 2, 7.6 million turns of the flipping thread, the main thread parked on `FUSE_GETATTR` 5,
  2,000 flushes outstanding). Linux's `do_dup2` calls `filp_close`, which waits for the flush;
  `dup3` now does too (as `close` already did), taking the slot's old file once and waiting on
  later runs only.

### Python's SIGSEGV: gone with the insert fix
MEASURED with a scratch fault trace at `353987bad` (every SIGSEGV/SIGBUS the kernel sends printed
with its address, pc and nearby mappings): `python3 /wc/system.py` with 2 saves and with none
ends with exit 0 in 52 s and 50 s and sends no fault signal. At `b2594b6d9`, one seed
(`SCHEDULE_SEEDS=1`): system program passes (42 s), language test modules pass (378 s), system
test modules reach 2,400 s (open). INFERRED: the SIGSEGV was the shared bss page above.

### KS at `353987bad` (MEASURED: fresh clone of the pushed tip, job kernel-port-ks4/20261127-060346-1299458, 06:03 to 07:44 UTC, load 9 to 170, three targets at a time, 4 test threads, 2,400 s per target)
- clippy `--all-targets -D warnings` 0; rustfmt 0; kernel unit tests 34 passed; kernel
  conformance on the stand-in 22 passed.
- Harness 114 targets: 88 pass, 23 fail, 3 reach the bound (backend, node, python). Tests 351
  passed, 33 failed, 8 ignored. Failing tests: fails16's 35 less the two inside the bounded backend
  target.
- LTP 30: 30 of 30 verdicts match (was 6).

### KS at `b2594b6d9` (MEASURED: fresh clone of the pushed tip from the remote, job kernel-port-ks5/20261127-075806-1033943, 07:58 to 09:27 UTC, load 14 to 53, three targets at a time, 4 test threads, 2,400 s per target)
- clippy `--all-targets --no-deps -D warnings` of the kernel and the harness 0; rustfmt `--check`
  0; kernel unit tests 34 passed; the kernel's conformance suite on the stand-in 22 passed.
- Harness 114 targets: 88 pass, 24 fail, 2 reach the bound (node, python). Tests 369 passed, 36
  failed, 8 ignored. Failing: exactly fails16's 35, each in a finished target (backend now
  finishes, 19 of 21 in 42 s, its 2 failures on the list), plus busybox `files`, a minute that
  turned between the reference's `ls -la` and ours (08:28 against 08:29); it passed 3 of 3 when
  run again at the same tip (MEASURED).
- sharedmap 2 of 2 (`remapped_file_pages` passes now, with the insert fix).
- LTP 30 to the end: 30 of 30 verdicts match the committed reference.
- With the `runtime` feature (the WebAssembly executor at 9802b9416): hello 1 of 1, counterclock
  2 of 2, busybox 3 passed, 1 failed, 1 ignored, the failure `system`, which fails the same way on
  the stand-in and is on fails16 (`/proc/config.gz`, `/proc/sys` values).
- The stand-in's own suite (executor track's): `next_state_after_an_error_executes_as_on_a_fresh_
  source` fails and `execution_is_deterministic` passes 1,500 s, as at the last KS.
- Logs: SCRATCH/kernel-port/ks5/out (whole.log, ks-suite/, ltp30.log, failed.txt), scripts in
  ks5/scripts; ks4/out for the run before.

### Left of KS
- node: `start_up_matches_reference` (one save) does not finish in 600 s even without saves; a
  profile (perf, 150 s) gives 66 percent in unicorn's `phys_page_set_level` and 27 percent in
  `phys_page_compact`, under the stand-in's `Memory::run_turn` and `write_unicorn` (memory.rs,
  executor track's): INFERRED, the stand-in's copy of shared pages into unicorn each turn rebuilds
  unicorn's memory map. For the executor track.
- python system test modules: one seed does not end in 2,400 s (the executor track saw the same at
  5,400 s). Open, mine next.
- KS IS NOT REACHED: node and python bounded.

### Linter (MEASURED at `b2594b6d9`, no key): the kernel's own crates 31,496 (31,381 at the last
run): magic_literal +65 and number_constant_without_link +6 (identification.rs's register fields),
name_unanswered +8, summary_unanswered +1, the summary shape rules +34, grammar_unclear +1.
Logs: SCRATCH/kernel-port/ks5/lint.

## 2026-11-27 (afternoon): python's stall, the four LTP regressions, the presented chip, ptrace

His main is still `ac696ea19` (MEASURED: FETCH_HEAD of his checkout an ancestor of the branch); every
`crates/*/interface.rs` byte-identical to it but the three outside-API crates his main does not
have (kvm, linux-socket-options, wasmtime: other tracks').

### Commits
- `ae1ee6d87` ID_AA64DFR0_EL1 reads 0x6 and ID_AA64MMFR0_EL1 0x0000_0111_FF00_0000, Linux's safe
  values of the fields it hides (MEASURED on the ARM64 machine's Linux 7.0); AT_HWCAP, AT_HWCAP2 and
  /proc/cpuinfo's `Features` are now derived from the same `Arm64Features` the trapped reads answer
  from, as Linux derives them, with HWCAP_CPUID set (Linux sets it on every processor, as it answers
  these reads). The abi's `HWCAP`/`HWCAP2` are no longer used by the kernel. AT_HWCAP is now 0x983
  (was 0x183), cpuinfo says `fp asimd crc32 atomics cpuid`.
- `b2c92eb35` harness test `identity`: guest/identity.c reads every register of the answered space
  (op0 3, op1 0, CRn 0, CRm 0 to 7, every op2) and the two words; the test compares with the
  reference: undefined in the same places; every hidden field equal; every visible field at most the
  reference's (the reference chip has more features); our capability bits a subset of the
  reference's, CPUID in both; MIDR and REVIDR left out (they name the chip). Before `ae1ee6d87` it
  failed on DFR0 and MMFR0 (MEASURED); after, it passes.
- `afe23989f` merge of `origin/executor-restart` db4051178 (the stand-in's shared-page fix 5bc2ee60f).

### Python system test modules: where it stood, and the fix (the stand-in's file)
- MEASURED (scratch probe, each module alone, seed 1, 2 saves, guest output echoed): ten modules end in
  52 to 246 s; `test_mmap.MmapTests.test_find_does_not_access_beyond_buffer` did not end (stopped after
  10 minutes; 7 GB resident). No guest thread waits: one thread runs 2,049 rounds of mmap PROT_NONE
  guard plus mmap PROT_READ up to 6,144 pages, then munmap. perf (30 s on the running process): 62%
  in unicorn's `phys_page_set_level`, from `Memory::remap` -> `restore_own_pages` -> `write_unicorn` ->
  `uc_mem_write`, in two equal halves (25.8% and 24.9%): unicorn opens a region it may not write and
  closes it again for each page written, and each rebuilds its whole memory map.
- MEASURED (scratch patch, not committed: map the region writable, restore its pages, set its
  permissions once): test_mmap alone 253 s, 53 tests, 0 failures; the whole system test modules
  target with one seed passes in 1,920 s (2.72 million turns, 2 saves) at load about 100. The patch
  is in SCRATCH/kernel-port/r32/remap-restore-once.patch. The stand-in track's 5bc2ee60f does the
  same for every write into unicorn (remap's included); it is merged here.
- The suite runs 3 seeds: at that speed python's target needs about 5,800 s, so this KS gave node
  and python a bound of 7,200 s (the others 2,400 s).

### LTP: the set of verdicts against ours-20260930-a9f8631e.tsv
- MEASURED (diff of verdict sets, a9f8631e against the stand-in check's run at aabf8dc2e): lost
  syscalls:clock_gettime04 (fail), mm:mtest05, mm:shm_test01, sched:hackbench01 (900 s); gained
  ltp-aiodio.part2:ADSP037, ADSP038, ADSP073, ADSP074 (timeouts before, pass now). At this branch's
  f02018c89 the four lost reproduce (job kernel-port-ltp4/20261127-123959-1096726).
- Causes, MEASURED by perf of each test on our kernel (150 s each): mtest05 64% in unicorn's
  `phys_page_set_level` from `copy_shared_in` -> `write_unicorn` (the per-turn copy of shared pages
  into read-only regions, the same as node); shm_test01 47% `memcpy` in `copy_shared_out` ->
  `uc_mem_read` (the per-turn copy out); hackbench01 15% in the shared-range tree lookups
  (`find_shown`, `is_touched`). All three are the stand-in's per-turn shared-memory work.
- clock_gettime04: `CLOCK_REALTIME(gettimeofday): Difference between successive readings greater
  than 5 ms (3): 15`, twice, alone too. Both readings come from the host's wall clock
  (time.rs `clock` and `gettimeofday` both call `realtime_nanoseconds`; LTP runs with the wall
  clock), so 15 ms passed between two guest calls. The run took 33.5 s (1.2 s in a9f8631e): 40% of
  its profile is the host kernel (the stand-in's turn handed to its own thread and back:
  `SyncWaker::notify`, futex), 6% unicorn map rebuilds. INFERRED: a turn of the stand-in that rebuilds
  unicorn's map (or a slow wake of its thread) gives the 15 ms gap. Owner of all four: the stand-in
  track (memory.rs, engine thread); nothing in the kernel changed the readings.

### The presented chip: one source (STOPPED, missing piece below)
- READ: no call of the owner's interfaces gives the processor an executor presents. `Executor`,
  `Execution`, `BehaviorSource` and `ProcessorArchitecture` carry no ID register value or feature;
  the ID registers are `ReadTrapped` in the list (system_register.rs), so executing an `mrs` of one
  on the executor only traps. `System::choose_processor_features(process, supported)` takes the
  supported features as input; the kernel has nothing to fill `supported` from but its own
  `identification::PRESENTED`. Probing instructions through `execute_single_instruction_in_place`
  would infer some features from behavior, not read them, and not every field can be told (DGH,
  RPRFM are hints).
- Missing piece, as code, for the owner (crates/behavior-source/interface.rs):
  `fn presented_features(&self) -> <Self::Architecture as ProcessorArchitecture>::Features;` with
  `type Features: Copy;` on `ProcessorArchitecture` (Arm64Features for arm64), so the kernel passes
  the executor's value to `choose_processor_features` and answers reads and AT_HWCAP from what
  comes back.
- Until then `PRESENTED` (fp, asimd, crc32, atomics; no LSE2, FP16, PMULL/AES, SSBS, DIT, MTE, RNG)
  agrees with the WebAssembly executor's presented chip as the list describes it (READ:
  PRESENTED_SYSTEM_REGISTERS on origin/arm64-instructions 233a8e39b), but is not taken from it.

### ptrace (call 117): what KS needs
- READ (reference logs, kernel/ltp/reference-1.tsv.logs): six LTP tests pass on the reference and
  are broken here: ptrace01 (TRACEME, the child stops on a signal, PTRACE_CONT and PTRACE_KILL, with
  and without a handler), ptrace02 (EPERM), ptrace03 (ATTACH to a traced process: EPERM; to one that
  does not exist: ESRCH), ptrace05 (TRACEME, kill(.., sig) for each signal: a stop the tracer's
  wait sees, SIGKILL kills), ptrace06 (PEEK/POKE TEXT, DATA and USER at 8 bad addresses each: EIO),
  ptrace11 (ATTACH to pid 1 as root succeeds; the reference machine's Yama ptrace_scope is 1,
  MEASURED, and pid 1 is an ancestor).
- KS's bar names 30 LTP tests and the checker picks its own; any of the six differs. So KS needs
  either these requests (TRACEME, ATTACH, CONT, KILL, DETACH, PEEK/POKE TEXT/DATA/USER) with the
  signal-delivery stop reported to the tracer's wait4 and Linux's ptrace_may_access and Yama checks,
  or ptrace written into the gap list. No interface change is needed (INFERRED: tracee memory goes
  through the kernel's own page tables).

### KS at `afe23989f` (MEASURED: fresh clone of the pushed tip from the remote, job kernel-port-ks6/20261127-134138-2200267, 13:41 to 16:14 UTC, load 46 to 193, three targets at a time, 4 test threads, 2,400 s per target, node and python 7,200 s)
- ARM64: clippy `--all-targets --no-deps -D warnings` of the kernel and the harness 0; rustfmt
  `--check` 0; kernel unit tests 35 passed; the kernel's conformance suite on the stand-in 22
  passed. x86-64 (job kernel-port-ks6/20261127-163457-3109109, same clone): kernel clippy
  `--all-targets --no-deps -D warnings` 0, unit tests 35 passed.
- Harness: 115 targets (identity new): 91 pass, 24 fail, none reaches its bound. Tests 376 passed,
  35 failed, 8 ignored. The 35 failing are exactly fails16's 35 (diff of names: none more, none
  fewer), each in a finished target, causes as on the ARM64 machine's known list.
- node 2 of 2 (135 s; the stand-in's 5bc2ee60f); python 3 of 3 in 5,623 s (system test modules
  seeds 1 to 3, with 2 saves each); identity 1 of 1.
- With the `runtime` feature (the WebAssembly executor): hello 1 of 1, counterclock 2 of 2, busybox
  3 passed, 1 failed (`system`, on fails16), 1 ignored, as at the last KS.
- The stand-in's own conformance (executor track's) reaches its 1,500 s bound, as before.
- LTP 30 to the end: 30 of 30 verdicts match the committed reference.
- LTP regressed and gained set (LTP_LIMIT 900, 8 jobs): clock_gettime04 pass (6.5 s; back with
  5bc2ee60f), ADSP037, 038, 073, 074 pass; mtest05, shm_test01, hackbench01 still timeout;
  ptrace05 broken (call 117). Profiles at this build (100 s each, MEASURED): mtest05 51% in unicorn's
  `phys_page_set_level` from `Memory::remap` (unicorn's map and unmap at each guest mapping
  change); shm_test01 34% `memcpy` in the per-turn `copy_shared_out`, `copy_shared_in` and
  `SharedPages::read`; hackbench01 18% in the shared-range tree lookups (`find_shown`,
  `is_touched`). For the stand-in track.
- KS's bar holds by this run: clippy on both machines, the unit tests, exactly fails16's 35, 30 of 30
  LTP verdicts. The breaks of the second check are the checker's to re-apply. Logs:
  SCRATCH/kernel-port/r32/ks6/out (whole.log, ks6-out.tgz), r32/perf3.out, r32/perf3b.out,
  r32/pyprobe1, r32/identity1.out.

### Linter (MEASURED on the working tree before `ae1ee6d87`'s summaries were shortened, no key): the kernel's own crates 31,527 (31,496)
New rules of the linter's newer build fire on interface files and other crates (independent_trait,
associated_type_unused, conformance_untested_method, trait_not_implemented +3); on this run's lines:
magic_literal +4, number_constant_without_link +2, name_unanswered +3, summary shape +8,
comment_too_long +1 (the summaries of the new items shortened after this count). Logs: r32/lint.

### Next
- KP2's leftovers: the harness's `Hooks` (37 uses, 5 test files implement it) and `DigestMemories`
  traits; the abi's `Cpu`, `Host`, `Event`, `Next`, `Exit` are gone from both crates (git grep: 0).
- KF as listed. The presented chip's source waits on the owner's call above.
