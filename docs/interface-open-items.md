# The embedder interface as files: what is open

Branch phase-1-interfaces, from origin/phase-1-streams at ac676b6b. Four interface crates were added; nothing
else in the repository was changed except the workspace Cargo.toml (members, and the reviewable prefix).
"Today" means origin/phase-1-streams (S) unless marked R, which is origin/phase-1-runtime at 7792036c, the
only branch with wcontainers-runtime. Rulings are cited by their heading in next-embedder-interface.md.

## The four crates, and why each exists

The crates follow who implements them, because an implementation sits under its interface and a
conformance suite runs on one value of one implementation.

- `crates/executor` (wcontainers-executor): `Executor`, `PerThreadWatch`. Implemented by each executor
  (wasmtime, native, the test stand-ins). The process interface needs it for `ExecutionConfig<E>`.
- `crates/process` (wcontainers-process): `Process`, `ProcessInfo`, `SuspendedProcess` and the data the
  kernel hands out (the kernel's interface file of "shared memory is announced per process"). Implemented
  by the kernel. `Process` is the root: the view and the suspended process are reached from it.
- `crates/system` (wcontainers-system): `System`. Implemented by the embedder (LocalSystem, the test
  hosts). Depends on the process interface, since every hook is handed a process.
- `crates/container` (wcontainers-container): `Container`, `Interrupter`, `StopReason`, `GuestMachine`.
  Implemented by the runtime.

## (a) Items in the files that were never ruled

executor/interface.rs
- line 14: `PerThreadWatch::set_thread_permissions(&mut self, thread: ThreadId, range: Range<u64>,
  permissions: Permissions) -> Result<(), CpuError>`: the method, its parameters and its error (today's
  abi/cpu.rs:104). Without it the subtrait has nothing.
- line 9: `Executor` holds only the ruled `TranslatorConfig`. Today's other executor items are not carried
  (see b).

process/interface.rs
- line 14: `type Executor: Executor`. Where the executor parameter goes once `Process` is a trait is not
  ruled (`Process<'_, E>` in "Evening of 2026-11-18", `Process<State = ...>` with no `E` in "shared memory
  is announced per process"). An associated type keeps the ruled hook bound `P: Process<State =
  Self::ProcessState>` exact.
- line 19: `state(&mut self) -> &mut Self::State`: the behaviour is ruled ("per-process state": handed over
  with the process, mutable, no lock), the name and signature are not.
- line 20: `memory_id(&self) -> MemoryId`: the getter is ruled (WorkloadPlacement, evening), the name is
  not, and see (d) on what the id means across exec.
- line 22: `read_memory` returns `Result<impl Iterator<Item = &[u8]>, MemoryError>`. "Pieces of borrowed
  bytes" is ruled; the iterator type, the `Result` and `MemoryError` are not.
- line 26: `write_memory` returns `Result<(), MemoryError>`: not ruled.
- line 28: `watch_thread_memory`: the verb-first spelling of the ruled `memory_watch_thread`, and the
  `where Self::Executor: PerThreadWatch` form of "only where the executor implements the subtrait".
- line 31: `take_dirty_memory` returns `Vec<Range<u64>>` (today's type; not ruled).
- line 34: `suspend` returns `impl SuspendedProcess<Self::Executor>`; line 46: `SuspendedProcess<E>` as
  an empty trait (a kernel value cannot be plain data in an interface file).
- lines 39 to 41: the accessor names `memory_mapped`, `ceiling_distance`, `compute_used` are today's;
  the ruling names the concepts ("memory mapped, distance to the ceiling, compute used").
- line 42: `ProcessInfo::parent(&self) -> Option<TaskId>` (today's system.rs:256).
- lines 56, 63, 73, 81, 89: derives `Clone, Copy, Debug, PartialEq, Eq`, as today.
- lines 88 to 96: `MemoryError { NotMapped, NotRead, SharedNotWritable, InvalidRange, Executor(CpuError) }`
  (today's system.rs:106).
- line 102: `SignalInformation = siginfo`, the generated binding of Linux's `siginfo` (kernel
  linux/general.rs:814). The ruling says "Linux's own record of a signal"; that it is this binding is
  my reading.
- lines 105, 116, 120, 124, 128: derives `Clone, Copy, Debug, PartialEq, Eq, Hash` on `MemoryObject` and
  its four ids, which distributed-everything's `MemoryShard` needs (workload-placement/interface.rs:5).
- lines 8 and 9: `TaskId`, `Errno` and `siginfo` are imported from wcontainers-kernel (see d).
- Every doc comment except the ruled one on `ReplayDeterminism::External` (line 66).

system/interface.rs
- line 3: `#![allow(unused_variables)]` (see f).
- line 17: the parameter `previous` of `allocate_task_id`, and its default body, which calls the kernel's
  `task_id_after` (system.rs:196): the ruled counter, wrapping past the low ids as today.
- line 20: the parameter name `id` of `free_task_id` (today's).
- lines 22 to 95: every hook and both proxy calls take `P: Process<State = Self::ProcessState>`. That form
  is ruled for the two shared-memory hooks only; the earlier hooks were ruled with an untyped `process`.
- line 45: `thread: TaskId` on `on_thread_starting` (today's; `send_signal(thread: TaskId)` is ruled).
- line 49: the parameter `bytes: u64` of `on_process_outgrown` (today's system.rs:175).
- lines 93 and 95: the inspection hooks' parameter `process: P` (only the names and the trigger are
  ruled), and the doc comment of `on_inspection_ended` (line 94).
- line 13: the doc comment of `System`.

container/interface.rs
- line 4: `Container` as a trait name (R container.rs:172 is a struct of that name).
- line 9: `Interrupter`, and its bounds `Send + Sync` (from "interrupter.interrupt() from any thread").
- lines 18 and 19: the field types `until: Option<u64>` (nanoseconds on the monotonic clock, as
  abi `Next::Idle`) and `status: i32` (a wait status, as abi `Next::Finished`).
- line 14: derives on `StopReason`, as on `Next`.
- lines 23 to 25: `GuestMachine`'s field `offer` and its derives, as R container.rs:44.

## (b) Where the files differ from today's code (the migration checklist)

Kernel, crates/wcontainers-kernel/src/system.rs
- [ ] `pub trait System: Host + Sync` (125) becomes `System: Sync` (system 14). `Host` stays in
      wcontainers-abi with the ruled `send_stream`, `credit_stream`, `close_stream`; the kernel would then
      bound its embedder by both. The linter also refuses a supertrait from another crate of the workspace.
- [ ] New `type ProcessState: Clone + Send` (system 15).
- [ ] Every hook and both proxy calls: `process: Process<'_>` becomes `process: P` with
      `P: Process<State = Self::ProcessState>`; default bodies `let _ = (...);` become `{}`.
- [ ] New hooks `on_signal_delivering`, `on_shared_memory_mapping`, `on_shared_memory_unmapped`,
      `on_inspection_starting`, `on_inspection_ended`.
- [ ] `pub struct Process<'a>` (227) becomes the trait `Process` (process 12), with `type State` and
      `type Executor`.
- [ ] `pub struct ProcessInfo<'a>` and its impl (234) become the trait `ProcessInfo` (process 38);
      `info` returns `impl ProcessInfo` instead of `ProcessInfo<'_>`.
- [ ] `execution_config(&self) -> ExecutionConfig` becomes `-> ExecutionConfig<Self::Executor>`.
- [ ] `read_memory(&mut self, range) -> Result<Vec<u8>, MemoryError>` (289) becomes
      `-> Result<impl Iterator<Item = &[u8]>, MemoryError>`. Today the executor copies (`Cpu::read`,
      abi/cpu.rs:45); lending bytes needs a new executor operation.
- [ ] `write_memory(&mut self, address: u64, bytes: &[u8])` (306) becomes
      `write_memory(&mut self, range: Range<u64>, bytes: &[u8])`.
- [ ] `watch_thread_memory(...) -> Result<(), Unsupported>` (348), refused on every executor, becomes a
      method that returns nothing and exists only `where Self::Executor: PerThreadWatch`.
- [ ] New on `Process`: `state`, `memory_id`, `send_signal`, `suspend`.
- [ ] `ExecutionConfig` (68): non-generic, two fields, `#[derive(Clone, Copy, Debug, PartialEq, Eq)]`,
      `impl Default` (73, Serial and Exact) and `supported()` (93). In the file: `ExecutionConfig<E:
      Executor>` with the ruled `translator: E::TranslatorConfig`, and no derives, no `Default`, no
      `supported()`. An interface file holds no impl blocks, and a derive on a struct generic over `E`
      would demand that the executor itself be `Clone`, `Copy` and so on.
- [ ] `Unsupported` (84) is not in the files: nothing there returns it any more. `Kernel::start`
      (container.rs:417) still does.
- [ ] `WatchesFor` (31), `AccessKind` (40), `ThreadOrdering` (49), `ReplayDeterminism` (57),
      `MemoryError` (106), `SystemCallResponse` (121) move to the process interface unchanged but for
      dropped variant comments (on `All`, `Execute`, `Serial`, `Exact` and `MemoryError`'s variants).
- [ ] `Kernel::process(&mut self, cpu, id)` (370) is not in the files (see d).
- [ ] `TaskId` (17), `Errno` (errno.rs:9), `task_id_after` (196) and `linux::general::siginfo` stay in
      the kernel and are imported (see d).

abi, crates/wcontainers-abi/src/cpu.rs
- [ ] `PerThreadWatch: Cpu` (102) becomes `PerThreadWatch: Executor` (executor 13);
      `set_thread_permissions(thread, address: u64, length: u64, permissions)` takes `range: Range<u64>`.
- [ ] `Access` (exit.rs:22) is unchanged beside the new `AccessKind` (see d).

Runtime (R)
- [ ] `trait Executor: Execution` with `Shared`, `TranslatorConfig`, `Offer`, `ARCHITECTURE`, `shared`,
      `new`, `stop_flag` (container.rs:22) becomes `Executor` with `TranslatorConfig` only. The rest makes
      an executor for a runtime, depends on `RuntimeConfig` and the withdrawn `StopFlag`, and is not on
      this branch; with `Cpu` and `Execution` it waits on the merge into one `Executor`.
- [ ] `pub struct Container<E, S>` with `run(&mut self) -> Result<Ending<S::End>, E::Failure>`
      (container.rs:172, 194) becomes the trait `Container` with `run(&mut self) -> StopReason`; today's
      `run` is the ruled `run_until_finished` (see d).
- [ ] `StopFlag` (lib.rs:47) becomes the trait `Interrupter`.
- [ ] `GuestMachine<Offer>` (container.rs:44) is the same in container 24.
- [ ] `StopReason` is new; today's nearest are abi `Next::Idle` and `Next::Finished`.

## (c) Names I suggested (nothing ruled was renamed)

- Crate folders `executor`, `process`, `system`, `container` (packages `wcontainers-` and the folder).
  `kernel`, `abi` and `runtime` would give the names of existing packages.
- `Process::Executor`, the associated type for the executor.
- `Process::state` and `Process::memory_id`.
- `Interrupter`.
- The field types of `StopReason`: `until: Option<u64>`, `status: i32`.
- The parameter `process: P` of the two inspection hooks, and the doc comment of `on_inspection_ended`:
  "Another process has finished reading this one's internal state."
- Every doc comment but the two you wrote.
- If you want the linter clean on `ProcessInfo`, `mapped_memory` for `memory_mapped` (see f).

## (d) Ruled, but not written yet, and why

- `kernel.process(id)` ("Evening of 2026-11-18"), `resume` taking `ExecutionConfig<E>`, and
  `SuspendedProcess`'s `export` and `import` ("Ruled and named by the owner, 2026-11-18 night"; "Agreed
  2026-11-18 night"). All need the kernel, and which value stands for the kernel in the interface is not
  ruled (`Kernel` today; after "interrupting a running container", `run` holds the container and so the
  kernel exclusively). That value also fixes the `State` and `Executor` of the handles it gives out.
- `ExportedProcess`: its wire form is still open (rkyv with a patch, `unaligned` only, or hand-laid).
- `let (mut container, interrupter) = runtime.container(...)`: its parameters' types (`Spec`, `Image`,
  `Error`, the executor's offer) exist only on phase-1-runtime, and its per-container translator settings
  conflict with the ruled per-process `ExecutionConfig<E>`. Without it nothing links an interrupter to
  its container.
- `run_until_finished`: its signature is not ruled; today's form returns `Ending<S::End>` from the
  runtime's own `System`, which was never ruled and clashes with the one ruled `System`.
- `TaskId` "carries a generation" (first section): no later ruling withdraws it, the code has none, and a
  pid-sized id cannot hold one. `TaskId` is kept as today.
- Where `TaskId` lives: the embedder hands it out, the process interface needs it, and the system
  interface depends on the process interface, so by the ids rule it belongs in wcontainers-abi, which this
  stage may not change. It is imported from wcontainers-kernel, as are `Errno` (its constants are an impl
  block, which an interface file cannot hold) and `siginfo`. Under the layout the kernel becomes an
  implementation of the process interface, so these must move before the kernel does.
- "Replace Access everywhere" with `AccessKind`: abi's `Access` in `Exit::MemoryFault` is on the
  executor's side. Replacing it makes `AccessKind` needed by two interfaces, so it would move to
  wcontainers-abi, which this stage may not change.
- The stream services as services of `System` (naming rule, "Later on 2026-11-18"): they stay on abi's
  `Host`; whether `Host` folds into `System` is not ruled.
- How `import` and the container's first process are given a `ProcessState` value.
- `run` returning `StopReason` when the executor fails: today's run returns `Result<_, E::Failure>`;
  `run` is written exactly as ruled.

## (e) Conformance: which state-building operations the traits lack

Every suite is left empty. None can be written against its traits alone from one value. These are the
operations each would need; none were added.

- `Executor`: no methods, so there is nothing to test.
- `PerThreadWatch`: `set_thread_permissions` needs a thread of the executor, on a memory with mapped
  pages. The trait has no way to make a memory or a thread (today `Cpu::memory_new` and `Cpu::thread_new`
  in abi).
- `Process` (and `ProcessInfo` and `SuspendedProcess`, reached through it): from one process the trait
  can read, watch, take dirty marks and suspend. It lacks:
  - an operation that maps memory into the process, so that `read_memory` and `write_memory` have a
    range and can be checked against each other, and `memory_mapped` and `ceiling_distance` change;
  - an operation that starts a thread, for `send_signal` and `watch_thread_memory` on another thread;
  - `resume`, so that what `suspend` returns can be checked;
  - a way to make a `SignalInformation` (Linux's union) without spelling out its fields.
  Also `watch_thread_memory` exists only where `Self::Executor: PerThreadWatch`. One suite function is
  generic over `T: Process` alone, so it can never call it.
- `System`: every hook and both proxy calls need a `P: Process<State = Self::ProcessState>`. `System`
  gives no way to get one, and nothing makes a `ProcessState` value (`Clone + Send`, no constructor).
  `allocate_task_id` and `free_task_id` alone could be tested (two ids handed out with neither freed
  differ), but not the trait as a whole.
- `Container`: `run` can be called, but nothing in the trait builds the states `StopReason` reports.
  `Idle` and `Finished` need a program, `Suspended` needs a hook, and `Interrupted` needs the
  interrupter, which only the creation call gives.
- `Interrupter`: `interrupt` can be seen only through the run of its container; the pair comes only from
  the creation call, which is in no trait.
- One general point: `tests!` hands the same one value to every suite function of a crate. So a crate's
  traits must hang from one root that the others are reached from (`Process` gives the view and the
  suspended process). In `container` the root would be the creation call.

## (f) The linter

Run with the model check over the whole workspace (`cargo-reviewable reviewable check`). For the new
crates there are 36 findings:
- 35 `conformance-untested-method`: every method of every trait above, since the suites are empty. They
  are kept as findings, as section (e) explains.
- 1 `verb-not-first`: `memory_mapped` (process/interface.rs:39), read by the model as a verb in second
  place (later_word, confidence 0.74). The name is today's and follows your words "memory mapped", so it
  is kept, not disputed; `mapped_memory` would pass.

The rest of the workspace, not yet in the layout, has 30,643 findings.

Not flagged today, and will be: the process and system interfaces depend on wcontainers-kernel. That
becomes `depends-on-implementation` once the kernel is an implementation.

The hooks have empty bodies `{}`, as the naming rule needs (`let _ = x;` would make them not hooks), and
keep their ruled parameter names. So rustc warns about 32 unused parameters unless
`#![allow(unused_variables)]` is set (system/interface.rs:3).
