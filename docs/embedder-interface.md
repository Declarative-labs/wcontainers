# The kernel's interface to its embedder

What the streams track built for S2, from the owner's rulings in
orchestrator/next-embedder-interface.md (its last sections are the specification). The stream
ends of S1 are in docs/streams.md. Where this document and the rulings differ, the differences are
listed under "Departures" in docs/status-streams.md.

## The embedder's trait

The kernel reaches its embedder through `wcontainers_kernel::System`, which extends the abi's
`Host` (clocks, randomness, warnings, the backend, connections, stream ends). Everything below has
a provided default, so an embedder that implements none of it gets a counter for task ids and no
hooks.

```rust
pub trait System: Host + Sync {
    // Services: the kernel asks and uses the answer.
    fn task_id_allocate(&self, previous: TaskId) -> TaskId { task_id_after(previous) }
    fn task_id_free(&self, id: TaskId) {}

    // Hooks: the kernel announces; synchronous, nothing returned.
    fn on_process_starting(&self, process: Process<'_>) {}
    fn on_program_starting(&self, process: Process<'_>) {}
    fn on_thread_starting(&self, process: Process<'_>, thread: TaskId) {}
    fn on_process_exiting(&self, process: Process<'_>) {}
    fn on_process_outgrown(&self, process: Process<'_>, bytes: u64) {}
    fn on_memory_accessing(&self, process: Process<'_>, thread: TaskId, address: u64, access: Access) {}
}

/// A process or thread id: one space for both, as Linux's `pid_t`.
pub struct TaskId(pub u32);
pub fn task_id_after(previous: TaskId) -> TaskId; // the default counter, wrapping as `alloc_pid`
```

Every method of `System` and of the abi's `Host` takes `&self`, and `System` requires `Sync`, so
that the kernel may later call hooks for different processes at the same time (the owner's ruling
of 2026-11-19); the embedder synchronizes what it changes (atomics, or its own locks; the test host
keeps its state and the test's hooks each behind a `Mutex`). Kernel calls that take the embedder
take `&dyn System`: `Kernel::start`, `exit`, `event`, `kill`, `restore`.

### Task ids

- `task_id_allocate` is asked for every new process and thread number, the container's first
  process included. `previous` is the number handed out last, which the default counter needs
  (the owner's names have no argument; listed for the owner). The kernel skips a number something
  still carries (as `alloc_pid` skips used ones) and asks again, counted (`task id still
  carried`); after as many tries as there are numbers, fork and clone fail with EAGAIN, as Linux's
  do when every number is taken. A number outside 1 to `pid_max` fails the fork or clone with
  EAGAIN at once, counted (`task id out of range`).
- `task_id_free` is called when no process, thread, process group or session carries a number any
  more: a thread that ended, a process reaped (or one whose parent did not want it), the numbers a
  process leaves behind by `setpgid` or `setsid`, the old number of a thread that took its
  process's number by `execve`, and every number still carried once the container has ended. It
  is called when the kernel settles, after the call that let go.
- Because the embedder hands out the numbers and hears when each is free, a number it keeps never
  names another process without its knowing; no generation is carried.

### When each hook fires

- `on_process_starting`: a process made by fork, vfork or clone without `CLONE_THREAD`, at the end
  of the clone call that made it, before the child's first instruction (in the parent's call, after
  the child's memory, registers and descriptors exist and after the stores `CLONE_PARENT_SETTID`
  and `CLONE_CHILD_SETTID` ask for). Also the container's first process, once its program is
  loaded, just before `on_program_starting`.
- `on_program_starting`: at the end of a successful `execve` (and the container's start), once the
  new program is loaded, its stack written and its registers set, before its first instruction.
  The old program is gone by then.
- `on_thread_starting`: a clone with `CLONE_THREAD`, at the end of the clone call, before the new
  thread's first instruction; `thread` is its number.
- `on_process_exiting`: when a process is about to be torn down (`exit_group`, the last thread's
  exit, a fatal signal), after the kernel's last stores into its memory (each thread's robust
  list and the word `CLONE_CHILD_CLEARTID` named, as Linux's `mm_release`) and before any of its
  threads or memory go: its memory and records are still readable, and a watch set then has
  nothing of the kernel's left to meet. The number stays carried by the zombie until it is reaped
  (`task_id_free`). When the container's first process ends it is announced first; then every
  other process is killed and announced in turn, before the first's memory goes (Linux's first process of a namespace kills
  the rest on its way out, `zap_pid_ns_processes`); once the container has ended every number
  still carried is freed.
- `on_process_outgrown`: a request for memory would reach past the executor's ceiling: a fixed
  mapping (`MAP_FIXED`, `MAP_FIXED_NOREPLACE`, `MREMAP_FIXED`) or a heap growth (`brk`) ending
  above it, whatever lies in the way (the stack sits just below the ceiling, so heap growth past
  it is always outgrowing it), a program segment the loader would place above it (the loader
  works on the new program's map before the old one is destroyed), or a placement (mmap, mremap,
  the loader) that finds no room below it. The request is refused before it has any effect; the
  hook fires with the bytes asked for, and only when it returns does the thread get its result,
  out of memory (ENOMEM; `brk` returns the old break), counted (`process outgrew the ceiling`)
  and warned. The ceiling is given at start (`Start::ceiling`, at most the address space's end);
  with none below the end it never fires.
- The container's address width stays the executor's address space: a fixed request above the
  width fails as before (`address above the address space`). Until low-first placement lands, the
  stack's top and the mmap base are put at the ceiling, so that what is placed stays below it.
- `on_memory_accessing`: a thread is about to access a page the embedder watches (below). If the
  watch is gone when the hook returns the access goes on; otherwise the thread waits until the
  process's watches change, then touches again (and the hook fires again if it is still watched).

### The kernel's guarantees

- A hook fires only when every operation on its process is legal: the process is whole, and nothing
  the hook can do on it conflicts with what the kernel is doing.
- Hooks never nest. A hook is handed only a `Process`, which holds no way back to the embedder, so
  nothing done through it can announce another hook; the test host checks this by running every
  operation of the handle inside every hook.
- The kernel touches a program's memory before a system call has any effect, so a watch fires while
  the call can still be repeated (see "Watches and system calls").
- A hook is synchronous; an embedder that works asynchronously returns and finishes later through
  `Kernel::process` between steps.

## The process handle

```rust
pub struct Process<'a> { /* the kernel, the executor, one process */ }

impl Process<'_> {
    pub fn id(&self) -> TaskId;
    pub fn info(&self) -> ProcessInfo<'_>;
    pub fn execution_config(&self) -> ExecutionConfig;
    pub fn memory_read(&mut self, range: Range<u64>) -> Result<Vec<u8>, MemoryError>;
    pub fn memory_write(&mut self, address: u64, bytes: &[u8]) -> Result<(), MemoryError>;
    pub fn memory_watch(&mut self, range: Range<u64>, watch: Watch);
    pub fn memory_watch_thread(&mut self, thread: TaskId, range: Range<u64>, watch: Watch) -> Result<(), Unsupported>;
    pub fn memory_changed(&mut self) -> Vec<Range<u64>>;
}

impl Kernel {
    /// Between steps: the process an id names, unless it has ended.
    pub fn process<'a>(&'a mut self, cpu: &'a mut dyn Cpu, id: TaskId) -> Option<Process<'a>>;
}

pub struct ProcessInfo<'a> { /* borrowed records */ }

impl ProcessInfo<'_> {
    pub fn memory_mapped(&self) -> u64;
    pub fn ceiling_distance(&self) -> u64;
    pub fn compute_used(&self) -> Duration;
    pub fn parent(&self) -> Option<TaskId>;
}

pub enum Watch { None, Writes, All }
pub enum Access { Read, Write }
pub enum MemoryError { NotMapped, NotRead, SharedNotWritable, InvalidRange, Executor(CpuError) }
```

- Hooks receive the handle by value (the owner's agreement of the night of 2026-11-18), so a later
  `suspend(self)` can consume it.
- `info` reads only: `memory_mapped` sums the mapped ranges; `ceiling_distance` is what new mappings
  can still take below the ceiling (the gaps between the heap's end and the mmap base);
  `compute_used` is the CPU time of the live threads plus the ended ones; `parent` is the
  process's parent as `getppid` reports it.
- `memory_read` and `memory_write` work on mapped ranges whatever their private pages allow; a
  page of a backend file not read yet is `NotRead`. A write makes a private file page the
  process's own copy first, and is not counted as written by the process. A shared page the
  process may not write is refused (`SharedNotWritable`), as Linux's forced writes (ptrace,
  `/proc/<pid>/mem`) refuse it: others share it. A range that ends before it starts, or bytes that
  would run past the last address, are `InvalidRange`; `memory_watch` acts on no page for them.
- `memory_watch` is a setting: idempotent, nothing returned. It acts on the mapped pages
  (`PAGE_SIZE`) the range touches. A watch belongs to the page's bytes: unmapping a page ends it,
  as does mapping something new over it; `mremap` takes it along to where the page moves; a
  fork's child has its parent's watches, as it has its memory, except on pages it gets fresh
  (`MADV_WIPEONFORK`); `execve` ends them all with the old memory. Changing a page's attributes
  (`mprotect`, `pkey_mprotect`, `mlock`, `mlock2`, `munlock`, `madvise`'s fork advice) keeps its
  watch and its place in the record of written pages. `Writes` watches writes; `All` watches reads, writes and running code (running
  code is reported as `Read`).
- `memory_changed` answers the pages whose bytes the process changed since it was last asked, as
  ranges: what its threads wrote, what the kernel's copies wrote for it, pages `mremap` moved (at
  their new place, as Linux's soft-dirty bits mark them) and pages `madvise` put back to zero or
  to their file (`MADV_DONTNEED`, `MADV_REMOVE`, and the old pages of `MREMAP_DONTUNMAP`). A new
  mapping is not in it until written. The first answer is every page mapped writable, since
  nothing was recorded before it; from then on the record costs one fault per page per interval. A
  fork's child starts with no record.
- `memory_watch_thread` is declared for executors that can watch one thread and not its siblings
  (the abi's `PerThreadWatch` subtrait of `Cpu`); no executor implements it today, and the kernel
  reaches the executor as `dyn Cpu`, so the call is refused by name (`Unsupported::PerThreadWatch`).

### Watches and system calls

Before a call runs, the kernel walks the memory its arguments say it will touch
(crates/wcontainers-kernel/src/syscall/touch.rs) and meets every watch on it, announcing
`on_memory_accessing` for each watched page in order; what names more memory (an iovec array, a
msghdr or mmsghdr array, a string pointer array, a futex_waitv array, a robust list) is met
before it is read. If a watch stays, the thread waits and the call runs again from the start once
the watches change: nothing of it has happened. If a hook raises a watch while the walk goes on
(sets one, or widens one to `All`), the walk starts again from its beginning once it reaches its
end, since the page may lie behind it; so does a signal's walk. Only a walk that meets no raised
watch lets the call act. Every built call that touches program memory is
described: the reads and writes of data, path and name strings (a page at a time to their zero),
`execve`'s argument and environment strings, socket addresses with their lengths, the
structures of the time, signal, scheduling, resource, capability, IPC and message queue calls,
`fcntl`'s locks and owners, `ioctl` by its request's encoded direction and size or, for the
terminal requests that predate the encoding, by their structure, `futex` by operation, `prctl`
and `seccomp` by option. Calls that change a page's bytes without copying are described too: a
move by `mremap` reads the pages it takes along (and with `MREMAP_DONTUNMAP` writes the old
ones, put back to zero), and `madvise`'s `MADV_DONTNEED`, `MADV_DONTNEED_LOCKED` and
`MADV_REMOVE` write theirs. A vfork parent going on after its child let go stores nothing more and
meets nothing.

The walk names what a call's arguments say it may touch, before it knows what it will: a read of
10 bytes into a 1 MiB buffer meets every watched page of the buffer as a write, and `nanosleep`'s
remaining time is met as a write though it is written only when the sleep is interrupted. An
embedder is told of every access that can happen, and of some that then do not.

The kernel's own stores are met the same way before what makes them acts:
- the ends of the threads a call or signal ends (`exit`; `exit_group` and a fatal signal, every
  thread; `execve`, every other thread): each thread's robust list (its head, its entries, and the
  lock words it holds), and the word `CLONE_CHILD_CLEARTID` named, while others still use the
  memory;
- `clone`'s `CLONE_PARENT_SETTID` and `CLONE_CHILD_SETTID` words (a child without `CLONE_VM`
  copies its parent's watches, so the parent's watch is the one met);
- a signal handler's frame on the stack it will run on, and its restartable sequence;
- a thread taken off the CPU in its restartable sequence: the area's sequence pointer and the
  sequence, before the abort is decided. If a watch stays the thread waits, and checks its sequence
  again when it goes on.

While the embedder watches anything, every copy a call makes is checked against what its
description named. A copy outside it is counted (`copy outside the call's description`, with the
call's number). If such a copy meets a watched page, the copy treats the page as not there (as it
treats a backend file's page not read yet), the hook is announced after the call, and a call that
failed for it runs again from its instruction: anything it did before the copy is done again
(counted: `watched memory met at a copy`). One that did part of its work first keeps its short
result (counted: `call short of watched memory`). Both are gaps in the descriptions, not a second
way of meeting watches; the tests run the shell scripts and 41 calls with watches and find none.

Left to the check, and why:
- `semctl`'s `GETALL` and `SETALL` (their size is the set's), and the `*_INFO` commands of the IPC
  control calls: each copies before it acts or acts on nothing;
- `restart_syscall`, whose addresses are in the thread's restart record, not its arguments;
- a priority-inheriting lock handed on at a thread's end, whose word is not on the robust list;
- memory of another process (`process_vm_readv`, `process_vm_writev`, `/proc/<pid>/mem`): its
  watches make the copy stop short there, with no hook (looking inside another process is the
  inspection the owner has not built yet);
- the stores of thread ends when the container ends and the kernel kills what is left.

A program's own access to a watched page faults; the fault's span (an access reaches at most 64
bytes) is checked only when the CPU side allows the access all of the faulting page, and the hook
fires before the instruction is run again.

A wake made between steps (a watch met through `Kernel::process`) makes `Kernel::next` answer
`Next::Idle` with a time of 0, so the driver's `Event::Timer` lets the kernel settle the woken
threads before anything runs: an embedder that resolves watches between steps must answer that
idle at once.

What watching costs: while no process has a watch or a record of written pages, a call, a
signal or a fault pays one flag test (`Kernel::watching`), recomputed when watches are set, a
program starts, or a process ends. Once some process has one, every other process's call and
signal also looks up whether its own process has watched pages (its thread's and its process's
entries, and a test); only a process with watched pages walks its calls' descriptions and has
its copies checked. Page permissions are told to the CPU side again only for a process with a
watch or a record (a move by `mremap` included).

## Running a process

```rust
pub enum ThreadOrdering { Serial, Parallel }
pub enum ReplayDeterminism { Exact, External, None }

pub struct ExecutionConfig {
    pub thread_ordering: ThreadOrdering,
    pub replay_determinism: ReplayDeterminism,
}

impl ExecutionConfig {
    pub fn supported(&self) -> Result<(), Unsupported>;
}

pub enum Unsupported {
    ThreadOrdering(ThreadOrdering),
    ReplayDeterminism(ReplayDeterminism),
    PerThreadWatch,
}
```

- The first process's configuration is `Start::execution`; `Kernel::start` refuses anything but
  `(Serial, Exact)` by name before it does anything, and a fork's child keeps its parent's.
  `Process::execution_config` reads it. There is no setter (the owner is ruling on whether the
  ordering changes by a setter or only by `resume`).
- `translator: E::TranslatorConfig` is not a field: the kernel reaches its executor as `dyn Cpu`,
  with no executor type to name it from.

## Not built, and where it attaches

- `suspend(self) -> SuspendedProcess<E>`, `resume`, `export` and `import`: `suspend` will be a
  method of `Process` consuming the handle (hooks already receive it by value); `resume` a method
  of `Kernel` taking the `SuspendedProcess` and an `ExecutionConfig`. The process's records are the
  kernel's `Process` and `Thread` entries and its `AddressMap` (watches included); its memory is the
  executor's `MemoryId`.
- Which processes share memory, and shared pages marked watched when one of them leaves (ruled
  late on 2026-11-18): leaving is `suspend`, so it comes with it. The report will be a
  `ProcessInfo` accessor read from the `AddressMap`s' shared mappings (`Mapping::shared` and the
  file node they map); the marking will set `Watch::All` on those pages in every process still
  sharing them, from `suspend`.
- `set_thread_ordering`, the inspection hooks and `StopReason`: not ruled or not asked for yet.
