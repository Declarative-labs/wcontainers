//! The embedder's side of the kernel beyond the abi's `Host`: the task ids it hands out, the hooks
//! the kernel announces to it, and the handle to one process that every hook is given.

use alloc::vec::Vec;
use core::ops::{ControlFlow, Range};
use core::time::Duration;

use wcontainers_abi::{Cpu, Host, MemoryId, SystemCallRequest};

use crate::memory::{page_down, page_up};
use crate::process::{Channel, Pid};
use crate::syscall::touch::{Ending, ThreadEnd};
use crate::{Kernel, PID_MAX, RESERVED_PIDS};

pub use wcontainers_kernel::{
    AccessKind, AnonymousMemoryId, DeviceId, InodeNumber, MemoryError, MemoryObject,
    ReplayDeterminism, SystemCallResponse, SystemVSegmentId, TaskId, ThreadOrdering, WatchesFor,
    WriteMemoryError,
};

/// The task id of a process or thread number.
pub(crate) fn task_id(pid: Pid) -> TaskId {
    TaskId(pid as u32)
}

/// The process or thread number of a task id.
pub(crate) fn pid_of(id: TaskId) -> Pid {
    id.0 as Pid
}

/// How one process is run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionConfig {
    pub thread_ordering: ThreadOrdering,
    pub replay_determinism: ReplayDeterminism,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            thread_ordering: ThreadOrdering::Serial,
            replay_determinism: ReplayDeterminism::Exact,
        }
    }
}

/// Something of the interface that is not built yet, by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    ThreadOrdering(ThreadOrdering),
    ReplayDeterminism(ReplayDeterminism),
    /// The executor cannot watch one thread and not its siblings.
    PerThreadWatch,
}

impl ExecutionConfig {
    /// Whether the kernel can run a process this way.
    pub fn supported(&self) -> Result<(), Unsupported> {
        if self.thread_ordering != ThreadOrdering::Serial {
            return Err(Unsupported::ThreadOrdering(self.thread_ordering));
        }
        if self.replay_determinism != ReplayDeterminism::Exact {
            return Err(Unsupported::ReplayDeterminism(self.replay_determinism));
        }
        Ok(())
    }
}

pub use wcontainers_kernel::signal_information::SignalInformation;

/// What an embedder implements: the abi's services, the task ids, and the hooks. Every hook is
/// synchronous, fires only when every operation on its process is legal, and never nests.
pub trait System: Host + Sync {
    /// A free id for a new process or thread; `previous` is the id handed out last.
    fn allocate_task_id(&self, previous: TaskId) -> TaskId {
        task_id_after(previous)
    }
    /// No process, thread, process group or session carries `id` any more.
    fn free_task_id(&self, id: TaskId);

    /// A system call a thread made, before anything of it happens: `Continue` runs the request
    /// returned, `Break` answers it without running it.
    fn intercept_system_call_request(
        &self,
        process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        let _ = (process, thread);
        ControlFlow::Continue(request)
    }
    /// What a system call the kernel ran answers, before the thread sees it; the thread sees what
    /// this returns.
    fn intercept_system_call_response(
        &self,
        process: Process<'_>,
        thread: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse {
        let _ = (process, thread, request);
        response
    }

    /// A process made by fork, or the container's first process, before its first instruction.
    fn on_process_starting(&self, process: Process<'_>) {
        let _ = process;
    }
    /// A process whose exec has loaded a new program, before the program's first instruction.
    fn on_program_starting(&self, process: Process<'_>) {
        let _ = process;
    }
    /// A new thread in an existing process, before its first instruction.
    fn on_thread_starting(&self, process: Process<'_>, thread: TaskId) {
        let _ = (process, thread);
    }
    /// A process about to be torn down, its final state still readable.
    fn on_process_exiting(&self, process: Process<'_>) {
        let _ = process;
    }
    /// A request for `bytes` of memory is about to cross the ceiling; if the process is still here
    /// when this returns, the request fails as out of memory.
    fn on_process_outgrown(&self, process: Process<'_>, bytes: u64) {
        let _ = (process, bytes);
    }
    /// A thread is about to access a watched page; if the watch is gone when this returns the
    /// thread carries on, otherwise it waits until the process's watches change.
    fn on_watched_memory_accessing(
        &self,
        process: Process<'_>,
        thread: TaskId,
        address: u64,
        kind: AccessKind,
    ) {
        let _ = (process, thread, address, kind);
    }
    /// The kernel wrote `range` of the process's memory for `thread`.
    fn on_memory_written_by_kernel(&self, process: Process<'_>, thread: TaskId, range: Range<u64>) {
        let _ = (process, thread, range);
    }
    /// The process maps `range` of its memory to `source` from `offset`, a source some process
    /// may write through a mapping.
    fn on_shared_memory_mapping(
        &self,
        process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        let _ = (process, source, offset, range);
    }
    /// The process no longer maps `range`, announced before as mapping `source` from `offset`.
    fn on_shared_memory_unmapped(
        &self,
        process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        let _ = (process, source, offset, range);
    }
    /// `thread` has taken `signal` to act on it, before anything of its action.
    fn on_signal_delivering(
        &self,
        process: Process<'_>,
        thread: TaskId,
        signal: &SignalInformation,
    ) {
        let _ = (process, thread, signal);
    }
}

/// The id after `previous`, wrapping past the low ids as Linux's `alloc_pid` does.
pub fn task_id_after(previous: TaskId) -> TaskId {
    let next = pid_of(previous) + 1;
    task_id(if next >= PID_MAX { RESERVED_PIDS } else { next })
}

/// A call's return value as the embedder sees it.
pub(crate) fn response_of(value: u64) -> SystemCallResponse {
    match crate::Errno(value.wrapping_neg() as u32).to_linux() {
        Some(errno) if crate::Errno::is_error_return(value) => Err(errno),
        _ => Ok(value),
    }
}

/// The return value a thread sees for a response.
pub(crate) fn return_of(response: SystemCallResponse) -> u64 {
    match response {
        Ok(value) => value,
        Err(errno) => crate::Errno::from_linux(errno).to_return(),
    }
}

/// A range the kernel wrote into a memory for a thread, not yet told to the embedder.
pub(crate) struct KernelWrite {
    pub pid: Pid,
    pub tid: crate::process::Tid,
    pub memory: wcontainers_abi::MemoryId,
    pub range: Range<u64>,
}

/// One process of a kernel, for as long as the borrow lasts.
pub struct Process<'a> {
    pub(crate) kernel: &'a mut Kernel,
    pub(crate) cpu: &'a mut dyn Cpu,
    pub(crate) pid: Pid,
}

/// What the kernel keeps for a process, read when asked.
pub struct ProcessInfo<'a> {
    kernel: &'a Kernel,
    process: &'a crate::process::Process,
}

impl ProcessInfo<'_> {
    /// Bytes of address space the process has mapped.
    pub fn memory_mapped(&self) -> u64 {
        self.process
            .map
            .borrow()
            .iter()
            .map(|mapping| mapping.end - mapping.start)
            .sum()
    }

    /// Bytes below the ceiling that new mappings can still take before a request crosses it.
    pub fn ceiling_distance(&self) -> u64 {
        self.process.map.borrow().placeable()
    }

    /// The process's parent, as `getppid` reports it; none for the container's first process.
    pub fn parent(&self) -> Option<TaskId> {
        (self.process.parent != 0).then(|| task_id(self.process.parent))
    }

    /// CPU time the process's threads have used, ended ones included.
    pub fn compute_used(&self) -> Duration {
        let live: u64 = self
            .process
            .threads
            .iter()
            .map(|tid| self.kernel.threads[tid].runtime)
            .sum();
        Duration::from_nanos(self.process.ended_time + live)
    }
}

impl Process<'_> {
    pub fn id(&self) -> TaskId {
        task_id(self.pid)
    }

    pub fn info(&self) -> ProcessInfo<'_> {
        ProcessInfo {
            kernel: self.kernel,
            process: &self.kernel.processes[&self.pid],
        }
    }

    pub fn execution_config(&self) -> ExecutionConfig {
        self.kernel.processes[&self.pid].execution
    }

    /// The memory the process runs in: the same for processes that share it (vfork, `CLONE_VM`),
    /// and a new one once it runs a new program.
    pub fn memory_id(&self) -> MemoryId {
        self.kernel.processes[&self.pid].memory
    }

    /// The bytes of a mapped range, whatever its pages allow.
    pub fn read_memory(&mut self, range: Range<u64>) -> Result<Vec<u8>, MemoryError> {
        let length = range
            .end
            .checked_sub(range.start)
            .and_then(|length| usize::try_from(length).ok())
            .ok_or(MemoryError::InvalidRange)?;
        let process = &self.kernel.processes[&self.pid];
        process.map.borrow().readable_whole(range.clone())?;
        let mut bytes = alloc::vec![0; length];
        self.cpu
            .read(process.memory, range.start, &mut bytes)
            .map_err(MemoryError::Executor)?;
        Ok(bytes)
    }

    /// Writes into a mapped range, whatever its private pages allow; not counted as written by
    /// the process.
    pub fn write_memory(&mut self, address: u64, bytes: &[u8]) -> Result<(), WriteMemoryError> {
        let process = &self.kernel.processes[&self.pid];
        let end = address
            .checked_add(bytes.len() as u64)
            .ok_or(WriteMemoryError::Memory(MemoryError::InvalidRange))?;
        let mut map = process.map.borrow_mut();
        map.readable_whole(address..end)
            .map_err(WriteMemoryError::Memory)?;
        if map.iter().any(|mapping| {
            mapping.start < end
                && mapping.end > address
                && mapping.shared
                && !mapping
                    .permissions
                    .contains(wcontainers_abi::Permissions::WRITE)
        }) {
            return Err(WriteMemoryError::SharedNotWritable);
        }
        map.copy_on_write(self.cpu, process.memory, address, end)
            .map_err(|_| WriteMemoryError::Memory(MemoryError::NotMapped))?;
        self.cpu
            .write(process.memory, address, bytes)
            .map_err(|error| WriteMemoryError::Memory(MemoryError::Executor(error)))
    }

    /// Watches the mapped pages a range touches (pages of [`PAGE_SIZE`]); unmapping a page ends
    /// its watch, and a fork's child has its parent's.
    pub fn watch_memory(&mut self, range: Range<u64>, watches: WatchesFor) {
        let process = &self.kernel.processes[&self.pid];
        let memory = process.memory;
        let (start, end) = (page_down(range.start), page_up(range.end));
        let (changed, raised) =
            process
                .map
                .borrow_mut()
                .set_watch(self.cpu, memory, start..end, watches);
        self.kernel.watches_raised += raised;
        if changed {
            self.kernel.update_watching();
            self.kernel.wake(Channel::Watch(memory.0));
            self.kernel.unsettled = true;
        }
    }

    /// Watches one thread's accesses and not its siblings'; refused where the executor cannot.
    pub fn watch_thread_memory(
        &mut self,
        thread: TaskId,
        range: Range<u64>,
        watches: WatchesFor,
    ) -> Result<(), Unsupported> {
        let _ = (thread, range, watches);
        Err(Unsupported::PerThreadWatch)
    }

    /// Sends `signal` to one thread of the process when the kernel next settles, before any
    /// thread runs again; a record must be built from zero, as Linux's `clear_siginfo` builds one.
    pub fn send_signal(&mut self, thread: TaskId, signal: SignalInformation) {
        let information = crate::siginfo::encode(&signal);
        self.kernel
            .signals_sent
            .push_back((self.pid, pid_of(thread), information));
        self.kernel.unsettled = true;
    }

    /// The ranges the process (its threads, and the kernel for it) wrote since this was last
    /// asked; the first answer is every page mapped writable.
    pub fn take_dirty_memory(&mut self) -> Vec<Range<u64>> {
        self.kernel.watching = true;
        let process = &self.kernel.processes[&self.pid];
        let memory = process.memory;
        process.map.borrow_mut().take_dirty(self.cpu, memory)
    }
}

impl Kernel {
    /// The process an id names, unless it has ended.
    pub fn process<'a>(&'a mut self, cpu: &'a mut dyn Cpu, id: TaskId) -> Option<Process<'a>> {
        let pid = pid_of(id);
        self.processes
            .get(&pid)
            .filter(|process| process.ended.is_none())?;
        Some(Process {
            kernel: self,
            cpu,
            pid,
        })
    }
}

impl Kernel {
    /// Notes whether any process's pages are watched or recorded, which calls and faults then
    /// look at.
    pub(crate) fn update_watching(&mut self) {
        self.watching = self
            .processes
            .values()
            .any(|process| process.map.borrow().watches.narrows());
    }

    /// Whether thread `tid`'s process has watched pages, which its calls and signals meet first.
    pub(crate) fn watched(&self, tid: crate::process::Tid) -> bool {
        self.watching && self.process_of(tid).map.borrow().watches.any()
    }

    /// Announces a hook about process `pid` to the embedder, after what the kernel wrote before.
    pub(crate) fn announce(
        &mut self,
        cx: &mut crate::Context<'_>,
        pid: Pid,
        hook: impl FnOnce(&dyn System, Process<'_>),
    ) {
        if self.map_notes.sharing_changed.get() {
            self.announce_sharing(cx);
        }
        self.announce_kernel_writes(cx);
        // Its process taken off the kernel in one of those hooks is not announced.
        if !self.processes.contains_key(&pid) {
            return;
        }
        let process = Process {
            kernel: self,
            cpu: &mut *cx.cpu,
            pid,
        };
        hook(cx.host, process);
    }

    /// As [`Self::announce`] for a process's start: the shared memory it maps is announced after.
    pub(crate) fn announce_start(
        &mut self,
        cx: &mut crate::Context<'_>,
        pid: Pid,
        hook: impl FnOnce(&dyn System, Process<'_>),
    ) {
        self.announce_kernel_writes(cx);
        if !self.processes.contains_key(&pid) {
            return;
        }
        let process = Process {
            kernel: self,
            cpu: &mut *cx.cpu,
            pid,
        };
        hook(cx.host, process);
        if self.map_notes.sharing_changed.get() {
            self.announce_sharing(cx);
        }
    }

    /// Tells the embedder what the kernel wrote into processes' memory, each to the process whose
    /// memory it is: the one it was written for, or, once that one has left the memory (an
    /// exec), another process still using it.
    pub(crate) fn announce_kernel_writes(&mut self, cx: &mut crate::Context<'_>) {
        if self.kernel_writes.get_mut().is_empty() {
            return;
        }
        let mut written = core::mem::take(self.kernel_writes.get_mut());
        for write in written.drain(..) {
            let uses = |process: &crate::process::Process| {
                process.ended.is_none() && process.memory == write.memory
            };
            let pid = match self.processes.get(&write.pid) {
                Some(process) if uses(process) => write.pid,
                _ => match self.processes.values().find(|process| uses(process)) {
                    Some(process) => process.pid,
                    None => continue,
                },
            };
            let process = Process {
                kernel: self,
                cpu: &mut *cx.cpu,
                pid,
            };
            cx.host
                .on_memory_written_by_kernel(process, task_id(write.tid), write.range);
        }
        // The list keeps its room; what a hook's own work wrote meanwhile goes after.
        written.append(self.kernel_writes.get_mut());
        *self.kernel_writes.get_mut() = written;
    }
}

impl Kernel {
    /// Runs a walk over what a thread is about to touch until it meets no watch: from its start
    /// again whenever a hook raised a watch during it, since that page may lie behind it; false
    /// if the thread must wait.
    pub(crate) fn walk_watched(
        &mut self,
        cx: &mut crate::Context<'_>,
        mut walk: impl FnMut(&mut Self, &mut crate::Context<'_>) -> bool,
    ) -> bool {
        loop {
            let before = self.watches_raised;
            if !walk(self, cx) {
                return false;
            }
            if self.watches_raised == before {
                return true;
            }
        }
    }

    /// Tells the embedder that thread `tid` is about to touch a watched address of process `pid`
    /// (its own, or one whose memory it reads); whether the watch is gone when the hook returns.
    pub(crate) fn watch_touched(
        &mut self,
        cx: &mut crate::Context<'_>,
        tid: crate::process::Tid,
        pid: Pid,
        address: u64,
        access: AccessKind,
    ) -> bool {
        let thread = task_id(tid);
        self.announce(cx, pid, |system, process| {
            system.on_watched_memory_accessing(process, thread, address, access)
        });
        // A process the hook took off the kernel has no watch here; the call finds it absent.
        let Some(process) = self.processes.get(&pid) else {
            return true;
        };
        let map = process.map.borrow();
        map.watches
            .first_watched(address, address + 1, access)
            .is_none()
    }

    /// Thread `tid` waits until its process's watches change: to run its call again, or, with
    /// `resume`, to go on from a touch outside a call (`delivering` if a signal's frame waits).
    pub(crate) fn wait_on_watch(
        &mut self,
        tid: crate::process::Tid,
        resume: Option<wcontainers_abi::Resume>,
        delivering: bool,
    ) {
        let memory = self.process_of(tid).memory;
        let thread = self.thread_mut(tid);
        if let Some(resume) = resume {
            thread.filling = true;
            thread.resume = resume;
            thread.delivering = delivering;
        }
        self.park(tid, Channel::Watch(memory.0));
    }
}

/// A call's memory as the walk over what it touches sees it.
pub(crate) struct CallMemory<'k, 'c, 'x> {
    pub kernel: &'k mut Kernel,
    pub cx: &'c mut crate::Context<'x>,
    pub tid: crate::process::Tid,
    /// The memory whose watch the walk stopped at, which the thread waits on.
    pub unmet: Option<MemoryId>,
}

impl<'k, 'c, 'x> CallMemory<'k, 'c, 'x> {
    pub(crate) fn new(
        kernel: &'k mut Kernel,
        cx: &'c mut crate::Context<'x>,
        tid: crate::process::Tid,
    ) -> Self {
        Self {
            kernel,
            cx,
            tid,
            unmet: None,
        }
    }
}

impl crate::syscall::touch::Memory for CallMemory<'_, '_, '_> {
    fn touch(&mut self, address: u64, length: u64, access: AccessKind) -> bool {
        let pid = self.kernel.threads[&self.tid].process;
        self.touch_process(pid, address, length, access)
    }

    fn touch_process(&mut self, pid: Pid, address: u64, length: u64, access: AccessKind) -> bool {
        let end = address.saturating_add(length);
        let map = &self.kernel.processes[&pid].map;
        if let Some(described) = &mut map.borrow_mut().watches.described {
            described.push((address, end, access));
        }
        loop {
            let Some(process) = self.kernel.processes.get(&pid) else {
                return true;
            };
            let hit = process
                .map
                .borrow()
                .watches
                .first_watched(address, end, access);
            let Some(at) = hit else {
                return true;
            };
            if !self
                .kernel
                .watch_touched(self.cx, self.tid, pid, at, access)
            {
                self.unmet = Some(self.kernel.processes[&pid].memory);
                return false;
            }
        }
    }

    fn own_task(&mut self, id: u64) -> bool {
        // A `pid_t` argument, as `process_vm` takes it.
        self.kernel
            .threads
            .get(&(id as Pid))
            .is_some_and(|thread| thread.process == self.kernel.threads[&self.tid].process)
    }

    fn file_memory(&mut self, fd: u64) -> Option<(Pid, u64, u64)> {
        self.kernel.file_memory(self.tid, fd)
    }

    fn read(&mut self, address: u64, into: &mut [u8]) -> bool {
        let memory = self.kernel.process_of(self.tid).memory;
        self.cx.cpu.read(memory, address, into).is_ok()
    }

    fn stack_pointer(&mut self) -> u64 {
        let mut registers = wcontainers_abi::RegisterBlock::ZERO;
        self.cx
            .cpu
            .registers(self.kernel.threads[&self.tid].cpu, &mut registers);
        registers.sp
    }

    fn thread_ends(&mut self, ending: Ending) -> Vec<ThreadEnd> {
        self.kernel.thread_ends(self.tid, ending)
    }

    fn resuming(&mut self) -> bool {
        self.kernel.threads[&self.tid].call.started
    }

    fn writable(&mut self, fd: u64) -> Option<bool> {
        let file = self.kernel.call_file(self.tid, fd as i32).ok()?;
        Some(file.borrow().writable())
    }
}

impl Kernel {
    /// The process and the range of its memory that thread `tid` reading its descriptor `fd`
    /// copies (`/proc/<pid>/cmdline`, `environ`); none for any other file.
    pub(crate) fn file_memory(&self, tid: crate::process::Tid, fd: u64) -> Option<(Pid, u64, u64)> {
        let file = self.call_file(tid, fd as i32).ok()?;
        let file = file.borrow();
        let node = file.node.borrow();
        let crate::fs::Content::Proc(crate::fs::proc::Entry::ProcessFile(task, which)) =
            node.content
        else {
            return None;
        };
        let (start, length) = crate::fs::proc::memory_read(self.processes.get(&task.pid)?, which)?;
        Some((task.pid, start, length))
    }

    /// Another process with watched pages whose memory thread `tid`'s call reads through a file,
    /// which the call then meets.
    pub(crate) fn reads_watched_process(
        &self,
        tid: crate::process::Tid,
        call: SystemCallRequest,
    ) -> Option<Pid> {
        let fd = crate::syscall::touch::read_descriptor(call)?;
        let (pid, _, _) = self.file_memory(tid, fd)?;
        let process = &self.processes[&pid];
        (pid != self.threads[&tid].process && process.map.borrow().watches.any()).then_some(pid)
    }

    /// What the ends of the threads `ending` names touch, as `thread_end` will: a thread's word
    /// for its joiners is cleared only while others still use the memory.
    pub(crate) fn thread_ends(&self, tid: crate::process::Tid, ending: Ending) -> Vec<ThreadEnd> {
        let pid = self.threads[&tid].process;
        let process = &self.processes[&pid];
        let shared = self.memory_in_use_elsewhere(pid, process.memory);
        let threads = &process.threads;
        let end = |tid: crate::process::Tid, others_use_memory: bool| {
            let thread = &self.threads[&tid];
            ThreadEnd {
                tid,
                clear_child_tid: others_use_memory.then_some(thread.clear_child_tid),
                robust_list: thread.robust_list.0,
            }
        };
        match ending {
            Ending::Caller => alloc::vec![end(tid, shared || threads.len() > 1)],
            Ending::Process => threads
                .iter()
                .enumerate()
                .map(|(index, &thread)| end(thread, shared || index + 1 < threads.len()))
                .collect(),
            Ending::Exec => threads
                .iter()
                .filter(|thread| **thread != tid)
                .map(|&thread| end(thread, true))
                .chain([end(tid, shared)])
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_serial_exact_runs_and_the_rest_are_refused_by_name() {
        let config = |thread_ordering, replay_determinism| ExecutionConfig {
            thread_ordering,
            replay_determinism,
        };
        assert_eq!(ExecutionConfig::default().supported(), Ok(()));
        assert_eq!(
            config(ThreadOrdering::Parallel, ReplayDeterminism::Exact).supported(),
            Err(Unsupported::ThreadOrdering(ThreadOrdering::Parallel))
        );
        for replay in [ReplayDeterminism::External, ReplayDeterminism::None] {
            assert_eq!(
                config(ThreadOrdering::Serial, replay).supported(),
                Err(Unsupported::ReplayDeterminism(replay))
            );
        }
    }

    #[test]
    fn the_default_task_ids_count_and_wrap_past_the_low_ones() {
        assert_eq!(task_id_after(TaskId(0)), TaskId(1));
        assert_eq!(task_id_after(task_id(PID_MAX - 1)), task_id(RESERVED_PIDS));
    }
}
