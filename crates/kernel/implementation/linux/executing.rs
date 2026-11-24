//! The kernel as the kernel interface sees it: with the executor its threads run on, and with the
//! embedder's state and execution configuration of each process kept beside the process's record.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;
use core::marker::PhantomData;
use core::num::NonZeroUsize;
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
/// come one at a time. The contexts and tasks it hands out share its records, so each is let go of
/// under that lock too ([`Held`]).
pub struct KernelWithExecutor<C, State, S> {
    shared: Arc<Shared<C>>,
    types: PhantomData<fn() -> (State, S)>,
}

/// What a kernel and the contexts and tasks it handed out share: its state behind one lock, and
/// what was let go of while another held that lock, for the next holder to drop.
struct Shared<C> {
    inner: spin::Mutex<Inner<C>>,
    let_go: spin::Mutex<Vec<Box<dyn Any>>>,
}

/// What [`KernelWithExecutor`]'s lock guards.
struct Inner<C> {
    kernel: crate::Kernel,
    executor: C,
}

impl<C> Shared<C> {
    /// Takes the lock, then drops what was let go of while another held it.
    fn lock(&self) -> spin::MutexGuard<'_, Inner<C>> {
        let guard = self.inner.lock();
        let let_go = core::mem::take(&mut *self.let_go.lock());
        drop(let_go);
        guard
    }

    /// `value`, which shares the kernel's records, handed out.
    fn hold<T: 'static>(self: &Arc<Self>, value: T) -> Held<T, C> {
        Held {
            value: Some(value),
            shared: Arc::clone(self),
        }
    }
}

// SAFETY: every use of the kernel's state and its executor holds the lock, and so does every drop
// of a context or task it handed out (`Held`, which is not `Send`), the only values outside the
// lock that share its records. So what they share through `Rc` and `RefCell` is touched by one
// host thread at a time, and the rest may move between host threads: the executor, the
// embedder's states and execution configurations, and the system with its handles are `Send`,
// and the system's waiting calls the kernel keeps were promised to be when it was started
// (`start`).
unsafe impl<C, State, S> Sync for KernelWithExecutor<C, State, S>
where
    C: Executor<TranslatorConfig: Send> + Send,
    State: Send,
    S: crate::System + Send,
    <S as wcontainers_tcp_sockets::TcpConnections>::Connection: Send,
    <S as wcontainers_tcp_sockets::TcpListeners>::Listener: Send,
    <S as wcontainers_udp_sockets::UdpSockets>::Socket: Send,
    <S as wcontainers_system::StreamTransport>::Endpoint: Send,
{
}

/// A context or task a [`KernelWithExecutor`] handed out. It shares the kernel's records, so it is
/// dropped under the kernel's lock, or, while that lock is held, by the lock's next holder.
pub struct Held<T: 'static, C> {
    /// Always there, until the kernel takes it back.
    value: Option<T>,
    shared: Arc<Shared<C>>,
}

impl<T: 'static, C> Held<T, C> {
    /// The value, if it was handed out by the kernel `shared` is of; else itself.
    fn take_from(mut self, shared: &Arc<Shared<C>>) -> Result<T, Self> {
        if !Arc::ptr_eq(&self.shared, shared) {
            return Err(self);
        }
        Ok(self.value.take().expect("a held value is there"))
    }
}

impl<T: 'static, C> core::ops::Deref for Held<T, C> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value.as_ref().expect("a held value is there")
    }
}

impl<T: 'static, C> Drop for Held<T, C> {
    fn drop(&mut self) {
        let Some(value) = self.value.take() else {
            return;
        };
        match self.shared.inner.try_lock() {
            Some(guard) => {
                drop(value);
                drop(guard);
            }
            None => self.shared.let_go.lock().push(Box::new(value)),
        }
    }
}

/// What a kernel is made from: a fresh kernel, the executor its threads run on, its system, the
/// container's start, and its first process's execution configuration and state.
pub struct KernelConfig<C: Executor, State, S: crate::System> {
    kernel: crate::Kernel,
    executor: C,
    system: Arc<S>,
    start: crate::Start<'static, <S as wcontainers_system::StreamTransport>::Endpoint>,
    execution: interface::ExecutionConfig<C>,
    state: State,
}

impl<C: Executor, State, S: crate::System> KernelConfig<C, State, S> {
    /// What [`KernelWithExecutor::start`] is given.
    ///
    /// # Safety
    ///
    /// As for [`KernelWithExecutor::start`].
    pub unsafe fn new(
        kernel: crate::Kernel,
        executor: C,
        system: Arc<S>,
        start: crate::Start<'static, <S as wcontainers_system::StreamTransport>::Endpoint>,
        execution: interface::ExecutionConfig<C>,
        state: State,
    ) -> Self {
        Self {
            kernel,
            executor,
            system,
            start,
            execution,
            state,
        }
    }
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static, S: crate::System>
    KernelWithExecutor<C, State, S>
{
    /// Starts a container on `kernel`, as [`crate::Kernel::start`], its first process with
    /// `state` and run as `config` says, unless the kernel cannot run it so.
    ///
    /// # Safety
    ///
    /// Every waiting call of `system` (each future its `async` calls return) may be polled and
    /// dropped on a host thread other than the one that made it, as if it were `Send`: the
    /// kernel keeps such calls, and its interface's calls may come from any host thread.
    pub unsafe fn start(
        mut kernel: crate::Kernel,
        mut executor: C,
        system: &Arc<S>,
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
            shared: Arc::new(Shared {
                inner: spin::Mutex::new(Inner { kernel, executor }),
                let_go: spin::Mutex::new(Vec::new()),
            }),
            types: PhantomData,
        })
    }

    /// Drives the kernel and the executor with `drive`, under the kernel's lock.
    pub fn with_parts<R>(&mut self, drive: impl FnOnce(&mut crate::Kernel, &mut C) -> R) -> R {
        let mut inner = self.shared.lock();
        let Inner { kernel, executor } = &mut *inner;
        drive(kernel, executor)
    }

    /// One process, unless it has ended or carries no state of this embedder. The kernel's lock
    /// is held while the handle lasts.
    pub fn process(&mut self, id: TaskId) -> Option<KernelProcess<'_, C, State, S>> {
        let pid = pid_of(id);
        let inner = self.shared.lock();
        inner
            .kernel
            .processes
            .get(&pid)
            .filter(|process| process.ended.is_none())?
            .embedder
            .as_ref()?
            .value
            .downcast_ref::<ProcessData<C, State>>()?;
        Some(KernelProcess {
            inner,
            shared: &self.shared,
            pid,
            read: Vec::new(),
            types: PhantomData,
        })
    }
}

impl<C, State, S> interface::Kernel for KernelWithExecutor<C, State, S>
where
    C: Cpu + Executor<TranslatorConfig: Send> + Send + 'static,
    State: Clone + Send + 'static,
    S: crate::System + Send,
    <S as wcontainers_tcp_sockets::TcpConnections>::Connection: Send,
    <S as wcontainers_tcp_sockets::TcpListeners>::Listener: Send,
    <S as wcontainers_udp_sockets::UdpSockets>::Socket: Send,
    <S as wcontainers_system::StreamTransport>::Endpoint: Send,
{
    type Executor = C;
    type Fallback = wcontainers_abi::Warning<'static>;
    type Config = KernelConfig<C, State, S>;
    type Runner = KernelRunner<C, State, S>;
    type SuspendedTask = Held<SuspendedTask, C>;
    type ProcessContext = Held<ProcessContext, C>;

    /// Starts the container `config` describes, its programs told that `parallelism` processors
    /// are online. Its threads run one at a time, which is within any `parallelism`.
    ///
    /// # Panics
    ///
    /// If the kernel cannot run the first process as its execution configuration asks.
    fn new(
        config: Self::Config,
        parallelism: NonZeroUsize,
    ) -> (Self, impl Iterator<Item = Self::Runner>) {
        let KernelConfig {
            mut kernel,
            executor,
            system,
            start,
            execution,
            state,
        } = config;
        kernel.machine.processors = parallelism;
        // SAFETY: the config was made by `KernelConfig::new`, whose caller promised what `start`
        // asks.
        let kernel = unsafe { Self::start(kernel, executor, &system, start, execution, state) }
            .unwrap_or_else(|unsupported| {
                panic!("the kernel cannot run as asked: {unsupported:?}")
            });
        let shared = Arc::clone(&kernel.shared);
        let runners = (0..parallelism.get()).map(move |_| KernelRunner {
            shared: Arc::clone(&shared),
            types: PhantomData,
        });
        (kernel, runners)
    }

    fn suspend_tasks(
        &self,
        tasks: &[TaskId],
    ) -> impl Iterator<Item = (TaskId, Self::SuspendedTask)> {
        let mut inner = self.shared.lock();
        let Inner { kernel, executor } = &mut *inner;
        let taken: Vec<_> = kernel
            .suspend_tasks(executor, tasks)
            .map(|(id, task)| (id, self.shared.hold(task)))
            .collect();
        drop(inner);
        taken.into_iter()
    }

    fn suspend_process(
        &self,
        process: TaskId,
    ) -> Option<(
        Self::ProcessContext,
        impl Iterator<Item = (TaskId, Self::SuspendedTask)>,
    )> {
        let mut inner = self.shared.lock();
        let Inner { kernel, executor } = &mut *inner;
        let (context, tasks) = kernel.suspend_process(executor, process)?;
        let tasks: Vec<_> = tasks
            .map(|(id, task)| (id, self.shared.hold(task)))
            .collect();
        let context = self.shared.hold(context);
        drop(inner);
        Some((context, tasks.into_iter()))
    }

    fn copy_process_context(&self, process: TaskId) -> Option<Self::ProcessContext> {
        let inner = self.shared.lock();
        let copy = inner.kernel.copy_process_context(process)?;
        let copy = self.shared.hold(copy);
        drop(inner);
        Some(copy)
    }

    /// Hands the context back, as the kernel's own resume does, also when another kernel handed
    /// it out, it carries no state of this embedder, or the kernel cannot run it as `config` says.
    fn resume_process(
        &self,
        context: Self::ProcessContext,
        config: interface::ExecutionConfig<C>,
    ) -> Option<Self::ProcessContext> {
        if supported(&config).is_err() {
            return Some(context);
        }
        let mut inner = self.shared.lock();
        let mut context = match context.take_from(&self.shared) {
            Ok(context) => context,
            Err(foreign) => return Some(foreign),
        };
        let Some(data) = context
            .embedder_mut()
            .and_then(|data| data.value.downcast_mut::<ProcessData<C, State>>())
        else {
            return Some(self.shared.hold(context));
        };
        let before = core::mem::replace(&mut data.config, Rc::new(config));
        let refused = inner.kernel.resume_process(context).map(|mut context| {
            if let Some(data) = context
                .embedder_mut()
                .and_then(|data| data.value.downcast_mut::<ProcessData<C, State>>())
            {
                data.config = before;
            }
            self.shared.hold(context)
        });
        drop(inner);
        refused
    }

    fn resume_tasks(
        &self,
        tasks: impl IntoIterator<Item = Self::SuspendedTask>,
    ) -> impl Iterator<Item = Self::SuspendedTask> {
        // The caller's iterator runs before the lock is taken, since it may do anything.
        let tasks: Vec<Self::SuspendedTask> = tasks.into_iter().collect();
        let mut inner = self.shared.lock();
        let mut handed_back = Vec::new();
        let mut own = Vec::new();
        for task in tasks {
            match task.take_from(&self.shared) {
                Ok(task) => own.push(task),
                Err(foreign) => handed_back.push(foreign),
            }
        }
        let Inner { kernel, executor } = &mut *inner;
        handed_back.extend(
            kernel
                .resume_tasks(executor, own)
                .map(|task| self.shared.hold(task)),
        );
        drop(inner);
        handed_back.into_iter()
    }
}

/// What runs a [`KernelWithExecutor`]'s threads on one host thread.
pub struct KernelRunner<C, State, S> {
    shared: Arc<Shared<C>>,
    types: PhantomData<fn() -> (State, S)>,
}

// SAFETY: as for `KernelWithExecutor`'s `Sync`: a runner reaches the kernel's state only under its
// lock, and the bounds are the same.
unsafe impl<C, State, S> Send for KernelRunner<C, State, S>
where
    C: Executor<TranslatorConfig: Send> + Send,
    State: Send,
    S: crate::System + Send,
    <S as wcontainers_tcp_sockets::TcpConnections>::Connection: Send,
    <S as wcontainers_tcp_sockets::TcpListeners>::Listener: Send,
    <S as wcontainers_udp_sockets::UdpSockets>::Socket: Send,
    <S as wcontainers_system::StreamTransport>::Endpoint: Send,
{
}

impl<C, State, S> interface::Runner<C> for KernelRunner<C, State, S>
where
    C: Executor<TranslatorConfig: Send> + Send,
    State: Send,
    S: crate::System + Send,
    <S as wcontainers_tcp_sockets::TcpConnections>::Connection: Send,
    <S as wcontainers_tcp_sockets::TcpListeners>::Listener: Send,
    <S as wcontainers_udp_sockets::UdpSockets>::Socket: Send,
    <S as wcontainers_system::StreamTransport>::Endpoint: Send,
{
    /// Not built: the executor interface has no call that runs a thread, so this kernel is driven
    /// through [`crate::Kernel::next`] and [`crate::Kernel::exit`] on the executor it holds
    /// ([`KernelWithExecutor::with_parts`]).
    fn run(&mut self, executor: &mut C) -> interface::StopReason {
        let _ = (executor, &self.shared);
        unimplemented!("running through the kernel interface is not built")
    }
}

/// One process of a [`KernelWithExecutor`], holding the kernel's lock for as long as it lasts.
pub struct KernelProcess<'a, C, State, S> {
    inner: spin::MutexGuard<'a, Inner<C>>,
    shared: &'a Arc<Shared<C>>,
    pid: Pid,
    /// The bytes of the last read.
    read: Vec<u8>,
    types: PhantomData<fn() -> (State, S)>,
}

impl<C: Cpu + Executor + 'static, State: Clone + 'static, S> KernelProcess<'_, C, State, S> {
    fn handle(&mut self) -> crate::Process<'_> {
        let inner = &mut *self.inner;
        crate::Process {
            kernel: &mut inner.kernel,
            cpu: &mut inner.executor,
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
    C: Cpu + Executor<TranslatorConfig: Send> + Send + 'static,
    State: Clone + Send + 'static,
    S: crate::System + Send,
    <S as wcontainers_tcp_sockets::TcpConnections>::Connection: Send,
    <S as wcontainers_tcp_sockets::TcpListeners>::Listener: Send,
    <S as wcontainers_udp_sockets::UdpSockets>::Socket: Send,
    <S as wcontainers_system::StreamTransport>::Endpoint: Send,
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
    ) -> impl Iterator<Item = (TaskId, Held<SuspendedTask, C>)> {
        let shared = self.shared;
        let taken: Vec<_> = self
            .handle()
            .suspend_tasks(threads)
            .map(|(id, task)| (id, shared.hold(task)))
            .collect();
        taken.into_iter()
    }

    fn suspend_process(
        mut self,
    ) -> (
        Held<ProcessContext, C>,
        impl Iterator<Item = (TaskId, Held<SuspendedTask, C>)>,
    ) {
        let shared = self.shared;
        let (context, tasks) = self.handle().suspend_process();
        let tasks: Vec<_> = tasks.map(|(id, task)| (id, shared.hold(task))).collect();
        (shared.hold(context), tasks.into_iter())
    }

    fn copy_process_context(&self) -> Held<ProcessContext, C> {
        self.shared.hold(self.kernel().context_copy(self.pid))
    }
}

impl<C, State, S> interface::ProcessOf<State, KernelWithExecutor<C, State, S>>
    for KernelProcess<'_, C, State, S>
where
    C: Cpu + Executor<TranslatorConfig: Send> + Send + 'static,
    State: Clone + Send + 'static,
    S: crate::System + Send,
    <S as wcontainers_tcp_sockets::TcpConnections>::Connection: Send,
    <S as wcontainers_tcp_sockets::TcpListeners>::Listener: Send,
    <S as wcontainers_udp_sockets::UdpSockets>::Socket: Send,
    <S as wcontainers_system::StreamTransport>::Endpoint: Send,
{
}

#[cfg(test)]
mod tests {
    use super::*;

    use wcontainers_executor::memory_space::{
        CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, VolatileBytes,
    };

    /// An executor with nothing to configure, which has no memory to run on.
    struct Plain;

    /// What `Plain` has no instance of: its memory, that memory's bytes and an interrupter.
    enum Absent {}

    impl Absent {
        /// Any value at all, as there is no `Absent` to ask.
        fn anything<T>(&self) -> T {
            match *self {}
        }
    }

    impl MemorySpace for Absent {
        type Bytes<'space> = Absent;

        fn borrow(&self, _addresses: Range<u64>) -> Result<Absent, InvalidAddressError> {
            match *self {}
        }

        fn set_permissions(
            &self,
            _addresses: Range<u64>,
            _allowed: wcontainers_abi::Permissions,
        ) -> Result<(), InvalidAddressError> {
            match *self {}
        }

        fn map_from(
            &self,
            _addresses: Range<u64>,
            _other: &Self,
            _other_start: u64,
        ) -> Result<(), MapMemoryError> {
            match *self {}
        }

        fn unmap_from(&self, _addresses: Range<u64>) -> Result<(), InvalidAddressError> {
            match *self {}
        }

        fn copy_from(
            &self,
            _addresses: Range<u64>,
            _other: &Self,
            _other_start: u64,
        ) -> Result<(), CopyMemoryError> {
            match *self {}
        }
    }

    impl VolatileBytes for Absent {
        fn len(&self) -> u64 {
            match *self {}
        }

        fn read(&self, _into: &mut [u8]) -> usize {
            match *self {}
        }

        fn write(&self, _bytes: &[u8]) -> usize {
            match *self {}
        }
    }

    impl wcontainers_executor::ExecutionInterrupter for Absent {
        fn interrupt(&self) {
            match *self {}
        }
    }

    /// Why `Plain` gives no memory.
    #[derive(Debug, PartialEq, Eq)]
    struct NoMemory;

    impl Executor for Plain {
        type Architecture = wcontainers_processor_architecture_arm64::Arm64;
        type TranslatorConfig = ();
        type GuestMemory = Absent;
        type Interrupter = Absent;
        type CreateGuestMemoryError = NoMemory;

        fn create_guest_memory(&self) -> Result<Absent, NoMemory> {
            Err(NoMemory)
        }

        fn execute<'execution>(
            &'execution mut self,
            _processor: wcontainers_processor_architecture_arm64::Processor,
            memory: &'execution Absent,
            _handler: &'execution mut impl wcontainers_executor::TrapHandler,
        ) -> (
            impl core::future::Future<Output = wcontainers_processor_architecture_arm64::Processor>
            + 'execution,
            Absent,
        ) {
            (core::future::ready(memory.anything()), memory.anything())
        }

        fn execute_single_instruction_in_place(
            &mut self,
            _processor: &mut wcontainers_processor_architecture_arm64::Processor,
            _memory: &[wcontainers_processor_architecture::MemoryDelta<'_, Self::Architecture>],
            _on_memory_mutated: impl FnMut(
                wcontainers_processor_architecture::MemoryDelta<'_, Self::Architecture>,
            ),
        ) {
            unreachable!("the kernel's tests use Plain only for its translator settings")
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
