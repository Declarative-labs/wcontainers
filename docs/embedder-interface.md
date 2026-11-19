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
pub trait System: Host {
    // Services: the kernel asks and uses the answer.
    fn task_id_allocate(&mut self, previous: TaskId) -> TaskId { task_id_after(previous) }
    fn task_id_free(&mut self, id: TaskId) {}

    // Hooks: the kernel announces; synchronous, nothing returned.
    fn on_process_starting(&mut self, process: Process<'_>) {}
    fn on_program_starting(&mut self, process: Process<'_>) {}
    fn on_thread_starting(&mut self, process: Process<'_>, thread: TaskId) {}
    fn on_process_exiting(&mut self, process: Process<'_>) {}
    fn on_process_outgrown(&mut self, process: Process<'_>, bytes: u64) {}
    fn on_memory_accessing(&mut self, process: Process<'_>, thread: TaskId, address: u64, access: Access) {}
}

/// A process or thread id: one space for both, as Linux's `pid_t`.
pub struct TaskId(pub u32);
pub fn task_id_after(previous: TaskId) -> TaskId; // the default counter, wrapping as `alloc_pid`
```

Kernel calls that take the embedder take `&mut dyn System`: `Kernel::start`, `exit`, `event`,
`kill`, `restore`.

### Task ids

- `task_id_allocate` is asked for every new process and thread number, the container's first
  process included. `previous` is the number handed out last, for a counter. The kernel skips a
  number something still carries (as `alloc_pid` skips used ones) and asks again; after as many
  tries as there are numbers, fork and clone fail with EAGAIN, as Linux's do when every number is
  taken.
- `task_id_free` is called when no process, thread, process group or session carries a number any
  more: a thread that ended, a process reaped (or one whose parent did not want it), the numbers a
  process leaves behind by `setpgid` or `setsid`, the old number of a thread that took its
  process's number by `execve`. It is called when the kernel settles, after the call that let go.
- Because the embedder hands out the numbers and hears when each is free, a number it keeps never
  names another process without its knowing; no generation is carried.

### When each hook fires

- `on_process_starting`: a process made by fork, vfork or clone without `CLONE_THREAD`, at the end
  of the clone call that made it, before the child's first instruction (in the parent's call, after
  the child's memory, registers and descriptors exist, before the parent's `CLONE_PARENT_SETTID`
  store). Also the container's first process, once its program is loaded, just before
  `on_program_starting`.
- `on_program_starting`: at the end of a successful `execve` (and the container's start), once the
  new program is loaded, its stack written and its registers set, before its first instruction.
  The old program is gone by then.
- `on_thread_starting`: a clone with `CLONE_THREAD`, at the end of the clone call, before the new
  thread's first instruction; `thread` is its number.
- `on_process_exiting`: when a process is about to be torn down (`exit_group`, the last thread's
  exit, a fatal signal), before any of its threads or memory go: its memory and records are still
  readable. The number stays carried by the zombie until it is reaped (`task_id_free`).
- `on_process_outgrown`: a request for memory (mmap, mremap, or the loader of a new program, which
  works on the new program's map before the old one is destroyed) found no room below the
  executor's ceiling. It fires after the request's placement failed and before the call returns,
  with the bytes asked for; when it returns the request fails as out of memory (ENOMEM), counted
  (`process outgrew the ceiling`) and warned. The ceiling is given at start
  (`Start::ceiling`, at most the address space's end); with none below the end it never fires.
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
pub enum MemoryError { NotMapped, NotRead, Executor(CpuError) }
```

- Hooks receive the handle by value (the owner's agreement of the night of 2026-11-18), so a later
  `suspend(self)` can consume it.
- `info` reads only: `memory_mapped` sums the mapped ranges; `ceiling_distance` is what new mappings
  can still take below the ceiling (the gaps between the heap's end and the mmap base);
  `compute_used` is the CPU time of the live threads plus the ended ones; `parent` is the
  process's parent as `getppid` reports it.
- `memory_read` and `memory_write` work on mapped ranges whatever their pages allow; a page of a
  backend file not read yet is `NotRead`. A write makes a private file page the process's own copy
  first, and is not counted as written by the process.
- `memory_watch` is a setting: idempotent, nothing returned. It acts on the mapped pages
  (`PAGE_SIZE`) the range touches; unmapping a page ends its watch; a fork's child has its parent's
  watches, as it has its memory; `execve` ends them all with the old memory. `Writes` watches
  writes; `All` watches reads, writes and running code (running code is reported as `Read`).
- `memory_changed` answers the pages the process wrote (its threads, and the kernel's copies for
  it) since it was last asked, as ranges. The first answer is every page mapped writable, since
  nothing was recorded before it; from then on the record costs one fault per page per interval. A
  fork's child starts with no record.
- `memory_watch_thread` is declared for executors that can watch one thread and not its siblings
  (the abi's `PerThreadWatch` subtrait of `Cpu`); no executor implements it today, and the kernel
  reaches the executor as `dyn Cpu`, so the call is refused by name (`Unsupported::PerThreadWatch`).

### Watches and system calls

Before a call runs, the kernel walks the memory its arguments say it will touch
(crates/wcontainers-kernel/src/syscall/touch.rs) and meets every watch on it, announcing
`on_memory_accessing` for each watched page in order; structures that name more memory (an iovec
array, a msghdr) are met before they are read. If a watch stays, the thread waits and the call runs
again from the start once the watches change: nothing of it has happened. The calls described:
read, pread64, readv, preadv, preadv2, write, pwrite64, writev, pwritev, pwritev2, recvfrom,
sendto, recvmsg, sendmsg, accept, accept4, getdents64, getrandom, getcwd, readlinkat, wait4,
waitid, pipe2, socketpair, the parent's stores of clone, clock_gettime, clock_getres,
gettimeofday, nanosleep, clock_nanosleep, fstat, newfstatat, statx, uname, sysinfo, getrusage,
prlimit64, rt_sigaction, rt_sigprocmask, epoll_pwait, epoll_pwait2, ppoll and rt_sigreturn.

A call not described meets a watch at its copy: the kernel's copy treats the watched page as not
there yet (as it treats a backend file's page not read yet), the call fails as Linux's does at a
fault, the hook is announced after it, and the call runs again from its instruction (counted:
`watched memory met at a copy`). One that did part of its work first keeps its short result
(counted: `call short of watched memory`).

A signal's frame is met before the signal is taken: the frame's range on the stack the handler will
run on is walked like a call's; if a watch stays, the thread waits with the signal still pending
and acts on its signals again when the watches change.

A program's own access to a watched page faults; the fault's span (an access reaches at most 64
bytes) is checked, and the hook fires before the instruction is run again.

A wake made between steps (a watch met through `Kernel::process`) makes `Kernel::next` answer
`Next::Idle` with a time of 0, so the driver's `Event::Timer` lets the kernel settle the woken
threads before anything runs.

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
- `set_thread_ordering`, the inspection hooks and `StopReason`: not ruled or not asked for yet.
