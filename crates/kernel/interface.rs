#![no_std]

extern crate alloc;

pub mod arm64_features;
pub mod errno;
pub mod signal;
pub mod signal_information;
pub mod x86_64_features;

use alloc::vec::Vec;
use core::num::NonZeroUsize;
use core::ops::Range;
use core::time::Duration;

use arm64_features::Arm64Features;
use errno::{Errno, UnknownErrno};
use signal::Signal;
use signal_information::SignalInformation;
use wcontainers_abi::{CpuError, MemoryId};
use wcontainers_executor::{Executor, PerThreadWatch};
use x86_64_features::X86_64Features;

pub trait Kernel: Sync + Sized {
    type Executor: Executor;
    /// A slow or incomplete path that the kernel can take.
    type Fallback;
    type Config;
    type Runner: Runner<Self::Executor>;
    type SuspendedTask;
    type ProcessContext;

    /// Makes a kernel that executes at most the given number of threads at the same time, and one runner for each.
    fn new(
        config: Self::Config,
        parallelism: NonZeroUsize,
    ) -> (Self, impl Iterator<Item = Self::Runner>);
    /// Takes the given tasks out of this kernel instance atomically, whichever processes they belong to, returning each one that is still executing on it.
    fn suspend_tasks(
        &self,
        tasks: &[TaskId],
    ) -> impl Iterator<Item = (TaskId, Self::SuspendedTask)>;
    /// Takes the process off this kernel instance, returning its context and each of its tasks that is still executing on it.
    fn suspend_process(
        &self,
        process: TaskId,
    ) -> Option<(
        Self::ProcessContext,
        impl Iterator<Item = (TaskId, Self::SuspendedTask)>,
    )>;
    fn copy_process_context(&self, process: TaskId) -> Option<Self::ProcessContext>;
    fn resume_process(
        &self,
        context: Self::ProcessContext,
        config: ExecutionConfig<Self::Executor>,
    ) -> Option<Self::ProcessContext>;
    fn resume_tasks(
        &self,
        tasks: impl IntoIterator<Item = Self::SuspendedTask>,
    ) -> impl Iterator<Item = Self::SuspendedTask>;
}


pub trait Runner<E: Executor>: Send {
    /// Runs what the kernel decides is next, using the given executor.
    fn run(&mut self, executor: &mut E) -> StopReason;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    Interrupted,
    Suspended,
    Idle { until: Option<Duration> },
    Finished(ProcessEnd),
    /// A host thread ended inside a run, so the kernel can no longer be run.
    Panicked,
}

/// The way a container's first process ended: its [wait status](https://man7.org/linux/man-pages/man2/wait.2.html#:~:text=If%20wstatus%20is%20not%20NULL), or the error that kept its program from starting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessEnd {
    Exited { code: u8 },
    Signaled { signal: Signal, core_dumped: bool },
    NotStarted { error: Errno },
}

/// A handle to one process of a kernel.
pub trait Process: ProcessInfo {
    type State;
    type Kernel: Kernel;

    fn id(&self) -> TaskId;
    fn execution_config(&self) -> &ExecutionConfig<ExecutorOf<Self::Kernel>>;
    fn state(&mut self) -> &mut Self::State;

    fn memory_id(&self) -> MemoryId;
    fn read_memory(
        &mut self,
        range: Range<u64>,
    ) -> Result<impl Iterator<Item = &[u8]>, MemoryError>;
    fn write_memory(&mut self, range: Range<u64>, bytes: &[u8]) -> Result<(), WriteMemoryError>;
    /// Sets which accesses to a range fire `ProcessTracer::on_watched_memory_accessing`.
    fn watch_memory(&mut self, range: Range<u64>, watches: WatchesFor);
    /// Sets the same watch as `watch_memory`, for one thread's accesses only.
    fn watch_thread_memory(&mut self, thread: TaskId, range: Range<u64>, watches: WatchesFor)
    where
        ExecutorOf<Self::Kernel>: PerThreadWatch;
    /// Returns the pages written since the last call, hardware accelerated when possible.
    fn take_dirty_memory(&mut self) -> Vec<Range<u64>>;

    fn send_signal(&mut self, thread: TaskId, signal: SignalInformation);
    /// Takes the given threads of this process out of this kernel instance atomically, returning each one that is still executing on it.
    fn suspend_threads(
        &mut self,
        threads: &[TaskId],
    ) -> impl Iterator<Item = (TaskId, SuspendedTaskOf<Self::Kernel>)>;
    /// Takes the process off this kernel instance, returning its context and each of its tasks that is still executing on it.
    fn suspend_process(
        self,
    ) -> (
        ProcessContextOf<Self::Kernel>,
        impl Iterator<Item = (TaskId, SuspendedTaskOf<Self::Kernel>)>,
    );
    fn copy_process_context(&self) -> ProcessContextOf<Self::Kernel>;
}

pub trait ProcessOf<State, K: Kernel>: Process<State = State, Kernel = K> {}

pub type ExecutorOf<K> = <K as Kernel>::Executor;
pub type SuspendedTaskOf<K> = <K as Kernel>::SuspendedTask;
pub type ProcessContextOf<K> = <K as Kernel>::ProcessContext;

pub trait ProcessInfo {
    /// Returns the number of bytes of address space the process has mapped.
    fn mapped_memory(&self) -> u64;
    /// Returns the number of bytes the process can still map before it reaches the executor's highest possible address.
    fn address_ceiling_distance(&self) -> u64;
    /// Returns the CPU time the process's threads have used, ended ones included.
    fn cpu_time(&self) -> Duration;
    /// Returns the process's current [parent](https://man7.org/linux/man-pages/man2/getppid.2.html), or none for the container's first process.
    fn parent(&self) -> Option<TaskId>;
    /// Tells whether another process is reading this one's internal state.
    fn is_inspected(&self) -> bool;
}

pub struct ExecutionConfig<E: Executor> {
    pub thread_ordering: ThreadOrdering,
    pub replay_determinism: ReplayDeterminism,
    pub translator: E::TranslatorConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadOrdering {
    /// Executes a thread only while no other thread sharing its memory executes.
    Serial,
    /// Executes threads one or more at a time.
    Parallel,
}

/// The guarantee one execution of a process gives about executing it again from the same inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayDeterminism {
    Exact,
    /// Consistent in everything the process does to the outside; no guarantee about its internal state.
    External,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchesFor {
    None,
    Writes,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
    Read,
    Write,
    Execute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryError {
    NotMapped,
    NotRead,
    InvalidRange,
    Executor(CpuError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteMemoryError {
    Memory(MemoryError),
    SharedNotWritable,
}

pub type SystemCallResponse = Result<u64, SystemCallError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SystemCallError {
    WellKnown(Errno),
    Unknown(UnknownErrno),
}

/// A [process or thread ID](https://man7.org/linux/man-pages/man3/pid_t.3type.html#:~:text=pid_t%20is%20a%20type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(pub u32);

/// A Linux [user ID](https://man7.org/linux/man-pages/man3/pid_t.3type.html#:~:text=uid_t%20is%20a%20type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UserId(pub u32);

/// The memory behind a shared mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemoryObject {
    /// A file mapped with [`MAP_SHARED`](https://man7.org/linux/man-pages/man2/mmap.2.html#:~:text=MAP_SHARED).
    File {
        device: DeviceId,
        inode: InodeNumber,
    },
    /// A [System V shared memory segment](https://man7.org/linux/man-pages/man7/sysvipc.7.html#:~:text=Shared%20memory%20segments).
    SystemVSegment(SystemVSegmentId),
    /// A mapping made with [`MAP_SHARED | MAP_ANONYMOUS`](https://man7.org/linux/man-pages/man2/mmap.2.html#:~:text=not%20backed%20by%20any%20file).
    Anonymous(AnonymousMemoryId),
}

/// A Linux [device ID](https://man7.org/linux/man-pages/man3/dev_t.3type.html).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeviceId(pub u64);

/// A file's [inode number](https://man7.org/linux/man-pages/man7/inode.7.html#:~:text=Inode%20number).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InodeNumber(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SystemVSegmentId(pub i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnonymousMemoryId(pub u64);

/// The processor a program is told it has: what it reads when it asks the processor to identify itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProcessorFeatures {
    Arm64(Arm64Features),
    X86_64(X86_64Features),
}
