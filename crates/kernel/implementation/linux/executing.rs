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
use crate::system::{Unsupported, pid_of};

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

/// Whether the kernel can run a process as `config` says: serially, with exact replay.
fn supported<E: Executor>(config: &interface::ExecutionConfig<E>) -> Result<(), Unsupported> {
    if config.thread_ordering != interface::ThreadOrdering::Serial {
        return Err(Unsupported::ThreadOrdering(config.thread_ordering));
    }
    if config.replay_determinism != interface::ReplayDeterminism::Exact {
        return Err(Unsupported::ReplayDeterminism(config.replay_determinism));
    }
    Ok(())
}

/// A kernel with the executor its threads run on, each process with the embedder's `State`, for
/// the system `S`.
///
/// Its state sits behind one lock, so that the kernel interface's calls, which take it shared,
/// come one at a time. The state is not yet shareable between host threads (it holds `Rc` and
/// `RefCell` throughout, and the system's waiting calls), so a kernel and what it hands out stay
/// on one host thread at a time.
pub struct KernelWithExecutor<C, State, S> {
    inner: spin::Mutex<Inner<C>>,
    types: PhantomData<fn() -> (State, S)>,
}

/// What [`KernelWithExecutor`]'s lock guards.
struct Inner<C> {
    kernel: crate::Kernel,
    executor: C,
}

// SAFETY: every access to the kernel's state and its executor goes through the lock (shared
// calls) or through an exclusive borrow. What they share through `Rc` stays inside them except in
// the contexts and tasks the kernel hands out, which an embedder keeps on the host thread it runs
// the kernel on, as it keeps the kernel itself: neither the state nor an executor such as the
// stand-in's is shareable between host threads yet (see the type's comment).
unsafe impl<C, State: Send, S: Send + Sync> Sync for KernelWithExecutor<C, State, S> {}

/// What a kernel is made from: a fresh kernel, the executor its threads run on, its system, the
/// container's start, and its first process's execution configuration and state.
pub struct KernelConfig<C: Executor, State, S: crate::System> {
    pub kernel: crate::Kernel,
    pub executor: C,
    pub system: alloc::sync::Arc<S>,
    pub start: crate::Start<'static, <S as wcontainers_system::StreamTransport>::Endpoint>,
    pub execution: interface::ExecutionConfig<C>,
    pub state: State,
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static, S: crate::System>
    KernelWithExecutor<C, State, S>
{
    /// Starts a container on `kernel`, as [`crate::Kernel::start`], its first process with
    /// `state` and run as `config` says, unless the kernel cannot run it so.
    pub fn start(
        mut kernel: crate::Kernel,
        mut executor: C,
        system: &alloc::sync::Arc<S>,
        start: crate::Start<'_, <S as wcontainers_system::StreamTransport>::Endpoint>,
        config: interface::ExecutionConfig<C>,
        state: State,
    ) -> Result<Self, Unsupported> {
        supported(&config)?;
        kernel.start(&mut executor, system, start);
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
            inner: spin::Mutex::new(Inner { kernel, executor }),
            types: PhantomData,
        })
    }

    /// The kernel and the executor, to drive them.
    pub fn parts(&mut self) -> (&mut crate::Kernel, &mut C) {
        let inner = self.inner.get_mut();
        (&mut inner.kernel, &mut inner.executor)
    }

    /// One process, unless it has ended or carries no state of this embedder.
    pub fn process(&mut self, id: TaskId) -> Option<KernelProcess<'_, C, State, S>> {
        let pid = pid_of(id);
        self.inner
            .get_mut()
            .kernel
            .processes
            .get(&pid)
            .filter(|process| process.ended.is_none())?
            .embedder
            .as_ref()?
            .value
            .downcast_ref::<ProcessData<C, State>>()?;
        Some(KernelProcess {
            inner: self.inner.get_mut(),
            pid,
            read: Vec::new(),
            types: PhantomData,
        })
    }
}

impl<C, State, S> interface::Kernel for KernelWithExecutor<C, State, S>
where
    C: Cpu + Executor + 'static,
    State: Clone + Send + 'static,
    S: crate::System + Send,
{
    type Executor = C;
    type Fallback = wcontainers_abi::Warning<'static>;
    type Config = KernelConfig<C, State, S>;
    type SuspendedTask = SuspendedTask;
    type ProcessContext = ProcessContext;

    /// Starts the container `config` describes.
    ///
    /// # Panics
    ///
    /// If the kernel cannot run the first process as its execution configuration asks.
    fn new(config: Self::Config) -> Self {
        let KernelConfig {
            kernel,
            executor,
            system,
            start,
            execution,
            state,
        } = config;
        Self::start(kernel, executor, &system, start, execution, state)
            .unwrap_or_else(|unsupported| panic!("the kernel cannot run as asked: {unsupported:?}"))
    }

    /// Not built: the executor interface has no call that runs a thread, so this kernel is driven
    /// through [`crate::Kernel::next`] and [`crate::Kernel::exit`] on the executor it holds.
    fn run(&self, executor: &mut C) -> interface::StopReason {
        let _ = executor;
        unimplemented!("running through the kernel interface is not built")
    }

    fn suspend_tasks(&self, tasks: &[TaskId]) -> impl Iterator<Item = (TaskId, SuspendedTask)> {
        let mut inner = self.inner.lock();
        let Inner { kernel, executor } = &mut *inner;
        kernel.suspend_tasks(executor, tasks)
    }

    fn suspend_process(
        &self,
        process: TaskId,
    ) -> Option<(
        ProcessContext,
        impl Iterator<Item = (TaskId, SuspendedTask)>,
    )> {
        let mut inner = self.inner.lock();
        let Inner { kernel, executor } = &mut *inner;
        kernel.suspend_process(executor, process)
    }

    fn copy_process_context(&self, process: TaskId) -> Option<ProcessContext> {
        self.inner.lock().kernel.copy_process_context(process)
    }

    /// Hands the context back, as the kernel's own resume does, also when it carries no state of
    /// this embedder or the kernel cannot run it as `config` says.
    fn resume_process(
        &self,
        mut context: ProcessContext,
        config: interface::ExecutionConfig<C>,
    ) -> Option<ProcessContext> {
        if supported(&config).is_err() {
            return Some(context);
        }
        let Some(data) = context
            .embedder_mut()
            .and_then(|data| data.value.downcast_mut::<ProcessData<C, State>>())
        else {
            return Some(context);
        };
        let before = core::mem::replace(&mut data.config, Rc::new(config));
        let refused = self.inner.lock().kernel.resume_process(context);
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
        &self,
        tasks: impl IntoIterator<Item = SuspendedTask>,
    ) -> impl Iterator<Item = SuspendedTask> {
        let mut inner = self.inner.lock();
        let Inner { kernel, executor } = &mut *inner;
        kernel.resume_tasks(executor, tasks)
    }
}

/// One process of a [`KernelWithExecutor`], for as long as the borrow lasts.
pub struct KernelProcess<'a, C, State, S> {
    inner: &'a mut Inner<C>,
    pid: Pid,
    /// The bytes of the last read.
    read: Vec<u8>,
    types: PhantomData<fn() -> (State, S)>,
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static, S> KernelProcess<'_, C, State, S> {
    fn handle(&mut self) -> crate::Process<'_> {
        crate::Process {
            kernel: &mut self.inner.kernel,
            cpu: &mut self.inner.executor,
            pid: self.pid,
        }
    }

    fn kernel(&self) -> &crate::Kernel {
        &self.inner.kernel
    }

    fn info(&self) -> crate::ProcessInfo<'_> {
        crate::ProcessInfo::of(self.kernel(), self.pid)
    }

    fn data(&self) -> &ProcessData<C, State> {
        self.kernel().processes[&self.pid]
            .embedder
            .as_ref()
            .and_then(|data| data.value.downcast_ref())
            .expect("a process handed out carries the embedder's data")
    }
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static, S> interface::ProcessInfo
    for KernelProcess<'_, C, State, S>
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

    fn is_inspected(&self) -> bool {
        self.info().inspected()
    }
}

impl<C, State, S> interface::Process for KernelProcess<'_, C, State, S>
where
    C: Cpu + Executor + 'static,
    State: Clone + Send + 'static,
    S: crate::System + Send,
{
    type State = State;
    type Kernel = KernelWithExecutor<C, State, S>;

    fn id(&self) -> TaskId {
        crate::system::task_id(self.pid)
    }

    fn execution_config(&self) -> &interface::ExecutionConfig<C> {
        &self.data().config
    }

    fn state(&mut self) -> &mut State {
        &mut self
            .inner
            .kernel
            .processes
            .get_mut(&self.pid)
            .and_then(|process| process.embedder.as_mut())
            .and_then(|data| data.value.downcast_mut::<ProcessData<C, State>>())
            .expect("a process handed out carries the embedder's data")
            .state
    }

    fn memory_id(&self) -> MemoryId {
        self.kernel().processes[&self.pid].memory
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

    fn suspend_threads(
        &mut self,
        threads: &[TaskId],
    ) -> impl Iterator<Item = (TaskId, SuspendedTask)> {
        self.handle().suspend_tasks(threads)
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
        self.kernel().context_copy(self.pid)
    }
}

impl<C, State, S> interface::ProcessOf<State, KernelWithExecutor<C, State, S>>
    for KernelProcess<'_, C, State, S>
where
    C: Cpu + Executor + 'static,
    State: Clone + Send + 'static,
    S: crate::System + Send,
{
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An executor with nothing to configure.
    struct Plain;

    impl Executor for Plain {
        type Architecture = wcontainers_processor_architecture_arm64::Arm64;
        type TranslatorConfig = ();

        fn execute_single_instruction_in_place(
            &mut self,
            _processor: &mut wcontainers_processor_architecture_arm64::Processor,
            _memory: &[wcontainers_processor_architecture::MemoryDelta<'_, Self::Architecture>],
            _on_memory_mutated: impl FnMut(
                wcontainers_processor_architecture::MemoryDelta<'_, Self::Architecture>,
            ),
        ) {
            todo!("single instruction execution is not built for the kernel's test executor yet")
        }
    }

    /// A way of running a process.
    fn config(
        thread_ordering: interface::ThreadOrdering,
        replay_determinism: interface::ReplayDeterminism,
    ) -> interface::ExecutionConfig<Plain> {
        interface::ExecutionConfig {
            thread_ordering,
            replay_determinism,
            translator: (),
        }
    }

    #[test]
    fn only_serial_exact_runs_and_the_rest_are_refused_by_name() {
        use interface::{ReplayDeterminism, ThreadOrdering};
        assert_eq!(
            supported(&config(ThreadOrdering::Serial, ReplayDeterminism::Exact)),
            Ok(())
        );
        assert_eq!(
            supported(&config(ThreadOrdering::Parallel, ReplayDeterminism::Exact)),
            Err(Unsupported::ThreadOrdering(ThreadOrdering::Parallel))
        );
        for replay in [ReplayDeterminism::External, ReplayDeterminism::None] {
            assert_eq!(
                supported(&config(ThreadOrdering::Serial, replay)),
                Err(Unsupported::ReplayDeterminism(replay))
            );
        }
    }

    #[test]
    fn a_copy_has_its_own_state_and_shares_the_configuration() {
        let data = EmbedderData::new(ProcessData::<Plain, u32> {
            config: Rc::new(config(
                interface::ThreadOrdering::Serial,
                interface::ReplayDeterminism::Exact,
            )),
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
