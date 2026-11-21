//! The kernel as the kernel interface sees it: with the executor its threads run on, and with the
//! embedder's state and execution configuration of each process kept beside the process's record.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;
use core::marker::PhantomData;
use core::ops::Range;
use core::time::Duration;

use wcontainers_abi::{Cpu, MemoryId};
use wcontainers_executor::{Executor, PerThreadWatch};
use wcontainers_kernel as interface;
use wcontainers_kernel::signal_information::SignalInformation;
use wcontainers_kernel::{MemoryError, TaskId, WatchesFor, WriteMemoryError};

use crate::process::Pid;
use crate::suspend::{ProcessContext, SuspendedTask};
use crate::system::{ExecutionConfig, pid_of};

/// What the embedder keeps of one process: its state, and how it asked for the process to run,
/// shared by the processes a fork made from it.
struct ProcessData<E: Executor, State> {
    config: Rc<interface::ExecutionConfig<E>>,
    state: State,
}

/// The embedder's data of one process, of a type only the embedder knows; a fork's child gets a
/// copy of its parent's.
pub(crate) struct EmbedderData {
    value: Box<dyn Any>,
    copy: fn(&dyn Any) -> Box<dyn Any>,
}

impl Clone for EmbedderData {
    fn clone(&self) -> Self {
        Self {
            value: (self.copy)(&*self.value),
            copy: self.copy,
        }
    }
}

impl EmbedderData {
    fn new<E: Executor + 'static, State: Clone + 'static>(data: ProcessData<E, State>) -> Self {
        fn copy<E: Executor + 'static, State: Clone + 'static>(value: &dyn Any) -> Box<dyn Any> {
            let data = value
                .downcast_ref::<ProcessData<E, State>>()
                .expect("the data's own type");
            Box::new(ProcessData::<E, State> {
                config: Rc::clone(&data.config),
                state: data.state.clone(),
            })
        }
        Self {
            value: Box::new(data),
            copy: copy::<E, State>,
        }
    }
}

/// The kernel's own way of running a process from the interface's.
fn kernel_config<E: Executor>(config: &interface::ExecutionConfig<E>) -> ExecutionConfig {
    ExecutionConfig {
        thread_ordering: config.thread_ordering,
        replay_determinism: config.replay_determinism,
    }
}

/// A kernel with the executor its threads run on, each process with the embedder's `State`.
pub struct KernelWithExecutor<C, State> {
    kernel: crate::Kernel,
    executor: C,
    state: PhantomData<fn() -> State>,
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static> KernelWithExecutor<C, State> {
    /// Starts a container on `kernel`, as [`crate::Kernel::start`], its first process with
    /// `state` and run as `config` says (the start's own execution is replaced by it).
    pub fn start<S: crate::System>(
        mut kernel: crate::Kernel,
        mut executor: C,
        system: &alloc::sync::Arc<S>,
        start: &crate::Start<'_>,
        config: interface::ExecutionConfig<C>,
        state: State,
    ) -> Result<Self, crate::Unsupported> {
        let start = crate::Start {
            execution: kernel_config(&config),
            ..*start
        };
        kernel.start(&mut executor, system, &start)?;
        let first = kernel
            .init
            .expect("a started container has a first process");
        let data = ProcessData {
            config: Rc::new(config),
            state,
        };
        if let Some(process) = kernel.processes.get_mut(&first) {
            process.embedder = Some(EmbedderData::new(data));
        }
        Ok(Self {
            kernel,
            executor,
            state: PhantomData,
        })
    }

    /// The kernel and the executor, to drive them.
    pub fn parts(&mut self) -> (&mut crate::Kernel, &mut C) {
        (&mut self.kernel, &mut self.executor)
    }

    /// One process, unless it has ended or carries no state of this embedder.
    pub fn process(&mut self, id: TaskId) -> Option<KernelProcess<'_, C, State>> {
        let pid = pid_of(id);
        self.kernel
            .processes
            .get(&pid)
            .filter(|process| process.ended.is_none())?
            .embedder
            .as_ref()?
            .value
            .downcast_ref::<ProcessData<C, State>>()?;
        Some(KernelProcess {
            kernel: self,
            pid,
            read: Vec::new(),
        })
    }
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static> interface::ProcessController
    for KernelWithExecutor<C, State>
{
    type Executor = C;
    type SuspendedTask = SuspendedTask;
    type ProcessContext = ProcessContext;

    fn suspend_tasks(&mut self, tasks: &[TaskId]) -> impl Iterator<Item = (TaskId, SuspendedTask)> {
        self.kernel.suspend_tasks(&mut self.executor, tasks)
    }

    fn suspend_process(
        &mut self,
        process: TaskId,
    ) -> Option<(
        ProcessContext,
        impl Iterator<Item = (TaskId, SuspendedTask)>,
    )> {
        self.kernel.suspend_process(&mut self.executor, process)
    }

    fn copy_process_context(&self, process: TaskId) -> Option<ProcessContext> {
        self.kernel.copy_process_context(process)
    }

    /// Hands the context back, as the kernel's own resume does, also when it carries no state of
    /// this embedder.
    fn resume_process(
        &mut self,
        mut context: ProcessContext,
        config: interface::ExecutionConfig<C>,
    ) -> Option<ProcessContext> {
        let kernel_config = kernel_config(&config);
        let Some(data) = context
            .embedder_mut()
            .and_then(|data| data.value.downcast_mut::<ProcessData<C, State>>())
        else {
            return Some(context);
        };
        let before = core::mem::replace(&mut data.config, Rc::new(config));
        let refused = self.kernel.resume_process(context, kernel_config);
        refused.map(|mut context| {
            if let Some(data) = context
                .embedder_mut()
                .and_then(|data| data.value.downcast_mut::<ProcessData<C, State>>())
            {
                data.config = before;
            }
            context
        })
    }

    fn resume_tasks(
        &mut self,
        tasks: impl IntoIterator<Item = SuspendedTask>,
    ) -> impl Iterator<Item = SuspendedTask> {
        self.kernel.resume_tasks(&mut self.executor, tasks)
    }
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static> interface::Kernel
    for KernelWithExecutor<C, State>
{
}

/// One process of a [`KernelWithExecutor`], for as long as the borrow lasts.
pub struct KernelProcess<'a, C, State> {
    kernel: &'a mut KernelWithExecutor<C, State>,
    pid: Pid,
    /// The bytes of the last read.
    read: Vec<u8>,
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static> KernelProcess<'_, C, State> {
    fn handle(&mut self) -> crate::Process<'_> {
        crate::Process {
            kernel: &mut self.kernel.kernel,
            cpu: &mut self.kernel.executor,
            pid: self.pid,
        }
    }

    fn info(&self) -> crate::ProcessInfo<'_> {
        crate::ProcessInfo::of(&self.kernel.kernel, self.pid)
    }

    fn data(&self) -> &ProcessData<C, State> {
        self.kernel.kernel.processes[&self.pid]
            .embedder
            .as_ref()
            .and_then(|data| data.value.downcast_ref())
            .expect("a process handed out carries the embedder's data")
    }
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static> interface::ProcessInfo
    for KernelProcess<'_, C, State>
{
    fn mapped_memory(&self) -> u64 {
        self.info().memory_mapped()
    }

    fn address_ceiling_distance(&self) -> u64 {
        self.info().ceiling_distance()
    }

    fn cpu_time(&self) -> Duration {
        self.info().compute_used()
    }

    fn parent(&self) -> Option<TaskId> {
        self.info().parent()
    }

    /// No reader of another process's state announces itself yet, so none is counted.
    fn is_inspected(&self) -> bool {
        false
    }
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static> interface::Process
    for KernelProcess<'_, C, State>
{
    type State = State;
    type Kernel = KernelWithExecutor<C, State>;

    fn id(&self) -> TaskId {
        crate::system::task_id(self.pid)
    }

    fn execution_config(&self) -> &interface::ExecutionConfig<C> {
        &self.data().config
    }

    fn state(&mut self) -> &mut State {
        &mut self
            .kernel
            .kernel
            .processes
            .get_mut(&self.pid)
            .and_then(|process| process.embedder.as_mut())
            .and_then(|data| data.value.downcast_mut::<ProcessData<C, State>>())
            .expect("a process handed out carries the embedder's data")
            .state
    }

    fn memory_id(&self) -> MemoryId {
        self.kernel.kernel.processes[&self.pid].memory
    }

    fn read_memory(
        &mut self,
        range: Range<u64>,
    ) -> Result<impl Iterator<Item = &[u8]>, MemoryError> {
        self.read = self.handle().read_memory(range)?;
        Ok(core::iter::once(&self.read[..]))
    }

    fn write_memory(&mut self, range: Range<u64>, bytes: &[u8]) -> Result<(), WriteMemoryError> {
        if range.end.checked_sub(range.start) != Some(bytes.len() as u64) {
            return Err(WriteMemoryError::Memory(MemoryError::InvalidRange));
        }
        self.handle().write_memory(range.start, bytes)
    }

    fn watch_memory(&mut self, range: Range<u64>, watches: WatchesFor) {
        self.handle().watch_memory(range, watches);
    }

    /// Not built: no executor can watch one thread and not its siblings yet.
    fn watch_thread_memory(&mut self, thread: TaskId, range: Range<u64>, watches: WatchesFor)
    where
        C: PerThreadWatch,
    {
        let _ = (thread, range, watches);
        unimplemented!("a watch of one thread is not built")
    }

    fn take_dirty_memory(&mut self) -> Vec<Range<u64>> {
        self.handle().take_dirty_memory()
    }

    fn send_signal(&mut self, thread: TaskId, signal: SignalInformation) {
        self.handle().send_signal(thread, signal);
    }

    fn suspend_tasks(&mut self, tasks: &[TaskId]) -> impl Iterator<Item = (TaskId, SuspendedTask)> {
        self.handle().suspend_tasks(tasks)
    }

    fn suspend_process(
        mut self,
    ) -> (
        ProcessContext,
        impl Iterator<Item = (TaskId, SuspendedTask)>,
    ) {
        let (context, tasks) = self.handle().suspend_process();
        (context, tasks)
    }

    fn copy_process_context(&self) -> ProcessContext {
        self.kernel.kernel.context_copy(self.pid)
    }
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static> interface::ProcessOf<State, C>
    for KernelProcess<'_, C, State>
{
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An executor with nothing to configure.
    struct Plain;

    impl Executor for Plain {
        type TranslatorConfig = ();
    }

    #[test]
    fn a_copy_has_its_own_state_and_shares_the_configuration() {
        let data = EmbedderData::new(ProcessData::<Plain, u32> {
            config: Rc::new(interface::ExecutionConfig {
                thread_ordering: interface::ThreadOrdering::Serial,
                replay_determinism: interface::ReplayDeterminism::Exact,
                translator: (),
            }),
            state: 1,
        });
        let mut copy = data.clone();
        let copied = copy
            .value
            .downcast_mut::<ProcessData<Plain, u32>>()
            .expect("the same type");
        copied.state += 1;
        let original = data
            .value
            .downcast_ref::<ProcessData<Plain, u32>>()
            .expect("the same type");
        assert_eq!((original.state, copied.state), (1, 2));
        assert!(Rc::ptr_eq(&original.config, &copied.config));
    }
}
