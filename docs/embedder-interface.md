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
    fn on_signal_delivering(&self, process: Process<'_>, thread: TaskId, signal: &SignalInformation) {}
    fn on_shared_memory_mapping(&self, process: Process<'_>, source: MemoryObject, offset: u64, range: Range<u64>) {}
    fn on_shared_memory_unmapped(&self, process: Process<'_>, source: MemoryObject, offset: u64, range: Range<u64>) {}
}

pub struct DeviceId(pub u64);
pub struct InodeNumber(pub u64);
pub struct SystemVSegmentId(pub i32);
pub struct AnonymousMemoryId(pub u64);
pub enum MemoryObject {
    File { device: DeviceId, inode: InodeNumber },
    SystemVSegment(SystemVSegmentId),
    Anonymous(AnonymousMemoryId),
}

/// Linux's own record of a signal (the generated binding of `struct siginfo`).
pub type SignalInformation = wcontainers_kernel::linux::general::siginfo;

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
  embedder watches (below), as `kind` (an instruction fetch is `Execute`). `process` is the
  process whose memory it is and `thread` the thread whose access it is: a thread of that
  process, or, when a call reads another process's memory through `/proc/<pid>/cmdline` (open
  to anyone) or `/proc/<pid>/environ` (open to whoever may inspect the process, Linux's
  `PTRACE_MODE_READ` check), both copied as Linux's `access_remote_vm` copies them, the reading
  thread of another process. If the watch is gone when the hook returns the access goes on;
  otherwise the thread waits until that process's watches change, or it ends or runs a program,
  then touches again (and the hook fires again if it is still watched).
- `on_memory_written_by_kernel`: after the kernel wrote `range` of a process's memory for
  `thread`: the data a call copies out, a signal's frame, restartable sequence stores, clone's
  `CLONE_PARENT_SETTID` and `CLONE_CHILD_SETTID` words, the word `CLONE_CHILD_CLEARTID` names and
  the robust list's lock words at a thread's end and at `execve` (in the memory the program
  leaves, as Linux's `exec_mm_release`). Every copy into program memory goes through one
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

- `on_signal_delivering`: a thread has taken a signal off its pending sets (its own, then its
  process's) to act on it, before anything of the action: before a handler's frame is written,
  before a default action ends, stops or ignores, and before an ignoring handler drops it. This
  is the point of Linux's signal-delivery-stop under ptrace (`get_signal` right after
  `dequeue_signal`). It is on the thread's way back to its program: after the call it was in has
  answered (after `intercept_system_call_response`, and before a call interrupted by a handler is
  set to restart or answer EINTR), or after the CPU side stopped it between instructions (a
  fault, the end of its time). So for a signal taken at a system call, the call is the position.
  `signal` is the record the handler will be given. Signals that are not acted on by a thread are
  not announced: one taken by `sigwaitinfo`, `sigtimedwait` or a read of a `signalfd` (the call's
  result), one dropped when sent because it is ignored, and one that ends its process from the
  sender's side (`SIGKILL`, or a fatal default action with no handler; the end is announced by
  `on_process_exiting`). If the embedder watches what acting on the signal touches (the frame,
  the restartable sequence, the stores of the threads' ends), the hook fires first and the walk
  over those watches after it; a thread that then waits on a watch keeps the signal it took and
  acts on it, with no second announcement, once the watches change.

- `on_shared_memory_mapping` and `on_shared_memory_unmapped`: what each process maps of shared
  memory, per process (a mapping belongs to a process's whole memory): `range` is where it sits
  in the process's memory and `offset` where in `source` that range starts. `source` is a file
  by its device and inode (also a memory file and POSIX shared memory), a System V segment by its
  id (`shmget`'s answer, the inode `/proc/<pid>/maps` shows for it), or shared anonymous memory
  (`MAP_SHARED | MAP_ANONYMOUS`, and a shared mapping of `/dev/zero`) by the kernel's own number
  for it. They describe a change of what a process maps, not a call: a mapping starts with mmap
  (`MAP_SHARED`), shmat, `mremap` (at its new place, or a mapping grown), and fork (in the
  child, every shared mapping it inherits but those `MADV_DONTFORK` keeps out) or a clone with
  `CLONE_VM` (in the new process, the memory it shares); one ends with munmap (only the part
  taken away), `mremap` (at the old place), a mapping put over it (`MAP_FIXED`, shmat with
  `SHM_REMAP`), shmdt, a new program (every one, in the process that runs it), and the process's
  end. Pieces of one mapping that `mprotect`, `mlock` or `madvise` split are still one piece of
  the source, and nothing is announced for them; a call that maps the same part of the same
  source at the same place again announces nothing either. Every end names exactly a range
  announced before with the same source and offset, ends first: a change that ends one mapping and
  starts another announces the ending first.
- Only sources some process may write through a mapping are announced: a shared mapping whose
  file was opened for writing (Linux's `VM_MAYWRITE`, whatever `mprotect` allows now). When a
  source that no mapping could write through gets one that can, every process already mapping
  it is announced then, before the new mapping, in each process's own hook. A source once
  announced stays announced while any process maps it, even if the last mapping that could
  write it ends; each mapping announced is announced ended.
- When: a change made by a call is announced before the call's answer reaches the proxy
  (`intercept_system_call_response`), and before any other hook; a fork's or clone's child is
  announced its mappings just after `on_process_starting`; a new program's ends come before
  `on_program_starting`; a process's end, just after `on_process_exiting`, while it is whole.
- What it costs: an address map notes that a shared mapping went in or out, and a process
  being made, running a new program or ending notes it when its memory has one; only then does
  the kernel, before its next hook or before any thread runs, walk every live process's
  mappings and compare what they map with what it announced. With no shared mapping anywhere
  nothing is walked.
- Not announced (for the owner): a private file mapping, which shows the file's bytes as other
  processes write them until it writes a page itself. The ruling names `MAP_SHARED` mappings
  only.

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
    pub fn memory_id(&self) -> MemoryId;
    pub fn read_memory(&mut self, range: Range<u64>) -> Result<Vec<u8>, MemoryError>;
    pub fn write_memory(&mut self, address: u64, bytes: &[u8]) -> Result<(), MemoryError>;
    pub fn watch_memory(&mut self, range: Range<u64>, watches: WatchesFor);
    pub fn watch_thread_memory(&mut self, thread: TaskId, range: Range<u64>, watches: WatchesFor) -> Result<(), Unsupported>;
    pub fn take_dirty_memory(&mut self) -> Vec<Range<u64>>;
    pub fn send_signal(&mut self, thread: TaskId, signal: SignalInformation);
    pub fn suspend_tasks(&mut self, tasks: &[TaskId]) -> vec::IntoIter<(TaskId, SuspendedTask)>;
    pub fn suspend_process(self) -> (ProcessContext, vec::IntoIter<(TaskId, SuspendedTask)>);
    pub fn copy_process_context(&self) -> ProcessContext;
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

- Hooks receive the handle by value (the owner's agreement of the night of 2026-11-18), so
  `suspend_process(self)` consumes it.
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

- `memory_id` is the executor's `MemoryId` of the memory the process runs in: the same for
  processes that run in one memory (a vfork child until it lets go, a clone with `CLONE_VM`),
  and a new one when the process runs a new program, which replaces its memory.
- `send_signal` sends a signal to one thread of the process, as `rt_tgsigqueueinfo` would with
  `signal` as its record (the number is the record's `si_signo`; the sender is whoever the
  record names, for example none with `SI_QUEUE` for a signal from outside the container). It
  is sent when the kernel next settles: after the hook it was called in returns, before any
  thread runs again; between steps, `Kernel::next` then answers `Next::Idle` with a time of 0, as
  for a watch changed between steps. From there it is any other signal: a thread blocked in an
  interruptible call is woken and the call answers EINTR or restarts by its handler's
  `SA_RESTART`; a thread waiting on a watch acts on it once the watches change. A signal that
  names no thread of the process, has a number outside 1 to 64, or does not fit the receiving
  user's queue is not sent, counted (`signal from the embedder not sent`, with its number) and
  warned. The record must be built from zero, as Linux's `clear_siginfo` builds one: the kernel
  reads it whole, and in Rust the bytes of a union member never written are not readable.

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
`MADV_REMOVE` write theirs. `process_vm_readv` and `process_vm_writev` on a task of the
caller's own process meet the other side's segments too (read, or written), which are the
caller's memory. A read of `/proc/<pid>/cmdline` or `environ` (by `read`, `pread64`, `readv`,
`preadv`, `preadv2`, `sendfile` or `splice`) meets the strings it copies in the memory of the
process it names before the caller's buffer, whichever process that is. A vfork parent going on
after its child let go stores nothing more and meets nothing.

The walk names what a call's arguments say it may touch, before it knows what it will: a read of
10 bytes into a 1 MiB buffer meets every watched page of the buffer as a write, and `nanosleep`'s
remaining time is met as a write though it is written only when the sleep is interrupted. An
embedder is told of every access that can happen, and of some that then do not.

The kernel's own stores are met the same way before what makes them acts:
- the ends of the threads a call or signal ends (`exit`; `exit_group` and a fatal signal, every
  thread; `execve`, every other thread and then the caller's own release of the memory it leaves):
  each thread's robust list (its head, its entries, and the lock words it holds), and the word
  `CLONE_CHILD_CLEARTID` named, while others still use the memory (for `execve`'s caller, while
  another process does, as a vfork child's parent);
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
description named (a read of another process's memory through `/proc` against that process's):
a read against anything named, a write against what was named as written (a range met as a read
meets only `All` watches). A copy outside it is counted (`copy outside the call's description`,
with the call's number). If such a copy meets a watched page, the copy treats the page as not there (as it
treats a backend file's page not read yet), the hook is announced after the call, and a call that
failed for it runs again from its instruction: anything it did before the copy is done again
(counted: `watched memory met at a copy`). One that did part of its work first keeps its short
result (counted: `call short of watched memory`). Both are gaps in the descriptions, not a second
way of meeting watches; the tests run the shell scripts, 51 calls, the `process_vm` calls and
reads of a command line with watches and find none.

Left to the check, and why:
- `semctl`'s `GETALL` and `SETALL` (their size is the set's), and the `*_INFO` commands of the IPC
  control calls: each copies before it acts or acts on nothing;
- `restart_syscall`, whose addresses are in the thread's restart record, not its arguments;
- a priority-inheriting lock handed on at a thread's end or at `execve`, whose word is not on the
  robust list;
- the stores of thread ends when the container ends and the kernel kills what is left.

Not built, so nothing reaches another process's memory that way: `process_vm_readv` and
`process_vm_writev` on a task of another process are refused with ENOSYS and counted (`call on
another process not built`), and `/proc/<pid>/mem` does not exist.

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

## Suspending and resuming tasks

```rust
impl Kernel {
    pub fn suspend_tasks(&mut self, cpu: &mut dyn Cpu, tasks: &[TaskId]) -> vec::IntoIter<(TaskId, SuspendedTask)>;
    pub fn suspend_process(&mut self, cpu: &mut dyn Cpu, process: TaskId) -> Option<(ProcessContext, vec::IntoIter<(TaskId, SuspendedTask)>)>;
    pub fn copy_process_context(&self, process: TaskId) -> Option<ProcessContext>;
    pub fn resume_process(&mut self, context: ProcessContext, config: ExecutionConfig) -> Option<ProcessContext>;
    pub fn resume_tasks(&mut self, cpu: &mut dyn Cpu, tasks: impl IntoIterator<Item = SuspendedTask>) -> vec::IntoIter<SuspendedTask>;
}

pub struct SuspendedTask { /* the kernel's records */ }  // id(), process()
pub struct ProcessContext { /* the kernel's records */ } // id()
```

- The kernel's calls take the executor (`cpu`), as `Kernel::process` does: a task's registers live
  there, and a task put back gets a new executor thread in its process's memory. They are made
  between steps (no thread running); the handle's three are the same calls limited to its process.
  Resuming is outside hooks only. The iterators are the concrete `vec::IntoIter`, which borrows
  nothing of the kernel.
- A `SuspendedTask` holds the task's whole record (its id, its registers, its signal mask and
  pending signals, its call slot with what it waits on, its credentials, and its references to the
  descriptor table and the filesystem information, which Linux's tasks also reach by reference)
  and where it was in line. A `ProcessContext` holds the process's record: the memory (the
  executor's `MemoryId`) and its address map with the embedder's watches, the signal handlers, the
  limits, the process group and session, and what the embedder has been told the process maps of
  shared memory, which moves with it so nothing is announced again or ended by the move.
- `suspend_tasks` takes, as of one moment, every task named that is on the kernel; one not here (it
  ended, it is on no process of this kernel, or it is already taken) is absent from the answer.
  Today the kernel runs one thread at a time, so between steps every task is stopped at a point it
  can be taken from, and the batch is simply the tasks taken in turn with nothing running between
  them. When threads run at once, a batch naming a running task will have to wait for it to stop;
  inside a hook that wait could not end, and that batch is not built.
- Where a task goes on from: a task waiting for its turn (or holding it) goes back in line as it
  was, and a task with a signal to act on still acts on it first. A task taken in a call, parked in
  it or woken and not yet run, is treated exactly as Linux treats a signal with no handler arriving
  during that call (a stop, or the freezer's wake, `fake_signal_wake_up`): the call's next run, when
  the task is put back, finds a signal pending (`signal_pending`), and the call answers as Linux's
  code for it answers; with no handler, the restart codes then run the call again. A waiter in a
  line a wake chooses from leaves it when it is taken, so no wake chooses it while it is away: a
  futex wait (it answers as interrupted, not timed out), a System V semaphore operation (told EINTR
  at once) and a POSIX message queue send or receive. Every other call sees the interruption when
  the task is put back: nothing of the task runs in between, so this is Linux's stop arriving then.
  A task taken at `intercept_system_call_request` has not begun its call and makes it again (the
  request is announced again). Per call, from the restart codes in Linux's source and signal(7),
  "Interruption of system calls by signal handlers" and "by stop signals":
  - Run again from the start, having had no effect (`ERESTARTSYS`, `ERESTARTNOINTR`,
    `ERESTARTNOHAND` with no handler): a read or write that has moved nothing (pipes, sockets,
    terminals, eventfd, signalfd, inotify, splice), the open of a named pipe, `wait4` and
    `waitid`, `accept`, `connect`, `flock` and `F_SETLKW`, a futex wait with no timeout (it checks
    the number again, and answers EAGAIN if it changed) and the priority-inheritance lock, a message
    queue send or receive (their times are absolute), `pause`, `rt_sigsuspend`,
    `clock_nanosleep` with `TIMER_ABSTIME`.
  - Go on with the time that was left: `nanosleep` and a relative `clock_nanosleep`, and a futex
    wait with a timeout, through `restart_syscall` (`ERESTART_RESTARTBLOCK`, to the same deadline);
    `ppoll` and `pselect6` write the time left into their timeout and run again (`ERESTARTNOHAND`).
  - Answer what they already did: a read or write that has moved bytes answers the count, and
    `sendmmsg` and `recvmmsg` the messages done; a futex waiter a wake had already chosen answers
    0; a semaphore operation already done and a message already handed over answer as done.
  - Answer EINTR, as Linux does after a stop even with no handler: `epoll_wait` and
    `epoll_pwait`, `semop` and `semtimedop`, `rt_sigtimedwait`.
  The test `a_task_taken_in_its_call_answers_as_a_stop_on_the_reference_makes_it_answer`
  (stopped.c) takes a process's threads in a futex wait, a futex wait already woken, a sleep, a
  write half done, a semaphore operation, `epoll_wait` and a pipe read, changes and wakes the futex
  while they are away, and compares every answer with the reference, where the same program is
  stopped and continued with SIGSTOP and SIGCONT.
- Inside a hook the kernel is in the middle of a step for one thread (the system call, signal or
  fault it is handling), or of starting a program or ending a process. That thread, the process
  being started by an exec and every task of a process being ended cannot be taken there: they are
  left out of the answer and counted (`task in the middle of a kernel step not suspended`, detail
  the task). The one exception is `intercept_system_call_request`: nothing of the call has
  happened, so its thread is taken and makes the call again (the request is announced again) where
  it is put back. A new thread or process announced by `on_thread_starting` or
  `on_process_starting` after a clone can be taken: it has not run.
- `suspend_process` takes the context and every task of the process; the process is then gone from
  the kernel (`process` and `copy_process_context` answer none). Through the handle, when one of its
  tasks cannot be taken in that hook, the process stays: the answer is a copy of its context and the
  tasks that could be taken, and the rest are counted as above.
- `copy_process_context` stops nothing. The copy shares the records the kernel reaches by reference
  (the address map, the handler table) with the process it copies, as two kernel values in one
  embedder share the executor's memories; keeping copies in step on several kernels is the
  embedder's business. It holds no tasks.
- `resume_process` puts the context on the kernel with `config` (given here, never kept in the
  value: a fork's child takes its parent's as before). A context for a process already on the
  kernel changes nothing and is handed back; so is one with a configuration the kernel cannot run
  (see `ExecutionConfig::supported`).
- `resume_tasks` puts each task on its process: tasks whose process context is not on the kernel,
  whose number is already carried, or for which the executor refused a thread (counted as `host
  cannot do this` when it cannot at all) are handed back. Putting a task back ends nothing and
  announces nothing.
- What a task leaves on the kernel it came from while away: results already given to it (a
  semaphore operation done or told EINTR, a message handed over) stay there keyed by its number
  and are found when it comes back to the same kernel; a wake that reaches nobody there is not
  passed on. Locks across kernels (`wake_waiters`, `move_waiters`) are not built. A thread that
  ends while its siblings are away is, to the kernel, the last thread of its process and ends the
  process.
- A second kernel value in the same embedder can take a context and tasks from another, run on the
  same executor (the memory ids are the executor's); its clock and its tables (ends, futexes,
  System V objects, file systems) are its own. A process brought to a kernel whose parent is not
  on it is not yet given a reaper: the test moves a process there and back without letting it end.

## Not built, and where it attaches

- `export` and `import` (a wire form for a task and for a context), and anything for a second
  machine.
- `type ProcessState` (ruled; to wait for the reviewable-rust layout).
- `set_thread_ordering`, the inspection hooks and `StopReason`: not ruled or not asked for yet.
