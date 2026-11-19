# wcontainers: guest threads in parallel

Status: plan, 2026-11-18. Nothing here is built. The owner rules on section 16 first.

Words used here:
- **executor**: what runs the guest's instructions behind the kernel's `Cpu` side (`Executor`,
  rt/container.rs:19). Three exist or are being built: the WebAssembly executor (wasmtime), the native
  executor (a virtual machine, docs/native.md on origin/phase-2-native) and the kernel tests' stand-in.
- **embedder**: whoever implements `System` (rt/lib.rs:146) and drives a container.
- **group**: every guest thread that can reach the same guest memory without the kernel in between
  (section 3). The owner's rulings are per group. A name is suggested in section 16.
- **stopped**: a guest thread is stopped when it is inside a hook or has left `run`. This is the
  translator's own word (tr/contract.rs:17-27). A stopped thread runs no guest instruction.
- **group stop**: every thread of a group is stopped at once, and held there.
- **processor**: one of the CPUs the container is told it has. One guest thread runs on a processor at a
  time. A name is suggested in section 16.
- **kernel entry**: one call into the kernel that changes it: a thread coming back (`Kernel::exit`), an
  event, a signal from outside.
- **position**: where a thread is, as (progress count, pc) (docs/runtime.md section 9).
- **reference**: a default Docker container on real Linux of the guest's architecture.

Paths are shortened: `kernel/` is crates/wcontainers-kernel/src, `abi/` crates/wcontainers-abi/src, `rt/`
crates/wcontainers-runtime/src, `wt/` crates/wcontainers-wasmtime/src, `tr/`
crates/wcontainers-translator/src, `arm64/` crates/wcontainers-arm64/src. Lines are at 77e50ddc (this
branch's base). `native.md` and `x86 plan` are docs/native.md at origin/phase-2-native ab448e58 and
docs/phase-2-guest-cpu-and-x86.md at origin/phase-2-x86 0de30f18. Every figure says whether it was measured
or is only expected. Measurements are in section 14 and in docs/status-threads.md.

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

Ruled by the owner on 2026-11-18 (brief-threads.md, next-embedder-interface.md), and taken as given:
- `enum ThreadOrdering { Serial, Parallel, ParallelOptimistic }` is how one group's threads are run.
  `ThreadOrdering::replay_determinism()` gives `Exact`, `None` and `External` for them.
- Different groups always run in parallel with each other.
- The kernel is the same under every executor. It is entered by one caller at a time to begin with. The
  interface must not prevent several at once later.
- Using cores by running more kernels of one container is rejected.
- Exact replay of a group that runs in parallel is not a kernel setting. The embedder composes it from
  per-thread memory watches (`memory_watch_thread`, on executors that implement `PerThreadWatch`).
- `ParallelOptimistic` is planned for as a third value. Nothing may depend on its being useful.

## 3. Groups

**What a group is.** A guest memory is one address space (`MemoryId`, abi/cpu.rs:5-7). The threads of a
process share one, and so do processes made with `CLONE_VM` (kernel/process.rs:70-72). Two memories are
joined when both show the same page of a block of shared pages and at least one of them may write it
through a shared mapping (`mmap` with `MAP_SHARED`, System V shared memory, a shared memory file). A group
is all threads of memories joined this way, directly or through others.

A private mapping does not join. This matters because the kernel shows every file mapping through a block
of shared pages, the program's own code included (kernel/shared.rs:1-5, kernel/memory.rs:46-50,
kernel/loader.rs:901-905). If showing a block joined memories, every process would be in one group with
every other that uses the same C library.

**Who keeps it.** The kernel. It already knows every mapping and which are shared (`Backing` and
`Mapping`, kernel/memory.rs:34, :69). It keeps, for each memory, the group it is in. Memories join when a
shared mapping that may write is made, or inherited by a fork. A group is split again, lazily, when the
mapping that joined it has gone: at the group's next group stop. A group left too large is slower, never
wrong.

**A new group has one thread.** A group is born at the container's start, at a fork whose child shares
no writable shared page with its parent, and at exec, which makes a new memory (kernel/loader.rs:292)
after ending the process's other threads (kernel/task.rs:648). For a group with one thread `Serial` and
`Parallel` are the same run. The ordering only starts to matter at the group's second thread.

**The ordering in force.** Each process carries the ordering it was given. A group's ordering in force is
the strictest of its processes' orderings (`Serial` is strictest, then `ParallelOptimistic`, then
`Parallel`) and of what the executor can run for this group (7.4 and section 9). It is worked out again
whenever the group's members change, which is always inside a kernel entry. Moving to a stricter ordering
needs a group stop first. So the kernel needs "the ordering in force changes at a group stop" whatever the
owner rules in section 6.

Rejected: one setting per group, set by whoever joined last. Two processes with different orderings can
come to share memory at any `mmap`, and then one of them would have its promise weakened without anyone
asking. With the strictest winning, a promise is only ever kept more strictly than asked.

## 4. The kernel entered by several host threads

### 4.1 Today

- Every entry point takes the whole kernel exclusively: `next`, `exit`, `event`, `kill`, `restore`
  (kernel/lib.rs:305, :317, :344, :397, :422) take `&mut self`, and `exit` takes the whole executor and
  the whole system exclusively too (`&mut dyn Cpu`, `&mut dyn Host`).
- The kernel models one CPU: one current thread and one running thread (kernel/lib.rs:136-140), one
  "handed out at" time for charging (kernel/lib.rs:161-163, kernel/task.rs:77-86), Linux's fair scheduler
  "for one CPU" (kernel/schedule.rs:1).
- The guest is told it has one CPU: `sched_getaffinity` (kernel/syscall/system.rs:161-174), `getcpu`
  (:192), `/proc/cpuinfo` (kernel/fs/proc.rs:1352-1355), `rseq` (kernel/syscall/rseq.rs:13-17).
  `membarrier` does nothing, "threads take turns on one CPU" (kernel/syscall/rseq.rs:1-4).
- A signal sent to a thread assumes the only running thread is the sender (kernel/signal.rs:489).
- The kernel's state is reached through `Rc` and `RefCell` in 155 places in 16 files (`grep -rn
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
because the kernel is `no_std` (kernel/lib.rs:9) and is entered from three places that wait differently: a
host program (`parking_lot`), the inside of the native executor's virtual machine (which waits inside,
native.md section 5.3), and the kernel built as a WebAssembly module, where only one caller exists.

**The state stays `Rc` for now.** One `unsafe impl Send` on the kernel's state, with its reason: every
`Rc` and `RefCell` is reachable only through the state, the state is reached only under the lock, and no
clone leaves it. A test checks that no public type of the kernel holds an `Rc`. The handle a hook is given
(`Process<'_, E>`) borrows under the lock and cannot outlive it.

Rejected for now: turning every `Rc` into `Arc` and every `RefCell` into a lock. It is the right end state
for the parts that are split out (4.7), and it costs every system call something (the kernel's share of
`getppid` is 116 to 120 ns, docs/runtime.md section 10). It is paid part by part at the split, each
measured.

### 4.3 What the kernel models: processors

- The container has a fixed number of processors (section 11). The kernel keeps, per processor, what it
  keeps once today: the current thread, the give-way mark, the picked runtime (kernel/lib.rs:136-140,
  :165).
- One run queue, as today. `next` takes the processor that asks (`Kernel::next(processor)`) and gives it
  the waiting thread with the least weighted time whose group may run one more thread: any thread of a
  group in force `Parallel`, and a thread of a `Serial` group only when none of that group runs.
- `exit` names the thread that came back (today it is implied, kernel/lib.rs:318-321).
- Each thread is charged from its own "handed out at" time (section 11).
- `sched_getaffinity`, `getcpu`, `/proc/cpuinfo`, `/proc/stat` and `rseq`'s CPU fields answer from the
  processors. `membarrier` becomes real work in a group in force `Parallel`: a group stop (4.5), which is
  what Linux's interrupt to the other CPUs is.

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
     is taken by the thread itself when it comes in (below).
5. Threads of other groups may be running. The kernel does not read or write the memory of a group in
   force `Serial` other than the caller's own without stopping that group's running thread first (4.5).
6. The kernel still never waits on its own stack. A `Host` call or a hook made inside an entry holds the
   lock while it runs. A slow hook stalls every processor's kernel entries until the split.

One known gap, to be closed by reading every call in T2: a call that waited and runs again reads its
arguments from guest memory again (kernel/task.rs:166-169, :360-368), except paths and descriptors, which
are kept (`Taken`, kernel/process.rs:407-414). Linux reads them once. With one thread running, nothing
could change them in between. In a `Parallel` group another thread can.

### 4.5 Who is stopped before what

Two primitives do all of it.

**Stop one thread** (suggested: a `Host` service `thread_stop(thread)`). The kernel asks the system to
bring a running thread back. The system raises the thread's stop flag, which is one atomic store from any
host thread (abi/registers.rs:54-57). The thread comes in by itself as `Exit::Stop` at its next stop check.
The kernel does not wait for it. It goes through the system, not straight to the executor, because when a
thread stops is one of the things a replay must repeat (4.6): the slice timer already works this way
(rt/system.rs:256-268).

**Group stop**. It is the executor's, done inside the `Cpu` call that needs it, because what must be
stopped is a property of each executor's output. The WebAssembly executor raises the flags of the group's
other running threads and waits until each is in a hook, where it is held. The native executor interrupts
their virtual CPUs (section 10). A thread waiting for the kernel's lock is in a hook, so it counts as
stopped; no deadlock can come of it. In a group with one running thread a group stop is nothing.

What is stopped before what:
- **A page loses a permission, is unmapped or remapped** (`set_permissions`, `shared_unmap`, a mapping
  going away): the group's other threads, by a group stop inside the `Cpu` call. This is the translated
  code's rule (tr/contract.rs:17-27).
- **A page gains a permission, or a new mapping appears**: nobody. A thread that still acts on the old
  table faults to the executor, which looks at the table again and carries the thread on. Counted "fault
  settled by the table as it is now".
- **Code is dropped** (`code_changed`, ARM64's `ic ivau`, a write to translated x86 code): the group's
  other threads, by a group stop, and each of them leaves `run`, so that no thread comes back into a piece
  made from the old bytes (docs/runtime.md section 6). Counted "left run: code changed under it" for each.
- **exec, `exit_group`, a fatal signal**: every other thread of the process. The kernel ends them
  (kernel/task.rs:648, :685). `thread_drop` of a running thread raises its flag and returns once the
  thread is out of guest code for good, as Linux waits for the other threads to die.
- **fork from a threaded process**: the group's other threads, for the length of the copy, by a group stop
  inside `memory_duplicate`, so that no page is copied half-written. Counted "group stopped for a fork".
- **A signal for a running thread**: that thread only, by `thread_stop`, and nobody waits. It takes the
  signal on its way back to the guest, as every thread does (kernel/task.rs:326-339).
- **A stop signal, `SIGCONT`, a tracer**: the process's running threads, by `thread_stop` for each. Each
  parks itself when it comes in.
- **A futex wait or wake**: nobody. Both are kernel entries, so the wait's "compare, then park" and the
  wake are already in one order.
- **`membarrier`**: the group's other threads, by a group stop. Then they carry on.
- **The kernel reads or writes the memory of another group in force `Serial`** (a `write` to a file that
  group maps, `process_vm_readv` and `process_vm_writev`, a tracer, `/proc/<pid>/cmdline`): that group's
  running thread, by `thread_stop`. The caller's call parks until the thread has come in, and the thread
  is held until the call is done. The stop is an ordinary recorded entry, so the read or write has one
  place in the order. Counted "another group stopped for its memory".
- **The ordering in force becomes stricter** (section 3): the whole group, by a group stop.

### 4.6 The order of kernel entries, recorded and replayed

With every group `Serial`, groups share no memory, by the definition of a group and the rule of 4.4 item 5.
So they affect each other only through the kernel. A run is then fixed by what is recorded today (the
host's answers, each event and each turn's end with its position, docs/runtime.md section 12) and one new
thing: the order in which kernel entries happened.

**One recording point.** Each kernel entry is given a sequence number before it takes effect. The kernel
asks the system for it (suggested: a `System` service `entry_sequence_number(entry) -> u64`, where the
entry says the processor, the thread, why it came in, and for an `Exit::Stop` its position). The kernel
then lets entries take effect in the order of their numbers.
- A system that records gives the next number and writes the entry down.
- A system that replays gives the number its log holds for that thread's next entry. A thread that
  arrives early waits in the kernel's own queue for its number. Nothing blocks inside the system.
- The provided default gives the next number and writes nothing.

In the first form every entry passes the lock, so every entry passes this point. After the split (4.7) the
point is the taking of any lock that is not the entering group's own. An entry that stays inside its group
(`brk`, a private `mmap`, a private futex, `sigaction`) then gets no number and waits for nobody, in a
recording and in a replay. This is the brief's rule: what another process or the system can observe goes
through the one point, what touches only the process's own state does not, and the kernel keeps no list of
system calls.

**Stops must come from the log too.** In a replay a thread must stop where it stopped in the recording, not
where a flag happens to reach it. So the replaying system gives each turn a counted limit that ends it at
the thread's next recorded position (`Turn::Counted`, rt/lib.rs:25-27), and its `thread_stop` does
nothing. This is why `thread_stop` is asked of the system (4.5).

**Which processor.** The processor a thread runs on is visible to the guest (`getcpu`, `rseq`). The kernel
chooses it inside an entry, from its own state. So it repeats with the entries. A replay may use fewer or
more host threads than the recording did; the order of entries is all that is kept.

**What this needs of other tracks.** An exact position at full speed needs the translator's progress
count (docs/runtime.md section 9, not built). Until then stop checks stand in for it, which repeats at one
look-ahead, as the kernel tests' `repeats` does today.

**Group stops add nothing to record here.** In a group in force `Serial` no other thread of the group is
running, so a group stop (4.5) stops nobody and a replay has nothing to repeat for it.

Rejected: a recording hook called with the lock held, and a replay that blocks inside a hook until its
turn. The first cannot gate a replay. The second holds the lock while it waits for a thread that needs the
lock.

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

**Today.** `Stepper::step` runs one thread and returns; `drive` repeats it (rt/lib.rs:197-243). The
executor is `&mut`, and one `StopFlag` raises "whichever thread is running" (rt/lib.rs:33-57). The local
system has one timer thread with one deadline (rt/system.rs:41-55).

**The change: the same loop, called by as many host threads as the embedder gives.** `step` and `drive`
take the kernel, the executor and the system by `&`. Each host thread that calls `drive` is one processor
until it returns. One host thread calling it is today's behaviour exactly. The embedder's limit on host
threads is simply how many of its threads call it. `Container::run` spawns them itself, up to the smaller
of the container's processors and a count in `RuntimeSettings` (suggested `host_threads`, default the
host's `std::thread::available_parallelism`).

Rejected: a pool of worker threads hidden in the runtime. An embedder with its own event loop (the owner's
other project drives many kernels) must decide which of its threads work for which container. Rejected: a
second loop for the parallel case. One loop, entered N times.

What changes with it:
- `Execution::run(&self, ...)` (rt/lib.rs:128-143): several host threads are inside `run` at once, each
  with its own thread. The executor does its own locking (section 7).
- **Idle.** A processor with nothing to run waits in `System::idle`. It must also wake when another
  processor's entry made a thread ready. `Kernel::exit` returns how many waiting processors now have work,
  and the loop wakes that many.
- **Stopping a thread.** `StopFlag` is replaced by one method any host thread may call on the shared
  executor (suggested `Executor::thread_stop(&self, thread)`), since the flag to raise now has to be
  named. The system's `thread_stop` service calls it. The timer thread keeps one deadline per processor.
- **A container stop.** `System::stopped` runs when no thread is in `run` (rt/lib.rs:156-158), which a
  save needs. With several processors the loop first brings every processor out of `run`, then calls it
  once.
- `Host` and `System` methods take `&self`, so a system keeps what it changes behind its own locks.

## 6. Changing the ordering of a running process: two forms weighed

**Form A, a setter.** `Process::set_thread_ordering(ordering)`, taking effect at the group's next group
stop.

**Form B, no setter.** The ordering is fixed while a process is in a kernel. It is given when the container
starts and when a process is imported. It is changed by exporting the group and importing it again, on the
same kernel when nothing else changes.

"The next stop" means the same in both: the first moment at which every thread of the group is stopped at
once. The kernel does not wait for it to happen by chance. It makes it happen with a group stop.

- **Work in the kernel.** A: almost none beyond section 3: change the process's ordering, and work out
  the group's ordering in force at a group stop. B: none beyond section 3, once export and import exist.
  Neither exists yet (next-embedder-interface.md: "nothing of export or of remote objects yet").
- **Work in the executors.** A: none on the WebAssembly executor for ARM64 guests (one translation, one
  store per thread, section 7); the native executor rebuilds the group's views (native.md section 5.3); an
  x86 guest on a weaker host would need its code translated again (section 9). B: the same work, done by
  the import path, which must exist anyway.
- **Cost of one change.** A: one group stop. Expected under a microsecond on average and tens at worst:
  the stop flag was noticed in 230 ns on average and 30 µs at worst in Chrome (docs/phase-1-translator.md,
  "The stop check"). B: one export and one import of the group: its kernel records (descriptors,
  mappings, threads, waits) and, if memory were copied, all its memory. Not measured: nothing exists to
  measure.
- **A consistent starting point for a replica.** A: the embedder takes its own at the same stop, when it
  wants one; a change to `Parallel` needs none. B: it comes with the change. The exported group is the
  starting point.
- **When the change cannot be made.** A: never. B: whenever export cannot carry something the group
  holds. Every kind of kernel object must be exportable, or a process holding one is stuck with its
  ordering.
- **Ways a process gets an ordering.** A: two, given at start or import and changed in place. B: one.
- **Interface.** A: one more method, already named. B: none.

**The orchestrator's condition for B** is right and is needed for B to be usable at all: export must hand a
group's memory over by ownership, not by copying, and ends whose other side is in the same kernel must
collapse back into local objects. Without the first, a change costs a copy of the group's memory (3.2 ms
for 48 MB, docs/runtime.md section 18). Without the second, a pipe between an exported process and its
neighbour stays a stream through the embedder for ever.

**Recommendation: B, with one addition that makes it usable before export exists.**
- The container's ordering is given in `Start` (kernel/container.rs:55, read by `Kernel::start`, :405) and
  every group
  born in the container gets it. Import states one.
- Until export and import exist, that is the only way to give one. This loses less than it seems: a group
  is born with one thread (section 3), where the ordering makes no difference, so "this container's
  threaded programs run in parallel" or "everything in this container replays exactly" is one value at
  start.
- Choosing per program arrives with export, at `on_program_starting`, which the owner already named as the
  cheap moment to move a process: one thread, a fresh memory, few records.
- The reasons: a change of ordering changes what replay promises, and B makes that visible as an import
  with an ordering written on it. The kernel gets no second path. And nothing has to be built for it now.
- What it costs: no running multi-threaded process can change its ordering until export and import are
  built and complete, which is another track's work and not small.

If the owner prefers A, it costs one method and its tests, on top of the "ordering in force changes at a
group stop" that section 3 needs anyway. Nothing in this plan's milestones would have to be undone for
either answer.

## 7. The WebAssembly output for a group whose threads run at once

### 7.1 Today

- The guest memory is imported unshared (tr/code.rs:1001-1010, tr/contract.rs:39-43). The state memory,
  holding thread blocks, the page table and the array of slots, is already shared (tr/thread.rs:33,
  tr/contract.rs:45), and the stop flag in it is already written by another host thread while the guest
  reads it with a plain load (tr/code.rs:1300).
- One wasmtime store per process holds all its threads' instances (docs/runtime.md section 8;
  wt/lib.rs:451-474). `run` takes the store out, so "one thread of a process runs at a time"
  (wt/lib.rs:2245-2251). The executor is one `&mut` object with one running thread (wt/lib.rs:409-433).
- ARM64's atomic instructions are already WebAssembly atomic operations: the single-instruction atomics
  (arm64/memory.rs:112-202), compare and swap (:203-216), load-acquire and store-release (:346-356), and
  the exclusives, where the load remembers address, size and value (:405-415) and the store is a compare
  and exchange against the remembered value (:329-338). Engines accept atomic operations on an unshared
  memory (measured, ordering-cost/result.json), which is why this works today.

### 7.2 A shared memory, and one translation for both orderings

**Decision: the guest memory import becomes shared, always. One translation serves `Serial` and
`Parallel`.** A wasmtime store is used by one host thread at a time, and only a shared memory can be
imported by instances in several stores (wasmtime 50 API documentation: `Store`, `SharedMemory`). So
threads at once need the shared import. Whether `Serial` needs a second, unshared translation is a question
of cost only.

Measured (section 14): the benchmark set at full look-ahead with the import shared against unshared,
instructions retired, eight workloads, alternating. For one translation, compile and run of each workload
the shared import took between 0.16% fewer and 0.33% more instructions, with both signs among the eight
and no more than the same workload differed between two identical runs. An earlier measurement on hand-written
kernels found the same in wasmtime 49, Node and Chrome: "at most 5% either way, within the noise"
(ordering-cost/result.json). The translator track measured the same in Chrome (docs/phase-1-translator.md,
"The stop check"). So one translation, and `Serial` does not get slower.

Rejected: two translations, keyed by ordering. It doubles compile time and cache size for every program
that runs under both, and adds a path, to save a cost that is not there.

What follows from a shared import:
- **A declared maximum.** A shared memory must declare one. V8 refuses, at compile time, a module whose
  declared maximum is above its own limit of 262,144 pages (16 GiB): measured in Node 24.14.1, "maximum
  memory size (281474976710656 pages) is larger than implementation limit" (section 14). So the declared
  maximum is the executor's address ceiling for that output, and it is part of the cache key through
  `OUTPUT_FORM` (wt/lib.rs:108), which changes once.
- **Host-supplied memory.** The runtime owns each guest memory's mapping (docs/runtime.md section 8). In
  wasmtime 50.0.1 `SharedMemory::new` makes its own mapping (its src/runtime/vm/memory/shared_memory.rs:34-50),
  but a shared memory that a module defines goes through the host's creator
  (src/runtime/vm/memory.rs:251-256) and is reached with `Instance::get_shared_memory`
  (src/runtime/instance.rs:554). So each process's memory is defined by a one-memory module. Read in the
  source, not run: it is T2's first task. If it fails, wasmtime's own mapping serves: the kernel's access
  by pointer and the fork copy work on any mapping.
- **One store per guest thread**, always, in place of one per process. Module and engine stay shared. A
  dropped thread's store is kept for the process's next thread, as its instances are today
  (wt/lib.rs:478-483). What a store costs in memory is not measured (section 15).
- **The executor becomes shareable.** Its tables of processes, threads and blocks go behind a lock. What it
  keeps per process (page records, code lists, shown blocks, wt/lib.rs:451-474) goes behind that process's
  own lock, taken by hooks of that process's threads. Order: the kernel's lock first, then a process's.
  A hook never calls the kernel while holding a process's lock.

### 7.3 Atomic and exclusive instructions

- **ARM64, nothing changes** for the forms listed in 7.1. They are atomic operations already.
- **Exclusives are a compare and exchange.** The store succeeds when memory holds the value the load saw,
  even if another thread changed it and changed it back. Hardware would fail that store. A program can
  tell only if it is written against the instruction itself; compiled code uses these instructions to
  build a compare and exchange, which has the same blind spot by definition. This is a written difference
  from the reference (decision 6). Rejected: an exact monitor, by a group stop at every exclusive pair or
  by watching every store: both cost every lock operation of every program.
- **128 bit pairs.** Compare and swap of a pair of 64 bit registers, and the exclusive load and store of
  such a pair, are plain loads and stores today, "with one thread this is exact" (arm64/memory.rs:239,
  :310). WebAssembly has no 128 bit atomic operation. They become one new guest hook, which makes the
  access under a group stop. Exact, and slow: counted "128 bit atomic made with the group stopped". In a
  group with one running thread the group stop is nothing and the hook costs a hook call (about 10 ns,
  docs/runtime.md section 10). These instructions are rare; how rare in real programs is measured in T3.
- **x86 lock prefixes** are emitted as their plain forms today, because one thread runs (x86 plan line
  1839). Asked of the x86 track: a locked instruction and `xchg` with memory become WebAssembly atomic
  operations when the address is aligned for its size, and the group-stop hook when it is not (x86 allows a
  locked access at any address; WebAssembly's atomic operations trap on a misaligned one). `cmpxchg16b`
  takes the hook. One translation still: on an x86 host an atomic operation is the lock prefix the program
  pays natively. The x86 track measures it on `Serial` runs before it lands.

### 7.4 The page table read by several threads while one changes it

The table is one byte per page, read by translated code with a plain load (tr/code.rs:674). What a piece
learned from it holds until its thread's next stop (tr/contract.rs:17-27). So:
- raising a permission needs no stop (the table of 4.5);
- lowering one is a group stop, then the write, then carry on. The executor's own write locks
  (wt/lib.rs:486-489) follow the same two rules: placing a lock lowers, lifting one raises;
- bytes are written with atomic stores, one byte or one aligned word at a time, so a reader never sees half
  a value.

**Shared pages between processes.** The WebAssembly executor shows a block of shared pages by copying: each
memory holds its own copy, and written pages are copied to the block and the other memories "before the
kernel sees anything" (docs/runtime.md section 8; `flush`, wt/lib.rs:1341, :2322). That is exact only
while one of the sharing memories runs at a time. So **a group that spans several memories runs `Serial`
on this executor**, whatever was asked: the executor reports that it can run nothing else for such a group
(section 3), counted "group of several memories kept Serial". Threads of one process share one memory and
are not affected. The native executor maps the same frames in each process (native.md section 5.3) and has
no such limit. A real shared mapping under wasmtime is possible only where the host's pages are no larger
than the guest's (docs/runtime.md section 8: this Mac's are 16 KiB); it is not in this plan.

### 7.5 Code a program writes while another thread runs it

- **Dropping code** is a group stop, and every other thread leaves `run` before it carries on (4.5). ARM64
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

### 7.6 What the engines must do, and a browser

Two things are relied on that WebAssembly's rules do not promise. Both are tested in the suite on every
engine version (section 12).
- **A plain load in a loop is done again each time round.** Guest programs wait on words another thread
  writes, with plain loads (optimistic-replay-results.json: "real programs peek at lock words with plain
  reads constantly"). Measured (section 14): wasmtime 50.0.1, and Node 24.14.1's V8 with its optimising
  compiler alone, its baseline compiler alone, and both, all saw another thread's store from a loop of
  plain loads on a shared memory.
- **Plain accesses are compiled to the host's plain accesses, in program order.** Then a guest gets the
  host's own ordering, which is its own when host and guest are the same architecture, and stronger for an
  ARM64 guest on an x86 host. Section 9 is the case where it is weaker.

**A browser** (not built, docs/runtime.md section 17) needs:
- the page to be cross-origin isolated, so that shared memories can be sent to workers (the HTML
  standard's `crossOriginIsolated`; the two response headers `Cross-Origin-Opener-Policy: same-origin` and
  `Cross-Origin-Embedder-Policy: require-corp`). The state memory is already shared and already written
  from the page's thread, so today's browser runner already needs this. A shared guest memory adds no new
  requirement;
- one worker per processor, each with its own instances of the same compiled modules;
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
- **A watched access** comes to the memory fault hook, the thread leaves `run`, the executor finds the
  watch and calls `on_memory_accessing`. If the watch is gone when it returns, the thread carries on.
- **Room.** Each table is one byte per page under the ceiling: 1020 KiB for ARM64's 4080 MiB, of which
  untouched pages cost nothing. They share the state memory's 4 GiB with the array of slots (docs/runtime.md
  section 8), so the ceiling's formula gains the number of watched threads.

**What it costs** (expected, from parts measured apart): a handover of a page from one thread to another
is one watched access (leaving `run` and coming back, 300 ns, docs/runtime.md section 10), one stop of the
thread that held the page (230 ns on average, 30 µs at worst, measured in Chrome), two table bytes, and the
embedder's own record. So about a microsecond before the embedder's work. The 2008 system the owner saw
paid over 40,000 cycles a handover on one machine (dsm-survey/results.json, SMP-ReVirt), most of it
hardware page protection and its interrupts, which a byte in a table does not have. The count of
handovers is the program's and is not reduced: programs that write the same pages from several threads
will hand over constantly. T4 measures it on real programs.

The stand-in does not implement `PerThreadWatch`. The native executor does it with a table root per thread
(section 10).

## 9. An x86-64 guest on a host with weaker memory ordering

**The promise.** x86 promises total store order: a thread's stores reach memory in the order it made them,
and its loads are not reordered with each other. Only a store followed by a load of another address may
appear swapped. ARM64 promises less: two loads, or two stores, to different addresses may take effect in
either order (Arm Architecture Reference Manual, the memory model). WebAssembly's own rules promise
nothing at all for plain accesses that race.

**Where it matters.** Only for an x86-64 guest, in a group in force `Parallel`, on the WebAssembly
executor, on a host that is not x86: wasmtime on ARM64, and any browser on ARM. On an x86 host wasmtime
compiles plain WebAssembly accesses to plain moves, which keep the order (ordering-cost/result.json); that
is the engine dependency of 7.6, tested with ordering tests (section 12). The native executor runs only
guests of the host's own architecture (native.md section 11), so the hardware gives the right order.

**What staying correct needs there.** Every guest load and every guest store as a WebAssembly atomic
operation. Atomic operations are sequentially consistent, which is stronger than total store order.
Making only the stores atomic is not enough: two plain loads may still be reordered on ARM. Atomic
operations trap on a misaligned address and x86 allows any address, so each access also needs an alignment
test, with misaligned accesses made another way (under a group stop, or plain and unordered as a written
difference).

**What it costs**, measured before this plan on this Mac, single-threaded (ordering-cost/result.json):
every access atomic costs from nothing (a pointer chase) through 1.1 to 1.4 times (sums, byte loops,
calls) to 1.8 to 2.2 times (copying 8 bytes at a time). With the alignment test, 2.0 to 2.7 times on the
copy. On ARM64 both engines turn an atomic load into a load-acquire and an atomic store into a
store-release. On an x86 host the same code costs 12 to 17 times, since each atomic store gets a full
barrier, so this form must never be used there. It is therefore a second translation, used only for x86
guests in parallel on weaker hosts, and a field of the cache key.

**What is done until it is built.** On such a host the executor reports that it can run an x86 group only
`Serial` (section 3). The group runs `Serial`, counted "x86 group kept Serial: the host orders memory more
weakly", warned once per container. Lock prefixes become atomic operations in every case (7.3), which is
all an x86 host needs.

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
- **Stopping one thread** is an interrupt to its virtual CPU. From the host, `hv_gic_set_spi` took 2.4 to
  4.1 µs to reach the inside's handler (native.md section 5.5). From one virtual CPU to another inside,
  through the interrupt controller, is not measured; the native track measures it with this work.
- **Group stop** is that interrupt to each virtual CPU running a thread of the group. Each lands at an
  instruction boundary, with an exact position, and waits in the inside. A change to a page table is then
  followed by the architecture's broadcast invalidation, as Linux does.
- **A table root per thread, only once it is watched.** A thread without per-thread watches uses its
  process's root. One with watches has its own root and address-space identifier, sharing every lower
  table that is the same for all (native.md section 5.3; its N1 builds and tests the per-thread root). This
  is `PerThreadWatch` there.
- **Atomics need nothing.** The guest's own atomic instructions run on the hardware, the 128 bit ones
  included. So do exclusives, with the hardware's own monitor.
- **Shared pages between processes** are the same frames, so a group that spans memories runs in parallel
  there.
- **Replay** is the same sequence numbers (4.6), asked through the link.

Asked of the native track: nothing before its own milestones need it. What it must not close off is
already written in its section 5.3.

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
  shared). The kernel prefers nothing yet. Keeping a thread on the processor it last ran on is added only
  if measured to matter.
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
  sequence numbers, the host's answers, each stop's position), then replays it. Every thread's registers
  at every entry and a hash of every memory at every entry are the same. Run over seeds, with fewer and
  with more host threads in the replay than in the recording.
- **A suite of threaded programs** (new, in the kernel test crate, built for the guest as its other
  programs are), each with output that does not depend on the schedule: locks; condition variables;
  atomics in each instruction form (the single-instruction ones, exclusives forced by the build flags,
  128 bit pairs); thread-local storage; signals sent to threads, masks and `sigwait`; fork from a threaded
  process; exec and `exit_group` with threads running; `membarrier`; robust futexes; code written by one
  thread and run by another while it is rewritten; a wait on a word with plain loads. Each matches the
  reference in `Parallel`, on every executor.
- **Under a stress schedule.** The test system raises stop flags of random running threads at random
  times, varies the number of processors, and delays entries at random, all from a seed. Each program runs
  over many seeds.
- **Ordering tests.** The standard small two-thread tests of memory ordering (store buffering, message
  passing, load buffering, independent reads of independent writes), as guest programs, many rounds each:
  no outcome the guest's architecture forbids is ever seen. This is the test of the engine dependencies of
  7.6, run on each engine version.
- **The runtimes.** The kernel test crate's Go, Java, Node and Python programs
  (crates/wcontainers-kernel-test/tests/go.rs, java.rs, node.rs, python.rs) with threads, in `Parallel`
  under the stress schedule.
- **Speed.** `Serial` must not get slower: the translator's benchmark set, instructions retired, within
  the spread between identical runs of the figure before each milestone (under 0.5% in section 14). Speed-up
  is measured, with machine and load, on a real build with parallel jobs and on a threaded program in
  each runtime, at 1, 2, 4, 8 and 16 processors, with the time spent waiting for the kernel's lock and the
  group stops by reason.

## 13. Milestones

Each bar is checkable by the named tests. The cheapest large win is first.

- **T1, groups in parallel, every group `Serial`.** No translated code changes. No ordering but `Serial`
  exists yet.
  - abi: `Cpu` and `Host` by `&self` and `Sync`; a processor in `Next` and `Kernel::next`; the thread in
    `Kernel::exit`; `thread_stop`; the sequence number service.
  - kernel: the lock and the `Send` reason (4.2); processors and the scheduler (4.3); groups (3); charging
    per thread (11); the told count; a signal to a running thread; reads and writes of another group's
    memory (4.5).
  - runtime: the loop entered by several host threads, idle wake, the container stop, `thread_stop` in
    place of `StopFlag`, the local system by `&self` with a deadline per processor (5).
  - WebAssembly executor: shareable, with the process's lock (7.2, last item). Stores stay one per process:
    one thread of a group runs at a time.
  - the stand-in stays at one processor.
  - Bar: section 12's "nothing changes with one processor", "groups in parallel" and "exact replay", and a
    scaling test: four processes that only compute, on four processors, finish in under half the time they
    take on one (a quarter is the ideal; the margin is for this machine's load and is the orchestrator's
    to set).
  - Measured: a real build with parallel jobs at 1 to 16 processors; the share of time waiting for the
    kernel's lock; the system call cost with one thread, against today's.
- **T2, a group's threads in parallel on the WebAssembly executor, ARM64 guests.** The shared import and
  its declared maximum; host-supplied shared memory; one store per thread; group stop; the table rules;
  code dropped and added; table growth; the 128 bit hook (section 7). In the kernel: the ordering in
  `Start`, the ordering in force, `membarrier`, the read of every call that runs again (4.4).
  - Bar: the threaded suite and the ordering tests match the reference in `Parallel` under the stress
    schedule at full, none and bounded look-ahead; T1's bars hold; the benchmark set with every group
    `Serial` is within the spread of its figure before T2.
  - Measured: group stops by reason and their length; a store's memory.
- **T3, the runtimes and real threaded programs.** Go, Java, Node and Python with threads, and code
  compiled while other threads run.
  - Bar: section 12's runtimes pass in `Parallel` under the stress schedule.
  - Measured: speed-up of a threaded program in each; group stops for code changes; how often the 128 bit
    forms run.
- **T4, per-thread watches on the WebAssembly executor** (section 8). After the embedder interface's
  memory watches exist (another track).
  - Bar: a test embedder holds each page for one writer or several readers, records every handover with
    both threads' positions, and replays a threaded program from that record with the same registers and
    memory at every handover.
  - Measured: the cost of one handover; handovers per second in the threaded suite and the runtimes.
- **T5, the native executor** (section 10), in the native track, after its single-thread suites.
  - Bar: T1's and T2's bars on the native executor; the per-thread root under T4's test.
- **T6, x86-64 guests**, in the x86 track: lock prefixes as atomic operations (7.3); the executor's answer
  on a weaker host (section 9).
  - Bar: the threaded suite and the ordering tests for an x86 guest in `Parallel` on the x86 laptop; on
    this Mac the same suite passes with the group kept `Serial`, its counter read by the test.
- **T7, splitting the kernel's lock**, only where T1's measurement points (4.7), one part at a time.
  - Bar per part: the single-thread system call cost is not above its figure before; the time waiting for
    locks on T1's build is lower; every earlier bar holds.

Not milestones here: `ParallelOptimistic` (decision 4), the ordered x86 form (decision 5), a browser, and
changing a running process's ordering, which comes with export and import (section 6).

## 14. Measurements behind the decisions

On this Mac (Apple M5 Max, 18 cores, macOS 27.2), wasmtime 50.0.1, release builds, 2026-11-18, with other
tracks running: the load (`sysctl vm.loadavg`, 1 minute) was 49 to 96 throughout. Scratch code and logs
are under pyro-parked/wcontainers-scratch/threads. Details and every command are in docs/status-threads.md.

- **A shared guest memory against an unshared one, single-threaded** (logs/shared-cost.log). The
  translator's benchmark set (`programs/bench.c`, -O2, full look-ahead, every check), each workload in its
  own process, alternating the two. Instructions retired by the whole process, three rounds with one run
  in each process (logs/shared-cost.log) and two rounds with three runs in each (logs/shared-cost-runs3.log).
  The difference of the two, halved, is one translation, compile and run with the process's fixed work
  (9.3 to 9.4 thousand million instructions) taken out. Shared against unshared: fib -0.03%, sieve +0.07%,
  hash +0.10%, sort +0.07%, interpret -0.11%, matrix -0.09%, walk -0.16%, search +0.33%. Two rounds of the
  same setting differed by up to 0.39% (search, shared). Times were within the load's noise: the best of
  three was lower with the shared import in five workloads of eight.
- **A loop of plain loads sees another thread's store** (logs/spin-wasmtime.log, logs/spin-node.log). One
  thread spins on a plain 64 bit load from a shared memory, another stores to the word after 200 to 300
  ms. wasmtime 50.0.1: seen, in a bare loop, in a loop with the translator's stop check, and with an
  atomic load. Node 24.14.1: seen in all three, with the default compilers, with `--no-liftoff` and with
  `--liftoff-only`.
- **V8's limit on a declared maximum** (same run): Node 24.14.1 refused to compile a module importing a
  shared 64 bit memory with a declared maximum of 2^48 pages; with 262,144 pages it compiled.
- From earlier work, not measured again: atomic accesses against plain ones, and shared against unshared
  memories, in wasmtime 49, Node and Chrome (ordering-cost/result.json); the stop flag's latency in Chrome
  (docs/phase-1-translator.md); the cost of recording page handovers in a 2008 system
  (dsm-survey/results.json); what ordering only atomics and kernel entries would miss
  (optimistic-replay-results.json); system call, fork and instantiation costs (docs/runtime.md section 18);
  the native executor's interrupts (native.md section 5.5).

## 15. Not yet measured

- How long processors wait for the kernel's one lock in a real build (T1). This decides whether and where
  the lock is split.
- A host-supplied mapping as a wasmtime shared memory: read in the source, not run (T2).
- What one store per thread costs in memory (T2).
- How long a group stop takes with several threads running, and how often real programs cause one (T2,
  T3).
- The ordering tests: none was run. That plain accesses misbehave for an x86 guest on this Mac rests on
  the architecture's manual, not on an experiment here.
- A handover through per-thread watches, end to end (T4).
- An interrupt from one virtual CPU to another inside the native executor's machine (T5).
- An x86 guest's locked instructions as atomic operations on `Serial` runs (the x86 track).
- Export and import of a group, by ownership or otherwise: nothing exists to measure (section 6).
- The x86 laptop was not used for this plan.

## 16. Decisions for the owner

1. **How does a running process's ordering change: only by export and import, or by a setter?**
   Recommended: only by export and import, with the container's ordering given in `Start` so that it is
   usable before export exists (section 6). Cost of this answer: until export and import are built, a
   running threaded process keeps the ordering its container started with; afterwards a change costs one
   export and import of the group, which is cheap only if memory moves by ownership. Cost of the setter:
   one method and its tests now; a change costs one group stop; the kernel has two ways a process gets an
   ordering, and the embedder takes its own starting point for a replica.
2. **How many processors is a container told, and who says?** Recommended: `GuestMachine::processors`,
   default 1, fixed for the container's life; the command line program passes the host's count unless told
   otherwise. Cost: a library user who sets nothing gets no speed-up. The other answer, defaulting to the
   host's count, speeds everyone up at once and makes a container's behaviour depend on the machine that
   first made it.
3. **When an executor cannot run the ordering asked for, does the group run under the strictest one it
   can, or is the request refused?** Two cases exist: a group spanning several memories on the WebAssembly
   executor (7.4), and an x86 guest in parallel on a weaker host (section 9). Recommended: run it `Serial`,
   counted and warned, and `Process::thread_ordering()` answers the ordering in force, so nothing is
   faked: `Serial` keeps every promise `Parallel` makes. Cost: such programs get no speed-up there and say
   so. Refusing costs more: the first case appears at an `mmap` in the middle of a run, where the only
   refusal is to fail the guest's call, which Linux would not.
4. **Is `ParallelOptimistic` built?** Recommended: no. The value stays in the enum, and by decision 3 a
   group asked for it runs `Serial`. The measurements say it would not replay any of the 11 programs
   measured on ARM64, because C libraries and language runtimes read lock words with plain loads, and its
   single sequence costs 24 to 97 ns an event, which by calculation adds 0.7 to 4 times the run time for
   Go and Java (optimistic-replay-results.json). Building it later costs a second translation with a
   ticket at every atomic instruction, and a field of the cache key; nothing here closes that off.
5. **Is the ordered form for x86 guests on weaker hosts built?** Recommended: not now. Until then such
   groups run `Serial` on ARM64 hosts and in parallel on x86 hosts. Building it costs a second translation
   of every x86 program, up to 2.7 times slower single-threaded (section 9), to gain cores for threaded x86
   programs on ARM hosts.
6. **Two places where the WebAssembly output differs from hardware in a parallel group: accept them?**
   An exclusive store succeeds if the value is the same again, where hardware would fail it. A 128 bit
   atomic is made with the group's other threads stopped, which is exact and slow (7.3). Recommended:
   accept both, on the written list of differences. The alternative for the first costs every lock
   operation of every program.
7. **The interface changes of T1: accept them now?** `Cpu`, `Host` and `System` methods take `&self` and
   must be `Sync`; `Kernel`'s entry points take `&self`; `next` takes a processor; `StopFlag` is replaced
   by a call that names the thread. Recommended: yes, all in T1, so that splitting the kernel's lock later
   changes no interface. Cost: every `System` (the shipped one, the tests', the owner's own) keeps its
   changing state behind its own locks. This also answers the open question of how a running container is
   interrupted from another thread.
8. **What counts as looking inside a process** (for `on_inspection_starting` and `on_inspection_ended`)?
   Suggested line, the same test the kernel uses in 4.5: reading or writing a group's memory or registers
   from outside the group is inspection (`process_vm_readv`, a tracer, a core dump, `/proc/<pid>/mem`, and
   also `/proc/<pid>/cmdline` and `environ`, which read the process's stack); reading what the kernel
   itself keeps about it (`stat`, `status`, descriptors) is not. Cost of drawing it there: `ps` reading
   command lines is inspection. Drawing it elsewhere needs a list of exceptions.
9. **Names.** Suggested, all yours: "group" for threads that share memory; `processors` and
   `ProcessorId`; the `Host` service `thread_stop` and the executor's `thread_stop`; the `System` service
   `entry_sequence_number` and its `KernelEntry`; `RuntimeSettings::host_threads`; the counters' names in
   sections 4, 7 and 9; the milestones' letter T.
