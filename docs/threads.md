# wcontainers: guest threads in parallel

Status: plan, 2026-11-18, corrected after its first independent check (docs/status-threads.md says what
changed). Nothing here is built. The owner rules on section 16 first.

Words used here:
- **executor**: what runs the guest's instructions behind the kernel's `Cpu` side (`Executor`,
  rt/container.rs:19). Three exist or are being built: the WebAssembly executor (wasmtime), the native
  executor (a virtual machine, docs/native.md on origin/phase-2-native) and the kernel tests' stand-in.
- **WebAssembly compiler**: what compiles WebAssembly to the host's machine code and runs it: wasmtime,
  or a browser's V8.
- **embedder**: whoever implements `System` (rt/lib.rs:146) and drives a container.
- **block**: a block of shared pages (`SharedId`, abi/cpu.rs:14-16). The kernel shows every mapped file
  through one. A memory **shows** a block's page where a mapping of it puts that page.
- **group**: every guest thread that can reach the same guest memory without the kernel in between
  (section 3). The owner's rulings are per group. A name is suggested in section 16.
- **stopped**: a guest thread is stopped when it is inside a hook or has left `Execution::run`. This is the
  translator's own word (tr/contract.rs:17-27). A stopped thread runs no guest instruction.
- **called in**: the kernel asks for a running thread with `thread_stop`, and the thread comes back by
  itself through `Kernel::exit`. A call that needs it back first parks, as any call that waits does.
- **group stop**: the executor holds every other running thread of a group in a hook, in place, while it
  makes one change.
- **processor**: one of the CPUs the container is told it has. It is the kernel's own object. One guest
  thread runs on a processor at a time.
- **worker**: a host thread inside the loop. A worker is not a processor (4.3).
- **`run`**: the call that runs a container, as the owner named it (section 2). In today's code it is
  `Stepper::step` (rt/lib.rs:197). `run_until_finished` is today's `drive` and `Container::run`
  (rt/lib.rs:230, rt/container.rs:172). The executor's own call that runs one guest thread is always
  written `Execution::run` here (rt/lib.rs:128-143).
- **kernel entry**: one call into the kernel that changes it: a thread handed out (`Kernel::next`), a
  thread coming back (`Kernel::exit`), an event, a signal from outside.
- **barrier**: a guest instruction that orders its thread's memory accesses as other threads see them:
  ARM64's `dmb` and `dsb`, x86's `mfence`.
- **position**: where a thread is, as (progress count, pc) (docs/runtime.md section 9).
- **reference**: a default Docker container on real Linux of the guest's architecture.

Paths are shortened: `kernel/` is crates/wcontainers-kernel/src, `abi/` crates/wcontainers-abi/src, `rt/`
crates/wcontainers-runtime/src, `wt/` crates/wcontainers-wasmtime/src, `tr/`
crates/wcontainers-translator/src, `arm64/` crates/wcontainers-arm64/src. Lines are at 77e50ddc (this
branch's base). `native.md` and `x86 plan` are docs/native.md at origin/phase-2-native ab448e58 and
docs/phase-2-guest-cpu-and-x86.md at origin/phase-2-x86 0de30f18. `interface notes` is
wcontainers-scratch/orchestrator/next-embedder-interface.md. Every figure says whether it was measured or
is only expected. Measurements are in section 14 and in docs/status-threads.md.

## 1. What this delivers

Today one guest thread runs at a time in a whole container. A build with sixteen jobs on a sixteen-core
machine uses one core. This plan lifts that on every executor, in this order:

1. **Different groups run at the same time**, each group still running one thread at a time. A build's
   compiler processes are separate groups, so this alone uses the cores. Translated code does not change.
   Replay stays exact.
2. **The threads of one group run at the same time**, where its ordering says so.
3. The same on the native executor, and for x86-64 guests.

It also settles what stays exact for replay, what the embedder composes itself, and what is refused until
it is built.

## 2. Rules, and what is already ruled

All rules of phase one hold: one primitive over parallel paths; no setting without a real use today; never
fake; every slow path has a name, a counter and a warning; run speed first; scope is whole architectures.

Ruled by the owner on 2026-11-18 (brief-threads.md, interface notes), and taken as given:
- `enum ThreadOrdering { Serial, Parallel, ParallelOptimistic }` is how one group's threads are run.
  `ThreadOrdering::replay_determinism()` gives `Exact`, `None` and `External` for them.
- Different groups always run in parallel with each other.
- The kernel is the same under every executor. It is entered by one caller at a time to begin with. The
  interface must not prevent several at once later.
- Using cores by running more kernels of one container is rejected.
- Exact replay of a group that runs in parallel is not a kernel setting. The embedder composes it from
  per-thread memory watches (`memory_watch_thread`, on executors that implement `PerThreadWatch`).
- `ParallelOptimistic` is planned for as a third value. Nothing may depend on its being useful.
- Taking a process out of its kernel is `suspend`, putting it back is `resume`, and the value between is
  `SuspendedProcess<E>`. Neither converts anything. `export` and `import` only make and read the form
  that crosses a wire (interface notes lines 110-123). `resume` takes `ExecutionConfig<E> {
  thread_ordering, translator }` (lines 131-133), and the getter is `Process::execution_config()`.
- Long-lived configuration types end in `Config`: `RuntimeConfig`, `TranslatorConfig` (brief-threads.md
  lines 124-129). The code still says `RuntimeSettings` and `TranslatorSettings` (rt/container.rs:13,
  :23); the runtime track renames them.
- The call that runs a container is `run`. It returns `StopReason { Interrupted, Suspended, Idle {
  until }, Finished { status } }`. Today's run-to-the-end call becomes `run_until_finished`. Suspending
  a process inside a hook makes `run` return at once (interface notes lines 148-150, 157).
- **Inspection follows Linux's own line**, the `ptrace_may_access` check (interface notes lines 104-109,
  118-119). Inspection is: `/proc/<pid>/mem`, `maps`, `smaps`, `environ`, `auxv`, the address fields of
  `stat`, ptrace itself, `process_vm_readv` and `process_vm_writev`, a crash dump. Not inspection:
  `stat`, `status`, `cmdline`, the thread list. The hooks `on_inspection_starting` and
  `on_inspection_ended` are the embedder interface track's to build. What this plan adds is only which
  threads are called in first (4.5).

## 3. Groups

**What a group is.** A guest memory is one address space (`MemoryId`, abi/cpu.rs:5-7). The threads of a
process share one, and so do processes made with `CLONE_VM` (kernel/process.rs:70-72). Beyond that,
memories are joined through blocks:

- A block is **writable through a mapping** when some memory has a shared mapping of it (`mmap` with
  `MAP_SHARED`, System V shared memory, a shared memory file) that may write. "May write" means what
  `mprotect` may later give the mapping, Linux's `VM_MAYWRITE` (`Mapping::allowed`,
  kernel/memory.rs:74-75), not the permission it has now. So `mprotect` never changes a group.
- **Every memory that shows a page of such a block is joined to every other**, whether its own mapping of
  it is shared or private. A private file mapping shows the file's live bytes until the page is first
  written (kernel/memory.rs:44-50; `map_shown` maps the block where `mapping.shared || !own`,
  kernel/shared.rs:476-487). So a writer's stores reach it with no kernel entry in between, and it must
  be in the writer's group.
- A block that no mapping may write joins nobody. This is the common case: the kernel shows every file
  mapping through a block, the program's own code and its C library included (kernel/shared.rs:1-5,
  kernel/loader.rs:901-905). Its bytes can still change through `write` to the file. That is a kernel
  entry, and 4.5 says who is called in for it.

A group is all threads of memories joined this way, directly or through others.

Joining is by block, not by page. Two processes that map different parts of one file, one of them
writable, are joined although they share no page. That is rare, and a group left too large is slower,
never wrong. Rejected: joining by page. It makes every `mmap`, `munmap` and first private write a
question about pages, to save a case that hardly occurs.

**Who keeps it.** The kernel. It already knows every mapping, which are shared and what each may be given
(`Backing` and `Mapping`, kernel/memory.rs:34, :69). It keeps, for each memory, the group it is in, and
for each block the memories that show it. Memories join inside the kernel entry that makes the mapping,
or the fork that inherits it. A group is split again when the mapping that joined it has gone, worked
out at a later entry of the group.

**A new group has one thread.** A group is born at the container's start, at a fork whose child shows no
block that is writable through a mapping, and at exec, which makes a new memory (kernel/loader.rs:292)
after ending the process's other threads (kernel/task.rs:648). For a group with one thread `Serial` and
`Parallel` are the same run. The ordering only starts to matter at the group's second thread.

**The ordering in force.** The ruling gives the setting to the group. But settings are given to processes
(at the container's start, and at `resume`), and groups merge at any `mmap`. So two processes with
different orderings can come to share memory, and one rule must say what is in force then. This plan's
rule: the group's ordering in force is the strictest of its processes' orderings (`Serial` is strictest,
then `ParallelOptimistic`, then `Parallel`) and of what the executor can run for this group (7.4 and
section 9). It is worked out again whenever the group's members change, which is always inside a kernel
entry.

This departs from a plain reading of the ruling in one way: a process can be run more strictly than it
was told. It is the owner's to confirm (decision 2), with what `Process::thread_ordering()` then answers.
The alternatives are there. Whatever he rules, the kernel needs "the ordering in force of a group can
change while it runs", because groups merge.

How a change takes effect:
- **Stricter** (towards `Serial`): the group's other running threads are called in (4.5). The entry that
  leaves at most one of its threads running makes the change. Until then the `mmap` that caused it is
  parked, or the threads of the process that was resumed are not handed out.
- **Looser**: at the group's next entry. Nobody is stopped.

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

**One lock a system call.** A system call answered in place is two entries today: `exit`, then `next`,
with `System::arrived` between (rt/lib.rs:264-270). The loop takes the lock once for both. It asks the
kernel for a short-lived handle (suggested `Kernel::enter() -> KernelEntered<'_>`), calls the entry
points on it, and the lock is let go when the handle is dropped. `arrived` is handed that handle, to
give the kernel its events through, and runs with the lock held, as a `Host` call does. After the split
the handle keeps whichever locks its entries took.

**What the lock costs one thread.** Measured (section 14): taking and letting go an uncontended
`parking_lot` lock could not be told from noise on this Mac (under 0.3 ns), and cost 7 to 9 ns on the
x86 laptop. The kernel's share of `getppid` is 116 to 120 ns on this Mac (docs/runtime.md section 18);
it was not measured on the laptop. So T1 is expected to make a `Serial` system call on an x86 host a few
per cent slower. That cannot be removed while the kernel is `Sync`. It is put to the owner (decision 7).

Rejected: a kernel type without a lock for containers with one processor. It needs the kernel compiled
twice into one program, or the embedder to choose at compile time.

**The state stays `Rc` for now.** One `unsafe impl Send` on the kernel's state, with its reason: every
`Rc` and `RefCell` is reachable only through the state, the state is reached only under the lock, and no
clone leaves it. A test checks that no public type of the kernel holds an `Rc`. The handle a hook is given
(`Process<'_, E>`) borrows under the lock and cannot outlive it.

Rejected for now: turning every `Rc` into `Arc` and every `RefCell` into a lock. It is the right end state
for the parts that are split out (4.7), and it costs every system call something. It is paid part by part
at the split, each measured.

### 4.3 Processors and workers

- The container has a fixed number of processors (section 11). The kernel keeps, per processor, what it
  keeps once today: the current thread, the give-way mark, the running thread, the picked runtime
  (kernel/lib.rs:136-140, :165).
- **A worker is not a processor.** A worker says which processor it is asking for: `Kernel::next(processor)`.
  In a live run each worker keeps one processor (section 5). In a replay the log says which processor
  each step is for, and any worker makes it (4.6). So a run may be replayed with fewer or more workers
  than it was recorded with, and the guest sees the same processor numbers.
- One run queue, as today. `next(processor)` gives the processor its current thread if that need not give
  way (as `pick` does today, kernel/task.rs:18-41), else the waiting thread with the least weighted time
  whose group may run one more thread: any thread of a group in force `Parallel`, and a thread of a
  `Serial` group only when none of that group runs. Which thread it gives depends only on the kernel's
  state and the processor asked for.
- **`next` is a kernel entry when it hands a thread out.** It changes the run queue, the processor's
  thread and the thread's processor, and the guest sees the last through `getcpu` and `rseq`. It takes
  its place in the order of entries like any other (4.6). When nothing can be handed out it answers
  `Idle` and marks the processor idle (section 5). That changes nothing the guest can see and nothing a
  replay must repeat, so it is not an entry.
- `exit` names the thread that came back (today it is implied, kernel/lib.rs:318-321).
- Each thread is charged from its own "handed out at" time (section 11).
- `sched_getaffinity`, `getcpu`, `/proc/cpuinfo`, `/proc/stat` and `rseq`'s CPU fields answer from the
  processors.

Rejected: a host thread is a processor (this plan's first form). Which host thread asks first is the
host's timing, and a replay with fewer host threads could not give the same processor numbers.
Rejected: `next` without a processor, the kernel choosing a free one. A thread that carries on in place
after its system call must get its own processor's answer, or it leaves `Execution::run` at every call (300 ns,
docs/runtime.md section 10).

### 4.4 What a system call may assume

1. No other kernel entry is in progress (the first form). After the split (4.7): no other entry that
   touches the same things.
2. Its own thread is stopped, with its registers saved (tr/contract.rs:13-15).
3. In a group in force `Serial`, no other thread of its group is running guest code.
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
   - **A thread of a group in force `Serial` stops only at a kernel entry of its own.** Its stop then has
     a place in the order of entries, and a position. So the kernel never reads or changes the memory,
     the page permissions or the code of such a group from outside it while its thread runs. It calls
     the thread in first.
   - **A thread ends only inside a kernel entry of its own.** The kernel never ends a running thread from
     another thread's entry.
6. The kernel still never waits on its own stack. A `Host` call or a hook made inside an entry holds the
   lock while it runs. A slow hook stalls every processor's kernel entries until the split.

One known gap, to be closed by reading every call in T2: a call that waited and runs again reads its
arguments from guest memory again (kernel/task.rs:166-169, :360-368), except paths and descriptors, which
are kept (`Taken`, kernel/process.rs:407-414). Linux reads them once. With one thread running, nothing
could change them in between. In a `Parallel` group another thread can.

### 4.5 Who is stopped before what

Three primitives do all of it.

**Stop one thread** (suggested: a `Host` service `thread_stop(thread)`). The kernel asks the system to
bring a running thread back. The system raises the thread's stop flag, which is one atomic store from any
host thread (abi/registers.rs:54-57). The thread comes in by itself as `Exit::Stop` at its next stop check.
The kernel does not wait for it. It goes through the system, not straight to the executor, because when a
thread stops is one of the things a replay must repeat (4.6): the slice timer already works this way
(rt/system.rs:256-268).

**Call a group in** (the kernel's). The kernel asks `thread_stop` for each running thread of the group and
marks the group wanted, so that `next` hands none of its threads out. The call that wanted it parks. It
is woken by the entry that brings the group's last running thread in, runs again, and the mark is taken
off. Each thread that came in did so through an ordinary entry, at a position. Nothing can wait in a
circle: the mark keeps threads from being handed out, never a woken call from running.

**Group stop** (the executor's), done inside the `Cpu` call that needs it, on the group of the memory the
call names. The WebAssembly executor raises the flags of that group's other running threads and waits
until each is in a hook, where it is held. The native executor interrupts their virtual CPUs (section
10). A thread waiting for the kernel's lock is in a hook, so it counts as stopped. A group stop holds
threads in place: where each stopped is not recorded. So it is used only for groups in force `Parallel`,
which promise nothing about that. In a `Serial` group it finds nobody to stop: within the group only the
caller runs, and from outside the kernel has called the group in first. The executor counts "group stop
reached a running thread of a Serial group", which must stay at zero.

What is stopped before what, where M is the memory changed:
- **A page loses a permission, is unmapped or remapped; code is dropped** (`set_permissions`,
  `shared_unmap`, a mapping going away, `code_changed`, ARM64's `ic ivau`, a write to translated x86
  code). This is the translated code's rule (tr/contract.rs:17-27).
  - M's group is in force `Parallel`: a group stop of it. After a code drop each of its other threads
    also leaves `Execution::run`, so that none comes back into a piece made from the old bytes (docs/runtime.md
    section 6), counted "left run: code changed under it".
  - M's group is `Serial` and the caller is in it: nobody else runs.
  - M's group is `Serial` and the caller is not in it: the group is called in. This happens when a file
    changes size or bytes: the kernel then lowers permissions and drops code in every process that maps
    the file (`changed`, kernel/shared.rs:581-600). Counted "another group called in for its pages".
- **A page gains a permission, or a new mapping appears**: nobody. A thread that still acts on the old
  table faults to the executor, which looks at the table again and carries the thread on. Counted "fault
  settled by the table as it is now".
- **The kernel reads or writes M's bytes from outside M's group** (a `write` to a file reaching pages M
  shows, `process_vm_readv` and `process_vm_writev`, a tracer, `/proc/<pid>/mem`, `environ`, `cmdline`).
  - M's group is `Serial`: it is called in. The read or write then has one place in the order. Counted
    "another group called in for its memory". `cmdline` is not inspection (section 2), but it is still
    such a read.
  - M's group is `Parallel`: nobody. The bytes land while its threads run, as on Linux. The executor's
    copy into M is made with pointer writes, since "no guest runs" (wt/lib.rs:1423) no longer holds.
- **Another thread's registers are read** (a tracer, a crash dump): that thread is called in, whatever
  its group's ordering.
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
- **fork from a threaded process**: in a `Parallel` group, a group stop inside `memory_duplicate` for the
  length of the copy, so that no page is copied half-written. Counted "group stopped for a fork".
- **A signal for a running thread**: that thread only, by `thread_stop`, and nobody waits. It takes the
  signal on its way back to the guest, as every thread does (kernel/task.rs:326-339).
- **A stop signal, `SIGCONT`, a tracer**: the process's running threads, by `thread_stop` for each. Each
  parks itself when it comes in.
- **A futex wait or wake**: nobody. Both are kernel entries, so the wait's "compare, then park" and the
  wake are already in one order.
- **`membarrier`**: a group stop of the caller's group when it is in force `Parallel`, and for the
  commands that mean every process, of every such group. Each held thread has passed a full barrier. A
  `Serial` group needs nothing: one of its threads runs at a time, and the hand-over between them goes
  through the kernel's lock.
- **The ordering in force becomes stricter** (section 3): the group is called in.
- **The container's first process ends**: every other process is ended as by a fatal signal, as Linux
  ends a container. Each worker's `next` then answers `Finished`.

### 4.6 The order of kernel entries, recorded and replayed

With every group `Serial`, no two groups reach the same bytes except through a kernel entry:
- memories that can see each other's stores with no kernel entry between are one group, by section 3;
- the WebAssembly executor's copies of written shared pages at a turn's end (`flush`, wt/lib.rs:1341)
  reach only memories that show the same block's pages. Those are in the writer's group, and none of
  them is running;
- everything else that crosses groups is done by the kernel inside an entry, with the other group
  called in first (4.5).

So a run is fixed by what is recorded today (the host's answers, each event and each turn's end with its
position, docs/runtime.md section 12) and one new thing: the order in which kernel entries took effect.
A clock read without a system call (`Answer::clock`, rt/lib.rs:120-122) is a host answer of its thread.
It changes nothing in the kernel and needs no place in the order.

**One recording point.** Before a kernel entry takes effect, the kernel asks the system whether it may,
with the lock held (suggested: a `Host` service `kernel_entry_admit(entry) -> EntryAdmission`, with
`Now` and `Later`). The entry says what it is: a thread handed out (the processor and the thread chosen),
a thread come back (the thread and why), an event, a signal from outside. The system already hears each
thread's progress (`System::ran`), so it has the position.
- A system that records answers `Now` and writes the entry down. The order of its log is the order of
  effects, because it was asked under the lock.
- The provided default answers `Now` and writes nothing.
- A system that replays answers `Now` when the entry is the next one in its log, and `Later` otherwise.

**An entry told `Later` never holds a host thread.**
- A thread come back too early: the kernel keeps the thread and its exit aside and returns. The thread
  leaves `Execution::run`, and its worker is free. After each entry that takes effect, the kernel offers the kept
  ones again.
- A thread to be handed out too early: `next` answers `Idle`.
- An event too early: the replaying system delivers its events itself, from its log, in their place.

**How a replay proceeds.** It is not `run_until_finished`. It is the same `run`, called in the log's
order: the replayer takes the log's next "thread handed out" entry and calls `run` for its processor,
from any free worker. With one worker it walks the log. With more, several threads run their turns at once and
only their entries are put in order. It cannot get stuck: the entry that is due is a hand-out, which a
free worker makes; or the return of a thread some worker is running, which will arrive; or an event,
which the system delivers. A return that is kept aside is never the due one, since it is offered again
the moment it becomes due. A worker whose thread came back early is free again at once, so even one
worker always has something to do.

**Stops must come from the log too.** In a replay a thread must stop where it stopped in the recording, not
where a flag happens to reach it. So the replaying system gives each turn a counted limit that ends it at
the thread's next recorded position (`Turn::Counted`, rt/lib.rs:25-27), and its `thread_stop` does
nothing. This is why `thread_stop` is asked of the system (4.5), and why `System::turn` is told the
thread (section 5). A group stop never stops a thread of a `Serial` group (4.5), so there is nothing of
it to repeat.

**After the split** (4.7) the point is the taking of any lock that is not the entering group's own, asked
with that lock held. An entry that stays inside its group (`brk`, a private `mmap`, a private futex,
`sigaction`) is then not asked about and waits for nobody, in a recording and in a replay. This is the
brief's rule: what another process or the system can observe goes through the one point, what touches
only the process's own state does not, and the kernel keeps no list of system calls.

**What this needs of other tracks.** An exact position at full speed needs the translator's progress
count (docs/runtime.md section 9, not built). Until then stop checks stand in for it, which repeats at one
look-ahead, as the kernel tests' `repeats` does today.

Rejected: the kernel hands out sequence numbers and makes an early entry wait for its number. A thread
that waits holds its host thread, and with fewer host threads than the recording had, the entry it waits
for has nobody to make it. Rejected: a recording hook that returns nothing. It cannot gate a replay.

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

**The change: the same `run`, called by as many host threads as the embedder gives, each naming a
processor.** `run` and `run_until_finished` take the kernel, the executor and the system by `&`, and a
processor. `run_until_finished` keeps its processor until it returns. One host thread calling it for processor 0 is today's
behaviour exactly. The embedder's limit on host threads is simply how many of its threads call it.
`Container::run_until_finished` spawns them itself, one for each of the first processors, up to a count in
`RuntimeConfig` (suggested `host_threads`, default the host's
`std::thread::available_parallelism`). Two callers naming one processor at once is a caller's error,
which the kernel answers instead of today's assertion (kernel/lib.rs:306-309).

Rejected: a pool of worker threads hidden in the runtime. An embedder with its own event loop (the owner's
other project drives many kernels) must decide which of its threads work for which container. Rejected: a
second loop for the parallel case. One `run`, entered N times.

What changes with it:
- `Execution::run(&self, ...)` (rt/lib.rs:128-143): several host threads are inside it at once, each
  with its own thread. The executor does its own locking (section 7).
- **The system is told who.** `turn` is told the processor and the thread, `ran` the thread, `arrived`
  and `idle` the processor (today none is, rt/lib.rs:150-164). A replaying system needs the thread to
  find the turn's recorded end. The local system needs the processor for its deadline.
- **Idle, without a lost wake.** `next(processor)` that answers `Idle` marks the processor idle, under
  the lock. Any later entry that leaves a thread ready that may run takes the mark off the lowest-numbered
  idle processor and announces it (suggested: a hook `on_processor_needed(processor)`), once for each
  such thread. Threads become ready in `exit`, `event` and `kill` alike, and in what `arrived` hands in.
  The system's `idle(processor, until)` returns at the time, at an event from outside, or at that
  announcement. The announcement must be kept if it comes before the worker has reached `idle`: `idle`
  then returns at once. This is the rule of `std::thread::park` and `unpark`, and the local system uses
  exactly those. The mark is made before any later entry can run, so no wake is lost. A worker woken for
  nothing asks `next` again and is told `Idle` again.
- **One timer for the container.** Every idle answer carries the kernel's next deadline, as today. The
  local system keeps the earliest and sends one `Event::Timer` for it, from whichever idle worker reaches
  it first.
- **Stopping a thread.** `StopFlag` is replaced by a handle the executor gives out (suggested
  `Executor::thread_stopper()`, in place of `Executor::stop_flag`, rt/container.rs:35-36), with one
  method any host thread may call: `stop(thread)`. The flag to raise now has to be named. The system is
  handed it as it is handed the flag today (rt/lib.rs:164-166). Its `thread_stop` service and its timer
  thread call it. The timer thread keeps one deadline per processor.
- **A thread never leaves without its exit**, except the thread whose own entry ended it. Today a thread
  the kernel dropped breaks out of `Execution::run` with no `Kernel::exit` (wt/lib.rs:2303-2306). By 4.4 a thread
  ends only in its own entry, so the processor's running thread is always cleared by that thread's own
  `exit`, and `next` finds it so.
- **A container stop.** `System::stopped` runs when no thread is in `Execution::run` (rt/lib.rs:156-158), which a
  save needs. With several workers the loop first asks every running thread in, waits until every worker
  is out of `Execution::run` and no entry is in progress, then calls it once. This is also how an embedder
  interrupts a running container from another thread, which the interface notes leave open (line 92).
- `Host` and `System` methods take `&self`, so a system keeps what it changes behind its own locks.

## 6. Changing the ordering of a running process: two forms weighed

**Form A, a setter.** `Process::set_thread_ordering(ordering)`, taking effect at the group's next stop.

**Form B, no setter.** The ordering is fixed while a process is in a kernel. It is part of the
`ExecutionConfig` given when the container starts and when a process is resumed. It is changed by
`suspend` and `resume` on the same kernel: `process.suspend()` gives a `SuspendedProcess<E>`, and
`kernel.resume(suspended, config)` puts it back with the new ordering. Nothing is converted and no
memory is copied: the process's records and its memory move out and back by ownership (interface notes
lines 110-114). `export` and `import` are not part of it. They are only for a process that crosses to
another kernel.

"The next stop" means the same in both: every thread of the process is stopped at once, and the kernel
makes that happen by calling them in (4.5). `suspend` has to do exactly that anyway.

- **Work in the kernel.** A: the setter, on top of what section 3 needs anyway. B: nothing beyond
  `suspend` and `resume`, which the embedder interface builds for other reasons and which are named and
  agreed but not built.
- **Work in the executors.** The same in both: none on the WebAssembly executor for ARM64 guests (one
  translation, 7.2; one byte for its barriers, 7.3); the native executor sets the process's page tables
  up again (native.md section 5.3, its last item on threads); an x86 guest on a weaker host would need
  its code translated again (section 9). In B it is done on the path of `resume`, which must do it for a
  process come from elsewhere too.
- **Cost of one change.** A: the threads called in, once. A stop flag was noticed in 230 ns on average
  and 30 µs at worst in Chrome (docs/phase-1-translator.md, "The stop check"). B: the same, and the
  process's kernel records taken out of the kernel's tables and put back (its threads out of the run
  queue and the waits, its timers, its place in its parent's children). Their count is the process's
  threads and waits, not its memory. From inside a hook the embedder can suspend but must resume after
  `run` has returned (interface notes lines 134-135), so a change made there also costs one leave of
  `Execution::run` (300 ns, docs/runtime.md section 10). Not measured: nothing exists to measure.
- **When the change cannot be made.** Never, in either. On the same kernel nothing is converted, so
  nothing a process holds can stand in the way. (Only `export` could meet something it cannot carry,
  and that is not this path.)
- **A consistent starting point for a replica.** A change towards `Serial` starts a promise of exact
  replay, and a replica needs the process's whole state at that moment. B hands it over: the
  `SuspendedProcess` is that state, whole and stopped, and `export` makes the wire form from it. With A
  the embedder must suspend at the same moment anyway to get one. A change towards `Parallel` needs none.
- **Ways a process gets an ordering.** A: two, given and changed in place. B: one. A reader of an
  embedder's code sees every change of what replay promises as a `resume` with the configuration
  written on it.
- **Interface.** A: one more method. B: none.

**A group of several processes.** `suspend` takes one process, and the ordering in force is the
group's (section 3). With the strictest winning:
- to make a group stricter, resume any one of its processes as `Serial`. The whole group is `Serial`
  once its other running threads have come in;
- to make it looser, every process of the group that was given the stricter ordering must be suspended
  and resumed. The embedder learns who shares with whom from the kernel (interface notes lines
  139-140, still open there). On the WebAssembly executor a group of several memories runs `Serial`
  whatever is asked (7.4), so there this only arises for processes made with `CLONE_VM`.
This is the same in form A: the setter is on one `Process` too.

**Recommendation: B.**
- The container's first process gets its `ExecutionConfig` at the start (through `Start`,
  kernel/container.rs:55, read by `Kernel::start`). A child inherits its parent's at fork and keeps them
  across exec.
- Until `suspend` and `resume` exist, that is the only way to give an ordering. This loses less than it
  seems: a group is born with one thread (section 3), where the ordering makes no difference, so "this
  container's threaded programs run in parallel" or "everything in this container replays exactly" is
  one value at the start.
- Choosing per program arrives with `suspend`, at `on_program_starting`, which the owner already named
  as the cheap moment to move a process: one thread, a fresh memory, few records.
- The reasons: the kernel gets no second path; a change of what replay promises is visible and comes
  with its starting point; and nothing has to be built for it in this plan.
- What it costs: no running process can change its ordering until `suspend` and `resume` are built,
  which is another track's work. And each change then costs the records' way out and back, where the
  setter costs only the stop.

If the owner prefers A, it costs one method and its tests. Nothing in this plan's milestones would have
to be undone for either answer.

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
  and exchange against the remembered value (:329-338). wasmtime, Node and Chrome accept atomic operations on an unshared
  memory (measured, ordering-cost/result.json), which is why this works today.
- **ARM64's barriers are translated to nothing**: `dmb`, `dsb` and `isb` become `I::Nothing`
  (arm64/decode.rs:704), "nothing happens in a single thread" (arm64/instruction.rs:989).

### 7.2 A shared memory, and one translation for both orderings

**Decision: the guest memory import becomes shared, always. One translation serves `Serial` and
`Parallel`.** A wasmtime store is used by one host thread at a time, and only a shared memory can be
imported by instances in several stores (wasmtime 50 API documentation: `Store`, `SharedMemory`). So
threads at once need the shared import. Whether `Serial` needs a second, unshared translation is a question
of cost only. With one translation the cache is not keyed by the ordering. Its key changes once, for the
shared import and its declared maximum (below).

One translation costs a `Serial` run two things. Both are measured (section 14).
- **The shared import itself: nothing that can be measured.** The benchmark set at full look-ahead with
  the import shared against unshared, instructions retired, eight workloads, alternating: between 0.16%
  fewer and 0.33% more, with both signs among the eight and no more than the same workload differed
  between two identical runs. An earlier measurement on hand-written kernels found the same in wasmtime
  49, Node and Chrome: "at most 5% either way, within the noise" (ordering-cost/result.json). The
  translator track measured the same in Chrome (docs/phase-1-translator.md, "The stop check"). The
  benchmark set has no barrier and no 128 bit atomic in it, so this says nothing about those.
- **One byte tested at each barrier and each 128 bit atomic: 0.1 to 0.8 ns each.** These are the
  instructions that must do more in a parallel group than they do today (7.3). Doing the parallel thing
  always would cost a `Serial` run 0.7 to 19 ns a barrier. So the translation tests a byte and does
  today's thing when it is zero.

So `Serial` gets slower by a load and a branch at each barrier and each 128 bit atomic it runs, and by
nothing else. The owner's rule is that `Serial` must not get slower, so this is put to him (decision 7).

Rejected: two translations, keyed by ordering. It costs a `Serial` run nothing at all. It doubles compile
time and cache size for every program that runs under both, makes a change of ordering a translation of
the whole program, and adds a path. Rejected: the barrier as an atomic operation always, with no test.
It is simplest, and it costs `Serial` what the next section measures.

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
  (src/runtime/instance.rs:554). So each process's memory is defined by a one-memory module. Read in the
  source, not run: it is T2's first task, with `Config::shared_memory` on as it is today. If it fails,
  wasmtime's own mapping serves: the kernel's access by pointer and the fork copy work on any mapping.
- **One store per guest thread**, always, in place of one per process. The compiled modules and
  wasmtime's `Engine` stay shared. A
  dropped thread's store is kept for the process's next thread, as its instances are today
  (wt/lib.rs:478-483). What a store costs in memory is not measured (section 15).
- **The executor becomes shareable.** Its tables of processes, threads and blocks go behind a lock. What it
  keeps per process (page records, code lists, shown blocks, wt/lib.rs:451-474) goes behind that process's
  own lock, taken by hooks of that process's threads. Order: the kernel's lock first, then a process's.
  A hook never calls the kernel while holding a process's lock.

### 7.3 Atomic instructions, exclusives and barriers

**The byte.** Each process has one byte in its state memory, at a place the translator's contract fixes
(suggested name `runs_in_parallel`). It is 1 when the process's group is in force `Parallel` or
`ParallelOptimistic` and has more than one thread, else 0. The kernel tells the executor when it changes
(suggested `Cpu::set_runs_in_parallel(memory, value)`). It is set to 1 inside the entry that makes the
group's second thread, or that loosens its ordering, when every thread of the group is stopped or about
to start. It is set back to 0 at a later entry. Translated code reads it with a plain load, as it reads
its stop flag (tr/code.rs:1300).

- **ARM64's atomic forms listed in 7.1: nothing changes.** They are atomic operations already.
- **Barriers.** ARM64's `dmb` and `dsb` become: load the byte, and when it is not zero,
  WebAssembly's `atomic.fence`. `isb` stays nothing: it orders instruction fetch, and code changes have
  their own hook. All three kinds of `dmb` become the one full fence.
  - Why it is needed: a program that orders plain accesses with a barrier gets no order at all from
    nothing. The owner's own results name such programs: crossbeam's queue in rayon does "`back.store(b,
    Relaxed); fence(SeqCst)` ... On ARM64 the fence is a DMB", and SQLite checks its shared index "with
    memory barriers" (optimistic-replay-results.json). Measured on this Mac with plain accesses on a
    shared WebAssembly memory (section 14): the store-then-load test saw the forbidden outcome in
    371,914 of 2,000,000 rounds with no barrier and in 0 of 2,000,000 with `atomic.fence`; the checker's
    message-passing test saw stale data 148 times in 3,500,000 rounds without and 0 times with.
  - What the WebAssembly compilers do with it: wasmtime compiles `atomic.fence` to `dmb ish` on ARM64 and `mfence` on
    x86-64 (cranelift-codegen 0.137.1, src/isa/aarch64/inst/emit.rs:1809-1811 and
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
  from the reference (decision 8). Rejected: an exact monitor, by a group stop at every exclusive pair or
  by watching every store: both cost every lock operation of every program.
- **128 bit pairs.** Compare and swap of a pair of 64 bit registers, and the exclusive load and store of
  such a pair, are plain loads and stores today, "with one thread this is exact" (arm64/memory.rs:239,
  :310). WebAssembly has no 128 bit atomic operation. They become: load the byte; when it is zero,
  today's plain loads and stores, inline; otherwise one new guest hook, which makes the access under a
  group stop. Exact, and slow: counted "128 bit atomic made with the group stopped". How often real
  programs run them is measured in T3.
- **x86.** Lock prefixes are emitted as their plain forms today, because one thread runs (x86 plan line
  1839). Asked of the x86 track:
  - a locked instruction and `xchg` with memory become WebAssembly atomic operations when the address is
    aligned for its size, and the group-stop hook when it is not (x86 allows a locked access at any
    address; WebAssembly's atomic operations trap on a misaligned one). `cmpxchg16b` is a 128 bit pair
    and is treated as one, behind the byte. On an x86 host an atomic operation is the lock prefix the
    program pays natively. The x86 track measures it on `Serial` runs before it lands; if it is slower
    there, the same byte chooses the plain form;
  - **`mfence` becomes the same tested fence as `dmb`.** Lock prefixes alone do not give an x86 program
    its store-then-load order: older compilers write a sequentially consistent store as "mov + mfence,
    no visible atomic" (optimistic-replay-results.json, the detector's validation). Measured on the x86
    laptop (section 14): with plain accesses both loads saw 0 in 134,938 of 2,000,000 rounds, with
    `atomic.fence` in none;
  - `sfence` and `lfence` stay nothing. The first orders only stores that bypass the cache, which the
    translation makes as ordinary stores. The second orders no memory access.

### 7.4 The page table read by several threads while one changes it

The table is one byte per page, read by translated code with a plain load (tr/code.rs:674). What a piece
learned from it holds until its thread's next stop (tr/contract.rs:17-27). So:
- raising a permission needs no stop (4.5);
- lowering one is a group stop, then the write, then carry on. The executor's own write locks
  (wt/lib.rs:486-489) follow the same two rules: placing a lock lowers, lifting one raises;
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
  writing the page, as a `read` of a file being written through a mapping is on Linux. When a second
  place comes to show a page, the group is called in, its written pages are copied and locked, and it
  runs `Serial` from then on. Counted "shared pages locked again: a second place shows them". `Serial`
  groups keep today's copies at every exit unchanged.

### 7.5 Code a program writes while another thread runs it

- **Dropping code** is a group stop, and every other thread leaves `Execution::run` before it carries on (4.5). ARM64
  announces a change with `ic ivau`, which is already a hook (arm64/memory.rs:375-383). x86 does not
  announce; its code pages carry a write lock, and the x86 plan already says the other threads are stopped
  first (x86 plan lines 1884-1894, 1941). This plan gives that the group stop.
- **Adding code needs no stop.** Today new code is instantiated in every thread of the process at once
  (docs/runtime.md section 9). With one store per thread, a thread cannot touch another's store. Instead:
  the new module goes on the process's list, its slots are written to the array of slots last, and each
  other thread instantiates it in its own store the first time it needs it. No translated code changes for
  this: a lookup that finds an empty table entry already goes to the load code hook (tr/thread.rs:118-130,
  tr/code.rs:1159), which then finds the module on the list.
- **Growing the tables.** A lookup beyond a thread's table would trap, so every thread's table must cover
  every slot written. Tables grow by doubling, each thread growing its own at a group stop, so a stop for
  this happens only when the process's slot count crosses a doubling. Counted "group stopped to grow the
  lookup tables".

A runtime that compiles code while its other threads run (Java, Node) pays a group stop at each change of
code already run. How many, and what they cost, is T3's measurement.

### 7.6 What the WebAssembly compilers must do, and a browser

Three things are relied on that WebAssembly's rules do not promise. All are tested in the suite on every
version of each WebAssembly compiler (section 12).
- **A plain load in a loop is done again each time round.** Guest programs wait on words another thread
  writes, with plain loads (optimistic-replay-results.json: "real programs peek at lock words with plain
  reads constantly"). Measured (section 14): wasmtime 50.0.1, and Node 24.14.1's V8 with its optimising
  compiler alone, its baseline compiler alone, and both, all saw another thread's store from a loop of
  plain loads on a shared memory.
- **Plain accesses are compiled to the host's plain accesses, in program order.** Then a guest gets the
  host's own ordering, which is its own when host and guest are the same architecture, and stronger for an
  ARM64 guest on an x86 host. Section 9 is the case where it is weaker.
- **`atomic.fence` is compiled to the host's full barrier**, and an atomic operation orders the plain
  accesses after it at least as the guest's own instruction would (7.3). Measured (section 14): on both
  hosts, in Node, the store-then-load test never saw the forbidden outcome with `atomic.fence`, with an
  atomic store, or with an atomic exchange in place of the barrier.

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
`memory_watch_thread`, where the executor implements `PerThreadWatch`. On the WebAssembly executor:

- **A thread's own page table.** Translated code takes the table's place as an imported value, per
  instance (`PERMISSIONS`, tr/contract.rs:62-64). So a thread's instances can be given a table of their
  own. No translated code changes.
- **Only once it is watched.** A thread uses its process's table until its first `memory_watch_thread`.
  Then it gets its own, and its instances are made again with the new place, once per thread (measured for
  one thread: 0.43 to 0.60 ms for busybox's code and 0.27 to 0.42 ms for its C library's, docs/runtime.md
  section 8). Rejected: a table per thread
  from the start. Every `mmap` would write as many tables as the process has threads, for processes that
  are never watched.
- **A watch is one more reason in the executor's lock layer** (docs/runtime.md section 8): a thread's
  table byte is the kernel's permissions, less what the process's locks take, less what that thread's
  watches take. One function makes it, as today (`effective`, wt/lib.rs:790).
- **Setting a watch stops one thread, not the group.** Only the watched thread's table changes, and the
  rule is that the thread whose table loses a permission must be stopped.
- **A watched access** comes to the memory fault hook, the thread leaves `Execution::run`, the executor finds the
  watch and calls `on_memory_accessing`. If the watch is gone when it returns, the thread carries on.
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
    write is made when the hook returns (decision 10). Without it an embedder's record of hand-overs
    misses these writes.
- **Space.** Each table is one byte per page under the ceiling: 1020 KiB for ARM64's 4080 MiB, of which
  untouched pages cost nothing. They share the state memory's 4 GiB with the array of slots (docs/runtime.md
  section 8), so the ceiling's formula gains the number of watched threads.

**What it costs** (expected, from parts measured apart): a hand-over of a page from one thread to another
is one watched access (leaving `Execution::run` and coming back, 300 ns, docs/runtime.md section 10), one stop of the
thread that held the page (230 ns on average, 30 µs at worst, measured in Chrome), two table bytes, and the
embedder's own record. So about a microsecond before the embedder's work. The 2008 system the owner saw
paid over 40,000 cycles a hand-over on one machine (dsm-survey/results.json, SMP-ReVirt), most of it
for changing page permissions in hardware and taking the interrupts that follow, which a byte in a table
does not have. The count of
hand-overs is the program's and is not reduced: programs that write the same pages from several threads
will hand over constantly. T4 measures it on real programs.

The stand-in does not implement `PerThreadWatch`. The native executor does it with a table root per thread
(section 10).

## 9. An x86-64 guest on a host with weaker memory ordering

**The promise.** x86 promises total store order: a thread's stores reach memory in the order it made them,
and its loads are not reordered with each other. Only a store followed by a load of another address may
appear swapped, and `mfence` or a locked instruction between them forbids that too. ARM64 promises less:
two loads, or two stores, to different addresses may take effect in either order (Arm Architecture
Reference Manual, the memory model). WebAssembly's own rules promise nothing at all for plain accesses
that race.

**Where it matters.** Only for an x86-64 guest, in a group in force `Parallel`, on the WebAssembly
executor, on a host that is not x86: wasmtime on ARM64, and any browser on ARM. On an x86 host wasmtime
compiles plain WebAssembly accesses to plain moves, which keep the order (ordering-cost/result.json); that
is what 7.6 relies on, tested with ordering tests (section 12). The native executor runs only
guests of the host's own architecture (native.md section 11), so the hardware gives the right order.

That it matters on this Mac is now measured, not only read in the manual: the checker's message-passing
test, with plain accesses on a shared WebAssembly memory in Node, saw the flag with stale data 148 times
in 3,500,000 rounds (section 14). x86 forbids that outcome.

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
barrier, so this form must never be used there. It is therefore a second translation, used only for x86
guests in parallel on weaker hosts, and a field of the cache key.

**What is done until it is built.** On such a host the executor reports that it can run an x86 group only
`Serial` (section 3). The group runs `Serial`, counted "x86 group kept Serial: the host orders memory more
weakly", warned once per container. On an x86 host, lock prefixes as atomic operations and `mfence` as the
tested fence (7.3) are together all that is needed, and the ordering tests check exactly that.

Whether the ordered form is built at all is the owner's (decision 6). Recommended: not now.

## 10. The native executor

docs/native.md section 5.3 already describes threads as agreed in discussion. This plan fixes what it
builds, in the native track's own milestones after its single-thread suites pass:

- **One virtual CPU per running guest thread.** The pool that today gives a virtual CPU to each container
  with something to run (native.md section 5.1) gives one to each processor that has a thread. The
  platform bounds it for the whole host process: 64 virtual CPUs on this Mac
  (`hv_vm_get_max_vcpu_count`, native.md section 5.1), one virtual machine per process. A processor that
  finds none waits, counted "waited for a virtual CPU".
- **The kernel's lock is inside.** A virtual CPU that finds it held waits in the inside, not on the host
  (native.md section 5.3). This is the `lock_api::RawMutex` of 4.2 for that build.
- **Stopping one thread** is an interrupt to its virtual CPU. From the host, `hv_gic_set_spi` took 2.4 to
  4.1 µs to reach the inside's handler (native.md section 5.5). From one virtual CPU to another inside,
  through the interrupt controller, is not measured; the native track measures it with this work.
- **Group stop** is that interrupt to each virtual CPU running a thread of the group. Each lands at an
  instruction boundary and waits in the inside. A change to a page table is then followed by the
  architecture's broadcast invalidation, as Linux does.
- **A table root per thread, only once it is watched.** A thread without per-thread watches uses its
  process's root. One with watches has its own root and address-space identifier, sharing every lower
  table that is the same for all (native.md section 5.3; its N1 builds and tests the per-thread root). This
  is `PerThreadWatch` there.
- **Atomic instructions need nothing.** The native output copies the guest's single-instruction atomics,
  compare and swap, load-acquire and store-release, with an alignment test in front (native.md lines
  363-366). The hardware does them, the 128 bit pair included. No hook and no byte.
- **Barriers must be copied.** The native output emits no instruction at all for what the decoder calls
  nothing, `dmb` and `dsb` among them, and leaves their route "decided with the WebAssembly output's"
  (native.md lines 350-362). Decided here: `dmb` and `dsb` are copied as they are, always. The guest then
  pays what the reference pays for the barrier it wrote, of the kind it wrote, and there is no byte to
  test. The other forms decoded as nothing stay nothing. The x86-64 native output copies `mfence` and
  locked instructions as they are; the plans do not say so yet. Asked of the native track, with the cost
  on a `Serial` run measured there.
- **Exclusives are a compare and exchange there too.** The native output keeps the WebAssembly output's
  record of the load and makes the store with `cas` or `casp` against the recorded value (native.md lines
  484-500). So the difference of 7.3 holds on both executors (decision 8). One thing in it assumes one
  thread: when an interrupt lands between the load and the writing of the record, the record's value is
  "read again from guest memory ... which nothing has changed since, as one virtual CPU runs the
  container's threads" (lines 492-496). With threads in parallel another thread can have changed it, and
  the store would then succeed against a value the guest never loaded. Asked of the native track: in a
  group in force `Parallel`, take the value from the registers the load filled, or run the load again.
- **Shared pages between processes** are the same frames, so a group that shows a page in several places
  runs in parallel there.
- **Replay** is the same recording point (4.6), asked through the link.

Asked of the native track: the copied barriers and the exclusive's record above, with its own thread
milestone. Nothing before that. What it must not close off is already written in its section 5.3.

## 11. Scheduling

- **How many processors the container is told.** A value of the container, the same on every machine,
  because the guest sees it and a replay or a move must see the same: suggested
  `GuestMachine::processors` (rt/container.rs:39-44), given to the kernel in `Start`. Suggested default: 1,
  so every existing test and every existing user sees what they see today; the command line program passes
  the host's count unless told otherwise (decision 3).
- **How many run at once.** The smaller of the processors and the host threads the embedder has in the
  loop (section 5). Never more than one thread of a group in force `Serial`.
- **Which host thread runs which guest thread.** Any. A guest thread is its register block and its memory;
  on the WebAssembly executor its store moves between host threads (a wasmtime store may be sent, not
  shared). In a live run a worker keeps its processor, and the kernel puts a thread on whichever
  processor asks. Keeping a thread on the processor it last ran on is added only if measured to matter.
- **Fairness.** Linux's fair scheduler as built (kernel/schedule.rs), with one run queue for all
  processors: each takes the thread with the least weighted time that may run (4.3). One queue is fair by
  construction and costs nothing extra under one lock. A woken thread that should run before a running
  one (kernel/task.rs:561) ends the turn of the running thread with the most weighted time, with
  `thread_stop`.
- **Told many, running one.** A threaded program in a `Serial` group is told the processors but runs one
  thread at a time. A program that spins on a lock because "there are other CPUs" wastes its turns. It is
  correct, and it is what `Serial` means; the cost shows in T2's measurements.
- **Charging CPU time.** Each thread is charged from its own "handed out at" time to its return, on the
  system's monotonic clock (today one time for the whole kernel, kernel/task.rs:77-86). As today, in a
  free turn this includes the executor's own work and the host's preemption (docs/runtime.md section 9).
  A process's CPU time is the sum of its threads' and may now run faster than the clock, as on Linux. CPU
  timers and `RLIMIT_CPU` end a turn by the thread's own slice, as today (kernel/task.rs:59).
- **The tests' clock.** The test system's repeatable clock moves by the progress each thread reports
  (docs/runtime.md section 9). With several processors it moves by all of them, in the order of entries, so
  it still repeats.

## 12. Tests and the bar

- **Nothing changes with one processor.** With every group `Serial` and one processor, every test of the
  kernel test crate gives the same verdict as before, with and without `--features runtime`, at full, none
  and bounded look-ahead, and `repeats` still repeats. Checked as J1 checked it: the `test ...` lines from
  both commits kept and their `diff` empty.
- **Groups in parallel.** The same crate's tests that run several processes pass with several processors
  and free turns, against the reference.
- **Exact replay with every group `Serial`.** A test system records a run with several processors (the
  order of entries, the host's answers, each stop's position), then replays it with the replayer of 4.6.
  Every thread's registers at every entry, a hash of every memory at every entry, and every answer of
  `getcpu` are the same. Run over seeds, with one worker, with fewer and with more than the recording
  had.
- **Reaching across groups.** Guest programs for each case of 4.5 where one group reaches another: a
  writer through a shared mapping beside a reader through a private mapping of the same file; a `write`
  to a file another process maps; a file made shorter while another process maps and runs it; a signal
  that ends a process whose thread is running. Each matches the reference, and each replays exactly.
- **A suite of threaded programs** (new, in the kernel test crate, built for the guest as its other
  programs are), each with output that does not depend on the schedule: locks; condition variables;
  atomics in each instruction form (the single-instruction ones, exclusives forced by the build flags,
  128 bit pairs); thread-local storage; signals sent to threads, masks and `sigwait`; fork from a threaded
  process; exec and `exit_group` with threads running; `membarrier`; robust futexes; code written by one
  thread and run by another while it is rewritten; a wait on a word with plain loads; a shared file
  mapping written by several threads and read back through the file. Each matches the reference in
  `Parallel`, on every executor.
- **Under a stress schedule.** The test system raises stop flags of random running threads at random
  times, varies the number of processors, and delays entries at random, all from a seed. Each program runs
  over many seeds.
- **Ordering tests.** The standard small two-thread tests of memory ordering (store buffering, message
  passing, load buffering, independent reads of independent writes), as guest programs, many rounds each,
  written three ways: with plain accesses only; with the guest's barrier instruction where the pattern
  needs one; and with each atomic form of the guest standing in for the barrier. No outcome the guest's
  architecture forbids is ever seen. This is the test of what 7.6 relies on, run on each
  version of each WebAssembly compiler and each host.
- **The runtimes.** The kernel test crate's Go, Java, Node and Python programs
  (crates/wcontainers-kernel-test/tests/go.rs, java.rs, node.rs, python.rs) with threads, in `Parallel`
  under the stress schedule.
- **`Serial` must not get slower, beyond what decision 7 allows.** Three measurements, each against its
  figure at the commit before the milestone, with machine and load:
  - the translator's benchmark set, instructions retired: within the spread between identical runs
    (under 0.5% in section 14);
  - a loop of barriers and a loop of 128 bit atomics, added to that set: no more than the byte's test
    above the figure before (0.1 to 0.8 ns a round by section 14, measured again there);
  - the system call cost with one thread (the `getppid` loop and the fork loop of docs/runtime.md section
    15, and the kernel's own share as section 18 measured it): no more than one uncontended lock above
    the figure before, on this Mac and on the x86 laptop.
- **Speed-up** is measured, with machine and load, on a real build with parallel jobs and on a threaded
  program in each runtime, at 1, 2, 4, 8 and 16 processors, with the time spent waiting for the kernel's
  lock, the group stops by reason and the groups called in by reason.

## 13. Milestones

Each bar is checkable by the named tests. The cheapest large win is first.

- **T1, groups in parallel, every group `Serial`.** No translated code changes. No ordering but `Serial`
  exists yet.
  - abi: `Cpu` and `Host` by `&self` and `Sync`; the processor in `Kernel::next`; the thread in
    `Kernel::exit`; the `Host` services `thread_stop` and `kernel_entry_admit` and the hook
    `on_processor_needed`.
  - kernel: the lock, the handle that holds it and the `Send` reason (4.2); processors and the scheduler
    (4.3); groups with the joining rule (3); the two rules of 4.4 item 5, with calling a group in and
    threads ending in their own entry (4.5); entries kept aside when told `Later` (4.6); the idle mark
    (5); charging per thread (11); the told count.
  - runtime: `run` and `run_until_finished` with a processor, the system told who, idle without a lost
    wake, the
    container stop, the executor's stopping handle in place of `StopFlag`, the local system by `&self`
    with a deadline per processor (5).
  - WebAssembly executor: shareable, with the process's lock (7.2, last item). Stores stay one per process:
    one thread of a group runs at a time.
  - kernel test crate: the recording and replaying test system with its replayer (4.6).
  - the stand-in stays at one processor.
  - Bar: section 12's "nothing changes with one processor", "groups in parallel", "exact replay" and
    "reaching across groups"; its `Serial` measurements, of which only the system call cost can move
    here; and a scaling test: four processes that only compute, on four processors, finish in under half
    the time they take on one (a quarter is the ideal; the margin is for this machine's load and is the
    orchestrator's to set).
  - Measured: a real build with parallel jobs at 1 to 16 processors; the share of time waiting for the
    kernel's lock; groups called in, by reason; the lock, the admission call and the system's own locks
    each apart, in the system call cost.
- **T2, a group's threads in parallel on the WebAssembly executor, ARM64 guests.** The shared import and
  its declared maximum; the host-supplied shared memory, first; one store per thread; group stop; the
  table rules and the shared pages of 7.4; code dropped and added; table growth; the byte, barriers and
  the 128 bit hook (7.3). In the kernel: `ExecutionConfig` at the start, the ordering in force and its
  changes, the executor's limits, telling the executor the byte, `membarrier`, the read of every call
  that runs again (4.4).
  - Bar: the threaded suite and the ordering tests match the reference in `Parallel` under the stress
    schedule at full, none and bounded look-ahead; T1's bars hold; all three `Serial` measurements of
    section 12 hold.
  - Measured: group stops by reason and their length; a store's memory.
- **T3, the runtimes and real threaded programs.** Go, Java, Node and Python with threads, and code
  compiled while other threads run.
  - Bar: section 12's runtimes pass in `Parallel` under the stress schedule.
  - Measured: speed-up of a threaded program in each; group stops for code changes; how often barriers
    (by kind) and the 128 bit forms run, counted by a build of the translator made for this measurement
    only.
- **T4, per-thread watches on the WebAssembly executor** (section 8). After the embedder interface's
  memory watches exist (another track).
  - Bar: a test embedder holds each page for one writer or several readers, records every hand-over with
    both threads' positions, and replays a threaded program from that record with the same registers and
    memory at every hand-over. The program's threads also fill each other's pages through `read` system
    calls, so the kernel's own accesses are in the record.
  - Measured: the cost of one hand-over; hand-overs per second in the threaded suite and the runtimes.
- **T5, the native executor** (section 10), in the native track, after its single-thread suites. With it:
  barriers copied, and the exclusive's record made safe for threads in parallel.
  - Bar: T1's and T2's bars on the native executor, the ordering tests with barriers among them; the
    per-thread root under T4's test.
- **T6, x86-64 guests**, in the x86 track: lock prefixes as atomic operations and `mfence` as the tested
  fence (7.3); the executor's answer on a weaker host (section 9).
  - Bar: the threaded suite and the ordering tests for an x86 guest in `Parallel` on the x86 laptop; on
    this Mac the same suite passes with the group kept `Serial`, its counter read by the test; the
    `Serial` measurements of section 12 for an x86 guest.
- **T7, splitting the kernel's lock**, only where T1's measurement points (4.7), one part at a time.
  - Bar per part: the single-thread system call cost is not above its figure before; the time waiting for
    locks on T1's build is lower; every earlier bar holds.

Not milestones here: `ParallelOptimistic` (decision 5), the ordered x86 form (decision 6), a browser, and
changing a running process's ordering, which comes with `suspend` and `resume` (section 6).

## 14. Measurements behind the decisions

On this Mac (Apple M5 Max, 18 cores, macOS 27.2), release builds, 2026-11-18, with other tracks running:
the load (`sysctl vm.loadavg`, 1 minute) was 49 to 107 throughout. On the x86 laptop (Intel Core i7-8750H,
12 threads, Linux 7.3.4) the load was 0.2 to 1.7, and its cores 2 and 8 were not used. Scratch code and
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
- **A loop of plain loads sees another thread's store** (logs/spin-wasmtime.log, logs/spin-node.log). One
  thread spins on a plain 64 bit load from a shared memory, another stores to the word after 200 to 300
  ms. wasmtime 50.0.1: seen, in a bare loop, in a loop with the translator's stop check, and with an
  atomic load. Node 24.14.1: seen in all three, with the default compilers, with `--no-liftoff` and with
  `--liftoff-only`.
- **V8's limit on a declared maximum** (same run): Node 24.14.1 refused to compile a module importing a
  shared 64 bit memory with a declared maximum of 2^48 pages; with 262,144 pages it compiled.
- From earlier work, not measured again: atomic accesses against plain ones, and shared against unshared
  memories, in wasmtime 49, Node and Chrome (ordering-cost/result.json); the stop flag's latency in Chrome
  (docs/phase-1-translator.md); the cost of recording page hand-overs in a 2008 system
  (dsm-survey/results.json); what ordering only atomics and kernel entries would miss
  (optimistic-replay-results.json); system call, fork and instantiation costs (docs/runtime.md section 18);
  the native executor's interrupts (native.md section 5.5).

## 15. Not yet measured

- How long processors wait for the kernel's one lock in a real build (T1). This decides whether and where
  the lock is split.
- What T1 adds to a system call end to end. Only the lock alone was measured (section 14).
- How often real programs run barriers and 128 bit atomics (T3). The cost of each is measured; their
  number is not.
- A host-supplied mapping as a wasmtime shared memory: read in the source, not run (T2).
- What one store per thread costs in memory (T2).
- How long a group stop takes with several threads running, and how often real programs cause one (T2,
  T3). How often a group is called in from outside (T1).
- The ordering tests as guest programs through the translator: none was run. What was run are the two
  hand-written tests of section 14, in Node only. No two-thread test was run in wasmtime.
- A hand-over through per-thread watches, end to end (T4).
- An interrupt from one virtual CPU to another inside the native executor's machine (T5).
- An x86 guest's locked instructions as atomic operations on `Serial` runs (the x86 track).
- `suspend` and `resume` on one kernel: nothing exists to measure (section 6).

## 16. Decisions for the owner

1. **How does a running process's ordering change: only by `suspend` and `resume`, or also by a
   setter?** Recommended: only by `suspend` and `resume`, with the ordering in the `ExecutionConfig`
   given at the container's start and at `resume` (section 6). Cost of this answer: until `suspend` and
   `resume` are built, a process keeps the ordering its container started with; afterwards each change
   moves the process's kernel records out and back. Cost of the setter: one method and its tests now; a
   change costs only the stop; the kernel has two ways a process gets an ordering, and an embedder that
   wants a starting point for a replica must suspend anyway.
2. **When processes that share memory were given different orderings, which is in force?** You ruled the
   setting per group; settings arrive per process, and groups merge at any `mmap`. Recommended: the
   strictest of them, for as long as they share. `Process::thread_ordering()` answers the ordering in
   force, and `Process::execution_config()` what the process was given. Cost: a process told `Parallel`
   can run `Serial` because of a neighbour, and says so only through that getter and a counter. The
   other answers: the last one given wins for the whole group, which weakens a neighbour's promise of
   exact replay without its being asked; or `resume` is refused when it disagrees with the group, which
   still leaves the `mmap` case, where the kernel cannot refuse what Linux allows.
3. **How many processors is a container told, and who says?** Recommended: `GuestMachine::processors`,
   default 1, fixed for the container's life; the command line program passes the host's count unless told
   otherwise. Cost: a library user who sets nothing gets no speed-up. The other answer, defaulting to the
   host's count, speeds everyone up at once and makes a container's behaviour depend on the machine that
   first made it.
4. **When an executor cannot run the ordering asked for, does the group run under the strictest one it
   can, or is the request refused?** Two cases exist: on the WebAssembly executor, a group in which a
   writable shared page is shown in two places (7.4), and an x86 guest in parallel on a weaker host
   (section 9).
   Recommended: run it `Serial`, counted and warned, and the getter of decision 2 says so. Nothing is
   faked: `Serial` keeps every promise `Parallel` makes. Cost: such programs get no speed-up there.
   Refusing costs more: the first case appears at an `mmap` in the middle of a run, where the only
   refusal is to fail the guest's call, which Linux would not.
5. **Is `ParallelOptimistic` built?** Recommended: no. The value stays in the enum, and by decision 4 a
   group asked for it runs `Serial`. The measurements say it would not replay any of the 11 programs
   measured on ARM64, because C libraries and language runtimes read lock words with plain loads, and its
   single sequence costs 24 to 97 ns an event, which by calculation adds 0.7 to 4 times the run time for
   Go and Java (optimistic-replay-results.json). Building it later costs a second translation with a
   ticket at every atomic instruction, and a field of the cache key; nothing here closes that off.
6. **Is the ordered form for x86 guests on weaker hosts built?** Recommended: not now. Until then such
   groups run `Serial` on ARM64 hosts and in parallel on x86 hosts. Building it costs a second translation
   of every x86 program, up to 2.7 times slower single-threaded (section 9), to gain cores for threaded x86
   programs on ARM hosts.
7. **Two costs to a `Serial` run that this plan cannot bring to zero: accept them?** Your rule is that
   `Serial` must not get slower.
   - A byte is tested at each barrier and each 128 bit atomic: 0.1 to 0.8 ns each, measured (7.2, 7.3).
     Removing it costs a second translation of every program that is ever run both ways.
   - A system call takes one uncontended lock: nothing measurable on this Mac, 7 to 9 ns on the x86
     laptop, against a kernel share of about 120 ns (4.2). Removing it costs the kernel compiled twice
     into every program, with and without the lock.
   Recommended: accept both, with the bars of section 12 holding each to that size.
8. **Two places where translated code differs from hardware in a parallel group: accept them?**
   An exclusive store succeeds if the value is the same again, where hardware would fail it, on both
   executors. On the WebAssembly executor a 128 bit atomic is made with the group's other threads
   stopped, which is exact and slow (7.3). Recommended: accept both, on the written list of differences.
   The alternative for the first costs every lock operation of every program.
9. **The interface changes: accept them now, all in T1, so that splitting the kernel's lock later
   changes none?** Recommended: yes. Cost: every `System` (the shipped one, the tests', your own) keeps
   its changing state behind its own locks and gains the arguments below.
   - `Kernel`: entry points take `&self`; `next` takes a processor; `exit` takes the thread and says
     whether it took effect; the handle that holds the lock across a thread's `exit` and `next`, which
     `arrived` is handed in the kernel's place.
   - `Cpu` and `Host`: methods take `&self`; both are `Sync`.
   - `Host` gains two services, `thread_stop(thread)` and `kernel_entry_admit(entry)`, and one hook,
     `on_processor_needed(processor)`.
   - `System`: methods take `&self`; `turn` is told the processor and the thread, `ran` the thread,
     `arrived` and `idle` the processor; `idle` also returns when its processor is needed; `stopped`
     runs once, with every worker out of `Execution::run`.
   - `Execution::run` takes `&self`. `StopFlag` and `Executor::stop_flag` go; the executor gives a handle
     that stops a named thread. `run` and `run_until_finished` take a processor.
   - In T2, `Cpu` gains `set_runs_in_parallel(memory, value)`, `parallel_limits()` and a call for
     `membarrier`'s group stop, and `Start` carries the processors and the `ExecutionConfig`.
   This also answers the open question of how a running container is interrupted from another thread.
10. **A kernel write into a watched process from outside it: how is the embedder told?** A `write` to a
    file the process maps, or `process_vm_writev`, has no thread of that process behind it, and
    `on_memory_accessing` names a thread. Recommended: the hook's thread becomes optional, none meaning
    "the kernel, for another process", and the write is made when the hook returns (section 8). Cost: one
    changed signature. Without it, an embedder that composes exact replay from watches cannot see these
    writes.
11. **Names.** Suggested, all yours: "group" for threads that share memory; "called in" and "group stop";
    `processors` and `ProcessorId`; the services `thread_stop` and `kernel_entry_admit`, with
    `KernelEntry` and `EntryAdmission { Now, Later }`; the hook `on_processor_needed`;
    `Kernel::enter` and `KernelEntered`; `Executor::thread_stopper`; `runs_in_parallel` for the byte and
    `Cpu::set_runs_in_parallel`; `Cpu::parallel_limits`; `RuntimeConfig::host_threads`; the counters'
    names in sections 4, 7 and 9; the milestones' letter T.
