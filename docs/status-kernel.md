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
