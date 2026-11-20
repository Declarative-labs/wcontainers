# wcontainers: guest threads in parallel

Status: plan, revised 2026-11-19 to the owner's rulings of that day and to the independent check of
the one scheduling rule. **This revision has had no independent check. Until one is recorded in
docs/status-threads.md, every part it changed is unchecked.** The form before it had been corrected
after its sixth independent check (docs/status-threads.md says what changed each time). Nothing here is
built. The owner rules on section 16 first.

The rulings are in the interface notes, from "Ruled by the owner, 2026-11-19: where replay is
recorded" to the end. The check is orchestrator/threads-mixed-ordering/REPORT.md, called "the
mixed-ordering check" here. What they changed, in short:
- **Replay is per process, at its own system calls.** The order of kernel entries recorded and replayed,
  the "recorded kernel", its services `kernel_entry_admit` and `on_kernel_entry_ended`, and the rule
  that a kernel is recorded whole are withdrawn (4.6). A process given `None` costs nothing and is never
  made to carry a neighbour's guarantee.
- **One scheduling rule, kept inside the kernel, with nothing on the interface**: a task whose code is
  `Serial` is given a processor only while no task that overlaps its memory has one. It replaces groups
  in force, the config in force, "the strictest wins" and the hook
  `on_execution_config_in_force_changing` (section 3).
- **Suspend and resume are by task**, with a process context and a `Kernel` trait. They replace
  `Process::suspend(self)`, `SuspendedProcess<E>`, and this plan's `Kernel::process_call_in` with its
  hook `on_process_called_in` (sections 5 and 6).
- **New hooks the plan now builds on**: the system call proxy, the hook for the kernel's writes, the
  signal hook, and the shared-memory hooks with `MemoryObject` (sections 2, 3, 4.6). Locks across
  machines (`wake_waiters`, `move_waiters`) are agreed in substance (4.5).

## Known faults, to settle when T1 is built

These are open. None is hidden further down.

1. **This revision is unchecked, and the scheduling rule's protocol is not modelled.** The model of
   call-ins that section 14 reports (62,189 states) was of the rules for groups. It must be redone for
   the rule of section 3 as the mixed-ordering check corrects it: exclusion by "handed out", readers
   called in and never held, a `Serial` task that is due a turn stopping reader hand-outs, a reader's
   waiter chosen only while no `Serial` task that overlaps it is handed out, and the kernel's rules
   following the phase. Until it is, the rule is sound by the check's reading and by nothing else.
   Also still unread by any checker from the form before: a woken call runs again whatever is called
   in, so two call-ins never wait for each other (4.3, 4.5); a worker inside a guest hook that is not a
   stop point may not enter the kernel (4.2); the kernel's lock handed back to the executor with a
   thread that leaves (4.2, 7.2); the native output's two forms and the map of this plan's calls onto
   the native executor (section 10). Section 12 names a test for each. They are the first tests to
   write.
2. **Replay has no milestone of its own yet.** It needs two kernel primitives the rulings name and
   nobody has designed: handing the embedder each system call result of a recorded process, and running
   a process with its system calls answered from a log (4.6). A process with two or more threads also
   needs the order of its threads' turns in its log, and a way to make a replay follow it (decision
   17). Until those exist, no test here replays anything.
3. **A live run needs one worker for each processor.** A worker serves one processor (section 5). A
   thread pinned to a processor that no worker serves would never run. So `run_until_finished` starts
   one worker for each processor, whatever the host has, and the plan has no setting for fewer
   (section 11). On a host with fewer cores than the container has processors, the host shares its
   cores among the workers. That is correct, and what it costs is not measured. An embedder that calls
   `run` itself must serve every processor. Running fewer workers than processors is not designed:
   the kernel would have to know which processors share a worker, and ask a thread in for a processor
   that is not its own.
4. **The length of a fair slice is still worked out as for one processor**, from the whole run queue
   (kernel/task.rs:43-53). With several processors that gives shorter slices than needed. It is never
   unfair. T1 counts the turns that ended for nobody.
5. **A `Serial` task pinned to a busy processor makes an overlapping `Serial` task that could run on an
   idle one wait** (4.3). Linux would run it. It is the price of a simple rule that keeps a pinned
   thread from starving, and it is counted.
6. **A call that waited and runs again reads its arguments from guest memory again** (4.4). That is
   wrong wherever another thread can run while the caller is parked: in a `Parallel` process, and also
   in a `Serial` process with two or more threads, where a sibling runs while the caller waits (the
   mixed-ordering check's side finding; the form before said `Parallel` only). So it is T1's, not T2's:
   T1 closes it by reading every call.
7. **What T1 adds to a system call is known only in parts**: the lock and its holder's word, each
   measured alone (section 14). The proxy's two calls to the system at each system call (4.6) are the
   interface track's, and were not measured at all. T1's three builds measure the whole (section 12,
   decision 7).
8. **The rule's mixed case on the WebAssembly executor is expected to work and is not built**: a
   `Serial` task beside `Parallel` ones that share a file mapping or System V memory with it, with the
   shared pages copied at each switch (7.4). Until it is built, those tasks all run as `Serial` there.

Words used here:
- **executor**: what runs the guest's instructions behind the kernel's `Cpu` side (`Executor`,
  rt/container.rs:19). Three exist or are being built: the WebAssembly executor (wasmtime), the native
  executor (a virtual machine, docs/native.md on origin/phase-2-native) and the kernel tests' stand-in.
- **WebAssembly compiler**: what compiles WebAssembly to the host's machine code and runs it: wasmtime,
  or a browser's V8.
- **embedder**: whoever implements `System` (rt/lib.rs:146) and drives a container.
- **block**: a block of shared pages (`SharedId`, abi/cpu.rs:14-16). The kernel shows every mapped file
  through one. A memory **shows** a block's page where a mapping of it puts that page.
- **task**: Linux's word for one thread of execution; one task is one thread. The owner's later rulings
  use it (`TaskId`, `suspend_tasks`). This plan says thread for the same thing, and task where it
  states a ruling.
- **overlap**: a task overlaps every task whose memory is its own memory, or shows a page of a block
  that some mapping may write and that its own memory shows too (section 3). The owner's scheduling
  rule is said in this word.
- **config**: a process's `ExecutionConfig` (section 2): its thread ordering, its replay guarantee and its
  translator config. A process runs under the config it was given. There is no config "in force" any
  more (section 3).
- **`Serial` code** and **`Parallel` code**: the form of translated code a memory runs (7.2). It is
  `Serial` code when any process that owns the memory was given `Serial`. A task **runs as `Serial`**
  when its code is `Serial` code, or when the executor cannot run it beside others (section 3).
- **guarantee**: a process's `replay_determinism`: `Exact`, `External` or `None`.
- **stop point**: a place in translated code after which a piece no longer relies on what it found out
  about page permissions before it. The translator's contract names three: a stop check whose hook runs,
  a system call, and leaving `Execution::run` (tr/contract.rs:17-20). 4.5 adds the hooks that are made
  stop points. Every other hook returns into the middle of a piece, which carries on with what it knew.
- **stop word**: the 32 bit word after a thread's registers that translated code tests at each stop check
  (abi/registers.rs:54-57). Zero means carry on. Section 4.5 makes it a set of reasons.
- **handed out**: the kernel has given a thread a processor and has not yet seen its next kernel entry.
  While handed out it may be in guest code or on its way into the kernel (the mixed-ordering check's
  word). The scheduling rule excludes by it (section 3).
- **called in**: the kernel asks for a running thread with `thread_stop`, and the thread comes back by
  itself through `Kernel::exit`. A call that needs it back first parks, as any call that waits does. A
  memory is called in while such a call's mark is on it (4.5).
- **waiter**: a thread whose call parked for a call-in and has been woken, and has not run again yet.
- **group stop**: the executor makes every other thread of one guest memory held, in place, while one
  change is made. It is kept per memory (4.5); the threads it reaches are always of one memory. It
  never holds a thread of another memory for a `Serial` task's turn: those are called in (section 3).
- **stop lock**: one lock per guest memory, in the executor. Whoever needs that memory's other threads
  held takes it first (4.5).
- **held**: a thread is held when it is at a stop point and has no access of its own to its memory under
  way: it waits there for the stop lock or for the kernel's lock, or does something that touches nothing
  of the memory, or it has left `Execution::run`. A thread inside a hook that is not a stop point is not
  held (4.5).
- **gives up its frames**: a thread leaves the translated code, and the executor enters it again at
  once from the bottom loop, where a piece knows nothing it found out before. The thread stays inside
  `Execution::run` and makes no kernel entry. The executor does this today for a thread whose code
  changed under it (`rerun`, wt/lib.rs:1729-1733, :2314-2317). Measured as "leaving `run` and coming
  back": 300.8 ns (docs/runtime.md section 10).
- **compiler fence**: an instruction across which a WebAssembly compiler does not merge, move or drop
  memory accesses. In wasmtime these are every atomic operation, `atomic.fence` and every call (7.6).
- **processor**: one of the CPUs the container is told it has. It is the kernel's own object. One guest
  thread runs on a processor at a time.
- **worker**: a host thread inside the loop. A worker is not a processor (4.3).
- **`run`**: the call that runs a container, as the owner named it (section 2). In today's code it is
  `Stepper::step` (rt/lib.rs:197). `run_until_finished` is today's `drive` and `Container::run`
  (rt/lib.rs:230, rt/container.rs:172). The executor's own call that runs one guest thread is always
  written `Execution::run` here (rt/lib.rs:128-143).
- **kernel entry**: one call into the kernel that changes it: a thread chosen for a processor (inside
  `Kernel::next`, 4.3), a thread coming back (`Kernel::exit`), an event, a signal from outside.
- **in the way**: a running thread stands in the way of a ready thread when the ready one could run if
  it left its processor (section 5).
- **due in**: a running thread is due in when it has a fair slice or the kernel has asked it in. It then
  makes a kernel entry of its own after a bounded time (section 5).
- **barrier**: a guest instruction that orders its thread's memory accesses as other threads see them:
  ARM64's `dmb` and `dsb`, x86's `mfence`.
- **position**: where a thread is, as (progress count, pc) (docs/runtime.md section 9).
- **reference**: a default Docker container on real Linux of the guest's architecture.

Paths are shortened: `kernel/` is crates/wcontainers-kernel/src, `abi/` crates/wcontainers-abi/src, `rt/`
crates/wcontainers-runtime/src, `wt/` crates/wcontainers-wasmtime/src, `tr/`
crates/wcontainers-translator/src, `arm64/` crates/wcontainers-arm64/src. Lines are at 77e50ddc (this
branch's base). `native.md` and `x86 plan` are docs/native.md at origin/phase-2-native ab448e58 and
docs/phase-2-guest-cpu-and-x86.md at origin/phase-2-x86 0de30f18. Where section 10 cites native.md "at
its head", that is a7099033, which has the owner's later word on where the embedder's system runs.
`interface notes` is
wcontainers-scratch/orchestrator/next-embedder-interface.md. Every figure says whether it was measured or
is only expected. Measurements are in section 14 and in docs/status-threads.md.

## 1. What this delivers

Today one guest thread runs at a time in a whole container. A build with sixteen jobs on a sixteen-core
machine uses one core. This plan lifts that on every executor, in this order:

1. **Tasks that overlap nobody run at the same time**, each `Serial` process still running one thread at
   a time. A build's compiler processes share no writable memory, so this alone uses the cores.
   Translated code does not change. A process that asks for exact replay still gets it, and one that
   does not pays nothing for it, whatever its neighbours asked for.
2. **The threads of one process run at the same time**, where its ordering says so, from a second form
   of code. The first form, for `Serial`, stays as it is. A `Serial` task and `Parallel` tasks that
   share memory with it take turns by the one rule of section 3.
3. The same on the native executor, and for x86-64 guests.

It also says what each replay guarantee asks of the kernel now that replay is per process, what the
embedder composes itself, and what is refused until it is built.

## 2. Rules, and what is already ruled

All rules of phase one hold: one primitive over parallel paths; no setting without a real use today; never
fake; every slow path has a name, a counter and a warning; run speed first; scope is whole architectures.

Ruled by the owner on 2026-11-18, and taken as given. Where the brief's and the interface notes' sections
disagree, the last one is followed (brief-threads.md lines 134-160, interface notes lines 155-181).
- **Two separate choices per process**, in `ExecutionConfig<E> { thread_ordering: ThreadOrdering,
  replay_determinism: ReplayDeterminism, translator: E::TranslatorConfig }`.
  - `enum ThreadOrdering { Serial, Parallel }`: how the threads of one process are run. `Serial`: one at
    a time. `Parallel`: at the same time.
  - `enum ReplayDeterminism { Exact, External, None }`: what a replay of the process promises. `Exact`:
    identical registers and memory at every logged position. `External`: "Consistent in everything the
    process does to the outside; no guarantee about its internal state." `None`: nothing.
  - `ParallelOptimistic` is no longer a value: it is the pair (`Parallel`, `External`). The getter
    `ThreadOrdering::replay_determinism()` is gone. The guarantee is a field.
- **The progress count is paid only for `Exact`.** Translated code holds the count if and only if the
  process was given `Exact`. That is derived from the field, never a switch of its own, and it is part
  of the translation cache key. With `None` nothing is recorded.
- **(`Parallel`, `Exact`)**: the kernel does its part (the count, and an exact position for everything it
  controls). The order of memory accesses between threads is the embedder's, through per-thread memory
  watches (`memory_watch_thread` in that ruling, on executors that implement `PerThreadWatch`).
- **There is no setter for the ordering.** A process is given its config when the container starts and
  when it is resumed. The getter is `Process::execution_config()`. How a process is taken out and put
  back is now by task (below, 2026-11-19).
- Hooks receive `Process<'_, E>` by value. The call that runs a container is `run`. It returns
  `StopReason { Interrupted, Suspended, Idle { until }, Finished { status } }`. Today's run-to-the-end
  call becomes `run_until_finished`.
- **Processes that share memory need not be on one kernel.** The owner dropped the "process closure" as a
  hard rule. Replay of processes that share memory across kernels is the embedder's concern (interface
  notes lines 158-162). Since 2026-11-19 one task may also be on another kernel than its siblings
  (below).
- Different processes that share no writable memory always run in parallel with each other.
- The kernel is the same under every executor. It is entered by one caller at a time to begin with. The
  interface must not prevent several at once later.
- Using cores by running more kernels of one container is rejected.
- Long-lived configuration types end in `Config`: `RuntimeConfig`, `TranslatorConfig` (brief-threads.md
  lines 124-129). The code still says `RuntimeSettings` and `TranslatorSettings` (rt/container.rs:13,
  :23); the runtime track renames them.
- **Inspection follows Linux's own line**, the `ptrace_may_access` check (interface notes lines 104-109,
  118-119). Inspection is: `/proc/<pid>/mem`, `maps`, `smaps`, `environ`, `auxv`, the address fields of
  `stat`, ptrace itself, `process_vm_readv` and `process_vm_writev`, a crash dump. Not inspection:
  `stat`, `status`, `cmdline`, the thread list. It breaks `External`, and the hooks
  `on_inspection_starting` and `on_inspection_ended` announce it. They are the embedder interface
  track's to build. What this plan adds is only which threads are stopped first (4.5).

Ruled by the owner on 2026-11-19, from "Ruled by the owner, 2026-11-19: where replay is recorded" to
the end of the interface notes. Each heading is quoted where the plan relies on it.
- **Replay is per process, at the process's own system calls** ("where replay is recorded"). A copy of
  a process is built from a memory transfer (a snapshot, or changed pages) and from running the process
  again as a pure function of its recorded system call results. Its log is those results, plus what
  reaches it without a system call: signals at a counted position, and shared memory through watches.
  It is not the kernel's outer boundary. A process given `None` costs nothing and breaks no neighbour;
  an unrecorded process that writes to a recorded one is covered because the reader's read result is
  logged. "A kernel is recorded whole or not at all" (this plan's earlier decision 1) is withdrawn. Not
  yet designed: the two kernel primitives this needs (4.6).
- **The system call proxy** ("the system call proxy and the kernel-write hook"):
  `SystemCallRequest { number, arguments: [u64; SYSTEM_CALL_ARGUMENT_COUNT] }`, `SystemCallResponse =
  Result<u64, Errno>`, `System::intercept_system_call_request(&self, process, thread, request) ->
  ControlFlow<SystemCallResponse, SystemCallRequest>` and `System::intercept_system_call_response(&self,
  process, thread, &request, response) -> SystemCallResponse`, each with a default that changes nothing.
- **The kernel's writes** (same heading): `on_memory_written_by_kernel(&self, process, thread, range)`
  fires after every write the kernel makes into a process's memory, with the exact range. Watches and
  dirty marks treat the kernel's writes exactly like the program's own. `memory_changed` is renamed
  `take_dirty_memory`, and method names are verb first (`read_memory`, `write_memory`, `watch_memory`).
  Watches are `Process::watch_memory(&mut self, range, watches: WatchesFor)` with `WatchesFor { None,
  Writes, All }`, and the hook `on_watched_memory_accessing(&self, process, thread, address, kind:
  AccessKind)`, which the kernel's own accesses to watched memory fire too. It replaces
  `on_memory_accessing`. The embedder's own writes fire neither hook.
- **Signals as an input** ("signals as an input"): `on_signal_delivering(&self, process, thread,
  &SignalInformation)` before the kernel delivers, and `Process::send_signal(thread, signal)`. In the
  middle of computing a signal's position is the count under `Exact`; otherwise the embedder takes a
  fresh snapshot there. "Later, with the threads plan: reading a thread's progress count and stopping a
  thread at a given count (needed only under Exact)" (decision 12).
- **Optimistic replay stays** ("optimistic replay STAYS (owner)"): `External` stays in the interface, as
  a mode chosen per program. The prototype's figures that this plan's earlier decision 3 leaned on are
  withdrawn: its replayer invented failures, and its comparison was stricter than the owner's
  definition. They are being measured again.
- **One scheduling rule, inside the kernel** ("threads plan decision 1 reworked" and "shared memory is
  announced per process"): a task whose code is `Serial` is given a processor only while no task that
  overlaps its memory has one; `Parallel` tasks may run beside their overlappers. It replaces groups in
  force, the config in force and "the strictest wins". The replay guarantee is never forced onto a
  neighbour. The embedder chooses orderings and builds its own view from the hooks; nothing about the
  rule is on the interface (section 3).
- **Shared memory is announced per process** (same heading): `MemoryObject { File { device, inode },
  SystemVSegment(id), Anonymous(id) }` with branded ids, and the hooks `on_shared_memory_mapping` and
  `on_shared_memory_unmapped(&self, process, source: MemoryObject, offset, range)`, fired per process for
  mmap, shmat, a mapping inherited at fork, and the ends, only for sources some process can write
  through a mapping. They replace the kernel's report of which processes share memory.
- **Suspend and resume by task** ("suspend and resume by task, and the process context"): a `Kernel`
  trait with `suspend_tasks`, `suspend_process`, `copy_process_context`, `resume_process(context,
  config)` and `resume_tasks`, the types `SuspendedTask` and `ProcessContext`, and the same calls on a
  `Process` handle, limited to its one process. A batch is taken as of one moment, and how the kernel
  stops the tasks is its own. Suspending some tasks in a hook does not use up the handle; taking the
  process does. Resume stays outside hooks. They replace `Process::suspend(self)` and
  `SuspendedProcess<E>`.
- **Per-thread distribution is core** ("the embedder-side view `WorkloadPlacement`"): the kernel must be
  able to move one task apart from its siblings.
- **Locks across machines** ("locks across machines", agreed in substance, details open): on the process
  handle, `wake_waiters(address, count, bitset)` and `move_waiters(from, to, count)` (4.5).

**What each pair means, and what it pays.** The count costs translated code 11.9% to 16.3% more
instructions in its best measured form (orchestrator/progress-count-cost.json, form 3, the x86 laptop
and this Mac). So the pairs differ in cost, and the plan keeps the cheap ones cheap. What is listed here
is what a process is given, and it is what the process runs under, whatever its neighbours were given.
What is logged, and where, is the embedder's, through the proxy and the hooks above (4.6).
- (`Serial`, `None`): today's run without its count. One thread of the process at a time. Nothing is
  recorded and nothing is asked of the embedder.
- (`Serial`, `External`): the same code. The embedder logs the process's system call results, and a
  replay is checked against the log and thrown away when it differs (4.6).
- (`Serial`, `Exact`): code with the count. What reaches the process without a system call is logged at
  a position (4.6).
- (`Parallel`, `None`): the process's threads run at once. Nothing is recorded.
- (`Parallel`, `External`): as (`Serial`, `External`), and each locking or atomic instruction also takes
  its place in one order. That needs a third kind of translated code, not built. Until it is, the
  process runs as `Serial` (section 3), which keeps the same promise.
- (`Parallel`, `Exact`): code with the count. Every stop the kernel or the executor causes is at a
  position. The order of memory accesses is the embedder's, through per-thread watches (section 8).

## 3. Memory that tasks overlap in, and the one scheduling rule

The owner's rule ("threads plan decision 1 reworked", made a ruling in "shared memory is announced per
process"): **a task whose code is `Serial` is given a processor only while no task that overlaps its
memory has one. `Parallel` tasks may run beside their overlappers.** It is kept inside the kernel, with
nothing on the interface. It replaces this plan's groups in force, the config in force and "the
strictest wins", and with them the hook `on_execution_config_in_force_changing`: no config changes
inside a kernel, and the embedder learns of each overlap from `on_shared_memory_mapping`, fired per
process in the entry that makes it. The rule below is the one the mixed-ordering check found sound
once corrected, with its corrections in. Its protocol has not been modelled (known fault 1).

**Which memories overlap.** A guest memory is one address space (`MemoryId`, abi/cpu.rs:5-7). The threads
of a process share one, and so do processes made with `CLONE_VM` (kernel/process.rs:70-72). Beyond
that, memories overlap through blocks:

- A block is **writable through a mapping** when some memory has a shared mapping of it (`mmap` with
  `MAP_SHARED`, System V shared memory, a shared memory file) that may write. "May write" means what
  `mprotect` may later give the mapping, Linux's `VM_MAYWRITE` (`Mapping::allowed`,
  kernel/memory.rs:74-75), not the permission it has now. So `mprotect` never changes an overlap.
- **Every memory that shows a page of such a block overlaps every other**, whether its own mapping of
  it is shared or private. A private file mapping shows the file's live bytes until the page is first
  written (kernel/memory.rs:44-50; `map_shown` maps the block where `mapping.shared || !own`,
  kernel/shared.rs:476-487). So a writer's stores reach it with no kernel entry in between.
- A block that no mapping may write makes no overlap. This is the common case: the kernel shows every
  file mapping through a block, the program's own code and its C library included (kernel/shared.rs:1-5,
  kernel/loader.rs:901-905). Its bytes can still change through `write` to the file. That is a kernel
  entry, and 4.5 says who is called in for it.

Two tasks overlap when their memories are one memory or overlap directly. Overlap is not followed
further: if A overlaps B and B overlaps C, A and C may run at once. Nothing they share can carry a store
between them without B's memory, and the rule keeps a `Serial` task alone with each of its own
overlappers. (The form before joined such memories into one group. The mixed-ordering check reasoned
about one set of sharers, and did not look at chains; that the direct rule is enough for them is this
revision's reading, unchecked.)

Overlap is by block, not by page. Two processes that map different parts of one file, one of them
writable, overlap although they share no page. That is rare, and a task that overlaps too much waits
more, never wrongly. Rejected: overlap by page. It makes every `mmap`, `munmap` and first private write
a question about pages, to save a case that hardly occurs.

**Who keeps it.** The kernel. It already knows every mapping, which are shared and what each may be given
(`Backing` and `Mapping`, kernel/memory.rs:34, :69). It keeps, for each block, the memories that show it,
and so for each memory the memories it overlaps. An overlap starts inside the kernel entry that makes
the mapping, or the fork that inherits it, and ends in the entry where the mapping has gone. The same
entries fire `on_shared_memory_mapping` and `on_shared_memory_unmapped` for each process, which is how
the embedder builds its own view.

**A new memory has one task.** A memory is born at the container's start, at a fork whose child shows no
block that is writable through a mapping, and at exec, which makes a new memory (kernel/loader.rs:292)
after ending the process's other threads (kernel/task.rs:648). For a task that overlaps nobody,
`Serial` and `Parallel` are the same run. The ordering starts to matter at the second task that
overlaps it.

**Overlap ends at the kernel's edge.** Processes, and since the ruling single tasks, may be on another
kernel than those they share memory with (section 2). The rule counts the tasks this kernel holds. What
crosses to a task elsewhere is the embedder's, through the shared-memory hooks and watches. Nothing in
this plan needs the other kernel.

**What runs as `Serial`.** The rule names a task's code. A task runs as `Serial`:
- **when its code is `Serial` code.** A memory has one form of code (7.2). It is `Serial` code when any
  process that owns the memory was given `Serial`. Processes that own one memory (`CLONE_VM`) therefore
  take turns when any of them was given `Serial` (the mixed-ordering check: a second form on one
  memory is not possible). Counted "Parallel process runs Serial code: it owns a memory with a Serial
  one", warned once per process. This is the one place where a neighbour's ordering reaches a process,
  and it is the ordering only; a guarantee never does;
- **when the executor cannot run it beside others**, which the executor says of itself
  (`Cpu::parallel_limits()`, 7.4): on the WebAssembly executor a writable block shown in two places
  of one memory, or in two memories that both have `Parallel` code (7.4); on that executor too, a
  `Serial` task beside `Parallel` ones that share a block with it, until the copy at each switch is
  built (7.4); an x86 guest on a host that orders memory more weakly (section 9); and (`Parallel`,
  `External`) until its form of code is built (4.6, decision 3). Each is counted and warned once per
  process.
This is a fact about one task, worked out from its own process's config and the executor, inside the
entry that changes it. It is not a config of anything, and no hook announces it. (The mixed-ordering
check says "the ordering half of the config in force stays" for these limits. The ruling replaces the
config in force. The plan keeps the limits as this per-task fact and keeps nothing else of it.)

**The rule, as the mixed-ordering check corrects it.** For a task S that runs as `Serial`:
1. **Exclusion is by "handed out", not by "in guest code".** S is handed out only when no task that
   overlaps it is handed out. While S is handed out, no overlapper is handed out, no overlapper's
   waiter is chosen, and any other kernel write into S's memory calls S in (4.5).
   - Why. Worded as "in guest code", the rule is unsound. A (`Serial`, one thread) and B (`Parallel`,
     four threads) map one file with `MAP_SHARED`; on the native executor these are the same frames.
     B's thread b1 enters the kernel for `FUTEX_WAKE_OP` on word w and waits for the kernel's lock: it
     is not in guest code. A runs `lock xadd [w],1`, which the native `Serial` form makes a plain read
     and a plain write (section 10). b1's kernel write lands between them, and both additions give
     w = 1, which x86 never produces. ARM64 has the same hole through the barriers the `Serial` form
     leaves out, and the WebAssembly executor through A's inline 128 bit compare and swap (7.3). A
     woken call of B's that runs again and writes a shared buffer does the same.
   - Overlappers then run only while S is parked or not on a processor. None can be handed out while
     S is inside a system call answered in place: a hand-out needs the kernel's lock, which S holds
     for the whole call (4.2).
2. **Overlappers are called in, never held.** Holding them in place with a group stop for S's turn
   hangs. They would still count as running, so S, handed out with nobody ready, gets no slice
   (kernel/task.rs:43) and, if it only computes, nobody else gets a processor (section 5). And the
   stop lock kept across S's turn would have S ask for the kernel's lock under a stop lock at its next
   system call, against the lock order (4.5).
3. **A `Serial` task that is due a turn stops hand-outs to its overlappers.** S is due a turn when it is
   ready and no ready task that overlaps it has less weighted time (ties by task id). While S is due,
   the memories of its handed-out overlappers carry a call-in mark (4.5), so they put forward only
   their waiters. Why: with S ready behind B's four threads, section 5 asks one of B's threads in, and
   its processor would go straight back to B, which puts forward every ready thread; S would never run.
   Basing "due" on the least weighted time is how fairness between S and its overlappers follows the
   kernel's fair scheduler. That is this plan's choice, not ruled (decision 15).
4. **The kernel's rules follow the phase.** While S is handed out, S's memory and every block it shows
   are under the `Serial` rules of 4.5: a kernel write into them from outside calls S in. While only
   `Parallel` tasks are handed out, they are under the `Parallel` rules: the compare and exchange for
   the words the kernel changes, a group stop or nothing. `next` chooses an overlapper's waiter, and
   does what is pending for it (a woken call run again, a signal frame written), only while no task
   that runs as `Serial` and overlaps it is handed out.

**Among tasks that run as `Serial`.** Two such tasks that overlap take turns, as two `Serial` groups did.
The threads of one `Serial` process are such tasks (one memory), so they run one at a time, as today.
Rule 3's "due a turn" is also what keeps a pinned task from starving: a ready `Serial` task is put
forward only when it is due a turn, so a task that gave way for an overlapper pinned to another
processor is not handed back at once while the overlapper has less weighted time. This is the form
before's "the group's next thread", said for overlaps (known fault 5, 4.3).

**The three ways a mixed pair runs.** For a `Serial` task A and `Parallel` tasks B that overlap it, the
embedder chooses by the orderings it gives, and nothing else is needed (the ruling: "the three ways to
run a mixed pair are exactly the three combinations of orderings the embedder gives"):
- both `Serial`: one task of the pair at a time, on every executor, with O(1) switching and the hook
  guarantee whole (4.4 item 7). Wins on any WebAssembly executor under `CLONE_VM`, when A rarely
  blocks, and when B has many threads and A blocks only briefly;
- mixed (the rule above): B's threads run at once while A is parked or off its processor, with no new
  translation and no slowdown of A's own code. Wins on the native executor with a mostly blocked A
  beside a compute-heavy B, sharing through `MAP_SHARED` or `shmat`; on the WebAssembly executor only
  when the copy at each switch costs less than it gains;
- both `Parallel`: A and B at once. A pays the `Parallel` form all the time (7.2, 7.6, section 10, all
  measured), and if A is `Exact` it needs the per-thread watches of section 8 (about 1 µs a page
  hand-over, expected). Wins under `CLONE_VM`, when A runs often, and on any executor with one memory.
Which wins depends on how often A blocks, how many threads B has, and the executor. There is no single
answer, and the plan gives none.

**What each executor needs for the mixed case** (the mixed-ordering check, section 2 of its report):
- the kernel: overlaps as above, the four corrections, and rule 3's marks; the model of call-ins redone
  for them (known fault 1);
- the native executor: nothing new beyond the per-thread interrupt and the two forms (section 10). The
  hand-over between A and B is ordered by the kernel's lock and the exception return, so the `Serial`
  form may keep emitting no barriers. The interrupt from one virtual CPU to another must be measured;
- the WebAssembly executor, for `MAP_SHARED` and `shmat` only: at each switch to A, all of B's threads
  come in, and B's written shared pages are copied from B's memory to the block and on to A's memory
  (7.4); B's `runs_in_parallel` byte follows B's own threads (7.3); `Cpu::parallel_limits()` says "can
  alternate", distinct from "Serial only". Expected to work, not built (known fault 8);
- a browser: as the WebAssembly executor, untested;
- `CLONE_VM`, any executor: one form on one memory, so the owners take turns (above).

**What it costs.** Measured, each cited where this plan measured or quotes it: leaving `Execution::run`,
300.8 ns a thread (docs/runtime.md section 10); noticing a stop, 230 ns on average and 30 µs at worst
in Chrome (4.5); an interrupt from the host into the machine, 2.4 to 4.1 µs (section 10); the kernel's
share of `getppid`, 116 to 120 ns, and the kernel's lock, 0 to 11.4 ns (4.2). Expected, not measured:
- each block and wake of A costs N call-ins and N hand-outs for B's N handed-out threads, each a kernel
  entry on the one lock, while every other entry waits (4.4 item 6). A's wake waits for the slowest of
  B's threads to reach a stop check, a thread that is compiling included. Both `Serial` pays one thread
  out and one in;
- the interrupt from one virtual CPU to another: not measured;
- on the WebAssembly executor, a copy of the dirty shared pages at every switch;
- nothing on a system call answered in place: it pays only the lock, so "`Serial` must not get slower"
  holds;
- no gain when B is (`Parallel`, `External`): it runs as `Serial` until its form is built.

**How a new overlap takes effect.** A mapping, an exec whose program files are mapped so, or a `resume`
can make two memories overlap where one side runs as `Serial`. Then every handed-out task of the other
side, other than the caller, is called in first (4.5), and the change is made in the first entry in
which the rule holds.
- Until they are in, the call that caused it is parked before it has had any effect: the `mmap` or
  `shmat` that makes the mapping (kernel/syscall/memory.rs:109, ipc.rs:356), or the exec. For a
  `resume`, the resumed tasks are not handed out. `mremap` and `mprotect` change a mapping inside one
  memory and make no overlap. A fork's child inherits its parent's overlaps, and it is new: it has no
  running thread.
- Why. The form before's fifth check: process P1 runs on one processor with a file mapped privately; P2,
  on another, maps the same file shared and writable; both carry on, and two tasks that run as `Serial`
  and overlap are in guest code at once. That breaks 4.4 items 3 and 7 and the executor's lock rule
  (7.2).
- Where both sides have `Parallel` code and no limit applies, nobody is stopped. An overlap that ends
  stops nobody either.
Counted "tasks called in for a new overlap".

**Withdrawn with the rulings.** "A kernel is recorded whole, or not at all", and with it the kernel's
order of entries, are withdrawn ("where replay is recorded"): replay is per process (4.6), so no
process's entries need to be logged for another's sake. The sixth check's case that the rule was made
for (an unlogged process moving the scheduler under a replay of the whole kernel) cannot arise, because
the kernel is no longer replayed whole.

**What stays for the embedder.** Which of the three ways to use, by the orderings it gives; its own view
of tasks and memory, from the shared-memory hooks and the process and thread hooks; fairness between A
and B if it does not take the plan's choice (decision 15); and, for an `Exact` A, A's inputs from shared
memory. No hook fires when A is handed out, and on the native executor B's writes set B's dirty marks,
not A's. So it needs either a moment before A's turn with the shared ranges others changed since A last
ran, or a way to take dirty memory across every place that shows a page (decision 16). Changes that land
at A's own stops also need A's progress count, which exists only under `Exact`; under `External` the
embedder takes a snapshot there, as for signals.

## 4. The kernel entered by several host threads

### 4.1 Today

- Every entry point takes the whole kernel exclusively: `next`, `exit`, `event`, `kill`, `restore`
  (kernel/lib.rs:305, :317, :344, :397, :422) take `&mut self`, and `exit` takes the whole executor and
  the whole system exclusively too (`&mut dyn Cpu`, `&mut dyn Host`).
- The kernel models one CPU: one current thread and one running thread (kernel/lib.rs:136-140), one
  "handed out at" time for charging (kernel/lib.rs:161-163, kernel/task.rs:77-86), Linux's fair scheduler
  "for one CPU" (kernel/schedule.rs:1). `next` and `kill` assert that no thread runs
  (kernel/lib.rs:306-309, :398).
- The guest is told it has one CPU: `sched_getaffinity` (kernel/syscall/system.rs:161-174), `getcpu`
  (:192), `/proc/cpuinfo` (kernel/fs/proc.rs:1352-1355), `rseq` (kernel/syscall/rseq.rs:13-17).
  `membarrier` does nothing, "threads take turns on one CPU" (kernel/syscall/rseq.rs:1-4).
- A signal sent to a thread assumes the only running thread is the sender (kernel/signal.rs:489), and a
  fatal signal ends the target process on the spot (kernel/signal.rs:481-485).
- The kernel's state is reached through `Rc` and `RefCell` on 155 lines in 29 files (`grep -rn
  "Rc<\|Rc::new\|RefCell" crates/wcontainers-kernel/src`). So the kernel can be neither shared between
  host threads nor moved to another one as it is.
- The kernel never waits on its own stack: a call that cannot finish parks its thread and returns
  (kernel/task.rs:1-6). This is what makes one lock cheap: it is held only while the kernel computes.

### 4.2 The first form: one lock, behind an interface that does not need it

**The interface changes now, the locking later.** The kernel's entry points take `&self`. `Cpu` and
`Host` methods take `&self`, and both traits require `Sync`. The lock is inside the kernel. So when the lock
is later split (4.7), nothing outside the kernel changes.

Rejected: a lock around the kernel in the runtime, with the kernel's methods left `&mut self`. It is less
work now, and it writes "one caller at a time" into the interface, which the brief forbids. Every `System`
and every executor would change again at the split.

**What is locked.** Everything in the kernel, by one lock, held for the length of one kernel entry. The
lock's kind comes from the build through `lock_api::RawMutex` (the `lock_api` crate, MIT OR Apache-2.0),
because the kernel is `no_std` (kernel/lib.rs:9) and is entered from three places that wait differently:
- a host program: `parking_lot`;
- the inside of the native executor's virtual machine, which waits inside (native.md section 5.3);
- the kernel built as a WebAssembly module (docs/runtime.md section 13, "final form"). With several
  callers that module has one shared memory and one instance per caller, each with its own stack, and
  the lock waits with WebAssembly's `memory.atomic.wait32`. In a browser each caller is a worker (7.6).
  Asked of the kernel track's K7: a build for a shared memory. Until it exists that form has one caller
  and one processor.

**The lock knows its holder.** Today a call into the kernel from inside a hook cannot be written: every
entry point takes `&mut self`. With `&self` it can, by an embedder that keeps a reference to the kernel,
and the lock is held there. `lock_api::RawMutex` has `lock`, `try_lock`, `unlock` and `is_locked`, and
nothing that says who holds it; other workers hold it rightly all the time. So without more, such a
call waits for ever on its own host thread (the fifth check's finding). So:
- the build gives, beside the lock's kind, a way to name the calling host thread: `lock_api`'s
  `GetThreadId`. In a host program that is `parking_lot`'s `RawThreadId`; inside the virtual machine,
  the virtual CPU's number; in the kernel built as a WebAssembly module, a value each instance holds;
- the kernel writes the holder's name into one word after it has taken the lock, and clears it before
  it lets go. Only a caller that finds the lock held reads the word;
- a caller that finds itself the holder is inside a hook, a `Host` call or `arrived`.
  `kernel.process(id)` then answers an error (section 5). Every other entry point panics and names
  the call: it is the embedder's error, the one the compiler refused until now.
- What it costs: a name read and two plain stores at each lock. Measured alone (section 14): nothing
  that could be told from the noise on either machine, around a lock that costs 0.4 to 3.5 ns on this
  Mac and 0.6 to 10.0 ns on the x86 laptop in the same runs. It is counted with the lock in T1's bar
  (section 12).
- On the native executor the kernel is inside the virtual machine, and the name is the virtual CPU's
  number. That is sound only if an entry stays on its virtual CPU from the take of the lock to its
  release. A `Host` call that needs the host leaves the machine and comes back on the same virtual
  CPU (native.md section 5.4). A container's stack is parked and taken up by another virtual CPU
  when it waits in `idle` and at a slice's end (native.md section 5.1). Asked of the native track:
  never between the take of the lock and its release.
Rejected: a lock that lets its holder in again (`lock_api`'s `ReentrantMutex`). A hook would then
change the kernel in the middle of an entry. Rejected: the word only in builds for tests, and a wait
for ever otherwise. It is cheaper by nothing that was measurable.

**A worker inside a guest hook that is not a stop point may not enter the kernel.** The holder's word
catches a host thread that asks again for a lock it has. It does not catch this case, which the sixth
check found. Three of the system's methods are called from inside guest code, in hooks that are not
stop points (4.5): `ran` and the clock's answer (`Host::monotonic_nanoseconds`) from the clock hook, and
`cpu_warning` from `warn` (rt/lib.rs:291-299). The worker has no lock of the kernel's there, and in a
group in force `Parallel` its thread still counts as in guest code (its mark is on, 4.5).
- What goes wrong. Thread X is in the clock hook. Its system's answer calls `kernel.process(id)` and
  waits for the kernel's lock. Worker Y has that lock for an `mprotect` in the same group, and its
  group stop waits until every mark is off, X's included. Neither moves.
- The rule, in the doc comment of each of the three: it may be called from inside guest code; it does
  not call the kernel, and it does not wait for anything that a caller of the kernel may hold. How long
  it takes is how long a group stop of that memory can be kept waiting.
- The first half is checked. Each worker has a mark of its own, "inside a guest hook that is not a stop
  point". The executor sets it before such a hook calls the system and clears it after: two plain
  stores into the thread's own record, which the hook has in hand already (wt/lib.rs:2124-2131). They
  are made on the clock hook and on `warn` only, in every group, so that an embedder's mistake shows
  under `Serial` as it would under `Parallel`.
- Every way into the kernel but the loop's own looks at the calling worker's mark, and panics with the
  call's name when it is set: `run`, `kernel.process(id)`, `process_call_in`, `kill`, an event handed
  in, `resume`. The kernel finds the mark through the same piece of the build that names the calling
  host thread.
- The loop's own entries do not look. `run` looked when it was called, and hands `Kernel::enter` a
  value that says so (suggested `Kernel::enter(checked)`, the value made only by a check). So a system
  call pays nothing for this. Measured for this choice (section 14): the same test made at every take
  of the lock moved a round by -0.1 to +0.1 ns on this Mac. On the x86 laptop it moved a round by
  -0.7 ns with work that stays in registers and by +0.9 to +1.0 ns with work that walks memory, the
  same in three runs. That is too near the lock's own cost to put on every system call.
- The second half cannot be checked: a system that takes a lock of its own in `ran`, while another of
  its threads holds that lock and calls the kernel, waits the same way. It is the doc comment's rule.
- On the native executor the clock is answered by the inside (native.md section 5.4), and `ran` and
  `cpu_warning` are the policy's, also inside (native.md section 6.2, R3). Where the inside calls the
  policy from guest code with no kernel entry, the same rule and the same mark hold, with the mark
  kept for each virtual CPU.
Rejected: the clock hook made a stop point. Its thread could then be held inside it. But a stop point
saves every register before the call and proves its pages again after it (4.5), in both forms of code,
at every counter read, and programs read the clock in their inner loops.

**One lock a system call.** A system call answered in place is two entries today: `exit`, then `next`,
with `System::arrived` between (rt/lib.rs:264-270). The loop takes the lock once for both. It asks the
kernel for a short-lived handle (suggested `Kernel::enter() -> KernelEntered<'_>`), calls the entry
points on it, and the lock is let go when the handle is dropped. `arrived` is handed that handle, to
give the kernel its events through, and runs with the lock held, as a `Host` call does. After the split
the handle keeps whichever locks its entries took.

**A thread that leaves takes the handle with it.** The executor has its own lock for a `Serial` process,
kept for a thread's whole stay in `Execution::run`, and it must let that lock go before the kernel's
lock is let go whenever the thread does not carry on (7.2). Today's code has the order the other way
round, as the sixth check showed. The kernel entry is made inside `Answer::exit`, which the executor
calls from inside its loop (wt/lib.rs:2325). The answer calls `kernel.exit`, `arrived` and
`kernel.next` and returns `Carry::Leave` (rt/lib.rs:264-289). Only then does the executor put the
process's store back (wt/lib.rs:2336-2344). With the handle taken and dropped inside the answer, the
kernel's lock would be free while the executor's is still held.
- So `Carry::Leave` carries the handle: `Carry::Leave(why, entered)`. The answer no longer drops it. The
  contract of `Execution::run` gains one line: on `Leave`, let go of everything held for the thread,
  then drop the handle. `Carry::On` carries none: the answer drops the handle itself, and the thread
  keeps its process's lock, as 7.2 allows for a thread that carries on.
- The kernel's answer kept for the worker's next step (`kept`, rt/lib.rs:191-194) stays as it is. It
  was made inside the same entry.
- On every path that leaves: another thread runs next, new registers, nothing to run, the container
  finished, the system asked for the thread out, and an entry told `Later` (4.6).
- It costs a system call nothing: a call answered in place returns `Carry::On`. A leave moves one
  more value.
- The try-lock of 7.2 and its test (section 12) check the order in every run of the stress schedule.
Rejected: the answer calls the executor back to let its lock go before it drops the handle. That is a
second way from the loop into the executor on every leave, and the order would then be kept by two
pieces of code in two crates. With the handle in `Leave` it is one line in one function.

**What the lock costs one thread.** It depends on the work around it, so only the system call measured
end to end can settle it, and that needs T1 built (section 15). The lock alone was measured twice
(section 14), each time as a compare and swap to take it and one to let it go:
- this plan's loop: nothing that could be told from the noise on this Mac, 7 to 9 ns on the x86 laptop;
- the checker's loop: 0 to 3.6 ns on this Mac and 0.6 to 11.4 ns on the laptop. The low figure is with
  work around the lock that walks memory, the high one with work that stays in registers.
The kernel's share of `getppid` is 116 to 120 ns on this Mac (docs/runtime.md section 18); it was not
measured on the laptop. So the lock is expected to cost a `Serial` system call between nothing and a few
per cent on this Mac, and more on an x86 host. That cannot be removed while the kernel is `Sync`. It is
put to the owner (decision 7).

**Everything else T1 could add to a `Serial` system call, and how each is kept off it.** The lock must be
the only cost, so each of these is designed away, and T1's bar checks that it is (section 12).
- *Asking the system whether an entry may take effect* (4.6) would be two indirect calls a system call:
  one for the thread coming back, one for its hand-out. The kernel asks only a system that said at the
  start that it orders entries. In any other kernel each entry tests one value.
- *The executor's locks* (7.2). A thread of a `Serial` group takes its process's lock once, when it
  enters `Execution::run`, and keeps it until it leaves. Its hooks take nothing.
- *The system's own locks.* `System` methods take `&self`, but what a system call touches in the
  shipped system is already atomic: `turn` stores one deadline word (rt/system.rs:256-266), and `ran` and
  `arrived` do nothing (:268-275). It keeps one such word per processor and adds no lock.
- *The stop word* (4.5): one plain load and a branch at each kernel entry, after the lock is taken.
- *The marks of a group stop* (4.5) are kept only for a memory in force `Parallel`: one flag tested in
  each hook that is a stop point.
- *The holder's word* (above): written and cleared with the lock, and read by nobody on this path.
- *The worker's mark for a guest hook* (above): looked at when `run` is called, never at a system call.
- *The handle in `Carry::Leave`* (above): a call answered in place returns `Carry::On`.
- *The scheduler's condition* (section 5): one mark tested when the lock is let go. A call answered in
  place for a thread that carries on writes nothing the condition reads, so the mark is clear. The
  kernel walks its processors and its ready threads only when the mark is set and a count says a
  breach is possible.
- *The kernel's deadline* (section 5): one value compared when the lock is let go, and a call to the
  system only when the earliest deadline has moved. Today's hand-out folds that deadline into every
  slice (kernel/task.rs:55-60); that goes.

Rejected: a kernel type without a lock for containers with one processor. It needs the kernel compiled
twice into one program, or the embedder to choose at compile time.

**The state stays `Rc` for now, and nothing that leaves the kernel shares one.** One `unsafe impl Send`
on the kernel's state, with its reason: every `Rc` and `RefCell` is reachable only through the state,
the state is reached only under the lock, and no clone leaves it. A test checks that no public type of
the kernel holds an `Rc`, but for the one below. The handle a hook is given (`Process<'_, E>`) borrows
under the lock and cannot outlive it.

`suspend` is where that reason would break, as the sixth check showed. The interface track builds it,
and section 6 relies on it. It takes a process's records out of the kernel by ownership, as a
`SuspendedProcess` (interface notes lines 110-114). The embedder may hold that value, move it to
another host thread, or drop it, all outside the lock. Today those records hold `Rc` clones that
processes left in the kernel share: an open file description after a fork (`FileRef`,
kernel/file.rs:94), and the address map, the signal handlers, the descriptor table and the directories
of processes made with `CLONE_VM`, `CLONE_SIGHAND`, `CLONE_FILES` and `CLONE_FS`
(kernel/process.rs:243, :247, :251, :262). A clone or a drop of one outside the lock, while a worker
changes the same count under it, is a data race. Nothing is wrong in T1, which has no `suspend`. The
rule is asked of the embedder interface track, for the day it builds it:
- **What `suspend` hands out shares no `Rc` with what stays.** An object that only the suspended
  process holds moves out with it. For an object it shares with a process that stays, the kernel keeps
  the suspended process's reference itself, in a table under the lock, and the value holds a key.
  `resume` puts the references back. `export` turns each into an end, which it must do anyway for
  whatever crosses a wire.
- `SuspendedProcess` is then `Send` for the kernel state's own reason: every `Rc` in it is reachable
  only through it. A build for tests checks at `suspend` that each `Rc` in the value has no holder
  outside it.
- A `SuspendedProcess` that is dropped lets its kept references go under the lock, not where it is
  dropped: its `Drop` puts the key on a list that the kernel empties at its next entry.
- Cost: one table entry for each shared object at `suspend` and at `resume`. Nothing on a system call.
Rejected: a `SuspendedProcess` that is not `Send` and may be dropped only under the kernel's lock. A
`resume` from another host thread is what the value is for, and no compiler checks where a value is
dropped. Rejected: these five types made `Arc` before `suspend` lands. Every descriptor lookup hands
out a clone (kernel/file.rs:181, kernel/syscall/io.rs:33), so every system call on a descriptor would
pay two atomic counts for a call few processes ever see. A part's objects become `Arc` when that part
is split out (4.7), each measured, and from then on they can leave as they are.

Rejected for now: turning every `Rc` into `Arc` and every `RefCell` into a lock. It is the right end state
for the parts that are split out (4.7), and it costs every system call something. It is paid part by part
at the split, each measured.

### 4.3 Processors and workers

- The container has a fixed number of processors (section 11). The kernel keeps, per processor, what it
  keeps once today: the current thread, the give-way mark, the running thread, the picked runtime
  (kernel/lib.rs:136-140, :165).
- **A worker is not a processor.** A worker says which processor it is asking for:
  `Kernel::next(processor)`. In a live run each worker keeps one processor (section 5). In a replay the
  log says which processor each hand-out is for, and any worker makes it (4.6). So a run may be replayed
  with fewer or more workers than it was recorded with, and the guest sees the same processor numbers.
- One run queue, as today. `next(processor)` gives the processor its current thread if that need not give
  way (as `pick` does today, kernel/task.rs:18-41), else the waiting thread with the least weighted time
  that its group puts forward and whose mask holds the processor. Which thread it gives depends only on
  the kernel's state and the processor asked for.
  - A group in force `Parallel` puts forward every one of its ready threads.
  - A group in force `Serial` puts forward one thread, and only when none of its threads runs: a
    waiter if it has one, else its ready thread with the least weighted time. This is "the group's
    next thread". So inside a `Serial` group the order is the one-processor fair scheduler's, as it
    is today, but for a waiter, which goes first because a whole group stands still for it (4.5).
  - A group that is called in puts forward its waiters and nobody else (4.5). A waiter is put forward
    whatever is called in, its own group included. Being chosen runs its call again in the kernel. It
    goes back to guest code only when its own group is not called in.
  - Why one thread and not any. With any, a `Serial` group's thread that gives way for a sibling pinned
    to another processor is the only thread of the group its own processor may take, and is handed
    out again at once. The sibling never runs, although Linux would run both. With one, the thread
    that gave way is not put forward while a sibling has less weighted time.
  - What that costs: when the group's next thread is pinned to a busy processor, a sibling that could
    run on an idle one waits for it. Counted "Serial group's next thread waits for its processor
    beside an idle one". With no thread pinned, and with one processor, the two rules choose the same
    thread: the least of all is the least of its group.
- **`next` makes one kernel entry for each thread it chooses.** Choosing a thread changes the run queue,
  the processor's thread and the thread's processor, and the guest sees the last through `getcpu` and
  `rseq`. The entry is "this processor chose this thread". It takes its place in the order of entries
  like any other, and the system is asked about it when the thread is chosen, before anything is done
  for the thread (4.6).
  - What the kernel then does for the thread (the next item) is that entry's effect. A woken call that
    runs again can park once more, or end its thread. Then the entry hands nobody out, although it may
    have read or written bytes or written a frame. It still took effect, and it keeps its place in
    the order. The plan's earlier form called `next` an entry "when it hands a thread out". Its fifth
    check showed that those effects then had no place a replay could put them.
  - `next` then chooses again. That is the next entry, asked about by itself. It goes on until a thread
    is handed out or none is put forward.
  - When none is put forward it answers `Idle` and marks the processor idle (section 5). An answer of
    `Idle` with nothing chosen changes nothing the guest can see and nothing a replay must repeat, so
    it is not an entry.
- **What the kernel does for a thread that is not running, it does when the thread is chosen for a
  processor, and not before.** Today the end of every entry runs woken threads' calls again and puts a
  signal's frame on the stack of a thread that waits its turn (`settle`, kernel/task.rs:343-404). With
  one thread running, nothing else could be using that thread's memory. With several processors a
  sibling could be, in guest code, on another processor. So both are done inside `next(processor)`,
  once it has chosen the thread and before it hands it out. A thread of a `Serial` group is chosen only
  when none of its group runs, so its woken call and its signal frame never touch the group's memory
  under a running sibling. 7.2's rule for the executor's locks stands on this.
- `exit` names the thread that came back (today it is implied, kernel/lib.rs:318-321).
- Each thread is charged from its own "handed out at" time (section 11).
- `sched_getaffinity`, `getcpu`, `/proc/cpuinfo`, `/proc/stat` and `rseq`'s CPU fields answer from the
  processors.
- **A thread runs only on the processors its mask allows.** Linux promises that for `sched_setaffinity`,
  and `getcpu` and `rseq` would show a breach. Today the call accepts any mask that holds CPU 0 and keeps
  nothing (kernel/syscall/system.rs:178-190), which is exact for one CPU. With several:
  - each thread keeps a mask of processors, inherited at clone and at fork, and `sched_getaffinity`
    answers it;
  - `next(processor)` hands out only a thread whose mask holds that processor, and the idle processor
    woken for a ready thread (section 5) is one in that thread's mask;
  - a thread running on a processor that its new mask leaves out is asked in with `thread_stop`, and the
    call that set the mask parks until it has come in. Linux's call also returns only once the thread
    has been moved (`affine_move_task`, kernel/sched/core.c, waits for the move);
  - a mask that gains processors stops nobody. If the thread is ready and one of the gained processors
    is idle, that processor is woken when the entry ends (section 5).

Rejected: a host thread is a processor (this plan's first form). Which host thread asks first is the
host's timing, and a replay with fewer host threads could not give the same processor numbers.
Rejected: `next` without a processor, the kernel choosing a free one. A thread that carries on in place
after its system call must get its own processor's answer, or it leaves `Execution::run` at every call
(300 ns, docs/runtime.md section 10).

### 4.4 What a system call may assume

1. No other kernel entry is in progress (the first form). After the split (4.7): no other entry that
   touches the same things.
2. Its own thread is stopped, with its registers saved (tr/contract.rs:13-15).
3. In a group in force `Serial`, no other thread of its group is running guest code. A call that joins
   its group to another is run only once that holds for the group that results (section 3).
4. In a group in force `Parallel`, the group's other threads may be running guest code this instant. So:
   - a value read from guest memory may change the next moment. The kernel reads each value once and
     acts on what it read. The futex wait reads its word once, as one aligned 32 bit load;
   - what it writes may be overwritten or seen half-written by another thread, as on Linux;
   - the address map cannot change during the call, because every change to it is a kernel entry. A range
     the call checked stays mapped until the call returns;
   - another running thread's registers cannot be read (`Cpu::registers` is "only valid while the thread
     is not running", abi/cpu.rs:94). Nothing in the kernel reads a running thread's registers: a signal
     is taken by the thread itself when it comes in, and a tracer's or a crash dump's read waits for the
     thread (4.5).
5. Threads of other groups may be running. Two rules follow, and 4.5 applies them case by case:
   - **A thread of a group in force `Serial`, or in force `Exact`, stops only at a kernel entry of its
     own.** There are two reasons, one for each word.
     - `Exact`: the stop then has a place in the order of entries, and a position. A thread held in
       place has neither.
     - `Serial`: the running thread keeps its process's executor lock for its whole stay in
       `Execution::run` (7.2). That is what keeps locks off a `Serial` system call, and it only works
       if nobody reaches the process from outside while its thread runs.
     So the kernel never reads or changes the memory, the page permissions or the code of such a group
     from outside it while one of its threads runs. It calls the group in first. Neither does the
     executor, from another group's hook (7.5).
   - **A thread ends only inside a kernel entry of its own.** The kernel never ends a running thread from
     another thread's entry.
6. The kernel still never waits on its own stack. A `Host` call or a hook made inside an entry holds the
   lock while it runs. A slow hook stalls every processor's kernel entries until the split.
7. **When a hook about a process fires, no thread of that process's group runs guest code, if the group
   is in force `Serial`.** This follows from item 5: a hook fires inside an entry, and an entry that acts
   on a `Serial` group from outside it has called the group in first. It holds across a join too: two
   groups become one `Serial` group only once the other running threads of both are in (section 3). In
   a group in force `Parallel` the process's other threads may be running. Section 6 relies on this
   for `suspend`.
   - The owner's guarantee is that "a hook fires only when every operation on that process is legal"
     (interface notes lines 57-58). With the other threads running, that still holds for every
     operation in a group that is not in force `Exact`: where one needs them stopped, the executor
     holds them in place for it.
   - **It does not hold for two operations in one case.** In a group in force `Parallel` and `Exact`
     whose other threads run, `suspend` and a change of config need those threads stopped at positions.
     Only a call-in gives that, and a hook cannot wait for one. There the two answer an error (section
     6). The hooks this reaches are `on_thread_starting`, `on_process_outgrown` and
     `on_memory_accessing`, and `on_process_starting` for a child made with `CLONE_VM`, whose memory
     its parent's threads share. This narrows the ruled guarantee, so it is the owner's (decision 8),
     with what keeping it whole would cost.

One known gap, to be closed by reading every call in T2: a call that waited and runs again reads its
arguments from guest memory again (kernel/task.rs:166-169, :360-368), except paths and descriptors, which
are kept (`Taken`, kernel/process.rs:407-414). Linux reads them once. With one thread running, nothing
could change them in between. In a `Parallel` group another thread can.

### 4.5 Who is stopped before what

Three primitives do all of it.

**Stop one thread** (suggested: a `Host` service `thread_stop(thread)`). The kernel asks the system to
bring a running thread back, and does not wait for it. The thread comes in by itself, through an entry of
its own, after its next stop check. The request goes through the system, not straight to the executor,
because when a thread stops is one of the things a replay must repeat (4.6): the slice timer already
works this way (rt/system.rs:256-268).

**A request to stop is kept until the thread's next kernel entry.** Today one word does everything. Zero
means carry on, anything else means call the stop hook (abi/registers.rs:54-57). The executor writes over
the word each time round its loop (wt/lib.rs:2283) and in the stop hook (wt/lib.rs:1786), and a request
that lands in between is put back by `StopFlag`'s own latch (rt/lib.rs:64-74). With a word per thread and
requests from any host thread, nothing would put it back. A lost request is a process that is never
killed: a thread that only computes, alone on its processor, has no slice timer (kernel/task.rs:43-53,
rt/system.rs:256-266), and a group called in is woken only by that thread's own entry.

So the stop word becomes a set of reasons. Each is a bit that only its owner writes, with an atomic or
and an atomic and, never a plain store of the whole word:
- **asked in** (suggested `STOP_ASKED`): set by `thread_stop`, from any host thread. Only the thread
  itself takes it off, and only inside a kernel entry of its own, as the first thing after the kernel's
  lock is taken, whatever that entry is for. Whatever the kernel wanted the thread for (a signal, a
  group called in, a process ending, a new mask), it finds at that entry.
- **held** (suggested `STOP_HELD`): set and taken off by whoever has the memory's stop lock (below). A
  group stop therefore never uses up a request to come in, and a request never lets a held thread go.
- **the thread's own** (suggested `STOP_OWN`): its turn is counted by stop checks, or it is leaving
  `Execution::run` (wt/lib.rs:1726-1728, :1737-1740). Only the thread's own host thread writes it.

Translated code still tests the word against zero (tr/code.rs:1300). One place in it writes the word,
and that changes: `raise_stop` stores 1 over the whole word with a plain store (tr/code.rs:693-699,
tr/full.rs:2746-2750). Only the translator's test guest uses it
(wcontainers-translator-test/tests/test_guest.rs:361), but it would wipe a bit that another host thread
had set. It becomes an atomic or of the thread's own bit. The stop hook looks at each bit. Held: it waits
for the stop lock. Asked in: it makes an entry that hands the kernel `Exit::Stop`. Counted: it counts.

Why nothing is lost. The bit is taken off under the kernel's lock, at the start of the entry, before the
kernel looks at anything. The kernel asks under the same lock, so its request is wholly before that
moment or wholly after it. Before: this entry meets it. After: the bit is set, and nobody but the thread
takes it off, so the thread's next stop check brings it in. (The plan's earlier form took the bit off
before the lock was taken. A request could then land between the two, and its fourth check showed what
that did to the executor's locks; 7.2 no longer leans on the bit at all.)
- The slice timer asks outside the lock. The kernel looks at the slice at every return, whatever
  brought the thread back (kernel/task.rs:87-90), so an entry meets that request on either side of the
  take-off.
- A container stop (section 5) sets its own mark before it asks, and each worker looks at that mark
  after every entry.
- A thread asked in while it is not in guest code at all (between its hand-out and its first
  instruction, settling a fault, compiling) finds the bit at its first stop check.
- What can happen is a request that crosses an entry and leaves the bit set with nothing left to do.
  That costs one stop check that hands in an `Exit::Stop` the kernel has no use for, counted "stop
  asked for nothing". In a replay `thread_stop` does nothing (4.6), so the bit is never set.

What it costs a system call: one plain load of the word and a branch, after the lock is taken. The
atomic and is made only when the bit is set.

**Call a group in** (the kernel's). A call that needs groups stopped names every one of them at once:
each memory it will reach, and its own when it joins its group to another (section 3). The kernel asks
`thread_stop` for each of their running threads other than the caller, puts the call's mark on each
memory named, and parks the call.
- A group is called in while any call's mark is on one of its memories. `next` then hands none of its
  threads back to guest code. A mark names a memory, so marks follow a join or a split of groups with
  nothing done for them.
- The call is woken by the entry after which no group it named has a thread in guest code. Its thread
  is now a waiter.
- The call runs again when the waiter is chosen (4.3), from its start, as every woken call does
  (kernel/task.rs:1-6, :343-375). It finds nobody running, does its work in that same entry, and takes
  its marks off. A call that then has to wait for something else parks for that as any call does,
  with its marks off.
- Each thread that came in did so through an ordinary entry, at a position when its code has the
  count.

**Two call-ins never wait for each other.** The plan's earlier form let them, and its sixth check
showed how. Process P1 has threads A1 and S1, process P2 has thread A2, all `Serial`, on two
processors. A1 calls `write` on a file that P2 maps: P2's group is marked, A2 is asked in, A1 parks,
and S1 is handed out. A2 comes in through its own `write` to a file that P1 maps: that wakes A1, marks
P1's group, asks S1 in and parks A2. S1 comes in and wakes A2. Now both groups are marked, and each
waiter is of a group the other's mark is on. Under the earlier rule a called-in group put forward only
the caller that wanted that same group, so neither waiter was ever chosen and neither mark came off.
A join, `process_vm_writev` or a tracer in the same shape hung the same way.

The rule now:
- **A mark keeps a group's threads out of guest code, and nothing else.** A waiter is put forward
  whatever is called in, its own group included (4.3). Being chosen runs its call again, inside the
  kernel.
- **When its call has run, a waiter goes back to guest code only if its own group is not called in**
  by another call. Otherwise it stays ready, with its call answered, and is handed out when the last
  mark on its group comes off. That is a choice that hands nobody out, which has its place in the order
  of entries like any other (4.3).
- **A waiter of a `Serial` group is still chosen only when none of its group's threads runs** (4.3). So
  a woken call never touches its group's memory under a running sibling, and when its work is done no
  thread of its own group or of a group it named is in guest code. Every process lock it needs is
  free, and 7.2's rule stands as it was.
- **A sibling in a waiter's way is asked in at once**, with `thread_stop`, whether or not it has a
  slice (section 5). A whole group stands still for the waiter, so it does not wait for a slice to end.
- **A mark belongs to its call.** It comes off when the call has run again. It also comes off when the
  call is given up: its thread is ended by a fatal signal while it waits. No other signal interrupts a
  wait for a call-in: the wait is bounded, and the call has had no effect yet.

Why no circle is left. What a waiter needs before its call runs again is three things, and none of
them is a mark coming off.
1. The running threads of the groups it named have come in. Each was asked, and no request to stop is
   lost (above). A marked group is handed nobody new.
2. In a `Serial` group, its own group's running thread has come in. It was asked at once.
3. A processor. Section 5's condition gives one: somebody in the waiter's way is due in.
So every mark comes off after a bounded time, whatever other marks are on. In the case above A1 is
chosen first. Its `write` finds nobody of P2 running, is made, and A1's mark comes off P2's group. A1
stays ready, because P1's group is still marked. A2 is chosen, its `write` is made with nobody of P1
running, its mark comes off, and it runs. P1's group then puts forward its next thread, A1 or S1.

A model of these rules was searched for this correction (section 14): two `Serial` groups of two
threads and one `Parallel` group of two, on one to four processors, with calls that reach another
group, join two groups, or call the caller's own group in. In up to 62,189 states it found no state
where nothing can move, no state from which the end cannot be reached, no thread in guest code beside
a sibling of its `Serial` group, no call at work under a running thread of a group it named, and no
call-in left pending when only the steps the kernel forces are taken. With the earlier rule the same
search finds the sixth check's case.

The same rule covers a group that calls itself in (a group in force `Parallel` and `Exact`) and two
groups being joined, where the caller's group is one of them (section 3): the caller is a waiter of a
group that is called in, and is chosen.

While a mark is on, each of the group's threads that came in found nothing to run on its processor,
and those processors may have gone idle. Taking a mark off makes no thread ready: the threads were
ready all along. So an idle processor is woken by a condition on the kernel's state, not by the event
"a thread became ready" (section 5): the entry that takes a group's last mark off ends by waking a
processor for each thread it let run again.

**Group stop** (the executor's), done inside the hook or the `Cpu` call that needs it. It makes every
other thread of one memory held while one change is made. It is per memory and not per group: every
change it guards is to one memory's own page table, code or bytes, and a change that reaches several
memories is made memory by memory. The native executor interrupts the virtual CPUs (section 10). On the
WebAssembly executor three rules make it safe:
1. **One stop lock per memory.** A group stop takes it exclusively before it sets any thread's held bit,
   and takes the bits off before it lets the lock go. So two group stops of one memory never run
   together.
2. **Whoever touches the memory outside guest code has the lock.** A hook, or a `Cpu` call made for the
   kernel, that reads or writes the memory's bytes, its page table, its slots or its code lists has the
   stop lock for as long as it does: shared for an ordinary access (`Cpu::read`, `Cpu::write`, a fault
   settled, code installed), exclusively for a group stop. So no hook's access and no access of the
   kernel's overlaps a group stop.
3. **Held is exact, and only at a stop point.** Each thread has a mark that says it may be in guest
   code, or in a piece that still relies on what it found out. The thread sets the mark before it enters
   guest code and then reads its stop word; if the held bit is set, it takes the mark off again and
   waits for the stop lock. **It takes the mark off only at a stop point**: in the stop hook, in the
   system call hook, in a hook that is made a stop point (below), and when it leaves `Execution::run`.
   In every other hook the mark stays on. A group stop sets the held bits and then waits until every
   mark is off. Each side writes before it reads, with sequentially consistent atomics, so at least one
   of them sees the other.

**Why only at a stop point.** A piece checks a page's permission once and relies on that until its
thread's next stop point (tr/contract.rs:17-20). A loop with no call and no system call keeps what it
knows about pages for all its rounds (tr/full.rs:1130-1146). Some hooks return into the middle of a
piece, which then carries on with what it knew: the clock hook (ARM64's counter read,
arm64/meaning.rs:1458, wt/lib.rs:2124-2131), the float hook, `warn` and the code-changed hook. Their
effects set neither of the two flags (guest-cpu/effects.rs:137-142, :159). If a thread counted as held
inside one of them, another thread's `munmap` could finish while it was there, and the thread would then
read or write the page from its loop with no fault. Linux forbids that. A per-thread watch would miss
accesses the same way. (The plan's earlier form took the mark off in every hook, and had this hole.)

So hooks are of two kinds, and the translator's contract says which each is.
- **Hooks that are stop points.**
  - The stop hook and the system call hook are by the contract, and so are the hooks after which the
    thread leaves, `memory_fault` and `trap` (tr/contract.rs:94-114).
  - The load code hook is one by where it is called: only from the dispatcher and the bottom loop
    (tr/code.rs:1150-1180, tr/thread.rs:110-140). A piece reaches those by a call or a jump out of
    itself. By its own contract it relies on nothing it knew about pages after either, because the code
    it lands in may reach a stop point (a piece may check the stop word at its entry, tr/full.rs:1639,
    and a loop with a call keeps nothing, tr/full.rs:1143-1146). The translator track confirms this for
    checks that cover several accesses when it builds the mark below.
  - Three more are **made** stop points, because they must wait for the stop lock or must touch the
    memory: the code-changed hook, the 128 bit atomic hook and x86's hook for a misaligned locked access
    (7.3). For each, the translator does what it does at a stop check whose hook ran. Before the call
    it saves every register, with pc at the instruction. After it, it forgets every kept run of pages
    and proves its loop's ranges again (`AfterStop::Reload` and `Reprove`, tr/full.rs:2162-2177). No
    check may cover accesses on both sides of it. This changes translated code on these three paths
    only, none of which a `Serial` run of a program without such instructions reaches. Asked of the
    translator track: one mark in the guest's table of hooks (suggested `HookType::stop_point`).
- **Hooks that are not**: the clock hook, the float hook and `warn`. The mark stays on. Such a hook may
  not wait for the stop lock or the kernel's lock, and may not touch the memory, its page table, its
  slots or its code lists. The executor's own code in them does none of that today: the clock hook
  asks the system (rt/lib.rs:120-122), the float hook works on the thread's own state, `warn` counts.
  A group stop that finds a thread there waits until the thread reaches its next stop check. A stop
  flag was noticed in 230 ns on average and 30 µs at worst in Chrome (docs/phase-1-translator.md, "The
  stop check").
  - Two of them call the embedder's system: the clock hook (`ran`, and the clock's answer) and `warn`
    (`cpu_warning`). The rule binds that code too, and the executor cannot see into it. So the three
    methods' doc comments carry the rule, and a mark of the worker's makes any call into the kernel
    from inside them a panic that names the call, where it would otherwise hang a group stop (4.2).

A held thread is therefore at a stop point, with every register saved (tr/contract.rs:13-15), and has no
access of its own under way. It may be waiting for the stop lock, waiting for the kernel's lock,
compiling, or out of `Execution::run`. Whatever it does next to the memory begins by taking the stop
lock, which waits for the group stop to end. And whatever its piece does next to the memory, it checks
first.

**Lock order**: the kernel's lock, then a stop lock, then a process's lock, then a block's (7.2). A
thread never asks for the kernel's lock while it has a stop lock. So a group stop made inside a kernel
entry cannot wait in a circle with a thread on its way into the kernel: that thread is held.
The order has one exception, and it is stated here and not hidden. A thread of a `Serial` group has its
process's lock for its whole stay in `Execution::run`, and asks for the kernel's lock under it at every
system call (7.2). That is safe only because nobody who has the kernel's lock ever waits for a `Serial`
process's lock. 7.2 makes that a rule the code checks.

**Which primitive, for which group.** For a change that needs the other threads of memory M stopped:
- **nobody**, when M's group is in force `Serial` and the caller is in it. No other thread of it runs;
- **a group stop**, when M's group is in force `Parallel` and its guarantee in force is not `Exact`.
  Where each thread was held is not recorded, and such a group promises nothing about that;
- **a call-in** otherwise: M's group is `Serial` and the caller is outside it, or M's group is in force
  `Parallel` and `Exact`. Every thread then stops at an entry of its own (4.4 item 5), and the caller
  parks until they are in.

Below, "M's other threads are stopped" means by this rule. One change is never "nobody", although its
caller may be in a `Serial` group: a mapping that joins the caller's group to another (below, and
section 3).

A group in force `Parallel` and `Exact` keeps five uses of holding a thread in place. Four only order
memory accesses, and that order is the embedder's part there (section 2): the 128 bit atomic (7.3), a
write of the kernel's from outside into a memory that has one thread (7.3), `membarrier`, and the
embedder setting a watch (section 8). The fifth changes nothing a guest can see: growing the
executor's lookup tables (7.5). Where the embedder needs a held thread's position, it can read it: the
thread is at a stop point, with its registers and its count saved.

A memory in force `Serial` keeps no marks, and accesses to it take no stop lock: within the group only
the caller runs, and from outside the kernel has called the group in first. The kernel tells the
executor each memory's config in force (7.3), and changes it only while none of that memory's other
threads is in guest code. The executor counts "group stop reached a running thread of a Serial group"
and "group stop for a change of the kernel's in an Exact group", which must both stay at zero.

What is stopped before what, where M is the memory changed:
- **A page loses a permission, is unmapped or remapped; code is dropped** (`set_permissions`,
  `shared_unmap`, a mapping going away, `code_changed`, ARM64's `ic ivau`, a write to translated x86
  code). This is the translated code's rule (tr/contract.rs:17-27). M's other threads are stopped.
  - After a code drop each of M's other threads also gives up its frames, so that none comes back
    into a piece made from the old bytes (docs/runtime.md section 6), counted "left run: code changed
    under it". A translation of those bytes that is under way is thrown away (7.5). Under a call-in they
    have left already.
  - **After a page that allowed reading and writing has lost a permission, been unmapped or been
    remapped, each of M's other threads gives up its frames too.** The translator's contract has one
    exception to "checked again at the next stop": what a piece proved about its own stack frame is
    checked again only at the next function entry (tr/contract.rs:20-23), and it is kept across a
    call whenever the callee gives the stack pointer back unchanged (tr/full.rs:2835-2853). So a
    thread that was held in place and then let go would carry on reading and writing a page of its
    frame that another thread had just protected or unmapped, with no fault, even into a new mapping
    put at the same address. Linux faults that access. Today this cannot happen across threads: a
    thread whose turn ends leaves `Execution::run`, and its frames go with it. A piece entered from
    the bottom loop knows nothing about its frame (`Seed::NOTHING`, tr/full.rs:851-864) and checks
    every access. Counted "left run: a page it could have a frame on changed".
    - A frame is only ever proved over pages that allow reading and writing (`Frame`,
      tr/full.rs:730-731). So a loss on any other page (code, read-only data, a reservation with no
      access) makes nobody give up anything.
    - It is needed only after a group stop. Under a call-in the threads have left already, and a
      `Serial` group's other threads are not in `Execution::run`.
    - It costs each other running thread 300.8 ns on top of its stop, and its calls in progress
      return through the bottom loop. That is paid at every `munmap` of data and every `mprotect`
      that takes writing away in a `Parallel` group. How often is T2's and T3's measurement.
    - Rejected: only the threads whose stack pointer lies at or below the page, in the mapping that
      holds it. That is the x86 plan's test for code on a live stack (x86 plan lines 1898-1905), and
      this plan's earlier form used it for watches. It misses a thread that has moved to another
      stack while calls on the first are still in progress. Go's `systemstack` has that shape: it
      moves the stack pointer to the scheduler's stack, calls a function and moves back (Go's
      runtime, asm_arm64.s). The frames on the first stack are relied on again when the callee
      returns. A narrower test that is still sound needs the translator to bound where a thread's
      live frames are. It is asked of the translator track only if the count above is measured to
      matter.
  - The change reaches M from another group when a file changes size or bytes: the kernel then lowers
    permissions and drops code in every process that maps the file (`changed`,
    kernel/shared.rs:581-600). Counted "another group called in for its pages" when it is a call-in.
  - In a group in force `Exact`, the hook that announces a code change (`ic ivau`) cannot make the drop
    in place: it hands the kernel an exit for it, and the kernel calls the group in. Counted "group
    called in for a code change".
- **A page gains a permission, or a new mapping appears**: nobody, where the rule above says nobody or a
  group stop. A thread that still acts on the old table faults to the executor, which looks at the
  table again and carries the thread on. Counted "fault settled by the table as it is now". Where the
  rule says a call-in, the group is called in, as for a loss: whether an access faults would otherwise
  depend on the host's timing. A file that grows under another process's mapping is such a case. A new
  mapping that joins groups is the next case, not this one.
- **Groups are joined, or the config in force becomes stricter** (section 3): the running threads the
  change binds are called in, and the call that caused it parks before it has any effect. For two
  `Serial` groups that is the other group's one running thread. Counted "group called in to be joined"
  and "group called in for a stricter config". The hook of section 3 fires in the call when it runs
  again, before the mapping is made.
- **The kernel reads or writes M's bytes from outside M's group** (a `write` to a file reaching pages M
  shows, `process_vm_readv` and `process_vm_writev`, a tracer, `/proc/<pid>/mem`, `environ`, `cmdline`).
  - M's group is `Serial`: it is called in. The read or write then has one place in the order. Counted
    "another group called in for its memory". `cmdline` is not inspection (section 2), but it is still
    such a read.
  - M's group is `Parallel`: nobody is called in. The bytes land while its threads run, as on Linux.
    The executor's copy into M is made with pointer writes, since "no guest runs" (wt/lib.rs:1423) no
    longer holds, and with M's stop lock shared (rule 2). With `Exact` this is a memory access like a
    thread's, and the embedder orders it through its watches (section 8).
    - One case needs more. A write into a memory that has one thread is made under a group stop of
      that memory, so that it cannot land inside a 128 bit atomic made inline (7.3).
- **The kernel changes a guest word that guest code changes too**: `FUTEX_WAKE_OP`
  (kernel/syscall/futex.rs:682-711), the owner word of a priority-inheritance lock (:760-786), the
  owner-died mark of a robust lock (:1020, :1163). Today each reads the word, works out the new value and
  writes it. In a group in force `Parallel` another thread can change the word in between, and its
  change is lost. Each becomes one compare and exchange, asked of the executor (suggested
  `Cpu::compare_exchange(memory, address, expected, new)`), and tried again when the word had changed,
  as Linux does (`futex_atomic_op_inuser`). Nobody is stopped. It is the one path for both orderings: in
  a `Serial` group it succeeds the first time.
- **Another thread's registers are read** (a tracer, a crash dump): that thread is called in, whatever
  its group's config.
- **exec**: the process's other threads that are not running are ended as today (kernel/task.rs:648).
  Its running ones, which exist only in a `Parallel` group, are called in, and each ends in its own
  entry. The caller parks until they have, as Linux's exec waits for the other threads to die.
- **`exit_group`, a fatal signal**: the process is marked as ending, and nobody waits.
  - When no thread of its group is running other than the caller, it ends at once, as today
    (kernel/task.rs:685, kernel/signal.rs:481-485). With one processor that is always so.
  - A `Serial` group whose thread is running (the signal came from outside the group): the thread is
    asked in with `thread_stop`, and the whole ending is done in the entry that brings it in. Ending a
    thread writes into the group's memory (the word cleared at a thread's exit), so it must not be done
    under a running thread.
  - A `Parallel` group: the threads that are not running are ended at once. Each running one other than
    the caller is asked in and ends in its own entry. The process ends, with its memory dropped and its
    parent told, at its last thread's end, as on Linux.
- **fork from a threaded process**: M's other threads are stopped for the length of the copy in
  `memory_duplicate`, so that no page is copied half-written. Counted "group stopped for a fork" or
  "group called in for a fork".
- **A signal for a running thread**: that thread only, by `thread_stop`, and nobody waits. It takes the
  signal on its way back to the guest, as every thread does (kernel/task.rs:326-339).
- **A signal for a process is never left with a thread that will not take it.** For a signal sent to a
  whole process the kernel picks one thread that does not block it (`complete`,
  kernel/signal.rs:459-492) and asks only that one in. Today the picked thread is not running, or is
  the sender itself, so nothing can change its mask before it takes the signal. With several
  processors the picked thread may be running on another one, and the entry it was about to make may
  be `pthread_sigmask` blocking that signal, or its own exit. The signal then stays pending on the
  process, and nobody else was told. A sibling that only computes, or one parked in a call that the
  signal should have interrupted, never takes it. So the kernel picks again in the two places Linux
  does (`retarget_shared_pending`, called from `__set_task_blocked` and from `exit_signals`; Linux
  6.12 kernel/signal.c:2974-2998, :3076, :3033):
  - when a thread's mask gains a signal that is pending on its process;
  - when a thread ends while a signal it did not block is pending on its process.
  For each such signal it takes another thread of the process that does not block it, and treats it as
  the first pick was treated: asked in if it runs, woken if it waits where a signal may wake it, left
  to take the signal at its hand-out otherwise. A thread's mask is written in nine places today
  (kernel/signal.rs:640, :892, :1000, :1035, kernel/syscall/epoll.rs:550, io.rs:3084, :3143,
  process.rs:942, signals.rs:344). They become one function, which is where the first case is caught.
  It tests one value when nothing is pending on the process. This holds in a `Serial` group as in a
  `Parallel` one: there the sibling left out is the parked one.
- **A stop signal, `SIGCONT`, a tracer**: the process's running threads, by `thread_stop` for each. Each
  parks itself when it comes in.
- **A futex wait or wake**: nobody. Both are kernel entries, so the wait's "compare, then park" and the
  wake are already in one order.
- **A thread's mask leaves out the processor it runs on** (`sched_setaffinity`): that thread only, by
  `thread_stop`. The caller parks until it has come in (4.3).
- **`membarrier`**: a group stop of the caller's memory when it is in force `Parallel`, and for the
  commands that mean every process, of every such memory. Each held thread has passed a full barrier:
  it took its mark off with a sequentially consistent store. A `Serial` group needs nothing: one of its
  threads runs at a time, and the hand-over between them goes through the kernel's lock.
  - One command needs more. `MEMBARRIER_CMD_PRIVATE_EXPEDITED_RSEQ` must restart the restartable
    sequence of every running thread of the process, or with `MEMBARRIER_CMD_FLAG_CPU` of the thread on
    one processor. A sequence is restarted only in a kernel entry of its own thread (`rseq_preempted`,
    kernel/signal.rs:648-665, called today only for a thread that gives way, kernel/task.rs:133-137),
    and a group stop holds threads in place with no entry. So for this command each such thread is asked
    in with `thread_stop` and restarts its sequence in the entry that brings it in, and the caller parks
    until all have. Linux's call also returns only after every such CPU has been interrupted. In a
    `Serial` group the other threads are not running, and each was taken off through an entry.
- **The container's first process ends**: every other process is ended as by a fatal signal, as Linux
  ends a container. Each worker's `next` then answers `Finished`.

### 4.6 The order of kernel entries, recorded and replayed

Whether anything is recorded follows from the kernel: it is recorded or it is not (section 3). What is
recorded about a process in a recorded kernel follows from its guarantee in force and from nothing
else.

**One recording point.** Before a kernel entry takes effect, the kernel asks the system whether it may,
with the lock held (suggested: a `Host` service `kernel_entry_admit(entry) -> EntryAdmission`, with
`Now` and `Later`). The entry says what it is: a thread chosen for a processor (the processor and the
thread), a thread come back (the thread and why), an event, a signal from outside. For a choice it is
asked when the thread is chosen, before the thread's woken call runs again or its signal's frame is
written, so that what those do is the effect of an entry that has its place (4.3).
- A system says once, when the kernel starts, whether it orders entries (suggested: a field of
  `Start`, `kernel_entry_admission`, with `Unasked` and `Asked`).
- A system that neither records nor replays says `Unasked` and is not asked at all. Each entry then
  tests that one value. The shipped system is such a system, and so pays no indirect call for this on
  any system call (4.2). This is the kernel in which "with `None` nothing is recorded".
- A system that said `Asked` is asked about every entry of every thread, and about every event and
  every signal from outside. No thread is left out, because every thread's entries move the state
  that every other thread's replay stands on (section 3). In such a kernel no process is in force
  `None`.
- A system that records answers `Now` and writes the entry down. The order of its log is the order of
  effects, because it was asked under the lock.
- A system that replays answers `Now` when the entry is the next one in its log, and `Later` otherwise.
- **What the kernel takes from the system inside an entry is part of that entry's record**: the clock
  it charges the thread by (`account`, kernel/task.rs:77-86, reads `Host::monotonic_nanoseconds`), an
  id from the id source (interface notes lines 16-19), random bytes. They are the host's answers,
  which a recording logs today (docs/runtime.md section 12, "Repeatable runs"). So in a replay each
  thread is charged what it was charged in the recording, whatever its guarantee and however long its
  turn took this time, and the scheduler makes the same choices.

**What an entry carries, by guarantee.**
- `Exact`: the thread's position. The system already hears each thread's progress (`System::ran`), and
  the thread's code has the count, so it has (count, pc).
- `External`: no position, because the code has no count. The entry's place is its place in the order,
  and among its own thread's entries. When the entry is over, the kernel hands the system a digest of
  everything it took from the thread for it: what the exit says (a system call's number and arguments,
  a fault's address and access, nothing for a stop), what a woken call read again when the entry is a
  choice, and each byte it read from the thread's memory
  (suggested: a hook `on_kernel_entry_ended(thread, digest)`, made only for a thread in force
  `External` and only when the system orders entries; BLAKE3, which the cache key already uses,
  tr/contract.rs:163-166). The registers at a stop are not in it, since a replay stops elsewhere. The
  kernel is a pure function of what it is handed (docs/runtime.md section 12), so the same digests in
  the same order mean the same effects on everything outside the process. This is the one recording
  point of the brief: the kernel keeps no list of which calls are "outside".

**What is exact.** A process in force `Exact` replays with the same registers and memory at every logged
position when two things hold.
- Nothing reaches its memory except through an entry in the log, or in an order the log fixes:
  - in a `Serial` group this is so by construction. Memories that can see each other's stores with no
    kernel entry between are one group (section 3). The WebAssembly executor's copies of written shared
    pages at a turn's end (`flush`, wt/lib.rs:1341) reach only memories that show the same block's
    pages, which are in the writer's group, and none of them is running. Everything else that crosses
    groups is done by the kernel inside an entry, with the other group called in first (4.5);
  - in a `Parallel` group every stop the kernel or the executor causes is an entry at a position (4.5),
    and the order of the threads' memory accesses is recorded by the embedder through per-thread
    watches (section 8). Without that record the kernel's part is exact and the memory is not.
- Every entry of the kernel is in the log, in order, and every process in it is at least `External`.
  In a recorded kernel that is so by section 3's rule. A neighbour in force `External` can still end
  the replay: when one of its entries differs, the replay is thrown away for everyone (below). It
  cannot make an `Exact` process differ with nobody told.

So a run is fixed by what is recorded today (the host's answers, each event and each turn's end with its
position, docs/runtime.md section 12) and one new thing: the order in which kernel entries took effect.
A clock read without a system call (`Answer::clock`, rt/lib.rs:120-122) is a host answer of its thread.
It changes nothing in the kernel and needs no place in the order.

**An entry told `Later` never holds a host thread.**
- A thread come back too early: the kernel keeps the thread and its exit aside and returns. The thread
  leaves `Execution::run`, and its worker is free. After each entry that takes effect, the kernel offers
  the kept ones again.
- A thread chosen too early: nothing is done for it, and `next` answers `Idle`. It does not mark the
  processor idle (section 5): whoever was told `Later` comes back by its log and is owed no wake. A
  choice that the same call of `next` made before it, and that took effect, stands.
- An event too early: the replaying system delivers its events itself, from its log, in their place.
  The kernel's timer is such an event (section 5).

**How a replay proceeds.** It is not `run_until_finished`. It is the same `run`, called in the log's
order: the replayer takes the log's next "thread chosen" entry and calls `run` for its processor,
from any free worker. With one worker it walks the log. With more, several threads run their turns at
once and only their entries are put in order. It cannot get stuck: the entry that is due is a choice,
which a free worker makes; or the return of a thread some worker is running, which will arrive; or an
event, which the system delivers. A choice that handed nobody out in the recording hands nobody out in
the replay, because the kernel is a pure function of what it is handed. `next` then chooses again, and
is told `Now` or `Later` for that choice like any other. A return that is kept aside is never the due
one, since it is offered again the moment it becomes due. A worker whose thread came back early is free
again at once, so even one worker always has something to do.

"Handed the same" is the condition, and it holds because every entry of every thread is in the log
(above) and every answer the kernel took from the system is too. The plan's earlier form left the
entries of a thread in force `None` out, and its replay could wait for ever (section 3).

**A replay that has left its recording ends. It never waits.** With every process `Exact` a replay
cannot leave its recording. A process in force `External` can make it: its inside may differ, and so
may what it does next. The replaying system sees that in one of four ways, and each ends the replay:
- an entry's digest differs from the log's (below);
- a thread comes back with another kind of exit than the log has next for that thread;
- **the due entry is a choice for a processor, and `next` for that processor chooses another thread.**
  A choice depends only on the kernel's state and the processor. So this is not an entry that came
  too early: the state is no longer the recording's. `Later` would bring the same choice back for
  ever. The system is asked about a choice for the due processor that is not the due thread, and that
  is how it knows;
- the due entry is the return of a thread, and the thread has ended or is parked.
In each case the system marks the replay thrown away and asks for a container stop (section 5).
Counted "replay thrown away", by which of the four. One thing is left that no rule of the kernel's
can end: a process in force `External` whose inside differs may compute on and never make its due
entry. The embedder bounds that with a limit of its own. It is what "not guaranteed to succeed" means.

The log need not exist beforehand. A system that makes its order up as the run goes answers the same
way: `Now` for the entry it wants next and `Later` for any other. The tests' repeatable order with one
worker is that (section 11). So there is one `run` and one way to order entries, for a recording, a
replay and a seeded test.

**Stops in a replay.**
- `Exact`: a thread must stop where it stopped in the recording, not where a flag happens to reach it.
  The replaying system gives each turn a counted limit that ends it at the thread's next recorded
  position (`Turn::Counted`, rt/lib.rs:25-27), and its `thread_stop` does nothing. This is why
  `thread_stop` is asked of the system (4.5), and why `System::turn` is told the thread (section 5).
  Only code with the count can be given a counted turn, so `turn` is also told the thread's guarantee in
  force, and a counted turn asked for any other thread is the system's error.
- `External`: a stop the log holds for a thread (its slice ended, a signal reached it, its group was
  called in) has no position to stop it at. The replaying system asks the thread in as soon as its entry
  before the stop has taken effect, and the thread comes in at its next stop check. So it has done less
  before the stop than it did in the recording, and the same in the end.
  - A process with one thread that takes no signal while it computes replays every time: nothing inside
    it can tell where it was stopped. Most processes of a build are such.
  - Anything else may differ inside: threads of a `Serial` group take turns at other places, and a
    signal's handler runs at another place (its return then hands the kernel a frame with other
    registers in it). When that changes what the process hands the kernel, the digest of that entry
    differs from the log's, the replaying system sees it, and the replay is thrown away, as the ruling
    says. Counted "replay thrown away: an entry differed". The digest covers more
    than what leaves the process (a private futex word, an argument of `mprotect`), so a replay can be
    thrown away for a difference nobody outside could see. After the split (4.7) only entries that
    reach beyond the group are digested.
- A group stop never stops a thread of a group in force `Exact` for a change of the kernel's (4.5), so
  there is nothing of it to repeat.

**In a `Parallel` group in force `External`**, the ruling also puts each locking and atomic instruction
in one order. Its design is the prototype's (optimistic-replay-results.json): a third form of code in
which each such instruction takes the next number of one sequence for the group, logs it for its thread,
and in a replay waits for its number. It is a form of its own in the cache key (7.2). Measured there: it
would not replay any of the 11 programs measured on ARM64, because C libraries and language runtimes
read lock words with plain loads; and the one sequence costs 24 to 97 ns an event, which by calculation
adds 0.7 to 4 times the run time for Go and Java. Whether it is built is decision 3. Until it is, a
group asked for (`Parallel`, `External`) runs (`Serial`, `External`) by the rule of section 3, which
keeps the same promise.

**After the split** (4.7) the point is the taking of any lock that is not the entering group's own, asked
with that lock held. An entry that stays inside its group (`brk`, a private `mmap`, a private futex,
`sigaction`) is then not asked about and waits for nobody, in a recording and in a replay. This is the
brief's rule: what another process or the system can observe goes through the one point, what touches
only the process's own state does not, and the kernel keeps no list of system calls.
- One thing must change with it, or the sixth check's fault comes back in another place. Today every
  return charges its thread and moves the run queue's least weighted time (`account`,
  kernel/task.rs:77-86), so every entry touches the scheduler that all share. An entry that is to stay
  out of the order must not. Its charge is kept in the thread's own record and reaches the scheduler
  at the thread's next entry that takes the scheduler's lock, which is asked about. The clock it read
  is a host answer of its thread, as a clock read without a system call is. T7's bar for the group's
  split has the replay tests of section 12, the recorded kernel's among them.

**What this needs of other tracks.** `Exact` at full speed needs the translator's progress count
(docs/runtime.md section 9, not built), in the form of code a process in force `Exact` runs. Until then
a process in force `Exact` runs counted turns in which stop checks stand in for the count
(wt/lib.rs:1792-1818), which costs a hook call a check and repeats only at one look-ahead, as the kernel
tests' `repeats` does today. That is for tests. `External` and `None` need nothing of it.

Rejected: the kernel hands out sequence numbers and makes an early entry wait for its number. A thread
that waits holds its host thread, and with fewer host threads than the recording had, the entry it waits
for has nobody to make it. Rejected: a recording hook that returns nothing. It cannot gate a replay.
Rejected: comparing only a system call's arguments for `External`. The bytes of a `write` would then go
unchecked, and a process in force `Exact` that read them would differ with nobody told.

### 4.7 What stays one at a time, and how it is split later

At first the whole kernel is one at a time. Guest code runs in parallel; kernel entries do not. How much
that costs a build is T1's first measurement (section 13): the time processors spend waiting for the lock.

The split is made only where that measurement points, in this order of expected worth:
1. **The group's own state** under the group's own lock: its address map, signal handlers, private
   futexes, its threads' records. Calls that touch nothing else then run in parallel between groups, and
   stop being recorded and ordered (4.6).
2. **The scheduler** (run queue, processors, parked threads) under its own lock.
3. **Files**: one lock for all files first, then per file system object, as Linux has.
4. The process table, timers, sockets, System V objects: each its own lock, when measured to matter.

Each split turns the `Rc` and `RefCell` of that part into `Arc` and a lock, and is measured against the
single-thread system call cost before it lands. Nothing outside the kernel changes: entries already take
`&self`, hooks already get one process and not the kernel.

## 5. The loop and the system

**Today.** `Stepper::step` runs one thread and returns; `drive` repeats it (rt/lib.rs:197-243). The owner
has named the first `run` and the second `run_until_finished` (section 2), and this plan uses his names.
The executor is `&mut`, and one `StopFlag` raises "whichever thread is running" (rt/lib.rs:33-57). The local
system has one timer thread with one deadline (rt/system.rs:41-55).

**The change: the same `run`, called by one host thread for each processor, each naming its
processor.** `run` and `run_until_finished` take the kernel, the executor and the system by `&`, and a
processor. With one processor, one host thread calling `run_until_finished` for processor 0 is today's
behaviour exactly.
- **A live run has one worker for each processor.** `Container::run_until_finished` starts them
  itself, one for each processor of the container, whatever the host has. There is no setting for
  fewer.
  - The plan's earlier form had one (`RuntimeConfig::host_threads`, default the host's
    `std::thread::available_parallelism`). Its sixth check showed where the default led. The count of
    processors is the container's and the same on every machine (section 11), and the count of
    workers was the host's. A container made on a host with 16 cores and brought back on one with 8
    got 8 workers for 16 processors, and a thread pinned to processors 8 to 15 never ran.
  - With more processors than the host has cores, the host shares its cores among the workers, as it
    does among the virtual CPUs of a virtual machine larger than its host. That is correct. What it
    costs is not measured (section 15).
  - An embedder that calls `run` itself must have a caller for every processor. Its limit on how many
    guest threads run at once is the count of processors it gives the container.
  - Fewer workers than processors is not designed (known fault 2). A replay and the tests' seeded
    order do not need it: there the system says which processor is served next, and any worker
    serves it (4.6, section 11).
- Two callers naming one processor at once is a caller's error, which the kernel answers instead of
  today's assertion (kernel/lib.rs:306-309).

`run(processor)` keeps handing that processor threads until it has a reason to return, and the reason is
the owner's `StopReason`, read per worker:
- `Idle { until }`: this processor has nothing to run, or, under a system that orders entries,
  nothing it may run yet (4.6). `run_until_finished` waits in `System::idle` and calls `run` again;
- `Finished { status }`: the container's first process has ended. Every worker gets it;
- `Suspended`: a process was suspended in a hook that ran on this worker (section 6);
- `Interrupted`: this worker was asked out (a container stop, below).

Rejected: a pool of worker threads hidden in the runtime. An embedder with its own event loop (the owner's
other project drives many kernels) must decide which of its threads work for which container. Rejected: a
second loop for the parallel case. One `run`, entered N times.

What changes with it:
- `Execution::run(&self, ...)` (rt/lib.rs:128-143): several host threads are inside it at once, each
  with its own thread. The executor does its own locking (section 7).
- **The system is told who.** `turn` is told the processor, the thread and the thread's guarantee in
  force, `ran` the thread, `arrived` and `idle` the processor (today none is, rt/lib.rs:150-164). A
  replaying system needs the thread to find the turn's recorded end, and the guarantee to know whether
  the turn can be counted (4.6). The local system needs the processor for its slice's end.
- **Idle, without a lost wake.** `next(processor)` that answers `Idle` marks the processor idle, under
  the lock. The rule is a condition on the kernel's state, checked whenever the kernel's lock is let
  go: **no processor is marked idle that `next` would hand a thread to.**
  - The plan's earlier form woke a processor when an entry made a thread ready. Its fourth check
    showed three ways a thread that is ready already comes to be runnable on an idle processor with
    no thread becoming ready: the mark of a call-in is taken off (4.5); the running thread of a
    `Serial` group leaves its processor, and a sibling pinned elsewhere may now run; a ready
    thread's mask gains a processor. A fourth is a config in force that becomes `Parallel`. A list
    of events will miss the next one. A condition cannot.
  - How it is checked. The kernel walks the idle processors from the lowest number and asks, for
    each, what `next` would hand it, counting each thread once and a `Serial` group's one running
    place once. Each processor that would get a thread has its mark taken off and is announced
    (suggested: a hook `on_processor_needed(processor)`). This is after the entry's own hand-out,
    so a thread the entering worker took for itself wakes nobody.
  - The system's `idle(processor, until)` returns at the time, at an event from outside, or at that
    announcement. The announcement must be kept if it comes before the worker has reached `idle`:
    `idle` then returns at once. This is the rule of `std::thread::park` and `unpark`, and the local
    system uses exactly those. The mark is made under the lock and the condition is checked before
    the lock is let go, so no wake is lost. A worker woken for nothing asks `next` again and is told
    `Idle` again.
- **No ready thread waits with nobody due in.** This is the other half of the same condition, checked
  at the same place: **every ready thread that could be chosen has a running thread in its way that is
  due in, unless an idle processor is being woken that it could run on.**
  - *Which ready threads.* Those `next` could choose if a processor came free: every ready thread of a
    group in force `Parallel`, and a `Serial` group's next thread (4.3), whether or not one of that
    group's threads runs now. A `Serial` group's other ready threads wait behind its next thread,
    inside the group. Of a group that is called in, only its waiters count: they are chosen whatever
    is called in (4.3). Its other threads wait for the marks, which come off after a bounded time
    because the waiters are served (4.5).
  - *In its way.* A running thread stands in the way of such a thread when the ready one could run if
    the running one left its processor.
    - The ready thread is its `Serial` group's next thread, and a thread of that group runs: that
      running sibling is in its way, and only it, whatever processor it is on and whatever the masks
      say. The ready thread cannot run while the sibling does.
    - Otherwise: every running thread on a processor that the ready thread's mask holds.
  - *Due in.* A running thread is due in when it was handed out with a fair slice, or the kernel has
    asked it in with `thread_stop` and it has not come in since. The kernel keeps both facts itself,
    for each processor's thread; it does not read the stop word. Either way the thread makes a kernel
    entry of its own after a bounded time (4.5: no request to stop is lost). That entry runs the
    scheduler's tick (kernel/task.rs:88-104): the thread gives way if it has had its share, and
    otherwise goes on, now with a slice.
  - *Why some thread is not due in.* A thread gets a fair slice at its hand-out only when the run
    queue is not empty then (kernel/task.rs:43-53). That test is kept as it is. So a thread handed out
    to an empty queue has no slice, and one that only computes never comes back by itself. Today two
    things always bring it back. A thread becomes ready in the running thread's own entry, whose
    hand-out then sets a slice. And a thread due to wake at a time has its deadline in the running
    thread's slice (kernel/task.rs:55-60). With several processors a thread also becomes ready
    through another processor's entry or through an event, and the deadline leaves the slices
    (below). So the kernel must ask.
  - *What the kernel does.* When such a ready thread has nobody due in in its way, the kernel asks in
    the one in its way with the most weighted time, with `thread_stop`. For a `Serial` group's next
    thread behind a running sibling, that is the sibling.
  - *A waiter is served at once.* For a waiter, a thread in its way that has only a slice does not
    count as due in: the kernel asks one in with `thread_stop` when the waiter is woken. A whole
    group stands still until the waiter's call has run (4.5), and a slice can be milliseconds.
    Counted "thread asked in for a waiter".
  - **The plan's earlier form left a thread waiting for ever behind its own sibling, with one
    processor too.** It asked somebody in only for "a thread ready that its group lets run", and a
    `Serial` group with a thread running lets none run. The fifth check's example: thread A of a
    `Serial` process computes alone, with no slice. Its sibling B sleeps, or waits in `read` on a
    pipe another group writes. `Event::Timer`, or the other group's entry, wakes B. B was counted
    nowhere, nobody was asked in, and A ran on. Go's background thread is exactly such a B (Go's
    runtime, proc.go, `sysmon`), so every Go program in a `Serial` group would have lost its
    preemption or hung. Now A stands in B's way and is not due in, so A is asked in.
- **What the two halves cost, and how they are checked.**
  - The kernel keeps one mark, "the scheduler's state was written", set by every write to what the
    condition reads: the run queue, a processor's thread and its idle mark, whether that thread is due
    in, a mask, and a group's members, its config in force and its call-in mark. Each is written in a
    few functions, and those set the mark. When the lock is let go, the kernel tests
    the mark. Clear: nothing more. A system call answered in place for a thread that carries on
    writes none of these, so it tests one value (4.2).
  - Set: the mark is cleared, and three counts say whether a breach is possible at all. They are the
    idle processors, the ready threads that could run now (a group that is called in has its waiters
    and no others; a `Serial` group with a thread running has none; any other `Serial` group has
    one), and the running threads that are not due in. The first half is walked only with an idle
    processor and a thread that could run now. The second only with a ready thread and a running thread
    that is not due in. On a busy container every thread was handed out with others waiting, so all are
    due in. With fewer threads than processors the queue is empty.
  - This is not the list of events the fourth check faulted. The mark is set where the data the
    condition reads is written, not by a guess at which changes matter. And a build for tests ignores
    the mark and the counts: it makes both walks at every lock release, and counts "a processor left
    idle with a thread it could run" and "a ready thread with nobody due in", which must both stay
    at zero.
  - The same build checks the rule the scheduler exists to keep: at every hand-out, no other thread of
    a `Serial` group is running. Counted "two threads of a Serial group running", which must stay at
    zero.
- **One timer for the container, in nobody's slice.** Today each turn's slice is cut at the kernel's
  next deadline (kernel/task.rs:55-60). With one thread running, ending its turn is the only way the
  kernel gets to look at its timers. Kept with several processors, every running thread's slice would
  carry the same deadline, and each expiry would stop all of them and make that many entries on the
  one lock, to wake one thread. A Go program's background thread alone sleeps for 20 µs to 10 ms at a
  time (Go's runtime, proc.go, `sysmon`). So:
  - a slice holds only what is the thread's own: its fair share and its CPU timers (`fair` and `cpu`,
    kernel/task.rs:43-54, :59). The kernel's deadline leaves it;
  - the kernel announces its earliest deadline whenever an entry has moved it (suggested: a hook
    `on_deadline_changed(deadline)`). It does so under the lock, so the system never holds a later
    deadline than the kernel has. An entry that sets no timer and parks nobody with a time makes no
    call;
  - the system hands the kernel `Event::Timer` at that time, from any host thread. That is an entry
    like any other. It wakes the threads that were due and fires the timers that were due
    (`wake_due` and `fire_due`, as kernel/lib.rs:354-355 does today), and by itself it stops nobody.
    A thread it wakes takes an idle processor by the condition above, or, when nobody in its way is
    due in, one thread in its way is asked in by the other half. That holds for a thread woken behind
    its own running sibling in a `Serial` group too. A signal it sends reaches its thread by 4.5. So
    an expiry with every processor busy stops one thread at most for each thread it wakes, where a
    deadline in every slice stops them all;
  - the local system sends the event from an idle worker when there is one, which wakes at the time
    as today, and from its timer thread otherwise (rt/system.rs:41-55; it exists already, for the
    slices). One atomic word holds the deadline and whoever takes it sends the event, so it is sent
    once;
  - every `Idle` answer still carries the deadline, as the owner's `StopReason::Idle { until }` has
    it. An embedder that drives `run` itself hands in `Event::Timer` by then, whether or not all its
    workers are idle;
  - a system whose clock is its own, as the tests' is (section 11), learns from the same hook how far
    a counted turn may go before the deadline, and hands the event in through `arrived`;
  - the native executor's inside arms its own timer for the deadline on one virtual CPU, in place of
    the timer thread (native.md section 5.5 has that timer for slices). Asked of the native track.
  Rejected: the deadline in one processor's slice only. The kernel must then choose a thread to
  carry it and move it each time that thread blocks, ends or changes processor, and that thread is
  still stopped for a timer that is not its own.
- **Stopping a thread.** `StopFlag` is replaced by a handle the executor gives out (suggested
  `Executor::thread_stopper()`, in place of `Executor::stop_flag`, rt/container.rs:35-36), with one
  method any host thread may call: `stop(thread)`. It sets the asked-in bit of that thread's stop word
  (4.5). Its promise is part of the interface, for every executor: once `stop` has returned, the thread
  makes a kernel entry of its own after a bounded amount of guest code, whatever it was doing when
  `stop` was called. `StopFlag`'s `raised` latch (rt/lib.rs:38-74) goes, because the bit is the latch.
  The system is handed the handle as it is handed the flag today (rt/lib.rs:164-166). Its `thread_stop`
  service and its timer thread call it. The timer thread keeps one slice's end and one thread per
  processor, as `turn` told it, and the container's one deadline, as the hook above told it.
- **A thread never leaves without its exit**, with two exceptions: the thread whose own entry ended it,
  and a thread whose process was suspended under it (section 6). Today a thread the kernel dropped
  breaks out of `Execution::run` with no `Kernel::exit` (wt/lib.rs:2303-2306). By 4.4 a thread ends only
  in its own entry, so the processor's running thread is always cleared by that thread's own `exit`, and
  `next` finds it so.
- **A container stop.** `System::stopped` runs when no thread is in `Execution::run`
  (rt/lib.rs:156-158), which a save needs. With several workers the loop first asks every running thread
  in, waits until every worker is out of `Execution::run` and no entry is in progress, then calls it
  once. This is also how an embedder interrupts a running container from another thread, which the
  interface notes leave open (line 92): each worker's `run` then returns `StopReason::Interrupted`.
  Every thread has stopped at an entry of its own, at a position.
- **Reading one process, and acting on it, from outside a hook.** The interface notes say
  `kernel.process(id)` is for "between steps" (line 55), and promise that "an embedder that wants to
  work asynchronously returns and finishes later through ordinary calls" (lines 60-61). With several
  workers there is no moment between steps unless the whole container is interrupted. Two things
  follow, and both are put to the owner (decision 8).
  - **Reading needs no stop.** `kernel.process(id)` answers a handle at any time, from any thread
    that can call the kernel. In a host program that is any host thread. On the native executor the
    kernel and the embedder's system are inside the virtual machine, and it is the system's code
    there, on any virtual CPU; a program on the host cannot borrow the kernel's data and asks through
    the link (section 10). The handle holds the kernel's lock while it lives, as a hook's handle
    does, so it is kept short.
    `id`, `info` and every accessor of `ProcessInfo` (memory mapped, distance to the ceiling, compute
    used, the config given and the config in force, which processes share its memory) read the
    kernel's own records. They touch no guest memory and no thread, and the lock keeps them whole.
    An embedder that decides where a process should run needs exactly these while the process runs
    (interface notes lines 11-14). The plan's earlier form answered an error outside a container
    stop, which closed that off for no reason.
    - Asked from the host thread that is inside a kernel entry, it answers an error: the notes rule
      that a hook acts only on the process it is handed (lines 76-77). That covers a hook,
      `on_processor_needed`, `on_deadline_changed`, a `Host` service and `arrived`. The kernel can
      tell because its lock knows its holder (4.2). A lock that did not would leave the call waiting
      for ever for a lock its own thread has.
    - Asked from inside `ran`, the clock's answer or `cpu_warning` when those are called from guest
      code, it panics and names the call (4.2). The worker has no lock there, so this is not the
      case above, and it would hang a group stop.
    - It is not a kernel entry. It changes nothing, so it has no place in the order of entries and a
      replay does not repeat it. What the embedder then does about what it read is an entry of its own.
    - Compute used is as of each thread's last entry. A running thread's time since its hand-out is
      added only if that accessor is asked for, as the notes say of anything that needs computing.
  - **Acting needs the group stopped, and one rule says when the handle can do it.** Every other
    operation touches the memory or the threads: `memory_read`, `memory_write`, `memory_watch`,
    `memory_changed`, `suspend`, a change of config. On any handle, from a hook or from
    `kernel.process(id)`, it is carried out with whatever stop 4.5 asks for, when that stop can be
    made in place: nobody, or a group stop. It answers an error where it would need a call-in,
    because a call-in parks its caller and a handle cannot wait: a group in force `Serial` with a
    thread running, and for `suspend` and a change of config a group in force `Parallel` and `Exact`
    with other threads running.
  - **For those cases the plan adds one call and one hook** (suggested `Kernel::process_call_in(id)`
    and `on_process_called_in(process)`). Without them the price of every late finish would be a
    stop of the whole container: in the owner's other project a thread left waiting by
    `on_memory_accessing` is let go when its page arrives from another machine.
    - Any thread that can call the kernel may make the call (above). It is a kernel entry, with its
      place in the order.
    - The kernel brings the process's group to a stop by the rule of 4.5 (nobody when none of its
      threads runs, a group stop, or a call-in), and then announces the hook, handing it the
      `Process` by value. Every operation on the process is legal there.
    - When none of the group's threads runs, which is the case of a process with one thread that
      waits for a page, the hook fires inside the call and nothing is stopped.
    - Otherwise it costs the group's own running threads their way in and back (300 ns each,
      docs/runtime.md section 10, and the stop check's latency), and no other group anything.
- `Host` and `System` methods take `&self`, so a system keeps what it changes in atomics or behind its
  own locks. The shipped system needs no lock on a system call (4.2).

**Two proposals of the embedding project, and what each would mean here.** The owner has ruled on
neither (the brief's last section). The plan builds on neither.

- **Hooks and services on `System` take `&self`, so that two hooks can run at once.** This plan asks for
  the same (decision 9), and needs it earlier than two hooks at once. From T1 on, several workers call
  `turn`, `ran`, `idle` and the clock's answer at the same moment, outside the kernel's lock. Hooks and
  `Host` services run under the kernel's one lock until it is split (4.7), so no two run at once in
  T1. After the split, two hooks about processes of different groups can, with nothing in the
  interface changed.
  - Does it need `System: Sync` from the start? Wherever more than one host thread is in the loop, yes,
    and the compiler asks for it whatever the trait says: Rust lets two threads share `&S` only when
    `S` is `Sync` (the standard library's rule for `Send` and `Sync`). The open question is only a
    system driven by one host thread. Recommended: the trait says `Sync` from T1 (through `Host`,
    4.2). An embedder's system is then written once, for any number of workers. Without it, a system
    for one worker may keep `Cell` and `RefCell`, and must be rewritten the day a second worker is
    added.
  - What that costs a system driven by one thread: it keeps what it changes in atomics, or in a lock
    nobody else ever holds (0 to 11.4 ns each time, section 14). The shipped system changes one
    word a system call and pays nothing (4.2).
- **Per-process state**: an associated type on `System`, kept by the kernel in each process's record,
  lent through the `Process` handle and mutable in a hook with no lock; cloned at fork, kept in
  `SuspendedProcess`, not in `ExportedProcess`, and free when it is `()`.
  - It fits this plan's locking as it is. A `Process` handle exists only while the kernel's lock is
    held: a hook's, and one from `kernel.process(id)` (above). After the split it exists while the
    process's group's lock is held (4.7). So whoever has the handle has the state alone. That holds
    in a `Parallel` group too: the process's other threads may be in guest code, but none is in a
    hook.
  - What locking it takes off an embedder: whatever it keeps about one process and touches only
    through that process's handle. Examples are where it decided to place the process, its record of
    the process's watches and page hand-overs (section 8), and the starting point it takes in
    `on_execution_config_in_force_changing` (section 3). With the state these need no lock, and no
    table from id to record.
  - What it does not take off: everything that is not about one process. That is the words `turn`,
    `ran` and `idle` keep per processor, the container's deadline, the one order of entries a
    recording system keeps across processes (4.6), its picture of other processes, which a hook
    cannot look up (interface notes lines 76-77), and its counters.
  - Five of this plan's calls are not handed a `Process`: `kernel_entry_admit`, `thread_stop`,
    `on_processor_needed`, `on_deadline_changed` and `on_kernel_entry_ended`. If the state is ruled
    in, the two that are about one thread's entry, `kernel_entry_admit` and `on_kernel_entry_ended`,
    could be handed it, and a recording system's log of one process would need no lock either. That
    is a change to make then.
  - What it would ask of this plan: one bound. The kernel's state moves between host threads under
    the lock (4.2), so the type must be `Send`. It need not be `Sync`: one host thread has it at a
    time.

## 6. Changing the config of a running process

The brief asked for two forms to be weighed, a setter and no setter. A later section of it has since
settled it: there is no setter. The weighing is kept, because it is what the rest of this section
stands on. One piece was open, a call that changes the config inside a hook. It has been measured
since, and it is settled at the end of this section.

**Form A, a setter** (not ruled in). `Process::set_thread_ordering(ordering)`, taking effect at the
group's next stop.

**Form B, no setter** (ruled). A process's config is fixed while it is in a kernel. It is given when the
container starts and when a process is resumed, and changed by `suspend` and `resume` on the same
kernel: `process.suspend()` gives a `SuspendedProcess<E>`, and `kernel.resume(suspended, config)` puts
it back with the new config. Nothing is converted and no memory is copied: the process's records and its
memory move out and back by ownership (interface notes lines 110-114). `export` and `import` are not
part of it. That meets the orchestrator's condition for agreeing (the brief, lines 91-94): the memory
moves and is not copied, and no ends are made for what the process has open, so there are none to
turn back into local objects.

One thing this plan asks of `suspend`, so that the value may be held, moved and dropped on any host
thread: it shares no `Rc` with what stays in the kernel. What the process shares with a process that
stays (an open file description after a fork, say) is kept inside the kernel for it under a key (4.2).
On the same kernel that is still no conversion: `resume` puts the same references back.

What each costs and makes simpler:
- **Work in the kernel.** A: the setter, on top of what section 3 needs anyway. B: nothing beyond
  `suspend` and `resume`, which the embedder interface builds for other reasons.
- **Work in the executors.** The same in both. A change of ordering or of guarantee can change the form
  of code the process runs (7.2), so its instances are made again; the native executor sets the
  process's page tables up again (native.md section 5.3). In B this is on the path of `resume`, which
  must do it for a process come from elsewhere too.
- **Cost of one change.** A: the threads called in, once. B: the stop that `suspend` needs (below), then
  the process's kernel records taken out of the kernel's tables and put back (its threads out of the
  run queue and the waits, its timers, its place in its parent's children). Their count is the
  process's threads and waits, not its memory. From inside a hook the embedder can suspend but must
  resume after `run` has returned (interface notes lines 134-135, 149-150).
  - What that return costs was measured since, by the interface track, on a prototype
    (orchestrator/interface-measurements.json, part "run-return"; this Mac, load 56 to 184). At a
    program start on the WebAssembly executor it costs nothing that could be measured: `run` already
    returns there at every exec, because exec gives the program a new store (20,001 returns for
    20,000 execs with and without the suspend). Returning and resuming, against changing the process
    inside the hook, differed by under 100 instructions of the 6.3 million a program start takes.
  - Elsewhere it costs that worker one leave of `Execution::run` (300 ns, docs/runtime.md section 10).
  - Not measured: a real `suspend` and `resume` with the records moved out and back, and anything on
    the native executor. The prototype only took the process's threads out of line and put them back,
    which cost 1,100 to 1,400 instructions.
- **A consistent starting point for a replica.** A change towards `Exact` starts a promise, and a
  replica needs the process's whole state at that moment. B hands it over: the `SuspendedProcess` is
  that state, whole and stopped. With A the embedder must suspend at the same moment anyway.
- **Ways a process gets a config.** A: two. B: one. A reader of an embedder's code sees every change of
  what replay promises as a `resume` with the config written on it.

Weighed so, the plan recommends B, which is what was ruled: the kernel gets no second path, a change of
what replay promises comes with its starting point, and nothing has to be built for it here. What B
costs is that each change moves the process's records out and back, and that a change made inside a hook
waits for `run` to return. The second was measured as nothing at a program start, which is where a
config is chosen.

**What "the next stop" is, and how `suspend` stops a process.** `suspend(self)` must hand back the whole
process at once. It is called with the kernel's lock held, in a hook or on a handle from
`kernel.process(id)`. So it cannot call threads in itself: a call-in parks its caller, and the threads
come in through entries that need the lock (4.5). It works with what is already stopped, or with a stop
the executor can make in place:
- **On a handle from `kernel.process(id)`** (section 5): nothing to stop when none of the group's
  threads runs, which is always so inside `System::stopped` and after a container stop. With threads
  running it is as in a hook below: a group stop in a group in force `Parallel` and not `Exact`, and
  an error otherwise, after which the embedder asks with `process_call_in`.
- **In `on_process_called_in`** (section 5): the kernel has stopped the group before the hook.
- **In any other hook, the group in force `Serial`**: no thread of the group runs guest code (4.4 item
  7). Nothing to stop. The hook's own thread is in its entry, with its registers saved.
- **In any other hook, the group in force `Parallel` and not `Exact`**: the process's other threads may
  be in guest code. `suspend` asks the executor for a group stop of the process's memory and takes the
  process while every thread is held.
  - A held thread is at a stop point, so all its registers are saved (4.5).
  - A thread held with an exit it has not yet handed to the kernel (it was waiting for the lock) is put
    back to the instruction that made the exit. It makes the exit again wherever the process is resumed,
    as Linux runs an interrupted call again.
  - When the group stop ends, each held thread finds its process gone, as a dropped thread does today
    (wt/lib.rs:2303-2306), and leaves `Execution::run`. The kernel has already cleared its processor.
  Counted "group stopped to suspend a process".
- **In any other hook, the group in force `Parallel` and `Exact`**: the other threads must stop at
  positions, which only a call-in gives, and a hook cannot wait for one. When other threads of the
  group run, `suspend` there answers an error that says so, and the embedder asks with
  `process_call_in` and suspends in its hook. When none runs it works as in a `Serial` group.
  - **This is less than the owner ruled.** His guarantee is that a hook fires only when every
    operation on its process is legal (interface notes lines 57-58). Here two are not: `suspend` and
    the change of config below. The hooks it reaches are those that fire with other threads of the
    group running: `on_thread_starting`, `on_process_outgrown` and `on_memory_accessing`, and
    `on_process_starting` for a child made with `CLONE_VM`.
  - The way to keep the guarantee whole is for the kernel to call the group in before each of those
    hooks. Then every watched access, which is what a (`Parallel`, `Exact`) process's record is made
    of, stops every thread of the group through the kernel: for each thread its stop and 300 ns
    (docs/runtime.md section 10), where section 8 stops one thread. An embedder would pay that at
    every hand-over to keep an operation it uses at a few of them.
  - So the plan narrows the guarantee and says so, and the owner rules on it (decision 8).

So `suspend` never stops a thread of a group in force `Serial` or `Exact` in place, and item 5 of 4.4
holds. Two things it does to the rules, and only in the group-stop case: a thread of a suspended process
leaves `Execution::run` without an exit of its own, and is taken, not ended, outside its own entry.

**A call that changes the config inside a hook is not in this plan.** The interface notes proposed one
(`Process::replace_execution_config` in the brief), and the plan's earlier form recommended it, to
save the return of `run`. That return has been measured as nothing on the WebAssembly executor
(above), so it is not needed for speed there. On the native executor it was not measured; at worst
it is one return of `run` in the life of a process. The plan was read again for any other need of
the call, and has none:
- the kernel changes a group's config in force itself when groups are joined (section 3). That is
  inside the kernel and needs no call on `Process`;
- the two moments an embedder chooses a config are a program's start and a process's first new
  thread, where the ordering begins to matter (section 3). Both have a hook, and in both a `suspend`
  followed by a `resume` does it, for one return of `run` in the life of the process;
- where `suspend` answers an error in a hook (a group in force `Parallel` and `Exact` with other
  threads running, above), the call would answer the same error for the same reason.
So there stays one way to give a process a config, `resume`, and it is off decision 9's list.

**A config in a recorded kernel.** `resume` in a recorded kernel with a guarantee of `None` gives a
process in force `External` (section 3). `resume` with `Parallel` and a guarantee below `Exact` gives
a group in force `Serial` there. Both are counted and warned as section 3 says, and
`ProcessInfo::execution_config_in_force()` answers what runs. Neither call fails.

**A group of several processes.** `suspend` takes one process, and the config in force is the group's
(section 3). With the strictest winning:
- to make a group stricter, resume any one of its processes with the stricter config. The whole group
  has it once its other running threads have come in;
- to make it looser, every process of the group that was given the stricter config must be suspended
  and resumed. The embedder learns who shares with whom from the kernel (interface notes lines 161-162).
The group stop of `suspend` is of the process's memory, so a process made with `CLONE_VM` that shares
the memory is held too. When one process of a group leaves the kernel, the pages it shared are marked
watched for those that stay (section 2); that is the embedder interface's to build.

**What is built here.**
- The container's first process gets its `ExecutionConfig` at the start (through `Start`,
  kernel/container.rs:55, read by `Kernel::start`). A child inherits its parent's at fork and keeps it
  across exec.
- Until `suspend` and `resume` exist, that is the only way to give a config. This loses less than it
  seems: a group is born with one thread (section 3), where the ordering makes no difference, so "this
  container's threaded programs run in parallel" or "everything in this container replays exactly" is
  one value at the start.
- Choosing per program arrives with `suspend` and `resume`, at `on_program_starting`, which the owner
  already named as the cheap moment to move a process: one thread, a fresh memory, few records.

## 7. The WebAssembly output for a group whose threads run at once

### 7.1 Today

- The guest memory is imported unshared (tr/code.rs:1001-1010, tr/contract.rs:39-43). The state memory,
  holding thread blocks, the page table and the array of slots, is already shared (tr/thread.rs:33,
  tr/contract.rs:45), and the stop flag in it is already written by another host thread while the guest
  reads it with a plain load (tr/code.rs:1300).
- One wasmtime store per process holds all its threads' instances (docs/runtime.md section 8;
  wt/lib.rs:451-474). `Execution::run` takes the store out, so "one thread of a process runs at a time"
  (wt/lib.rs:2245-2251). The executor is one `&mut` object with one running thread (wt/lib.rs:409-433).
- ARM64's atomic instructions are already WebAssembly atomic operations: the single-instruction atomics
  (arm64/memory.rs:112-202), compare and swap (:203-216), load-acquire and store-release (:346-356), and
  the exclusives, where the load remembers address, size and value (:405-415) and the store is a compare
  and exchange against the remembered value (:329-338). wasmtime, Node and Chrome accept atomic
  operations on an unshared memory (measured, ordering-cost/result.json), which is why this works today.
- **ARM64's barriers are translated to nothing**: `dmb`, `dsb` and `isb` become `I::Nothing`
  (arm64/decode.rs:704), "nothing happens in a single thread" (arm64/instruction.rs:989).

### 7.2 A shared memory, and two forms of code by the ordering

**Decision: the guest memory import becomes shared, always. A process runs one of two forms of code,
chosen by the ordering it was given.** A wasmtime store is used by one host thread at a time, and only a
shared memory can be imported by instances in several stores (wasmtime 50 API documentation: `Store`,
`SharedMemory`). So threads at once need the shared import.

- **The Serial form** is today's translation with the import shared and nothing else changed, compiled
  by wasmtime as today.
- **The Parallel form** differs in four places: barriers and 128 bit atomics (7.3), the hooks that are
  made stop points (4.5), and how wasmtime compiles it: with plain loads and stores left as written
  (7.6).

The plan's earlier form had one form of code for both orderings. Its third check, and what was measured
for this correction, ended that. The brief asks for the measured cost of each choice on `Serial` runs.
There are three (section 14):
- **One form, compiled as today.** `Serial` pays one byte tested at each barrier and each 128 bit atomic,
  0.1 to 0.8 ns each. **It is wrong in `Parallel`.** wasmtime's compiler merges repeated plain loads,
  which breaks x86's ordering, and drops a store of a value the thread has just loaded from the same
  address, which breaks ARM64's too (7.6). Not a choice.
- **One form, with that part of the compiler switched off for every run.** Correct. `Serial` pays the
  byte tests, and 0.0% to 1.8% more instructions on the benchmark set (geometric mean 0.44%), for an
  optimisation it could have kept.
- **Two forms.** `Serial` pays nothing but the shared import, which measured as nothing: between 0.16%
  fewer and 0.33% more instructions over eight workloads, with both signs, and no more than the same
  workload differed between two identical runs (wasmtime 49, Node and Chrome agree,
  ordering-cost/result.json and docs/phase-1-translator.md, "The stop check"). `Parallel` pays the
  costs of the second choice. Compile time and cache size double only for a program that is run under
  both orderings.

Chosen: two forms. The owner's rule is that `Serial` must not get slower, and with no setter (section 2)
a process never changes form while it is in a kernel, which was the main cost of two forms before.

What two forms mean elsewhere:
- **Which form a process runs.** The Parallel form if it was given `Parallel`, else the Serial form. It
  is fixed while the process is in a kernel. `resume` makes the instances again in any case.
- **A group forced to `Serial`** (a neighbour's config, or the executor's limits of 7.4 and section 9)
  keeps running the Parallel form where it has it. That is correct: the Parallel form is right under
  both orderings, and only pays for it. Counted "Parallel form run under Serial".
- **A process with one thread, given `Parallel`**, runs the Parallel form from its start. Its byte is
  zero (7.3), so a barrier or a 128 bit atomic costs it one test, and the compiler setting costs it the
  0.0% to 1.8% above. Rejected: the Serial form until the group's second thread, then the Parallel
  form. It spares programs that never start a thread. But every runtime that starts threads at once
  (Java, Node, Go) would have its start-up code translated and compiled twice, and it is one more path.
- **A memory has one form.** Processes that own one memory together (`CLONE_VM`) run the Parallel form
  exactly when every one of them was given `Parallel`. It is worked out again when the owners change,
  and a change is made with the memory's threads called in.
- **Two wasmtime engines** in the executor, one for each form. They differ in one compiler setting
  (7.6). A process's stores, modules and memory belong to one engine.

**The cache key** (wt/lib.rs:107-115, :183) changes once for the shared import and its declared maximum,
through `OUTPUT_FORM`, and gains two fields:
- the form by ordering, Serial or Parallel;
- whether the code holds the progress count, which is exactly "the guarantee in force is `Exact`"
  (sections 2 and 3). The count is the translator track's to build and measure; this plan only fixes
  that it is a field of the key and never a switch of its own.
So a program has up to four entries at one look-ahead. Two more values would join the first field if
they are ever built: the form that orders atomic instructions for (`Parallel`, `External`) (4.6), and
x86's ordered form for weaker hosts (section 9).

What follows from a shared import:
- **A declared maximum.** A shared memory must declare one. V8 refuses, at compile time, a module whose
  declared maximum is above its own limit of 262,144 pages (16 GiB): measured in Node 24.14.1, "maximum
  memory size (281474976710656 pages) is larger than implementation limit" (section 14). So the declared
  maximum is the executor's address ceiling for that output, and it is part of the cache key through
  `OUTPUT_FORM` (wt/lib.rs:108), which changes once.
- **wasmtime's shared memories are switched on by a setting**, `Config::shared_memory`, off by default.
  `SharedMemory::wrap` refuses without it (wasmtime 50.0.1 src/runtime/vm/memory/shared_memory.rs:59-62).
  The executor already turns it on for the state memory (wt/lib.rs:144). wasmtime's documentation of
  that setting says threads are a "tier 2" feature there, with no security fixes to past releases
  (src/config.rs, the comment above `shared_memory`). The guest memory joins the state memory in
  depending on it.
- **Host-supplied memory.** The runtime owns each guest memory's mapping (docs/runtime.md section 8). In
  wasmtime 50.0.1 `SharedMemory::new` makes its own mapping (shared_memory.rs:34-50), but a shared memory
  that a module defines goes through the host's creator and is then wrapped
  (src/runtime/vm/memory.rs:251-256), and is reached with `Instance::get_shared_memory`
  (src/runtime/instance.rs:554). So each process's memory is defined by a one-memory module.
  - Run by the plan's fifth checker (section 14): the engine set up as the executor's, a one-memory
    module defining a shared 64 bit memory, and the host's creator called exactly once. The memory
    that `Instance::get_shared_memory` gave was the creator's own mapping. An instance in a second
    store on another host thread imported it, and stores went both ways between guest and host
    pointer. `memory.grow` went through the creator. So it works as read, and it is still T2's first
    task to do it in the executor, where today's state memory is made by `SharedMemory::new` and not
    through the creator (wt/lib.rs:1242).
- **One store per guest thread**, always, in place of one per process. The compiled modules and the
  engine of the process's form stay shared. A dropped thread's store is kept for the process's next
  thread, as its instances are today (wt/lib.rs:478-483).
  - This changes how a `Serial` process with several threads runs, although its code is the same: a
    store for each thread, new code instantiated in each store at first use (7.5), another lock
    scheme (below). None of the single-threaded measurements sees that. So T2's bar measures threaded
    programs run `Serial` against the commit before: a switch between two threads, a thread's start,
    the runtimes' programs, and the memory a thread costs (section 12).
  - If that bar fails, the Serial form keeps one store for each process, as today, and only the
    Parallel form has one for each thread. That is a second path through the executor, taken only
    if the measurement asks for it.
- **The executor becomes shareable, with no lock on a `Serial` system call.**
  - Its tables of processes, threads and blocks go behind a lock that is taken only to add or remove
    one. A hook reaches its own thread's and its own process's records through its store's data, as it
    reaches its thread block today (wt/lib.rs:2271-2276).
  - What it keeps per process (page records, code lists, shown blocks, wt/lib.rs:451-474) goes behind
    that process's own lock.
    - In a group in force `Serial` the worker that runs the thread takes it when it enters
      `Execution::run` and keeps it until it leaves, as it takes the store out today
      (wt/lib.rs:2245-2251). Its hooks use what it already has. It asks for the kernel's lock under it
      at every system call. That is the one exception to the lock order (4.5), and it is safe because
      nobody who has the kernel's lock ever waits for this lock: a worker that does not have it only
      ever tries it. Six rules make every try succeed, and the try checks them:
      - the kernel reaches a `Serial` group from outside only after calling it in (4.4 item 5);
      - two groups become one `Serial` group only once every other running thread of both has come in
        (section 3). So a `Serial` group never has two workers that each hold a process's lock of
        it for a running thread;
      - **a worker lets its process's lock go before it lets the kernel's lock go, whenever its
        thread is not told to carry on.** A thread is told to carry on only when it is handed out
        again in the same entry, and a thread of a group that is called in is not handed out. So
        whoever takes the kernel's lock next finds no lock held for a thread that has come in.
        How the order is kept: the answer that tells the thread to leave hands the executor the
        kernel's handle with it (`Carry::Leave(why, entered)`, 4.2). The executor puts the thread's
        store back and lets the process's lock go, and only then drops the handle. Today's code
        has it the other way round: the kernel entry is over inside `Answer::exit`, before the
        executor learns that the thread leaves (rt/lib.rs:264-289, wt/lib.rs:2325-2344). The
        plan's earlier form had the thread let the lock go when it found its asked-in bit set, before
        the entry. Its fourth check showed the gap: the bit can be set after that look, while the
        thread waits for the kernel's lock. The thread then came in with the lock, the kernel woke
        the call that wanted the group, and that call could run on another worker before the lock
        was let go. The rule now does not look at the bit;
      - **a `Cpu` call made inside an entry uses the lock when the worker that makes the entry has
        it.** That covers the thread's own calls. It also covers what else the kernel does in that
        entry: the call that wanted the group in may run again right there, on the worker of the
        thread that came in last, and reach that thread's memory. That worker is in the kernel, not
        in guest code, and no other thread of the group runs;
      - the kernel does a thread's work only when the thread is chosen for a processor (4.3). So a
        woken call of a `Serial` group's thread never runs again, and no signal frame is written to
        its stack, while a sibling runs and its worker has the lock. That holds for a waiter too: it
        is chosen whatever is called in, but never while a thread of its `Serial` group runs (4.5);
      - **a write to a block reaches every memory that shows it, and each of their locks is free or
        the worker's own.** `block_write` puts the bytes in the block and in every memory that shows
        them (wt/lib.rs:1409-1433). It has two callers, and neither leaves a memory out:
        - the copies a `Serial` thread's written shared pages get when it leaves (`flush`,
          wt/lib.rs:1341-1388, :1376). The block is writable through a mapping, so every memory that
          shows it is of the writer's own group (section 3). That group is `Serial` and its one
          running thread is the writer, so the other memories' locks are free. This is the ordinary
          case of a parent and its forked child sharing a mapping, and it stays as today (7.4);
        - a write of the kernel's, for a `write` to a file that memories map. A memory of the
          caller's group has its lock free or held by this worker. A memory of another `Serial`
          group has been called in first (4.5), so its lock is free. A memory in force `Parallel`
          takes the bytes with its stop lock shared (4.5).
        The plan's earlier form said such a walk "skips every other memory in force `Serial`". As a
        rule about where bytes are written that was wrong, and it contradicted 7.4 and 4.5: the
        fifth check showed that a child would stop seeing its parent's stores, and that a file's
        new bytes would not reach a group called in to receive them. The reason given there holds
        for a memory as a **source** only, and that is all that is left of it: nobody reads a
        `Serial` memory of another group to bring a block up to date. A `read` of the file takes the
        block's bytes as they are, which hold all that group wrote before its last entry (7.4);
      - so on a worker that does not have it, taking a `Serial` process's lock is a try-lock. A
        failure is a bug, and it panics, naming both threads. The stress schedule of section 12 runs
        with it, and one test there asks a thread in while it waits for the kernel's lock.
    - In a group in force `Parallel` each hook takes it for as long as it needs it, and never asks for
      the kernel's lock while it has it.
  - A block's bytes go behind that block's own lock. `block_write` finds who shows a block by going
    through every process (wt/lib.rs:1413-1414). It may no longer read the records of processes of other
    groups, so each block keeps the list of the memories that show it.
  - Order: the kernel's lock, then a stop lock, then a process's lock, then a block's, with the one
    exception above.

### 7.3 Atomic instructions, exclusives and barriers

All of this section is about the Parallel form. The Serial form keeps today's translation of each of
these instructions.

**The byte.** Each process has one byte in its state memory, at a place the translator's contract fixes
(suggested name `runs_in_parallel`). It is 1 when the process's group is in force `Parallel` and has
more than one thread, else 0. The Parallel form reads it with a plain load, as it reads its stop word
(tr/code.rs:1300). It is what lets a process that has one thread, or was forced to `Serial`, run the
Parallel form at nearly the Serial form's speed.

The kernel tells the executor each memory's config in force and whether its group has more than one
thread (suggested `Cpu::set_execution_config(memory, thread_ordering, replay_determinism,
several_threads)`). From it the executor sets the byte, and knows which primitive of 4.5 may reach the
memory: a group stop whenever the ordering is `Parallel`, with one thread too, because a file that
changes size reaches the memory from another process. The call is made inside the entry that makes the
group's second thread or changes its config in force, when every other thread of the memory is stopped
or about to start.

- **ARM64's atomic forms listed in 7.1: nothing changes.** They are atomic operations already, in both
  forms.
- **Barriers.** ARM64's `dmb`, `dsb` and `isb` become: load the byte, and when it is not zero,
  WebAssembly's `atomic.fence`. All three kinds of `dmb` become the one full fence.
  - Why it is needed: a program that orders plain accesses with a barrier gets no order at all from
    nothing. The owner's own results name such programs: crossbeam's queue in rayon does "`back.store(b,
    Relaxed); fence(SeqCst)` ... On ARM64 the fence is a DMB", and SQLite checks its shared index "with
    memory barriers" (optimistic-replay-results.json). Measured on this Mac with plain accesses on a
    shared WebAssembly memory (section 14): the store-then-load test saw the forbidden outcome in
    371,914 of 2,000,000 rounds with no barrier and in 0 of 2,000,000 with `atomic.fence`; the checker's
    message-passing test saw stale data 148 times in 3,500,000 rounds without and 0 times with.
  - `isb` orders nothing in memory by itself, but Arm orders a load after a conditional branch followed
    by `isb`, and translated to nothing that order is lost (the third check's finding). The fence gives
    it. `isb` is also what programs put in a spin loop to wait a moment (Rust's `spin_loop` hint on
    ARM64), where a fence does the same job.
  - What the WebAssembly compilers do with it: wasmtime compiles `atomic.fence` to `dmb ish` on ARM64
    and `mfence` on x86-64 (cranelift-codegen 0.137.1, src/isa/aarch64/inst/emit.rs:1809-1811 and
    src/isa/x64/lower.isle:3425-3426; seen in wasmtime's output on this Mac). WebAssembly's rules give the
    instruction no meaning of its own, so this depends on the WebAssembly compiler, and is tested (7.6).
  - What it costs (measured, section 14, a round of store, barrier, load): the fence itself 0.7 ns in
    wasmtime and 6.1 ns in Node on this Mac, 19 ns and 16 ns on the x86 laptop. The test with the byte
    zero: 0.17 and 0.12 ns on this Mac, 0.8 and 0.18 ns on the laptop.
  - On an x86 host the two weaker kinds of `dmb` need nothing, and they still pay `mfence` in a parallel
    group, because the output is the same for every host. How often real programs run them is T3's
    measurement.
- **Exclusives are a compare and exchange.** The store succeeds when memory holds the value the load saw,
  even if another thread changed it and changed it back. Hardware would fail that store. A program can
  tell only if it is written against the instruction itself; compiled code uses these instructions to
  build a compare and exchange, which has the same blind spot by definition. This is a written difference
  from the reference (decision 6). Rejected: an exact monitor, by a group stop at every exclusive pair or
  by watching every store: both cost every lock operation of every program.
- **128 bit pairs.** Compare and swap of a pair of 64 bit registers, and the exclusive load and store of
  such a pair, are plain loads and stores today, "with one thread this is exact" (arm64/memory.rs:239,
  :310). WebAssembly has no 128 bit atomic operation. In the Parallel form they become: load the byte;
  when it is zero, today's plain loads and stores, inline; otherwise one new guest hook, which makes the
  access under a group stop.
  - The hook has the memory's stop lock exclusively, so two threads that run such an instruction on the
    same words go one after the other, and no write of the kernel's lands between the compare and the
    store (4.5, rules 1 and 2).
  - The hook waits for the stop lock, so it is made a stop point (4.5): the translator saves every
    register before it and forgets what it knew about pages after it.
  - The hook's own accesses obey the thread's page table, a per-thread watch included (section 8): it
    checks the two words as translated code would and takes the fault path when it may not touch them.
  - **With the byte at 0, a write of the kernel's from outside must not land inside the inline form.**
    The inline form is a load, a compare and a store, and takes no lock. Against the memory's own
    threads that is exact: there is one. But a `Parallel` memory takes the kernel's writes from other
    processes with nobody called in (4.5): a `write` to a file it maps, `process_vm_writev`, a
    tracer, and the embedder's own `memory_write` (section 5). One that landed between the load and
    the store would be lost while the instruction reported success. Linux forbids that: the
    instruction is atomic against the kernel's copy too. So such a write into a memory whose byte is
    0 is made under a group stop of that memory. Its one thread is then held at a stop point, and
    the inline form has none inside it. With the byte at 1 nothing changes: the write has the stop
    lock shared and the hook has it exclusively. Counted "memory with one thread stopped for a write
    from outside".
    - It costs that write one stop of one thread (230 ns on average by the Chrome measurement of
      4.5), and only when the target is in force `Parallel`, has one thread and shows the pages
      written.
    - Rejected: the byte at 1 whenever the ordering in force is `Parallel`. Every barrier and every
      128 bit atomic of a process with one thread would then pay the fence and the hook, which is
      what the byte is there to spare.
  Exact, and slow: counted "128 bit atomic made with the group stopped". How often real programs run
  them is measured in T3.
- **A dependency the WebAssembly compiler removes.** Arm orders two loads when the second one's address
  is computed from the first one's value, and that holds even when the computation cannot change the
  address (`x ^ x`). A WebAssembly compiler folds such a computation away, and the order goes with it.
  A real dependency, where the address does depend on the value, is kept, because the compiled code
  needs the value to make the address. Only code written to test the processor does the first. A
  written difference (decision 6).
- **x86.** Lock prefixes are emitted as their plain forms today, because one thread runs (x86 plan line
  1839). Asked of the x86 track, for the Parallel form:
  - a locked instruction and `xchg` with memory become WebAssembly atomic operations when the address is
    aligned for its size, and a hook that makes the access under a group stop when it is not (x86 allows
    a locked access at any address; WebAssembly's atomic operations trap on a misaligned one). That
    hook is a stop point too (4.5). `cmpxchg16b` is a 128 bit pair and is treated as one, behind the
    byte. On an x86 host an atomic operation is the lock prefix the program pays natively;
  - **`mfence` becomes the same tested fence as `dmb`.** Lock prefixes alone do not give an x86 program
    its store-then-load order: older compilers write a sequentially consistent store as "mov + mfence,
    no visible atomic" (optimistic-replay-results.json, the detector's validation). Measured on the x86
    laptop (section 14): with plain accesses both loads saw 0 in 134,938 of 2,000,000 rounds, with
    `atomic.fence` in none;
  - `sfence` and `lfence` stay nothing. The first orders only stores that bypass the cache, which the
    translation makes as ordinary stores. The second orders no memory access;
  - plain loads and stores stay plain. What that needs of the WebAssembly compiler and of the host is
    in 7.6 and section 9.

### 7.4 The page table read by several threads while one changes it

The table is one byte per page, read by translated code with a plain load (tr/code.rs:674). What a piece
learned from it holds until its thread's next stop (tr/contract.rs:17-27). So:
- raising a permission needs no stop (4.5);
- lowering one stops the memory's other threads by 4.5's rule, then the write, then carry on. The
  executor's own write locks (wt/lib.rs:486-489) follow the same two rules: placing a lock lowers,
  lifting one raises;
- bytes are written with atomic stores, one byte or one aligned word at a time, so a reader never sees half
  a value.

**Blocks are shown by copying.** Each memory holds its own copy of a block's pages. A page that shows a
block carries a write lock; the first write lifts it and marks the page written; and before the kernel
sees anything, at every exit, written pages are copied to the block and to every other place that shows
them, and locked again (docs/runtime.md section 8; `flush`, wt/lib.rs:1341-1388, called at :2322). That
is exact only while one thread of everything that shows the page runs at a time. Two things follow.

- **A group in which a page of a block that is writable through a mapping is shown in more than one
  place runs `Serial` on this executor**, whatever was asked. More than one place means two memories, or
  one memory twice (a ring buffer mapped twice side by side). The executor states this as a fact about
  itself (suggested `Cpu::parallel_limits()`: blocks are shown by copying; and, for section 9, plain
  accesses are ordered more weakly than the guest's), and the kernel, which knows every mapping, applies
  it (section 3). Counted "group kept Serial: a shared page is shown in two places". The native executor
  maps the same frames in each place (native.md section 5.3) and has no such limit. A real shared mapping
  under wasmtime is possible only where the host's pages are no larger than the guest's (docs/runtime.md
  section 8: this Mac's are 16 KiB); it is not in this plan.
- **In a group in force `Parallel`, a written shared page is not locked again at a turn's end.** Such a
  group has every such page in one place, so the memory's own bytes are the only ones a guest thread
  can see. Locking the page again would lower a permission, which is a group stop, at every system call
  of every thread that had written a shared page (SQLite's shared index, a shared memory file, shared
  anonymous memory). Instead the page stays written, and the executor copies it to the block when the
  block's bytes are asked for: `shared_read` (a `read` of the file, kernel/fs/mod.rs:585), a new
  `shared_map` of the page, `shared_unmap`, `memory_duplicate`. The copy is taken while threads may be
  writing the page, as a `read` of a file being written through a mapping is on Linux.
  - **That copy goes one way, from the memory to the block's own bytes.** Today's `flush` ends in
    `block_write` (wt/lib.rs:1376), which also writes the bytes into every memory that shows the block,
    the writer's own included (wt/lib.rs:1409-1433, with the comment "no guest runs"). Used here it
    would put a snapshot back over the stores the guest made after it was taken, and a `read` of a file
    would undo another thread's stores. So this is its own, smaller copy, which writes no guest memory.
  - The page stays on the written list for as long as it is not locked, so each later ask copies it
    again.
  - A `write` to the file still goes through `block_write`, into the block and into the memory, as any
    write of the kernel's does (4.5).
  - When a second place comes to show a page, the group is called in, its written pages are copied and
    locked, and it runs `Serial` from then on. Counted "shared pages locked again: a second place shows
    them". `Serial` groups keep today's copies at every exit unchanged.

### 7.5 Code a program writes while another thread runs it

- **Dropping code** stops the memory's other threads by 4.5's rule (a group stop, or a call-in where the
  group is in force `Exact`), and every other thread of the memory gives up its frames before it
  carries on. ARM64 announces a change with `ic ivau`, which is already a hook
  (arm64/memory.rs:375-383); it is made a stop point (4.5). x86 does not announce; its code pages carry
  a write lock, and the x86 plan already says the other threads are stopped first (x86 plan lines
  1884-1894, 1941). This plan gives that the same rule.
- **A translation never outlives a drop of its bytes.** `load_code` reads the guest's bytes and compiles
  them in one hook (wt/lib.rs:1885-1981). That hook is a stop point, so a thread that is compiling is
  held (4.5). A drop can therefore go ahead while it compiles, and the pieces it then installed would be
  made from the old bytes. So each code range of a process keeps a count of its drops:
  - the hook reads the count and the bytes with the stop lock shared, and lets the lock go;
  - it compiles with no lock;
  - it takes the stop lock shared again to install. If the count has moved, the compiled pieces are
    thrown away and the hook starts again from the bytes as they are now. Counted "translation thrown
    away: its code was dropped meanwhile";
  - a drop has the stop lock exclusively, so it never overlaps an install, and it moves the count.
  For an x86 guest the page is locked against writes before its bytes are read (x86 plan lines
  1882-1885). Placing that lock takes a permission away, so it stops the other threads too. A write that
  comes later faults into a drop, which moves the count.
  Rejected: the stop lock or the process's lock kept from reading the bytes to installing the pieces.
  Every group stop of the memory, and every other hook of the process, would then wait for a compile.
- **A drop reaches the caller's group only.** Today `ic ivau` also drops the same line in every other
  memory that shows the block (wt/lib.rs:2008-2010). Across groups that is not needed and not allowed.
  Memories of different groups share only blocks that no mapping may write (section 3), so no guest
  store can have changed their bytes; and the executor must not change a `Serial` group's code from
  outside it (4.4 item 5). Such a block's bytes change only through a `write` to the file, which is the
  kernel's entry (4.5).
  - **This one is T1's, not T2's.** Groups already run at the same time in T1. A hook that walked
    every memory showing the block would reach the process records of another group, which that
    group's running thread has locked for its whole stay (7.2). The try-lock would panic.
- **Adding code needs no stop.** Today new code is instantiated in every thread of the process at once
  (docs/runtime.md section 9). With one store per thread, a thread cannot touch another's store. Instead:
  the new module goes on the process's list, its slots are written to the array of slots last, and each
  other thread instantiates it in its own store the first time it needs it. No translated code changes for
  this: a lookup that finds an empty table entry already goes to the load code hook (tr/thread.rs:118-130,
  tr/code.rs:1159), which then finds the module on the list. The list and the slots are written with the
  stop lock shared, as any install is.
- **Growing the tables.** A lookup beyond a thread's table would trap, so every thread's table must cover
  every slot written. Tables grow by doubling, each thread growing its own at a group stop, so a stop for
  this happens only when the process's slot count crosses a doubling. Counted "group stopped to grow the
  lookup tables".

A runtime that compiles code while its other threads run (Java, Node) pays a group stop at each change of
code already run. How many, and what they cost, is T3's measurement.

### 7.6 What the WebAssembly compilers must do, and a browser

WebAssembly's rules promise little for plain accesses that race with another thread's. Its compilers may
merge, move and drop them, as a C compiler may for a program with a data race. A guest's plain accesses
mean more: its processor's own ordering. So four things are relied on that the rules do not promise.
All are tested in the suite on every version of each WebAssembly compiler (section 12).

- **A plain load in a loop is done again each time round.** Guest programs wait on words another thread
  writes, with plain loads (optimistic-replay-results.json: "real programs peek at lock words with plain
  reads constantly"). Measured (section 14): wasmtime 50.0.1, and Node 24.14.1's V8 with its optimising
  compiler alone, its baseline compiler alone, and both, all saw another thread's store from a loop of
  plain loads on a shared memory.
- **In the Parallel form, plain loads and stores are compiled as written: each one made, in program
  order.** The plan's earlier form claimed this of wasmtime as it is. That was false, and the third
  check showed it. wasmtime's compiler has one pass that changes plain accesses, its alias analysis
  (cranelift-codegen 0.137.1, src/alias_analysis.rs:46-67, called for every load and store at
  src/egraph/mod.rs:549-575). Between two compiler fences it can do four things. Three were measured
  for the third correction and the fourth by the fourth check (section 14):
  - *A repeated load is merged into the first.* "Load x, load y, load x" keeps two loads. x86 forbids
    what follows: a reader of a sequence lock (load the count, load the data, load the count again,
    with no barrier, because x86 needs none) never sees the count change. On the x86 laptop, with two
    host threads, such a reader accepted 931,346 torn reads of 1,670,130. Arm allows the merge.
  - *A store of the value just loaded from the same address is dropped*, also when it is under a
    branch. "r = [x]; f = [flag]; if f then [x] = r" keeps no store. ARM64 and x86 both forbid what
    follows: the other thread's store, made before it set the flag, is never overwritten. The outcome
    was seen in 57,368 of 57,368 rounds on this Mac and 199,975 of 199,975 on the laptop. The same is
    done to the second of two stores of one value. The third check did not have this one.
  - *A store is forwarded to a later load of the same address.* Both processors allow that: a thread
    reads its own store.
  - *A store that a later store to the same address overwrites is dropped*, when nothing between
    them is counted as having seen it (the pass's own header, src/alias_analysis.rs:57-66; the
    optimiser acts on it at src/egraph/mod.rs:570-573). Of "[x] = 1; [y] = 2; [x] = 3" it would
    keep two stores. x86 forbids what follows: another thread could see y as 2 with x still old.
    **It does not happen today.** The pass counts every instruction that may trap as seeing every
    store, and every access to the guest memory may trap. The fourth check compiled that shape and
    the same with a load between, by register and by constant offsets from one base: every store
    was kept. But a wasmtime release could change that, so the shape is in the tests (section 12).
    The plan's earlier form said "three things" and left this one out.

  It does none of them across a compiler fence: an atomic operation, `atomic.fence`, or a call
  (src/alias_analysis.rs:93-106). That is why a `Serial` run is safe as it is: a thread's turn ends
  only inside a hook, which is a call. It is also why a guest that orders its accesses with a barrier
  or an atomic instruction is safe (the tested fence keeps the second load, check3).

  **The cure: the Parallel form is compiled with that pass off.** All four are the one pass, so the
  one switch covers them.
  - With it off, on the x86 laptop the sequence lock's reader accepted 0 torn reads of 33,327, and the
    dropped store was seen in 0 of 199,977 rounds; on this Mac in 0 of 61,447.
  - It costs a run 0.0% to 1.8% more instructions (the benchmark set on this Mac, geometric mean
    0.44%), which only the Parallel form pays (7.2).
  - Cranelift has a setting for exactly this, `enable_alias_analysis`, "Do redundant-load optimizations
    with alias analysis" (cranelift-codegen-meta 0.137.1, src/shared/settings.rs:60-68). **In the
    version this project pins, nothing reads it**: the pass runs whenever the optimisation level is not
    none (cranelift-codegen 0.137.1, src/context.rs:194-196). Measured: wasmtime 49.0.2's output is the
    same with the setting off. The fix is to make the optimiser skip the pass when the setting is off,
    at the one place it is called. It was made on a scratch copy for these measurements. So the plan
    needs it sent to wasmtime's project, and carried as a patch to that one crate until a release has
    it (decision 4).
  - Without it there is a cure for x86 guests only: every guest load as an atomic load, which on x86 is
    a plain move and is a compiler fence. Measured: 0 torn reads and 0 dropped stores. But it costs a
    load-heavy loop up to 1.48 times, and up to 1.97 times with the test for alignment that atomic
    operations need (the kernels of ordering-cost on the x86 laptop, section 14). For an ARM64 guest
    on an ARM64 host there is no cheap cure without it: an atomic load is an acquiring load there.
  - A browser's compiler has no such setting. Whether V8 does any of the four was not looked at; the
    suite's tests decide it when a browser is built, and the atomic-load form is what is left if it
    does.
- **`atomic.fence` is compiled to the host's full barrier**, and an atomic operation orders the plain
  accesses after it at least as the guest's own instruction would (7.3). Measured (section 14) with the
  store-then-load test on both hosts: in Node it never saw the forbidden outcome with `atomic.fence`,
  with an atomic store, or with an atomic exchange in place of the barrier; in wasmtime 50.0.1, with two
  host threads on one `SharedMemory`, it never saw it with `atomic.fence`, with the byte tested and set,
  with a compare and exchange, or with atomic stores and loads. With the byte tested and zero it saw
  it as often as with no barrier at all, which is what a process with one thread is given.
- **An aligned plain load or store is one access: no other thread sees half of it.** Guest code
  compiles relaxed atomics, and many reads made without a lock, to one plain aligned `ldr`, `str` or
  `mov`, and relies on an aligned 8 byte access never being seen half done. Both guest architectures
  promise that (the Arm manual's "single-copy atomicity"; Intel's manual, volume 3, "Guaranteed
  Atomic Operations", for a quadword aligned on a 64 bit boundary). The fifth check named this as a
  fourth thing relied on and not promised, and asked for its premise to be confirmed.
  - Read for this correction. JavaScript's rules promise it for integers up to 32 bits only. For a
    64 bit integer they promise it only when the access is atomic: `IsNoTearConfiguration` answers
    false for a `BigInt64` element read or written unordered (ECMAScript, 25.1.3.13 and the rule
    "Tear Free Reads", 29.7.3). The WebAssembly threads proposal's overview says nothing about a
    plain access being seen half done (proposals/threads/Overview.md in its repository). So the
    premise holds as far as these two were read: nothing promises it.
  - What the compilers do today (section 14). wasmtime 49.0.2 compiles a plain 64 bit store and load
    to one `str` and one `ldr` of a 64 bit register on ARM64, and to one `movq` each on x86-64, read
    in its output on both hosts. Run with two threads, one storing 0 and -1 in turn with plain
    stores and one loading with plain loads: no load saw anything else in 1,000,000,000 loads, in
    wasmtime 50.0.1 and in Node, on both hosts. The same test with the word laid across a cache
    line saw mixed halves in 232,854 to 73,917,899 of 1,000,000,000 loads, so the test can see one.
  - So nothing is wrong today, and nothing in the translation changes. It is the suite's test from
    T2 on (section 12), on every version of each WebAssembly compiler. A compiler that split an
    aligned access would leave only atomic loads and stores for the Parallel form, at section 9's
    cost.

With these four, a guest gets the host's own ordering for its plain accesses. That is its own when host
and guest are the same architecture, and stronger for an ARM64 guest on an x86 host. Section 9 is the
case where it is weaker.

**A browser** (not built, docs/runtime.md section 17) needs:
- the page to be cross-origin isolated, so that shared memories can be sent to workers (the HTML
  standard's `crossOriginIsolated`; the two response headers `Cross-Origin-Opener-Policy: same-origin` and
  `Cross-Origin-Embedder-Policy: require-corp`). The state memory is already shared and already written
  from the page's thread, so today's browser runner already needs this. A shared guest memory adds no new
  requirement;
- one worker for each host thread in the loop, each with its own instances of the same compiled modules,
  and its own instance of the kernel's module on the kernel's shared memory (4.2);
- every wait (for the kernel's lock, in `idle`) inside a worker, since a page's own thread may not block;
- a declared maximum within the browser's limit (7.2).

## 8. Per-thread watches on the WebAssembly executor

The owner ruled that exact replay of a parallel group is composed by the embedder from
`memory_watch_thread`, where the executor implements `PerThreadWatch`. That is the pair (`Parallel`,
`Exact`). The kernel's own part of it is the count and the call-ins of 4.5. This section is what the
embedder's part needs. On the WebAssembly executor:

- **A thread's own page table.** Translated code takes the table's place as an imported value, per
  instance (`PERMISSIONS`, tr/contract.rs:62-64). So a thread's instances can be given a table of their
  own. No translated code changes.
- **Only once it is watched.** A thread uses its process's table until its first `memory_watch_thread`.
  Then it gets its own, and its instances are made again with the new place, once per thread (measured for
  one thread: 0.43 to 0.60 ms for busybox's code and 0.27 to 0.42 ms for its C library's, docs/runtime.md
  section 8). Rejected: a table per thread from the start. Every `mmap` would write as many tables as
  the process has threads, for processes that are never watched.
- **A watch is one more reason in the executor's lock layer** (docs/runtime.md section 8): a thread's
  table byte is the kernel's permissions, less what the process's locks take, less what that thread's
  watches take. One function makes it, as today (`effective`, wt/lib.rs:790).
- **Setting a watch stops one thread, not the group.** Only the watched thread's table changes, and the
  rule is that the thread whose table loses a permission must be stopped. It is done with the memory's
  stop lock, with one thread's held bit set, and the thread is held only at a stop point (4.5). So a
  thread that reads the clock in a loop is not taken for stopped while its loop still relies on the
  old table, and no access is missed.
- **The held thread's position can be read.** It is at a stop point, so its registers are saved, and in
  a group in force `Exact` so is its count. The embedder reads both through the handle (suggested, on a
  thread's info, `position()`), which is what its record of a hand-over needs. Holding a thread in
  place is allowed here although the group is `Exact` (4.5): the watch only orders memory accesses,
  which is the embedder's part, and in a replay the embedder brings the thread to that position with a
  counted turn before it sets the watch again.
- **A watch on a page a frame could cover.** This is 4.5's rule for a page that loses a permission,
  and the same piece of code. What a piece proved about its own stack frame is checked again only at
  the next function entry (tr/contract.rs:20-23). Threads share objects on their stacks all the time
  (scoped threads, a joined thread's result, a condition variable on a stack). A watch set on a thread
  for such a page would miss that thread's accesses to its own frame, and the embedder's record would
  miss writes. So when a watch takes a permission from a page that allowed reading and writing, each
  thread it is set on gives up its frames when it is let go. A piece entered from the bottom loop
  knows nothing about its frame and checks every access. It holds for `memory_watch` as for
  `memory_watch_thread`. Counted "left run: a watch was set on a page it could have a frame on".
  - The test is by the page, not by where the thread's stack pointer is. The plan's earlier form
    asked whether the page lay at or above the stack pointer in the same mapping. A thread can have
    frames on a stack it is not on (4.5), and an access missed there is a wrong record with nobody
    told.
  - It costs the watched thread 300.8 ns (docs/runtime.md section 10) and returns that go through
    the bottom loop until its calls in progress have returned. That is paid at every hand-over of a
    page away from a thread that could write it.
- **A watched access** comes to the memory fault hook, and the thread leaves `Execution::run`. The
  executor finds the watch and hands the kernel an exit for it, and the kernel, inside that entry of the
  thread's own, calls `on_memory_accessing`. If the watch is gone when the hook returns, the thread
  carries on.
- **The kernel's own accesses.** `Cpu::read` and `Cpu::write` ignore page permissions (abi/cpu.rs:30-31),
  so no table byte catches a `read` system call filling a page another thread holds. The kernel checks
  these itself, and for that it keeps each process's watched ranges (given to it through
  `memory_watch` and `memory_watch_thread` in any case). A process with none pays one test a call.
  - An access the kernel makes for a thread of the process (a system call's buffers, a signal frame on
    its stack, the words cleared at its exit) is that thread's access. It is checked against that
    thread's watches and the process's before the call has any effect, and the hook fires with that
    thread. The interface already promises this moment for process-wide watches (interface notes lines
    57-59). If the watch stays, the call parks.
  - A write that reaches the memory from outside the process (a `write` to a file it maps, reaching
    pages it still shows; `process_vm_writev`; a tracer) has no thread of the process behind it. The
    ruled hook cannot say that. Recommended: its thread is optional, none meaning "the kernel, for
    another process"; it fires when the process's watch or any of its threads' covers the range; and the
    write is made when the hook returns (decision 9). Without it an embedder's record of hand-overs
    misses these writes.
- **Space.** Each table is one byte per page under the ceiling: 1020 KiB for ARM64's 4080 MiB, of which
  untouched pages cost nothing. They share the state memory's 4 GiB with the array of slots (docs/runtime.md
  section 8), so the ceiling's formula gains the number of watched threads.

**What it costs** (expected, from parts measured apart): a hand-over of a page from one thread to another
is one watched access (leaving `Execution::run` and coming back, 300 ns, docs/runtime.md section 10), one
stop of the thread that held the page (230 ns on average, 30 µs at worst, measured in Chrome), that
thread's frames given up (300 ns again), two table bytes, and the embedder's own record. So about a
microsecond before the embedder's work. The 2008 system
the owner saw paid over 40,000 cycles a hand-over on one machine (dsm-survey/results.json, SMP-ReVirt),
most of it for changing page permissions in hardware and taking the interrupts that follow, which a byte
in a table does not have. The count of hand-overs is the program's and is not reduced: programs that
write the same pages from several threads will hand over constantly. T4 measures it on real programs.

The stand-in does not implement `PerThreadWatch`. The native executor does it with a table root per thread
(section 10).

## 9. An x86-64 guest in parallel, and hosts with weaker memory ordering

**The promise.** x86 promises total store order: a thread's stores reach memory in the order it made them,
and its loads are not reordered with each other. Only a store followed by a load of another address may
appear swapped, and `mfence` or a locked instruction between them forbids that too. ARM64 promises less:
two loads, or two stores, to different addresses may take effect in either order (Arm Architecture
Reference Manual, the memory model). WebAssembly's own rules promise nothing at all for plain accesses
that race.

**Where it matters.** Only for an x86-64 guest, in a group in force `Parallel`, on the WebAssembly
executor. The native executor runs only guests of the host's own architecture (native.md section 11), so
the hardware gives the right order. On the WebAssembly executor there are two cases, by the host.

**On an x86 host**, the host's own order is the guest's, so it is enough that the compiled code makes
every plain access as written. That is 7.6's second point, and it is not so in wasmtime as it is: the
third check found that its compiler merges repeated loads, and x86's order forbids that. With that pass
off (7.6), measured on the x86 laptop with two host threads: the reader of a sequence lock, made of
plain accesses only, accepted 0 torn reads of 33,327; with the pass on, 931,346 of 1,670,130 (section
14). So on an x86 host the Parallel form for an x86 guest is: plain accesses compiled as written, lock
prefixes as atomic operations and `mfence` as the tested fence (7.3). The ordering tests of section 12
check exactly that, the sequence lock and the store-back among them.

**On a host that is not x86** (wasmtime on ARM64, and any browser on ARM) the host's order is weaker.
That it matters on this Mac is measured, not only read in the manual: the checker's message-passing
test, with plain accesses on a shared WebAssembly memory in Node, saw the flag with stale data 148 times
in 3,500,000 rounds (section 14), and the sequence lock above accepted torn reads on this Mac in every
form, 82,860 of 4,204,412 with the pass off. x86 forbids both.

**What staying correct needs there.** Every guest load and every guest store as a WebAssembly atomic
operation. Atomic operations are sequentially consistent, which is stronger than total store order.
Making only the stores atomic is not enough: two plain loads may still be reordered on ARM. Atomic
operations trap on a misaligned address and x86 allows any address, so each access also needs an alignment
test, with misaligned accesses made another way (under a group stop, or plain and unordered as a written
difference).

**What it costs**, measured before this plan on this Mac, single-threaded (ordering-cost/result.json):
every access atomic costs from nothing (a pointer chase) through 1.1 to 1.4 times (sums, byte loops,
calls) to 1.8 to 2.2 times (copying 8 bytes at a time). With the alignment test, 2.0 to 2.7 times on the
copy. On ARM64 wasmtime and V8 both turn an atomic load into a load-acquire and an atomic store into a
store-release. On an x86 host the same code costs 12 to 17 times, since each atomic store gets a full
barrier, so this form must never be used there. It is therefore a form of its own, used only for x86
guests in parallel on weaker hosts, and a value of the cache key's form field (7.2).

**What is done until it is built.** On such a host the executor reports that it can run an x86 group only
`Serial` (section 3). The group runs `Serial`, counted "x86 group kept Serial: the host orders memory more
weakly", warned once per container.

Whether the ordered form is built at all is the owner's (decision 5). Recommended: not now.

## 10. The native executor

docs/native.md section 5.3 already describes threads as agreed in discussion. This plan fixes what it
builds, in the native track's own milestones after its single-thread suites pass:

- **One virtual CPU per running guest thread.** The pool that today gives a virtual CPU to each container
  with something to run (native.md section 5.1) gives one to each processor that has a thread. A
  processor that finds none waits, counted "waited for a virtual CPU". The platform bounds the pool
  for the whole host process, since one virtual machine holds every container the process runs
  natively (native.md section 5.1). So the bound is on the processors with a thread, summed over those
  containers:
  - Hypervisor.framework: 64 virtual CPUs on this Mac (`hv_vm_get_max_vcpu_count`, native.md section
    5.1);
  - KVM: what `KVM_CAP_MAX_VCPUS` reports (Documentation/virt/kvm/api.rst). native.md had not read it
    (line 880). Read on the x86 laptop for this correction (section 14): 4096, and `KVM_CAP_NR_VCPUS`,
    the number KVM recommends, 12, which is the laptop's count of logical processors. KVM on ARM64 was
    not read: no such machine is at hand.
- **The kernel's lock is inside.** A virtual CPU that finds it held waits in the inside, not on the host
  (native.md section 5.3). This is the `lock_api::RawMutex` of 4.2 for that build.
- **Stopping one thread** is an interrupt to its virtual CPU. The request is kept as it is on the
  WebAssembly executor (4.5): a bit in the thread's record, set before the interrupt is sent, and looked
  at by the inside, with interrupts masked, each time it is about to enter guest code. An interrupt that
  arrives while the virtual CPU runs the inside's own code is then not lost. From the host,
  `hv_gic_set_spi` took 2.4 to 4.1 µs to reach the inside's handler (native.md section 5.5). From one
  virtual CPU to another inside, through the interrupt controller, is not measured; the native track
  measures it with this work.
- **Group stop** is that interrupt to each virtual CPU running a thread of the group. Each lands at an
  instruction boundary and waits in the inside. The hardware checks each access of the native output
  as it is made, so every instruction boundary is a stop point there, once the next item is done.
  Nothing there relies on a frame checked earlier, so no thread has frames to give up (4.5).
  - **Each held virtual CPU drops its own cached translations of the changed pages before it is let
    go.** The plan's earlier form said the architecture broadcasts the invalidation. That had no source
    and is not so on x86, which the native output also covers: x86 has no broadcast form, and a change
    is carried to the other processors by interrupting them (Intel's manual, volume 3, "Propagation of
    Paging-Structure Changes to Multiple Processors"; `INVLPGB` exists only on recent AMD processors).
    The held virtual CPUs are in the inside's handler already, so each does it there, on both
    architectures. ARM64's broadcast forms (`tlbi vaae1is`, native.md line 987) may replace it there
    if the native track measures them as cheaper. native.md already names interrupts between virtual
    CPUs "for TLB shootdown" (line 1032).
  - One stop lock per memory and the count of drops per code range (4.5 rule 1, 7.5) hold there as on
    the WebAssembly executor. Rule 2 is needed only for the page tables and the code lists: the
    hardware makes the 128 bit atomics, so no hook reads and writes guest bytes under a group stop.
  - Which primitive is used for which group is 4.5's rule there too: a group in force `Serial` or
    `Exact` is called in for a change of the kernel's.
- **A table root per thread, only once it is watched.** A thread without per-thread watches uses its
  process's root. One with watches has its own root and address-space identifier, sharing every lower
  table that is the same for all (native.md section 5.3; its N1 builds and tests the per-thread root). This
  is `PerThreadWatch` there.
- **Atomic instructions need nothing.** The native output copies the guest's single-instruction atomics,
  compare and swap, load-acquire and store-release, with an alignment test in front (native.md lines
  363-366). The hardware does them, the 128 bit pair included. No hook and no byte.
- **Barriers: two forms by ordering, as on the WebAssembly output.** The native output emits no
  instruction at all for what the decoder calls nothing, `dmb`, `dsb` and `isb` among them, and leaves
  their route "decided with the WebAssembly output's" (native.md lines 350-362). Decided here: the
  native output has the same two forms of code, under the same field of the cache key (7.2).
  - **The Serial form emits nothing for `dmb`, `dsb` and `isb`**, as native.md has it today. One
    thread of the group runs at a time, and the hand-over between two of them goes through the
    kernel's lock and an exception return, which order everything. So a `Serial` run pays nothing,
    as on the WebAssembly output.
  - **The Parallel form copies them as written**, each of the kind the guest wrote. The guest then
    pays what the reference pays. There is no byte to test. The other forms decoded as nothing stay
    nothing in both.
  - The plan's earlier form copied them always, and left what that cost a `Serial` run to be measured
    later. Its sixth check faulted that against the rule that `Serial` must not get slower. Measured
    for this correction, as hardware instructions at user level on this Mac (section 14): `dmb ish`
    1.3 ns, `dmb ishld` and `dmb ishst` 1.2 ns, `dsb ish` 4.8 ns and `isb` 8.2 ns each. Rust's hint
    for a spin loop is `isb` on ARM64, so a `Serial` program that spins would have paid 8 ns a round
    for nothing.
  - What two forms cost: a program that is run under both orderings is compiled twice, as on the
    WebAssembly output.
  - Rejected: one form that tests a byte before each barrier. The test measured 0.1 to 0.8 ns in the
    WebAssembly output (7.3), on every barrier of every `Serial` run.
  - **x86-64 the same.** In the Serial form `mfence` emits nothing and a locked instruction is
    emitted without its prefix, as the WebAssembly output's Serial form makes it plain (x86 plan line
    1839). The Parallel form copies both. Measured on the x86 laptop (section 14): `mfence` 17.2 ns
    and a locked add 5.9 ns each. `xchg` with memory locks with no prefix and has no plain form; the
    Serial form copies it unless the native track measures that two moves are worth writing.
  Asked of the native track with its N9 and its thread milestone. The plans do not say any of this
  yet.
- **Exclusives are a compare and exchange there too.** The native output keeps the WebAssembly output's
  record of the load and makes the store with `cas` or `casp` against the recorded value (native.md lines
  484-500). So the difference of 7.3 holds on both executors (decision 6). One thing in it assumes one
  thread: when an interrupt lands between the load and the writing of the record, the record's value is
  "read again from guest memory ... which nothing has changed since, as one virtual CPU runs the
  container's threads" (lines 492-496). With threads in parallel another thread can have changed it, and
  the store would then succeed against a value the guest never loaded. Asked of the native track: in
  the Parallel form, take the value from the registers the load filled, or run the load again.
- **Shared pages between processes** are the same frames, so a group that shows a page in several places
  runs in parallel there.
- **Replay** is the same recording point (4.6), asked inside (below).

**This plan's calls, on the native executor.** There the kernel, the loop and the embedder's system run
inside the virtual machine, as plain function calls. Only the system's services that need the host
(files, the network, time, pages and streams carried to other machines) leave the machine, through the
link (native.md at its head, lines 1092-1095, the owner's word on its decision 1; section 6.1; section
6.2, R3, which rejects "forwarding every `System` call to the host"). One crossing of the link costs
0.7 µs on this Mac and 5.6 to 6.2 µs on the x86 laptop (native.md section 5.6). The plan's earlier
form promised calls "from any host thread" and added methods to `System` and `Host` without saying
on which side each falls. Its sixth check faulted that. They fall so:
- **Inside, with no crossing: everything the kernel calls at an entry.**
  - `kernel_entry_admit` and `on_kernel_entry_ended`. A recorded kernel asks at every entry. A
    crossing there would add microseconds to each system call, against 21 to 32 ns for a call
    answered inside (native.md section 6.1). So a system that records keeps its log inside and
    sends it out in batches, as the inside sends warnings (native.md section 5.4).
  - `thread_stop`: an interrupt from one virtual CPU to another through the interrupt controller
    (above; not measured).
  - `on_deadline_changed`: arms the inside's timer (section 5).
  - `on_processor_needed`: wakes a virtual CPU that waits inside. When that processor's stack is
    parked and its virtual CPU has gone back to the pool (native.md section 5.1), it is one message
    to the monitor, which is the cost of waking a worker there.
  - `on_execution_config_in_force_changing` and `on_process_called_in`: hooks handed a `Process`,
    run inside.
  - `turn`, `ran`, `arrived`, `stopped` and the deciding half of `idle`, with the arguments decision
    9 adds: the policy's, inside, as R3 has them.
- **Calls into the kernel are made by the system's code inside, on any virtual CPU**: `run` for a
  processor, `kernel.process(id)`, `Kernel::process_call_in(id)`, `resume`. Where sections 5 and 6 say
  "any thread that can call the kernel", that is what it means there. A program on the host cannot
  make them, and cannot borrow a `Process` or a `ProcessInfo`: "with the kernel inside a virtual
  machine, a host program cannot borrow its data" (native.md lines 1368-1371; its decision 11
  recommends building whatever reads kernel state into the inside). A host program that wants a
  process read or called in sends a request through the link to the system's code inside and gets a
  copy back, for one crossing.
- **Stopping a thread** through the executor's handle (section 5): from inside, the interrupt between
  virtual CPUs; from a host thread, an interrupt raised into the machine (2.4 to 4.1 µs, above).
- **A worker** is a virtual CPU with its pool thread. `run_until_finished` hands each one call that
  runs inside, and a worker told `Idle` parks its stack and gives its virtual CPU back (native.md
  sections 5.1 and 6.1).
- **The lock** waits inside, its holder's name is the virtual CPU's number, and the mark for a guest
  hook is kept for each virtual CPU (4.2).
- **So decision 9's promise holds on this executor too.** Every call this plan adds is on the inside
  half. Splitting the kernel's lock later (4.7) moves none of them across the link and changes no
  signature.
The exact split of `System` into policy and services is the native track's to show the owner as code
(its N3). This plan adds the list above to it.

Asked of the native track: the two forms with their barriers, the exclusive's record and the list
above, with its own thread milestone. Nothing before that. What it must not close off is already
written in its section 5.3.

## 11. Scheduling

- **How many processors the container is told.** A value of the container, the same on every machine,
  because the guest sees it and a replay or a move must see the same: suggested
  `GuestMachine::processors` (rt/container.rs:39-44), given to the kernel in `Start`. Suggested default: 1,
  so every existing test and every existing user sees what they see today; the command line program passes
  the host's count unless told otherwise (decision 2).
- **How many run at once.** As many guest threads as the container has processors, and never more than
  one thread of a group in force `Serial`. The embedder's limit is the count of processors it gives
  the container. A live run has one worker for each processor, also on a host with fewer cores
  (section 5): a container told 16 processors and brought to a host with 8 cores runs 16 workers,
  which that host shares its cores among. The plan has no count of workers below the processors
  (known fault 2).
- **Which host thread runs which guest thread.** Any. A guest thread is its register block and its memory;
  on the WebAssembly executor its store moves between host threads (a wasmtime store may be sent, not
  shared). In a live run a worker keeps its processor, and the kernel puts a thread on whichever
  processor asks. Keeping a thread on the processor it last ran on is added only if measured to matter.
- **Fairness.** Linux's fair scheduler as built (kernel/schedule.rs), with one run queue for all
  processors: each takes the thread with the least weighted time that may run there (4.3: its group
  puts it forward, and its mask holds the processor). One queue is fair by construction and costs
  nothing extra under one lock. Inside a `Serial` group the order is the one-processor scheduler's:
  the group puts forward its ready thread with the least weighted time, and no other (4.3). The one
  exception is a waiter, which goes first (4.5).
  - **A woken thread that should run before a running one takes the place of a thread in its way.**
    Today the comparison is with the one running thread (`check_preempt`, kernel/task.rs:577-584,
    called at :561 and :574; `preempts`, kernel/schedule.rs:224-238). With several processors, and
    when no idle processor can take the woken thread, it is compared with the thread in its way that
    has the most weighted time (section 5). If it should run first, that thread's processor is
    marked to give way and the thread is asked in with `thread_stop`. The plan's earlier form
    compared it with the running thread with the most weighted time in the whole container. That
    may be a thread of another group while the woken thread waits behind its own sibling, or a
    thread on a processor outside the woken thread's mask, and stopping it gains nothing.
  - **The comparison is not what keeps a thread from waiting for ever.** It says no for a batch or
    idle thread, and inside the wake-up granularity (kernel/schedule.rs:233-237). What keeps it is
    section 5's condition: somebody in the ready thread's way is due in, by a slice or by being
    asked. That thread's entry runs the tick, which gives way or sets a slice.
  - The length of a slice is worked out as today, from the whole queue (kernel/task.rs:43-53). With
    several processors that is shorter than it need be, and a thread may come in at its slice's end
    with nobody that could take its place. It is then handed out again. Counted "turn ended for
    nobody", and listed at the top as a known fault to settle with T1's measurements.
- **Timers.** The kernel's deadline is in nobody's slice. The system hands the kernel one
  `Event::Timer` for the container at the deadline the kernel last announced, and that entry stops
  nobody by itself (section 5). A thread's slice holds its fair share and its own CPU timers only.
- **Told many, running one.** A threaded program in a `Serial` group is told the processors but runs one
  thread at a time. A program that spins on a lock because "there are other CPUs" wastes its turns. It is
  correct, and it is what `Serial` means; the cost shows in T2's measurements.
- **Charging CPU time.** Each thread is charged from its own "handed out at" time to its return, on the
  system's monotonic clock (today one time for the whole kernel, kernel/task.rs:77-86). As today, in a
  free turn this includes the executor's own work and the host's preemption (docs/runtime.md section 9).
  A process's CPU time is the sum of its threads' and may now run faster than the clock, as on Linux. CPU
  timers and `RLIMIT_CPU` end a turn by the thread's own slice, as today (kernel/task.rs:59).
- **The tests' clock, and what repeats without a log.** The test system's repeatable clock moves by the
  progress each thread reports (docs/runtime.md section 9).
  - With one worker a run still repeats, with any number of processors, and it is built on the same
    `run`. One worker for several processors is allowed here, where it is not in a live run (section
    5), because the system says which processor is served next. `run(processor)` returns only for one of
    the owner's four reasons (section 5), and none of them is "this turn is over, serve another
    processor". So the test system makes its order the way a replay follows one (4.6). It says at the
    start that it orders entries, and it draws from the seed which processor is served next. A choice
    for any other processor it answers `Later`; `next` then answers `Idle`, `run` returns, and the
    driver calls `run` for the processor drawn. It is a log made up as the run goes. Every turn is
    counted, so the order of entries, and of clock reads, is the same each time. Rejected: a second
    driver loop for tests that steps one turn at a time. It would be a path the shipped loop never
    takes.
    - This holds for processes in force `Exact`, which is what the test system gives (section 12).
      Only code with the count takes a counted turn. A process in force `External` is asked about
      like the rest but takes free turns, so a test that gives a process anything but `Exact` gives
      up repeating it without a log.
  - With several workers nothing repeats without a log. The order of entries is the host's timing. So is
    a clock read made without a system call (`Answer::clock`, rt/lib.rs:291-293): it makes no kernel
    entry, and takes whatever the other workers' threads had reported by that host moment. Such a run is
    repeated from its recording (4.6), where each clock read is a recorded answer of its thread.

## 12. Tests and the bar

The test system gives every process (`Serial`, `Exact`) unless a test says otherwise. That is what the
kernel test crate's runs are today: one thread at a time, counted turns. It says that it orders
entries, so its kernel is a recorded kernel. Tests in free turns against the reference use a system
that does not.

- **Nothing changes with one processor.** With every process (`Serial`, `Exact`) and one processor,
  every test of the kernel test crate gives the same verdict as before, with and without `--features
  runtime`, at full, none and bounded look-ahead, and `repeats` still repeats. Checked as J1 checked it:
  the `test ...` lines from both commits kept and their `diff` empty. The same suites also pass against
  the reference with every process (`Serial`, `None`), in free turns.
- **Groups in parallel.** The same crate's tests that run several processes pass with several processors
  and free turns, against the reference.
- **Exact replay with every process (`Serial`, `Exact`).** A test system records a run with several
  processors (the order of entries, the host's answers, each stop's position), then replays it with the
  replayer of 4.6. Every thread's registers at every entry, a hash of every memory at every entry, and
  every answer of `getcpu` are the same. Run over seeds, with one worker, with fewer and with more than
  the recording had.
- **`External`.** With every process (`Serial`, `External`):
  - the kernel test crate's programs that have one thread and take no signal while they compute replay
    every time, over seeds and worker counts, with every digest equal and every byte handed to the
    system equal;
  - a threaded program written to depend on where its threads take turns is recorded and replayed over
    many seeds. Each replay either has every digest equal, or is thrown away and says at which entry.
    No replay is accepted that handed the system a different byte;
  - **a kernel is recorded whole.** A process given `None` beside them, in the recorded kernel: it
    runs in force `External`, the counter of section 3 says so, and the recording system is asked
    about every one of its entries. The run replays as above, also when that process writes a pipe
    the others read. The sixth check's case is one of the runs: one processor, two processes that
    wait and wake by turns, and a third given `None` that only computes, with the lengths of its
    turns varied by the seed between the recording and the replay. The replay makes the recording's
    choices and never waits;
  - the same programs in a kernel whose system does not order entries: the system's count of
    questions is zero for every process, whatever each was given.
- **A replay that has left its recording ends.** A recording is replayed from a log that was changed
  on purpose, one change at a time: two choices swapped; a return's kind of exit changed; a digest
  changed; a thread's return put after that thread's end. Each replay ends within a bounded number of
  entries with "replay thrown away" and the kind of difference (4.6). None waits. In T1 the first,
  second and fourth, with every process `Exact`; the digest in T8.
- **The config in force.** Two processes given different configs come to share a file mapping, and
  later stop sharing it. Until `resume` exists, a build for tests gives a forked child its own config.
  - In a kernel that is not recorded, one given `None` and one `Exact` (T1):
    `Process::execution_config()` answers what each was given throughout. The config in force and
    both counters change at the `mmap` and back.
  - The hook `on_execution_config_in_force_changing` fires once for each process whose config in force
    changes, before the mapping is made. At a join the test build finds every thread of both groups
    at a kernel entry of its own.
  - In a recorded kernel, from T8 on, since it needs a process below `Exact` there: a process given
    `External` (and one given `None`, which is in force `External`) joins an `Exact` neighbour while
    both run. The test system asks for a container stop in the hook and saves the container at it
    (`System::stopped`, rt/lib.rs:156-158). From the join on, that process's entries are recorded with
    positions (from code with the count, once the translator has it; in counted turns until then).
    Restored from the save, the two replay exactly.
  - The same run with a test system that does nothing in the hook: the neighbour replays exactly up
    to the join's entry, and the test says so. It claims nothing beyond it.
  - In T1 a recorded kernel is given a first process with `None` or `External`: the start fails with
    the error that says `External` is not built.
- **Groups joined while both run.** Two processes of two `Serial` groups each compute on a processor of
  their own. One has a file mapped privately and reads it in a loop. The other maps the same file
  shared and writable, and writes it. Then the other way round: the running one has the file mapped
  shared and has written pages it has not yet left with, and the other maps it privately. Over seeds,
  with the test build's pause points on both sides of the `mmap`:
  - "two threads of a Serial group running" (section 5) stays at zero, and the try-lock of 7.2 never
    fires;
  - the `mmap` returns only after the other process's thread has made an entry, and the mapper then
    sees every store the other made before that;
  - "group called in to be joined" counts each join once;
  - the run replays exactly (with both given `Exact`), the call-in's entry among its entries;
  - the same with `shmat`, and with an exec of a file that another process has mapped shared and
    writable. Each matches the reference.
- **Two call-ins that want each other's groups.** The sixth check's case, as guest programs. Process
  P1 has two threads and maps file g; process P2 has one and maps file f. A thread of P1 calls `write`
  on f in a loop while P2's thread calls `write` on g in a loop, and P1's other thread computes. With
  two processors and with four, over seeds, with the test build's pause points after each call has
  parked and after each thread has come in:
  - both loops finish, each `write` lands whole, and every mapped byte matches the reference;
  - "two threads of a Serial group running" stays at zero and the try-lock of 7.2 never fires;
  - a build for tests checks at every lock release that no call-in is pending with nothing in the
    kernel left to move it: every pending call either has a running thread still asked in, or its
    thread is a waiter that `next` would choose. Counted "call-in with nothing to move it", which
    stays at zero in every run of the stress schedule;
  - the same shape with two `mmap` calls that each join the other's group, with `process_vm_writev`
    both ways, and with a waiter that is sent `SIGKILL` while it waits: its mark comes off and the
    other group runs on;
  - from T4 on, two threads of one (`Parallel`, `Exact`) process that call `mprotect` at the same
    time, each calling its own group in;
  - recorded with every process `Exact` and replayed with one worker and with more: each choice that
    ran a waiter's call and handed nobody out is in the log at its place.
- **No thread waits behind its own sibling.** In free turns, with no slice timer running for a thread
  alone on its processor, with one processor and with four:
  - a `Serial` process whose thread A only computes and never makes a system call, while its sibling
    B sleeps a millisecond a hundred times and then ends the process with `exit_group`. The process
    ends, in no more than ten times the reference's time (the margin is for this machine's load and
    is the orchestrator's to set, as for T1's scaling test);
  - the same with B waiting in `read` on a pipe that a process of another group writes a byte to a
    hundred times;
  - the same with A pinned to processor 0 and B to processor 1;
  - the same with B a batch thread (`SCHED_BATCH`), for which the wake-up comparison always says no;
  - the kernel test crate's Go programs with one goroutine that only computes (go.rs): they finish,
    as they do today.
  In every run of the stress schedule the test build's count "a ready thread with nobody due in"
  (section 5) is zero.
- **A choice that hands nobody out.** Two processes wait in `read` on one pipe and a third writes one
  byte: both are woken, the first chosen takes the byte, and the second's call runs again when it is
  chosen and parks again. And a thread is chosen with a fatal signal waiting for it, which ends it.
  Recorded with several processors and replayed with one worker and with more: each such choice is
  in the log at the same place, and every thread's registers and memory are the same at every entry
  after it.
- **The kernel called from inside a hook.** A test system keeps a reference to the kernel and calls
  `kernel.process(id)` from a hook, from `on_processor_needed`, from `on_deadline_changed` and from
  `arrived`: each answers the error, and none waits. It calls another entry point from a hook: the
  panic names the call. The same calls from another host thread while a hook runs wait for the lock
  and then succeed.
- **The kernel called from inside guest code.** A test system calls `kernel.process(id)` from its
  clock's answer, from `ran` when the clock hook calls it, and from `cpu_warning`: each panics and
  names the call, with the kernel's lock free and with it held by another worker, and none waits. The
  same `ran` called at a kernel entry does not panic. In T1 in a `Serial` group; in T2 also in a
  `Parallel` group whose other thread calls `mprotect` in a loop, where the earlier form would have
  hung.
- **Reaching across groups.** Guest programs for each case of 4.5 where one group reaches another: a
  writer through a shared mapping beside a reader through a private mapping of the same file; a `write`
  to a file another process maps; a file made shorter, and a file made longer, while another process
  maps and runs it; a signal that ends a process whose thread is running. Each matches the reference,
  and each replays exactly.
- **No request to stop is lost.** A process that only computes, alone on its processor and with no
  timer, is sent `SIGKILL` from another group, over many seeds. A build of the executor for tests pauses
  the target's worker, by the seed, at each place where today's loop writes over the stop word: after
  the hand-out, after a fault is settled, after a rerun, after each answer to carry on, and in the stop
  hook (wt/lib.rs:2283, :1786). The process always dies. The same test with a group called in for its
  pages, and with `sched_setaffinity` moving the thread. The translator's own test of `raise_stop`
  (tests/test_guest.rs) runs with another host thread setting the asked-in bit at the same time, and
  neither bit is lost.
- **No processor idles with a thread it could run.** Guest programs with no timer set, in free turns,
  over seeds. Each finishes, and makes the progress the reference makes:
  - a process that only computes maps a file that another process writes in a loop, so its group is
    called in again and again while its processor goes idle. It keeps computing;
  - in a `Serial` group, one thread is pinned to processor 1 and waits on a futex. The other, on
    processor 0, wakes it and then parks on a futex only the first can wake;
  - a ready thread pinned to a busy processor has its mask widened by another thread to hold an idle
    one. It runs at once;
  - from T4 on: eight threads of a (`Parallel`, `Exact`) process on eight processors, one of which
    calls `mprotect` in a loop. All eight keep running between the calls.
  In every run of the stress schedule the test build's count "a processor left idle with a thread it
  could run" (section 5) is zero.
- **Timers and slices.** Read from the kernel's counters, against the reference for what the guest
  sees:
  - a thread that sleeps 20 µs at a time, beside as many threads that only compute as there are
    processors: each sleep ends on time, and each expiry brought in one thread at most, not one a
    processor. With one processor left idle it brought in none;
  - a process forked while every processor runs a thread that only computes and has no slice gets
    to run, and no thread is asked in for it more than once;
  - with one processor and the tests' clock, a counted turn still ends at the kernel's deadline, now
    by the test system's own limit, and `repeats` still repeats.
- **A process's signal finds a thread.** `SIGTERM` with a handler is sent to a process. The test
  build pauses the thread the kernel picked between the pick and its next entry, which is
  `pthread_sigmask` blocking the signal in one run and `pthread_exit` in another. The handler runs
  on a sibling: in T1 one that is parked in `read` in a `Serial` group, in T2 one that only computes
  in a `Parallel` group. Matches the reference.
- **The executor's lock rule.** Every run of the stress schedule has the try-lock of 7.2 on: reaching a
  `Serial` process's lock from a worker that does not have it panics. Two runs aim at it:
  - a thread of a `Serial` group is paused after it last read its stop word and before it has the
    kernel's lock, and its group is called in from another group during the pause;
  - the call that wanted the group runs again on the worker of the thread that came in last, and on
    another worker, by the seed;
  - a thread of a `Serial` group is told to leave (another thread runs next; nothing to run; an entry
    told `Later`), and its worker is paused after the answer and before the executor has let the
    process's lock go. Another worker's entry that reaches that process is waiting for the kernel's
    lock. It gets the lock only after the process's lock is free (4.2, the handle in `Leave`).
- **A worker for each processor.** A container told more processors than the host has cores (16 on
  the x86 laptop held to four cores with `taskset`, cores 2 and 8 left out): `run_until_finished`
  starts 16 workers, a thread pinned to processor 15 runs and reads 15 from `getcpu`, and the
  multi-process programs pass against the reference.
- **One worker, several processors, no log.** The test system with one worker and four processors
  runs the kernel test crate's multi-process programs twice from one seed. The order of entries, each
  entry's position and every answer of `getcpu` are the same, through the same `run` (section 11).
- **Processors and masks.** A thread pinned to each processor in turn, by itself and by another thread
  while it runs, reads `getcpu` and its `rseq` area and never sees a processor outside its mask. A
  child inherits the mask. Matches the reference.
- **A suite of threaded programs** (new, in the kernel test crate, built for the guest as its other
  programs are), each with output that does not depend on the schedule: locks; condition variables;
  atomics in each instruction form (the single-instruction ones, exclusives forced by the build flags,
  128 bit pairs); thread-local storage; signals sent to threads, masks and `sigwait`; fork from a threaded
  process; exec and `exit_group` with threads running; `membarrier`, and its restartable-sequence command
  against a thread that is inside a sequence; robust futexes, priority-inheritance locks and
  `FUTEX_WAKE_OP` under contention; code written by one thread and run by another while it is rewritten;
  a wait on a word with plain loads; a shared file mapping written by several threads and read back
  through the file while they write. Each matches the reference in `Parallel`, on every executor.
- **The races this plan names**, each forced by a pause point of the test build, by the seed:
  - **a permission change is never torn.** One thread runs a loop with no call in it that reads the
    counter register, loads an atomic flag, and reads and writes a page. Another thread takes the
    page's permissions away with `mprotect` and then sets the flag. The first thread is paused inside
    its clock hook while the `mprotect` runs. It never completes an access to the page in a round
    whose flag it read as set: it faults. The same with `munmap`, and in T4 with a per-thread watch;
  - **a permission change reaches a live frame.** One thread keeps an array in its stack frame and
    reads and writes it in a loop, with a call made and returned inside the loop. Another thread
    takes the page's permissions away with `mprotect` and then sets a flag. The first never
    completes an access to the array in a round whose flag it read as set. The same with `munmap`
    followed by a new mapping at the same address, whose bytes the first thread must never change.
    The same again with the first thread on a second stack while the page is on the first: it has
    moved its stack pointer inside a call, as Go's `systemstack` does, and faults when it is back.
    In T4 the same three with a watch;
  - two threads run a 128 bit compare and swap on the same pair while a third fills the same words
    through `read`: no update is lost. A fourth unmaps a page the loop around the compare and swap
    also touches: no access after the unmap;
  - a process with one thread, given `Parallel`, runs a 128 bit compare and swap in a loop on two
    words of a file it maps, while another process changes the same words with `write`. Over many
    rounds no `write` is lost under a swap that reported success, and the counter of 7.3 for the
    stopped memory is above zero;
  - one thread rewrites code and announces it while another is translating the same code, paused
    between reading the bytes and installing the pieces: the old code never runs after the
    announcement;
  - a thread stores to a shared file page while another thread's `read` of the file copies the page
    to its block: no store is undone.
- **Under a stress schedule.** The test system asks random running threads in at random times, varies
  the number of processors and of workers, and delays entries at random, all from a seed. Each program
  runs over many seeds. With several workers a seed does not make a run repeat (section 11), so every
  such run is recorded, and a failure in a group in force `Exact` is replayed from its log.
- **Ordering tests.** Small two-thread tests of memory ordering, many rounds each, as guest programs
  through the translator and also as hand-written modules run directly in each WebAssembly compiler:
  - the four standard ones (store buffering, message passing, load buffering, independent reads of
    independent writes), written three ways: with plain accesses only; with the guest's barrier
    instruction where the pattern needs one; and with each atomic form of the guest standing in for the
    barrier;
  - the four shapes a compiler's merging and dropping breaks, which the standard four do not have (the
    third and fourth checks' findings): a sequence lock read with plain loads (load the count, load
    two words, load the count again); a value loaded and stored back under a branch that another
    thread decides; a store read back across a load of another address; and a store overwritten by a
    later store to the same address, with a store or a load of another address between, where the
    other thread must never see the second address new and the first still old;
  - for ARM64, a load ordered after a conditional branch by `isb`;
  - an aligned 64 bit word that one thread stores two values to in turn with plain stores, while the
    other loads it with plain loads: no load sees a mix of the two (7.6). Its hand-written form
    exists (scratch, run6/tear) and passes today in wasmtime and in Node on both hosts.
  No outcome the guest's architecture forbids is ever seen. This is the test of what 7.6 relies on, run
  on each version of each WebAssembly compiler and each host. The hand-written forms of the second
  group's first three exist (scratch, check4) and fail today as section 14 says. The fourth exists as
  a module (check4/independent/dse.wat) and passes today.
- **The compiler setting is on the Parallel engine.** A test compiles a module with those four shapes
  in each of the executor's two engines and reads the machine code. The Parallel engine's has every
  load and store of all four. The Serial engine's lacks the ones the pass removes today, in the first
  three shapes, and has every store of the fourth. The test fails the day a wasmtime release changes
  any of that, in either engine: a release that began to drop overwritten stores would show here.
- **The runtimes.** The kernel test crate's Go, Java, Node and Python programs
  (crates/wcontainers-kernel-test/tests/go.rs, java.rs, node.rs, python.rs) with threads, in `Parallel`
  under the stress schedule.
- **`Serial` must not get slower, beyond what decision 7 allows.** Each against its figure at the commit
  before the milestone, with machine and load:
  - the Serial form is the same code: the modules the translator makes for the benchmark set and for
    busybox at each look-ahead are compared with the commit before's, byte for byte, and differ only in
    the type of the memory import (from T2 on; in T1 they do not differ at all);
  - the translator's benchmark set in the Serial form, instructions retired by the run alone: within
    the spread between identical runs (under 0.1% in section 14);
  - the system call cost with one thread (the `getppid` loop and the fork loop of docs/runtime.md section
    15, and the kernel's own share as section 18 measured it), on this Mac and on the x86 laptop, in
    three builds at the same load: the commit before; the milestone; and the milestone with the kernel's
    lock replaced by one that does nothing (the lock's kind is a type parameter, 4.2). The no-lock build
    is within the spread between identical runs of the commit before: this checks that nothing but the
    lock was added (4.2). The milestone less the no-lock build is the lock with its holder's word
    (4.2), reported against decision 7's figures. It is the one cost the owner is asked to accept;
  - **threaded programs run `Serial`, from T2 on.** T2 changes how a `Serial` process with several
    threads runs in the executor (a store for each thread, code instantiated in each store at first
    use, another lock scheme: 7.2, 7.5), and nothing above has two threads in it. So, on this Mac and
    on the x86 laptop, at the same load, the milestone against the commit before:
    - two threads of one process that wake each other through a futex in turn: the time of one
      hand-over;
    - a loop that starts a thread which calls into the C library and is joined: the time of one
      round, which holds the new thread's store and its first use of each piece of code;
    - the kernel test crate's Go, Java, Node and Python programs with threads, run `Serial`: the
      instructions retired and the time of each;
    - a process that starts 64 threads which then wait: its resident memory, less the same process
      with one thread, divided by the threads.
    Each is within the spread between identical runs of the commit before. If one is not, the Serial
    form keeps one store for each process (7.2), and the bar is run again.
- **What the Parallel form costs a process with one thread**, measured and reported, not a bar: the
  benchmark set in the Parallel form against the Serial form (expected 0.0% to 1.8% more instructions),
  and a loop of barriers and a loop of 128 bit atomics with the byte zero (expected 0.1 to 0.8 ns a
  round more).
- **Speed-up** is measured, with machine and load, on a real build with parallel jobs and on a threaded
  program in each runtime, at 1, 2, 4, 8 and 16 processors, with the time spent waiting for the kernel's
  lock, the group stops by reason and the groups called in by reason.

## 13. Milestones

Each bar is checkable by the named tests. The cheapest large win is first.

- **T1, groups in parallel, every group `Serial`.** No translated code changes. The config exists with
  `Serial` and with `None` and `Exact`. A config with `Parallel` or `External` is refused where it is
  given (the container's start), with an error that says it is not built yet: that is the embedder's
  call failing, never a guest's. A recorded kernel needs every process at least `External` (section
  3), and `External` is T8's. So in T1 a recorded kernel accepts only `Exact`, and a kernel that is
  not recorded accepts `None` and `Exact`.
  - abi: `Cpu` and `Host` by `&self` and `Sync`; the processor in `Kernel::next`; the thread in
    `Kernel::exit`; the `Host` services `thread_stop` and `kernel_entry_admit`, asked only of a system
    that orders entries, and then about every entry (4.6); the hooks
    `on_processor_needed` and `on_deadline_changed` (5) and
    `on_execution_config_in_force_changing` (3); the stop word as a set of reasons, with `raise_stop`
    as an atomic or (4.5).
  - kernel: the lock with its holder's word, the handle that holds it and the `Send` reason (4.2);
    the check of the calling worker's mark at every way in but the loop's own (4.2);
    processors, masks and the scheduler, with one entry for each thread `next` chooses, a `Serial`
    group putting forward its next thread only, and a thread's woken call and its signals acted on
    only when it is chosen for a processor (4.3); groups with the joining rule, joined only with
    their other running threads called in, and the config in force with its hook (3); the two rules
    of 4.4 item 5, with calling a group in, a mark for each call, waiters chosen whatever is called
    in, and threads ending in their own entry (4.5); a process's
    signal picked again when its thread blocks it or ends, through one function that sets a mask
    (4.5); entries kept aside when told `Later` (4.6); the scheduler's condition in both halves (no
    processor idle with a thread it could run, no ready thread with nobody due in), with its mark and
    its counts, the wake-up comparison aimed at a thread in the woken one's way, and the deadline
    out of the slices (5, 11); charging per thread (11); the told count; `ExecutionConfig` at the
    start, inherited at fork; `kernel.process(id)` at any time for reading (5, if decision 8 is
    ruled so).
  - runtime: `run` and `run_until_finished` with a processor and `StopReason` per worker, one worker
    started for each processor, the system told who, idle without a lost wake, the container stop,
    the executor's stopping handle and its promise in place of `StopFlag`, the kernel's handle
    carried by `Carry::Leave` (4.2), the local system by `&self` with a slice's end per processor and
    one deadline for the container, sent by an idle worker or by the timer thread (5).
  - WebAssembly executor: shareable, with the process's lock kept for a thread's whole stay in
    `Execution::run`, let go before the kernel's handle is dropped, and its try-lock rule (7.2); the
    asked-in bit, taken off only inside an entry (4.5); the worker's mark set around the system's
    calls in the clock hook and `warn` (4.2); a code drop that reaches the caller's group only (7.5);
    counted turns for a thread in force `Exact` and free turns for any other. Stores stay one per
    process: one thread of a group runs at a time.
  - kernel test crate: the recording and replaying test system with its replayer, which ends a
    replay that has left its recording, and its seeded order for one worker made the same way (4.6,
    11).
  - the stand-in stays at one processor.
  - Bar: section 12's "nothing changes with one processor", "groups in parallel", "exact replay", "a
    replay that has left its recording ends" (three of its four changes), "the config in force" for
    the guarantee (its parts marked T1), "groups joined while both run", "two call-ins that want each
    other's groups", "reaching across groups", "no
    request to stop is lost", "no processor idles with a thread it could run" (its first three
    programs), "no thread waits behind its own sibling", "timers and slices", "a process's signal
    finds a thread" (the `Serial` run), "a choice that hands nobody out", "the kernel called from
    inside a hook", "the kernel called from inside guest code" (the `Serial` run), "the executor's
    lock rule", "a worker for each processor", "one worker, several processors, no log" and
    "processors and masks"; its `Serial` measurements, of which only the system call cost can move
    here; and a scaling test: four processes that only compute, on four processors, finish in under
    half the time they take on one (a quarter is the ideal; the margin is for this machine's load and
    is the orchestrator's to set).
  - Measured: a real build with parallel jobs at 1 to 16 processors; the share of time waiting for the
    kernel's lock; groups called in, by reason, and how long a mark stayed on; stops asked for
    nothing; threads asked in for a timer, for a waiting thread or for a waiter; turns that ended for
    nobody; how often the scheduler's condition was walked and how long a walk took; the cost of the
    lock and its holder's word in a system call, by the build without them; the same build with 16
    processors on four and on eight cores, against 16 on 16 where a machine has them.
- **T2, a group's threads in parallel on the WebAssembly executor, ARM64 guests.** After decision 4.
  - translator: the Parallel form (7.2): barriers and the 128 bit hook behind the byte (7.3), the mark
    for hooks that are stop points (4.5); the shared import and its declared maximum in both forms.
  - executor: the two engines, with the compiler setting and a test that it took effect (7.6); the
    host-supplied shared memory, first; one store per thread; group stop with its stop lock, its marks
    and the held bit, held only at a stop point (4.5); frames given up after a page that allowed
    reading and writing lost a permission (4.5); a write from outside into a memory with one thread
    made under a group stop (7.3); the table rules and the shared pages of 7.4, with the one-way
    copy; code dropped and added, with the count of drops (7.5); table growth.
  - kernel: `Parallel` in the config, its changes in force, the executor's limits, telling the executor
    each memory's config, the compare and exchange for the words the kernel changes, `membarrier` with
    its restartable-sequence command, the read of every call that runs again (4.4).
  - Bar: the threaded suite, "the races this plan names", "a process's signal finds a thread" (the
    `Parallel` run), the ordering tests and "the compiler setting" match the reference in `Parallel`
    under the stress schedule at full, none and bounded look-ahead; T1's bars hold; the `Serial`
    measurements of section 12 hold, the byte-for-byte one and the four of threaded programs run
    `Serial` among them.
  - Measured: group stops by reason and their length; threads that gave up their frames, by reason;
    translations thrown away; a store's memory; what the Parallel form costs a process with one
    thread.
- **T3, the runtimes and real threaded programs.** Go, Java, Node and Python with threads, and code
  compiled while other threads run.
  - Bar: section 12's runtimes pass in `Parallel` under the stress schedule.
  - Measured: speed-up of a threaded program in each; group stops for code changes; how often barriers
    (by kind) and the 128 bit forms run, counted by a build of the translator made for this measurement
    only.
- **T4, (`Parallel`, `Exact`) and per-thread watches on the WebAssembly executor** (section 8). After
  the embedder interface's memory watches exist and the translator's count exists in the Parallel form
  (other tracks). With it: a change of the kernel's in a group in force `Exact` is a call-in (4.5).
  - Bar: a test embedder holds each page for one writer or several readers, records every hand-over with
    both threads' positions, and replays a threaded program from that record with the same registers and
    memory at every hand-over and at every kernel entry. The program's threads also fill each other's
    pages through `read` system calls, so the kernel's own accesses are in the record; they write
    objects on each other's stacks (a scoped thread filling its parent's array, a result joined into a
    stack slot), one of them while it runs on a second stack, so the rule for frames is in it too; and
    they `mprotect`, fork and rewrite code while the others run, so every call-in is in it. The
    torn-permission and live-frame tests of section 12 pass with a watch in place of `mprotect`, and
    the eight-thread program of "no processor idles" keeps all eight running. `suspend` in
    `on_memory_accessing` with other threads running answers its error, and works in
    `on_process_called_in` (if decision 8 is ruled so).
  - Measured: the cost of one hand-over; hand-overs per second in the threaded suite and the runtimes;
    frames given up for a watch; call-ins by reason.
- **T5, the native executor** (section 10), in the native track, after its single-thread suites. With it:
  the two forms of native code, with barriers (and x86's `mfence` and lock prefixes) left out of the
  Serial form and copied in the Parallel form; the exclusive's record made safe for threads in
  parallel; each held virtual CPU dropping its own cached translations; this plan's calls on the
  inside half (section 10).
  - Bar: T1's and T2's bars on the native executor, the ordering tests with barriers among them; the
    per-thread root under T4's test; the Serial form of the benchmark set and of busybox holds no
    barrier instruction and is the same code as before the milestone, read from the output; the
    question at an entry of a recorded kernel makes no crossing of its own: over a loop of `getppid`
    the monitor counts no more exits than the log's batches sent out.
- **T6, x86-64 guests**, in the x86 track: the Parallel form for x86, with lock prefixes as atomic
  operations, the hook for a misaligned one and `mfence` as the tested fence (7.3); the executor's
  answer on a weaker host (section 9); frames given up by 4.5's test, by the page, where a code page
  is locked against writes.
  - Bar: the threaded suite and the ordering tests, the sequence lock and the store-back among them,
    for an x86 guest in `Parallel` on the x86 laptop; on this Mac the same suite passes with the group
    kept `Serial`, its counter read by the test; the `Serial` measurements of section 12 for an x86
    guest.
- **T7, splitting the kernel's lock**, only where T1's measurement points (4.7), one part at a time.
  - Bar per part: the single-thread system call cost is not above its figure before; the time waiting for
    locks on T1's build is lower; every earlier bar holds.
- **T8, `External` for `Serial` groups** (4.6), any time after T1; it changes no translated code. The
  digest of an entry, its hook, and the replaying test system's stops without positions. With it a
  recorded kernel accepts every guarantee, and runs a process given `None` in force `External`
  (section 3).
  - Bar: section 12's "`External`", "a kernel is recorded whole" in it, the digest's change in "a
    replay that has left its recording ends", and the recorded kernel's parts of "the config in
    force".
  - Measured: what the question and the digest cost a system call, a small one and one that reads
    many bytes (a large `write`), which is what a process given `None` pays in a recorded kernel; and
    the share of the kernel test crate's programs whose replays are never thrown away.

Not milestones here: the form that orders atomic instructions for (`Parallel`, `External`) (decision 3),
the ordered x86 form (decision 5), a browser, `suspend` and `resume`, which are the embedder
interface's, and section 5's call and hook for acting on a process from outside a hook (decision 8),
which come with them. The group stop that `suspend` needs in a `Parallel` process is T2's. What
`suspend` may hand out (4.2) is asked of the interface track. Fewer workers than processors (known
fault 2) is not a milestone.

## 14. Measurements behind the decisions

On this Mac (Apple M5 Max, 18 cores, macOS 27.2), release builds, 2026-11-18, with other tracks running:
the load (`sysctl vm.loadavg`, 1 minute) was 49 to 162 throughout. On the x86 laptop (Intel Core i7-8750H,
12 threads, Linux 7.3.4) the load was 0.2 to 1.7 for the first runs and 1.6 to 11 for the checkers' and
the corrections', when other tracks were using it too; its cores 2 and 8 were not used. Scratch code and
logs are under pyro-parked/wcontainers-scratch/threads. Details and every command are in
docs/status-threads.md.

- **A shared guest memory against an unshared one, single-threaded** (logs/shared-cost.log). The
  translator's benchmark set (`programs/bench.c`, -O2, full look-ahead, every check), each workload in its
  own process, alternating the two, wasmtime 50.0.1. Instructions retired by the whole process, three
  rounds with one run in each process and two rounds with three runs in each
  (logs/shared-cost-runs3.log). The difference of the two, halved, is one translation, compile and run
  with the process's fixed work (9.3 to 9.4 thousand million instructions) taken out. Shared against
  unshared: fib -0.03%, sieve +0.07%, hash +0.10%, sort +0.07%, interpret -0.11%, matrix -0.09%, walk
  -0.16%, search +0.33%. Two rounds of the same setting differed by up to 0.39% (search, shared).
- **A barrier in a single-threaded run, in three forms** (barrier/bench.wat and its logs). A round is a
  store, the barrier, a load of another word, on a shared 64 bit memory. Nanoseconds a round, best of
  five, for: nothing; `atomic.fence`; the byte tested and zero; the byte tested and set.
  - this Mac, wasmtime 49.0.2's command line program (load 90): 0.98, 1.67, 1.15, 1.74;
  - this Mac, Node 24.14.1 (load 90): 0.46, 6.59, 0.59, 6.91;
  - the laptop, wasmtime 49.0.2: 1.76, 20.34, 2.56, 20.12;
  - the laptop, Node 24.22.3: 1.05, 17.05, 1.23, 17.58.
  So the fence costs 0.7 to 19 ns a barrier and the test 0.12 to 0.8 ns. wasmtime's output on this Mac
  was read: `dmb ish` where the fence is, and a byte load and a branch around it in the tested form.
  wasmtime 50.0.1 was not run for this; its compiler's rule for the fence is cited in 7.3.
- **The store-then-load test, two threads, plain accesses on a shared memory in Node**
  (barrier/litmus.wat). Each thread stores to its own word and loads the other's; "both saw 0" is
  forbidden when a full barrier stands between store and load. Rounds with both 0, of 2,000,000:
  - this Mac, Node 24.14.1 (load 89 to 91): no barrier 371,914; `atomic.fence` 0; an atomic store in
    place of the plain one 0; an atomic exchange 0;
  - the laptop, Node 24.22.3 (load 1.2): no barrier 134,938; the other three 0.
- **The message-passing test, by the plan's checker** (check1/mp.mjs, mp-node.log, mp-node-2.log; this
  Mac, Node 24.14.1, load 99 to 107). Plain accesses on a shared memory, two workers. The flag was seen
  with stale data 84 times in 1,500,000 rounds and 64 times in 2,000,000. With `atomic.fence` between the
  two stores and between the two loads: 0 times in 3,500,000.
- **An uncontended lock** (lock/src/main.rs). A `parking_lot` raw lock taken and let go through
  `lock_api::RawMutex` around a small piece of work, one thread, best of five, nanoseconds a round with
  no lock, one, two:
  - this Mac (load 83 to 96): 4.86, 5.14, 4.98. No cost can be told from the noise;
  - the laptop (load 0.6 to 1.7): 5.51, 12.60, 21.54. So 7 to 9 ns a lock.
- **An uncontended lock, by the plan's second checker** (check2/lock/lock.c and its logs). The lock is
  `parking_lot`'s fast path written out: a compare and swap to take it, one to let it go. Two kinds of
  work around it: one that stays in registers, and one that walks a chain of loads in a 64 KiB table
  and stores once. Best of five, 100,000,000 rounds, nanoseconds a round without the lock and with it:
  - this Mac (load 53 to 71): register work 0.95 and 4.52, so 3.6 ns a lock; memory work 8.42 and 8.41,
    so nothing;
  - the laptop, core 4 (load 5.5 to 5.7): register work 2.67 and 14.04, so 11.4 ns; memory work 16.85
    and 17.49, so 0.6 ns.
  So a lock costs 0 to 3.6 ns on this Mac and 0.6 to 11.4 ns on the laptop, by the work around it. This
  plan's own loop above is of the first kind on the laptop, and on this Mac its work hid the lock.
- **The store-then-load test in wasmtime, by the plan's second checker** (check2/host, sb-mac.log,
  sb-x86.log). Two host threads, each with its own `Store` and `Instance`, on one wasmtime
  `SharedMemory`; wasmtime 50.0.1, as the repository pins it, with `Config::wasm_threads` and
  `Config::shared_memory`. Rounds in which both loads saw 0, of 2,000,000 for each form:
  - this Mac (load 102 to 106): plain accesses 30,199; `atomic.fence` 0; the byte tested and zero
    30,425; the byte tested and set 0; a compare and exchange then a plain load 0; an atomic store then
    an atomic load 0;
  - the laptop, cores 4 and 10 (load 8.8 to 11.3): 143,587; 0; 39,671; 0; 0; 0.
  So the tested byte of 7.3 is correct in wasmtime on both hosts, and a process with one thread, with
  the byte at zero, gets exactly what a `Serial` run gets today.
- **A loop of plain loads sees another thread's store** (logs/spin-wasmtime.log, logs/spin-node.log). One
  thread spins on a plain 64 bit load from a shared memory, another stores to the word after 200 to 300
  ms. wasmtime 50.0.1: seen, in a bare loop, in a loop with the translator's stop check, and with an
  atomic load. Node 24.14.1: seen in all three, with the default compilers, with `--no-liftoff` and with
  `--liftoff-only`.
- **V8's limit on a declared maximum** (same run): Node 24.14.1 refused to compile a module importing a
  shared 64 bit memory with a declared maximum of 2^48 pages; with 262,144 pages it compiled.
- **What wasmtime's compiler does to plain accesses, read in its output** (check4/shapes.wat and its
  .dis files; this Mac, wasmtime 49.0.2 from `nix develop`, load 110 to 113). A shared 64 bit memory.
  - "Load x, load y, load x": two loads, the first used twice (the third check found this too).
  - The same with each load atomic when its address is aligned and plain when not: all three kept.
  - "r = [x]; f = [y]; if f then [x] = r", and the same with no branch: no store in the output at all.
  - The same with the middle load atomic: the store is kept.
  - "[x] = v; f = [y]; [x] = v": one store.
  - Compiled again with `-C cranelift-enable_alias_analysis=false`: the same machine code, line for
    line. The setting does nothing (7.6).
- **Whether that pass drops an overwritten store, by the plan's fourth checker**
  (check4/independent/dse.wat and its two .dis files; this Mac, wasmtime 49.0.2 from `nix develop` of
  this checkout, load 101 to 109). A shared 64 bit memory. "[x] = 1; [y] = 2; [x] = 3" and "[x] = 1;
  f = [y]; [x] = 3", each with x and y in registers and again as constant offsets from one base:
  every store is in the output, ten of ten. The output with
  `-C cranelift-enable_alias_analysis=false` is the same, line for line. The same run merged the
  repeated load of "[p]; [p+8]; [p]" into one, as above. So the pass's fourth thing does not happen
  to guest stores today; the files were read again for this correction and say what the checker
  said.
- **The same, with two threads** (check4/host: a Rust program on wasmtime 50.0.1 as the repository pins
  it, two host threads each with its own `Store` and `Instance` on one `SharedMemory`; check4/host/src/
  tso.wat; logs tso-mac.log, tso-x86.log, tso-x86-long.log). Cranelift was a scratch copy of 0.137.1
  with one change: the optimiser skips the alias analysis when `WC_NO_ALIAS=1`.
  - *Sequence lock.* A writer makes a count odd, writes two words, makes the count even, 20,000,000
    times, all plain stores. A reader loads the count, the two words, the count again, and accepts when
    the count was even and the same. Accepted reads whose two words differed, of all accepted:
    - the laptop, cores 4 and 10 (load 7.7): plain loads 931,346 of 1,670,130; each load atomic when
      aligned 0 of 3,692; atomic loads 0 of 21,038; plain loads with the pass off 0 of 33,327;
    - the laptop again with 200,000,000 writes (load 5.3): 9,288,492 of 15,869,202; 0 of 24,783; 0 of
      98,511; with the pass off 0 of 109,791;
    - this Mac (load 129): 7,784 of 3,311,008; 20 of 1,821,534; 1 of 237,583; with the pass off 82,860
      of 4,204,412. On this host every form fails, because the host reorders the writer's plain stores
      and the reader's plain loads: that is section 9's case.
  - *Store-back.* One thread: r = [x]; a short wait; f = [flag]; if f then [x] = r. The other: [x] = 1;
    a full barrier; [flag] = 1. Rounds where r was 0, f was 1 and x was 1 at the end, of rounds where r
    was 0 and f was 1:
    - this Mac (load 129): plain loads 57,368 of 57,368; each load atomic when aligned 0 of 41,951;
      atomic loads 0 of 120,649; plain loads with the pass off 0 of 61,447;
    - the laptop (load 7.7): 199,975 of 199,975; 0 of 199,625; 0 of 199,882; with the pass off 0 of
      199,977. With 1,000,000 rounds (load 5.3): 999,877 of 999,877, and 0 of 999,730 with the pass off.
- **What that pass is worth to a single-threaded run** (noalias-cost.sh, logs/noalias-cost.log; this
  Mac, load 123 to 142). The translator's benchmark set at full look-ahead, wasmtime 50.0.1 with the
  scratch Cranelift, on a shared guest memory, each workload in its own process, the pass on and off
  alternating, three rounds. Instructions retired by the run alone (`proc_pid_rusage` read before and
  after the run, so compiling is left out), mean of three, in thousand millions, on then off: fib
  81.513 and 81.948 (+0.53%); sieve 13.842 and 13.844 (+0.01%); hash 7.801 and 7.804 (+0.04%); sort
  31.437 and 31.439 (+0.01%); interpret 22.512 and 22.734 (+0.99%); matrix 7.716 and 7.725 (+0.11%);
  walk 16.578 and 16.879 (+1.82%); search 6.777 and 6.778 (+0.00%). Geometric mean +0.44%. Three runs
  of one setting differed by 0.02% to 0.10%. Not measured on an x86 host.
- **Atomic loads in place of plain ones, single-threaded, on an x86 host** (check4/kernels: the kernels
  of ordering-cost with two new forms, loads atomic and stores plain, and the same with each load's
  address tested for alignment first; the laptop, wasmtime 49.0.2, core 4, load 5.2 to 7.0, best of
  seven, forms alternated). Seconds for plain, atomic loads, tested loads: sum of words 1.253, 1.855
  (1.48 times), 2.467 (1.97); pointer chase 1.354, 1.366 (1.01), 1.395 (1.03); byte loops 2.239, 2.170
  (0.97), 2.526 (1.13); calls with a frame in memory 1.225, 1.217 (0.99), 1.533 (1.25); fill and copy
  0.428, 0.438 (1.02), 0.477 (1.11); the copy one byte off alignment 0.435 plain and 0.449 tested
  (1.03). Checksums agreed in every form.
- **The holder's word on an uncontended lock** (run6/owner/src/main.rs and its two logs). A
  `parking_lot` raw lock around a small piece of work, one thread, best of seven, 200,000,000 rounds.
  Four forms: no lock; the lock; the lock with the holder's name read (`parking_lot`'s `RawThreadId`
  through `lock_api::GetThreadId`) and stored after the lock is taken, and cleared before it is let
  go; the same with the name read once, outside the loop. Two kinds of work, as the second checker's:
  one that stays in registers, one that walks a chain of loads and stores once. Nanoseconds a round:
  - this Mac (load 133 to 137): register work 2.02, 5.52, 5.01, 5.92; memory work 8.74, 9.14, 9.17,
    9.11;
  - the laptop, core 4 (load 4.7 to 6.1): register work 1.81, 11.81, 11.57, 12.07; memory work 13.89,
    14.52, 14.76, 14.37.
  So the lock costs 0.4 to 3.5 ns on this Mac and 0.6 to 10.0 ns on the laptop in these runs, as
  before. The word moves a round by -0.5 to +0.4 ns on this Mac and by -0.2 to +0.3 ns on the
  laptop, with both signs in both kinds of work. No cost can be told from the noise.
- **Whether an aligned plain 64 bit access is seen half done** (run6/tear: tear.wat, tear.mjs, host/,
  and the logs and listings beside them). On a shared memory, one thread stores 0 and -1 in turn to
  one word with plain stores, 200,000,000 times, and another loads it with plain loads as often and
  counts the values that are neither. Five runs of each form, so 1,000,000,000 loads. Loads that saw
  a mix, with the word aligned and then laid across a cache line as a control:
  - this Mac, Node 24.14.1 (load 112 to 113): 0; 73,917,899;
  - this Mac, wasmtime 50.0.1 as the repository pins it, two host threads each with its own `Store`
    on one `SharedMemory` (load 113 to 114): 0; 57,220,868;
  - the laptop, Node 24.22.3, cores 5 and 10 (load 5.2 to 6.3): 0; 232,854;
  - the laptop, wasmtime 50.0.1, cores 5 and 10 (load 3.3 to 3.8): 0; 37,550,933.
  In every run the reader saw both values. wasmtime 49.0.2's output for the two loops was read on
  both hosts: the store is one `str x13, [x10, w5, uxtw]` and the load one `ldr x1, [x0, w5, uxtw]`
  on this Mac, and one `movq %rdi, (%rax, %rcx)` and one `movq (%rdi, %rcx), %r9` on the laptop.
  V8's machine code was not read.
- **Two call-ins that want each other's groups, by the plan's sixth checker** (check6/callin_model.py
  and its two logs; Python 3, this Mac, load about 118). A model of 4.3's and 4.5's rules as they were,
  searched breadth-first: process P1 with two threads, P2 with one, all `Serial`, two processors, each
  of two threads making one call that reaches the other group. It reaches a state where nothing can
  move after 4 steps (67 states). With a waiter put forward whatever its own group's mark says, no
  such state exists (106 states).
- **The corrected call-in rules, as a model** (run7/callin/callin_model2.py and callin_model2.log;
  Python 3.15, this Mac, load 117 to 154; each search takes seconds). The rules of 4.3 and 4.5 as
  they are now: a mark for each call, a group called in while any mark names it, a waiter put
  forward whatever is called in but never beside a running sibling of its `Serial` group, back to
  guest code only when its own group is not called in, and a thread in a waiter's way asked in.
  - The sixth check's case: 156 states. With the earlier rule the search finds its two waiters left
    with nothing the kernel forces to move them; with the rule now, none.
  - A larger case: two `Serial` groups of two threads and one `Parallel` group of two; each of four
    threads makes one call, chosen freely among reaching another group, reaching two, joining its
    group to another, and calling its own group in; any thread may come in, end, or be ended while
    it waits. On 1, 2, 3 and 4 processors: 4,998, 55,053, 60,743 and 62,189 states.
  - In every one: no state where nothing can move; the end can be reached from every state; no
    thread in guest code beside a sibling of its `Serial` group; no call's work done while a thread
    of a group it named, or of its own `Serial` group, is in guest code; no thread in guest code
    while its group is called in; no woken call that had to park again for its call-in.
  - Taking only the steps the kernel forces (an asked thread comes in, a waiter is chosen when a
    processor is free, a thread in a waiter's way is asked in): from every state every path ends
    with no call-in pending, in at most 9 steps, and none goes round. With the earlier rule 656 of
    the larger case's states end with a call-in pending, and with no waiting thread ever ended the
    search reaches a state where nothing can move in four steps.
  A model is not the code, and it leaves out masks, weighted time and the merging of two groups
  after a join: section 12's test and its count "call-in with nothing to move it" check the code.
- **A barrier as a hardware instruction, single-threaded** (run7/native-barrier/barrier.c and its two
  logs). A round is a store, the barrier, a load of another word, at user level with no virtual
  machine. Nanoseconds a round, best of seven, 200,000,000 rounds, and what each form adds to a round
  with no barrier:
  - this Mac (Apple clang 21, load 158 to 162): nothing 0.24; `dmb ishld` 1.46 (+1.22); `dmb ishst`
    1.47 (+1.23); `dmb ish` 1.56 (+1.32); `dsb ish` 5.02 (+4.78); `isb` 8.40 (+8.16); an atomic add
    in place of the barrier 1.67 (+1.43);
  - the laptop, core 4 (gcc 9.4, load 2.6): nothing 0.51; `mfence` 17.69 (+17.18); a locked add in
    place of the barrier 6.43 (+5.92).
  The instructions were read in both programs. Inside a virtual machine at the guest's level the
  cost was not measured; nothing traps there, so it is expected to be the same.
- **A per-thread flag tested at every take of an uncontended lock** (run7/flag/src/main.rs and its
  three logs; run6/owner's program with a fifth form: a `thread_local!` flag read and tested before
  the lock is taken, beside the holder's name read and stored). One thread, best of seven,
  200,000,000 rounds. The fifth form against the third, nanoseconds a round:
  - this Mac (load 126 to 143): work in registers 4.76 and 4.89 (+0.13); work that walks memory
    8.47 and 8.38 (-0.10);
  - the laptop, core 4 (load 1.6 to 2.5), three runs: work in registers 11.87 and 11.18, 11.76 and
    11.14, 11.87 and 11.14 (-0.6 to -0.7); work that walks memory 14.11 and 15.10, 14.11 and 15.04,
    13.96 and 14.95 (+0.9 to +1.0).
  So on the laptop the test moves a round by up to 1 ns either way, and the same way each time for
  each kind of work. That is why the check is kept off the system call (4.2).
- **How many virtual CPUs KVM allows one machine** (run7/kvm/maxvcpus.py and its log; the laptop,
  Linux 7.3.4, load 2.7). `KVM_CHECK_EXTENSION` on /dev/kvm: `KVM_CAP_NR_VCPUS` 12,
  `KVM_CAP_MAX_VCPUS` 4096, `KVM_CAP_MAX_VCPU_ID` 16384. KVM on ARM64 was not read.
- **A host-supplied mapping as a wasmtime shared memory, by the plan's fifth checker**
  (check5/hostmem and its run-mac.log; wasmtime 50.0.1 as the repository pins it, a debug build, this
  Mac at load 113 to 127). The engine was set up as the executor's is, with a creator that records
  its calls. A one-memory module defining a shared 64 bit memory of at least 64 KiB and at most
  4080 MiB called the creator exactly once. `Instance::get_shared_memory` gave a memory whose bytes
  are the creator's own mapping. An instance in a second store on another host thread imported it,
  and its atomic store was seen through the host's pointer. A host write was seen by a guest load in
  a third store. `memory.grow` went through the creator's own call. The log was read again for this
  correction and says what the checker said.
- **What it costs for `run` to return at a program start and be called again**, by the interface
  track's measurement and its independent re-run, not this plan's (orchestrator/
  interface-measurements.json, part "run-return"; this Mac, load 56 to 184, a prototype on 77e50ddc).
  `run` already returns at every exec: 20,001 returns for 20,000 execs, with and without a suspend in
  the hook. Suspending in the hook and resuming after the return, against changing the process
  inside the hook, differed by +4 with a spread of 27 instructions in the re-run and +23 with a
  spread of 34 in the first run, of about 6.3 million instructions a program start. A real `suspend`
  and `resume` and the native executor were not measured.
- **The progress count**, by the translator track's measurement, not this plan's
  (orchestrator/progress-count-cost.json): its best form costs the seven quoted benchmarks 16.3% more
  instructions on this Mac and 11.9% on the x86 laptop, and 4.4% more time on the laptop.
- From earlier work, not measured again: atomic accesses against plain ones, and shared against unshared
  memories, in wasmtime 49, Node and Chrome (ordering-cost/result.json); the stop flag's latency in Chrome
  (docs/phase-1-translator.md); the cost of recording page hand-overs in a 2008 system
  (dsm-survey/results.json); what ordering only atomics and kernel entries would miss
  (optimistic-replay-results.json); system call, fork and instantiation costs (docs/runtime.md section 18);
  the native executor's interrupts (native.md section 5.5).

## 15. Not yet measured

- How long processors wait for the kernel's one lock in a real build (T1). This decides whether and where
  the lock is split.
- What T1 adds to a system call end to end. Only the lock alone was measured, and it costs between
  nothing and 11.4 ns by the work around it; its holder's word could not be told from the noise
  (section 14). T1's three builds settle it (section 12).
- What the scheduler's condition costs when it is walked, and how often that is, in a real build and
  in a threaded program run `Serial` (T1). A call answered in place tests one mark (section 5).
- How many turns end for nobody because a slice is worked out from the whole queue (T1, section 11).
- What switching the compiler's alias analysis off costs on an x86 host, and on programs larger than
  the benchmark set (T2). On this Mac it cost the set 0.0% to 1.8%.
- Whether V8 merges or drops plain accesses as wasmtime's compiler does. Not looked at: no browser is
  built (7.6).
- How often real programs run barriers and 128 bit atomics (T3). The cost of each is measured; their
  number is not.
- A host-supplied mapping as a wasmtime shared memory, in the executor and with translated code on it.
  It was run once outside the executor, by the fifth checker, and worked (section 14). T2's first
  task.
- What one store per thread costs in memory, what it costs a threaded program run `Serial`, and a
  second wasmtime engine (T2; the first two are in its bar, section 12).
- Whether V8 can show half of an aligned plain 64 bit access: its machine code was not read. In Node
  on both hosts no load saw one in 1,000,000,000 (section 14).
- How long a group stop takes with several threads running, now that a thread is held only at a stop
  point, and how often real programs cause one (T2, T3). How often a group is called in from outside
  (T1).
- The ordering tests as guest programs through the translator: none was run. What was run are
  hand-written tests (section 14).
- What the stop lock costs the kernel's accesses to a `Parallel` memory, and how many translations a
  runtime that compiles while it runs has thrown away (T2, T3).
- What the digest of `External` costs a system call, and how many real programs replay under it (T8).
- A hand-over through per-thread watches, end to end, and what calling a group in for every change of
  the kernel's costs a (`Parallel`, `Exact`) group (T4).
- An interrupt from one virtual CPU to another inside the native executor's machine (T5).
- A barrier inside the native executor's virtual machine, and `xchg` with memory against two moves in
  the native output's Serial form (T5). At user level on hardware the barriers are measured (section
  14).
- What a container costs when it has more processors, and so more workers, than its host has cores:
  the host then takes a worker off its core while it may hold the kernel's lock or keep a group stop
  waiting. Correct, and measured with T1's build at 16 processors on four and eight cores.
- How long a mark of a call-in stays on in real programs, and how often a thread is asked in for a
  waiter (T1).
- What the question and the digest cost a process given `None` in a recorded kernel (T8).
- `KVM_CAP_MAX_VCPUS` on an ARM64 host: none is at hand.
- An x86 guest's Parallel form on an x86 host through the translator (the x86 track, T6).
- A real `suspend` and `resume` on one kernel, with the records moved out and back, and section 5's
  call for acting on a process from outside a hook: nothing exists to measure. Only the return of
  `run` at a program start was measured, on a prototype (section 14).
- How often a `Parallel` group takes a permission from a data page or unmaps one, which makes each of
  its other running threads give up its frames (300.8 ns each, 4.5), and what the returns through the
  bottom loop cost afterwards (T2, T3).
- What a timer costs when it is sent by the timer thread and not by an idle worker, and how many
  threads a real program's timers bring in (T1). The design brings in one at most for each expiry.
- How often a memory with one thread is stopped for a write from outside (T2).

## 16. Decisions for the owner

Already ruled, and so not asked again: `ThreadOrdering` with two values; `ReplayDeterminism` as its own
field of `ExecutionConfig`; the count only for `Exact`; no setter; hooks handed `Process` by value;
`run` and `StopReason`; where inspection begins; that processes which share memory may be on different
kernels (section 2).

1. **When a process cannot be run as its config says, what happens?** Two things cause it: a process
   that shares memory with it was given a stricter config (settings arrive per process, and groups merge
   at any `mmap`), or the executor cannot run the ordering (7.4; section 9; (`Parallel`, `External`)
   until it is built). Recommended: it runs under the strictest that applies, for each field by itself
   (`Serial` over `Parallel`; `Exact` over `External` over `None`), never under a weaker one.
   `Process::execution_config()` answers what it was given, the config in force is read beside it, and
   each case has a counter and a warning (section 3).
   - Cost: a process told `Parallel` can run `Serial` because of a neighbour, and a process told `None`
     can be made to carry the count, with its code translated again.
   - The other answers: the weakest wins, which takes a promise of exact replay away in the middle of a
     run; or the request is refused, which works for `resume` but not for `mmap`, where the only refusal
     is to fail a call Linux would not fail.
   - **With it, a second question: in a kernel that is being recorded, what does `None` mean?** You
     ruled that with `None` nothing is recorded. That cannot hold for one process beside a recorded
     one. Every process's entries move the one scheduler, the one source of ids and the files all
     share, so a log that leaves one process out cannot be replayed: the replay makes other choices
     and then waits for ever (section 3, the sixth check's case). Recommended: a kernel is recorded
     whole or not at all. Its system says at the start whether it orders entries. If it does not,
     nothing is recorded, whatever the configs say. If it does, every entry is put in order and
     logged, and a process given `None` runs in force `External`, counted and warned.
     - Cost: in a recorded kernel a process given `None` pays the question and the digest at each
       system call (measured in T8) and runs `Serial`, since (`Parallel`, `External`) is not built
       (decision 3). An embedder that wants unrecorded parallel processes beside recorded ones puts
       them in a kernel of their own. Until T8, a recorded kernel accepts only `Exact`.
     - The other answers. Refuse `None` in a recorded kernel, at the start and at `resume`: the same
       rule, with an error in place of a counter, and an embedder that mixes must then name
       `External` itself. Or record only what the scheduler needs of a `None` process (its choices,
       its returns and its charges, with no digest): that saves the digest, and leaves what the
       process writes to pipes and files unchecked, so an `Exact` reader of them could differ with
       nobody told. Or keep the ruling as it stands and accept that a replay of a kernel with a
       `None` process in it may be thrown away or wait for ever: not recommended.
   - With it, one hook: `on_execution_config_in_force_changing(process, config)`, fired before the
     change, when the process is whole and stopped. A process that becomes `Exact` through a
     neighbour has no state before that moment that a replay could start from, so the embedder takes
     its starting point there.
     If it does not, the neighbour that was promised `Exact` replays exactly up to the join and no
     further. Cost: one more hook, which an embedder with one config for all its processes never
     sees fire. Without the hook the strictest rule cannot keep the promise it is there to keep,
     and the answer would have to be to refuse a `resume` or an `mmap` that mixes guarantees.
2. **What is a container given when nobody says?** Recommended: `GuestMachine::processors` 1, fixed for
   the container's life, and (`Serial`, `None`) for its first process; the command line program passes
   the host's count of processors unless told otherwise. Cost: a library user who sets nothing gets no
   speed-up and no recording. The other answer for processors, the host's count, speeds everyone up at
   once and makes a container's behaviour depend on the machine that first made it. `Parallel` as a
   default is not recommended before T3's bar holds.
   - With it, to confirm: a live run has one worker for each processor, whatever the host has, and
     there is no setting for fewer (section 5). A container made with 16 processors and brought to a
     host with 8 cores runs 16 workers on 8 cores. Cost: that host shares its cores among them, which
     is correct and not yet measured. The other answer, fewer workers than processors, needs a design
     this plan does not have (known fault 2); with the earlier default, a thread pinned to a
     processor that no worker served never ran.
3. **How much of `External` is built?** Recommended: the `Serial` half now (T8): entries in order, a
   digest of each, no count, no change to translated code. It replays every process with one thread
   that takes no signal while it computes, which is most of a build, and throws away the rest when they
   differ (4.6). Not the `Parallel` half: ordering every atomic instruction would not replay any of the
   11 programs measured on ARM64, costs 24 to 97 ns an event, and needs a third form of code
   (optimistic-replay-results.json). Until it is built, (`Parallel`, `External`) runs as (`Serial`,
   `External`) by decision 1. Cost of building neither: `External` is refused where it is given.
4. **wasmtime's compiler must leave plain accesses alone in a parallel group, and its own setting for
   that does nothing in the version we pin. Carry a patch?** Without it, a parallel group is wrong for
   ARM64 guests (a store of a value just loaded is dropped) and for x86 guests (repeated loads are
   merged), both measured (7.6). Recommended: send the fix to wasmtime's project, and carry it as a
   patch to the one crate (`cranelift-codegen`, a few lines where the optimiser calls the pass) until
   a release has it; T2 waits for this answer.
   - Cost: one patched crate to carry across wasmtime upgrades until then, and 0.0% to 1.8% more
     instructions for code in the Parallel form.
   - The other answers: no patch, with ARM64's dropped store on the list of written differences and
     x86 guests paying up to 1.97 times on load-heavy loops for atomic loads; or no `Parallel` on the
     WebAssembly executor until wasmtime releases the fix.
5. **Is the ordered form for x86 guests on weaker hosts built?** Recommended: not now. Until then such
   groups run `Serial` on ARM64 hosts and in parallel on x86 hosts. Building it costs one more form of
   code for every x86 program, up to 2.7 times slower single-threaded (section 9), to gain cores for
   threaded x86 programs on ARM hosts.
6. **Three places where a parallel group differs from hardware: accept them?** An exclusive store
   succeeds if the value is the same again, where hardware would fail it, on both executors. On the
   WebAssembly executor a 128 bit atomic is made with the group's other threads stopped, which is exact
   and slow. And an address computed from a loaded value in a way that cannot change it no longer
   orders the two loads (7.3). Recommended: accept all three, on the written list of differences. The
   alternative for the first costs every lock operation of every program.
7. **One cost to a `Serial` run that this plan cannot bring to zero: accept it?** A system call takes
   one uncontended lock. Alone it measured between nothing and 3.6 ns on this Mac and between 0.6 and
   11.4 ns on the x86 laptop, by the work around it, against a kernel share of about 120 ns on this Mac
   (4.2). The lock also writes down who holds it, so that a call into the kernel from inside a hook
   is answered and does not wait for ever; that word could not be told from the noise on either
   machine. What both are inside a real system call is not known until T1 is built. Removing it costs the
   kernel compiled twice into every program, with and without the lock. Everything else is kept off a
   `Serial` run: its translated code is the same code (7.2), on the native executor too, whose Serial
   form emits nothing for the guest's barriers (section 10), and T1's bar checks the rest with a
   build that has no lock (section 12). Recommended: accept it. If you would rather see its real size first,
   T1 can be built and measured before you rule on this one; nothing else in T1 depends on the answer.
8. **What may an embedder do to one process while its threads run?** You ruled two things that
   several workers strain: `kernel.process(id)` is for "between steps", and "a hook fires only when
   every operation on that process is legal". With several workers there is no moment between steps,
   and a hook of a `Parallel` process fires with its other threads running. Recommended, in three
   parts (sections 5 and 6):
   - **Reading, at any time.** `kernel.process(id)` answers a handle whenever it is asked, and `id`,
     `info` and every `ProcessInfo` accessor work on it, with nobody stopped. Cost: the handle holds
     the kernel's lock while it lives, as a hook does. Asked from inside a hook it answers an error,
     as you ruled; any other call into the kernel from inside a hook panics and names the call. So
     does any call into the kernel from the three methods the executor calls from inside guest code
     (`ran`, the clock's answer, `cpu_warning`): their doc comments say they may not call the kernel
     or wait for anyone who does (4.2).
   - **Who "any thread" is.** In a host program, any host thread. On the native executor the kernel
     and your system run inside the virtual machine, so it is your system's code there; a program on
     the host asks through the link and gets a copy, as native.md's decision 11 recommends (section
     10).
   - **Acting from outside a hook, through one call.** Any other operation on that handle works when
     nobody needs calling in for it, and answers an error otherwise. For those cases any such thread
     may call `Kernel::process_call_in(id)`; the kernel stops that process's group and announces
     `on_process_called_in(process)`, where every operation is legal. Cost: one call and one hook on
     the interface; each use stops that group's running threads for a moment, and nobody when the
     process is only waiting.
   - **Your guarantee for hooks is narrowed in one case.** In a (`Parallel`, `Exact`) process whose
     other threads run, `suspend` and a change of config answer an error inside
     `on_thread_starting`, `on_process_outgrown` and `on_memory_accessing` (and `on_process_starting`
     for a child that shares its parent's memory); the embedder asks with `process_call_in` and does
     it there. Every other operation stays legal in every hook, and both stay legal in every hook of
     every other kind of process.
   - The other answers. For the third part: the kernel calls the group in before each of those
     hooks, which keeps your guarantee whole and makes every watched access stop every thread of the
     group, where the plan stops one (section 8). For the first two: only `kernel.process(id)` while
     no worker is inside `run`, which adds nothing, lets an embedder read nothing about a running
     process, and means every page that arrives from another machine for a waiting thread stops
     every processor of the container; or a narrow call that only writes a watched range, lifts its
     watch and wakes the waiter, which is cheaper in a `Parallel` group (nobody is stopped) but
     cannot serve a `Serial` group without stopping its running thread anyway (7.2), and is a second
     path for one case.
9. **The interface changes: accept them, so that splitting the kernel's lock later changes none?**
   Recommended: yes. Cost: every `System` (the shipped one, the tests', your own) keeps what it changes
   in atomics or behind its own locks, and gains the arguments below.
   - The embedding project asked for the first of these itself: hooks and services by `&self`. With
     it comes `System: Sync` from T1, also for a system driven by one host thread; the other answer
     lets such a system keep `Cell` and `RefCell` until its second worker (section 5). Its other
     proposal, state kept per process, is not needed by this plan and not built on. If you rule it
     in, it takes off an embedder the locks around what it keeps about one process, and this plan
     asks one thing of it: the type is `Send` (section 5).
   - **On the native executor every call below is on the inside half** (section 10): the kernel
     calls them as plain functions inside the virtual machine, and none crosses to the host at an
     entry. So the promise holds there too. A system that records keeps its log inside and sends it
     out in batches.
   - In T1. `Kernel`: entry points take `&self`; `next` takes a processor; `exit` takes the thread and
     says whether it took effect; the handle that holds the lock across a thread's `exit` and `next`,
     which `arrived` is handed in the kernel's place, and which `Carry::Leave` hands the executor so
     that it lets its own lock go first (4.2). `Start` carries the processors, the first
     process's `ExecutionConfig`, and whether the system orders entries. `Cpu` and `Host`: methods take
     `&self`; both are `Sync`. `Host` gains two services, `thread_stop(thread)` and
     `kernel_entry_admit(entry)`, and three hooks, `on_processor_needed(processor)`,
     `on_deadline_changed(deadline)` and `on_execution_config_in_force_changing(process, config)`
     (decision 1). The kernel's deadline is no longer in a thread's slice: the
     system hands in `Event::Timer` at the deadline it was last told, whether or not a thread runs.
     The build that makes the kernel gives it a lock and a way to name the calling host thread
     (`lock_api`'s `RawMutex` and `GetThreadId`).
     `System`: methods take `&self`; `turn` is told the processor, the thread and its guarantee in
     force, `ran` the thread, `arrived` and `idle` the processor; `idle` also returns when its
     processor is needed; `stopped` runs once, with every worker out of `Execution::run`.
     `Execution::run` takes `&self`. `StopFlag` and `Executor::stop_flag` go; the executor gives a
     handle that stops a named thread, and promises that a stop asked for is never lost. `run` and
     `run_until_finished` take a processor, and `run_until_finished` starts a worker for each. This
     also answers how a running container is interrupted from another thread: every worker's `run`
     returns `StopReason::Interrupted`.
     `kernel.process(id)` answers at any time (decision 8).
   - In T2. `Cpu` gains `set_execution_config(memory, thread_ordering, replay_determinism,
     several_threads)`, `parallel_limits()`, `compare_exchange(memory, address, expected, new)` and a
     call for `membarrier`'s group stop.
   - In T8. The hook `on_kernel_entry_ended(thread, digest)`.
   - With the embedder interface's own work, two things this plan asks of it. The thread of
     `on_memory_accessing` becomes optional, none meaning "the kernel, for another process", so that a
     `write` to a file a watched process maps is seen; without it an embedder that composes exact
     replay from watches misses those writes (section 8). And a `SuspendedProcess` shares no `Rc`
     with what stays in the kernel, so that it may be held, moved and dropped on any host thread
     (4.2).
   - Taken off this list: `Process::replace_execution_config`. It was there to save a change of
     config the return of `run`. That return was measured as nothing at a program start, and the
     plan needs the call for nothing else (section 6).
10. **Names.** Suggested, all yours: "group" for threads that share memory; "config in force", with
    `ProcessInfo::execution_config_in_force()`; "recorded kernel" for one whose system orders
    entries; "called in", "waiter", "group stop", "held", "stop point" and "stop lock"; "the Serial
    form" and "the Parallel form" of code; `processors` and `ProcessorId`; the services `thread_stop`
    and `kernel_entry_admit`, with `KernelEntry` and `EntryAdmission { Now, Later }`; `Start`'s field
    `kernel_entry_admission` with `Unasked` and `Asked`; the hooks `on_processor_needed`,
    `on_deadline_changed`, `on_kernel_entry_ended`, `on_process_called_in` and
    `on_execution_config_in_force_changing`; "in the way" and "due in" for the scheduler's condition,
    and a `Serial` group's "next thread"; "gives up its frames" for a thread that starts again from the
    bottom loop; `Kernel::enter` and `KernelEntered`, and `Carry::Leave(why, entered)`;
    `Kernel::process_call_in`; `Executor::thread_stopper`; the stop word's bits `STOP_ASKED`,
    `STOP_HELD` and `STOP_OWN`; `HookType::stop_point`; `runs_in_parallel` for the byte;
    `Cpu::set_execution_config`, `Cpu::compare_exchange` and `Cpu::parallel_limits`; the counters' names
    in sections 3 to 12; the milestones' letter T. One more: the container's call is now `run`, and the
    executor's own call that runs one guest thread is `Execution::run` (rt/lib.rs:131). Two calls named
    `run` one layer apart will be misread; suggested `Execution::run_thread`.
