# The kernel's interface to its embedder

What the streams track built for S2, from the owner's rulings in
orchestrator/next-embedder-interface.md (its last sections are the specification). The stream
ends of S1 are in docs/streams.md. Where this document and the rulings differ, the differences are
listed under "Departures" in docs/status-streams.md.

## The embedder's trait

The kernel reaches its embedder through `wcontainers_kernel::System`, which extends the abi's
`Host` (clocks, randomness, warnings, the backend, connections, stream ends). Everything below but
`free_task_id` has a provided default, so an embedder that implements only that gets a counter for
task ids, every system call run as made, and no hooks.

```rust
pub trait System: Host + Sync {
    // Services: the kernel asks and uses the answer.
    fn allocate_task_id(&self, previous: TaskId) -> TaskId { task_id_after(previous) }
    fn free_task_id(&self, id: TaskId);

    // The system call proxy.
    fn intercept_system_call_request(&self, process: Process<'_>, thread: TaskId, request: SystemCallRequest)
        -> ControlFlow<SystemCallResponse, SystemCallRequest> { ControlFlow::Continue(request) }
    fn intercept_system_call_response(&self, process: Process<'_>, thread: TaskId, request: &SystemCallRequest,
        response: SystemCallResponse) -> SystemCallResponse { response }

    // Hooks: the kernel announces; synchronous, nothing returned.
    fn on_process_starting(&self, process: Process<'_>) {}
    fn on_program_starting(&self, process: Process<'_>) {}
    fn on_thread_starting(&self, process: Process<'_>, thread: TaskId) {}
    fn on_process_exiting(&self, process: Process<'_>) {}
    fn on_process_outgrown(&self, process: Process<'_>, bytes: u64) {}
    fn on_watched_memory_accessing(&self, process: Process<'_>, thread: TaskId, address: u64, kind: AccessKind) {}
    fn on_memory_written_by_kernel(&self, process: Process<'_>, thread: TaskId, range: Range<u64>) {}
}

/// A process or thread id: one space for both, as Linux's `pid_t`.
pub struct TaskId(pub u32);
pub fn task_id_after(previous: TaskId) -> TaskId; // the default counter, wrapping as `alloc_pid`

// In the abi.
pub const SYSTEM_CALL_ARGUMENT_COUNT: usize = 6; // man 2 syscall: x0 to x5 on arm64
pub struct SystemCallRequest { pub number: u64, pub arguments: [u64; SYSTEM_CALL_ARGUMENT_COUNT] }
// In the kernel.
pub type SystemCallResponse = Result<u64, Errno>;
```

Every method of `System` and of the abi's `Host` takes `&self`, and `System` requires `Sync`, so
that the kernel may later call hooks for different processes at the same time (the owner's ruling
of 2026-11-19); the embedder synchronizes what it changes (atomics, or its own locks; the test host
keeps its state and the test's hooks each behind a `Mutex`). Kernel calls that take the embedder
take `&dyn System`: `Kernel::start`, `exit`, `event`, `kill`, `restore`.

### Task ids

- `allocate_task_id` is asked for every new process and thread number, the container's first
  process included. `previous` is the number handed out last, which the default counter needs
  (the owner's names have no argument; listed for the owner). The kernel skips a number something
  still carries (as `alloc_pid` skips used ones) and asks again, counted (`task id still
  carried`); after as many tries as there are numbers, fork and clone fail with EAGAIN, as Linux's
  do when every number is taken. A number outside 1 to `pid_max` fails the fork or clone with
  EAGAIN at once, counted (`task id out of range`).
- `free_task_id` is called when no process, thread, process group or session carries a number any
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
  (`free_task_id`). When the container's first process ends it is announced first; then every
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
  and warned. The ceiling is given at start (`Start::ceiling`, at most the address space's end,
  any size); with none below the end it never fires. An `execve` checks the whole new program
  (its segments, its interpreter and the vDSO, where the loader will put them) against the ceiling
  before the old program goes, so a program that does not fit fails the call with ENOMEM and the
  old one carries on. The container's first program is the one exception to the hook: its process
  is announced only once its program is loaded, so a first program that does not fit is counted
  and warned, and the start fails with ENOMEM (`Kernel::start_failure`) with no hook.
- The container's address width stays the executor's address space: a fixed request above the
  width fails as before (`address above the address space`). Until low-first placement lands, the
  stack's top is put at the ceiling and the mmap base below it by Linux's gap (`mmap_base`: 128
  MiB, at most five sixths of the top), so that what is placed stays below the ceiling.
- `on_watched_memory_accessing`: a thread, or the kernel for it, is about to access a page the
  embedder watches (below), as `kind` (an instruction fetch is `Execute`). If the watch is gone
  when the hook returns the access goes on; otherwise the thread waits until the process's
  watches change, then touches again (and the hook fires again if it is still watched).
- `on_memory_written_by_kernel`: after the kernel wrote `range` of a process's memory for
  `thread`: the data a call copies out, a signal's frame, restartable sequence stores, clone's
  `CLONE_PARENT_SETTID` and `CLONE_CHILD_SETTID` words, the word `CLONE_CHILD_CLEARTID` names and
  the robust list's lock words at a thread's end. Every copy into program memory goes through one
  routine (crates/wcontainers-kernel/src/user.rs `Guest`), which notes the exact range written
  (adjacent ranges for the same thread joined); the kernel announces them, in order, before the
  next hook of any kind and before any thread runs again, to the process using that memory (the
  one it was written for, or, if that one has left it by `execve`, another process still sharing
  it; none if no process uses it any more). Not announced, and why: memory a mapping call
  changes without copying (mmap, munmap, `madvise`, `mremap`'s move, `MADV_WIPEONFORK` at fork),
  which `take_dirty_memory` reports; a new program's stack, vDSO and segment tails, written by
  `execve` into the new memory before `on_program_starting`, where the whole new memory is the
  embedder's to read; pages of a mapped backend file filled from the backend, which are written
  into the file every process mapping it shares, as another process's `write` to a shared file
  is. The embedder's own `write_memory` fires neither this hook nor a watch.

### The system call proxy

- `intercept_system_call_request` is asked once for every system call a thread makes, when the
  thread comes back with it and before anything of it happens (before the watch walk, `seccomp`
  and the call). `Continue(request)` runs `request`: when it differs from what the thread made, the
  thread's registers (x8 and x0 to x5) are changed to it, as a tracer's change at Linux's
  syscall-entry stop changes them, so a call that waits and runs again runs the changed one.
  `Break(response)` answers the call without running it; the thread sees the response.
- `intercept_system_call_response` is asked when a call the kernel ran answers, before the thread
  sees it; the thread sees what it returns. A call that waits answers once, when it finishes; a
  call that never returns (`exit`, `exit_group`) does not answer, and one that restarts from its
  instruction is asked again as a new call. A call that replaces its thread's registers answers
  with the new x0, and what the embedder returns goes into x0, as at Linux's syscall-exit stop: a
  successful `execve` answers 0 in the new program, after `on_program_starting`, to the thread
  that carries the process's number by then; `rt_sigreturn` answers the x0 it put back. The answer
  is given as it reaches the thread's signal handling, internal restart codes (`ERESTARTSYS`)
  included, as at Linux's syscall-exit stop.
- With neither implemented, each call costs two calls of empty provided methods through the
  trait object, and every copy into program memory notes its range for
  `on_memory_written_by_kernel`, announced through an empty provided method. Measured on the memory
  test binary: no difference beyond noise (the seventh check's six alternating rounds, docs/status-streams.md,
  "Measurements"). A kernel generic over `System` would remove these calls at compile time;
  the kernel takes `&dyn System` today.

### The kernel's guarantees

- A hook fires only when every operation on its process is legal: the process is whole, and nothing
  the hook can do on it conflicts with what the kernel is doing.
- Hooks never nest. A hook is handed only a `Process`, which holds no way back to the embedder, so
  nothing done through it can announce another hook; the test host checks this by running every
  operation of the handle inside every hook, and reports a hook called while another runs rather
  than waiting on its own lock.
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
    pub fn read_memory(&mut self, range: Range<u64>) -> Result<Vec<u8>, MemoryError>;
    pub fn write_memory(&mut self, address: u64, bytes: &[u8]) -> Result<(), MemoryError>;
    pub fn watch_memory(&mut self, range: Range<u64>, watches: WatchesFor);
    pub fn watch_thread_memory(&mut self, thread: TaskId, range: Range<u64>, watches: WatchesFor) -> Result<(), Unsupported>;
    pub fn take_dirty_memory(&mut self) -> Vec<Range<u64>>;
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

pub enum WatchesFor { None, Writes, All }
pub enum AccessKind { Read, Write, Execute }
pub enum MemoryError { NotMapped, NotRead, SharedNotWritable, InvalidRange, Executor(CpuError) }
```

- Hooks receive the handle by value (the owner's agreement of the night of 2026-11-18), so a later
  `suspend(self)` can consume it.
- `info` reads only: `memory_mapped` sums the mapped ranges; `ceiling_distance` is what new mappings
  can still take below the ceiling (the gaps between the heap's end and the mmap base);
  `compute_used` is the CPU time of the live threads plus the ended ones; `parent` is the
  process's parent as `getppid` reports it.
- `read_memory` and `write_memory` work on mapped ranges whatever their private pages allow; a
  page of a backend file not read yet is `NotRead`. A write makes a private file page the
  process's own copy first, and is not counted as written by the process. A shared page the
  process may not write is refused (`SharedNotWritable`), as Linux's forced writes (ptrace,
  `/proc/<pid>/mem`) refuse it: others share it. A range that ends before it starts, or bytes that
  would run past the last address, are `InvalidRange`; `watch_memory` acts on no page for them.
- `watch_memory` is a setting: idempotent, nothing returned. It acts on the mapped pages
  (`PAGE_SIZE`) the range touches, and costs time in proportion to them, not to the range: ending
  watches visits only the watched pages, setting them only the mapped ones. A watch belongs to the page's bytes: unmapping a page ends it,
  as does mapping something new over it; `mremap` takes it along to where the page moves; a
  fork's child has its parent's watches, as it has its memory, except on pages it gets fresh
  (`MADV_WIPEONFORK`); `execve` ends them all with the old memory. Changing a page's attributes
  (`mprotect`, `pkey_mprotect`, `mlock`, `mlock2`, `munlock`, `madvise`'s fork advice) keeps its
  watch and its place in the record of written pages. `Writes` watches writes; `All` watches reads, writes and
  instruction fetches (`Execute`).
- `take_dirty_memory` answers the pages whose bytes the process changed since it was last asked, as
  ranges: what its threads wrote, what the kernel's copies wrote for it, pages `mremap` moved (at
  their new place, as Linux's soft-dirty bits mark them) and pages `madvise` put back to zero or
  to their file (`MADV_DONTNEED`, `MADV_REMOVE`, and the old pages of `MREMAP_DONTUNMAP`). A new
  mapping (mmap, also over pages that were mapped, heap and stack growth, a mapping grown by
  `mremap`) is in it whatever it allows, as Linux marks a new mapping soft-dirty. The embedder's
  own `write_memory` is not. The first answer is every page mapped writable, since nothing was
  recorded before it; from then on the record costs one fault per page per interval. It is kept as
  ranges, so a large new mapping costs no more than a small one. A fork's child starts with no
  record.
- `watch_thread_memory` is declared for executors that can watch one thread and not its siblings
  (the abi's `PerThreadWatch` subtrait of `Cpu`); no executor implements it today, and the kernel
  reaches the executor as `dyn Cpu`, so the call is refused by name (`Unsupported::PerThreadWatch`).

### Watches and system calls

Before a call runs, the kernel walks the memory its arguments say it will touch
(crates/wcontainers-kernel/src/syscall/touch.rs) and meets every watch on it, announcing
`on_watched_memory_accessing` for each watched page in order; what names more memory (an iovec array, a
msghdr or mmsghdr array, a string pointer array, a futex_waitv array, a robust list) is met
before it is read, and what a call writes back into what it read (a received message's lengths and
flags, `sched_setattr`'s size and the capability calls' version when they refuse them, a socket
address's length) is met as a write. If a watch stays, the thread waits and the call runs again from the start once
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

A fatal signal (`SIGKILL`, or one whose default action ends the process and that has no handler)
sent to a process whose memory is watched, by another process or by a call of its own, is left to
the process's own threads, as Linux's threads end themselves on their way back to the program
(`SIGNAL_GROUP_EXIT`): each thread is woken from any wait but a watch's (a stopped process
included), and the first to go back meets the watches on what the ends of all its threads store,
then ends the process; until then none of them runs its program or a call again, and one waiting
on a watch acts on the signal once the watches change. The process's end and its hooks follow
from there, in its own thread. A process with no watched memory ends at once, in the sender's
call, as before.

While the embedder watches anything, every copy a call makes is checked against what its
description named: a read against anything named, a write against what was named as written
(a range met as a read meets only `All` watches). A copy outside it is counted (`copy outside the call's description`, with the
call's number). If such a copy meets a watched page, the copy treats the page as not there (as it
treats a backend file's page not read yet), the hook is announced after the call, and a call that
failed for it runs again from its instruction: anything it did before the copy is done again
(counted: `watched memory met at a copy`). One that did part of its work first keeps its short
result (counted: `call short of watched memory`). Both are gaps in the descriptions, not a second
way of meeting watches; the tests run the shell scripts and 46 calls with watches and find none.

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

What watching costs: while no process has a watch or a record of written pages, a call or a
fault pays one flag test (`Kernel::watching`), recomputed when watches are set, a program starts,
or a process ends, and a thread's way back to its program one more (whether a fatal signal waits
on watches anywhere). Every copy a call makes, watched or not, also takes its address map's
watches and finds no description and no watched page (two empty tests). Once some process has one, every other process's call and
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
- Which processes share memory (ruled on the evening of 2026-11-19, replacing the pairwise report
  and `memory_sharers()`): two hooks on `System`, `on_shared_memory_mapping` and
  `on_shared_memory_unmapped(process, source: MemoryObject, offset, range)`, with
  `MemoryObject::{File { device: DeviceId, inode: InodeNumber }, SystemVSegment(SystemVSegmentId),
  Anonymous(AnonymousMemoryId)}` and the four branded ids in the kernel's interface. They will fire
  per process where the `AddressMap` gains or loses a shared mapping (`Mapping::shared` and the
  node it maps): mmap, shmat, the maps a fork's child inherits, and munmap, exit and exec. The
  `WorkloadPlacement` ruling of the same evening also needs a getter for a process's memory id on
  `Process` (the executor's `MemoryId`, which holds across vfork and exec). Shared pages marked
  watched when one sharer leaves come with `suspend`: the marking will set `WatchesFor::All` on
  those pages in every process still sharing them.
- `type ProcessState` (ruled; to wait for the reviewable-rust layout), signals as an input
  (`on_signal_delivering`, `Process::send_signal`, ruled at 17:10 on 2026-11-19).
- `set_thread_ordering`, the inspection hooks and `StopReason`: not ruled or not asked for yet.
