# The embedder interface as files: what is open

Branch phase-1-interfaces, from origin/phase-1-streams at ac676b6b. Four interface crates were added; nothing
else in the repository was changed except the workspace Cargo.toml (members, and the reviewable prefix),
Cargo.lock (the eight new packages) and reviewable-answers.toml (new: the model's answers the linter
records).
"Today" means origin/phase-1-streams (S) unless marked R, which is origin/phase-1-runtime at ac0079e6, the
only branch with wcontainers-runtime. Rulings are cited by their heading in next-embedder-interface.md.

## The four crates, and why each exists

The crates follow who implements them, because an implementation sits under its interface and a
conformance suite runs on one value of one implementation.

- `crates/executor` (wcontainers-executor): `Executor`, `PerThreadWatch`. Implemented by each executor
  (wasmtime, native, the test stand-ins). The process interface needs it for `Kernel::Executor` and
  `ExecutionConfig<E>`.
- `crates/process` (wcontainers-process): `Kernel`, `Process`, `ProcessInfo` and the data the kernel hands
  out (the kernel's interface file of "shared memory is announced per process"). Implemented by the
  kernel. `Kernel` is the root its conformance suite takes; `Process` is not yet reached from it (see e).
- `crates/system` (wcontainers-system): `System`. Implemented by the embedder (LocalSystem, the test
  hosts). Depends on the process interface, since every hook is handed a process.
- `crates/container` (wcontainers-container): `Container`, `Interrupter`, `StopReason`, `GuestMachine`.
  Implemented by the runtime.

Where `Kernel` lives. In wcontainers-process, beside `Process`. `Process` names `Kernel` (`type Kernel:
Kernel`) and `Kernel` names `ExecutionConfig` (`resume_process`), which is in this crate, so a `Kernel`
in a crate of its own would need this crate and this crate would need it: a cycle. A crate of its own
would work only if `ExecutionConfig`, `ThreadOrdering` and `ReplayDeterminism` moved into it with
`Kernel` (wcontainers-process would then depend on it); that moves ruled items to another crate and needs
a folder name no ruling gives. This crate is already the kernel's interface file (the ids ruling says
so), and the ruling's "the kernel is a trait" puts the kernel's public surface in an interface, which
this one is. The folder `crates/kernel` was not used: its package would be `wcontainers-kernel`, the
existing kernel crate's name. For the later move: the existing crate becomes an implementation of this
interface, so it goes to `crates/process/implementation/<name>` (package `wcontainers-process-<name>`);
renaming this interface's folder to `kernel` would then be free, but that is your call, not done here.
Before either, `TaskId`, `Errno`, `siginfo` and `task_id_after` must leave wcontainers-kernel: this crate
depends on wcontainers-kernel for them, and the kernel, implementing this crate, would depend on it: a
cycle (see d). The kernel's struct is also called `Kernel` (kernel lib.rs:137); it would implement
`wcontainers_process::Kernel` by path, so the two names do not clash.

## (a) Items in the files that were never ruled

All three crates the kernel uses (executor, process, system)
- line 3: `#![no_std]`, with `core::ops::Range`, `core::time::Duration`, `core::ops::ControlFlow` and
  `alloc::vec::Vec`. The kernel is `#![no_std]` (kernel lib.rs:9) and implements or calls all three. These
  are the same types as the `std::` paths you named; only the path differs. container/interface.rs keeps
  `std::`, since only the runtime uses it.

executor/interface.rs
- line 16: `PerThreadWatch::set_thread_permissions(&mut self, thread: ThreadId, range: Range<u64>,
  allowed: Permissions) -> Result<(), CpuError>`: the method and its error are today's (abi/cpu.rs:104).
  Without it the subtrait has nothing. The parameter is `allowed`, not today's `permissions`, so that it
  does not repeat its type ("Names ruled 2026-11-19 16:55"); abi's own doc of `Permissions` is "What a
  page allows".
- line 10: `Executor` holds only the ruled `TranslatorConfig`. Today's other executor items are not carried
  (see b).

process/interface.rs
- line 1: the crate doc now names the kernel; line 16: the doc comment of `Kernel`.
- lines 17 to 43 (`Kernel`) and 48, 68 to 78 (on `Process`) are as ruled ("suspend and resume by task,
  and the process context"). Only rustfmt's line breaks differ.
- lines 52 and 64: the executor is reached through the kernel, as `<Self::Kernel as Kernel>::Executor`,
  in `execution_config` and in the `where` of `watch_thread_memory`. The ruling gives `Kernel` its
  `type Executor` and `Process` its `type Kernel: Kernel`, and so settles where the executor parameter
  goes; the spelling of the path is mine.
- line 52: `execution_config(&self) -> &ExecutionConfig<...>`. The getter is ruled ("Agreed
  2026-11-18 night"); that it lends a reference is mine. By value it cannot be written by a kernel generic
  over its executor: `E::TranslatorConfig` has no bound, so the kernel cannot copy it out of `&self`
  (today's `ExecutionConfig` is `Copy`, so this never came up). The runtime already hands translator
  settings by reference (R container.rs:35, 103). The other way is `type TranslatorConfig: Clone` and a
  return by value. Your choice.
- line 53: `state(&mut self) -> &mut Self::State`: the behaviour is ruled ("per-process state": handed over
  with the process, mutable, no lock), the name and signature are not.
- line 54: `memory_id(&self) -> MemoryId`: the getter is ruled (WorkloadPlacement, evening), the name is
  not. See (d) on what the id means across exec.
- line 56: `read_memory` returns `Result<impl Iterator<Item = &[u8]>, MemoryError>`. "Pieces of borrowed
  bytes" is ruled; the iterator type, the `Result` and `MemoryError` are not.
- line 60: `write_memory` returns `Result<(), MemoryError>`: not ruled.
- line 62: `watch_thread_memory`: the verb-first spelling of the ruled `memory_watch_thread`, and the
  `where ...Executor: PerThreadWatch` form of "only where the executor implements the subtrait". See
  (e) for what that form does to a conformance suite.
- line 65: `take_dirty_memory` returns `Vec<Range<u64>>` (today's type; not ruled).
- lines 83 to 85: the accessor names `memory_mapped`, `ceiling_distance`, `compute_used` are today's;
  the ruling names the concepts ("memory mapped, distance to the ceiling, compute used").
- line 86: `ProcessInfo::parent(&self) -> Option<TaskId>` (today's system.rs:256).
- line 87: `ProcessInfo::is_inspected(&self) -> bool`. It is in the inspection proposal ("Proposed to the
  owner 2026-11-18 night, NOT ruled": "the kernel announces `on_inspection_starting` at the first and
  `on_inspection_ended` after the last; `ProcessInfo::is_inspected()`"), and that proposal was then
  "RULED" "as proposed above" ("Ruled and named by the owner, 2026-11-18 night"). The two hooks come from
  the same sentence, so all three are written. No later section names it again; strike it if your ruling
  covered only the line Linux draws.
- lines 98, 105, 114, 122, 130: derives `Clone, Copy, Debug, PartialEq, Eq`, as today.
- lines 129 to 137: `MemoryError { NotMapped, NotRead, SharedNotWritable, InvalidRange, Executor(CpuError) }`
  (today's system.rs:106).
- line 143: `SignalInformation = siginfo`, the generated binding of Linux's `siginfo` (kernel
  linux/general.rs:814). The ruling says "Linux's own record of a signal"; that it is this binding is
  my reading.
- lines 146, 157, 161, 165, 169: derives `Clone, Copy, Debug, PartialEq, Eq, Hash` on `MemoryObject` and
  its four ids, which distributed-everything's `MemoryShard` needs (workload-placement/interface.rs:5).
- lines 13 and 14: `TaskId`, `Errno` and `siginfo` are imported from wcontainers-kernel (see d).
- Every doc comment except the ruled one on `ReplayDeterminism::External` (line 109).

system/interface.rs
- line 4: `#![allow(unused_variables)]` (see f).
- line 18: the parameter `previous` of `allocate_task_id`, and its default body, which calls the kernel's
  `task_id_after` (system.rs:196): the ruled counter, wrapping past the low ids as today.
- line 21: the parameter name `id` of `free_task_id` (today's).
- lines 23 to 96: every hook and both proxy calls take `P: Process<State = Self::ProcessState>`. That form
  is ruled for the two shared-memory hooks only; the earlier hooks were ruled with an untyped `process`.
- line 43: `thread: TaskId` on `on_thread_starting` (today's; `send_signal(thread: TaskId)` is ruled).
- line 50: the parameter `bytes: u64` of `on_process_outgrown` (today's system.rs:175).
- lines 94 and 96: the inspection hooks' parameter `process: P` (only the names and the trigger are
  ruled), and the doc comment of `on_inspection_ended` (line 95).
- line 14: the doc comment of `System`.

container/interface.rs
- line 6: `Container` as a trait name (R container.rs:172 is a struct of that name).
- line 11: `Interrupter`, and its bounds `Send + Sync` (from "interrupter.interrupt() from any thread").
- lines 20 and 21: the field types `until: Option<Duration>` (the time on the monotonic clock, which abi
  `Next::Idle` gives as `until_nanoseconds: Option<u64>`, exit.rs:79; `Duration` is the standard type for
  it) and `status: i32` (a wait status, as abi `Next::Finished`).
- line 16: derives on `StopReason`, as on `Next`.
- lines 25 to 27: `GuestMachine`'s field `offer` and its derives, as R container.rs:44.

## (b) Where the files differ from today's code (the migration checklist)

Kernel, crates/wcontainers-kernel/src/system.rs
- [ ] `pub trait System: Host + Sync` (125) becomes `System: Sync` (system 15). `Host` stays in
      wcontainers-abi with the ruled `send_stream`, `credit_stream`, `close_stream`; the kernel would then
      bound its embedder by both. The linter also refuses a supertrait from another crate of the workspace.
- [ ] New `type ProcessState: Clone + Send` (system 16).
- [ ] Every hook and both proxy calls: `process: Process<'_>` becomes `process: P` with
      `P: Process<State = Self::ProcessState>`; default bodies `let _ = (...);` become `{}`.
- [ ] New hooks `on_signal_delivering`, `on_shared_memory_mapping`, `on_shared_memory_unmapped`,
      `on_inspection_starting`, `on_inspection_ended`.
- [ ] `pub struct Process<'a>` (227) becomes the trait `Process` (process 46), with `type State` and
      `type Kernel`.
- [ ] `pub struct Kernel` (lib.rs:137) implements the trait `Kernel` (process 17): `type Executor`,
      `type SuspendedTask`, `type ProcessContext` and the five calls. Today's kernel has no suspend or
      resume at all.
- [ ] `pub struct ProcessInfo<'a>` and its impl (234) become the trait `ProcessInfo` (process 82);
      `info` returns `impl ProcessInfo` instead of `ProcessInfo<'_>`. New on it: `is_inspected`.
- [ ] `execution_config(&self) -> ExecutionConfig` (284, by value) becomes
      `-> &ExecutionConfig<<Self::Kernel as Kernel>::Executor>`.
- [ ] `read_memory(&mut self, range) -> Result<Vec<u8>, MemoryError>` (289) becomes
      `-> Result<impl Iterator<Item = &[u8]>, MemoryError>`. Today the executor copies (`Cpu::read`,
      abi/cpu.rs:45); lending bytes needs a new executor operation.
- [ ] `write_memory(&mut self, address: u64, bytes: &[u8])` (306) becomes
      `write_memory(&mut self, range: Range<u64>, bytes: &[u8])`.
- [ ] `watch_thread_memory(...) -> Result<(), Unsupported>` (348), refused on every executor, becomes a
      method that returns nothing and exists only `where <Self::Kernel as Kernel>::Executor:
      PerThreadWatch`.
- [ ] New on `Process`: `state`, `memory_id`, `send_signal`, `suspend_tasks`, `suspend_process`,
      `copy_process_context`.
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
- [ ] exec makes a new memory (loader.rs:292, `cpu.memory_new()`), so the process's `MemoryId` changes;
      the ruling needs it kept (see d).

abi, crates/wcontainers-abi/src/cpu.rs
- [ ] `PerThreadWatch: Cpu` (102) becomes `PerThreadWatch: Executor` (executor 15);
      `set_thread_permissions(thread, address: u64, length: u64, permissions)` takes `range: Range<u64>`
      and `allowed`.
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
- [ ] `GuestMachine<Offer>` (container.rs:44) is the same in container 26.
- [ ] `StopReason` is new; today's nearest are abi `Next::Idle` and `Next::Finished`.

Distributed Everything
- [ ] workload-placement/interface.rs:2 imports `wcontainers_kernel::{MemoryObject, TaskId}`. There is no
      `MemoryObject` in wcontainers-kernel; it is `wcontainers_process::MemoryObject` here. Nothing in that
      repository was changed by this stage.

## (c) Names I suggested (nothing ruled was renamed)

- Crate folders `executor`, `process`, `system`, `container` (packages `wcontainers-` and the folder).
  `kernel`, `abi` and `runtime` would give the names of existing packages.
- That `Kernel` is in wcontainers-process (see "Where `Kernel` lives").
- The doc comment of `Kernel`: "A kernel, which suspends and resumes the tasks of its processes and their
  contexts."
- `Process::state` and `Process::memory_id`.
- `Interrupter`.
- The field types of `StopReason`: `until: Option<Duration>`, `status: i32`.
- The parameter `allowed` of `set_thread_permissions`.
- The parameter `process: P` of the two inspection hooks, and the doc comment of `on_inspection_ended`:
  "Announces that another process has finished reading this one's internal state."
- Every doc comment but the two you wrote.
- If you want the linter clean on `ProcessInfo`, `mapped_memory` for `memory_mapped` (see f).

## (d) Ruled, but not written yet, and why

- `kernel.process(id)` ("Evening of 2026-11-18"). The kernel is now the trait `Kernel`, but the ruling
  lists only its five suspend and resume calls (with `// ...` for the rest), and how `process` is spelled
  is not ruled: `Kernel` has no `State`, so what `State` the handle it returns has is open. `resume`
  taking `ExecutionConfig<E>` is now written, as `resume_process`; `SuspendedProcess`, `export` and
  `import` are replaced (below).
- The wire form. It is now two forms, a task and a context, the context sent once for several tasks
  (open in "suspend and resume by task"). `ExportedProcess` and its one form are replaced.
- Whether the per-process state ("per-process state": handed over with the process) travels in the
  `ProcessContext`: `resume_process` takes a context and a config only, and nothing says where the state
  goes.
- `let (mut container, interrupter) = runtime.container(...)`: its parameters' types (`Spec`, `Image`,
  `Error`, the executor's offer) exist only on phase-1-runtime, and its per-container translator settings
  conflict with the ruled per-process `ExecutionConfig<E>`. Without it nothing links an interrupter to
  its container.
- `run_until_finished`: its signature is not ruled; today's form returns `Ending<S::End>` from the
  runtime's own `System`, which was never ruled and clashes with the one ruled `System`.
- The memory id across exec. "the memory's id (true across vfork and exec)" (WorkloadPlacement, evening).
  The getter is written, but today exec makes a new memory and so a new `MemoryId` (loader.rs:292). The
  interface cannot say this; the kernel must keep the id, or the ruling must change.
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
- How a resumed process and the container's first process are given a `ProcessState` value.
- `run` returning `StopReason` when the executor fails: today's run returns `Result<_, E::Failure>`;
  `run` is written exactly as ruled.

Ruled before, and written now
- Per-thread distribution ("Per-thread distribution is CORE ... the kernel must be able to move one task
  apart from its siblings", WorkloadPlacement, evening). "The unit that moves is the TASK (per-thread
  distribution is core)": a task is suspended and resumed apart from its siblings by `suspend_tasks` and
  `resume_tasks` on `Kernel`, and by `suspend_tasks` on a hook's `Process`.

Ruled once and replaced by a later ruling, so not written
- `Process::suspend(self)` and `SuspendedProcess<E>`, with its `export` and `import`: replaced by
  `suspend_tasks`, `suspend_process`, `copy_process_context` on `Process` and on `Kernel`, and
  `resume_process` and `resume_tasks` on `Kernel` ("suspend and resume by task, and the process
  context"). Removed from the files.
- `Process::thread_ordering()`. "Build `ThreadOrdering`, `ReplayDeterminism` and the getter
  `Process::thread_ordering()` only" ("Correction, 2026-11-18 night"). Later the same night: "Getter
  `Process::execution_config()`" ("Agreed 2026-11-18 night"), and `ExecutionConfig` holds the
  `thread_ordering` field ("the progress count is paid only for exact replay"). Today's code has only
  `execution_config` (system.rs:284). That the second getter replaces the first is my reading; no ruling
  says so in words.
- `set_thread_ordering` ("Names the owner fixed on 2026-11-18, evening"): held back by the correction
  that followed, and replaced by `resume` taking `ExecutionConfig<E>`.
- `ThreadOrdering::replay_determinism`: "The getter on `ThreadOrdering` is gone".

## (d2) What "suspend and resume by task, and the process context" settles, and what it leaves open

Settled or changed by it
- Where the executor parameter goes (a, process line 52): on `Kernel` as `type Executor: Executor`, and
  `Process` reaches it through `type Kernel: Kernel`. The hook bound `P: Process<State =
  Self::ProcessState>` stays exact. `Process::Executor` is gone.
- Which value stands for the kernel (d): the trait `Kernel`.
- `SuspendedProcess<E>` as an empty trait: gone, with `suspend`. The kernel's own types are now the
  associated types `SuspendedTask` and `ProcessContext`.
- `ExportedProcess` and its one wire form: replaced by two forms, still open (below).
- `resume` taking `ExecutionConfig<E>`: written, as `resume_process`, and "given at resume, never stored
  in the value".
- The conformance root of the process interface (e): now the kernel, no longer the process.
Not settled, and touched by it
- `execution_config` lending a reference (a, line 52): unchanged, but `resume_process` takes the config
  by value, so whoever resumes needs an owned `TranslatorConfig`; nothing in the traits gives one (e).
- The per-container translator settings of `runtime.container(...)` (d): the ruling gives the config
  per process at resume, which still conflicts with them.

Open in the ruling itself

- The wire form is two forms, a task and a context (context sent once for several tasks). Not written.
- The batch that cannot wait inside a hook: a `suspend_tasks` from a hook that names other running tasks
  of an exactly recorded process. The files say nothing about it.
- Locks across machines: next with you, deferred. Nothing written.
- A task can run only where a context for its process exists; the embedder coordinates. `resume_tasks`
  hands such a task back, as ruled; nothing else is on the interface.
- A consequence for the hooks, not resolved: a hook is generic over `P: Process<State =
  Self::ProcessState>` and says nothing of `P::Kernel`, so a `System` cannot keep the `SuspendedTask`
  or `ProcessContext` a hook's handle gives it except through `P`'s own types inside that call. Sending
  them anywhere waits on the wire forms above. No ruled signature had to change for `Kernel` to exist.

## (e) Conformance: which state-building operations the traits lack

Every suite but the process interface's is left empty. These are the operations each would need; none
were added.

- `Executor`: no methods, so there is nothing to test.
- `PerThreadWatch`: `set_thread_permissions` can be called, but nothing it does can be checked. That needs
  a thread of the executor on a memory with mapped pages, and the trait has no way to make either (today
  `Cpu::memory_new` and `Cpu::thread_new` in abi).
- `Kernel`: the suite's one value is a kernel, the root of the crate. Written, with nothing from
  outside: `suspend_no_tasks` (an empty batch takes no task: "it returns the tasks it took") and
  `resume_no_tasks` (resuming no tasks hands none back: "both resume calls return what they did not
  place"). Not written, and why:
  - `suspend_process`, `copy_process_context` and the two checks you named (a context handed to
    `resume_process` twice is handed back the second time; tasks handed to `resume_tasks` without their
    context come back, after `suspend_process` took the context) all need the `TaskId` of a process
    that is on the kernel. `Kernel` has no operation that gives one: none lists its processes or starts
    one. The ruled `kernel.process(id)` would not help either, as it also takes an id. Checks on fixed
    ids (`TaskId(u32::MIN)` and `TaskId(u32::MAX)`, each run only if a process is there) do compile, a
    check found, but on a kernel with no process at those ids they check nothing, so none were written.
  - The iterator `suspend_process` returns keeps the kernel borrowed, so a check must collect or drop it
    before any resume call. That follows from the ruled signature.
  - `resume_process` also needs an `ExecutionConfig<Self::Executor>`, and nothing makes its
    `translator: E::TranslatorConfig`: the type has no bound (no `Clone`, no `Default`), and the only
    config the traits show is lent by `Process::execution_config`, which a `Kernel` cannot reach. A suite function can get one only
    as a second argument, and `tests!` passes one value.
  - So the missing operations are: one on `Kernel` that gives a process on it (its id or its handle),
    and one that gives an owned `TranslatorConfig` (for example `Clone` on it, with the getter).
- `Process` (and `ProcessInfo`, reached through it): as a root, a suite can be written on one process,
  and every method can be called from it. But the crate's one value is now a kernel, and `Kernel` gives
  no `Process`; a process as the value instead would leave both resume calls unreachable, since `Process`
  gives no way to its kernel. So `Process`'s methods are untested until `Kernel` hands out a process.
  The checks below are what a process suite could hold once it is reached. A check found this, with a
  draft suite in
  pyro-parked/wcontainers-scratch/orchestrator/interface-files/check-linter-and-code/wcontainers/crates/process/conformance/.
  - What can be checked now: the view, `id`, `memory_id` and `execution_config` read the same twice;
    a second `take_dirty_memory` in a row is empty; `state` gives the same place twice; bytes written read
    back the same. `process.id()` is the first thread (Linux's thread group id), so no thread-starting
    operation is needed. A `SignalInformation` can be built in safe code from the kernel's generated
    `SIGINT` and `SI_USER`.
  - A writable range for the write-and-read check comes only from the first `take_dirty_memory`, which
    today's code documents as "every page mapped writable" (system.rs:358). That meaning is not ruled.
    A ruled operation that maps memory into a process would remove the reliance.
  - `suspend_tasks`, `suspend_process` and `copy_process_context` give tasks and a context, but only the
    kernel resumes them, so what they return cannot be checked from a process alone. And nothing
    observes `send_signal`.
  - `watch_thread_memory` can be called from a suite function `where <T::Kernel as
    Kernel>::Executor: PerThreadWatch`, and the
    linter counts it. But `tests!` calls every suite function on the one value, so an implementation
    whose executor lacks `PerThreadWatch` (today every executor) would fail to compile its conformance
    test. One conformance crate cannot serve a method that only some executors have. This needs your
    ruling (for example a second `tests!` for the subtrait, or the method moved to a trait of its own).
  - The suite is not added here: its checks rest on the unruled meaning above and on the open
    `watch_thread_memory` question.
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
  traits must hang from one root that the others are reached from. In `process` that is now `Kernel`,
  which reaches nothing yet; `Process` reaches the view. In `container` the root would be the creation
  call.

## (f) The linter, and the rules

Run with the model check over the whole workspace (`cargo-reviewable reviewable check`). For the new
crates there are 42 findings:
- 41 `conformance-untested-method`: every method of every trait above but `Kernel::suspend_tasks` and
  `Kernel::resume_tasks`, the two the process suite calls. They are kept as findings, as section (e)
  explains.
- 1 `verb-not-first`: `memory_mapped` (process/interface.rs:83), read by the model as a verb in second
  place. The name is today's and follows your words "memory mapped", so it is kept, not disputed;
  `mapped_memory` would pass.
- `is_inspected` is disputed in reviewable-answers.toml. The model gave the right answer (first_word) at
  confidence 0.4995, just under the floor of 0.5, which is a `grammar-unclear` finding. The name is the
  one in the ruled proposal, so it is kept and the answer disputed: "its answer, first_word, is right: is_
  is the verb of a predicate, as in is_mapped and is_full". Remove the dispute if you would rather see it.

The rest of the workspace, not yet in the layout, has 30,643 findings (30,685 in all). The model's
answers for the new names (`suspend_tasks`, `suspend_process`, `copy_process_context`, `resume_process`,
`resume_tasks`, `suspend_no_tasks`, `resume_no_tasks`) are all first_word, at confidence 0.91 or more.

Getters and the verb-first rule. "method names are verb first" ("Ruled by the owner, 2026-11-19
afternoon"), and in the naming rules you agreed for the linter the same day, "getters stay noun phrases".
So the getters `id`, `info`, `state`, `memory_id`, `execution_config`, `memory_mapped`, `ceiling_distance`,
`compute_used` and `parent` have no verb, as your own `execution_config()` has none. Only `memory_mapped`
is read by the model as a verb phrase, which is why it alone is flagged.

Doc comments and the summary rule. A function's doc is to start with a third-person verb. Your ruled doc
of `on_inspection_starting` ("Another process is about to read this one's internal state.") does not,
and it is kept word for word. The doc of `on_inspection_ended`, which I wrote, does.

Not flagged today, and will be: the process and system interfaces depend on wcontainers-kernel. That
becomes `depends-on-implementation` once the kernel is an implementation.

The hooks have empty bodies `{}`, as the naming rule needs (`let _ = x;` would make them not hooks), and
keep their ruled parameter names. So rustc warns about 32 unused parameters unless
`#![allow(unused_variables)]` is set (system/interface.rs:4).
