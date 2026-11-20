# wcontainers: guest threads in parallel

Status: plan, revised 2026-11-19 to the owner's rulings of that day and to the independent check of
the one scheduling rule, then corrected twice on 2026-11-20: for the faults that the first independent
check of that revision found (orchestrator/overnight-2026-11-20/threads-check-the-revision.md, "the
revision's check" here), and for those its second check found (threads-check-round-2.md in the same
folder, "the second check" here). **This revision has had no passing independent check. Two checks
were made and neither passed. Until one is recorded in docs/status-threads.md, every part the revision
changed, and every correction of 2026-11-20, is unchecked.** The form before it had been corrected
after its sixth independent check (docs/status-threads.md says what changed each time). Nothing here is
built. The owner rules on section 16 first.

Three things in the corrections were decided by the orchestrator after the checks. They are
**proposed, to be confirmed by the owner**, and not ruled (decisions 14 and 19):
- **a task taken out of its kernel while it waits is treated exactly as Linux treats a signal with no
  handler arriving during that call**: a call that has had no effect yet runs again at resume (a futex
  wait compares its word again); a wait with a relative time limit goes on with the time left; a call
  that has already had an effect is not run again and returns what it did (a partial count, a wake's
  count); a waiter already chosen by a wake returns woken. This replaces the first correction's "its
  call runs from the start" (4.5, section 6, decision 19);
- **operations on one lock that span machines have one ordering point for each lock**: one kernel, or
  one sequencer on the embedder's side, decides the order of the wakes, the requeues and the changes
  the kernel makes to the number at that place. The first correction's "local part first, remote part
  later" is withdrawn: it lets glibc 2.17's condition variable lose a wake even when the waker waits,
  and makes a requeue two steps. How the point is chosen, and what it costs, is open for the owner;
  4.5 and decision 14 describe the options and recommend none;
- **nobody waits inside a hook for another machine** (as in the first correction; 4.4 item 6, 4.5).

The rulings are in the interface notes, from "Ruled by the owner, 2026-11-19: where replay is
recorded" to the end. The check is orchestrator/threads-mixed-ordering/REPORT.md, called "the
mixed-ordering check" here. What they changed, in short:
- **Replay is per process, at its own system calls.** The order of kernel entries recorded and replayed,
  the "recorded kernel", its services `kernel_entry_admit` and `on_kernel_entry_ended`, and the rule
  that a kernel is recorded whole are withdrawn (4.6). A process given `None` costs nothing and is never
  made to carry a neighbour's guarantee.
- **One scheduling rule, kept inside the kernel, with nothing on the interface**: a task whose code is
  `Serial` is given a processor only while no task that overlaps its memory has one. With it the plan
  drops groups in force, the config in force, "the strictest wins" and the hook
  `on_execution_config_in_force_changing`, as the proposed section before the ruling says (section 3).
- **Suspend and resume are by task**, with a process context and a `Kernel` trait. They replace
  `Process::suspend(self)` and `SuspendedProcess<E>` (the ruling says so). This plan also drops its own
  `Kernel::process_call_in` and hook `on_process_called_in`, because in its design `suspend_tasks` does
  that job (sections 5 and 6).
- **New hooks the plan now builds on**: the system call proxy, the hook for the kernel's writes, the
  signal hook, and the shared-memory hooks with `MemoryObject` (sections 2, 3, 4.6). Locks across
  machines (`wake_waiters`, `move_waiters`) are agreed in substance, not ruled (4.5).

## Known faults, to settle when T1 is built

These are open. None is hidden further down.

1. **This revision is unchecked, and the scheduling rule's protocol is not modelled.** The model of
   call-ins that section 14 reports (62,189 states) was of the rules for groups. It must be redone for
   the rule of section 3 as the mixed-ordering check corrects it: exclusion by "handed out", readers
   called in and never held, a `Serial` task that is due a turn stopping reader hand-outs, a reader's
   waiter chosen only while no `Serial` task that overlaps it is handed out, and the kernel's rules
   following the phase. Until it is, the rule is sound by the check's reading and by nothing else.
   The corrections of 2026-11-20 add to what the model must cover: a new overlap that calls in the
   caller's own siblings and marks the caller's own memory (section 3); a resume that makes an overlap
   and puts a mark of its own on the other side without parking (section 3); a batch taken from outside
   a hook, which marks named tasks and no memory (section 6); a task taken out while its call waits
   for a call-in, whose marks then come off (4.5). The second check's corrections add: a change in how
   a memory runs made only with none of its other threads handed out, and with the thread whose entry
   makes it leaving `Execution::run` (section 3); an overlap that ends, which keeps its memories
   running as `Serial` until then (section 3); a batch's mark on every named task, handed out or not
   (section 6). Also unchecked: that "due a turn", which narrows
   the mixed-ordering check's "while a writer is pending", leaves no `Serial` task waiting for ever;
   that rests on weighted time alone (section 3, rule 3; decision 15).
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
9. **Tasks that must take turns may be placed on two kernels** (the revision's check, its fourth
   finding). The rule counts only the tasks one kernel holds. Two tasks of one `Serial` process moved
   to two kernels, or a `Serial` task and an overlapper on two kernels, would run `Serial` code beside
   each other: the `Serial` form makes `lock xadd` a plain read and a plain write, and watches, which
   act per page, cannot make the two one step. Section 3 states what placement must keep; which way it
   is kept is the owner's (decision 20).
10. **A process can come to overlap with no hook naming it** (the revision's check, its ninth finding).
    The shared-memory hooks fire per process at its own mapping, and only for sources some process
    can write through a mapping. A process that mapped a file before any mapping could write it is not
    named when another process later maps the file shared and writable (section 3, decision 21).
11. **Locks across machines are not designed** (the second check, its second, fifth and twelfth
    findings). Their order needs one ordering point for each lock, and how that point is chosen is open
    (4.5, decision 14). Until it is chosen, three cases have no answer in this plan: a wake or requeue
    that touches two words whose points differ; the embedder's index of where waiters are, after a
    requeue made on the waiters' own kernel, which no hook or call reports task by task; and the wake
    a thread's end makes, which has no call to park and so goes without waiting, under conditions this
    plan has not designed. Nothing here is built across machines until decision 14 is ruled.
12. **Under `CLONE_VM`, an owner given `None` still pays while its `Exact` co-owner's threads come in**
    (the second check, its sixth finding). A call-in's mark names a memory, and a marked memory hands
    none of its threads back to guest code, the `None` owner's included. The plan's way to keep that
    owner free (a mark per owner) is a suggestion, not built or checked; the owner decides whether the
    ruling asks for it (decision 22).
13. **A resume's pending overlap is worked out once** (the second check, its seventh finding). The pairs
    a resume would break are found in the resume's entry; a mapping made before the overlap takes
    effect can add pairs that nobody calls in. Section 3 says what the plan suggests; it is unchecked,
    and that it ends is not argued.

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
- **called in**: the kernel asks for a running thread with `stop_thread`, and the thread comes back by
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
   Translated code does not change. A process given `Exact` keeps what the kernel does for it (its
   count and its stops at positions); replay itself arrives with T8, once the two kernel primitives
   exist (known fault 2). A process not given `Exact` pays nothing for it, whatever its neighbours
   asked for.
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
  hard rule. The kernel marks the pages a process shared watched when it leaves. Replay of processes
  that share memory across kernels is the embedder's concern (interface notes lines 158-162). (The
  kernel's report of who shares memory is replaced by the shared-memory hooks, below; whether the
  marking on leaving still holds when single tasks leave is the interface track's to say.) Since
  2026-11-19 one task may also be on another kernel than its siblings (below), within what section 3
  says placement must keep.
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

From the owner on 2026-11-19, from "Ruled by the owner, 2026-11-19: where replay is recorded" to the
end of the interface notes, and one earlier section of the same day. Each heading is quoted where the
plan relies on it. A thing is ruled here only where its section's heading says "Ruled" or "RULED";
three items below come from sections that do not, and each says so.
- **Per-process state** ("Ruled by the owner, 2026-11-19: per-process state", which sits above the range
  named): `type ProcessState: Clone + Send` on the embedder's trait. The kernel keeps one value with
  each process and hands it over with the process, mutable and with no lock. At fork the child gets a
  clone of the parent's. It stays with a suspended process. It is not part of an exported process: the
  importing side supplies its own. The empty type costs nothing. "Not to be built until the interface
  is rewritten in the reviewable-rust layout."
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
- **Optimistic replay stays** (the owner's word, recorded under "2026-11-19 17:30: optimistic replay
  STAYS (owner), and my recommendation to drop it was wrong"; that heading does not say "Ruled"):
  "`External` stays in the interface", as a mode chosen per program, and "Re-measurement running". The
  independent checks it cites found that the prototype's replayer invented failures and that its
  comparison was stricter than the owner's definition. So this plan no longer leans on the prototype's
  figures, as its earlier decision 3 did, until the new ones are in.
- **One scheduling rule, inside the kernel** ("RULED by the owner, 2026-11-19 evening: shared memory is
  announced per process (threads plan decision 1)"): "The kernel keeps the one scheduling rule (a
  Serial task has a processor only while no task overlapping its memory has one) with nothing on the
  interface; the embedder chooses orderings and builds its own view from these hooks plus the process
  and thread hooks." The section before it ("2026-11-19 evening: threads plan decision 1 reworked with
  the owner (PROPOSED unless marked ruled)") says that the rule replaces groups in force, the config in
  force and "the strictest wins"; that is proposed there, and this plan follows it because the ruled
  rule leaves no place for them. The replay guarantee never reaches a neighbour by "where replay is
  recorded" ("a process with ReplayDeterminism::None costs nothing and breaks no neighbour") (section
  3).
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
- **Locks across machines** ("2026-11-19 late night: locks across machines (discussed with the owner; the
  two calls agreed in substance, details open)"; agreed in substance, not ruled): on the process handle,
  `wake_waiters(address, count, bitset)` and `move_waiters(from, to, count)`, and "the count a wake
  returns must be exact" (4.5). The ordering point for each lock is the orchestrator's proposal, not
  part of what was agreed (top of this plan, decision 14).

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

The owner's rule (ruled in "RULED by the owner, 2026-11-19 evening: shared memory is announced per
process (threads plan decision 1)"; first written in the proposed section "threads plan decision 1
reworked"): **a task whose code is `Serial` is given a processor only while no task that overlaps its
memory has one. `Parallel` tasks may run beside their overlappers.** It is kept inside the kernel, with
nothing on the interface. With it this plan drops its groups in force, the config in force and "the
strictest wins", and with them the hook `on_execution_config_in_force_changing`: no config changes
inside a kernel. The embedder builds its view of who overlaps from the shared-memory hooks, which fire
per process at that process's own mapping; known fault 10 is a case they miss. The rule below is the
one the mixed-ordering check found sound once corrected, with its corrections in. Its protocol has not
been modelled (known fault 1).

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
entries fire `on_shared_memory_mapping` and `on_shared_memory_unmapped` for the process whose mapping
it is, which is how the embedder builds its own view. That view can miss an overlap (the revision's
check): process P1 maps a file privately and read-only, and no hook fires, because no mapping can
write the file yet; later P2 maps it shared and writable, and the hook fires for P2 only. P1 now
overlaps P2 (a private mapping shows the file's live bytes, above), and no hook ever named P1. The
kernel's rule is not affected, because the kernel knows every mapping. The embedder's view is. The
plan's suggestion, not ruled: when a source first becomes writable through a mapping, the same entry
fires `on_shared_memory_mapping` for every process that already shows it (decision 21).

**A new memory has one task.** A memory is born at the container's start, at a fork whose child shows no
block that is writable through a mapping, and at exec, which makes a new memory (kernel/loader.rs:292)
after ending the process's other threads (kernel/task.rs:648). For a task that overlaps nobody,
`Serial` and `Parallel` are the same run. The ordering starts to matter at the second task that
overlaps it.

**The rule ends at the kernel's edge, so placement must keep it.** Processes, and since "the embedder-side
view `WorkloadPlacement`" single tasks, may be on another kernel than those they share memory with
(section 2). The rule counts the tasks this kernel holds. What crosses to a task elsewhere is the
embedder's, through the shared-memory hooks and watches. That is enough only between tasks that may
run at once anyway: tasks that run as `Parallel` and have no overlapper that runs as `Serial`.
- **Why not more** (the revision's check, its fourth finding). S1 and S2 are threads of one `Serial`
  process. The embedder moves S2 to kernel K2 (`copy_process_context`, then `suspend_tasks` and
  `resume_tasks`), and each kernel hands its task out: each counts one task of the memory. The `Serial`
  form makes `lock xadd` a plain read and a plain write (section 10), and watches act per page at a
  stop point, so they cannot make the read and the write one step: on the native executor every
  instruction boundary is a stop point, and the page can be taken between them. That is the
  mixed-ordering check's first case again, across machines. The same holds for a `Serial` task on one
  kernel and its overlapper on another.
- **So a task that runs as `Serial` takes turns with its overlappers wherever they are.** Either they
  are kept on one kernel, or the embedder keeps them taking turns across kernels. Which of the two, or
  both, is the owner's (decision 20); this plan does not choose it. Until it is chosen, the plan asks
  the embedder to move a task apart from its siblings only when the task runs as `Parallel` and has no
  overlapper that runs as `Serial`, and a kernel checks nothing of it.

**What runs as `Serial`.** The rule names a task's code. A task runs as `Serial`:
- **when its code is `Serial` code.** A memory has one form of code (7.2). It is `Serial` code when any
  process that owns the memory was given `Serial`. Processes that own one memory (`CLONE_VM`) therefore
  take turns when any of them was given `Serial` (the mixed-ordering check: a second form on one
  memory is not possible). Counted "Parallel process runs Serial code: it owns a memory with a Serial
  one", warned once per process. This is the one place where a neighbour's ordering reaches a process,
  and it is meant to be the ordering only. The plan's design is that a guarantee never reaches a
  process, under `CLONE_VM` either: an owner given `None` beside one given `Exact` is not asked in for
  the other's sake (4.5, "which primitive"), and its code holds no count (7.2, the cache key; not built
  or checked). It does not yet keep that whole: while the `Exact` owner's threads come in, the call's
  mark is on the memory, and the `None` owner's threads that come in meanwhile are not handed back
  until it comes off (known fault 12, decision 22). The form before the first correction called in
  every thread of a memory that any owner given `Exact` shared, which the revision's check found
  against the ruling;
- **when the executor cannot run it beside others**, which the executor says of itself
  (`Cpu::parallel_limits()`, 7.4): on the WebAssembly executor a writable block shown in two places
  of one memory, or in two memories that both have `Parallel` code (7.4); on that executor too, a
  `Serial` task beside `Parallel` ones that share a block with it, until the copy at each switch is
  built (7.4); an x86 guest on a host that orders memory more weakly (section 9); and (`Parallel`,
  `External`) until its form of code is built (4.6, decision 3). Each is counted and warned once per
  process.
This is a fact about one task, worked out from the configs of the processes that own its memory, its
overlaps and the executor, inside the entry that changes it. It is not a config of anything, and no
hook announces it. (The mixed-ordering check says "the ordering half of the config in force stays" for
these limits. The ruled rule leaves no place for a config in force. The plan keeps the limits as this
per-task fact and keeps nothing else of it; decision 1 asks the owner to confirm.)

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
   - **How the handed-out overlappers then come in.** The mark only stops new hand-outs. An overlapper
     that holds a fair slice comes in at its slice's end, or sooner for any other reason, and is not
     handed out again. An overlapper that is not due in (it was handed out with no slice) is asked in
     with `stop_thread` by section 5's condition, as soon as no overlapper in S's way is due in. So S
     waits at most for the longest slice among its handed-out overlappers, and calls nobody in early
     who has a slice. This is the one description of it; section 5, section 11 and decision 15 say the
     same.
   - Basing "due" on the least weighted time is how fairness between S and its overlappers follows the
     kernel's fair scheduler. That is this plan's choice, not ruled (decision 15). It narrows the
     mixed-ordering check's correction, which marks the overlappers "while a writer is pending": here
     they are marked only once S is due. That S then always comes to be due rests on weighted time
     alone (the ready overlappers that run ahead of it gain weighted time, and S's does not move while
     it waits); that is the fair scheduler's own argument and has not been checked for overlaps
     (known fault 1).
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
embedder chooses by the orderings it gives, and nothing else is needed. That is the mixed-ordering
check's finding, summed up in the proposed section "2026-11-19 evening: threads plan decision 1 reworked
with the owner (PROPOSED unless marked ruled)" as "the three ways to run a mixed pair are exactly the
three combinations of orderings the embedder gives". It is not a ruling: the ruled section after it
says the owner has "NOT yet read" the check or its consequences, "three combinations each win
somewhere" among them.
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

**What it costs.** Measured, each by earlier work or by this plan, with where it is and its load (all
are listed in section 14):
- leaving `Execution::run` and coming back, 300.8 ns a call, this Mac (docs/runtime.md sections 10 and
  18; load 39.78, 33.56 and 28.45 over 1, 5 and 15 minutes);
- noticing a stop, 230 ns on average and 30 µs at worst, in Chrome (docs/phase-1-translator.md, "The
  stop check"; no load given there);
- an interrupt from the host into the machine, 2.4 to 4.1 µs, this Mac (native.md section 5.5, from
  docs/status-native.md; no load given in native.md);
- the kernel's share of `getppid`, 116.3 to 120.0 ns, this Mac (docs/runtime.md section 18; load 64.8,
  45.7 and 34.1);
- the kernel's lock alone, 0 to 3.6 ns on this Mac (load 53 to 71) and 0.6 to 11.4 ns on the x86 laptop
  (load 5.5 to 5.7), by the work around it (section 14).
Expected, not measured:
- each block and wake of A costs N call-ins and N hand-outs for B's N handed-out threads, each a kernel
  entry on the one lock, while every other entry waits (4.4 item 6). A's wake waits for the slowest of
  B's threads to reach a stop check, a thread that is compiling included. Both `Serial` pays one thread
  out and one in;
- the interrupt from one virtual CPU to another: not measured;
- on the WebAssembly executor, a copy of the dirty shared pages at every switch;
- nothing on a system call answered in place: it pays only the lock, so "`Serial` must not get slower"
  holds;
- no gain when B is (`Parallel`, `External`): it runs as `Serial` until its form is built.

**How a new overlap takes effect.** A mapping (`mmap`, `shmat`), an exec whose program files are mapped
so, a fork whose child inherits a mapping that may write, or a resume call can make two memories
overlap. It can also change how tasks run: a side may run as `Serial`, and on the WebAssembly executor a
memory with `Parallel` code may come to run as `Serial` itself, its own threads included (an executor
limit, above). The kernel works out the state after the change and every pair of tasks that would
then break rule 1: two tasks that overlap, at least one of them running as `Serial`, both handed out.
The caller counts as handed out, because it goes back to guest code when its call returns, and so do
the tasks a resume brings in. Only pairs the change makes can arise; any other would break rule 1
already.
- **Who is called in.** From each such pair, the task that is not the caller or a resumed task; where
  neither is, every one of the two that runs as `Serial`. So the other side's handed-out tasks are
  called in where the caller's side runs as `Serial`, the other side's `Serial` tasks where only they
  do, and **the caller's own handed-out siblings where its own memory comes to run as `Serial`**. The
  call names each of those memories, its own included, and puts its mark on each (4.5). This is the
  one statement of who is called in for a new overlap; 4.5, 7.2's lock rules and T1 say the same.
- **Until they are in, nothing of the change has taken effect.** The `mmap` or `shmat` that makes the
  mapping (kernel/syscall/memory.rs:109, ipc.rs:356), the exec, or the fork, is parked before it has
  had any effect, and is made in the first entry in which none of those pairs is handed out. The caller
  is then a waiter of a memory that may be marked, its own, and is chosen as any waiter is (4.5).
  Where its own memory comes to run as `Serial`, it leaves `Execution::run` when its call has been
  made (below, "a change in how a memory runs").
- **A resume call cannot park**, and need not ("Resume stays outside hooks": it is called by the
  embedder, between entries). `resume_process` brings in a memory with its mappings, and
  `resume_tasks` brings in tasks to a memory already here; either can make the overlap (the second
  check: the first correction said this only of a resume of tasks). The entry of either puts in what
  it brings but does not put any resumed task forward, and records the overlap as pending: it changes
  neither the overlaps the rule counts nor how any task runs. It puts a mark of its own on the memory
  of each task it calls in, asks them in with `stop_thread`, and returns. The overlap, and any change
  in how tasks run, takes effect in the entry in which the last of them has come in; the mark comes
  off there, and the resumed tasks may then be put forward. The mark belongs to the pending overlap as
  a call's belongs to its call (4.5).
  - **A known gap** (the second check, its seventh finding; known fault 13). The pairs are worked out
    in the resume's entry only. If another memory maps the block before the overlap takes effect, on
    the WebAssembly executor its tasks may come to run as `Serial` with nobody calling them in. The
    plan's suggestion, unchecked: the entry where the overlap would take effect works the pairs out
    again from the state then, with every overlap pending included, asks in the tasks of any new pair
    under the same mark, and lets the overlap take effect only when a working-out finds no pair handed
    out. That this ends while mappings keep being made is not argued.
- `mremap` and `mprotect` change a mapping inside one memory and make no overlap. A fork's child is new
  and has no running thread; the fork is here only for what it does to its parent's way of running.
- **Why the caller's siblings.** The revision's check: B is `Parallel` with four threads, all handed
  out on the WebAssembly executor. Its thread b1 calls `mmap(MAP_SHARED)` on a file that A (`Serial`)
  or C (`Parallel`) maps. B now runs as `Serial` (7.4: the copy at each switch is not built, and two
  memories with `Parallel` code that show a writable block run as `Serial`). The form before called in
  only A or C. b2 to b4 stayed handed out, and when b1's call ran again and b1 was handed out, four
  tasks of one memory that runs as `Serial` were in guest code at once. Now b2 to b4 are called in with
  A's or C's tasks, b1's call runs only when none is handed out, and B's tasks then take turns.
- **Why at all.** The form before's fifth check: process P1 runs on one processor with a file mapped
  privately; P2, on another, maps the same file shared and writable; both carry on, and two tasks that
  run as `Serial` and overlap are in guest code at once. That breaks 4.4 items 3 and 7 and the
  executor's lock rule (7.2).
- Where every task concerned runs as `Parallel` before and after, nobody is stopped. On the native
  executor, b1's `mmap` beside A calls in A alone: B keeps `Parallel` code and runs as `Parallel`
  beside its siblings.
Counted "tasks called in for a new overlap".

**A change in how a memory runs, either way.** A thread that runs as `Serial` keeps its process's lock
for its whole stay in `Execution::run`, and one that runs as `Parallel` takes it only in hooks (7.2).
So a thread must never carry on in guest code across a change in how its memory runs. Two rules, the
plan's own (the second check, its first finding and the related one):
- **The thread whose entry makes the change leaves `Execution::run`** in that entry (`Carry::Leave`,
  4.2), and is handed out again under the new way, taking or not taking the lock as that way says.
  This is the caller of a new overlap whose own memory comes to run as `Serial` (it had no lock, and
  would otherwise run `Serial` code without it), and the caller of an `munmap`, exit or exec that ends
  an overlap and lets its own memory run as `Parallel` (it would otherwise keep the lock while its
  siblings are handed out and wait for it in their hooks). Cost: one leave of `Execution::run` and
  coming back, 300.8 ns measured for that alone (above), at each such change.
- **An overlap that ends changes how a memory runs only in an entry where none of that memory's
  threads is handed out other than the one whose entry it is.** Until then the memory goes on running
  as `Serial`, which is never wrong, only slower: the overlap is gone from the rule's counting, and the
  executor limit that made the memory run as `Serial` is kept for it until that entry. A memory that
  runs as `Serial` has at most one thread handed out (rule 1), so the change is made at that thread's
  next entry, by the rule above. The kernel does not ask it in for this; a thread that computes alone
  with nothing ready in its way keeps the memory as `Serial` for as long as it computes, which costs
  nothing, because nothing of it waits.
- Why (the second check, its first finding). The first correction said "an overlap that ends stops
  nobody". On the WebAssembly executor B ran as `Serial` (an executor limit) with b1 handed out and
  holding B's lock for its whole stay. The overlap ended, B ran as `Parallel` at once, and B's other
  threads were handed out; their hooks waited on b1's lock, and a `Cpu` call on B's memory from
  another worker's entry either failed the try-lock of 7.2 and panicked, or waited for B's lock while
  b1 waited for the kernel's at its next system call. 7.3 already said the change is made only with
  the memory's other threads stopped; this now agrees with it.

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
embedder takes a snapshot there, as for signals. And where it places tasks: a task that runs as
`Serial` takes turns with its overlappers wherever they are (above, decision 20).

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
`Host` methods take `&self`, and both traits require `Sync`. The lock is inside the kernel. So when the
lock is later split (4.7), nothing outside the kernel changes.

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
memory with `Parallel` code its thread still counts as in guest code (its mark is on, 4.5).
- What goes wrong. Thread X is in the clock hook. Its system's answer calls `kernel.process(id)` and
  waits for the kernel's lock. Worker Y has that lock for an `mprotect` in the same process, and its
  group stop waits until every mark is off, X's included. Neither moves.
- The rule, in the doc comment of each of the three: it may be called from inside guest code; it does
  not call the kernel, and it does not wait for anything that a caller of the kernel may hold. How long
  it takes is how long a group stop of that memory can be kept waiting.
- The first half is checked. Each worker has a mark of its own, "inside a guest hook that is not a stop
  point". The executor sets it before such a hook calls the system and clears it after: two plain
  stores into the thread's own record, which the hook has in hand already (wt/lib.rs:2124-2131). They
  are made on the clock hook and on `warn` only, in every process, so that an embedder's mistake shows
  under `Serial` as it would under `Parallel`.
- Every way into the kernel but the loop's own looks at the calling worker's mark, and panics with the
  call's name when it is set: `run`, `kernel.process(id)`, the `Kernel` calls that suspend and resume
  tasks (section 6), `kill`, an event handed in. The kernel finds the mark through the same piece of the
  build that names the calling host thread.
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
  finished, the system asked for the thread out. (The form before had a sixth, an entry told `Later`
  by a recording system; the order of entries is withdrawn, 4.6.)
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
- *Asking the system whether an entry may take effect*: withdrawn with the order of entries (4.6). In
  its place the ruled proxy calls the system twice at each system call,
  `intercept_system_call_request` and `intercept_system_call_response`, and the kernel's writes call
  `on_memory_written_by_kernel`. Each has a default that does nothing, and the interface notes leave
  it unverified that an unused one costs nothing. They are the interface track's, but they sit on
  every `Serial` system call, so T1's three builds measure them with the lock (section 12, decision 7).
- *The executor's locks* (7.2). A thread of a `Serial` process takes its process's lock once, when it
  enters `Execution::run`, and keeps it until it leaves. Its hooks take nothing.
- *The system's own locks.* `System` methods take `&self`, but what a system call touches in the
  shipped system is already atomic: `turn` stores one deadline word (rt/system.rs:256-266), and `ran` and
  `arrived` do nothing (:268-275). It keeps one such word per processor and adds no lock.
- *The stop word* (4.5): one plain load and a branch at each kernel entry, after the lock is taken.
- *The marks of a group stop* (4.5) are kept only for a memory with `Parallel` code: one flag tested in
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

Taking tasks out is where that reason would break, as the sixth check showed. The interface track
builds it, and section 6 relies on it. Since the ruling on suspend and resume by task it hands out two
kinds of value, both types the kernel provides: a `SuspendedTask`, and a `ProcessContext` (the memory
space, the descriptor table, the signal handlers and the filesystem information that a process's tasks
share), taken by `suspend_process` or copied by `copy_process_context`. The embedder may hold either,
move it to another host thread, or drop it, all outside the lock. Today the kernel's records hold `Rc`
clones that processes left in the kernel share: an open file description after a fork (`FileRef`,
kernel/file.rs:94), and the address map, the signal handlers, the descriptor table and the directories
of processes made with `CLONE_VM`, `CLONE_SIGHAND`, `CLONE_FILES` and `CLONE_FS` (kernel/process.rs:243,
:247, :251, :262). A task taken alone also leaves its siblings holding what it shares with them. A
clone or a drop of one outside the lock, while a worker changes the same count under it, is a data
race. Nothing is wrong in T1, which takes nothing out. The rule is asked of the embedder interface
track, for the day it builds the calls:
- **What leaves the kernel shares no `Rc` with what stays.** An object that only the value holds moves
  out with it. For an object it shares with a task or a process that stays, the kernel keeps the
  reference itself, in a table under the lock, and the value holds a key. `resume_process` and
  `resume_tasks` put the references back. A copied context holds keys too, or copies; which is the
  interface track's to choose. The form that crosses a wire turns each into an end, which it must do
  anyway.
- `SuspendedTask` and `ProcessContext` are then `Send` for the kernel state's own reason: every `Rc` in
  them is reachable only through them. A build for tests checks, at each call that hands one out, that
  each `Rc` in the value has no holder outside it.
- A value that is dropped lets its kept references go under the lock, not where it is dropped: its
  `Drop` puts the key on a list that the kernel empties at its next entry.
- Cost: one table entry for each shared object when a value is handed out and when it is put back.
  Nothing on a system call.
Rejected: values that are not `Send` and may be dropped only under the kernel's lock. A resume from
another host thread, or on another machine, is what the values are for, and no compiler checks where a
value is dropped. Rejected: these five types made `Arc` before the calls land. Every descriptor lookup
hands out a clone (kernel/file.rs:181, kernel/syscall/io.rs:33), so every system call on a descriptor
would pay two atomic counts for a call few processes ever see. A part's objects become `Arc` when that
part is split out (4.7), each measured, and from then on they can leave as they are.

Rejected for now: turning every `Rc` into `Arc` and every `RefCell` into a lock. It is the right end state
for the parts that are split out (4.7), and it costs every system call something. It is paid part by part
at the split, each measured.

### 4.3 Processors and workers

- The container has a fixed number of processors (section 11). The kernel keeps, per processor, what it
  keeps once today: the current thread, the give-way mark, the running thread, the picked runtime
  (kernel/lib.rs:136-140, :165).
- **A worker is not a processor.** A worker says which processor it is asking for:
  `Kernel::next(processor)`. In a live run each worker keeps one processor (section 5). The tests'
  repeatable order serves several processors with one worker (section 11), and the guest sees the same
  processor numbers whatever the count of workers.
- One run queue, as today. `next(processor)` gives the processor its current thread if that need not give
  way (as `pick` does today, kernel/task.rs:18-41), else the waiting thread with the least weighted time
  that is put forward and whose mask holds the processor. Which thread it gives depends only on the
  kernel's state and the processor asked for. Section 3's rule decides who is put forward:
  - A ready thread with `Parallel` code that runs as `Parallel` is put forward when its memory is not
    called in and no task that runs as `Serial` and overlaps it is handed out or due a turn (section
    3, rules 1 and 3).
  - A ready thread that runs as `Serial` is put forward only when it is due a turn and none of its
    overlappers is handed out: a waiter if it has one among them, else the one with the least weighted
    time. So among the threads of one `Serial` process the order is the one-processor fair scheduler's,
    as it is today, but for a waiter, which goes first because its overlappers stand still for it
    (4.5).
  - A memory that is called in puts forward its waiters and nobody else (4.5). A waiter is put forward
    whatever is called in, its own memory included, with one exception from section 3: a waiter of a
    task with `Parallel` code is not chosen while a task that runs as `Serial` and overlaps it is
    handed out. Being chosen runs its call again in the kernel. It goes back to guest code only when
    its own memory is not called in.
  - Why "due a turn" and not any ready `Serial` thread. With any, a `Serial` thread that gives way for
    an overlapper pinned to another processor is the only one of them its own processor may take, and
    is handed out again at once. The overlapper never runs, although Linux would run both. With "due a
    turn", the thread that gave way is not put forward while an overlapper has less weighted time.
  - What that costs: when the task due a turn is pinned to a busy processor, an overlapper that could
    run on an idle one waits for it. Counted "Serial task due a turn waits for its processor beside an
    idle one". With no thread pinned, and with one processor, the two rules choose the same thread.
- **`next` makes one kernel entry for each thread it chooses.** Choosing a thread changes the run queue,
  the processor's thread and the thread's processor, and the guest sees the last through `getcpu` and
  `rseq`. The entry is "this processor chose this thread".
  - What the kernel then does for the thread (the next item) is that entry's effect. A woken call that
    runs again can park once more, or end its thread. Then the entry hands nobody out, although it may
    have read or written bytes or written a frame. It is still an entry of that thread's, so what it
    read and wrote reaches the proxy and the hook for the kernel's writes as the thread's own (4.6).
    The form before called `next` an entry "when it hands a thread out". Its fifth check showed that
    those effects then had no place a replay could put them.
  - `next` then chooses again. That is the next entry. It goes on until a thread is handed out or none
    is put forward.
  - When none is put forward it answers `Idle` and marks the processor idle (section 5). An answer of
    `Idle` with nothing chosen changes nothing the guest can see and nothing a replay must repeat, so
    it is not an entry.
- **What the kernel does for a thread that is not running, it does when the thread is chosen for a
  processor, and not before.** Today the end of every entry runs woken threads' calls again and puts a
  signal's frame on the stack of a thread that waits its turn (`settle`, kernel/task.rs:343-404). With
  one thread running, nothing else could be using that thread's memory. With several processors a
  sibling could be, in guest code, on another processor. So both are done inside `next(processor)`,
  once it has chosen the thread and before it hands it out. A thread that runs as `Serial` is chosen
  only when none of its overlappers is handed out, and a waiter of a `Parallel` thread only when no
  `Serial` overlapper is (section 3, rule 4). So a woken call and a signal frame never touch a memory
  under a running thread that the memory's code assumes is alone. 7.2's rule for the executor's locks
  stands on this.
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
  - a thread running on a processor that its new mask leaves out is asked in with `stop_thread`, and the
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
3. When its thread runs as `Serial`, no task that overlaps its memory is handed out (section 3, rule 1).
   A call that makes a new overlap is run only once that holds for the overlap that results, its own
   siblings included where its memory comes to run as `Serial` (section 3). This counts the tasks of
   this kernel only; across kernels it is the placement's (section 3, decision 20).
4. When its thread has `Parallel` code, the other threads of its memory, and the `Parallel` tasks that
   overlap it, may be running guest code this instant. So:
   - a value read from guest memory may change the next moment. The kernel reads each value once and
     acts on what it read. The futex wait reads its word once, as one aligned 32 bit load;
   - what it writes may be overwritten or seen half-written by another thread, as on Linux;
   - the address map cannot change during the call, because every change to it is a kernel entry. A range
     the call checked stays mapped until the call returns;
   - another running thread's registers cannot be read (`Cpu::registers` is "only valid while the thread
     is not running", abi/cpu.rs:94). Nothing in the kernel reads a running thread's registers: a signal
     is taken by the thread itself when it comes in, and a tracer's or a crash dump's read waits for the
     thread (4.5).
5. Threads of other memories may be running. Two rules follow, and 4.5 applies them case by case:
   - **A thread that runs as `Serial`, or of a process given `Exact`, stops only at a kernel entry of
     its own.** There are two reasons, one for each.
     - `Exact`: a stop through its own entry ends the thread's turn at a position, which the system
       hears as it hears every turn's end (`turn` and `ran`, section 5) and can log with the process
       (4.6). A thread held in place ends no turn, and its position is told to nobody. (The form before
       said "a place in the order of entries"; that order is withdrawn.)
     - `Serial`: the running thread keeps its process's executor lock for its whole stay in
       `Execution::run` (7.2). That is what keeps locks off a `Serial` system call, and it only works
       if nobody reaches the process from outside while its thread runs. It is also what section 3's
       rule 2 needs: overlappers are called in, never held.
     So the kernel never reads or changes the memory, the page permissions or the code of such a
     thread's memory from outside while one of its threads runs. It calls the memory in first. Neither
     does the executor, from another memory's hook (7.5).
   - **A thread ends only inside a kernel entry of its own.** The kernel never ends a running thread from
     another thread's entry.
6. The kernel still never waits on its own stack. A `Host` call or a hook made inside an entry holds the
   lock while it runs. A slow hook stalls every processor's kernel entries until the split. The
   proxy's hooks are such hooks, at every system call (4.6). "Nobody waits inside a hook for another
   machine" (the orchestrator's proposal, to be confirmed by the owner; decision 14): waiting there for
   another kernel's answer can deadlock two kernels, not only stall them (4.5). The plan adds, as its
   own reading of the same danger and not the orchestrator's words: nobody holds a handle of one kernel
   while calling or waiting for another (4.5, "where they are called").
7. **When a hook about a process fires, no thread of that process runs guest code, if the process runs
   as `Serial`.** This follows from item 5: a hook fires inside an entry, and an entry that acts on a
   `Serial` process from outside it has called it in first. It holds across a new overlap too (section
   3). With `Parallel` code the process's other threads may be running. Section 6 relies on this.
   - The owner's guarantee is that "a hook fires only when every operation on that process is legal"
     (interface notes lines 57-58), and the ruling on suspend by task keeps it: the handle a hook is
     given has the calls of the `Kernel` trait limited to its one process. With the other threads
     running, it still holds for every operation of a process that is not given `Exact`: where one
     needs them stopped, the executor holds them in place for it.
   - **It does not hold for one kind of operation in one case.** In a (`Parallel`, `Exact`) process
     whose other threads run, `suspend_tasks` naming one of them, and `suspend_process`, need those
     threads stopped at positions. Only a call-in gives that, and a hook cannot wait for one. What the
     batch does then is not ruled; the ruling lists it as open, and its return has no error and reads
     a task left out as one that had exited (decision 11). The hooks this reaches are those that fire
     with other threads of the process running: `on_thread_starting`, `on_process_outgrown`,
     `on_watched_memory_accessing`, the proxy's two hooks, `on_memory_written_by_kernel`,
     `on_signal_delivering`, the shared-memory hooks, and `on_process_starting` for a child made with
     `CLONE_VM`, whose memory its parent's threads share. This narrows the ruled guarantee, so it is the
     owner's (decisions 8 and 11), with what keeping it whole would cost.

One known gap, to be closed by reading every call in T1: a call that waited and runs again reads its
arguments from guest memory again (kernel/task.rs:166-169, :360-368), except paths and descriptors, which
are kept (`Taken`, kernel/process.rs:407-414). Linux reads them once. Another thread can change them in
between whenever one runs while the caller is parked: in a process with `Parallel` code, and also in a
`Serial` process with two or more threads, where a sibling runs while the caller waits. The form before
said "in a `Parallel` group" only, and put the fix in T2; the mixed-ordering check found the `Serial`
case, which T1 already has. A `Parallel` task that overlaps a parked `Serial` caller's memory can do the
same to arguments in shared memory.

### 4.5 Who is stopped before what

Three primitives do all of it.

**Stop one thread** (suggested: a `Host` service `stop_thread(thread)`; the form before called it
`thread_stop`, renamed for the ruling that method names are verb first). The kernel asks the system to
bring a running thread back, and does not wait for it. The thread comes in by itself, through an entry of
its own, after its next stop check. The request goes through the system, not straight to the executor,
because a system that replays must be able to stop a thread at its recorded count in place of a stop
asked for now (4.6, decision 12): the slice timer already
works this way (rt/system.rs:256-268).

**A request to stop is kept until the thread's next kernel entry.** Today one word does everything. Zero
means carry on, anything else means call the stop hook (abi/registers.rs:54-57). The executor writes over
the word each time round its loop (wt/lib.rs:2283) and in the stop hook (wt/lib.rs:1786), and a request
that lands in between is put back by `StopFlag`'s own latch (rt/lib.rs:64-74). With a word per thread and
requests from any host thread, nothing would put it back. A lost request is a process that is never
killed: a thread that only computes, alone on its processor, has no slice timer (kernel/task.rs:43-53,
rt/system.rs:256-266), and a memory called in is woken only by that thread's own entry.

So the stop word becomes a set of reasons. Each is a bit that only its owner writes, with an atomic or
and an atomic and, never a plain store of the whole word:
- **asked in** (suggested `STOP_ASKED`): set by `stop_thread`, from any host thread. Only the thread
  itself takes it off, and only inside a kernel entry of its own, as the first thing after the kernel's
  lock is taken, whatever that entry is for. Whatever the kernel wanted the thread for (a signal, a
  memory called in, a `Serial` overlapper due a turn, a process ending, a new mask), it finds at that
  entry.
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
  asked for nothing". A replaying system whose `stop_thread` does nothing never sets the bit (4.6).

What it costs a system call: one plain load of the word and a branch, after the lock is taken. The
atomic and is made only when the bit is set.

**Call a memory in** (the kernel's). A call that needs threads stopped names every memory it needs at
once: each memory it will reach, and, for an overlap it makes, the memory of every task section 3 says
is called in, which is its own memory too when its own siblings are. The kernel asks `stop_thread` for
each of their running threads other than the caller, puts the call's mark on each memory named, and
parks the call. Two more owners of a mark use the same mark: section 3's rule 3 for the overlappers of
a `Serial` task that is due a turn (that mark belongs to the due task, and comes off when it has been
handed out or is no longer due), and a resume that makes an overlap (that mark belongs to the pending
overlap, and comes off in the entry where it takes effect, section 3). A batch taken out from outside a
hook marks no memory (section 6).
- A memory is called in while any mark is on it. `next` then hands none of its threads back to guest
  code. A mark names a memory, so marks follow an overlap that starts or ends with nothing done for
  them.
- The call is woken by the entry after which no memory it named has a thread handed out (section 3,
  rule 1: handed out, not only in guest code). Its thread is now a waiter.
- The call runs again when the waiter is chosen (4.3), from its start, as every woken call does
  (kernel/task.rs:1-6, :343-375). It finds nobody running, does its work in that same entry, and takes
  its marks off. A call that then has to wait for something else parks for that as any call does,
  with its marks off.
- Each thread that came in did so through an ordinary entry, at a position when its code has the
  count.

**Two call-ins never wait for each other.** The form before the sixth check let them, and that check
showed how. Process P1 has threads A1 and S1, process P2 has thread A2, all `Serial`, on two processors.
A1 calls `write` on a file that P2 maps: P2's memory is marked, A2 is asked in, A1 parks, and S1 is
handed out. A2 comes in through its own `write` to a file that P1 maps: that wakes A1, marks P1's
memory, asks S1 in and parks A2. S1 comes in and wakes A2. Now both memories are marked, and each
waiter is of a memory the other's mark is on. Under the earlier rule a called-in memory put forward
only the caller that wanted that same memory, so neither waiter was ever chosen and neither mark came
off. A new overlap, `process_vm_writev` or a tracer in the same shape hung the same way.

The rule now:
- **A mark keeps a memory's threads out of guest code, and nothing else.** A waiter is put forward
  whatever is called in, its own memory included (4.3). Being chosen runs its call again, inside the
  kernel.
- **When its call has run, a waiter goes back to guest code only if its own memory is not called in**
  by another call. Otherwise it stays ready, with its call answered, and is handed out when the last
  mark on its memory comes off. That is a choice that hands nobody out (4.3).
- **A waiter is chosen only where section 3's rule lets its work run.** A waiter that runs as `Serial`
  is chosen only when none of its overlappers is handed out, and a waiter with `Parallel` code only
  when no `Serial` task that overlaps it is handed out (rule 4). So a woken call never touches a memory
  under a running thread whose code assumes it is alone, and when its work is done no overlapper that
  must not run beside it, and no thread of a memory it named, is in guest code. Every process lock it
  needs is free, and 7.2's rule stands as it was.
- **A thread in a waiter's way is asked in at once**, with `stop_thread`, whether or not it has a slice
  (section 5). Its overlappers stand still for the waiter, so it does not wait for a slice to end.
- **A mark belongs to its call.** It comes off when the call has run again. It also comes off when the
  call is given up: its thread is ended by a fatal signal while it waits, or its task is taken out of
  the kernel while it waits (section 6). A call waiting for a call-in has had no effect yet (section
  3), so a task taken out then is treated as for a signal with no handler: its call runs again where
  the task is resumed, with marks of its own there if it needs them (proposed, to be confirmed by the
  owner; decision 19, and "a task taken out while it waits" below). No other signal interrupts a wait
  for a call-in: the wait is bounded, and the call has had no effect yet.

Why no circle is left. What a waiter needs before its call runs again is three things, and none of
them is a mark coming off.
1. The running threads of the memories it named have come in. Each was asked, and no request to stop
   is lost (above). A marked memory is handed nobody new.
2. Where it runs as `Serial`, its overlappers' handed-out threads have come in; where it has `Parallel`
   code, its handed-out `Serial` overlapper has. Each was asked at once.
3. A processor. Section 5's condition gives one: somebody in the waiter's way is due in.
So every mark comes off after a bounded time, whatever other marks are on. In the case above A1 is
chosen first. Its `write` finds nobody of P2 running, is made, and A1's mark comes off P2's memory. A1
stays ready, because P1's memory is still marked. A2 is chosen, its `write` is made with nobody of P1
running, its mark comes off, and it runs. P1 then puts forward the thread due a turn, A1 or S1.

A model of the form before's rules was searched for its last correction (section 14): two `Serial`
groups of two threads and one `Parallel` group of two, on one to four processors, with calls that reach
another group, join two groups, or call the caller's own group in. In up to 62,189 states it found no
state where nothing can move, no state from which the end cannot be reached, no thread in guest code
beside a sibling of its `Serial` group, no call at work under a running thread of a group it named, and
no call-in left pending when only the steps the kernel forces are taken. With the earlier rule the same
search finds the sixth check's case. **That model is of groups. It has none of section 3's rule: no
mixed overlap, no exclusion by "handed out", no mark for a task due a turn, no waiter held back by a
handed-out `Serial` overlapper. It must be redone for them** (known fault 1). In particular, rule 3's
mark, a resume's mark for a pending overlap and rule 4's held-back waiter are new ways for a waiter to
wait, and the "why no circle" argument above has not been checked against them. The argument's three
needs still hold for the two new marks as written: neither belongs to a waiter, and each comes off by
something that needs no mark to come off (the due task handed out or no longer due; the last asked
task coming in). That is this correction's reading, unchecked.

The same rule covers a memory that calls itself in (a (`Parallel`, `Exact`) process) and two memories
that come to overlap, where the caller's memory is one of them (section 3): the caller is a waiter of a
memory that is called in, and is chosen.

While a mark is on, each of the memory's threads that came in found nothing to run on its processor,
and those processors may have gone idle. Taking a mark off makes no thread ready: the threads were
ready all along. So an idle processor is woken by a condition on the kernel's state, not by the event
"a thread became ready" (section 5): the entry that takes a memory's last mark off ends by waking a
processor for each thread it let run again.

**Group stop** (the executor's), done inside the hook or the `Cpu` call that needs it. It makes every
other thread of one memory held while one change is made. It is per memory: every
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
The order has one exception, and it is stated here and not hidden. A thread of a `Serial` process has its
process's lock for its whole stay in `Execution::run`, and asks for the kernel's lock under it at every
system call (7.2). That is safe only because nobody who has the kernel's lock ever waits for a `Serial`
process's lock. 7.2 makes that a rule the code checks.

**Which primitive, for which memory.** For a change that needs the other threads of memory M stopped:
- **nobody**, when no other thread of M is handed out. That is always so when M's threads run as
  `Serial` and the caller is one of them, or overlaps M and runs as `Serial` itself (section 3, rule 1);
- **a group stop**, when M's threads run as `Parallel` and none of them is of a process given `Exact`.
  Where each thread was held is not recorded, and such a process promises nothing about that;
- **a call-in** otherwise: M's threads run as `Serial` and the caller is not one of them; or, where they
  run as `Parallel`, **for the threads of M that are of a process given `Exact`, and only for them**.
  Each such thread then stops at an entry of its own (4.4 item 5), and the caller parks until they are
  in. When the call runs again, M's other running threads, of owners not given `Exact` under
  `CLONE_VM`, are held by a group stop in that entry, as they would be with no `Exact` owner. That is
  meant to make an owner given `None` pay only what it pays alone, and it does not yet: the call's
  mark names M, so while the `Exact` owner's threads come in, a thread of the `None` owner that comes
  in for any reason is not handed back until the mark is off (the second check, its sixth finding;
  known fault 12). A mark that names a memory and, under `CLONE_VM`, the owners it calls in would
  close it; that is a suggestion, not built or checked, and whether the ruling asks for it is the
  owner's (decision 22). (The form before the first correction called in every thread of M whenever
  an owner of M was given `Exact`. The revision's check found that against "where replay is
  recorded": "a process with ReplayDeterminism::None costs nothing and breaks no neighbour".)
And for a `Serial` task's turn, its overlappers are called in, never held (section 3, rule 2).

Below, "M's other threads are stopped" means by this rule. One change is never "nobody", although its
caller may run as `Serial`: a mapping that makes a new overlap with a memory whose threads are handed
out, or that makes the caller's own memory run as `Serial` while its siblings are handed out (below,
and section 3).

A (`Parallel`, `Exact`) process keeps five uses of holding a thread in place. Four only order memory
accesses, and that order is the embedder's part there (section 2): the 128 bit atomic (7.3), a write of
the kernel's from outside into a memory that has one thread (7.3), `membarrier`, and the embedder
setting a watch (section 8). The fifth changes nothing a guest can see: growing the executor's lookup
tables (7.5). Where the embedder needs a held thread's position, it can read it: the thread is at a stop
point, with its registers and its count saved.

A memory whose threads run as `Serial` keeps no marks of a group stop, and accesses to it take no stop
lock: within the memory only the caller runs, no overlapper is handed out beside it, and from outside
the kernel has called it in first. The kernel tells the executor each memory's form of code, whether it
runs as `Serial`, whether it has several threads, and which of its owners was given `Exact` (7.3), and
changes it only while none of that memory's other threads is handed out. The executor counts "group
stop reached a running thread that runs as Serial" and "group stop reached a thread of an Exact process
for a change of the kernel's", which must both stay at zero.

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
    - It is needed only after a group stop. Under a call-in the threads have left already, and the
      other threads of a memory that runs as `Serial` are not in `Execution::run`.
    - It costs each other running thread 300.8 ns on top of its stop, and its calls in progress
      return through the bottom loop. That is paid at every `munmap` of data and every `mprotect`
      that takes writing away in a `Parallel` process. How often is T2's and T3's measurement.
    - Rejected: only the threads whose stack pointer lies at or below the page, in the mapping that
      holds it. That is the x86 plan's test for code on a live stack (x86 plan lines 1898-1905), and
      this plan's earlier form used it for watches. It misses a thread that has moved to another
      stack while calls on the first are still in progress. Go's `systemstack` has that shape: it
      moves the stack pointer to the scheduler's stack, calls a function and moves back (Go's
      runtime, asm_arm64.s). The frames on the first stack are relied on again when the callee
      returns. A narrower test that is still sound needs the translator to bound where a thread's
      live frames are. It is asked of the translator track only if the count above is measured to
      matter.
  - The change reaches M from another process when a file changes size or bytes: the kernel then lowers
    permissions and drops code in every process that maps the file (`changed`,
    kernel/shared.rs:581-600). Counted "memory called in for its pages" when it is a call-in.
  - In an `Exact` process, the hook that announces a code change (`ic ivau`) cannot make the drop
    in place: it hands the kernel an exit for it, and the kernel calls the memory in. Counted "memory
    called in for a code change".
- **A page gains a permission, or a new mapping appears**: nobody, where the rule above says nobody or a
  group stop. A thread that still acts on the old table faults to the executor, which looks at the
  table again and carries the thread on. Counted "fault settled by the table as it is now". Where the
  rule says a call-in, the memory is called in, as for a loss: whether an access faults would otherwise
  depend on the host's timing. A file that grows under another process's mapping is such a case. A new
  mapping that makes a new overlap is the next case, not this one.
- **Two memories come to overlap** (section 3): every handed-out task that would break rule 1 once the
  change is made is called in: the other side's where the caller's side runs as `Serial`, the other
  side's `Serial` tasks where only they do, and the caller's own siblings where its memory comes to run
  as `Serial`. The call that caused it parks before it has any effect; a resume instead marks and
  returns (section 3, "how a new overlap takes effect"). For two `Serial` processes that is the other's
  one running thread. Counted "tasks called in for a new overlap". The shared-memory hook fires for the
  mapping process in the call when it runs again, before the mapping is made (decision 21 for the
  processes it does not name). (The form before also called threads in for "a stricter config in
  force", and fired `on_execution_config_in_force_changing` here. Both are gone with the config in
  force.)
- **The kernel reads or writes M's bytes from outside M's process** (a `write` to a file reaching pages
  M shows, `process_vm_readv` and `process_vm_writev`, a tracer, `/proc/<pid>/mem`, `environ`,
  `cmdline`). Every such write fires `on_memory_written_by_kernel` for M's process with its exact range
  (section 2).
  - M's threads run as `Serial`: M is called in. The read or write then lands while every thread of M
    is at an entry of its own, at a position. Counted "memory called in for a read or write from
    outside". `cmdline` is not inspection (section 2), but it is still such a read.
  - M's threads run as `Parallel`: nobody is called in. The bytes land while its threads run, as on
    Linux. The executor's copy into M is made with pointer writes, since "no guest runs"
    (wt/lib.rs:1423) no longer holds, and with M's stop lock shared (rule 2). With `Exact` this is a
    memory access like a thread's, and the embedder orders it through its watches, which the kernel's
    own accesses fire too (section 8).
    - One case needs more. A write into a memory that has one thread is made under a group stop of
      that memory, so that it cannot land inside a 128 bit atomic made inline (7.3).
- **The kernel changes a guest word that guest code changes too**: `FUTEX_WAKE_OP`
  (kernel/syscall/futex.rs:682-711), the owner word of a priority-inheritance lock (:760-786), the
  owner-died mark of a robust lock (:1020, :1163). Today each reads the word, works out the new value and
  writes it. In a memory with `Parallel` code another thread can change the word in between, and its
  change is lost. Each becomes one compare and exchange, asked of the executor (suggested
  `Cpu::compare_exchange(memory, address, expected, new)`), and tried again when the word had changed,
  as Linux does (`futex_atomic_op_inuser`). Nobody is stopped. It is the one path for both orderings: in
  a memory that runs as `Serial` it succeeds the first time. By section 3's rule 4 it is made in the
  phase where the word's other writers run: a `Parallel` writer's change is never made while a `Serial`
  task that shows the word is handed out.
- **Another thread's registers are read** (a tracer, a crash dump): that thread is called in, whatever
  its process's config.
- **exec**: the process's other threads that are not running are ended as today (kernel/task.rs:648).
  Its running ones, which exist only in a `Parallel` process, are called in, and each ends in its own
  entry. The caller parks until they have, as Linux's exec waits for the other threads to die.
- **`exit_group`, a fatal signal**: the process is marked as ending, and nobody waits.
  - When no thread of its memory is running other than the caller, it ends at once, as today
    (kernel/task.rs:685, kernel/signal.rs:481-485). With one processor that is always so.
  - A `Serial` process whose thread is running (the signal came from outside the process): the thread
    is asked in with `stop_thread`, and the whole ending is done in the entry that brings it in. Ending
    a thread writes into the process's memory (the word cleared at a thread's exit, which fires
    `on_memory_written_by_kernel`), so it must not be done under a running thread.
  - A `Parallel` process: the threads that are not running are ended at once. Each running one other than
    the caller is asked in and ends in its own entry. The process ends, with its memory dropped and its
    parent told, at its last thread's end, as on Linux.
- **fork from a threaded process**: M's other threads are stopped for the length of the copy in
  `memory_duplicate`, so that no page is copied half-written. Counted "memory stopped for a fork" or
  "memory called in for a fork".
- **A signal for a running thread**: that thread only, by `stop_thread`, and nobody waits. It takes the
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
  It tests one value when nothing is pending on the process. This holds in a `Serial` process as in a
  `Parallel` one: there the sibling left out is the parked one. Wherever the signal is delivered, the
  kernel fires `on_signal_delivering` first, inside the entry of the thread that takes it, so the
  signal is at that thread's position (section 2).
- **A stop signal, `SIGCONT`, a tracer**: the process's running threads, by `stop_thread` for each. Each
  parks itself when it comes in.
- **A futex wait or wake**: nobody. Both are kernel entries, so the wait's "compare, then park" and the
  wake are already in one order.
  - **Across machines** (interface notes, "2026-11-19 late night: locks across machines (discussed with
    the owner; the two calls agreed in substance, details open)"; not ruled). A lock is a 4-byte word
    plus the futex call; taking and releasing it is a write to the word, which goes through the memory
    system, and the kernel keeps only the list of waiters for each place. The interface notes' discussion
    says the embedder knows where it has waiters from the proxy: the request names the place and the
    bitset before the kernel acts, and the response says when the wait ends. That is the discussion's
    claim, not a ruling, and it is not whole: a requeue made on the waiters' own kernel moves them with
    no report task by task (below, and known fault 11). Two calls "on the process handle" edit a
    waiter list for a thread on another machine: `wake_waiters(address, count, bitset)` and `move_waiters(from, to,
    count)`. They cover wake, wake with a bitset, wake-and-change, requeue, compare-and-requeue and a
    thread's end (the kernel clears a word and wakes: seen through `on_memory_written_by_kernel`). Each
    is a kernel entry of the kernel that holds the waiters, and stops nobody. What they return is
    proposed and not confirmed (decision 14).
  - **One ordering point for each lock** (proposed by the orchestrator, to be confirmed by the owner;
    decision 14). Every operation on one lock word that spans machines is put in order at one point for
    that word: the wakes, the requeues to and from it, and the changes the kernel makes to the word
    itself (`FUTEX_WAKE_OP`'s write of its second word, the clear at a thread's end, the owner-died
    mark). Linux does the same on one machine under the word's hash bucket lock. A waking call that
    reaches another machine parks **before it has had any effect**; its whole operation, the part on
    its own kernel and the parts on others, is then made at the point as one step, and the call is
    finished with the count. Withdrawn: the first correction's "its local part done in its entry", with
    the remote part made later.
    - Why (the second check, its second finding; the wake-result-use report's counterexample A, from
      glibc 2.17's cond_signal.c:41-60 and cond_wait.c:118-171, as CentOS 7 ships it). W1 sleeps on a
      condition variable's word on K3. Thread A on K1 signals: it raises `wakeup_seq`, then makes one
      `FUTEX_WAKE_OP` that wakes on the condition's word and releases the condition's internal lock
      word. With the local part first, K1 writes the internal lock word in A's entry and A parks for
      the remote wake: the internal lock is free before the wake is in order. W2 on K2 takes the
      internal lock, records seq = 1 and sleeps on the condition's word. The remote wake, made later,
      picks W2, which sees `val == seq` and sleeps again; W1 sleeps on. A wake is lost although A
      waited. With one point, the release of the internal lock and the wake are one step there, so W2
      cannot queue before the wake has picked W1. The report's counterexample B, a requeue made
      locally and then remotely, is two steps the same way; at one point it is one.
    - Not covered yet (known fault 11): an operation on two words whose points differ (a requeue from
      one memory object to another; `FUTEX_WAKE_OP` on two words in two objects) is two steps again
      unless both points are taken together, in a fixed order, as Linux takes two buckets. How is part
      of decision 14.
  - **How the point is chosen is open, for the owner** (decision 14). The options, each with what it
    costs (all expected, nothing measured), none recommended here:
    - **the kernel that holds the lock's page decides.** No new state, and on a contended lock the
      waker fetches the page anyway (the report, section 4: about one round trip before it unlocks).
      Cost: the point moves with the page, so a step under way must finish before the page leaves, and
      every other access to that page waits meanwhile; an operation on two words on two pages needs
      both pages on one kernel; a requeue made on the waiters' own kernel is made there with no report
      to the embedder's index, task by task.
    - **a fixed home kernel for each memory object** (a `MemoryObject` of the ruling on shared memory,
      or a process's own memory). The point never moves, and two words of one object, such as glibc's
      condition variable and its internal lock, share it. Cost: every operation on a word with a waiter
      on another machine is a round trip to the home, even when the waker and every waiter are
      elsewhere; the home must take the page, or ask for it, to make the kernel's changes of the word;
      a home that leaves needs a hand-over; a requeue between two objects still meets two homes.
    - **the embedder sequences.** The kernel keeps only the two calls; the embedder already keeps an
      index for each place, and every operation on a word with waiters on two machines passes through
      it, so the index can follow requeues. Cost: a round trip to the sequencer for each such operation,
      the waker's own kernel's part included, so a wake whose waiters are partly local is no longer
      made in place; the kernel's changes of the word must then be made by the sequencer or through
      it, which needs a call the agreed two do not have.
    In each, a word whose waiters and waker are all on one kernel is ordered by that kernel's lock, as
    today.
  - **Where they are called.** The ruling-in-substance says only "on the process handle". This plan:
    on a handle of the kernel that holds the waiters, taken with `kernel.process(id)` from outside any
    hook. A hook never calls them on another kernel, because that waits for the other kernel's lock
    ("nobody waits inside a hook for another machine", the orchestrator's proposal, to be confirmed by
    the owner). The plan adds, as its own rule and not the orchestrator's: **the embedder never holds a
    handle of one kernel while it calls or waits for another.** It drops K1's handle before it takes
    K2's. Why (the second check, its eighth finding): a handle from `kernel.process(id)` holds its
    kernel's lock while it lives (section 5), so a thread holding K1's handle, to finish T1's parked
    call, while it calls `wake_waiters` on K2, against a thread doing the reverse, is the circle below
    again, outside any hook. Section 5 says the same.
  - **The waking thread never waits for another machine inside a hook** (proposed by the orchestrator,
    to be confirmed by the owner; decision 14). The count a wake returns must be exact ("the count a
    wake returns must be exact (Linux defines it), so the waking thread waits for the other machine WHEN
    there is a waiter there"). That wait is made with no kernel's lock held: the waking call is parked
    as any waiting call is, and the embedder, from outside any hook and holding no other kernel's
    handle, has the step made at the ordering point and then finishes the call with the count. Or the
    proxy's request hook answers the call (`Break`) and the call is finished later from outside the
    hook. Both need one thing the ruled proxy does not have: a way for the embedder to finish a parked
    call with its result later (the ruled `Break` carries the response at once). That is decision 14.
    - Why (the revision's check, its third finding). T1 on kernel K1 is inside a proxy hook, holding
      K1's lock and waiting for K2's wake count; at the same time T2 on K2 is inside its hook, holding
      K2's lock and waiting for K1's. Each embedder's `wake_waiters` needs the other kernel's lock, so
      each waits for the other for ever. Splitting the lock (4.7) does not remove the circle, because
      the waiter lists stay under whichever lock the waiting hook holds. Now neither hook waits: each
      call parks and its entry ends, so K1's and K2's locks are free, and each `wake_waiters` takes
      its lock and returns.
    - What it costs (expected, not measured): a waking call that crosses machines leaves
      `Execution::run` and comes back (300.8 ns measured for that alone, section 3), on top of the
      round trip it waited for anyway; a wake with no remote waiter is answered in place as today.
  - **What any form of it must keep** (orchestrator/wake-result-use/REPORT.md, section 2, from Linux's
    source and manual; the conditions on a wake sent without waiting are expected there, not tested):
    one point for each lock word decides the order of the operations on it; a requeue moves and wakes
    as one step; a waiter already chosen by a wake returns woken (0), not timed out ("If we were woken
    (and unqueued), we succeeded, whatever", Linux's kernel/futex/waitwake.c); and a wake sent without
    waiting for the other machine is sound only if nothing the waker does afterwards (a memory write
    leaving its machine, a signal, another futex call, its exit) becomes visible off its machine before
    the wake is put in order at that point.
  - **The wake at a thread's end** (the second check, its twelfth finding). The kernel clears the
    thread's word and wakes one waiter on it; no call is behind it, so there is nothing to park, and
    no hook may wait. Nobody reads its count, so it goes without waiting, and the report's conditions
    on such a wake apply to it. The clear itself is a change the kernel makes to the word, so at the
    point it is one step with the wake, and a `pthread_join` that reads the word sees it no earlier than
    the wake. What the ending thread's end makes visible besides (its process's exit, seen by a parent
    through `wait4`) overtaking the wake is not designed (known fault 11).
  - **The embedder's index after a requeue on the waiters' own kernel** (the second check, its fifth
    finding, which it marked plausible). A `FUTEX_CMP_REQUEUE` made on K2 moves K2's waiters from one
    word to another with no response for them and no call of the embedder's; the proposed return of
    `move_waiters` covers only moves the embedder asks for. A later wake of the second word from K1 is
    then not sent to K2, and the moved waiters sleep with the lock free. Whether this arises depends on
    the ordering point: where the point is the embedder's, or a home that is not K2, the requeue passes
    through it and the index can follow; where it is the kernel holding the page, it does not. Known
    fault 11, decision 14; the plan adds no mechanism for it.
  - **A task taken out while it waits** (proposed by the orchestrator, to be confirmed by the owner;
    decision 19). `suspend_tasks` treats a waiting task exactly as Linux treats a signal with no handler
    arriving during that call. The kernel already has that path for a signal (kernel/signal.rs:628-638:
    "No handler ran: the call runs again, from `restart_syscall` for one that keeps state"), and taking
    a task out uses it:
    - **a call that has had no effect yet runs again where the task is resumed.** It answers its
      restart code, and its registers are set to make it again (pc back one instruction, and
      `restart_syscall` for a call that keeps state); the `SuspendedTask` carries them with the task's
      restart record (kernel/process.rs:300). A futex wait is taken off its waiter list and, resumed,
      compares its word again;
    - **a wait with a relative time limit goes on with the time left.** The kernel already turns the
      limit into a deadline on its monotonic clock at the call's first run (kernel/syscall/futex.rs:
      274-286), and `restart_syscall` waits until the same deadline (:508-551) and answers `ETIMEDOUT`
      once it has passed (:493-495). So the time a task spends between kernels counts, as Linux's
      restart counts the time a task spent stopped, and a wait whose deadline passed while it was out
      returns `ETIMEDOUT`, not `EAGAIN` (the second check, its ninth finding). That reading of "the time
      left" is the plan's, to confirm with decision 19, and it needs the kernels' monotonic clocks to
      agree, which is the embedder's and not designed;
    - **a call that has already had an effect is not run again, and returns what it did**: a `read` or
      `write` that has moved some bytes returns that count, as Linux returns a partial count after a
      signal; a waking call whose step has been made at the ordering point returns its count;
    - **a waiter already chosen by a wake returns woken**, 0, and does not wait again (the third
      requirement above; the kernel's `woken`, kernel/syscall/futex.rs:487-490);
    - **every other waiting call** (`read`, `poll`, `nanosleep`, `wait4`, `epoll_wait` and the rest)
      follows the same rule by the restart code it already answers for a signal. Linux's manual also
      lists calls that answer `EINTR` after a stop signal and `SIGCONT` even with no handler (signal(7),
      "Interruption of system calls and library functions by stop signals"); which of them this kernel
      follows is T1's to read, not checked here.
    A waking call parked for the ordering point (above) has had no effect until its step is made:
    taken out before, it runs again where it is resumed and wakes once; taken out after, it carries the
    count. Finishing a call whose task has moved is part of the same open call as finishing a parked
    call (decision 14). The embedder takes the task's entry out of its index of waiters itself, since
    it made the call that took the task out. No call is needed to wake a `SuspendedTask`: it is in no
    list.
    - Why (the revision's check, its second finding). W waits on a futex on K2, and the embedder's
      index says K2 has a waiter there. The embedder takes W out (`suspend_tasks`) to move it to K3. In
      the form before, the wait travelled in the `SuspendedTask` and the index still said K2. T on K1
      calls `FUTEX_WAKE`; the embedder's `wake_waiters` on K2 finds nobody; T's call returns 0 where
      Linux returns 1, and W, resumed on K3 still waiting, sleeps for ever. Now: the index no longer
      names K2, and T's wake returns 0, which is what Linux returns for a waiter that has left the queue
      to run its call again. W's call runs again on K3, compares the word, finds the value T changed
      before it woke, and returns `EAGAIN` without sleeping, as Linux does.
    - Why not "runs from its start" for every call (the second check, its third finding). A waking call
      parked for the other machine's count had, under the first correction, already made its local
      wake; run from its start, it woke again and applied `FUTEX_WAKE_OP`'s operation twice. A pipe
      `write` that had written part of its bytes and waited for room wrote them twice. Now the first
      has had no effect until its step (above), and the second returns its partial count.
    - It closes the lost wake only where the waker changes the word before it wakes, as locks,
      condition variables and once-flags do (not checked here site by site), and only where the memory
      system shows that change to the machine where the call runs again once the wake has been put in
      order. One machine writes a page at a time (the interface notes, same heading), which is expected
      to give that; it is not built or checked. A program that waits on a word nobody changes (the
      second check recalls perf's futex wake bench and LTP's `futex_wake` tests so; not checked here)
      has its wait run again and sleep on. That is Linux's own window for a wait restarted after a stop,
      opened here at every move.
  - The priority-inheritance group (six operations) is not covered by the two calls (decision 13).
- **A thread's mask leaves out the processor it runs on** (`sched_setaffinity`): that thread only, by
  `stop_thread`. The caller parks until it has come in (4.3).
- **`membarrier`**: a group stop of the caller's memory when its threads run as `Parallel`, and for the
  commands that mean every process, of every such memory. Each held thread has passed a full barrier:
  it took its mark off with a sequentially consistent store. A memory that runs as `Serial` needs
  nothing: one of its threads runs at a time, its overlappers are not handed out beside it, and every
  hand-over goes through the kernel's lock.
  - One command needs more. `MEMBARRIER_CMD_PRIVATE_EXPEDITED_RSEQ` must restart the restartable
    sequence of every running thread of the process, or with `MEMBARRIER_CMD_FLAG_CPU` of the thread on
    one processor. A sequence is restarted only in a kernel entry of its own thread (`rseq_preempted`,
    kernel/signal.rs:648-665, called today only for a thread that gives way, kernel/task.rs:133-137),
    and a group stop holds threads in place with no entry. So for this command each such thread is asked
    in with `stop_thread` and restarts its sequence in the entry that brings it in, and the caller parks
    until all have. Linux's call also returns only after every such CPU has been interrupted. In a
    `Serial` process the other threads are not running, and each was taken off through an entry.
- **The container's first process ends**: every other process is ended as by a fatal signal, as Linux
  ends a container. Each worker's `next` then answers `Finished`.

### 4.6 Replay, per process, at its own system calls

The owner ruled where replay is recorded ("where replay is recorded"): per process, at the process's
own system calls, and not at the kernel's outer boundary. The form before recorded and replayed the
order of every kernel entry of the whole kernel: a service asked before each entry took effect
(`kernel_entry_admit`, answered `Now` or `Later`), a field of `Start` saying whether the system orders
entries, a digest of each entry for `External` (`on_kernel_entry_ended`), a replayer that called `run`
in the log's order, entries kept aside when told `Later`, and the rule that a kernel is recorded whole.
All of it is withdrawn. What replaced it: the system call proxy, the hook for the kernel's writes and
the signal hook (section 2), with the embedder keeping each process's log.

**What a process's log is.** The results of its system calls, and what reaches it without one: signals
at a counted position, and shared memory through watches (section 2). The embedder sees:
- each system call of each thread through the proxy: the request before the kernel acts, the response
  after, both with the process and the thread. A call answered from a log is a `Break` in the request
  hook;
- every write the kernel makes into the process's memory, with its exact range
  (`on_memory_written_by_kernel`): the data of a `read`, a signal frame, a page of a mapped file being
  filled, the word cleared at a thread's exit. The writes of a woken call that runs again inside `next`
  (4.3) are its thread's;
- every signal before it is delivered (`on_signal_delivering`), in the entry of the thread that takes
  it, so at that thread's position;
- each turn's end with its position, as the system hears it today (`turn` and `ran`, docs/runtime.md
  section 12). A stop that the kernel or the executor causes to a thread that stops only at its own
  entry (4.4 item 5) is such a turn's end.

**Not every call can be answered from the log.** The interface notes record, as a note to tell the owner
and not as a ruling, that calls which change the process's own records in the kernel (`mmap`, `munmap`,
`mprotect`, `brk`, `rt_sigaction`, `rt_sigprocmask`, `clone`, `futex`, `exit`) must really run on the
replaying kernel, and only calls that bring something in from outside are answered from the log. The
proxy allows both, and which is which is the embedder's table. For threads this matters: a futex call
between two threads of one process runs for real in a replay, so its result depends on where the other
threads are, and the threads must take their turns as they did (decision 17).

**What the kernel must still give, and nobody has designed.** The ruling names two primitives: handing the
embedder each system call result of a recorded process, and running a process with its system calls
answered from a log. Threads add three needs to the second:
- **the order of a process's threads' turns, and a way to make a replay follow it.** With one thread a
  process's log orders itself. With two or more, which thread runs next is the kernel's choice, made
  on state that other processes move (weighted time and the run queue, 4.3), and a replay of one
  process alone does not have that state. The form before fixed it by logging the whole kernel's order.
  Nothing replaces that yet (decision 17);
- **reading a thread's progress count, and stopping a thread at a given count**, needed only under
  `Exact` and ruled to come with this plan ("signals as an input"; decision 12). Counted turns
  (`Turn::Counted`, rt/lib.rs:25-27) already end a turn at a count for a system that asks for one, and
  only code with the count can be given one, so `turn` is told the thread's guarantee. `stop_thread` is
  asked of the system (4.5), so a replaying system can do nothing for a stop and end the turn at the
  recorded count instead;
- **a moment before a `Serial` `Exact` task's turn**, or a take of dirty memory across every place
  that shows a page, for what other processes wrote to its shared memory (section 3, decision 16).

**What each guarantee asks of the kernel.**
- `None`: nothing. The proxy's defaults change nothing, and nobody logs anything. A process given
  `None` beside a recorded one costs nothing and breaks nothing: a recorded process that reads what it
  wrote has the read's result in its own log.
- `External`: no count. The embedder logs results, and a replay checks each request against the log and
  is thrown away when one differs. The check must cover what a call takes from the process's memory,
  and not only its arguments: the bytes of a `write` are what the process does to the outside. The
  embedder reads them through the handle the request hook is given. A stop has no position, so a
  process with one thread that takes no signal while it computes replays every time; anything else may
  differ inside, which is what "not guaranteed to succeed" means. A signal that arrives while it
  computes is a fresh snapshot (section 2).
- `Exact`: code with the count. Everything that reaches it without a system call is logged at a
  position.

**What is exact.** A process given `Exact` replays with the same registers and memory at every logged
position when everything that reaches its memory is in its log, in order:
- its own calls' results, answered from the log or run again for real;
- the kernel's writes into it. Where its threads run as `Serial` the kernel has called it in first
  (4.5), so each lands while every one of its threads is at an entry, at a position. With `Parallel`
  code they are ordered through the embedder's watches, which the kernel's own accesses fire (section
  8);
- other processes' stores through shared memory: through watches, and for a task that runs as `Serial`
  the moment of decision 16;
- signals at counted positions;
- its own threads' order: through watches with `Parallel` code (section 8), and by decision 17 with
  `Serial` code and two or more threads.
A neighbour's guarantee never enters into it.

**In a `Parallel` process given `External`**, the ruling also puts each locking and atomic instruction in
one order. Its design is the prototype's (optimistic-replay-results.json): a third form of code in which
each such instruction takes the next number of one sequence for the process, logs it for its thread,
and in a replay waits for its number. It is a form of its own in the cache key (7.2). The form before
quoted the prototype's figures (it replayed none of 11 programs on ARM64; 24 to 97 ns an event). The
independent checks of those figures found that the replayer invented failures and that the comparison
was stricter than the owner's definition. The owner's word under "2026-11-19 17:30: optimistic replay
STAYS (owner), and my recommendation to drop it was wrong" is that "`External` stays in the interface"
and that re-measurement is running; it does not name the figures or the halves of `External`. So this
plan only stops leaning on the old figures until the new ones are in. Until the form is built, such a
process runs as `Serial` (section 3), which keeps the same promise. When it is built is decision 3.

**Instructions that read the outside world without a system call** are still open in the interface
notes ("Ruled by the owner, 2026-11-19 afternoon: the system call proxy and the kernel-write hook":
"Still open: signals as an input (delivery point, signal information); instructions that read the
outside world without a system call"; signals have since been ruled, the instructions have not). A
clock read without a system call (`Answer::clock`, rt/lib.rs:120-122) is one. Today a recording logs
it as a host answer of its thread, and the plan leaves it so.

**After the split** (4.7) nothing here changes. No entry is ordered across processes, so an entry that
stays inside its process waits for nobody. (The form before had to keep a thread's charge out of the
shared scheduler for that; with no order of entries it is not needed.)

**What this needs of other tracks.** `Exact` at full speed needs the translator's progress count
(docs/runtime.md section 9, not built), in the form of code a process given `Exact` runs. Until then a
process given `Exact` runs counted turns in which stop checks stand in for the count
(wt/lib.rs:1792-1818), which costs a hook call a check and repeats only at one look-ahead, as the kernel
tests' `repeats` does today. That is for tests. The two kernel primitives are the embedder interface
track's to design; decisions 12 and 17 are what this plan asks of them.

Rejected: comparing only a system call's arguments for `External`. The bytes of a `write` would then go
unchecked, and a process given `Exact` that read them would differ with nobody told.

### 4.7 What stays one at a time, and how it is split later

At first the whole kernel is one at a time. Guest code runs in parallel; kernel entries do not. How much
that costs a build is T1's first measurement (section 13): the time processors spend waiting for the lock.

The split is made only where that measurement points, in this order of expected worth:
1. **A process's own state** under its own lock: its address map, signal handlers, private futexes, its
   threads' records. Calls that touch nothing else then run in parallel between processes.
2. **The scheduler** (run queue, processors, parked threads, overlaps and marks) under its own lock.
3. **Files**: one lock for all files first, then per file system object, as Linux has.
4. The process table, timers, sockets, System V objects: each its own lock, when measured to matter.

Each split turns the `Rc` and `RefCell` of that part into `Arc` and a lock, and is measured against the
single-thread system call cost before it lands. Nothing outside the kernel changes: entries already take
`&self`, hooks already get one process and not the kernel.

## 5. The loop and the system

**Today.** `Stepper::step` runs one thread and returns; `drive` repeats it (rt/lib.rs:197-243). The owner
has named the first `run` and the second `run_until_finished` (section 2), and this plan uses his names.
The executor is `&mut`, and one `StopFlag` raises "whichever thread is running" (rt/lib.rs:33-57). The
local system has one timer thread with one deadline (rt/system.rs:41-55).

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
  - Fewer workers than processors is not designed (known fault 3). The tests' seeded order does not
    need it: there the test system makes `run` return after each turn and says which processor is
    served next, and its one worker serves it (section 11).
- Two callers naming one processor at once is a caller's error, which the kernel answers instead of
  today's assertion (kernel/lib.rs:306-309).

`run(processor)` keeps handing that processor threads until it has a reason to return, and the reason is
the owner's `StopReason`, read per worker:
- `Idle { until }`: this processor has nothing to run. `run_until_finished` waits in `System::idle` and
  calls `run` again;
- `Finished { status }`: the container's first process has ended. Every worker gets it;
- `Suspended`: a hook that ran on this worker took its own thread out, with `suspend_tasks` or
  `suspend_process` (section 6);
- `Interrupted`: this worker was asked out (a container stop, below).

Rejected: a pool of worker threads hidden in the runtime. An embedder with its own event loop (the owner's
other project drives many kernels) must decide which of its threads work for which container. Rejected: a
second loop for the parallel case. One `run`, entered N times.

What changes with it:
- `Execution::run(&self, ...)` (rt/lib.rs:128-143): several host threads are inside it at once, each
  with its own thread. The executor does its own locking (section 7).
- **The system is told who.** `turn` is told the processor, the thread and the guarantee the thread's
  process was given, `ran` the thread, `arrived` and `idle` the processor (today none is,
  rt/lib.rs:150-164). A replaying system needs the thread to find the turn's recorded end, and the
  guarantee to know whether the turn can be counted (4.6). The local system needs the processor for its slice's end.
- **Idle, without a lost wake.** `next(processor)` that answers `Idle` marks the processor idle, under
  the lock. The rule is a condition on the kernel's state, checked whenever the kernel's lock is let
  go: **no processor is marked idle that `next` would hand a thread to.**
  - The plan's earlier form woke a processor when an entry made a thread ready. Its fourth check
    showed three ways a thread that is ready already comes to be runnable on an idle processor with
    no thread becoming ready: the mark of a call-in is taken off (4.5); the running thread of a
    `Serial` process leaves its processor, and a sibling pinned elsewhere may now run; a ready
    thread's mask gains a processor. Section 3's rule adds more: a `Serial` task that leaves lets its
    `Parallel` overlappers run, and one that stops being due a turn takes its mark off them. A list
    of events will miss the next one. A condition cannot.
  - How it is checked. The kernel walks the idle processors from the lowest number and asks, for
    each, what `next` would hand it, counting each thread once and, for tasks that run as `Serial`
    and overlap, one running place between them. Each processor that would get a thread has its mark
    taken off and is announced (suggested: a hook `on_processor_needed(processor)`). This is after the
    entry's own hand-out, so a thread the entering worker took for itself wakes nobody.
  - The system's `idle(processor, until)` returns at the time, at an event from outside, or at that
    announcement. The announcement must be kept if it comes before the worker has reached `idle`:
    `idle` then returns at once. This is the rule of `std::thread::park` and `unpark`, and the local
    system uses exactly those. The mark is made under the lock and the condition is checked before
    the lock is let go, so no wake is lost. A worker woken for nothing asks `next` again and is told
    `Idle` again.
- **No ready thread waits with nobody due in.** This is the other half of the same condition, checked
  at the same place: **every ready thread that could be chosen has a running thread in its way that is
  due in, unless an idle processor is being woken that it could run on.**
  - *Which ready threads.* Those `next` could choose if a processor came free: every ready thread that
    runs as `Parallel`, and every ready thread that runs as `Serial` and is due a turn (section 3,
    rule 3), whether or not an overlapper of it runs now. Other ready `Serial` threads wait behind the
    one due a turn. Of a memory that is called in, only its waiters count: they are chosen whatever is
    called in (4.3), but for a `Parallel` waiter while a `Serial` overlapper is handed out. Its other
    threads wait for the marks, which come off after a bounded time because the waiters are served
    (4.5).
  - *In its way.* A running thread stands in the way of such a thread when the ready one could run if
    the running one left its processor.
    - The ready thread runs as `Serial` and is due a turn, and overlappers of it are handed out: those
      overlappers are in its way, and only they, whatever processor they are on and whatever the masks
      say. The ready thread cannot run while any of them is handed out.
    - The ready thread runs as `Parallel`, and a `Serial` overlapper of it is handed out: that
      overlapper is in its way, whatever processor it is on (the mixed-ordering check, section 2 of
      its report).
    - Otherwise: every running thread on a processor that the ready thread's mask holds.
  - *Due in.* A running thread is due in when it was handed out with a fair slice, or the kernel has
    asked it in with `stop_thread` and it has not come in since. The kernel keeps both facts itself,
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
    the one in its way with the most weighted time, with `stop_thread`. For a `Serial` thread due a
    turn behind running overlappers, those are its overlappers, and every one of them still handed out
    is asked in (they are called in, section 3, rule 2). While one of them is due in, nobody is asked:
    those come in at their slices' ends, and rule 3's mark keeps each from being handed out again
    (section 3, rule 3, which says the same).
  - *A waiter is served at once.* For a waiter, a thread in its way that has only a slice does not
    count as due in: the kernel asks one in with `stop_thread` when the waiter is woken. Its
    overlappers stand still until the waiter's call has run (4.5), and a slice can be milliseconds.
    Counted "thread asked in for a waiter".
  - **The plan's earlier form left a thread waiting for ever behind its own sibling, with one
    processor too.** It asked somebody in only for "a thread ready that its group lets run", and a
    `Serial` group with a thread running lets none run. The fifth check's example: thread A of a
    `Serial` process computes alone, with no slice. Its sibling B sleeps, or waits in `read` on a
    pipe another process writes. `Event::Timer`, or the other process's entry, wakes B. B was counted
    nowhere, nobody was asked in, and A ran on. Go's background thread is exactly such a B (Go's
    runtime, proc.go, `sysmon`), so every Go program run `Serial` would have lost its
    preemption or hung. Now A stands in B's way and is not due in, so A is asked in.
- **What the two halves cost, and how they are checked.**
  - The kernel keeps one mark, "the scheduler's state was written", set by every write to what the
    condition reads: the run queue, a processor's thread and its idle mark, whether that thread is due
    in, a mask, the overlaps, which tasks run as `Serial`, and the call-in marks. Each is written in a
    few functions, and those set the mark. When the lock is let go, the kernel tests
    the mark. Clear: nothing more. A system call answered in place for a thread that carries on
    writes none of these, so it tests one value (4.2).
  - Set: the mark is cleared, and three counts say whether a breach is possible at all. They are the
    idle processors, the ready threads that could run now (a memory that is called in has its waiters
    and no others; a `Serial` thread with an overlapper handed out has none), and the running threads
    that are not due in. The first half is walked only with an idle
    processor and a thread that could run now. The second only with a ready thread and a running thread
    that is not due in. On a busy container every thread was handed out with others waiting, so all are
    due in. With fewer threads than processors the queue is empty.
  - This is not the list of events the fourth check faulted. The mark is set where the data the
    condition reads is written, not by a guess at which changes matter. And a build for tests ignores
    the mark and the counts: it makes both walks at every lock release, and counts "a processor left
    idle with a thread it could run" and "a ready thread with nobody due in", which must both stay
    at zero.
  - The same build checks the rule the scheduler exists to keep: at every hand-out of a thread that
    runs as `Serial`, no task that overlaps it is handed out, and at every hand-out of any thread, no
    `Serial` task that overlaps it is handed out. Counted "Serial task handed out beside an
    overlapper", which must stay at zero.
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
    its own running sibling in a `Serial` process too. A signal it sends reaches its thread by 4.5. So
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
  The system is handed the handle as it is handed the flag today (rt/lib.rs:164-166). Its `stop_thread`
  service and its timer thread call it. The timer thread keeps one slice's end and one thread per
  processor, as `turn` told it, and the container's one deadline, as the hook above told it.
- **A thread never leaves without its exit**, with two exceptions: the thread whose own entry ended it,
  and a thread taken out of the kernel under it by `suspend_tasks` or `suspend_process` (section 6).
  Today a thread the kernel dropped breaks out of `Execution::run` with no `Kernel::exit`
  (wt/lib.rs:2303-2306). By 4.4 a thread ends only in its own entry, so the processor's running thread is
  always cleared by that thread's own `exit`, and `next` finds it so.
- **A container stop.** `System::stopped` runs when no thread is in `Execution::run`
  (rt/lib.rs:156-158), which a save needs. With several workers the loop first asks every running thread
  in, waits until every worker is out of `Execution::run` and no entry is in progress, then calls it
  once. This is also how an embedder interrupts a running container from another thread: each worker's
  `run` then returns `StopReason::Interrupted`. The owner has since ruled its form (interface notes,
  2026-11-19: creation hands out a container and an interrupter, and `interrupter.interrupt()` may be
  called from any thread); this plan asks only that it reach every worker. Every thread has stopped at
  an entry of its own, at a position.
- **Reading one process, and acting on it, from outside a hook.** The interface notes say
  `kernel.process(id)` is for "between steps" (line 55), and promise that "an embedder that wants to
  work asynchronously returns and finishes later through ordinary calls" (lines 60-61). With several
  workers there is no moment between steps unless the whole container is interrupted. Reading is put
  to the owner (decision 8); acting is now partly settled by the ruling on suspend and resume by task.
  - **Reading needs no stop.** `kernel.process(id)` answers a handle at any time, from any thread
    that can call the kernel. In a host program that is any host thread. On the native executor the
    kernel and the embedder's system are inside the virtual machine, and it is the system's code
    there, on any virtual CPU; a program on the host cannot borrow the kernel's data and asks through
    the link (section 10). The handle holds the kernel's lock while it lives, as a hook's handle
    does, so it is kept short.
    `id`, `info` and every accessor of `ProcessInfo` (memory mapped, distance to the ceiling, compute
    used, the config given) read the kernel's own records. They touch no guest memory and no thread,
    and the lock keeps them whole. An embedder that decides where a process should run needs exactly
    these while the process runs (interface notes lines 11-14). (The form before also offered "the
    config in force" and "which processes share its memory". The first is gone with the config in
    force; the second is replaced by the shared-memory hooks, from which the embedder builds its own
    view.)
    - Asked from the host thread that is inside a kernel entry, it answers an error: the notes rule
      that a hook acts only on the process it is handed (lines 76-77). That covers a hook,
      `on_processor_needed`, `on_deadline_changed`, a `Host` service and `arrived`. The kernel can
      tell because its lock knows its holder (4.2). A lock that did not would leave the call waiting
      for ever for a lock its own thread has.
    - Asked from inside `ran`, the clock's answer or `cpu_warning` when those are called from guest
      code, it panics and names the call (4.2). The worker has no lock there, so this is not the
      case above, and it would hang a group stop.
    - It changes nothing, so a replay does not repeat it. What the embedder then does about what it
      read is an entry of its own.
    - Compute used is as of each thread's last entry. A running thread's time since its hand-out is
      added only if that accessor is asked for, as the notes say of anything that needs computing.
  - **Acting needs the right threads stopped, and one rule says when the handle can do it.** Every
    other operation touches the memory or the threads: `read_memory`, `write_memory`, `watch_memory`,
    `take_dirty_memory`, `send_signal`. On any handle, from a hook or from `kernel.process(id)`, it is
    carried out with whatever stop 4.5 asks for, when that stop can be made in place: nobody, or a
    group stop. It answers an error where it would need a call-in, because a call-in parks its caller
    and a handle cannot wait: a process whose threads run as `Serial` with one of them handed out, or
    an `Exact` process with other threads running where the act needs them at positions.
  - **`wake_waiters` and `move_waiters` stop nobody** and touch only the kernel's waiter lists, so they
    never answer that error. This plan calls them on a handle of the kernel that holds the waiters,
    from `kernel.process(id)` outside any hook, and never from a hook for another kernel, because that
    would wait for the other kernel's lock inside a hook (4.5; proposed, to be confirmed by the owner).
    The embedder also never holds one kernel's handle while it calls or waits for another (4.5; the
    plan's own rule, from the second check's eighth finding). The interface notes say only "on the
    process handle". 4.5 says the same.
  - **For those cases the embedder suspends the tasks first.** The form before added a call and a hook
    for this (`Kernel::process_call_in(id)` and `on_process_called_in(process)`). They are withdrawn.
    That the ruled `Kernel::suspend_tasks` does the same job is this plan's design, not the ruling's:
    the ruling says the batch is taken as of one moment and "how the kernel stops them is internal".
    In this plan's design, called on the `Kernel` from outside any hook, it may wait, so it asks the
    named tasks in where 4.5 says so and returns when they are in (section 6). The embedder then acts
    on the process with nobody of it running, and puts the tasks back with `resume_tasks`. In the
    owner's other project a thread left waiting by `on_watched_memory_accessing` is let go when its
    page arrives from another machine: that is a `write_memory` with nothing to stop when the process
    has no other thread running, and a `suspend_tasks` of the running ones otherwise.
    - What it costs: the tasks' way in and back (300.8 ns each measured for leaving `Execution::run`
      and coming back, section 3, and the stop check's latency, 230 ns on average measured in Chrome),
      and the records taken out and put back. The second is not measured; nothing to measure it exists.
      The batch's other tasks, and every task not named, keep running (section 6).
- `Host` and `System` methods take `&self`, so a system keeps what it changes in atomics or behind its
  own locks. The shipped system needs no lock on a system call (4.2).

**Two proposals of the embedding project, and what each would mean here.** When the form before was
written the owner had ruled on neither. Both are now in his rulings of 2026-11-19: every hook in the
signatures he ruled takes `&self`, and per-process state has a ruling of its own ("Ruled by the owner,
2026-11-19: per-process state"). That `System` is also `Sync` does not follow from them, and stays
decision 9.

- **Hooks and services on `System` take `&self`, so that two hooks can run at once.** This plan asks for
  the same (decision 9), and needs it earlier than two hooks at once. From T1 on, several workers call
  `turn`, `ran`, `idle` and the clock's answer at the same moment, outside the kernel's lock. Hooks and
  `Host` services run under the kernel's one lock until it is split (4.7), so no two run at once in
  T1. After the split, two hooks about processes that share nothing can, with nothing in the
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
- **Per-process state**, ruled ("Ruled by the owner, 2026-11-19: per-process state"): `type
  ProcessState: Clone + Send` on the embedder's trait, one value kept with each process and handed over
  with it, mutable with no lock, cloned for the child at fork, staying with a suspended process, not
  part of an exported process (the importing side supplies its own), and free when it is the empty
  type; "not to be built until the interface is rewritten in the reviewable-rust layout". The later
  rulings use it in their signatures (`P: Process<State = Self::ProcessState>` in the shared-memory
  hooks, `type State` on `Process` in the ruling on suspend by task). What the ruling does not say is
  which value goes with what since suspend became by task: "stays with a suspended process" reads as
  staying with the `ProcessContext`, which this plan takes; a `SuspendedTask` holds none. That reading
  is the interface track's to confirm.
  - It fits this plan's locking as it is. A `Process` handle exists only while the kernel's lock is
    held: a hook's, and one from `kernel.process(id)` (above). After the split it exists while the
    process's own lock is held (4.7). So whoever has the handle has the state alone. That holds with
    `Parallel` code too: the process's other threads may be in guest code, but none is in a hook.
  - What locking it takes off an embedder: whatever it keeps about one process and touches only
    through that process's handle. Examples are where it decided to place the process, its record of
    the process's watches and page hand-overs (section 8), and the process's replay log, which the
    proxy's hooks are handed the process for (4.6). With the state these need no lock, and no table
    from id to record.
  - What it does not take off: everything that is not about one process. That is the words `turn`,
    `ran` and `idle` keep per processor, the container's deadline, its picture of other processes,
    which a hook cannot look up (interface notes lines 76-77), and its counters.
  - Three of this plan's calls are not handed a `Process`: `stop_thread`, `on_processor_needed` and
    `on_deadline_changed`. None is about one process's record. (The form before had two more that
    were, `kernel_entry_admit` and `on_kernel_entry_ended`; both are withdrawn, 4.6.)
  - What it asks of this plan: the ruled bounds, `Clone + Send`. The kernel's state moves between host
    threads under the lock (4.2), which needs `Send`; the kernel clones the value at fork, which needs
    `Clone`. It need not be `Sync`: one host thread has it at a time.

## 6. Changing the config of a running process, and taking tasks out

The brief asked for two forms to be weighed, a setter and no setter. A later section of it settled it:
there is no setter. The weighing is kept, because it is what the rest of this section stands on. Since
then the owner has ruled how a process is taken out and put back ("suspend and resume by task, and the
process context"), which replaces `Process::suspend(self)` and `SuspendedProcess<E>` everywhere below.

**Form A, a setter** (not ruled in). `Process::set_thread_ordering(ordering)`, taking effect at the
process's next stop.

**Form B, no setter** (ruled). A process's config is fixed while it is in a kernel. It is given when the
container starts and when a process is resumed: `resume_process(context, config)` takes it, and it is
never stored in the value. To change it on the same kernel, the embedder takes the process out with
`suspend_process`, which hands back its `ProcessContext` and every task of it still on this kernel,
then puts the context back with `resume_process(context, new_config)` and the tasks with
`resume_tasks`. Nothing is converted and no memory is copied: the records move out and back by
ownership. The form that crosses a wire is not part of it. That meets the orchestrator's condition for
agreeing (the brief, lines 91-94): the memory moves and is not copied, and no ends are made for what the
process has open.

One thing this plan asks of the values, so that they may be held, moved and dropped on any host thread:
they share no `Rc` with what stays in the kernel (4.2). On the same kernel that is still no conversion:
the resume calls put the same references back.

What each costs and makes simpler:
- **Work in the kernel.** A: the setter. B: nothing beyond the suspend and resume calls, which the
  embedder interface builds for other reasons.
- **Work in the executors.** The same in both. A change of ordering or of guarantee can change the form
  of code the process runs (7.2), so its instances are made again; the native executor sets the
  process's page tables up again (native.md section 5.3). In B this is on the path of `resume_process`,
  which must do it for a process come from elsewhere too.
- **Cost of one change.** A: the threads called in, once. B: the stop that taking the process needs
  (below), then the process's records taken out of the kernel's tables and put back (its threads out
  of the run queue and the waits, its timers, its place in its parent's children). Their count is the
  process's threads and waits, not its memory. From inside a hook the embedder can take the process
  out but must resume after `run` has returned ("Resume stays outside hooks").
  - What that return costs was measured by the interface track, on a prototype
    (orchestrator/interface-measurements.json, part "run-return"; this Mac, load 56 to 184). At a
    program start on the WebAssembly executor it costs nothing that could be measured: `run` already
    returns there at every exec, because exec gives the program a new store (20,001 returns for
    20,000 execs with and without the suspend). Returning and resuming, against changing the process
    inside the hook, differed by under 100 instructions of the 6.3 million a program start takes.
  - Elsewhere it costs that worker one leave of `Execution::run` (300 ns, docs/runtime.md section 10).
  - Not measured: real suspend and resume calls with the records moved out and back, and anything on
    the native executor. The prototype only took the process's threads out of line and put them back,
    which cost 1,100 to 1,400 instructions.
- **A consistent starting point for a copy.** A change towards `Exact` starts a promise. Since the
  ruling on where replay is recorded, a copy starts from a memory transfer and runs on from the
  process's recorded system call results (4.6). Taking the process out gives the moment at which
  memory and the kernel's records are one consistent view. With A the embedder must take that moment
  anyway.
- **Ways a process gets a config.** A: two. B: one. A reader of an embedder's code sees every change of
  what replay promises as a `resume_process` with the config written on it.

Weighed so, the plan recommends B, which is what was ruled. What B costs is that each change moves the
process's records out and back, and that a change made inside a hook waits for `run` to return. The
second was measured as nothing at a program start, which is where a config is chosen.

**How the kernel takes a batch of tasks.** The ruling leaves it to the kernel ("one atomic batch, taken
as of one moment; how the kernel stops them is internal"). A task that had already exited is simply
absent from what comes back, and a thread started just before the call is not in the batch and keeps
running. `suspend_process` is the same for every task of the process, with its context. What the
kernel does depends on where it is called from, because a call-in parks its caller and the tasks come
in through entries that need the kernel's lock (4.5):
- **On the `Kernel`, from outside any hook** (this plan's design; the ruling says only "how the kernel
  stops them is internal"): the call may wait. For each memory of the batch it makes the stop 4.5
  names: nothing when none of the named tasks is handed out, a group stop where it may hold them, and
  otherwise it asks in **the named tasks only**, after which it waits, outside the lock, until they are
  in. Then it takes the batch in one entry. This is the path an embedder uses for a running `Serial`
  or `Exact` process.
  - **It marks named tasks, not memories, and every named task.** Each named task gets a mark of the
    batch's own at the call's start, whether it is handed out, ready, or waiting: a marked task is not
    handed out, and a marked waiter is not chosen, so its call is not run again here (it is taken with
    its call, below). A handed-out one, when it comes in, is not handed out again. Nothing else of its
    memory is held back. (The first correction marked only the named tasks that were handed out. The
    second check, its fourth finding: a named task that was ready, or woken, but not handed out when
    the call started had no mark; another worker could hand it out while the call waited outside the
    lock, and the take "in one entry" then met a running `Serial` or `Exact` task, for which the plan
    had no rule.) Unnamed siblings keep running, and so does "a thread started just before the call", which
    the ruling says "is not in the batch and keeps running". The mark comes off when the batch is
    taken, in the entry where the last named task has come in, or when the last named task still out
    has ended (it is then absent from what comes back, as the ruling has it). It belongs to no guest
    call, so it is not a call-in's mark, and 4.5's "why no circle" is not touched: nothing waits for a
    task that has come in. (The form before put a call-in's mark on the memory. The revision's check
    found that it had no rule for coming off, and that it kept every thread of the memory out of
    guest code, unnamed ones included.)
  - A named task that is waiting is taken as Linux treats a signal with no handler arriving during
    its call (4.5, "a task taken out while it waits"; proposed, to be confirmed by the owner; decision
    19): a call that has had no effect yet (a wait for a call-in, whose marks come off; a futex wait,
    taken off its waiter list) runs again where the task is resumed; a wait with a time limit goes on
    to the same deadline; a call that has had an effect returns what it did; a waiter already chosen by
    a wake returns woken.
  - The call does not wait for its own host thread. The revision's check asked about an embedder that
    calls `run` itself and calls `suspend_tasks` from the host thread that serves the named task's
    processor. That host thread is then outside `run`, and `run` returns only for one of the four
    reasons of section 5, each after the thread it ran has come in through its own entry. So no task
    is handed out on that processor while its worker makes the call, and the call never waits for it.
    The named tasks it waits for are handed out on other processors, whose workers bring them in.
- **On a hook's handle, the process running as `Serial`**: no other thread of it runs (4.4 item 7).
  Nothing to stop. The hook's own thread is in its entry, with its registers saved.
- **On a hook's handle, the process with `Parallel` code and not `Exact`**, and no other owner of its
  memory given `Exact` (`CLONE_VM`; otherwise as the next case but one for those threads): the named
  tasks may be in guest code. The kernel asks the executor for a group stop of the memory and takes
  them while every thread is held.
  - A held thread is at a stop point, so all its registers are saved (4.5).
  - A thread held with an exit it has not yet handed to the kernel (it was waiting for the lock) is put
    back to the instruction that made the exit. It makes the exit again wherever the task is resumed,
    as Linux runs an interrupted call again.
  - When the group stop ends, each held thread that was taken finds its task gone, as a dropped thread
    does today (wt/lib.rs:2303-2306), and leaves `Execution::run`. The kernel has already cleared its
    processor. Threads that were held and not taken carry on.
  Counted "memory stopped to take tasks out".
- **On a hook's handle, the process given `Exact`, with a named task other than the hook's own
  running**: the task must stop at a position, which only a call-in gives, and a hook cannot wait for
  one. What the batch does then is open: the ruling lists it ("the batch that cannot wait inside a hook
  when it names other running tasks of an exactly recorded process"), and its return has no error and
  reads a task left out as one that had exited (decision 11). When none of the named tasks runs, it
  works as for a `Serial` process.
- When the hook's own thread is in the batch, that worker's `run` returns `StopReason::Suspended`
  (section 5). Taking some tasks does not use up the handle; taking the process does.

So the kernel never stops a thread that runs as `Serial`, or of an `Exact` process, in place, and item 5
of 4.4 holds. Two things it does to the rules, and only in the group-stop case: a thread that is taken
leaves `Execution::run` without an exit of its own, and is taken, not ended, outside its own entry.

**One task apart from its siblings.** Per-thread distribution is core (section 2). `suspend_tasks` of one
task leaves its siblings running here and the process's context on this kernel. The task runs again
only where a context for its process exists ("a task can only run where a context for its process
exists (embedder coordinates)"); `copy_process_context` makes one for another kernel without stopping
anything, because the context changes only inside system calls, which are entries under the lock.
Keeping several copies in step is the embedder's: it sees every change through the proxy. On this kernel
the scheduling rule then counts one task fewer, and what the moved task shares with its siblings is the
embedder's, through watches, as for a process on another kernel (section 3). That is enough only where
the moved task and every task it overlaps may run at once: the task runs as `Parallel` and none of its
overlappers runs as `Serial`. Two tasks of a `Serial` process on two kernels would each be handed out by
their own kernel and run `Serial` code beside each other (section 3, "the rule ends at the kernel's
edge"; known fault 9). How placement keeps such tasks taking turns is decision 20.

**A call that changes the config inside a hook is not in this plan.** The interface notes proposed one
(`Process::replace_execution_config` in the brief), and an earlier form of the plan recommended it, to
save the return of `run`. That return has been measured as nothing on the WebAssembly executor (above),
so it is not needed for speed there. On the native executor it was not measured; at worst it is one
return of `run` in the life of a process. The plan has no other need of it: no config changes inside
the kernel now (section 3), and the two moments an embedder chooses a config, a program's start and a
process's first new thread, both have a hook in which taking the process out and resuming it after the
return does it. So there stays one way to give a process a config, `resume_process`, and it is off
decision 9's list.

**Processes that own one memory.** Its form of code is `Serial` code when any of them was given
`Serial` (section 3). To make it `Parallel` code, every owner given `Serial` is taken out and resumed
with `Parallel`. The form before also had a "group of several processes", with the strictest config
winning; both are gone, and a process that only shares a block with others keeps its own config
whatever theirs is. (The form before also said how a config was read "in a recorded kernel"; that
kernel is withdrawn, 4.6.)

**What is built here.**
- The container's first process gets its `ExecutionConfig` at the start (through `Start`,
  kernel/container.rs:55, read by `Kernel::start`). A child inherits its parent's at fork and keeps it
  across exec.
- Until the suspend and resume calls exist, that is the only way to give a config. This loses less
  than it seems: a memory is born with one task (section 3), where the ordering makes no difference,
  so "this container's threaded programs run in parallel" or "everything in this container replays
  exactly" is one value at the start.
- Choosing per program arrives with the suspend and resume calls, at `on_program_starting`, which the
  owner already named as the cheap moment to move a process: one thread, a fresh memory, few records.

## 7. The WebAssembly output for a process whose threads run at once

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
  is fixed while the process is in a kernel. `resume_process` makes the instances again in any case.
- **A process that runs as `Serial` with `Parallel` code** (the executor's limits of 7.4 and section 9,
  or (`Parallel`, `External`) until its form is built; section 3) keeps running the Parallel form. That
  is correct: the Parallel form is right under both orderings, and only pays for it. Counted "Parallel
  form run under Serial".
- **A process with one thread, given `Parallel`**, runs the Parallel form from its start. Its byte is
  zero (7.3), so a barrier or a 128 bit atomic costs it one test, and the compiler setting costs it the
  0.0% to 1.8% above. Rejected: the Serial form until the process's second thread, then the Parallel
  form. It spares programs that never start a thread. But every runtime that starts threads at once
  (Java, Node, Go) would have its start-up code translated and compiled twice, and it is one more path.
- **A memory has one form.** Processes that own one memory together (`CLONE_VM`) run the Parallel form
  exactly when every one of them was given `Parallel`. It is worked out again when the owners change,
  and a change is made with the memory's threads called in. With `Serial` code all their threads take
  turns, by the rule of section 3: that is the one case where a neighbour's ordering reaches a
  process, because a second form on one memory is not possible.
- **Two wasmtime engines** in the executor, one for each form. They differ in one compiler setting
  (7.6). A process's stores, modules and memory belong to one engine.

**The cache key** (wt/lib.rs:107-115, :183) changes once for the shared import and its declared maximum,
through `OUTPUT_FORM`, and gains two fields:
- the form by ordering, Serial or Parallel;
- whether the code holds the progress count, which is exactly "the process was given `Exact`" (section
  2). A neighbour's guarantee never changes it, so no process is translated again for a neighbour. The
  count is the translator track's to build and measure; this plan only fixes that it is a field of the
  key and never a switch of its own.
  - Under `CLONE_VM` this field is per process while the form by ordering is per memory (the revision's
    check found the two sit uneasily; it marked that part plausible). So that a co-owner given `None`
    never runs code with the count, the executor keeps code per owning process on a shared memory: each
    owner's threads run modules of its own key, on the one memory and the one engine of its form. That
    is this correction's design, not built or checked. Each thread already has its own instances and
    lookup table (7.5), and its page table is an imported value per instance (section 8); whether
    the array of slots, kept per process today, can stay per owner on one memory is T4's to show. The
    other answer, the count per memory, would make an owner
    given `None` pay 11.9% to 16.3% more instructions (measured, section 2) for its neighbour's
    guarantee, against "where replay is recorded".
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
    - For a thread that runs as `Serial` the worker that runs it takes it when it enters
      `Execution::run` and keeps it until it leaves, as it takes the store out today
      (wt/lib.rs:2245-2251). Its hooks use what it already has. It asks for the kernel's lock under it
      at every system call. That is the one exception to the lock order (4.5), and it is safe because
      nobody who has the kernel's lock ever waits for this lock: a worker that does not have it only
      ever tries it. Six rules make every try succeed, and the try checks them:
      - the kernel reaches a memory that runs as `Serial` from outside only after calling it in (4.4
        item 5);
      - two memories come to overlap only once every handed-out task that would break rule 1 after
        the change has come in, the caller's own siblings included where its memory comes to run as
        `Serial` (section 3, "how a new overlap takes effect"), and a task that runs as `Serial` is
        handed out only when no overlapper is (section 3, rule 1). So two overlapping memories never
        have two workers that each hold a process's lock for a running thread where one side runs as
        `Serial`, and one memory never has two such workers. A change in how a memory runs, either
        way, is made only with none of its other threads handed out, and the thread whose entry makes
        it is not told to carry on: it leaves, and is handed out again under the new way (section 3, "a
        change in how a memory runs"). So no worker keeps a `Serial` thread's lock for a thread that
        now runs as `Parallel`, and none runs a thread as `Serial` without it;
      - **a worker lets its process's lock go before it lets the kernel's lock go, whenever its
        thread is not told to carry on.** A thread is told to carry on only when it is handed out
        again in the same entry, and a thread of a memory that is called in is not handed out. So
        whoever takes the kernel's lock next finds no lock held for a thread that has come in.
        How the order is kept: the answer that tells the thread to leave hands the executor the
        kernel's handle with it (`Carry::Leave(why, entered)`, 4.2). The executor puts the thread's
        store back and lets the process's lock go, and only then drops the handle. Today's code
        has it the other way round: the kernel entry is over inside `Answer::exit`, before the
        executor learns that the thread leaves (rt/lib.rs:264-289, wt/lib.rs:2325-2344). The
        plan's earlier form had the thread let the lock go when it found its asked-in bit set, before
        the entry. Its fourth check showed the gap: the bit can be set after that look, while the
        thread waits for the kernel's lock. The thread then came in with the lock, the kernel woke
        the call that wanted the memory, and that call could run on another worker before the lock
        was let go. The rule now does not look at the bit;
      - **a `Cpu` call made inside an entry uses the lock when the worker that makes the entry has
        it.** That covers the thread's own calls. It also covers what else the kernel does in that
        entry: the call that wanted the memory in may run again right there, on the worker of the
        thread that came in last, and reach that thread's memory. That worker is in the kernel, not
        in guest code, and no other thread of the memory runs;
      - the kernel does a thread's work only when the thread is chosen for a processor (4.3). So a
        woken call of a thread that runs as `Serial` never runs again, and no signal frame is written
        to its stack, while an overlapper runs and its worker has the lock. That holds for a waiter
        too: it is chosen whatever is called in, but never while an overlapper is handed out (4.5);
      - **a write to a block reaches every memory that shows it, and each of their locks is free or
        the worker's own.** `block_write` puts the bytes in the block and in every memory that shows
        them (wt/lib.rs:1409-1433). It has two callers, and neither leaves a memory out:
        - the copies a `Serial` thread's written shared pages get when it leaves (`flush`,
          wt/lib.rs:1341-1388, :1376). The block is writable through a mapping, so every memory that
          shows it overlaps the writer's (section 3). The writer runs as `Serial`, so none of them
          is handed out (section 3, rule 1), and their locks are free. This is the ordinary
          case of a parent and its forked child sharing a mapping, and it stays as today (7.4);
        - a write of the kernel's, for a `write` to a file that memories map. A memory of the
          caller's process has its lock free or held by this worker. A memory of another process
          that runs as `Serial` has been called in first (4.5), so its lock is free. A memory that
          runs as `Parallel`
          takes the bytes with its stop lock shared (4.5).
        The plan's earlier form said such a walk "skips every other memory in force `Serial`". As a
        rule about where bytes are written that was wrong, and it contradicted 7.4 and 4.5: the
        fifth check showed that a child would stop seeing its parent's stores, and that a file's
        new bytes would not reach a memory called in to receive them. The reason given there holds
        for a memory as a **source** only, and that is all that is left of it: nobody reads a
        `Serial` memory of another process to bring a block up to date. A `read` of the file takes
        the block's bytes as they are, which hold all that memory wrote before its last entry (7.4);
      - so on a worker that does not have it, taking a `Serial` process's lock is a try-lock. A
        failure is a bug, and it panics, naming both threads. The stress schedule of section 12 runs
        with it, and one test there asks a thread in while it waits for the kernel's lock.
    - For a thread that runs as `Parallel` each hook takes it for as long as it needs it, and never asks
      for the kernel's lock while it has it.
  - A block's bytes go behind that block's own lock. `block_write` finds who shows a block by going
    through every process (wt/lib.rs:1413-1414). It may no longer read the records of other processes,
    so each block keeps the list of the memories that show it.
  - Order: the kernel's lock, then a stop lock, then a process's lock, then a block's, with the one
    exception above.

### 7.3 Atomic instructions, exclusives and barriers

All of this section is about the Parallel form. The Serial form keeps today's translation of each of
these instructions.

**The byte.** Each process has one byte in its state memory, at a place the translator's contract fixes
(suggested name `runs_in_parallel`). It is 1 when the process's memory runs as `Parallel` and has more
than one thread, else 0. It follows the memory's own threads, not its overlappers (the mixed-ordering
check: in the form before it followed the group): a `Serial` task beside it never runs at the same
time, and its code does not read the byte. The Parallel form reads it with a plain load, as it reads its
stop word (tr/code.rs:1300). It is what lets a process that has one thread, or was forced to `Serial`,
run the Parallel form at nearly the Serial form's speed.

The kernel tells the executor each memory's form of code, whether it runs as `Serial`, which of the
processes that own it were given `Exact`, and whether it has more than one thread (suggested
`Cpu::set_execution_config(memory, thread_ordering, exact_owners, several_threads)`, with the ordering
it runs as). From it the executor sets the byte, and knows which primitive of 4.5 may reach which of
the memory's threads: a group stop whenever it runs as `Parallel`, with one thread too, because a file
that changes size reaches the memory from another process, but never for a change of the kernel's on a
thread of an owner given `Exact`, which is called in (4.5). The call is made inside the entry that
makes the memory's second thread or changes how it runs, when every other thread of the memory is stopped or
about to start.

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
    process, because the output is the same for every host. How often real programs run them is T3's
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
    tracer, and the embedder's own `write_memory` (section 5). One that landed between the load and
    the store would be lost while the instruction reported success. Linux forbids that: the
    instruction is atomic against the kernel's copy too. So such a write into a memory whose byte is
    0 is made under a group stop of that memory. Its one thread is then held at a stop point, and
    the inline form has none inside it. With the byte at 1 nothing changes: the write has the stop
    lock shared and the hook has it exclusively. Counted "memory with one thread stopped for a write
    from outside".
    - It costs that write one stop of one thread (230 ns on average by the Chrome measurement of
      4.5), and only when the target runs as `Parallel`, has one thread and shows the pages
      written.
    - Rejected: the byte at 1 whenever the memory runs as `Parallel`. Every barrier and every
      128 bit atomic of a process with one thread would then pay the fence and the hook, which is
      what the byte is there to spare.
  Exact, and slow: counted "128 bit atomic made with the memory stopped". How often real programs run
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
- bytes are written with atomic stores, one byte or one aligned word at a time, so a reader never sees
  half a value.

**Blocks are shown by copying.** Each memory holds its own copy of a block's pages. A page that shows a
block carries a write lock; the first write lifts it and marks the page written; and before the kernel
sees anything, at every exit, written pages are copied to the block and to every other place that shows
them, and locked again (docs/runtime.md section 8; `flush`, wt/lib.rs:1341-1388, called at :2322). That
is exact only while one thread of everything that shows the page runs at a time. Three things follow.

- **Tasks whose memories show a page of a block that is writable through a mapping in more than one
  place run as `Serial` on this executor**, whatever was asked, but for the next item. More than one
  place means two memories, or one memory twice (a ring buffer mapped twice side by side). The executor
  states this as a fact about itself (suggested `Cpu::parallel_limits()`: blocks are shown by copying;
  and, for section 9, plain accesses are ordered more weakly than the guest's), and the kernel, which
  knows every mapping, applies it (section 3). Counted "task kept Serial: a shared page is shown in two
  places".
- **A `Serial` task beside `Parallel` ones can alternate with them** (the mixed-ordering check, for
  `MAP_SHARED` and `shmat`, not `CLONE_VM`). Under section 3's rule A, the task that runs as `Serial`,
  and the `Parallel` memories B that share the block with it are never handed out at once. So at each
  switch to A every thread of B has come in (rule 2), and B's written shared pages are copied from B's
  memory to the block and on to A's memory before A is handed out; at A's exits its written pages go
  to the block and into B's memory as today, with none of B running. The two phases never overlap, so
  one copy each way keeps the places in step. `Cpu::parallel_limits()` then says "can alternate",
  which is not "Serial only". B's threads run together in their phase from B's one place. Expected to
  work, not built (known fault 8); until it is, A and B all run as `Serial`. It costs a copy of the
  dirty shared pages at every switch, not measured. The native executor
  maps the same frames in each place (native.md section 5.3) and has no such limit. A real shared mapping
  under wasmtime is possible only where the host's pages are no larger than the guest's (docs/runtime.md
  section 8: this Mac's are 16 KiB); it is not in this plan.
- **In a memory that runs as `Parallel`, a written shared page is not locked again at a turn's end.**
  Such a memory has every such page in one place, and no other place is in use while its threads run,
  so the memory's own bytes are the only ones a guest thread can see. Locking the page again would lower
  a permission, which is a group stop, at every system call of every thread that had written a shared
  page (SQLite's shared index, a shared memory file, shared anonymous memory). Instead the page stays
  written, and the executor copies it to the block when the block's bytes are asked for: `shared_read` (a
  `read` of the file, kernel/fs/mod.rs:585), a new `shared_map` of the page, `shared_unmap`,
  `memory_duplicate`. The copy is taken while threads may be writing the page, as a `read` of a file
  being written through a mapping is on Linux.
  - **That copy goes one way, from the memory to the block's own bytes.** Today's `flush` ends in
    `block_write` (wt/lib.rs:1376), which also writes the bytes into every memory that shows the block,
    the writer's own included (wt/lib.rs:1409-1433, with the comment "no guest runs"). Used here it
    would put a snapshot back over the stores the guest made after it was taken, and a `read` of a file
    would undo another thread's stores. So this is its own, smaller copy, which writes no guest memory.
  - The page stays on the written list for as long as it is not locked, so each later ask copies it
    again.
  - A `write` to the file still goes through `block_write`, into the block and into the memory, as any
    write of the kernel's does (4.5).
  - When a second place comes to show a page, the memory is called in, its written pages are copied
    and locked, and it runs as `Serial` from then on, or alternates by the item above when that is
    built and the other place's task runs as `Serial`. Counted "shared pages locked again: a second
    place shows them". Memories that run as `Serial` keep today's copies at every exit unchanged.

### 7.5 Code a program writes while another thread runs it

- **Dropping code** stops the memory's other threads by 4.5's rule (a group stop, or a call-in for the
  threads of an owner given `Exact`), and every other thread of the memory gives up its
  frames before it carries on. ARM64 announces a change with `ic ivau`, which is already a hook
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
- **A drop reaches only the caller's memory and the memories that overlap it through that block.**
  Today `ic ivau` also drops the same line in every other memory that shows the block
  (wt/lib.rs:2008-2010). A memory that shows the block but does not overlap the caller's shows it
  through no mapping that may write (section 3), so no guest store can have changed its bytes; such a
  block's bytes change only through a `write` to the file, which is the kernel's entry (4.5). Reaching
  it is not needed, and not allowed: its threads may be running, and the executor must not change the
  code of a memory that runs as `Serial` from outside it (4.4 item 5). A memory that does overlap the
  caller's through the block is reached by 4.5's rule: where either side runs as `Serial` none of its
  threads is handed out (section 3, rule 1), and otherwise a group stop of it is made. On this executor
  such overlapping memories run as `Serial` anyway (7.4).
  - **This one is T1's, not T2's.** Processes already run at the same time in T1. A hook that walked
    every memory showing the block would reach the records of a process that overlaps nobody here and
    whose running thread has locked them for its whole stay (7.2). The try-lock would panic.
- **Adding code needs no stop.** Today new code is instantiated in every thread of the process at once
  (docs/runtime.md section 9). With one store per thread, a thread cannot touch another's store. Instead:
  the new module goes on the process's list, its slots are written to the array of slots last, and each
  other thread instantiates it in its own store the first time it needs it. No translated code changes for
  this: a lookup that finds an empty table entry already goes to the load code hook (tr/thread.rs:118-130,
  tr/code.rs:1159), which then finds the module on the list. The list and the slots are written with the
  stop lock shared, as any install is.
- **Growing the tables.** A lookup beyond a thread's table would trap, so every thread's table must cover
  every slot written. Tables grow by doubling, each thread growing its own at a group stop, so a stop for
  this happens only when the process's slot count crosses a doubling. Counted "memory stopped to grow the
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

The owner ruled that exact replay of a process whose threads run at once is composed by the embedder
from `memory_watch_thread`, where the executor implements `PerThreadWatch`. That is the pair
(`Parallel`, `Exact`). Since then he has ruled the process-wide watch as
`Process::watch_memory(&mut self, range, watches: WatchesFor)`, the hook as
`on_watched_memory_accessing(&self, process, thread, address, kind: AccessKind)`, and method names verb
first (section 2). The per-thread call keeps its earlier name here; its verb-first name is not ruled
(decision 10). Replay being per process changes nothing in this section: a hand-over between two
threads of one process is inside that process's log. The kernel's own part is the count and the
call-ins of 4.5. This section is what the embedder's part needs. On the WebAssembly executor:

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
- **Setting a watch stops one thread, not the memory's others.** Only the watched thread's table changes,
  and the rule is that the thread whose table loses a permission must be stopped. It is done with the
  memory's stop lock, with one thread's held bit set, and the thread is held only at a stop point (4.5).
  So a thread that reads the clock in a loop is not taken for stopped while its loop still relies on the
  old table, and no access is missed.
- **The held thread's position can be read.** It is at a stop point, so its registers are saved, and in
  an `Exact` process so is its count. The embedder reads both through the handle (suggested, on a
  thread's info, `position()`; decision 12 asks for the same read in general), which is what its
  record of a hand-over needs. Holding a thread in place is allowed here although the process is
  `Exact` (4.5): the watch only orders memory accesses,
  which is the embedder's part, and in a replay the embedder brings the thread to that position with a
  counted turn before it sets the watch again.
- **A watch on a page a frame could cover.** This is 4.5's rule for a page that loses a permission,
  and the same piece of code. What a piece proved about its own stack frame is checked again only at
  the next function entry (tr/contract.rs:20-23). Threads share objects on their stacks all the time
  (scoped threads, a joined thread's result, a condition variable on a stack). A watch set on a thread
  for such a page would miss that thread's accesses to its own frame, and the embedder's record would
  miss writes. So when a watch takes a permission from a page that allowed reading and writing, each
  thread it is set on gives up its frames when it is let go. A piece entered from the bottom loop
  knows nothing about its frame and checks every access. It holds for `watch_memory` as for
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
  thread's own, calls `on_watched_memory_accessing` with the address and the kind of access (`Read`,
  `Write` or `Execute`). If the watch is gone when the hook returns, the thread carries on.
- **The kernel's own accesses.** `Cpu::read` and `Cpu::write` ignore page permissions (abi/cpu.rs:30-31),
  so no table byte catches a `read` system call filling a page another thread holds. The owner ruled
  that the kernel's own accesses to watched memory fire `on_watched_memory_accessing` too, and that
  watches and dirty marks treat the kernel's writes exactly like the program's own (section 2). So the
  kernel checks these itself, and for that it keeps each process's watched ranges (given to it through
  `watch_memory` and `memory_watch_thread` in any case). A process with none pays one test a call.
  - An access the kernel makes for a thread of the process (a system call's buffers, a signal frame on
    its stack, the words cleared at its exit) is that thread's access. It is checked against that
    thread's watches and the process's before the call has any effect, and the hook fires with that
    thread. If the watch stays, the call parks. After the write, `on_memory_written_by_kernel` fires
    with the exact range.
  - A write that reaches the memory from outside the process (a `write` to a file it maps, reaching
    pages it still shows; `process_vm_writev`; a tracer) has no thread of the process behind it. Both
    ruled hooks take a thread, and which thread they name then is not ruled. The form before
    recommended making it optional, none meaning "the kernel, for another process". That stays a
    recommendation (decision 18). Without an answer an embedder's record of hand-overs cannot say who
    wrote.
- **Space.** Each table is one byte per page under the ceiling: 1020 KiB for ARM64's 4080 MiB, of which
  untouched pages cost nothing. They share the state memory's 4 GiB with the array of slots
  (docs/runtime.md section 8), so the ceiling's formula gains the number of watched threads.

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

**Where it matters.** Only for an x86-64 guest whose threads run as `Parallel`, on the WebAssembly
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

**What is done until it is built.** On such a host the executor reports that it can run an x86 process
only as `Serial` (section 3). Its threads, and its overlappers toward it, run by the rule for a `Serial`
task, counted "x86 process kept Serial: the host orders memory more
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
- **Group stop** is that interrupt to each virtual CPU running a thread of the memory. Each lands at an
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
  - Which primitive is used for which memory is 4.5's rule there too: a memory that runs as `Serial`,
    and the threads of an `Exact` process, are called in for a change of the kernel's, and a
    `Serial` task's overlappers are called in for its turn (section 3).
- **A table root per thread, only once it is watched.** A thread without per-thread watches uses its
  process's root. One with watches has its own root and address-space identifier, sharing every lower
  table that is the same for all (native.md section 5.3; its N1 builds and tests the per-thread root).
  This is `PerThreadWatch` there.
- **Atomic instructions need nothing.** The native output copies the guest's single-instruction atomics,
  compare and swap, load-acquire and store-release, with an alignment test in front (native.md lines
  363-366). The hardware does them, the 128 bit pair included. No hook and no byte.
- **Barriers: two forms by ordering, as on the WebAssembly output.** The native output emits no
  instruction at all for what the decoder calls nothing, `dmb`, `dsb` and `isb` among them, and leaves
  their route "decided with the WebAssembly output's" (native.md lines 350-362). Decided here: the
  native output has the same two forms of code, under the same field of the cache key (7.2).
  - **The Serial form emits nothing for `dmb`, `dsb` and `isb`**, as native.md has it today. A task
    that runs as `Serial` is never handed out beside an overlapper (section 3, rule 1), and every
    hand-over between it and an overlapper, its own siblings included, goes through the kernel's lock
    and an exception return, which order everything. The mixed-ordering check confirms this holds for
    the mixed case too. So a `Serial` run pays nothing,
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
- **Shared pages between processes** are the same frames, so memories that show a page in several places
  run in parallel there, and the mixed case of section 3 needs nothing new.
- **Replay** is the same proxy and hooks (4.6), called inside (below).

**This plan's calls, on the native executor.** There the kernel, the loop and the embedder's system run
inside the virtual machine, as plain function calls. Only the system's services that need the host
(files, the network, time, pages and streams carried to other machines) leave the machine, through the
link (native.md at its head, lines 1092-1095, the owner's word on its decision 1; section 6.1; section
6.2, R3, which rejects "forwarding every `System` call to the host"). One crossing of the link costs
0.7 µs on this Mac and 5.6 to 6.2 µs on the x86 laptop (native.md section 5.6). The plan's earlier
form promised calls "from any host thread" and added methods to `System` and `Host` without saying
on which side each falls. Its sixth check faulted that. They fall so:
- **Inside, with no crossing: everything the kernel calls at an entry.**
  - The proxy's two hooks, `on_memory_written_by_kernel` and `on_signal_delivering`. A system that
    records is called at every system call. A crossing there would add microseconds to each system
    call, against 21 to 32 ns for a call answered inside (native.md section 6.1). So a system that
    records keeps each process's log inside and sends it out in batches, as the inside sends
    warnings (native.md section 5.4). (The form before put `kernel_entry_admit` and
    `on_kernel_entry_ended` here; both are withdrawn, 4.6.)
  - `stop_thread`: an interrupt from one virtual CPU to another through the interrupt controller
    (above; not measured).
  - `on_deadline_changed`: arms the inside's timer (section 5).
  - `on_processor_needed`: wakes a virtual CPU that waits inside. When that processor's stack is
    parked and its virtual CPU has gone back to the pool (native.md section 5.1), it is one message
    to the monitor, which is the cost of waking a worker there.
  - `on_shared_memory_mapping`, `on_shared_memory_unmapped` and `on_watched_memory_accessing`: hooks
    handed a `Process`, run inside.
  - `turn`, `ran`, `arrived`, `stopped` and the deciding half of `idle`, with the arguments decision
    9 adds: the policy's, inside, as R3 has them.
- **Calls into the kernel are made by the system's code inside, on any virtual CPU**: `run` for a
  processor, `kernel.process(id)`, the `Kernel` trait's suspend and resume calls, `wake_waiters` and
  `move_waiters`. Where sections 5 and 6 say
  "any thread that can call the kernel", that is what it means there. A program on the host cannot
  make them, and cannot borrow a `Process` or a `ProcessInfo`: "with the kernel inside a virtual
  machine, a host program cannot borrow its data" (native.md lines 1368-1371; its decision 11
  recommends building whatever reads kernel state into the inside). A host program that wants a
  process read or its tasks taken out sends a request through the link to the system's code inside and
  gets a copy back, for one crossing.
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
  because the guest sees it and a copy or a move must see the same: suggested
  `GuestMachine::processors` (rt/container.rs:39-44), given to the kernel in `Start`. Suggested default:
  1, so every existing test and every existing user sees what they see today; the command line program
  passes the host's count unless told otherwise (decision 2).
- **How many run at once.** As many guest threads as the container has processors, and never a task that
  runs as `Serial` beside one that overlaps it (section 3). The embedder's limit is the count of
  processors it gives the container. A live run has one worker for each processor, also on a host with
  fewer cores (section 5): a container told 16 processors and brought to a host with 8 cores runs 16
  workers, which that host shares its cores among. The plan has no count of workers below the processors
  (known fault 3).
- **Which host thread runs which guest thread.** Any. A guest thread is its register block and its memory;
  on the WebAssembly executor its store moves between host threads (a wasmtime store may be sent, not
  shared). In a live run a worker keeps its processor, and the kernel puts a thread on whichever
  processor asks. Keeping a thread on the processor it last ran on is added only if measured to matter.
- **Fairness.** Linux's fair scheduler as built (kernel/schedule.rs), with one run queue for all
  processors: each takes the thread with the least weighted time that may run there (4.3: it is put
  forward by section 3's rule, and its mask holds the processor). One queue is fair by construction and
  costs nothing extra under one lock. Among tasks that run as `Serial` and overlap, the order is the
  one-processor scheduler's: only the one due a turn is put forward (4.3). The one exception is a
  waiter, which goes first (4.5). Between a `Serial` task and its `Parallel` overlappers, "due a turn"
  is also by the least weighted time, so fairness between the two phases follows the same scheduler;
  once the task is due, its overlappers are handed out no more and come in at their slices' ends, and
  one with no slice is asked in (section 3, rule 3). That is this plan's choice and not ruled (decision
  15).
  - **A woken thread that should run before a running one takes the place of a thread in its way.**
    Today the comparison is with the one running thread (`check_preempt`, kernel/task.rs:577-584,
    called at :561 and :574; `preempts`, kernel/schedule.rs:224-238). With several processors, and
    when no idle processor can take the woken thread, it is compared with the thread in its way that
    has the most weighted time (section 5). If it should run first, that thread's processor is
    marked to give way and the thread is asked in with `stop_thread`. The plan's earlier form
    compared it with the running thread with the most weighted time in the whole container. That
    may be a thread of another process while the woken thread waits behind its own sibling, or a
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
- **Told many, running one.** A threaded program run `Serial` is told the processors but runs one
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
    5), because the test system says which processor is served next. `run(processor)` returns only for
    one of the owner's four reasons (section 5), and none of them is "this turn is over, serve another
    processor". The form before made its order with the withdrawn `kernel_entry_admit`, answering
    `Later` for any other processor. Now the test system uses the ruled interrupter: at the end of
    each counted turn it interrupts the container from `arrived`, the loop looks at the container's
    stop mark before it calls `next`, so the processor hands nobody out, and `run` returns
    `Interrupted`. The driver then draws from the seed which processor is served next and calls `run`
    for it. Every turn is counted, so the order of entries, and of clock reads, is the same each time.
    It costs a container stop a turn, in tests only. This is this revision's design and unchecked.
    Rejected: a second driver loop for tests that steps one turn at a time. It would be a path the
    shipped loop never takes.
    - This holds for processes given `Exact`, which is what the test system gives (section 12). Only
      code with the count takes a counted turn. A process given anything else takes free turns, so a
      test that gives a process anything but `Exact` gives up repeating it.
  - With several workers nothing repeats. The order of entries is the host's timing. So is a clock read
    made without a system call (`Answer::clock`, rt/lib.rs:291-293): it makes no kernel entry, and
    takes whatever the other workers' threads had reported by that host moment. Such a run can be
    replayed only per process, from each process's log (4.6), once the kernel's primitives for it
    exist (known fault 2).

## 12. Tests and the bar

The test system gives every process (`Serial`, `Exact`) unless a test says otherwise. That is what the
kernel test crate's runs are today: one thread at a time, counted turns. It makes its repeatable order
with one worker and the interrupter (section 11). Tests in free turns against the reference use a system
that does neither. Until the two kernel primitives for replay exist (4.6, known fault 2), no test here
replays a recording; the tests that did in the form before are moved to T8 and rewritten per process.

- **Nothing changes with one processor.** With every process (`Serial`, `Exact`) and one processor,
  every test of the kernel test crate gives the same verdict as before, with and without `--features
  runtime`, at full, none and bounded look-ahead, and `repeats` still repeats. Checked as J1 checked it:
  the `test ...` lines from both commits kept and their `diff` empty. The same suites also pass against
  the reference with every process (`Serial`, `None`), in free turns.
- **Processes in parallel.** The same crate's tests that run several processes pass with several
  processors and free turns, against the reference.
- **Repeatable with one worker.** The test system with one worker and four processors runs the kernel
  test crate's multi-process programs twice from one seed, and then once from each of many seeds. Two
  runs from one seed have the same order of entries, the same position at each entry, the same
  answer from every `getcpu`, and every thread's registers and a hash of every memory the same at
  every entry. With every process (`Serial`, `Exact`).
- **Replay per process** (T8, once the primitives exist). A test system logs, for each process, its
  system call results through the proxy, the kernel's writes into it, its signals with their
  positions, and its threads' turns. It then runs each process alone, its system calls that bring
  something in answered from its log and the others run for real (4.6), with its neighbours not run at
  all. For a process given `Exact`: its registers and a hash of its memory are the same at every
  system call and every logged position.
  - A process given `None` writes a pipe that an `Exact` process reads, and only computes between
    writes, with the lengths of its turns varied by the seed. The `Exact` process replays alone. The
    `None` process's code has no count, no hook of the system is called for it but the proxy's
    defaults, and nobody logs it. (The sixth check's case, which needed the whole kernel recorded, is
    this test's ancestor; it now passes by construction, because the reader's read is in its log.)
  - A `Serial` process with two threads that wait and wake each other through a futex, and one that
    takes turns with a sibling that only computes, replays alone (decision 17 says how its turns are
    followed).
  - **`External`.** The kernel test crate's programs that have one thread and take no signal while they
    compute replay every time, with every request equal to the log's and every byte a `write` hands
    out equal. A threaded program written to depend on where its threads take turns replays many
    times: each replay either matches at every request, or is thrown away and says at which request.
    No replay is accepted that handed out a different byte.
  - **A replay that has left its recording ends.** A log is changed on purpose, one change at a time: a
    request's number; one argument; the bytes of a `write`; a result. Each replay ends at the first
    request that differs, with "replay thrown away" and the kind of difference. None waits.
- **A neighbour's guarantee never reaches a process.** In one kernel, a process given `None` and one
  given `Exact` come to share a file mapping, and later stop sharing it. `Process::execution_config()`
  answers what each was given throughout; the `None` process's code is never translated again (the
  cache key's count field does not change for it); the shared-memory hooks fire once for each process
  at the `mmap` and at the `munmap`, with the same `MemoryObject` and offset, and not for a mapping no
  process can write. Until the resume calls exist, a build for tests gives a forked child its own
  config. In T1, with both `Serial`.
- **The one scheduling rule** (section 3). From T2 on, where `Parallel` exists. Over seeds, with the
  test build's pause points, and in every run of the stress schedule the count "Serial task handed out
  beside an overlapper" (section 5) stays at zero:
  - *the reader's kernel work* (the mixed-ordering check's first case). A (`Serial`, one thread) and B
    (`Parallel`, four threads) map one file with `MAP_SHARED`. A adds 1 to a word with its
    architecture's atomic add, in a loop; B's threads add 1 to the same word with `FUTEX_WAKE_OP`.
    The test build pauses a B thread after it has entered the kernel and before it has the kernel's
    lock, while A is handed out. No addition is lost: the word ends at the sum of all. On the
    WebAssembly executor the same with a 128 bit compare and swap in place of the add;
  - *readers held for a writer's turn* (the second case). A only computes, beside B's threads, with
    one processor more than B has threads and then one fewer. A third process that only computes
    makes progress throughout, and no stop lock is ever held when a kernel lock is asked for (the
    executor's test build checks the lock order);
  - *a writer that waits for ever* (the third case). A is ready behind B's four threads, which only
    compute, on four processors. A runs within ten slices, and goes on getting its fair share;
  - *the three ways*: the same programs given both `Serial`, mixed, and both `Parallel` each match the
    reference;
  - *one memory, two forms asked*: a child made with `CLONE_VM`, given `Parallel` while its parent
    was given `Serial`, runs `Serial` code and takes turns with its parent. Counted;
  - *executor limits*: each limit of section 3 counted once per process and warned once, and the
    process's threads then never run beside each other;
  - *the copy at each switch* (T2's WebAssembly executor, once built): A and B alternate through a
    shared page that each writes and reads back between switches; every value each reads is the last
    the other wrote before the switch;
  - *a new overlap that changes the caller's own memory* (the revision's check, its first finding). B
    (`Parallel`, four threads that add to a word in B's own memory with plain loads and stores while
    they hold B's own lock, and with the architecture's atomic add outside it) runs all four on four
    processors; its thread b1 calls `mmap(MAP_SHARED)` on a file that A (`Serial`) maps, and in another
    run that C (`Parallel`, two threads) maps. On the WebAssembly executor B then runs as `Serial`:
    the `mmap` returns only after b2 to b4 have come in, "Serial task handed out beside an overlapper"
    stays at zero, and no addition is lost. On the native executor the same `mmap` beside A calls in A
    alone, and b2 to b4 keep running. With the test build's pause points on both sides of the `mmap`
    and while b2 to b4 are on their way in. The same with a fork in place of the `mmap`, whose child
    inherits a shared writable mapping, and with a resume, once the resume calls exist, that brings in
    a memory showing the file: the resume returns at once, and the resumed task is handed out only
    after B's threads are in. The caller b1 leaves `Execution::run` when its `mmap` returns on the
    WebAssembly executor (counted "left run: its memory changed how it runs");
  - *an overlap that ends* (the second check, its first finding; T2's WebAssembly executor). The same
    B, running as `Serial` beside C, with b1 handed out and computing; C's thread `munmap`s the file.
    B goes on running as `Serial` until b1's next entry, b1 then leaves `Execution::run`, and B's
    threads are then handed out as `Parallel`: the try-lock of 7.2 never fails, no hook of B waits for
    B's lock, and "Serial task handed out beside an overlapper" stays at zero. The same with b1 itself
    making the `munmap`. With the test build's pause points on both sides of the `munmap`;
- **A neighbour's guarantee under `CLONE_VM`** (from T4, where `Exact` with `Parallel` code exists).
  Two processes own one memory, both given `Parallel`, one `Exact` and one `None`. Until decision 22
  is ruled this test records, and does not bar on, the time the `None` one's threads spend kept out of
  guest code by a call-in's mark (known fault 12). The `None` one's
  threads are never called in for a change of the kernel's (counted per process), its code holds no
  count (read in its modules' cache keys), and the counter "group stop reached a thread of an Exact
  process for a change of the kernel's" stays at zero.
- **A batch taken from outside a hook names only its tasks** (once the suspend calls exist). A
  (`Parallel`, `Exact`) process of four threads that only compute; `suspend_tasks` of one of them on the
  `Kernel` from another host thread. The other three never stop, a thread the process starts just
  before the call keeps running and is not in what comes back, and the call returns once the named
  task is in. The same with the named task parked in a futex wait, and with it parked for a call-in:
  resumed, its call runs again (decision 19). The same with the named task ready but not handed out
  when the call starts, and with it woken: the test build pauses the call while it waits outside the
  lock, and the task is never handed out before it is taken (the second check, its fourth finding).
- **A task taken out while it waits** (once the suspend calls exist; a test embedder with two kernels
  in one host program stands for two machines; decision 19). Each case against the reference with a
  stop signal and `SIGCONT` in place of the move, where Linux has the same case:
  - W waits on a futex on kernel K2 and is taken out to be resumed on K3. A thread on K1 sets the word
    to a new value and wakes it, before W is resumed and again after. Before: the wake returns 0 and
    W's call, resumed, returns `EAGAIN`; after: W is woken on K3 and the wake returns 1. In both W never
    sleeps past the wake. The same with W woken but not yet run when it is taken out: resumed, it
    returns 0 and does not wait again;
  - W waits with a time limit and is kept out past its deadline: resumed, it returns `ETIMEDOUT`; kept
    out for less: it returns no sooner than the deadline (the second check, its ninth finding);
  - a requeue of W's word to another word is made while W is out: resumed, W compares its first word
    again, as Linux's restarted wait does;
  - a waker parked for the ordering point is taken out, once before its step is made and once after:
    the waiters are woken once, a `FUTEX_WAKE_OP`'s operation is applied once, and the resumed call
    returns the count it made (the second check, its third finding);
  - a pipe `write` that has written part of its bytes and waits for room is taken out: resumed, it
    returns the partial count, and the reader reads each byte once.
- **No hook waits for another kernel, and no handle is held across kernels** (with the same test
  embedder, once decision 14 has its answer). Threads on K1 and on K2 wake each other's waiters across
  the two kernels at the same time, many rounds, over seeds: both finish, each wake's count is the
  number of waiters it woke, and a test build counts "hook waited for another kernel" and "handle held
  while another kernel was called", which both stay at zero.
- **A condition variable across machines** (with the same test embedder, once decision 14 has its
  answer). glibc 2.17's condition variable, as its `pthread_cond_signal` and `pthread_cond_wait` are
  written (the wake-result-use report's counterexample A), with waiters on K2 and K3 and a signaller
  on K1, the test build pausing between the release of the internal lock and the wake wherever the
  ordering point allows one: no signal is lost. The same for a requeue by `pthread_cond_broadcast`
  (counterexample B).
- **A woken call reads its arguments once** (4.4). A thread of a `Serial` process parks in `ppoll`
  with an array of descriptors in its memory, and its sibling changes the array while it waits; the
  call that runs again acts on the array as it first read it, as the reference does. In T1. In T2 the
  same in a `Parallel` process, and with the array in memory a `Parallel` overlapper writes.
- **Memories that come to overlap while both run.** Two `Serial` processes each compute on a processor of
  their own. One has a file mapped privately and reads it in a loop. The other maps the same file
  shared and writable, and writes it. Then the other way round: the running one has the file mapped
  shared and has written pages it has not yet left with, and the other maps it privately. Over seeds,
  with the test build's pause points on both sides of the `mmap`:
  - "Serial task handed out beside an overlapper" (section 5) stays at zero, and the try-lock of 7.2
    never fires;
  - the `mmap` returns only after the other process's thread has made an entry, and the mapper then
    sees every store the other made before that;
  - "tasks called in for a new overlap" counts each new overlap once;
  - the run repeats with one worker from one seed, the call-in's entry among its entries;
  - from T2 on, the same with one side given `Parallel` with two threads;
  - the same with `shmat`, and with an exec of a file that another process has mapped shared and
    writable. Each matches the reference.
- **Two call-ins that want each other's memories.** The sixth check's case, as guest programs. Process
  P1 has two threads and maps file g; process P2 has one and maps file f. A thread of P1 calls `write`
  on f in a loop while P2's thread calls `write` on g in a loop, and P1's other thread computes. With
  two processors and with four, over seeds, with the test build's pause points after each call has
  parked and after each thread has come in:
  - both loops finish, each `write` lands whole, and every mapped byte matches the reference;
  - "Serial task handed out beside an overlapper" stays at zero and the try-lock of 7.2 never fires;
  - a build for tests checks at every lock release that no call-in is pending with nothing in the
    kernel left to move it: every pending call either has a running thread still asked in, or its
    thread is a waiter that `next` would choose. Counted "call-in with nothing to move it", which
    stays at zero in every run of the stress schedule;
  - the same shape with two `mmap` calls that each make an overlap with the other's memory, with
    `process_vm_writev` both ways, and with a waiter that is sent `SIGKILL` while it waits: its mark
    comes off and the other process runs on;
  - from T2 on, the same with P2 given `Parallel` with two threads and sharing a file with P1, so that
    a call-in's mark, a mark for a `Serial` task due a turn and a `Parallel` waiter held back while a
    `Serial` overlapper is handed out are all in one run (the shapes the redone model must cover,
    known fault 1);
  - from T4 on, two threads of one (`Parallel`, `Exact`) process that call `mprotect` at the same
    time, each calling its own memory in;
  - repeated with one worker from one seed: each choice that ran a waiter's call and handed nobody out
    is at the same place both times.
- **No thread waits behind its own sibling.** In free turns, with no slice timer running for a thread
  alone on its processor, with one processor and with four:
  - a `Serial` process whose thread A only computes and never makes a system call, while its sibling
    B sleeps a millisecond a hundred times and then ends the process with `exit_group`. The process
    ends, in no more than ten times the reference's time (the margin is for this machine's load and
    is the orchestrator's to set, as for T1's scaling test);
  - the same with B waiting in `read` on a pipe that another process writes a byte to a
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
  Run twice with one worker and four processors from one seed: each such choice is at the same place,
  and every thread's registers and memory are the same at every entry after it. What the call that
  ran again read and wrote reaches the proxy and `on_memory_written_by_kernel` as its own thread's.
- **The kernel called from inside a hook.** A test system keeps a reference to the kernel and calls
  `kernel.process(id)` from a hook, from `on_processor_needed`, from `on_deadline_changed` and from
  `arrived`: each answers the error, and none waits. It calls another entry point from a hook: the
  panic names the call. The same calls from another host thread while a hook runs wait for the lock
  and then succeed.
- **The kernel called from inside guest code.** A test system calls `kernel.process(id)` from its
  clock's answer, from `ran` when the clock hook calls it, and from `cpu_warning`: each panics and
  names the call, with the kernel's lock free and with it held by another worker, and none waits. The
  same `ran` called at a kernel entry does not panic. In T1 in a `Serial` process; in T2 also in a
  `Parallel` process whose other thread calls `mprotect` in a loop, where the earlier form would have
  hung.
- **Reaching across processes.** Guest programs for each case of 4.5 where one process reaches another: a
  writer through a shared mapping beside a reader through a private mapping of the same file; a `write`
  to a file another process maps; a file made shorter, and a file made longer, while another process
  maps and runs it; a signal that ends a process whose thread is running. Each matches the reference,
  and each repeats with one worker from one seed. Each write into another process's memory fires
  `on_memory_written_by_kernel` for that process with the exact range, and a watched range there fires
  `on_watched_memory_accessing` before the write (section 8).
- **No request to stop is lost.** A process that only computes, alone on its processor and with no
  timer, is sent `SIGKILL` from another process, over many seeds. A build of the executor for tests pauses
  the target's worker, by the seed, at each place where today's loop writes over the stop word: after
  the hand-out, after a fault is settled, after a rerun, after each answer to carry on, and in the stop
  hook (wt/lib.rs:2283, :1786). The process always dies. The same test with a memory called in for its
  pages, and with `sched_setaffinity` moving the thread. The translator's own test of `raise_stop`
  (tests/test_guest.rs) runs with another host thread setting the asked-in bit at the same time, and
  neither bit is lost.
- **No processor idles with a thread it could run.** Guest programs with no timer set, in free turns,
  over seeds. Each finishes, and makes the progress the reference makes:
  - a process that only computes maps a file that another process writes in a loop, so its memory is
    called in again and again while its processor goes idle. It keeps computing;
  - in a `Serial` process, one thread is pinned to processor 1 and waits on a futex. The other, on
    processor 0, wakes it and then parks on a futex only the first can wake;
  - a ready thread pinned to a busy processor has its mask widened by another thread to hold an idle
    one. It runs at once;
  - from T4 on: eight threads of a (`Parallel`, `Exact`) process on eight processors, one of which
    calls `mprotect` in a loop. All eight keep running between the calls;
  - from T2 on: a `Serial` process that sleeps a millisecond at a time beside a `Parallel` process of
    four threads that share a file with it and only compute, on four processors. Each time the
    `Serial` one sleeps, all four `Parallel` threads are handed out again.
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
  on a sibling: in T1 one that is parked in `read` in a `Serial` process, in T2 one that only computes
  in a `Parallel` process. Matches the reference. `on_signal_delivering` fires once, in the entry of
  the thread that takes it.
- **The executor's lock rule.** Every run of the stress schedule has the try-lock of 7.2 on: reaching a
  `Serial` process's lock from a worker that does not have it panics. Two runs aim at it:
  - a thread of a `Serial` process is paused after it last read its stop word and before it has the
    kernel's lock, and its memory is called in from another process during the pause;
  - the call that wanted the memory runs again on the worker of the thread that came in last, and on
    another worker, by the seed;
  - a thread of a `Serial` process is told to leave (another thread runs next; nothing to run), and its
    worker is paused after the answer and before the executor has let the
    process's lock go. Another worker's entry that reaches that process is waiting for the kernel's
    lock. It gets the lock only after the process's lock is free (4.2, the handle in `Leave`).
- **A worker for each processor.** A container told more processors than the host has cores (16 on
  the x86 laptop held to four cores with `taskset`, cores 2 and 8 left out): `run_until_finished`
  starts 16 workers, a thread pinned to processor 15 runs and reads 15 from `getcpu`, and the
  multi-process programs pass against the reference.
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
  runs over many seeds. With several workers a seed does not make a run repeat (section 11), so a
  failure there is kept with its seed, its counters and the test build's trace, and tried again with one
  worker from the same seed. Replaying it from logs waits for T8.
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
    (4.2), reported against decision 7's figures. It is the one cost the owner is asked to accept.
    The proxy's two hooks and the hook for the kernel's writes, with their default bodies, are in all
    three builds once the interface track has added them; the commit before them, against the commit
    after, with the same loops, is their cost, reported beside the lock's. It is not this plan's to
    accept or refuse, but it lands on the same system call (4.2);
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
  lock, the group stops by reason and the memories called in by reason. From T2 on, the same for a
  `Serial` process beside a `Parallel` one that share a file, given both `Serial`, mixed and both
  `Parallel`, with the call-ins and hand-outs at each switch counted (section 3's expected cost).

## 13. Milestones

Each bar is checkable by the named tests. The cheapest large win is first.

- **T1, processes in parallel, every process `Serial`.** No translated code changes. The config exists
  with `Serial` and with `None` and `Exact`. A config with `Parallel` or `External` is refused where it
  is given (the container's start), with an error that says it is not built yet: that is the
  embedder's call failing, never a guest's. (The form before also limited a "recorded kernel" to
  `Exact`; that kernel is withdrawn, 4.6.)
  - abi: `Cpu` and `Host` by `&self` and `Sync`; the processor in `Kernel::next`; the thread in
    `Kernel::exit`; the `Host` service `stop_thread`; the hooks `on_processor_needed` and
    `on_deadline_changed` (5); the stop word as a set of reasons, with `raise_stop` as an atomic or
    (4.5). The proxy, the hook for the kernel's writes, the signal hook and the shared-memory hooks
    are the interface track's; T1 fires each where its entry is, once that track has added it.
  - kernel: the lock with its holder's word, the handle that holds it and the `Send` reason (4.2);
    the check of the calling worker's mark at every way in but the loop's own (4.2);
    processors, masks and the scheduler, with one entry for each thread `next` chooses, a `Serial`
    task put forward only when it is due a turn, and a thread's woken call and its signals acted on
    only when it is chosen for a processor (4.3); overlaps kept per memory, with a new overlap made
    only once every handed-out task that would break rule 1 after it is called in, the caller's own
    siblings included where its memory comes to run as `Serial` (in T1, where everything is `Serial`
    and runs one thread of a memory at a time, that is the other side's tasks), and the resume's
    form of it once the resume calls exist (3); the rule of section 3 with its
    four corrections, of which T1, where everything is `Serial`, uses exclusion by "handed out", the
    call-in of overlappers and "due a turn"; the two rules of 4.4 item 5, with calling a memory in, a
    mark for each call, waiters chosen whatever is called in, and threads ending in their own entry
    (4.5); every call that runs again reading its arguments once (4.4, the gap that the mixed-ordering
    check found in `Serial` processes); a process's signal picked again when its thread blocks it or
    ends, through one function that sets a mask (4.5); the scheduler's condition in both halves (no
    processor idle with a thread it could run, no ready thread with nobody due in), with its mark and
    its counts, the wake-up comparison aimed at a thread in the woken one's way, and the deadline out
    of the slices (5, 11); charging per thread (11); the told count; `ExecutionConfig` at the start,
    inherited at fork; `kernel.process(id)` at any time for reading (5, if decision 8 is ruled so).
  - runtime: `run` and `run_until_finished` with a processor and `StopReason` per worker, one worker
    started for each processor, the system told who, idle without a lost wake, the container stop
    reached through the ruled interrupter, the executor's stopping handle and its promise in place of
    `StopFlag`, the kernel's handle carried by `Carry::Leave` (4.2), the local system by `&self` with a
    slice's end per processor and one deadline for the container, sent by an idle worker or by the
    timer thread (5).
  - WebAssembly executor: shareable, with the process's lock kept for a thread's whole stay in
    `Execution::run`, let go before the kernel's handle is dropped, and its try-lock rule (7.2); the
    asked-in bit, taken off only inside an entry (4.5); the worker's mark set around the system's
    calls in the clock hook and `warn` (4.2); a code drop that reaches only the memories it must (7.5);
    counted turns for a thread of an `Exact` process and free turns for any other. Stores stay one per
    process: one thread of a `Serial` process runs at a time.
  - kernel test crate: the test system's repeatable order with one worker, made with the interrupter
    (11). (The form before had a recording and replaying test system here; replay is T8's now.)
  - the stand-in stays at one processor.
  - Bar: section 12's "nothing changes with one processor", "processes in parallel", "repeatable with
    one worker", "a neighbour's guarantee never reaches a process" (with both `Serial`), "a woken call
    reads its arguments once" (the `Serial` run), "memories that come to overlap while both run" (its
    `Serial` runs), "two call-ins that want each other's memories" (its `Serial` runs), "reaching
    across processes", "no request to stop is lost", "no processor idles with a thread it could run"
    (its first three programs), "no thread waits behind its own sibling", "timers and slices", "a
    process's signal finds a thread" (the `Serial` run), "a choice that hands nobody out", "the kernel
    called from inside a hook", "the kernel called from inside guest code" (the `Serial` run), "the
    executor's lock rule", "a worker for each processor" and "processors and masks"; its `Serial`
    measurements, of which only the system call cost can move here; and a scaling test: four processes
    that only compute, on four processors, finish in under half the time they take on one (a quarter
    is the ideal; the margin is for this machine's load and is the orchestrator's to set).
  - Measured: a real build with parallel jobs at 1 to 16 processors; the share of time waiting for the
    kernel's lock; memories called in, by reason, and how long a mark stayed on; stops asked for
    nothing; threads asked in for a timer, for a waiting thread or for a waiter; turns that ended for
    nobody; how often the scheduler's condition was walked and how long a walk took; the cost of the
    lock and its holder's word in a system call, by the build without them, and of the proxy's hooks
    beside it; the same build with 16 processors on four and on eight cores, against 16 on 16 where a
    machine has them.
  - Before T1's bar is trusted, the model of call-ins is redone for section 3's rule (known fault 1). It
    checks T1's rules, not its code, and the form before's model took seconds a search.
- **T2, a process's threads in parallel on the WebAssembly executor, ARM64 guests.** After decision 4.
  - translator: the Parallel form (7.2): barriers and the 128 bit hook behind the byte (7.3), the mark
    for hooks that are stop points (4.5); the shared import and its declared maximum in both forms.
  - executor: the two engines, with the compiler setting and a test that it took effect (7.6); the
    host-supplied shared memory, first; one store per thread; group stop with its stop lock, its marks
    and the held bit, held only at a stop point (4.5); frames given up after a page that allowed
    reading and writing lost a permission (4.5); a write from outside into a memory with one thread
    made under a group stop (7.3); the table rules and the shared pages of 7.4, with the one-way
    copy; the copy at each switch for a `Serial` task beside `Parallel` ones, and "can alternate" in
    `Cpu::parallel_limits()` (7.4), or, if it is left for later, those tasks kept `Serial` and
    counted; the byte following the memory's own threads (7.3); code dropped and added, with the
    count of drops (7.5); table growth.
  - kernel: `Parallel` in the config; the rule's mixed case, with rule 3's mark on the overlappers of a
    `Serial` task due a turn and rule 4's phases (a `Parallel` waiter held back while a `Serial`
    overlapper is handed out); one memory with one form under `CLONE_VM`; the executor's limits as a
    fact about each task (3), with a new overlap that makes the caller's own memory run as `Serial`
    calling its siblings in (3); telling the executor each memory's form and how it runs (7.3); the
    compare and exchange for the words the kernel changes; `membarrier` with its restartable-sequence
    command.
  - Bar: the threaded suite, "the one scheduling rule" (its WebAssembly runs, with the copy at each
    switch if built), "the races this plan names", the `Parallel` runs of "memories that come to
    overlap", "two call-ins" and "a woken call reads its arguments once", "a process's signal finds a
    thread" (the `Parallel` run), the ordering tests and "the compiler setting" match the reference in
    `Parallel` under the stress schedule at full, none and bounded look-ahead; T1's bars hold; the
    `Serial` measurements of section 12 hold, the byte-for-byte one and the four of threaded programs
    run `Serial` among them.
  - Measured: group stops by reason and their length; threads that gave up their frames, by reason;
    translations thrown away; a store's memory; what the Parallel form costs a process with one
    thread; for a `Serial` task beside `Parallel` ones, the call-ins and hand-outs at each switch and
    the time a `Serial` wake waits for the slowest overlapper, and the copy at each switch if built.
- **T3, the runtimes and real threaded programs.** Go, Java, Node and Python with threads, and code
  compiled while other threads run.
  - Bar: section 12's runtimes pass in `Parallel` under the stress schedule.
  - Measured: speed-up of a threaded program in each; group stops for code changes; how often barriers
    (by kind) and the 128 bit forms run, counted by a build of the translator made for this measurement
    only.
- **T4, (`Parallel`, `Exact`) and per-thread watches on the WebAssembly executor** (section 8). After
  the embedder interface's memory watches exist, with the kernel's own accesses firing
  `on_watched_memory_accessing`, and the translator's count exists in the Parallel form (other
  tracks). With it: a change of the kernel's in an `Exact` process is a call-in (4.5).
  - Bar: a test embedder holds each page for one writer or several readers and records every hand-over
    with both threads' positions. A test build of the executor that logs every store checks that the
    record is whole: every store another thread or the kernel made to a page while a thread held it
    is in the record at its place. Replaying a program from such a record waits for T8. The
    program's threads also fill each other's pages through `read` system calls, so the kernel's own
    accesses are in the record; they write objects on each other's stacks (a scoped thread filling its
    parent's array, a result joined into a stack slot), one of them while it runs on a second stack,
    so the rule for frames is in it too; and they `mprotect`, fork and rewrite code while the others
    run, so every call-in is in it. The torn-permission and live-frame tests of section 12 pass with a
    watch in place of `mprotect`, and the eight-thread program of "no processor idles" keeps all eight
    running. `suspend_tasks` naming a running sibling in `on_watched_memory_accessing` does what
    decision 11 rules, and `suspend_tasks` on the `Kernel` from outside any hook takes it. Section 12's
    "a neighbour's guarantee under `CLONE_VM`" passes, and, once the suspend calls exist, "a batch
    taken from outside a hook names only its tasks".
    A replay of the whole record through the kernel's replay primitives is T8's.
  - Measured: the cost of one hand-over; hand-overs per second in the threaded suite and the runtimes;
    frames given up for a watch; call-ins by reason.
- **T5, the native executor** (section 10), in the native track, after its single-thread suites. With it:
  the two forms of native code, with barriers (and x86's `mfence` and lock prefixes) left out of the
  Serial form and copied in the Parallel form; the exclusive's record made safe for threads in
  parallel; each held virtual CPU dropping its own cached translations; this plan's calls on the
  inside half (section 10).
  - Bar: T1's and T2's bars on the native executor, the ordering tests with barriers among them, and
    "the one scheduling rule" with its first case on `lock xadd` (where the native `Serial` form drops
    the prefix); the per-thread root under T4's test; the Serial form of the benchmark set and of
    busybox holds no barrier instruction and is the same code as before the milestone, read from the
    output; the proxy's hooks at a system call make no crossing of their own: over a loop of
    `getppid` with a recording system, the monitor counts no more exits than the log's batches sent
    out.
  - Measured: the interrupt from one virtual CPU to another, which the mixed case pays at each switch.
- **T6, x86-64 guests**, in the x86 track: the Parallel form for x86, with lock prefixes as atomic
  operations, the hook for a misaligned one and `mfence` as the tested fence (7.3); the executor's
  answer on a weaker host (section 9); frames given up by 4.5's test, by the page, where a code page
  is locked against writes.
  - Bar: the threaded suite and the ordering tests, the sequence lock and the store-back among them,
    for an x86 guest in `Parallel` on the x86 laptop; on this Mac the same suite passes with the process
    kept `Serial`, its counter read by the test; the `Serial` measurements of section 12 for an x86
    guest.
- **T7, splitting the kernel's lock**, only where T1's measurement points (4.7), one part at a time.
  - Bar per part: the single-thread system call cost is not above its figure before; the time waiting for
    locks on T1's build is lower; every earlier bar holds.
- **T8, replay per process** (4.6), after the embedder interface track has designed the two kernel
  primitives ("where replay is recorded": not yet designed) and the owner has ruled decisions 12 and
  17. It changes no translated code. With it: `External` for processes that run as `Serial` (decision
  3); the replaying test system, which answers a process's calls from its log through the proxy and
  runs for real the calls that change the process's own records (4.6); reading a thread's count and
  stopping a thread at a count; the order of a `Serial` process's threads' turns followed in a replay.
  - Bar: section 12's "replay per process", with its `External` part and "a replay that has left its
    recording ends"; T4's record replayed through the primitives where T4 is built.
  - Measured: what the proxy's hooks cost a system call when a recording system logs results, a small
    call and one that reads many bytes (a large `write`); and the share of the kernel test crate's
    programs whose `External` replays are never thrown away.

Not milestones here: the form that orders atomic instructions for (`Parallel`, `External`) (decision 3),
the ordered x86 form (decision 5), a browser, the suspend and resume calls, which are the embedder
interface's, and locks across machines (4.5; decisions 13 and 14), and placing tasks across kernels
(decision 20). What the kernel itself must do for them is built with the suspend calls and tested by
section 12's tests that name them: a batch that marks only its tasks, and every one of them, and a task
taken out while it waits (decision 19). The group stop that taking tasks out needs in a `Parallel` process is T2's. What
the values that leave the kernel may hold (4.2) is asked of the interface track. Fewer workers than processors (known fault 3) is not a milestone.

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
  they were in the form before: a mark for each call, a group called in while any mark names it, a waiter
  put forward whatever is called in but never beside a running sibling of its `Serial` group, back to
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
  **It is a model of groups, and this revision has none.** It has no mixed overlap, no exclusion by
  "handed out", no mark for a `Serial` task due a turn and no waiter held back by a handed-out
  `Serial` overlapper (section 3). Its results say nothing about those; the model is to be redone
  (known fault 1).
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
  spread of 34 in the first run, of about 6.3 million instructions a program start. Real suspend and
  resume calls and the native executor were not measured.
- **The one scheduling rule's costs, by the mixed-ordering check**: it measured nothing new. Every
  figure it gives is one of this section's or one this plan quotes (leaving `Execution::run`, a stop
  noticed in Chrome, an interrupt from the host into the machine, the kernel's share of `getppid`, the
  lock), and every cost of its own is expected (section 3).
- **The progress count**, by the translator track's measurement, not this plan's
  (orchestrator/progress-count-cost.json): its best form costs the seven quoted benchmarks 16.3% more
  instructions on this Mac and 11.9% on the x86 laptop, and 4.4% more time on the laptop.
- From earlier work, not measured again: atomic accesses against plain ones, and shared against unshared
  memories, in wasmtime 49, Node and Chrome (ordering-cost/result.json); the stop flag's latency in Chrome
  (docs/phase-1-translator.md); the cost of recording page hand-overs in a 2008 system
  (dsm-survey/results.json); what ordering only atomics and kernel entries would miss
  (optimistic-replay-results.json); system call, fork and instantiation costs (docs/runtime.md section
  18); the native executor's interrupts (native.md section 5.5).
- **The earlier figures this plan quotes, each with where it is and its load** (added on 2026-11-20:
  the revision's check found them cited in the plan and missing here). None was measured again.
  - Leaving `Execution::run` and coming back: 300.8 ns a call, median of 5 runs of 100,000 calls, min
    285.7 ns; this Mac; docs/runtime.md section 18 (also quoted in its section 10), load 39.78, 33.56
    and 28.45 over 1, 5 and 15 minutes. Quoted in sections 3, 4.3, 4.5, 5, 6 and 8, some as "300 ns".
  - Instantiating a thread's code: 0.43 to 0.60 ms for busybox and 0.27 to 0.42 ms for musl, over 8
    runs; this Mac; docs/runtime.md section 18 (its section 8 cites it), load 41.5 to 60.6. Quoted in
    section 8.
  - The kernel's share of `getppid`: 116.3 to 120.0 ns, 5 runs of 1,000,000; this Mac;
    docs/runtime.md section 18, load 64.8, 45.7 and 34.1. Quoted in sections 3 and 4.2.
  - Noticing a stop: 230 ns on average and 30 µs at worst, 20,000 stops, in Chrome;
    docs/phase-1-translator.md, "The stop check"; no machine load is given there.
  - An interrupt from another host thread into the native executor's machine (`hv_gic_set_spi`):
    medians 2.4 to 4.1 µs over its runs; this Mac; native.md section 5.5, from docs/status-native.md;
    native.md gives no load.
  - A crossing of the native executor's link: 0.7 µs on this Mac and 5.6 to 6.2 µs on an Intel laptop;
    and a system call answered inside, 21 to 32 ns; native.md sections 5.6 and 6.1, both from that
    track's brief; native.md gives no load. Quoted in section 10.
  - Taking a process's threads out of line and putting them back, on the interface track's prototype:
    1,100 to 1,400 instructions; orchestrator/interface-measurements.json, part "run-return" (this
    Mac, load 56 to 184, as the item above). Quoted in section 6.

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
  point, and how often real programs cause one (T2, T3). How often a memory is called in from outside
  (T1).
- The ordering tests as guest programs through the translator: none was run. What was run are
  hand-written tests (section 14).
- What the stop lock costs the kernel's accesses to a `Parallel` memory, and how many translations a
  runtime that compiles while it runs has thrown away (T2, T3).
- What the proxy's two hooks cost a system call with their default bodies (T1's builds), and with a
  system that logs results (T8); and how many real programs replay under `External` (T8). The
  interface notes leave it unverified that an unused hook costs nothing.
- A hand-over through per-thread watches, end to end, and what calling a memory in for every change of
  the kernel's costs a (`Parallel`, `Exact`) process (T4).
- An interrupt from one virtual CPU to another inside the native executor's machine (T5).
- A barrier inside the native executor's virtual machine, and `xchg` with memory against two moves in
  the native output's Serial form (T5). At user level on hardware the barriers are measured (section
  14).
- What a container costs when it has more processors, and so more workers, than its host has cores:
  the host then takes a worker off its core while it may hold the kernel's lock or keep a group stop
  waiting. Correct, and measured with T1's build at 16 processors on four and eight cores.
- How long a mark of a call-in stays on in real programs, and how often a thread is asked in for a
  waiter (T1).
- What a `Serial` task's block and wake costs beside N handed-out `Parallel` overlappers: N call-ins
  and N hand-outs on the one lock, and the wait for the slowest to reach a stop check (section 3,
  expected; T2 and T5). The copy of dirty shared pages at each switch on the WebAssembly executor (T2).
- The prototype's figures for (`Parallel`, `External`): withdrawn, being measured again by the
  interface work (4.6).
- `KVM_CAP_MAX_VCPUS` on an ARM64 host: none is at hand.
- An x86 guest's Parallel form on an x86 host through the translator (the x86 track, T6).
- Real `suspend_tasks`, `suspend_process` and the resume calls on one kernel, with the records moved
  out and back, and their use for acting on a process from outside a hook (section 5): nothing exists
  to measure. Only the return of `run` at a program start was measured, on a prototype (section 14).
- How often a `Parallel` process takes a permission from a data page or unmaps one, which makes each of
  its other running threads give up its frames (300.8 ns each, 4.5), and what the returns through the
  bottom loop cost afterwards (T2, T3).
- What a timer costs when it is sent by the timer thread and not by an idle worker, and how many
  threads a real program's timers bring in (T1). The design brings in one at most for each expiry.
- How often a memory with one thread is stopped for a write from outside (T2).

## 16. Decisions for the owner

Already ruled, and so not asked again: `ThreadOrdering` with two values; `ReplayDeterminism` as its own
field of `ExecutionConfig`; the count only for `Exact`; no setter; hooks handed `Process` by value;
`run` and `StopReason`; where inspection begins; that processes which share memory may be on different
kernels (section 2). And from 2026-11-19, each quoted by its heading in the interface notes (section
2): replay per process; per-process state; the system call proxy and the hook for the kernel's writes;
signals as an input; the one scheduling rule and the shared-memory hooks; `WorkloadPlacement` with
per-thread distribution as core; suspend and resume by task. Not ruled, though the owner's word is
recorded: `External` staying (its heading says "(owner)", not "Ruled"), and the two calls for locks
across machines (agreed in substance, details open).

Each of the ten decisions the form before put to you is said below to be settled (by which ruling),
partly settled (what is left), or open as before. What is written as recommended is this plan's
recommendation, not a ruling, and what is written as proposed by the orchestrator is the orchestrator's
decision, to be confirmed by you. Decisions 11 to 18 are new: the rulings create them. Decisions 19 to
21 are new on 2026-11-20, from the revision's check, and 22 from its second check.

1. **When a process cannot be run as its config says, what happens? And what does `None` mean beside a
   recorded process?** **Partly settled** (the form before this correction said "settled" while still
   asking you to confirm two things below). By "Ruled by the owner, 2026-11-19: where replay is recorded"
   (a kernel is not recorded whole; `None` costs nothing and breaks no neighbour) and by "RULED by the
   owner, 2026-11-19 evening: shared memory is announced per process (threads plan decision 1)" (one
   scheduling rule in the kernel, nothing on the interface). That the rule replaces groups in force,
   the config in force and the strictest wins is said in the proposed section before it, and the plan
   follows it because the ruled rule leaves them no place. The hook
   `on_execution_config_in_force_changing` and the "recorded kernel" go with them (sections 3, 4.6).
   What the rulings leave, each asked below or noted:
   - the mixed-ordering check's corrections to the rule (exclusion by "handed out", overlappers called
     in and never held, a `Serial` task due a turn stopping hand-outs, the kernel's rules following the
     phase) are in the plan (section 3). You have not read the check yet (the interface notes say it
     is queued); the plan treats its corrections as required, because without them the rule is
     unsound;
   - the executor's limits still make a task run as `Serial` (7.4, section 9, (`Parallel`,
     `External`) until built), and processes that own one memory take turns when any was given
     `Serial`. The check says "the ordering half of the config in force stays" for these; your ruled
     rule leaves no config in force. The plan keeps them as a fact about each task, counted and warned,
     with no hook and nothing on the interface. Say if that is not what you meant;
   - under `CLONE_VM`, an owner given `None` beside one given `Exact`: the plan calls in only the
     `Exact` owner's threads for a change of the kernel's, and keeps code per owner so the `None`
     owner's code holds no count (4.5, 7.2). The second is this correction's design, unchecked;
   - fairness between a `Serial` task and its `Parallel` overlappers (decision 15) and an `Exact`
     `Serial` task's inputs from shared memory (decision 16).
2. **What is a container given when nobody says?** **Open as before.** Recommended:
   `GuestMachine::processors` 1, fixed for the container's life, and (`Serial`, `None`) for its first
   process; the command line program passes the host's count of processors unless told otherwise.
   Cost: a library user who sets nothing gets no speed-up and no recording. The other answer for
   processors, the host's count, speeds everyone up at once and makes a container's behaviour depend
   on the machine that first made it. `Parallel` as a default is not recommended before T3's bar holds.
   - With it, to confirm: a live run has one worker for each processor, whatever the host has, and
     there is no setting for fewer (section 5). A container made with 16 processors and brought to a
     host with 8 cores runs 16 workers on 8 cores. Cost: that host shares its cores among them, which
     is correct and not yet measured. The other answer, fewer workers than processors, needs a design
     this plan does not have (known fault 3).
3. **How much of `External` is built?** **Partly settled**, by your word recorded under "2026-11-19
   17:30: optimistic replay STAYS (owner), and my recommendation to drop it was wrong" (a heading that
   does not say "Ruled"): "`External` stays in the interface", as a mode chosen per program, and
   "Re-measurement running". That section does not speak of the `Serial` and `Parallel` halves; that
   both stay is this plan's reading, since (`Parallel`, `External`) is a value of the interface. The
   independent checks it cites found that the prototype's replayer invented failures and that its
   comparison was stricter than your definition, so this plan no longer leans on the figures the form
   before used against the `Parallel` half (none of 11 programs replayed; 24 to 97 ns an event) until
   the new ones are in. What is left:
   - when each half is built. Recommended: the `Serial` half with T8, which changes no translated code,
     once the kernel's replay primitives exist; the `Parallel` half after the new figures are in,
     weighed on them. Until it is built a (`Parallel`, `External`) process runs as `Serial` (section 3),
     which keeps the same promise;
   - what the `Serial` half records is no longer a digest of each kernel entry (withdrawn,
     `on_kernel_entry_ended`) but each system call's request and result through the proxy, with the
     bytes a call takes from memory compared, not only its arguments (4.6).
4. **wasmtime's compiler must leave plain accesses alone in a parallel process, and its own setting for
   that does nothing in the version we pin. Carry a patch?** **Open as before.** Without it, parallel
   threads are wrong for ARM64 guests (a store of a value just loaded is dropped) and for x86 guests
   (repeated loads are merged), both measured (7.6). Recommended: send the fix to wasmtime's project,
   and carry it as a patch to the one crate (`cranelift-codegen`, a few lines where the optimiser calls
   the pass) until a release has it; T2 waits for this answer.
   - Cost: one patched crate to carry across wasmtime upgrades until then, and 0.0% to 1.8% more
     instructions for code in the Parallel form (measured, this Mac).
   - The other answers: no patch, with ARM64's dropped store on the list of written differences and
     x86 guests paying up to 1.97 times on load-heavy loops for atomic loads; or no `Parallel` on the
     WebAssembly executor until wasmtime releases the fix.
5. **Is the ordered form for x86 guests on weaker hosts built?** **Open as before.** Recommended: not
   now. Until then such processes run as `Serial` on ARM64 hosts and in parallel on x86 hosts. Building
   it costs one more form of code for every x86 program, up to 2.7 times slower single-threaded
   (measured before this plan, section 9), to gain cores for threaded x86 programs on ARM hosts.
6. **Three places where threads in parallel differ from hardware: accept them?** **Open as before.** An
   exclusive store succeeds if the value is the same again, where hardware would fail it, on both
   executors. On the WebAssembly executor a 128 bit atomic is made with the memory's other threads
   stopped, which is exact and slow. And an address computed from a loaded value in a way that cannot
   change it no longer orders the two loads (7.3). Recommended: accept all three, on the written list
   of differences. The alternative for the first costs every lock operation of every program. (The
   one scheduling rule adds no difference: a `Serial` task's inline 128 bit form is safe because no
   overlapper is handed out beside it, section 3, rule 1.)
7. **One cost to a `Serial` run that this plan cannot bring to zero: accept it?** **Open as before.** A
   system call takes one uncontended lock. Alone it measured between nothing and 3.6 ns on this Mac
   and between 0.6 and 11.4 ns on the x86 laptop, by the work around it, against a kernel share of
   about 120 ns on this Mac (4.2). Its holder's word could not be told from the noise on either
   machine. Removing the lock costs the kernel compiled twice into every program. Everything else this
   plan adds is kept off a `Serial` run, and T1's bar checks it with a build that has no lock (section
   12). Recommended: accept it, or have T1 built and measured first; nothing else in T1 depends on the
   answer.
   - New beside it, and not this plan's to ask: the ruled proxy calls the system twice at every system
     call, and the kernel's writes call one more hook. Whether an unused hook costs nothing is
     unverified in the interface notes. T1's builds measure it beside the lock (section 12).
8. **What may an embedder do to one process while its threads run?** **Partly settled.** "RULED by the
   owner, 2026-11-19 night: suspend and resume by task, and the process context" settles the acting
   half: the `Kernel` trait's calls take tasks or a process out as one batch, how the kernel stops them
   is its own, and the handle a hook is given has the same calls limited to its one process. The plan's
   own call and hook for acting from outside a hook (`Kernel::process_call_in`,
   `on_process_called_in`) are withdrawn, because in this plan's design `suspend_tasks` and
   `resume_tasks` do that job (section 5); that they do is the plan's, not the ruling's. What is left:
   - **How a batch from outside a hook stops its tasks**, this plan's design (section 6): it asks in the
     named tasks only, marks them and not their memory, and waits outside the lock. Recommended; the
     ruling leaves it to the kernel.
   - **Reading, at any time.** Recommended: `kernel.process(id)` answers a handle whenever it is asked,
     and `id`, `info` and every `ProcessInfo` accessor work on it, with nobody stopped. Cost: the
     handle holds the kernel's lock while it lives, as a hook does. Asked from inside a hook it answers
     an error, as you ruled; any other call into the kernel from inside a hook panics and names the
     call. So does any call into the kernel from the three methods the executor calls from inside guest
     code (`ran`, the clock's answer, `cpu_warning`): their doc comments say they may not call the
     kernel or wait for anyone who does (4.2). The other answer: only while no worker is inside `run`,
     which lets an embedder read nothing about a running process.
   - **Who "any thread" is.** In a host program, any host thread. On the native executor the kernel
     and your system run inside the virtual machine, so it is your system's code there; a program on
     the host asks through the link and gets a copy, as native.md's decision 11 recommends (section 10).
   - **Your guarantee for hooks is narrowed in one case**, as before but now for the new calls: in an
     `Exact` process with `Parallel` code whose other threads run, taking those threads out from inside
     a hook needs a call-in, which a hook cannot wait for (4.4 item 7). What the call does then is
     decision 11. The other answer, as before: the kernel calls the memory in before each hook that can
     fire with other threads running, which keeps the guarantee whole and makes every watched access
     stop every thread of the process.
9. **The interface changes: accept them, so that splitting the kernel's lock later changes none?**
   **Partly settled.** The rulings of 2026-11-19 put `&self` on every hook they write, and "Ruled by the
   owner, 2026-11-19: per-process state" rules `type ProcessState: Clone + Send`; those were the
   embedding project's two proposals. They also rule the proxy, the
   hooks for the kernel's writes, signals and shared memory, and the `Kernel` trait. Withdrawn from
   this list by them: `kernel_entry_admit` with `KernelEntry` and `EntryAdmission`, `Start`'s field
   saying whether the system orders entries, `on_kernel_entry_ended`,
   `on_execution_config_in_force_changing`, `Kernel::process_call_in` and `on_process_called_in`, and
   `SuspendedProcess` (its rule on `Rc` now applies to `SuspendedTask` and `ProcessContext`, 4.2). What
   is left, recommended: yes, to all of it. Cost: every `System` keeps what it changes in atomics or
   behind its own locks, and gains the arguments below.
   - `System: Sync` from T1, also for a system driven by one host thread. The other answer lets such a
     system keep `Cell` and `RefCell` until its second worker (section 5).
   - In T1. `Kernel`: entry points take `&self`; `next` takes a processor; `exit` takes the thread; the
     handle that holds the lock across a thread's `exit` and `next`, which `arrived` is handed in the
     kernel's place, and which `Carry::Leave` hands the executor so that it lets its own lock go first
     (4.2). `Start` carries the processors and the first process's `ExecutionConfig`. `Cpu` and `Host`:
     methods take `&self`; both are `Sync`. `Host` gains one service, `stop_thread(thread)`, and two
     hooks, `on_processor_needed(processor)` and `on_deadline_changed(deadline)`. The kernel's deadline
     is no longer in a thread's slice: the system hands in `Event::Timer` at the deadline it was last
     told, whether or not a thread runs. The build that makes the kernel gives it a lock and a way to
     name the calling host thread (`lock_api`'s `RawMutex` and `GetThreadId`). `System`: `turn` is told
     the processor, the thread and its guarantee, `ran` the thread, `arrived` and `idle` the processor;
     `idle` also returns when its processor is needed; `stopped` runs once, with every worker out of
     `Execution::run`. `Execution::run` takes `&self`. `StopFlag` and `Executor::stop_flag` go; the
     executor gives a handle that stops a named thread, and promises that a stop asked for is never
     lost. `run` and `run_until_finished` take a processor, and `run_until_finished` starts a worker for
     each. `kernel.process(id)` answers at any time (decision 8).
   - In T2. `Cpu` gains `set_execution_config(memory, thread_ordering, exact_owners,
     several_threads)`, `parallel_limits()` with "Serial only" and "can alternate" (7.4),
     `compare_exchange(memory, address, expected, new)` and a call for `membarrier`'s group stop.
   - **On the native executor every call above is on the inside half** (section 10), and so are the
     ruled hooks; none crosses to the host at an entry.
   - Asked of the embedder interface track: the values that leave the kernel share no `Rc` with what
     stays (4.2); and decisions 11, 12, 14, 17, 18, 19 and 21.
10. **Names.** **Partly settled.** Ruled since the form before: `SystemCallRequest`,
    `SystemCallResponse`, `SYSTEM_CALL_ARGUMENT_COUNT`, `intercept_system_call_request`,
    `intercept_system_call_response`, `on_memory_written_by_kernel`, `take_dirty_memory`, method names
    verb first (`read_memory`, `write_memory`, `watch_memory`, `send_stream`, `allocate_task_id`),
    `WatchesFor`, `AccessKind`, `on_watched_memory_accessing`, `on_signal_delivering`, `send_signal`,
    `SignalInformation`, `MemoryObject` with `DeviceId`, `InodeNumber`, `SystemVSegmentId` and
    `AnonymousMemoryId`, `on_shared_memory_mapping`, `on_shared_memory_unmapped`, `Kernel`,
    `suspend_tasks`, `suspend_process`, `copy_process_context`, `resume_process`, `resume_tasks`,
    `SuspendedTask`, `ProcessContext`, "task" (Linux's word) and "memory shard"; agreed in substance,
    `wake_waiters` and `move_waiters`. Gone with what they named: "group", "config in force" with
    `ProcessInfo::execution_config_in_force()`, "recorded kernel", `kernel_entry_admit`, `KernelEntry`,
    `EntryAdmission`, `kernel_entry_admission`, `on_kernel_entry_ended`,
    `on_execution_config_in_force_changing`, `Kernel::process_call_in`, `on_process_called_in`. The
    plan's suggested `thread_stop` is renamed `stop_thread` for the verb-first ruling. Still suggested,
    all yours: "overlap", "runs as `Serial`", "due a turn" and "handed out" (the check's word) for the
    scheduling rule; "the Serial form" and "the Parallel form" of code, and "`Serial` code"; "called
    in", "waiter", "group stop", "held", "stop point" and "stop lock"; `processors` and `ProcessorId`;
    the service `stop_thread`; the hooks `on_processor_needed` and `on_deadline_changed`; "in the way"
    and "due in"; "gives up its frames"; `Kernel::enter` and `KernelEntered`, and `Carry::Leave(why,
    entered)`; `Executor::thread_stopper`; the stop word's bits `STOP_ASKED`, `STOP_HELD` and
    `STOP_OWN`; `HookType::stop_point`; `runs_in_parallel` for the byte; `Cpu::set_execution_config`,
    `Cpu::compare_exchange` and `Cpu::parallel_limits`; the counters' names; the milestones' letter T.
    Open with the verb-first rule: whether getters such as `parallel_limits()` and the earlier ruled
    `execution_config()` follow it, and the per-thread watch's name (`memory_watch_thread` in the
    earlier ruling). One more, as before: two calls named `run` one layer apart will be misread;
    suggested `Execution::run_thread`.
11. **New. What does a batch do when it cannot wait inside a hook?** The ruling lists it as open: "the
    batch that cannot wait inside a hook when it names other running tasks of an exactly recorded
    process". In an `Exact` process with `Parallel` code, a running task can stop only at an entry of
    its own (4.4 item 5), which a call-in gives, and a hook holds the kernel's lock and cannot wait for
    one. The ruled return has no error, and reads a task left out as one that had already exited, so a
    task left out for this reason would look exited. The answers the plan sees, none recommended over
    the others yet:
    - the batch takes the tasks it can now (the hook's own, and any not running) and leaves the rest
      out; the return then needs a way to say "running, not taken", which the ruling's shape does not
      have;
    - the call panics or answers an error inside a hook in that case, and the embedder takes the batch
      from outside the hook, where the call may wait (section 6); this adds an error the ruling chose
      not to have;
    - the kernel calls the tasks in and finishes the batch later, handing the result in a hook of its
      own; this is the withdrawn `on_process_called_in` again;
    - the kernel calls the memory in before every hook that can fire with other threads of an `Exact`
      process running, so the batch never meets this case; every watched access then stops every
      thread of the process.
12. **New. Reading a task's progress count, and stopping a task at a given count, under `Exact`.** The
    signals ruling defers both to this plan ("Later, with the threads plan"). They are what replaying a
    signal or a hand-over at a position needs, and what a replay of a process with several threads
    needs to end each turn where it ended (4.6). What exists: the count in code given `Exact`, the
    system's `ran`, counted turns (`Turn::Counted`), and the plan's `position()` on a held thread
    (section 8). Recommended: a read of (count, pc) on a stopped task's handle, and a counted turn as
    the one way to stop at a count, asked of the system through `turn` as today; a stop asked through
    `stop_thread` is then something a replaying system may decline. Cost: nothing on code that is not
    `Exact`. The other answer, a call that stops a running task at a future count from outside, needs
    the executor to compare the count at every stop check against a value another host thread sets,
    which `Exact` code does not do today.
13. **New. Priority-inheritance locks across machines.** The two agreed calls do not cover the six
    priority-inheritance operations, where the kernel records the holder in the word, boosts it, and
    hands the lock to the best waiter. The interface notes name two ways, not yet discussed with you:
    full support (a hand-off call, waiter priorities, boosting across machines), or the embedder keeps
    the tasks that contend on such a lock on one machine, which it can see at the first such call
    through the proxy. Not recommended here; the plan has nothing that tilts it.
14. **New. A wake that waits for another machine, and what `wake_waiters` and `move_waiters` return.**
    The count a wake returns must be exact, so a waking thread waits for the other machine when that
    machine has a waiter. **Proposed by the orchestrator, to be confirmed by you: that wait never
    happens inside a hook while the kernel's lock is held.** Two of the answers the form before
    offered did exactly that (a `Break` from the request hook after both wakes, "which still waits
    under the lock"; and the wait under the lock "accepted until the lock is split"). The revision's
    check showed that they deadlock two kernels, not only stall them: a thread on K1 waits in its hook,
    holding K1's lock, for K2's count, while a thread on K2 does the same for K1's, and each embedder's
    `wake_waiters` needs the lock the other holds; splitting the lock does not help, because the
    waiter lists stay under whichever lock the waiting hook holds (4.5). Both are withdrawn. The plan
    adds, as its own rule and not the orchestrator's, that the embedder never holds one kernel's handle
    while it calls or waits for another, which the second check found to be the same circle outside
    hooks. What is left to rule:
    - **one ordering point for each lock, and how it is chosen.** Proposed by the orchestrator, to be
      confirmed by you: every operation on one lock word that spans machines (wakes, requeues, and the
      changes the kernel makes to the word) is put in order at one point, one kernel or one sequencer of
      the embedder's. The first correction's "local part first, remote part later" is withdrawn: the
      second check showed, from the wake-result-use report, that it lets glibc 2.17's condition
      variable lose a wake although the waker waits, and makes a requeue two steps (4.5 walks it). How
      the point is chosen is yours; the options, with what each costs (expected, nothing measured), are
      in 4.5: the kernel that holds the lock's page; a fixed home kernel for each memory object; the
      embedder as sequencer. **None is recommended here.** Each must also say what it does with an
      operation on two words whose points differ, with the embedder's index after a requeue made on the
      waiters' own kernel, and with the wake at a thread's end, which goes without waiting (known
      fault 11);
    - **how the call is finished from outside the hook.** Either the waking call parks, before it has
      had any effect, as any waiting call does, and the embedder, from outside any hook, has its step
      made at the ordering point and finishes the call with the count; or the proxy's request hook
      answers "break" and the call is finished later from outside the hook. Either needs a way for the
      embedder to finish a parked call with its result, which the ruled proxy does not have: its `Break`
      carries the response at once; and that way must reach a task that has been taken out after its
      step and before its call was finished (decision 19). Recommended: the first, with that one call
      added, because the parked call is already the kernel's one way of waiting (4.1). Cost (expected):
      a waking call that crosses machines leaves `Execution::run` and comes back (300.8 ns measured for
      that alone, section 14) besides the round trip;
    - what both calls return: the tasks they woke or moved, proposed in the interface notes and not
      confirmed. With the per-task index of 4.5 the embedder needs them to keep its index exact.
    The investigation of call sites that never read the count (interface notes; its report,
    orchestrator/wake-result-use/REPORT.md) would let the wake go without waiting where it is proven,
    under that report's conditions, which 4.5 lists; its second condition names the ordering point.
15. **New. Fairness between a `Serial` task and its `Parallel` overlappers.** The mixed-ordering check
    puts it to you: whether a `Serial` task that becomes ready calls its overlappers in at once, or at
    the end of their slices. The plan's choice, made for you and not a ruling (section 3, rule 3, which
    section 5 and section 11 now say the same way): the task is "due a turn" when no ready overlapper
    has less weighted time; from then on its overlappers are handed out no more, those with a fair
    slice come in at its end, and those with none are asked in at once. Recommended: it needs nothing
    new, and it also keeps two `Serial` tasks from shutting the `Parallel` ones out between them. Cost
    (expected, section 3): the task waits up to the longest slice among its handed-out overlappers,
    and each switch is N call-ins and N hand-outs. (The form before said here that the task "calls
    every running overlapper in at once", which section 3 and section 5 did not say.) It narrows the
    mixed-ordering check's "while a writer is pending" to "due a turn"; that no `Serial` task then
    waits for ever rests on weighted time and is unchecked (known fault 1). The other answers: call
    every overlapper in as soon as the task is ready, which is the check's wording and costs the
    `Parallel` ones their slices; or mark them as soon as the task is ready but wait for their slices.
16. **New. An `Exact` `Serial` task's inputs from shared memory.** No hook fires when a task is handed
    out, and on the native executor a `Parallel` overlapper's stores set its own dirty marks, not the
    `Serial` task's (section 3). So an embedder cannot log, at the `Serial` task's position, what the
    others wrote between its turns. The two answers the check names: a hook at the task's hand-out with
    the shared ranges others changed since it last ran (one more hook, and a walk of the overlappers'
    dirty marks at each switch), or a take of dirty memory that reads across every place showing a
    page (one call, paid only by an embedder that asks). Recommended: the second, because it costs
    nothing to an embedder that does not record; but the moment to call it still needs a hook or a
    stop, so it needs decision 12 too.
17. **New. The order of a process's threads' turns in a replay.** With replay per process, the kernel
    no longer logs which thread of a `Serial` process ran when; that choice depends on state other
    processes move (4.3), which a replay of the process alone does not have, and futex calls between
    its threads must run for real (4.6). It belongs with the two primitives the ruling says are not
    designed. Answers the plan sees: the replaying system takes out every task of the process but the
    one due next (`suspend_tasks`, then `resume_tasks` at each turn's end), which uses only ruled calls
    and moves records at every turn; or the primitive that runs a process from its log is told which
    of its threads runs next, which is one more thing on the interface. Recommended: the second, with
    the first as a way to test before it exists; not ruled.
18. **New. Which thread the hooks for the kernel's writes and for watched accesses name for a write from
    another process.** Both ruled hooks take a thread, and a `write` to a file a process maps, or
    `process_vm_writev`, has no thread of that process behind it (section 8). The form before
    recommended making it optional, none meaning "the kernel, for another process". That stays the
    recommendation. The other answer, naming the writer's thread, names a task of another process in a
    hook about this one, which a hook cannot look up.
19. **New (2026-11-20). What a task taken out while it waits carries.** Proposed by the orchestrator, to
    be confirmed by you: **taking a waiting task out of its kernel is treated exactly as Linux treats a
    signal with no handler arriving during that call.** A call that has had no effect yet runs again
    where the task is resumed (a futex wait compares its word again; a call waiting for a call-in has
    its marks taken off and makes them again where it runs); a wait with a relative time limit goes on
    with the time left; a call that has already had an effect is not run again and returns what it did
    (a partial count, a wake's count); a waiter already chosen by a wake returns woken (4.5, section
    6). This replaces the first correction's "its call runs from its start", which the second check
    found would wake twice for a waker taken out after its step, apply `FUTEX_WAKE_OP`'s operation
    twice, and write a pipe's bytes twice. The kernel already has the path, for a signal with no
    handler (kernel/signal.rs:628-638). Two readings are the plan's own, to confirm with this: "the
    time left" is counted from the deadline the kernel already keeps (kernel/syscall/futex.rs:274-286),
    so time between kernels counts and a deadline that passed meanwhile gives `ETIMEDOUT`, as Linux's
    restart counts time stopped, which needs the kernels' clocks to agree; and the call run again
    passes through the proxy's request hook again, as a restarted call is seen again by a tracer on
    Linux. Why at all: the revision's check found a lost wake when a waiter is moved between kernels
    (4.5 walks the case). It closes that case where the waker changes the word before it wakes, as
    locks, condition variables and once-flags do, and where the memory system shows that change on
    the machine where the call runs again (expected, not checked); a program that waits on a word
    nobody changes meets Linux's own window for a restarted wait, at every move. Cost: nothing on a
    call that is not taken out; a resumed wait makes one more kernel entry than a wait that travelled
    whole. The other answer, the waiter list travelling with the task, needs a call that wakes a
    `SuspendedTask` and the embedder's index kept in step with every move, which the first check found
    nothing in the plan or the rulings to do.
20. **New (2026-11-20). How tasks that must take turns are placed across kernels.** The rule counts
    only the tasks one kernel holds, and "the kernel must be able to move one task apart from its
    siblings" ("the embedder-side view `WorkloadPlacement`"). Two tasks of one `Serial` process on two
    kernels, or a `Serial` task and an overlapper on two, would run `Serial` code beside each other,
    which watches cannot order (section 3; known fault 9). The answers the revision's check names, none
    recommended over the other yet:
    - only tasks that run as `Parallel` and have no overlapper that runs as `Serial` may be placed apart;
      a `Serial` process, and a `Serial` task with its overlappers, stay on one kernel. Cost: such
      tasks cannot be spread at all;
    - the embedder keeps such tasks taking turns across kernels, at most one of them handed out
      anywhere at once. Cost: a distributed turn, with a round trip at each switch, and something on
      the interface for a kernel to ask before it hands such a task out, which the ruled rule ("nothing
      on the interface") does not have.
    Until you choose, the plan asks the embedder for the first, and no kernel checks it.
21. **New (2026-11-20). Which processes the shared-memory hooks name when a source becomes writable
    through a mapping.** The ruling fires `on_shared_memory_mapping` per process at its own mapping, and
    "only for sources some process can write through a mapping". So a process that mapped a file
    before any mapping could write it is never named when another process later maps it shared and
    writable, although it now overlaps (section 3; known fault 10). Suggested, not ruled: in the entry
    where a source first becomes writable through a mapping, the hook fires for every process that
    already shows it, and `on_shared_memory_unmapped` likewise for each when the last such mapping
    goes. Cost: one hook call for each such process at that moment, and nothing at any other. The
    other answer: the embedder is told nothing, and its view misses those processes.
22. **New (2026-11-20). Under `CLONE_VM`, what an owner given `None` pays for a co-owner given
    `Exact`.** "Where replay is recorded" rules that a process given `None` "costs nothing and breaks
    no neighbour", and the proposed section after it says "the replay guarantee is never forced onto a
    neighbour". Under `CLONE_VM` two processes own one memory, and a call-in's mark names a memory: so
    while the `Exact` owner's threads come in for a change of the kernel's, a thread of the `None` owner
    that comes in meanwhile, for any reason, is not handed back until the mark is off (4.5; the second
    check, its sixth finding; known fault 12). The answers the plan sees, none recommended: a mark that
    names a memory and, under `CLONE_VM`, the owners it calls in, so that the `None` owner's threads
    are handed back and held by the group stop when the call runs (a suggestion, not built or checked;
    it adds a case to the model of known fault 1); or the cost is accepted for `CLONE_VM` as the
    ordering already is (section 3), bounded by the time the `Exact` owner's threads take to come in
    (the stop check's latency, 230 ns on average and 30 µs at worst measured in Chrome, plus 300.8 ns
    measured for leaving `Execution::run` and coming back). Whether the ruling asks for the first is
    yours.
