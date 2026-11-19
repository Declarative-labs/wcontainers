//! The embedder's side of the kernel beyond the abi's `Host`: the task ids it hands out, the hooks
//! the kernel announces to it, and the handle to one process that every hook is given.

use alloc::vec::Vec;
use core::ops::Range;
use core::time::Duration;

use wcontainers_abi::{Cpu, Host};

use crate::memory::{page_down, page_up};
use crate::process::{Channel, Pid};
use crate::syscall::touch::{Ending, ThreadEnd};
use crate::{Kernel, PID_MAX, RESERVED_PIDS};

/// A process or thread id: one space for both, as Linux's `pid_t`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(pub u32);

impl TaskId {
    pub(crate) fn of(pid: Pid) -> Self {
        Self(pid as u32)
    }

    pub(crate) fn pid(self) -> Pid {
        self.0 as Pid
    }
}

/// Which accesses of a process's pages come back to the embedder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watch {
    None,
    Writes,
    /// Reads, writes and running code.
    All,
}

/// What a watched access was; running code reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

/// How the threads of one process are run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadOrdering {
    /// One thread at a time.
    Serial,
    Parallel,
}

/// What a run of a process guarantees about running it again from the same inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayDeterminism {
    /// Every instruction at the same place.
    Exact,
    /// Consistent in everything the process does to the outside; no guarantee about its internal
    /// state.
    External,
    None,
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

/// Why the embedder's read or write of a process's memory failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryError {
    /// Part of the range is not mapped.
    NotMapped,
    /// Part of the range is a page of a backend file not read yet.
    NotRead,
    /// Part of the range is shared memory the process may not write, which Linux's forced
    /// writes refuse too.
    SharedNotWritable,
    /// The executor refused.
    Executor(wcontainers_abi::CpuError),
}

/// What an embedder implements: the abi's services, the task ids, and the hooks. Every hook is
/// synchronous, fires only when every operation on its process is legal, and never nests.
pub trait System: Host + Sync {
    /// A free id for a new process or thread; `previous` is the id handed out last.
    fn task_id_allocate(&self, previous: TaskId) -> TaskId {
        task_id_after(previous)
    }
    /// No process, thread, process group or session carries `id` any more.
    fn task_id_free(&self, id: TaskId) {
        let _ = id;
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
    fn on_memory_accessing(
        &self,
        process: Process<'_>,
        thread: TaskId,
        address: u64,
        access: Access,
    ) {
        let _ = (process, thread, address, access);
    }
}

/// The id after `previous`, wrapping past the low ids as Linux's `alloc_pid` does.
pub fn task_id_after(previous: TaskId) -> TaskId {
    let next = previous.pid() + 1;
    TaskId::of(if next >= PID_MAX { RESERVED_PIDS } else { next })
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
        (self.process.parent != 0).then(|| TaskId::of(self.process.parent))
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
        TaskId::of(self.pid)
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

    /// The bytes of a mapped range, whatever its pages allow.
    pub fn memory_read(&mut self, range: Range<u64>) -> Result<Vec<u8>, MemoryError> {
        let process = &self.kernel.processes[&self.pid];
        process.map.borrow().readable_whole(range.clone())?;
        let mut bytes = alloc::vec![0; (range.end - range.start) as usize];
        self.cpu
            .read(process.memory, range.start, &mut bytes)
            .map_err(MemoryError::Executor)?;
        Ok(bytes)
    }

    /// Writes into a mapped range, whatever its private pages allow; not counted as written by
    /// the process.
    pub fn memory_write(&mut self, address: u64, bytes: &[u8]) -> Result<(), MemoryError> {
        let process = &self.kernel.processes[&self.pid];
        let end = address + bytes.len() as u64;
        let mut map = process.map.borrow_mut();
        map.readable_whole(address..end)?;
        if map.iter().any(|mapping| {
            mapping.start < end
                && mapping.end > address
                && mapping.shared
                && !mapping
                    .permissions
                    .contains(wcontainers_abi::Permissions::WRITE)
        }) {
            return Err(MemoryError::SharedNotWritable);
        }
        map.copy_on_write(self.cpu, process.memory, address, end)
            .map_err(|_| MemoryError::NotMapped)?;
        self.cpu
            .write(process.memory, address, bytes)
            .map_err(MemoryError::Executor)
    }

    /// Watches the mapped pages a range touches (pages of [`PAGE_SIZE`]); unmapping a page ends
    /// its watch, and a fork's child has its parent's.
    pub fn memory_watch(&mut self, range: Range<u64>, watch: Watch) {
        let process = &self.kernel.processes[&self.pid];
        let memory = process.memory;
        let (start, end) = (page_down(range.start), page_up(range.end));
        let changed = process
            .map
            .borrow_mut()
            .set_watch(self.cpu, memory, start..end, watch);
        if changed {
            self.kernel.update_watching();
            self.kernel.wake(Channel::Watch(memory.0));
            self.kernel.unsettled = true;
        }
    }

    /// Watches one thread's accesses and not its siblings'; refused where the executor cannot.
    pub fn memory_watch_thread(
        &mut self,
        thread: TaskId,
        range: Range<u64>,
        watch: Watch,
    ) -> Result<(), Unsupported> {
        let _ = (thread, range, watch);
        Err(Unsupported::PerThreadWatch)
    }

    /// The ranges the process (its threads, and the kernel for it) wrote since this was last
    /// asked; the first answer is every page mapped writable.
    pub fn memory_changed(&mut self) -> Vec<Range<u64>> {
        self.kernel.watching = true;
        let process = &self.kernel.processes[&self.pid];
        let memory = process.memory;
        process.map.borrow_mut().take_changed(self.cpu, memory)
    }
}

impl Kernel {
    /// The process an id names, unless it has ended.
    pub fn process<'a>(&'a mut self, cpu: &'a mut dyn Cpu, id: TaskId) -> Option<Process<'a>> {
        let pid = id.pid();
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

    /// Announces a hook about process `pid` to the embedder.
    pub(crate) fn announce(
        &mut self,
        cx: &mut crate::Context<'_>,
        pid: Pid,
        hook: impl FnOnce(&dyn System, Process<'_>),
    ) {
        let process = Process {
            kernel: self,
            cpu: &mut *cx.cpu,
            pid,
        };
        hook(cx.host, process);
    }
}

impl Kernel {
    /// Tells the embedder that thread `tid` is about to touch a watched address; whether the watch
    /// is gone when the hook returns.
    pub(crate) fn watch_touched(
        &mut self,
        cx: &mut crate::Context<'_>,
        tid: crate::process::Tid,
        address: u64,
        access: Access,
    ) -> bool {
        let pid = self.threads[&tid].process;
        let thread = TaskId::of(tid);
        self.announce(cx, pid, |system, process| {
            system.on_memory_accessing(process, thread, address, access)
        });
        let map = self.processes[&pid].map.borrow();
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
}

impl crate::syscall::touch::Memory for CallMemory<'_, '_, '_> {
    fn touch(&mut self, address: u64, length: u64, access: Access) -> bool {
        let end = address.saturating_add(length);
        if let Some(described) = &mut self
            .kernel
            .process_of(self.tid)
            .map
            .borrow_mut()
            .watches
            .described
        {
            described.push((address, end));
        }
        loop {
            let hit = self
                .kernel
                .process_of(self.tid)
                .map
                .borrow()
                .watches
                .first_watched(address, end, access);
            let Some(at) = hit else {
                return true;
            };
            if !self.kernel.watch_touched(self.cx, self.tid, at, access) {
                return false;
            }
        }
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
            Ending::Others => threads
                .iter()
                .filter(|thread| **thread != tid)
                .map(|&thread| end(thread, true))
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
        assert_eq!(
            task_id_after(TaskId::of(PID_MAX - 1)),
            TaskId::of(RESERVED_PIDS)
        );
    }
}
