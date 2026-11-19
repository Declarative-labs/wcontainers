# wcontainers: guest threads in parallel

Status: plan, 2026-11-18, corrected after its third independent check (docs/status-threads.md says what
changed each time). Nothing here is built. The owner rules on section 16 first.

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
- **config**: a process's `ExecutionConfig` (section 2): its thread ordering, its replay guarantee and its
  translator config. The **config in force** is what its group really runs under (section 3).
- **guarantee**: a process's `replay_determinism`: `Exact`, `External` or `None`.
- **stop point**: a place in translated code after which a piece no longer relies on what it found out
  about page permissions before it. The translator's contract names three: a stop check whose hook runs,
  a system call, and leaving `Execution::run` (tr/contract.rs:17-20). 4.5 adds the hooks that are made
  stop points. Every other hook returns into the middle of a piece, which carries on with what it knew.
- **stop word**: the 32 bit word after a thread's registers that translated code tests at each stop check
  (abi/registers.rs:54-57). Zero means carry on. Section 4.5 makes it a set of reasons.
- **called in**: the kernel asks for a running thread with `thread_stop`, and the thread comes back by
  itself through `Kernel::exit`. A call that needs it back first parks, as any call that waits does.
- **group stop**: the executor makes every other thread of one guest memory held, in place, while one
  change is made. It is kept per memory (4.5); the threads it reaches are always of one group.
- **stop lock**: one lock per guest memory, in the executor. Whoever needs that memory's other threads
  held takes it first (4.5).
- **held**: a thread is held when it is at a stop point and has no access of its own to its memory under
  way: it waits there for the stop lock or for the kernel's lock, or does something that touches nothing
  of the memory, or it has left `Execution::run`. A thread inside a hook that is not a stop point is not
  held (4.5).
- **compiler fence**: an instruction across which a WebAssembly compiler does not merge, move or drop
  memory accesses. In wasmtime these are every atomic operation, `atomic.fence` and every call (7.6).
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
   A process that asks for exact replay still gets it, and one that does not pays nothing for it.
2. **The threads of one group run at the same time**, where its ordering says so, from a second form of
   code. The first form, for `Serial`, stays as it is.
3. The same on the native executor, and for x86-64 guests.

It also settles what each replay guarantee records and what it costs, what the embedder composes itself,
and what is refused until it is built.

## 2. Rules, and what is already ruled

All rules of phase one hold: one primitive over parallel paths; no setting without a real use today; never
fake; every slow path has a name, a counter and a warning; run speed first; scope is whole architectures.

Ruled by the owner on 2026-11-18, and taken as given. Where the brief's and the interface notes' sections
disagree, the last one is followed (brief-threads.md lines 134-160, interface notes lines 155-181).
- **Two separate choices per process**, in `ExecutionConfig<E> { thread_ordering: ThreadOrdering,
  replay_determinism: ReplayDeterminism, translator: E::TranslatorConfig }`.
  - `enum ThreadOrdering { Serial, Parallel }`: how the threads of one group are run. `Serial`: one at a
    time. `Parallel`: at the same time.
  - `enum ReplayDeterminism { Exact, External, None }`: what a replay of the process promises. `Exact`:
    identical registers and memory at every logged position. `External`: "Consistent in everything the
    process does to the outside; no guarantee about its internal state." `None`: nothing.
  - `ParallelOptimistic` is no longer a value: it is the pair (`Parallel`, `External`). The getter
    `ThreadOrdering::replay_determinism()` is gone. The guarantee is a field.
- **The progress count is paid only for `Exact`.** Translated code holds the count if and only if the
  process's `replay_determinism` is `Exact`. That is derived from the field, never a switch of its own,
  and it is part of the translation cache key. With `External`, kernel entries are put in order and
  logged, with no count; in a `Parallel` group so are locking and atomic instructions. With `None`
  nothing is recorded.
- **(`Parallel`, `Exact`)**: the kernel does its part (the count, and an exact position for everything it
  controls). The order of memory accesses between threads is the embedder's, through per-thread memory
  watches (`memory_watch_thread`, on executors that implement `PerThreadWatch`).
- **There is no setter for the ordering.** A process is given its config when the container starts and
  when it is resumed. Taking a process out of its kernel is `suspend`, putting it back is `resume`, and
  the value between is `SuspendedProcess<E>`. Neither converts anything. `export` and `import` only make
  and read the form that crosses a wire (interface notes lines 110-123, 131-137). The getter is
  `Process::execution_config()`. A call that changes the config inside a hook without `run` returning is
  proposed and not ruled (`Process::suspend_and_resume(self, config)`, interface notes lines 165-166; the
  brief calls it `Process::replace_execution_config`). Section 6 weighs it.
- Hooks receive `Process<'_, E>` by value, and `suspend(self)` consumes it. Suspending a process inside a
  hook makes `run` return at once.
- The call that runs a container is `run`. It returns `StopReason { Interrupted, Suspended, Idle {
  until }, Finished { status } }`. Today's run-to-the-end call becomes `run_until_finished`.
- **Processes that share memory need not be on one kernel.** The owner dropped the "process closure" as a
  hard rule. The kernel reports which processes share memory, and marks shared pages watched when one of
  them leaves. Replay of processes that share memory across kernels is the embedder's concern (interface
  notes lines 158-162).
- Different groups always run in parallel with each other.
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

**What each pair means, and what it pays.** The count costs translated code 11.9% to 16.3% more
instructions in its best measured form (orchestrator/progress-count-cost.json, form 3, the x86 laptop
and this Mac). So the pairs differ in cost, and the plan keeps the cheap ones cheap.
- (`Serial`, `None`): today's run without its count. One thread of the group at a time. Nothing is
  recorded and nothing is asked of the embedder.
- (`Serial`, `External`): the same code. Each kernel entry of the process is put in order and logged,
  with no position. A replay is checked against the log and thrown away when it differs (4.6).
- (`Serial`, `Exact`): code with the count. Each entry is logged with its position. A replay is the same
  at every logged position (4.6).
- (`Parallel`, `None`): the group's threads run at once. Nothing is recorded.
- (`Parallel`, `External`): as (`Serial`, `External`), and each locking or atomic instruction also takes
  its place in one order. That needs a third kind of translated code. Section 16 recommends not
  building it now; until it is, the group runs as (`Serial`, `External`), which keeps the same promise.
- (`Parallel`, `Exact`): code with the count. Every stop the kernel or the executor causes is an entry at
  a position. The order of memory accesses is the embedder's, through per-thread watches (section 8).

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

**A group ends at the kernel's edge.** Processes that share memory may be on different kernels (section
2). A group is the threads this kernel holds. When a process of a group is suspended and leaves, the
kernel marks the pages it shared watched (the embedder interface's work), the embedder hands those pages
over from then on, and the group here is what is left. Nothing in this plan needs the other kernel.

**The config in force.** A config is given to a process (at the container's start and at `resume`). A
child inherits its parent's at fork and keeps it across exec. But threads are run per group, and groups
merge at any `mmap`. So two processes with different configs can come to share memory, and one rule must
say what is in force then. This plan's rule, for each of the two fields by itself:

- **The strictest wins.** The ordering in force of a group is `Serial` if any of its processes was given
  `Serial`, or if the executor can only run this group `Serial` (7.4, section 9, and (`Parallel`,
  `External`) until it is built). The guarantee in force is the strongest any of its processes was
  given: `Exact`, else `External`, else `None`.
- It is worked out again whenever the group's members change, which is always inside a kernel entry.
- `Process::execution_config()` answers what the process was given. What it runs under is read beside it
  (suggested `ProcessInfo::execution_config_in_force()`).

Why the strictest: a process then never gets less than it was promised. `Serial` keeps every promise
`Parallel` makes, and code with the count is correct for every guarantee. The cost falls on the
neighbour: a process told `Parallel` can run `Serial`, and a process told `None` can be made to carry the
count, which means its code is translated again (the count is in the cache key, 7.2). Each is counted
("ordering in force stricter than given", "guarantee in force stricter than given") and warned once per
process. It is the owner's to confirm (decision 1), with the alternatives there. Whatever he rules, the
kernel needs "the config in force of a group can change while it runs", because groups merge.

How a change takes effect:
- **Stricter in either field**: the group's other running threads are called in (4.5). The entry that
  brings the last one in makes the change. Until then the `mmap` that caused it is parked, or the
  threads of the process that was resumed are not handed out. A thread whose guarantee in force became
  `Exact` is handed out next from code with the count. Its count starts at zero there: its positions are
  counted from the change.
- **Looser**: at the group's next entry. Nobody is stopped. A process already runs the form of code for
  the ordering it was given (7.2), so a looser ordering needs no new code. A thread whose guarantee in
  force is no longer `Exact` is handed out next from code without the count, so that no process carries
  the count longer than somebody asks for it.

**What a guarantee covers, between groups.** A guarantee is about one process. `None` means the kernel
neither orders nor logs that process's entries. So a replay cannot repeat what a `None` process did to
anything else: a process that reads a pipe a `None` process wrote gets, in a replay, whatever that
process writes this time. The kernel does not follow which bytes came from whom, and keeps no such
list. An embedder that wants a process repeated gives at least `External` to every process that can
reach it through the kernel. It is stated in the doc comment of `ReplayDeterminism::None`, and it is the
owner's to confirm with decision 1.

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
  start that it orders entries, and only about a thread whose guarantee in force is not `None`. For any
  other, each entry tests one value.
- *The executor's locks* (7.2). A thread of a `Serial` group takes its process's lock once, when it
  enters `Execution::run`, and keeps it until it leaves. Its hooks take nothing.
- *The system's own locks.* `System` methods take `&self`, but what a system call touches in the
  shipped system is already atomic: `turn` stores one deadline word (rt/system.rs:256-266), and `ran` and
  `arrived` do nothing (:268-275). It keeps one such word per processor and adds no lock.
- *The stop word* (4.5): one plain load and a branch before each kernel entry.
- *The marks of a group stop* (4.5) are kept only for a memory in force `Parallel`: one flag tested in
  each hook that is a stop point.

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
- **A worker is not a processor.** A worker says which processor it is asking for:
  `Kernel::next(processor)`. In a live run each worker keeps one processor (section 5). In a replay the
  log says which processor each hand-out is for, and any worker makes it (4.6). So a run may be replayed
  with fewer or more workers than it was recorded with, and the guest sees the same processor numbers.
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
- **A thread runs only on the processors its mask allows.** Linux promises that for `sched_setaffinity`,
  and `getcpu` and `rseq` would show a breach. Today the call accepts any mask that holds CPU 0 and keeps
  nothing (kernel/syscall/system.rs:178-190), which is exact for one CPU. With several:
  - each thread keeps a mask of processors, inherited at clone and at fork, and `sched_getaffinity`
    answers it;
  - `next(processor)` hands out only a thread whose mask holds that processor, and the idle processor
    woken for a ready thread (section 5) is one in that thread's mask;
  - a thread running on a processor that its new mask leaves out is asked in with `thread_stop`, and the
    call that set the mask parks until it has come in. Linux's call also returns only once the thread
    has been moved (`affine_move_task`, kernel/sched/core.c, waits for the move).

Rejected: a host thread is a processor (this plan's first form). Which host thread asks first is the
host's timing, and a replay with fewer host threads could not give the same processor numbers.
Rejected: `next` without a processor, the kernel choosing a free one. A thread that carries on in place
after its system call must get its own processor's answer, or it leaves `Execution::run` at every call
(300 ns, docs/runtime.md section 10).

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
   on a `Serial` group from outside it has called the group in first. In a group in force `Parallel` the
   process's other threads may be running. Section 6 relies on this for `suspend`.

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
  itself takes it off, and only just before a kernel entry of its own, whatever that entry is for.
  Whatever the kernel wanted the thread for (a signal, a group called in, a process ending, a new mask),
  it finds at that entry.
- **held** (suggested `STOP_HELD`): set and taken off by whoever has the memory's stop lock (below). A
  group stop therefore never uses up a request to come in, and a request never lets a held thread go.
- **the thread's own** (suggested `STOP_OWN`): its turn is counted by stop checks, or it is leaving
  `Execution::run` (wt/lib.rs:1726-1728, :1737-1740). Only the thread's own host thread writes it.

Translated code still tests the word against zero (tr/code.rs:1300). One place in it writes the word,
and that changes: `raise_stop` stores 1 over the whole word with a plain store (tr/code.rs:693-699,
tr/full.rs:2746-2750). Only the translator's test guest uses it
(wcontainers-translator-test/tests/test_guest.rs:361), but it would wipe a bit that another host thread
had set. It becomes an atomic or of the thread's own bit. The stop hook looks at each bit. Held: it waits
for the stop lock. Asked in: it takes the bit off and hands the kernel `Exit::Stop`. Counted: it counts.

Why nothing is lost. The bit is taken off before the entry, never after it. The kernel asks under its
lock, so its request is wholly before the thread's entry or wholly after it. Before: that entry meets it.
After: the bit is set, and nobody but the thread takes it off. The slice timer asks outside the lock, and
the kernel looks at the slice at every return whatever brought the thread back (kernel/task.rs:87-90), so
an entry meets that request too. A thread asked in while it is not in guest code at all (between its
hand-out and its first instruction, settling a fault, compiling) finds the bit at its first stop check.
What can happen is a request that crosses an entry and leaves the bit set with nothing left to do. That
costs one stop check that hands in an `Exit::Stop` the kernel has no use for, counted "stop asked for
nothing". In a replay `thread_stop` does nothing (4.6), so the bit is never set.

What it costs a system call: one plain load of the word and a branch before the entry. The atomic and is
made only when the bit is set.

**Call a group in** (the kernel's). The kernel asks `thread_stop` for each running thread of the group and
marks the group wanted, so that `next` hands none of its threads out. The call that wanted it parks. It
is woken by the entry that brings the group's last running thread in, runs again, and the mark is taken
off. Each thread that came in did so through an ordinary entry, at a position when its code has the
count. Nothing can wait in a circle: the mark keeps threads from being handed out, never a woken call
from running. And it cannot wait for ever, because no request to stop is lost.

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
  slots or its code lists. None does today: the clock hook asks the system (rt/lib.rs:120-122), the
  float hook works on the thread's own state, `warn` counts. A group stop that finds a thread there
  waits until the thread reaches its next stop check. A stop flag was noticed in 230 ns on average and
  30 µs at worst in Chrome (docs/phase-1-translator.md, "The stop check").

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

Below, "M's other threads are stopped" means by this rule.

A group in force `Parallel` and `Exact` keeps four uses of holding a thread in place. Three only order
memory accesses, and that order is the embedder's part there (section 2): the 128 bit atomic (7.3),
`membarrier`, and the embedder setting a watch on one thread (section 8). The fourth changes nothing a
guest can see: growing the executor's lookup tables (7.5). Where the embedder needs a held thread's
position, it can read it: the thread is at a stop point, with its registers and its count saved.

A memory in force `Serial` keeps no marks, and accesses to it take no stop lock: within the group only
the caller runs, and from outside the kernel has called the group in first. The kernel tells the
executor each memory's config in force (7.3), and changes it only while none of that memory's other
threads is in guest code. The executor counts "group stop reached a running thread of a Serial group"
and "group stop for a change of the kernel's in an Exact group", which must both stay at zero.

What is stopped before what, where M is the memory changed:
- **A page loses a permission, is unmapped or remapped; code is dropped** (`set_permissions`,
  `shared_unmap`, a mapping going away, `code_changed`, ARM64's `ic ivau`, a write to translated x86
  code). This is the translated code's rule (tr/contract.rs:17-27). M's other threads are stopped.
  - After a code drop each of M's other threads also leaves `Execution::run`, so that none comes back
    into a piece made from the old bytes (docs/runtime.md section 6), counted "left run: code changed
    under it". A translation of those bytes that is under way is thrown away (7.5). Under a call-in they
    have left already.
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
  depend on the host's timing. A file that grows under another process's mapping is such a case.
- **The kernel reads or writes M's bytes from outside M's group** (a `write` to a file reaching pages M
  shows, `process_vm_readv` and `process_vm_writev`, a tracer, `/proc/<pid>/mem`, `environ`, `cmdline`).
  - M's group is `Serial`: it is called in. The read or write then has one place in the order. Counted
    "another group called in for its memory". `cmdline` is not inspection (section 2), but it is still
    such a read.
  - M's group is `Parallel`: nobody. The bytes land while its threads run, as on Linux. The executor's
    copy into M is made with pointer writes, since "no guest runs" (wt/lib.rs:1423) no longer holds, and
    with M's stop lock shared (rule 2). With `Exact` this is a memory access like a thread's, and the
    embedder orders it through its watches (section 8).
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
- **The config in force becomes stricter** (section 3): the group is called in.
- **The container's first process ends**: every other process is ended as by a fatal signal, as Linux
  ends a container. Each worker's `next` then answers `Finished`.

### 4.6 The order of kernel entries, recorded and replayed

What is recorded about a process follows from its guarantee in force and from nothing else.

**One recording point.** Before a kernel entry takes effect, the kernel asks the system whether it may,
with the lock held (suggested: a `Host` service `kernel_entry_admit(entry) -> EntryAdmission`, with
`Now` and `Later`). The entry says what it is: a thread handed out (the processor and the thread chosen),
a thread come back (the thread and why), an event, a signal from outside.
- It is asked about an entry of a thread whose guarantee in force is `Exact` or `External`, and about
  every event and every signal from outside. It is not asked about a thread in force `None`: "with
  `None` nothing is recorded".
- A system that records answers `Now` and writes the entry down. The order of its log is the order of
  effects, because it was asked under the lock.
- A system that neither records nor replays is not asked at all. It says so once, when the kernel
  starts (suggested: a field of `Start`, `kernel_entry_admission`, with `Unasked` and `Asked`), and each
  entry then tests that one value. The shipped system is such a system, and so pays no indirect call for
  this on any system call (4.2).
- A system that replays answers `Now` when the entry is the next one in its log, and `Later` otherwise.

**What an entry carries, by guarantee.**
- `Exact`: the thread's position. The system already hears each thread's progress (`System::ran`), and
  the thread's code has the count, so it has (count, pc).
- `External`: no position, because the code has no count. The entry's place is its place in the order,
  and among its own thread's entries. When the entry is over, the kernel hands the system a digest of
  everything it took from the thread for it: what the exit says (a system call's number and arguments,
  a fault's address and access, nothing for a stop), and each byte it read from the thread's memory
  (suggested: a hook `on_kernel_entry_ended(thread, digest)`, made only for a thread in force
  `External` and only when the system orders entries; BLAKE3, which the cache key already uses,
  tr/contract.rs:163-166). The registers at a stop are not in it, since a replay stops elsewhere. The kernel is a pure function of what it is handed (docs/runtime.md
  section 12), so the same digests in the same order mean the same effects on everything outside the
  process. This is the one recording point of the brief: the kernel keeps no list of which calls are
  "outside".

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
- Every process that reaches it through the kernel is at least `External` (section 3).

So a run is fixed by what is recorded today (the host's answers, each event and each turn's end with its
position, docs/runtime.md section 12) and one new thing: the order in which kernel entries took effect.
A clock read without a system call (`Answer::clock`, rt/lib.rs:120-122) is a host answer of its thread.
It changes nothing in the kernel and needs no place in the order.

**An entry told `Later` never holds a host thread.**
- A thread come back too early: the kernel keeps the thread and its exit aside and returns. The thread
  leaves `Execution::run`, and its worker is free. After each entry that takes effect, the kernel offers
  the kept ones again.
- A thread to be handed out too early: `next` answers `Idle`.
- An event too early: the replaying system delivers its events itself, from its log, in their place.

**How a replay proceeds.** It is not `run_until_finished`. It is the same `run`, called in the log's
order: the replayer takes the log's next "thread handed out" entry and calls `run` for its processor,
from any free worker. With one worker it walks the log. With more, several threads run their turns at
once and only their entries are put in order. It cannot get stuck: the entry that is due is a hand-out,
which a free worker makes; or the return of a thread some worker is running, which will arrive; or an
event, which the system delivers. A return that is kept aside is never the due one, since it is offered
again the moment it becomes due. A worker whose thread came back early is free again at once, so even one
worker always has something to do.

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

**The change: the same `run`, called by as many host threads as the embedder gives, each naming a
processor.** `run` and `run_until_finished` take the kernel, the executor and the system by `&`, and a
processor. One host thread calling `run_until_finished` for processor 0 is today's behaviour exactly. The
embedder's limit on host threads is simply how many of its threads call it.
`Container::run_until_finished` spawns them itself, one for each of the first processors, up to a count
in `RuntimeConfig` (suggested `host_threads`, default the host's `std::thread::available_parallelism`).
Two callers naming one processor at once is a caller's error, which the kernel answers instead of today's
assertion (kernel/lib.rs:306-309).

`run(processor)` keeps handing that processor threads until it has a reason to return, and the reason is
the owner's `StopReason`, read per worker:
- `Idle { until }`: this processor has nothing to run. `run_until_finished` waits in `System::idle` and
  calls `run` again;
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
  the turn can be counted (4.6). The local system needs the processor for its deadline.
- **Idle, without a lost wake.** `next(processor)` that answers `Idle` marks the processor idle, under
  the lock. Any later entry that leaves a thread ready that may run takes the mark off the
  lowest-numbered idle processor that the thread's mask holds (4.3) and announces it (suggested: a hook
  `on_processor_needed(processor)`), once for each such thread. Threads become ready in `exit`, `event`
  and `kill` alike, and in what `arrived` hands in. The system's `idle(processor, until)` returns at the
  time, at an event from outside, or at that announcement. The announcement must be kept if it comes
  before the worker has reached `idle`: `idle` then returns at once. This is the rule of
  `std::thread::park` and `unpark`, and the local system uses exactly those. The mark is made before any
  later entry can run, so no wake is lost. A worker woken for nothing asks `next` again and is told
  `Idle` again.
- **One timer for the container.** Every idle answer carries the kernel's next deadline, as today. The
  local system keeps the earliest and sends one `Event::Timer` for it, from whichever idle worker reaches
  it first.
- **Stopping a thread.** `StopFlag` is replaced by a handle the executor gives out (suggested
  `Executor::thread_stopper()`, in place of `Executor::stop_flag`, rt/container.rs:35-36), with one
  method any host thread may call: `stop(thread)`. It sets the asked-in bit of that thread's stop word
  (4.5). Its promise is part of the interface, for every executor: once `stop` has returned, the thread
  makes a kernel entry of its own after a bounded amount of guest code, whatever it was doing when
  `stop` was called. `StopFlag`'s `raised` latch (rt/lib.rs:38-74) goes, because the bit is the latch.
  The system is handed the handle as it is handed the flag today (rt/lib.rs:164-166). Its `thread_stop`
  service and its timer thread call it. The timer thread keeps one deadline and one thread per
  processor, as `turn` told it.
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
- **Acting on one process from outside a hook.** The interface notes say `kernel.process(id)` is for
  "between steps" (line 55), and promise that "an embedder that wants to work asynchronously returns and
  finishes later through ordinary calls" (lines 60-61). With several workers there is no moment between
  steps unless the whole container is interrupted. That would be the price of every late finish: in the
  owner's other project a thread left waiting by `on_memory_accessing` is let go when its page arrives
  from another machine, and every such page would stop every processor. So the plan adds one call and
  one hook (suggested `Kernel::process_call_in(id)` and `on_process_called_in(process)`).
  - Any host thread may make the call. It is a kernel entry, with its place in the order.
  - The kernel brings the process's group to a stop by the rule of 4.5 (nobody when none of its threads
    runs, a group stop, or a call-in), and then announces the hook, handing it the `Process` by value.
    Every operation on the process is legal there: `memory_write`, lifting a watch, `suspend`, a change
    of config.
  - When none of the group's threads runs, which is the case of a process with one thread that waits
    for a page, the hook fires inside the call and nothing is stopped.
  - Otherwise it costs the group's own running threads their way in and back (300 ns each,
    docs/runtime.md section 10, and the stop check's latency), and no other group anything.
  `kernel.process(id)` itself stays for the moments when no worker is inside `run`: inside
  `System::stopped`, and after a container stop. Called at any other time it answers an error, not a
  handle. This is put to the owner with its alternatives (decision 8).
- `Host` and `System` methods take `&self`, so a system keeps what it changes in atomics or behind its
  own locks. The shipped system needs no lock on a system call (4.2).

## 6. Changing the config of a running process

The brief asked for two forms to be weighed, a setter and no setter. Its last section has since settled
it: there is no setter. The weighing is kept short, because it is what the rest of this section stands
on, and because one piece is still open: a call that changes the config inside a hook.

**Form A, a setter** (not ruled in). `Process::set_thread_ordering(ordering)`, taking effect at the
group's next stop.

**Form B, no setter** (ruled). A process's config is fixed while it is in a kernel. It is given when the
container starts and when a process is resumed, and changed by `suspend` and `resume` on the same
kernel: `process.suspend()` gives a `SuspendedProcess<E>`, and `kernel.resume(suspended, config)` puts
it back with the new config. Nothing is converted and no memory is copied: the process's records and its
memory move out and back by ownership (interface notes lines 110-114). `export` and `import` are not
part of it.

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
  resume after `run` has returned (interface notes lines 134-135, 149-150), so a change made there also
  costs that worker one leave of `Execution::run` (300 ns, docs/runtime.md section 10). Not measured:
  nothing exists to measure.
- **A consistent starting point for a replica.** A change towards `Exact` starts a promise, and a
  replica needs the process's whole state at that moment. B hands it over: the `SuspendedProcess` is
  that state, whole and stopped. With A the embedder must suspend at the same moment anyway.
- **Ways a process gets a config.** A: two. B: one. A reader of an embedder's code sees every change of
  what replay promises as a `resume` with the config written on it.

Weighed so, the plan recommends B, which is what was ruled: the kernel gets no second path, a change of
what replay promises comes with its starting point, and nothing has to be built for it here. What B
costs is that each change moves the process's records out and back, and that a change made inside a hook
waits for `run` to return. The call below takes that second cost away without bringing A's back.

**What "the next stop" is, and how `suspend` stops a process.** `suspend(self)` must hand back the whole
process at once. It is called with the kernel's lock held, in a hook or on a handle from
`kernel.process(id)`. So it cannot call threads in itself: a call-in parks its caller, and the threads
come in through entries that need the lock (4.5). It works with what is already stopped:
- **On a handle from `kernel.process(id)`**: no worker is inside `run` (section 5). Nothing to stop.
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
  positions, which only a call-in gives, and a hook cannot wait for one. `suspend` there answers an
  error that says so, and the embedder asks with `process_call_in` and suspends in its hook.

So `suspend` never stops a thread of a group in force `Serial` or `Exact` in place, and item 5 of 4.4
holds. Two things it does to the rules, and only in the group-stop case: a thread of a suspended process
leaves `Execution::run` without an exit of its own, and is taken, not ended, outside its own entry.

**A call that changes the config inside a hook** (proposed in the interface notes, not ruled). It is
the same operation the kernel already needs when groups merge (section 3): change a group's config in
force at an entry, with its threads stopped as `suspend` stops them. Offered on `Process`, it saves the
embedder the return of `run` and the records' way out and back. In a hook of a process with one thread,
which `on_program_starting` always is, it stops nobody and costs only the next hand-out from the other
form of code. It adds no mechanism to the kernel, and it keeps form B's property: the change is one
visible call with the config written on it, at a moment when the process is whole and stopped. Put to
the owner (decision 9); recommended, under the brief's name `Process::replace_execution_config`.

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
- Choosing per program arrives with `suspend` or with the call above, at `on_program_starting`, which
  the owner already named as the cheap moment to move a process: one thread, a fresh memory, few
  records.

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
  (src/runtime/instance.rs:554). So each process's memory is defined by a one-memory module. Read in the
  source, not run: it is T2's first task, with `Config::shared_memory` on as it is today. If it fails,
  wasmtime's own mapping serves: the kernel's access by pointer and the fork copy work on any mapping.
- **One store per guest thread**, always, in place of one per process. The compiled modules and the
  engine of the process's form stay shared. A dropped thread's store is kept for the process's next
  thread, as its instances are today (wt/lib.rs:478-483). What a store costs in memory is not measured
  (section 15).
- **The executor becomes shareable, with no lock on a `Serial` system call.**
  - Its tables of processes, threads and blocks go behind a lock that is taken only to add or remove
    one. A hook reaches its own thread's and its own process's records through its store's data, as it
    reaches its thread block today (wt/lib.rs:2271-2276).
  - What it keeps per process (page records, code lists, shown blocks, wt/lib.rs:451-474) goes behind
    that process's own lock.
    - In a group in force `Serial` the running thread takes it when it enters `Execution::run` and keeps
      it until it leaves, as it takes the store out today (wt/lib.rs:2245-2251). Its hooks, and the `Cpu`
      calls of its own entries, use what it already has. It asks for the kernel's lock under it at every
      system call. That is the one exception to the lock order (4.5), and it is safe because nobody who
      has the kernel's lock ever waits for this lock. Three rules make that so, and a fourth checks them:
      - the kernel reaches a `Serial` group from outside only after calling it in (4.4 item 5);
      - a thread that finds its asked-in bit set lets its process's lock go before it makes the entry,
        and takes it again only if it is told to carry on. So when the entry that brings a group's last
        thread in wakes the call that wanted the group, no thread of the group has a lock;
      - a `Cpu` call takes a memory's lock only when the memory is the caller's own, or the kernel named
        it after calling its group in, or it is in force `Parallel`. A walk over the memories that show
        a block (`block_write`; the copies `shared_read` asks for, 7.4) skips every other memory in
        force `Serial`: it was not written, or all it wrote is in the block already (7.4);
      - so taking a `Serial` process's lock from anywhere but its own thread is a try-lock. A failure is
        a bug, and it panics, naming both threads. The stress schedule of section 12 runs with it.
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
  group is in force `Exact`), and every other thread of the memory leaves `Execution::run` before it
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
mean more: its processor's own ordering. So three things are relied on that the rules do not promise.
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
  src/egraph/mod.rs:549-575). Between two compiler fences it does three things. Measured for this
  correction (section 14):
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

  It does none of them across a compiler fence: an atomic operation, `atomic.fence`, or a call
  (src/alias_analysis.rs:93-106). That is why a `Serial` run is safe as it is: a thread's turn ends
  only inside a hook, which is a call. It is also why a guest that orders its accesses with a barrier
  or an atomic instruction is safe (the tested fence keeps the second load, check3).

  **The cure: the Parallel form is compiled with that pass off.**
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
  - A browser's compiler has no such setting. Whether V8 does any of the three was not looked at; the
    suite's tests decide it when a browser is built, and the atomic-load form is what is left if it
    does.
- **`atomic.fence` is compiled to the host's full barrier**, and an atomic operation orders the plain
  accesses after it at least as the guest's own instruction would (7.3). Measured (section 14) with the
  store-then-load test on both hosts: in Node it never saw the forbidden outcome with `atomic.fence`,
  with an atomic store, or with an atomic exchange in place of the barrier; in wasmtime 50.0.1, with two
  host threads on one `SharedMemory`, it never saw it with `atomic.fence`, with the byte tested and set,
  with a compare and exchange, or with atomic stores and loads. With the byte tested and zero it saw
  it as often as with no barrier at all, which is what a process with one thread is given.

With these three, a guest gets the host's own ordering for its plain accesses. That is its own when host
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
- **A watch on a page of a live stack frame.** The translator's contract has one exception to "checked
  again at the next stop": what a piece proved about its own stack frame, at or above the stack
  pointer, is checked again only at the next function entry (tr/contract.rs:19-24). Threads share
  objects on their stacks all the time (scoped threads, a joined thread's result, a condition variable
  on a stack). A watch set on a thread for such a page would miss that thread's accesses to its own
  frame, and the embedder's record would miss writes. So a watch that takes a permission from a page at
  or above a thread's stack pointer, inside the mapping that holds that stack pointer, also makes that
  thread give up its WebAssembly frames: it leaves `Execution::run` when it is let go and carries on from
  the bottom loop. A piece entered there knows nothing about its frame (`Seed::NOTHING`,
  tr/full.rs:851-864) and checks every access. This is the x86 plan's rule for code on a live stack
  (x86 plan lines 1898-1905), used again: one rule and one piece of code for both. It holds for
  `memory_watch` as for `memory_watch_thread`. Counted "left run: a watch was set on a live stack frame".
  It costs one leave (300 ns, docs/runtime.md section 10) and returns that go through the bottom loop
  until the calls in progress have returned.
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
stop of the thread that held the page (230 ns on average, 30 µs at worst, measured in Chrome), two table
bytes, and the embedder's own record. So about a microsecond before the embedder's work. The 2008 system
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
  with something to run (native.md section 5.1) gives one to each processor that has a thread. The
  platform bounds it for the whole host process: 64 virtual CPUs on this Mac
  (`hv_vm_get_max_vcpu_count`, native.md section 5.1), one virtual machine per process. A processor that
  finds none waits, counted "waited for a virtual CPU".
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
- **Barriers must be copied.** The native output emits no instruction at all for what the decoder calls
  nothing, `dmb`, `dsb` and `isb` among them, and leaves their route "decided with the WebAssembly
  output's" (native.md lines 350-362). Decided here: `dmb`, `dsb` and `isb` are copied as they are,
  always. The guest then pays what the reference pays for the barrier it wrote, of the kind it wrote,
  and there is no byte to test and no second form by ordering. The other forms decoded as nothing stay
  nothing. The x86-64 native output copies `mfence` and
  locked instructions as they are; the plans do not say so yet. Asked of the native track, with the cost
  on a `Serial` run measured there.
- **Exclusives are a compare and exchange there too.** The native output keeps the WebAssembly output's
  record of the load and makes the store with `cas` or `casp` against the recorded value (native.md lines
  484-500). So the difference of 7.3 holds on both executors (decision 6). One thing in it assumes one
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
  the host's count unless told otherwise (decision 2).
- **How many run at once.** The smaller of the processors and the host threads the embedder has in the
  loop (section 5). Never more than one thread of a group in force `Serial`.
- **Which host thread runs which guest thread.** Any. A guest thread is its register block and its memory;
  on the WebAssembly executor its store moves between host threads (a wasmtime store may be sent, not
  shared). In a live run a worker keeps its processor, and the kernel puts a thread on whichever
  processor asks. Keeping a thread on the processor it last ran on is added only if measured to matter.
- **Fairness.** Linux's fair scheduler as built (kernel/schedule.rs), with one run queue for all
  processors: each takes the thread with the least weighted time that may run there (4.3: its group
  allows one more thread, and its mask holds the processor). One queue is fair by
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
- **The tests' clock, and what repeats without a log.** The test system's repeatable clock moves by the
  progress each thread reports (docs/runtime.md section 9).
  - With one worker a run still repeats, with any number of processors. The worker runs the processors'
    turns one after another, in an order drawn from the seed, every turn counted. So the order of
    entries, and of clock reads, is the same each time.
  - With several workers nothing repeats without a log. The order of entries is the host's timing. So is
    a clock read made without a system call (`Answer::clock`, rt/lib.rs:291-293): it makes no kernel
    entry, and takes whatever the other workers' threads had reported by that host moment. Such a run is
    repeated from its recording (4.6), where each clock read is a recorded answer of its thread.

## 12. Tests and the bar

The test system gives every process (`Serial`, `Exact`) unless a test says otherwise. That is what the
kernel test crate's runs are today: one thread at a time, counted turns.

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
  - a process given `None` beside them: the recording system is never asked about it (its count of asks
    is zero), and the others replay as above when it does nothing to them.
- **The config in force.** Two processes given different configs come to share a file mapping, and
  later stop sharing it. `Process::execution_config()` answers what each was given throughout; the
  config in force and both counters change at the `mmap` and back; a process given `None` that joins
  an `Exact` neighbour is recorded with positions from that entry on (from code with the count, once
  the translator has it; in counted turns until then), and the two replay exactly from there.
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
- **The executor's lock rule.** Every run of the stress schedule has the try-lock of 7.2 on: reaching a
  `Serial` process's lock from outside its own thread panics.
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
  - two threads run a 128 bit compare and swap on the same pair while a third fills the same words
    through `read`: no update is lost. A fourth unmaps a page the loop around the compare and swap
    also touches: no access after the unmap;
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
  - the three shapes a compiler's merging breaks, which the standard four do not have (the third
    check's finding): a sequence lock read with plain loads (load the count, load two words, load the
    count again); a value loaded and stored back under a branch that another thread decides; a store
    read back across a load of another address;
  - for ARM64, a load ordered after a conditional branch by `isb`.
  No outcome the guest's architecture forbids is ever seen. This is the test of what 7.6 relies on, run
  on each version of each WebAssembly compiler and each host. The hand-written forms of the second
  group exist (scratch, check4) and fail today as section 14 says.
- **The compiler setting is on the Parallel engine.** A test compiles a module with those three shapes
  in each of the executor's two engines and reads the machine code: the Parallel engine's has every
  load and store, the Serial engine's does not. It fails the day a wasmtime release changes either.
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
    lock was added (4.2). The milestone less the no-lock build is the lock, reported against decision
    7's figures. It is the one cost the owner is asked to accept.
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
  call failing, never a guest's.
  - abi: `Cpu` and `Host` by `&self` and `Sync`; the processor in `Kernel::next`; the thread in
    `Kernel::exit`; the `Host` services `thread_stop` and `kernel_entry_admit`, asked only of a system
    that orders entries and only about a thread not in force `None` (4.6); the hook
    `on_processor_needed`; the stop word as a set of reasons, with `raise_stop` as an atomic or (4.5).
  - kernel: the lock, the handle that holds it and the `Send` reason (4.2); processors, masks and the
    scheduler (4.3); groups with the joining rule, and the config in force (3); the two rules of 4.4
    item 5, with calling a group in and threads ending in their own entry (4.5); entries kept aside
    when told `Later` (4.6); the idle mark (5); charging per thread (11); the told count;
    `ExecutionConfig` at the start, inherited at fork.
  - runtime: `run` and `run_until_finished` with a processor and `StopReason` per worker, the system
    told who, idle without a lost wake, the container stop, the executor's stopping handle and its
    promise in place of `StopFlag`, the local system by `&self` with a deadline per processor (5).
  - WebAssembly executor: shareable, with the process's lock kept for a thread's whole stay in
    `Execution::run` and its try-lock rule (7.2); the asked-in bit, taken off only before an entry
    (4.5); counted turns for a thread in force `Exact` and free turns for any other. Stores stay one per
    process: one thread of a group runs at a time.
  - kernel test crate: the recording and replaying test system with its replayer (4.6).
  - the stand-in stays at one processor.
  - Bar: section 12's "nothing changes with one processor", "groups in parallel", "exact replay", "the
    config in force" for the guarantee, "reaching across groups", "no request to stop is lost", "the
    executor's lock rule" and "processors and masks"; its `Serial` measurements, of which only the
    system call cost can move here; and a scaling test: four processes that only compute, on four
    processors, finish in under half the time they take on one (a quarter is the ideal; the margin is
    for this machine's load and is the orchestrator's to set).
  - Measured: a real build with parallel jobs at 1 to 16 processors; the share of time waiting for the
    kernel's lock; groups called in, by reason; stops asked for nothing; the lock's own cost in a
    system call, by the build without it.
- **T2, a group's threads in parallel on the WebAssembly executor, ARM64 guests.** After decision 4.
  - translator: the Parallel form (7.2): barriers and the 128 bit hook behind the byte (7.3), the mark
    for hooks that are stop points (4.5); the shared import and its declared maximum in both forms.
  - executor: the two engines, with the compiler setting and a test that it took effect (7.6); the
    host-supplied shared memory, first; one store per thread; group stop with its stop lock, its marks
    and the held bit, held only at a stop point (4.5); the table rules and the shared pages of 7.4,
    with the one-way copy; code dropped and added, with the count of drops (7.5); table growth.
  - kernel: `Parallel` in the config, its changes in force, the executor's limits, telling the executor
    each memory's config, the compare and exchange for the words the kernel changes, `membarrier` with
    its restartable-sequence command, the read of every call that runs again (4.4).
  - Bar: the threaded suite, "the races this plan names", the ordering tests and "the compiler setting"
    match the reference in `Parallel` under the stress schedule at full, none and bounded look-ahead;
    T1's bars hold; the `Serial` measurements of section 12 hold, the byte-for-byte one among them.
  - Measured: group stops by reason and their length; translations thrown away; a store's memory; what
    the Parallel form costs a process with one thread.
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
    stack slot), so the rule for live stack frames is in it too; and they `mprotect`, fork and rewrite
    code while the others run, so every call-in is in it. The torn-permission test of section 12 passes
    with a watch in place of `mprotect`.
  - Measured: the cost of one hand-over; hand-overs per second in the threaded suite and the runtimes;
    leaves for a watch on a live stack frame; call-ins by reason.
- **T5, the native executor** (section 10), in the native track, after its single-thread suites. With it:
  barriers copied, the exclusive's record made safe for threads in parallel, each held virtual CPU
  dropping its own cached translations.
  - Bar: T1's and T2's bars on the native executor, the ordering tests with barriers among them; the
    per-thread root under T4's test.
- **T6, x86-64 guests**, in the x86 track: the Parallel form for x86, with lock prefixes as atomic
  operations, the hook for a misaligned one and `mfence` as the tested fence (7.3); the executor's
  answer on a weaker host (section 9).
  - Bar: the threaded suite and the ordering tests, the sequence lock and the store-back among them,
    for an x86 guest in `Parallel` on the x86 laptop; on this Mac the same suite passes with the group
    kept `Serial`, its counter read by the test; the `Serial` measurements of section 12 for an x86
    guest.
- **T7, splitting the kernel's lock**, only where T1's measurement points (4.7), one part at a time.
  - Bar per part: the single-thread system call cost is not above its figure before; the time waiting for
    locks on T1's build is lower; every earlier bar holds.
- **T8, `External` for `Serial` groups** (4.6), any time after T1; it changes no translated code. The
  digest of an entry, its hook, and the replaying test system's stops without positions.
  - Bar: section 12's "`External`".
  - Measured: what the digest costs a system call that reads many bytes (a large `write`), and the
    share of the kernel test crate's programs whose replays are never thrown away.

Not milestones here: the form that orders atomic instructions for (`Parallel`, `External`) (decision 3),
the ordered x86 form (decision 5), a browser, `suspend` and `resume`, which are the embedder
interface's, and the two calls of sections 5 and 6 (decisions 8 and 9), which come with them. The group
stop that `suspend` needs in a `Parallel` process is T2's.

## 14. Measurements behind the decisions

On this Mac (Apple M5 Max, 18 cores, macOS 27.2), release builds, 2026-11-18, with other tracks running:
the load (`sysctl vm.loadavg`, 1 minute) was 49 to 142 throughout. On the x86 laptop (Intel Core i7-8750H,
12 threads, Linux 7.3.4) the load was 0.2 to 1.7 for the first runs and 5 to 11 for the checkers' and
this correction's, when other tracks were using it too; its cores 2 and 8 were not used. Scratch code and
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
  nothing and 11.4 ns by the work around it (section 14). T1's three builds settle it (section 12).
- What switching the compiler's alias analysis off costs on an x86 host, and on programs larger than
  the benchmark set (T2). On this Mac it cost the set 0.0% to 1.8%.
- Whether V8 merges or drops plain accesses as wasmtime's compiler does. Not looked at: no browser is
  built (7.6).
- How often real programs run barriers and 128 bit atomics (T3). The cost of each is measured; their
  number is not.
- A host-supplied mapping as a wasmtime shared memory: read in the source, not run (T2).
- What one store per thread costs in memory, and a second wasmtime engine (T2).
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
- An x86 guest's Parallel form on an x86 host through the translator (the x86 track, T6).
- `suspend` and `resume` on one kernel, and the two calls of sections 5 and 6: nothing exists to measure.

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
   - With it, to confirm: a guarantee is about one process. Nothing follows which bytes a `None`
     process gave to others through the kernel, so an embedder that wants a process repeated gives at
     least `External` to every process that can reach it.
2. **What is a container given when nobody says?** Recommended: `GuestMachine::processors` 1, fixed for
   the container's life, and (`Serial`, `None`) for its first process; the command line program passes
   the host's count of processors unless told otherwise. Cost: a library user who sets nothing gets no
   speed-up and no recording. The other answer for processors, the host's count, speeds everyone up at
   once and makes a container's behaviour depend on the machine that first made it. `Parallel` as a
   default is not recommended before T3's bar holds.
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
   (4.2). What it is inside a real system call is not known until T1 is built. Removing it costs the
   kernel compiled twice into every program, with and without the lock. Everything else is kept off a
   `Serial` run: its translated code is the same code (7.2), and T1's bar checks the rest with a build
   that has no lock (section 12). Recommended: accept it. If you would rather see its real size first,
   T1 can be built and measured before you rule on this one; nothing else in T1 depends on the answer.
8. **How does an embedder act on one process, from outside a hook, while the container runs?**
   Recommended: one call any host thread may make, `Kernel::process_call_in(id)`, after which the
   kernel stops that process's group and announces `on_process_called_in(process)`, where every
   operation is legal (section 5). Cost: one call and one hook on the interface; each use stops that
   group's running threads for a moment, and nobody when the process is only waiting.
   - The other answers: only `kernel.process(id)` while no worker is inside `run`, which adds nothing
     and means every page that arrives from another machine for a waiting thread stops every processor
     of the container; or a narrow call that only writes a watched range, lifts its watch and wakes the
     waiter, which is cheaper in a `Parallel` group (nobody is stopped) but cannot serve a `Serial`
     group without stopping its running thread anyway (7.2), and is a second path for one case.
9. **The interface changes: accept them, so that splitting the kernel's lock later changes none?**
   Recommended: yes. Cost: every `System` (the shipped one, the tests', your own) keeps what it changes
   in atomics or behind its own locks, and gains the arguments below.
   - In T1. `Kernel`: entry points take `&self`; `next` takes a processor; `exit` takes the thread and
     says whether it took effect; the handle that holds the lock across a thread's `exit` and `next`,
     which `arrived` is handed in the kernel's place. `Start` carries the processors, the first
     process's `ExecutionConfig`, and whether the system orders entries. `Cpu` and `Host`: methods take
     `&self`; both are `Sync`. `Host` gains two services, `thread_stop(thread)` and
     `kernel_entry_admit(entry)`, and one hook, `on_processor_needed(processor)`. `System`: methods
     take `&self`; `turn` is told the processor, the thread and its guarantee in force, `ran` the
     thread, `arrived` and `idle` the processor; `idle` also returns when its processor is needed;
     `stopped` runs once, with every worker out of `Execution::run`. `Execution::run` takes `&self`.
     `StopFlag` and `Executor::stop_flag` go; the executor gives a handle that stops a named thread, and
     promises that a stop asked for is never lost. `run` and `run_until_finished` take a processor.
     This also answers how a running container is interrupted from another thread: every worker's
     `run` returns `StopReason::Interrupted`.
   - In T2. `Cpu` gains `set_execution_config(memory, thread_ordering, replay_determinism,
     several_threads)`, `parallel_limits()`, `compare_exchange(memory, address, expected, new)` and a
     call for `membarrier`'s group stop.
   - In T8. The hook `on_kernel_entry_ended(thread, digest)`.
   - With the embedder interface's own work, two things this plan asks of it. The thread of
     `on_memory_accessing` becomes optional, none meaning "the kernel, for another process", so that a
     `write` to a file a watched process maps is seen; without it an embedder that composes exact
     replay from watches misses those writes (section 8). And `Process::replace_execution_config(self,
     config)` is offered in hooks: it is the operation the kernel has anyway for groups that merge, and
     saves a change of config the return of `run` (section 6).
10. **Names.** Suggested, all yours: "group" for threads that share memory; "config in force", with
    `ProcessInfo::execution_config_in_force()`; "called in", "group stop", "held", "stop point" and
    "stop lock"; "the Serial form" and "the Parallel form" of code; `processors` and `ProcessorId`; the
    services `thread_stop` and `kernel_entry_admit`, with `KernelEntry` and `EntryAdmission { Now,
    Later }`; `Start`'s field `kernel_entry_admission` with `Unasked` and `Asked`; the hooks
    `on_processor_needed`, `on_kernel_entry_ended` and `on_process_called_in`; `Kernel::enter` and
    `KernelEntered`; `Kernel::process_call_in`; `Executor::thread_stopper`; the stop word's bits
    `STOP_ASKED`, `STOP_HELD` and `STOP_OWN`; `HookType::stop_point`; `runs_in_parallel` for the byte;
    `Cpu::set_execution_config`, `Cpu::compare_exchange` and `Cpu::parallel_limits`;
    `RuntimeConfig::host_threads`; the counters' names in sections 3 to 9; the milestones' letter T. One
    more: the container's call is now `run`, and the executor's own call that runs one guest thread is
    `Execution::run` (rt/lib.rs:131). Two calls named `run` one layer apart will be misread; suggested
    `Execution::run_thread`.
