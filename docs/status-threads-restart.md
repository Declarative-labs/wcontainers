# Status of the threads track, restarted 2026-11-25

Brief: orchestrator/restart-2026-11-25/brief-threads.md. Plan: docs/threads.md, whose top section ("Restart,
2026-11-25") wins over the rest. The old status, docs/status-threads.md, stays as history. Branch `threads-restart`,
cut from the owner's main 95bceadf0. Results are in wcontainers-scratch/threads-restart/.

## TH0, inventory (2026-11-25)

### What exists, and where
- MEASURED: `git rev-list --count main..origin/phase-2-threads` is 0; the old branch's tip 0e40985b0 is an ancestor of
  main. The plan (4,647 lines) and its status (1,179) are on main; nothing is only on the old branch.
- MEASURED: the track has no code: no crate, no trait, no line with `unsafe`.
- READ: today's kernel (origin/kernel-port 64013e048, crates/kernel/implementation/linux/executing.rs) has one
  `spin::Mutex` around the whole kernel and the executor's memories (:100-104), and `Runner::run` runs each guest turn
  while holding it (:496-523), so a second runner only gets `Idle { until: None }` from a failed try-lock (:497-501).
  `Parallel` is refused at the start (:77-78). The per-thread watch is `unimplemented!` (:698). On that branch the
  kernel holds 86 lines with `RefCell<` in 23 files and 111 with `Arc<` in 22 (MEASURED, `git grep -nP`); INFERRED that all of it is
  reached only under the one lock.

### Compile status on main 95bceadf0 (MEASURED)
ARM64 machine, load 10.79 at start, `cargo check -j 12 -p <crate> --lib` (log: wcontainers-scratch/threads-restart/
th0-main-check.out; per-crate output th0/).

| crate | exit | errors | cause |
|---|---|---|---|
| wcontainers-kernel, -executor, -system (interfaces) | 0 | 0 | |
| wcontainers-translator | 0 | 0 | |
| wcontainers-kernel-linux | 101 | 346 | 327 `unsafe` refused by the workspace's `forbid` (222 blocks, 104 methods, 1 trait impl), 4 E0453 (`allow(unsafe_code)` under `forbid`), 12 E0277 (slices and `Vec<u8>` passed where `VolatileBytes` is asked), 3 against the committed `Kernel` (E0046 no `Runner`, E0050 `new` without `parallelism`, E0407 `run` not in the trait) |
| wcontainers-runtime-single-kernel | 101 | 346 | the same, in its dependency kernel-linux; its own code not reached |
| wcontainers-executor-wasmtime, -unicorn | 101 | 3 | in the dependency processor-architecture-arm64 (E0046 twice, E0407): the instruction port's; own code not reached |

Main's kernel is older than origin/agents and origin/kernel-port; those are the other tracks' to bring in (kernel KP1,
runtime RT1, executor X1 to X3). Nothing of this track depends on main's copies.

### Lines with `unsafe` (MEASURED, the root manifest's counting command)
| crate | main 95bceadf0 | origin/agents 198bb11d6 | origin/kernel-port 64013e048 | whose, and where it goes |
|---|---|---|---|---|
| kernel/implementation/linux | 332 | 0 | 0 | kernel track: KP1 brings agents' 0 |
| runtime/implementation/single-kernel | 7 | 0 | 7 | runtime track: RT1 brings agents' 0 |
| executor/implementation/wasmtime | 52 | 29 | 52 | executor track: marked exceptions, X2 |
| executor/implementation/unicorn | 35 | 13 (agents) | 35 | executor and kernel tracks: X0's destinations |
| translator | 0 | 0 | 0 | |
| arm64 | 1 | 0 | 1 | translator track |
| x86 | 0 | 0 | 0 | |
| this track | 0 | 0 | 0 | writes none |

### Traits the plan proposed, with their verdicts
None was built or ruled. Each was against the legacy abi and is withdrawn; the list below says what replaces it.

## Names used by the plan's restart section, resolved

Every backticked name in the section resolves to one of: an item on main (`git grep -w` in the interface files), a
line below, or a Rust or Linux name. Check: wcontainers-scratch/threads-restart/resolve-names.sh, run from the clone;
its output must end with `unresolved 0`.

Withdrawn with the legacy abi (each with what replaces it):
- withdrawn: `Cpu`, `Host` (by `&self` and `Sync`): `Executor`, one for each runner, and `System`'s hooks by `&self`.
- withdrawn: `Next`, `Exit`, `Event`, `next`, `exit` (with a processor or a thread): `Runner::run`.
- withdrawn: `Start` (processors and the first config): `Kernel::new`'s `parallelism` and the container's `ExecutionConfig`.
- withdrawn: `run_until_finished` (a worker for each processor): one host thread for each runner, the runtime's.
- withdrawn: `StopFlag`, `stop_thread`, `thread_stopper`, the stop word's bits: the `ExecutionInterrupter` of each turn.
- withdrawn: `Carry::Leave`, `Kernel::enter`, `KernelEntered`: private to the kernel, inside its `TrapHandler`.
- withdrawn: `on_deadline_changed`, the timer event: `StopReason::Idle`'s deadline.
- withdrawn: `on_processor_needed`: waking an idle runner, the parking question the kernel track has asked.
- withdrawn: `parallel_limits`, `set_execution_config`: question 1's `CodeForm`, refused by an executor that cannot run it.
- withdrawn: `process(id)` (reading at any time): no caller needs it.

Proposed, for the owner (code below):
- proposed: `CodeForm` (question 1), `compare_exchange_u32` (question 2), `progress` (question 3), an optional thread
  in two hooks (question 4).

Rust and Linux names: `Send`, `Sync`, `&self`, `&mut self`, `CLONE_VM`, `FUTEX_WAKE_OP`, `interrupt` (the interrupter's
method), `parallelism` (the kernel's argument).

## TH1, the plan on the committed interfaces (2026-11-25)
- Written: docs/threads.md, section "Restart, 2026-11-25" (100 lines, at the top).
- MEASURED: `bash resolve-names.sh` (output resolve-names.out): 55 names; 28 defined on main, 19 withdrawn or proposed
  here, 8 Rust or Linux names; unresolved 0.
- MEASURED: 23 decision rows: 6 ruled (2, 4, 5, 6, 7, 15), 9 decided here (1, 3, 8, 10, 11, 16, 19, 21, 22), 2 the
  owner's (12, 18), 4 later with Distributed Everything (13, 14, 20, 23), 1 with T8 (17), 1 withdrawn (9).
- Decided here where the plan recommended nothing: 11 (the memory called in before such a hook: no interface change,
  cost measured at T4), 22 (cost accepted, bounded and counted). The cross-machine ones wait, by the owner's word of
  2026-11-22 ("cross-machine threads decisions (later, with DE)").

## TH2, the thread-test design for the owner (2026-11-25)
- Written: docs/thread-tests.md, 113 lines: scheduler, bounded search, even sampling with its measured signal,
  checks, register isolation, hostile guests, guest-memory views, needs, five questions. The suite waits for his review.

## Questions for the owner, as code

1. How an executor learns the form of a memory's code (T2, and the progress count for `Exact`):
```rust
// crates/executor/interface.rs
pub trait Executor: BehaviorSource {
    // ...
    fn create_guest_memory(
        &self,
        form: CodeForm<'_, Self::TranslatorConfig>,
    ) -> Result<(Self::GuestMemory, PageTableOf<Self>), Self::CreateGuestMemoryError>;
}

pub struct CodeForm<'config, T> {
    /// Several threads may run on the memory at once.
    pub threads_in_parallel: bool,
    pub progress_count: bool,
    pub translator: &'config T,
}
```
2. Atomic words for the kernel in a memory whose threads run (T2):
```rust
// crates/volatile-bytes/interface.rs
pub trait VolatileBytes {
    // ...
    /// Replaces the aligned word at the offset if it holds the expected value, and returns what it held.
    fn compare_exchange_u32(&self, offset: u64, expected: u32, new: u32) -> Result<u32, u32>;
}
```
3. Decision 12, a stopped task's position under `Exact` (T8):
```rust
// crates/kernel/interface.rs
pub trait Process: ProcessInfo {
    // ...
    /// Returns the progress count and program counter of a thread that is not running.
    fn progress(&self, thread: TaskId) -> Option<(u64, u64)>;
}
```
4. Decision 18, writes from another process (T4):
```rust
// crates/system/interface.rs, ProcessTracer
fn on_memory_written_by_kernel<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
    &self, process: P, thread: Option<TaskId>, range: Range<u64>,
) {}
fn on_watched_memory_accessing<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
    &self, process: P, thread: Option<TaskId>, address: u64, kind: AccessKind,
) {}
```

## SECURITY
Lines with `unsafe` in this track's paths: 0 before, 0 after. No marked exceptions. No new dependency. No hostile-guest
suite exists; none was run.
