//! The kernel's side of its interface to the embedder: the kernel, one process, what it shows, and the data it hands out.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::ops::Range;
use core::time::Duration;

use wcontainers_abi::{CpuError, MemoryId};
use wcontainers_executor::{Executor, PerThreadWatch};
use wcontainers_kernel::linux::general::siginfo;
use wcontainers_kernel::{Errno, TaskId};

/// The owner of processes, which suspends and resumes their tasks and their contexts.
pub trait Kernel {
    type Executor: Executor;
    type SuspendedTask;
    type ProcessContext;

    fn suspend_tasks(
        &mut self,
        tasks: &[TaskId],
    ) -> impl Iterator<Item = (TaskId, Self::SuspendedTask)>;
    fn suspend_process(
        &mut self,
        process: TaskId,
    ) -> Option<(
        Self::ProcessContext,
        impl Iterator<Item = (TaskId, Self::SuspendedTask)>,
    )>;
    fn copy_process_context(&self, process: TaskId) -> Option<Self::ProcessContext>;
    fn resume_process(
        &mut self,
        context: Self::ProcessContext,
        config: ExecutionConfig<Self::Executor>,
    ) -> Option<Self::ProcessContext>;
    fn resume_tasks(
        &mut self,
        tasks: impl IntoIterator<Item = Self::SuspendedTask>,
    ) -> impl Iterator<Item = Self::SuspendedTask>;
}

/// A handle to one process of a kernel.
pub trait Process {
    type State;
    type Kernel: Kernel;

    fn id(&self) -> TaskId;
    fn info(&self) -> impl ProcessInfo;
    fn execution_config(&self) -> &ExecutionConfig<<Self::Kernel as Kernel>::Executor>;
    fn state(&mut self) -> &mut Self::State;
    fn memory_id(&self) -> MemoryId;

    fn read_memory(
        &mut self,
        range: Range<u64>,
    ) -> Result<impl Iterator<Item = &[u8]>, MemoryError>;
    fn write_memory(&mut self, range: Range<u64>, bytes: &[u8]) -> Result<(), MemoryError>;
    fn watch_memory(&mut self, range: Range<u64>, watches: WatchesFor);
    fn watch_thread_memory(&mut self, thread: TaskId, range: Range<u64>, watches: WatchesFor)
    where
        <Self::Kernel as Kernel>::Executor: PerThreadWatch;
    fn take_dirty_memory(&mut self) -> Vec<Range<u64>>;

    fn send_signal(&mut self, thread: TaskId, signal: SignalInformation);
    fn suspend_tasks(
        &mut self,
        tasks: &[TaskId],
    ) -> impl Iterator<Item = (TaskId, <Self::Kernel as Kernel>::SuspendedTask)>;
    fn suspend_process(
        self,
    ) -> (
        <Self::Kernel as Kernel>::ProcessContext,
        impl Iterator<Item = (TaskId, <Self::Kernel as Kernel>::SuspendedTask)>,
    );
    fn copy_process_context(&self) -> <Self::Kernel as Kernel>::ProcessContext;
}

/// A short-lived view of what the kernel keeps for one process.
pub trait ProcessInfo {
    fn memory_mapped(&self) -> u64;
    fn ceiling_distance(&self) -> u64;
    fn compute_used(&self) -> Duration;
    fn parent(&self) -> Option<TaskId>;
    fn is_inspected(&self) -> bool;
}

/// How one process is run.
pub struct ExecutionConfig<E: Executor> {
    pub thread_ordering: ThreadOrdering,
    pub replay_determinism: ReplayDeterminism,
    pub translator: E::TranslatorConfig,
}

/// The way the threads of one process are run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadOrdering {
    Serial,
    Parallel,
}

/// The guarantee a run of a process gives about running it again from the same inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayDeterminism {
    Exact,
    /// Consistent in everything the process does to the outside; no guarantee about its internal state.
    External,
    None,
}

/// The accesses of a process's memory that come back to the embedder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchesFor {
    None,
    Writes,
    All,
}

/// The kind of a watched access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
    Read,
    Write,
    Execute,
}

/// The reason the embedder's read or write of a process's memory failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryError {
    NotMapped,
    NotRead,
    SharedNotWritable,
    InvalidRange,
    Executor(CpuError),
}

/// The answer to a system call: its value, or the error it fails with.
pub type SystemCallResponse = Result<u64, Errno>;

/// Linux's own record of a signal.
pub type SignalInformation = siginfo;

/// The memory a shared mapping shows, as the embedder addresses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemoryObject {
    File {
        device: DeviceId,
        inode: InodeNumber,
    },
    SystemVSegment(SystemVSegmentId),
    Anonymous(AnonymousMemoryId),
}

/// The device a file lives on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeviceId(pub u64);

/// The number of a file on its device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InodeNumber(pub u64);

/// The id of a System V shared memory segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SystemVSegmentId(pub i32);

/// The kernel's own number for a shared anonymous memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnonymousMemoryId(pub u64);
