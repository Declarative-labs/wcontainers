//! The kernel as the kernel interface sees it: with the executor its threads run on, and with the
//! embedder's state and execution configuration of each process kept beside the process's record.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;
use core::marker::PhantomData;
use core::num::NonZeroUsize;
use core::ops::Range;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::time::Duration;

use wcontainers_abi::{Cpu, MemoryId};
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor::{ExecutionInterrupter, Executor, PerThreadWatch};
use wcontainers_kernel as interface;
use wcontainers_kernel::signal_information::SignalInformation;
use wcontainers_kernel::{MemoryError, TaskId, WatchesFor, WriteMemoryError};

use crate::guest_machine::GuestMachine;
use crate::process::Pid;
use crate::running::{self, Exit};
use crate::suspend::{ProcessContext, SuspendedTask};
use crate::system::{Unsupported, pid_of};
use crate::task::Next;

/// What the embedder keeps of one process: its state, and how it asked for the process to run,
/// shared by the processes a fork made from it.
struct ProcessData<E: Executor, State> {
    config: Arc<interface::ExecutionConfig<E>>,
    state: State,
}

/// The embedder's data of one process, of a type only the embedder knows; a fork's child gets a
/// copy of its parent's.
pub(crate) struct EmbedderData {
    value: Box<dyn Any + Send>,
    copy: fn(&dyn Any) -> Box<dyn Any + Send>,
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
    fn new<E: Executor<TranslatorConfig: Send + Sync> + 'static, State: Clone + Send + 'static>(
        data: ProcessData<E, State>,
    ) -> Self {
        fn copy<
            E: Executor<TranslatorConfig: Send + Sync> + 'static,
            State: Clone + Send + 'static,
        >(
            value: &dyn Any,
        ) -> Box<dyn Any + Send> {
            let data = value
                .downcast_ref::<ProcessData<E, State>>()
                .expect("the data's own type");
            Box::new(ProcessData::<E, State> {
                config: Arc::clone(&data.config),
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
pub struct KernelWithExecutor<C: Executor, State, S> {
    shared: Arc<Shared<C>>,
    types: PhantomData<fn() -> (State, S)>,
}

/// What a kernel, its runners and the contexts and tasks it handed out share: its state behind one
/// lock, what was let go of while another held that lock, for the next holder to drop, and what
/// ends a guest's turn when the kernel's interface waits for that lock.
struct Shared<C: Executor> {
    inner: spin::Mutex<Inner<C>>,
    let_go: spin::Mutex<Vec<Box<dyn Any + Send>>>,
    /// The interrupter of the turn a runner runs under the lock, while it runs.
    running: spin::Mutex<Option<C::Interrupter>>,
    /// How many of the interface's calls wait for the lock; runners leave it to them.
    waiting: AtomicUsize,
}

/// What [`KernelWithExecutor`]'s lock guards.
struct Inner<C: Executor> {
    kernel: crate::Kernel,
    executor: GuestMachine<C>,
}

impl<C: Executor> Shared<C> {
    /// A kernel's shared parts, its state `inner`.
    fn new(inner: Inner<C>) -> Self {
        Self {
            inner: spin::Mutex::new(inner),
            let_go: spin::Mutex::new(Vec::new()),
            running: spin::Mutex::new(None),
            waiting: AtomicUsize::new(0),
        }
    }

    /// Takes the lock, ending the turn of a guest that runs under it, then drops what was let go
    /// of while another held it.
    fn lock(&self) -> spin::MutexGuard<'_, Inner<C>> {
        let guard = self.inner.try_lock().unwrap_or_else(|| {
            self.waiting.fetch_add(1, Ordering::SeqCst);
            if let Some(turn) = &*self.running.lock() {
                turn.interrupt();
            }
            let guard = self.inner.lock();
            self.waiting.fetch_sub(1, Ordering::SeqCst);
            guard
        });
        Self::dropped_let_go(&self.let_go);
        guard
    }

    /// Takes the lock for a runner, unless it is held or an interface call waits for it.
    fn try_lock_for_runner(&self) -> Option<spin::MutexGuard<'_, Inner<C>>> {
        if self.waiting.load(Ordering::SeqCst) != 0 {
            return None;
        }
        let guard = self.inner.try_lock()?;
        Self::dropped_let_go(&self.let_go);
        Some(guard)
    }

    /// Drops what was let go of while another held the lock.
    fn dropped_let_go(let_go: &spin::Mutex<Vec<Box<dyn Any + Send>>>) {
        let let_go = core::mem::take(&mut *let_go.lock());
        drop(let_go);
    }

    /// Keeps `turn`, the interrupter of the turn about to run under the lock, for the interface's
    /// calls to end it, and ends it at once if one already waits.
    fn turn_started(&self, turn: C::Interrupter) {
        let mut running = self.running.lock();
        if self.waiting.load(Ordering::SeqCst) != 0 {
            turn.interrupt();
        }
        *running = Some(turn);
    }

    /// Forgets the interrupter of the turn that has ended.
    fn turn_ended(&self) {
        *self.running.lock() = None;
    }

    /// `value`, which shares the kernel's records, handed out.
    fn hold<T: Send + 'static>(self: &Arc<Self>, value: T) -> Held<T, C> {
        Held {
            value: Some(value),
            shared: Arc::clone(self),
        }
    }
}

/// A context or task a [`KernelWithExecutor`] handed out. It shares the kernel's records, so it is
/// dropped under the kernel's lock, or, while that lock is held, by the lock's next holder.
pub struct Held<T: Send + 'static, C: Executor> {
    /// Always there, until the kernel takes it back.
    value: Option<T>,
    shared: Arc<Shared<C>>,
}

impl<T: Send + 'static, C: Executor> Held<T, C> {
    /// The value, if it was handed out by the kernel `shared` is of; else itself.
    fn take_from(mut self, shared: &Arc<Shared<C>>) -> Result<T, Self> {
        if !Arc::ptr_eq(&self.shared, shared) {
            return Err(self);
        }
        Ok(self.value.take().expect("a held value is there"))
    }
}

impl<T: Send + 'static, C: Executor> core::ops::Deref for Held<T, C> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value.as_ref().expect("a held value is there")
    }
}

impl<T: Send + 'static, C: Executor> Drop for Held<T, C> {
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
    pub fn new(
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

impl<
    C: Cpu + Executor<TranslatorConfig: Send + Sync> + 'static,
    State: Clone + Send + 'static,
    S: crate::System,
> KernelWithExecutor<C, State, S>
{
    /// Starts a container on `kernel`, as [`crate::Kernel::start`], its first process with
    /// `state` and run as `config` says, unless the kernel cannot run it so.
    pub fn start(
        mut kernel: crate::Kernel,
        executor: C,
        system: &Arc<S>,
        start: crate::Start<'_, <S as wcontainers_system::StreamTransport>::Endpoint>,
        config: interface::ExecutionConfig<C>,
        state: State,
    ) -> Result<Self, Unsupported> {
        supported(&config)?;
        let mut executor = GuestMachine::new(executor);
        kernel.start(&mut executor, system, start);
        let first = kernel
            .init
            .expect("a started container has a first process");
        let data = ProcessData {
            config: Arc::new(config),
            state,
        };
        if let Some(process) = kernel.processes.get_mut(&first) {
            process.embedder = Some(EmbedderData::new(data));
        }
        Ok(Self {
            shared: Arc::new(Shared::new(Inner { kernel, executor })),
            types: PhantomData,
        })
    }

    /// Drives the kernel and the executor, with the guests' memories and registers, with `drive`,
    /// under the kernel's lock.
    pub fn with_parts<R>(
        &mut self,
        drive: impl FnOnce(&mut crate::Kernel, &mut GuestMachine<C>) -> R,
    ) -> R {
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
            types: PhantomData,
        })
    }
}

impl<C, State, S> interface::Kernel for KernelWithExecutor<C, State, S>
where
    C: Cpu
        + Executor<TranslatorConfig: Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>>
        + Send
        + 'static,
    State: Clone + Send + 'static,
    S: crate::System + Send,
{
    type Executor = C;
    type Fallback = wcontainers_abi::Warning<'static>;
    type Config = KernelConfig<C, State, S>;
    type Runner = KernelRunner<C, State, S>;
    type SuspendedTask = Held<SuspendedTask, C>;
    type ProcessContext = Held<ProcessContext, C>;
    type Runners = alloc::vec::IntoIter<Self::Runner>;
    type SuspendedTasks<'kernel> = alloc::vec::IntoIter<(TaskId, Self::SuspendedTask)>;
    type UnresumedTasks<'kernel> = alloc::vec::IntoIter<Self::SuspendedTask>;

    /// Starts the container `config` describes, its programs told that `parallelism` processors
    /// are online. Its threads run one at a time, which is within any `parallelism`.
    ///
    /// # Panics
    ///
    /// If the kernel cannot run the first process as its execution configuration asks.
    fn new(config: Self::Config, parallelism: NonZeroUsize) -> (Self, Self::Runners) {
        let KernelConfig {
            mut kernel,
            executor,
            system,
            start,
            execution,
            state,
        } = config;
        kernel.machine.processors = parallelism;
        let kernel = Self::start(kernel, executor, &system, start, execution, state)
            .unwrap_or_else(|unsupported| {
                panic!("the kernel cannot run as asked: {unsupported:?}")
            });
        let shared = Arc::clone(&kernel.shared);
        let runners: Vec<_> = (0..parallelism.get())
            .map(|_| KernelRunner {
                shared: Arc::clone(&shared),
                system: Arc::clone(&system),
                types: PhantomData,
            })
            .collect();
        (kernel, runners.into_iter())
    }

    fn suspend_tasks(&self, tasks: &[TaskId]) -> Self::SuspendedTasks<'_> {
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
    ) -> Option<(Self::ProcessContext, Self::SuspendedTasks<'_>)> {
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
        let before = core::mem::replace(&mut data.config, Arc::new(config));
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
    ) -> Self::UnresumedTasks<'_> {
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
pub struct KernelRunner<C: Executor, State, S> {
    shared: Arc<Shared<C>>,
    system: Arc<S>,
    types: PhantomData<fn() -> State>,
}

impl<C, State, S> interface::Runner<C> for KernelRunner<C, State, S>
where
    C: Executor<TranslatorConfig: Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>>
        + Send
        + 'static,
    State: Send,
    S: crate::System + Send,
{
    /// Runs the kernel's threads, one turn at a time under the kernel's lock, on `executor` and
    /// the memories the kernel's own executor made, until none can run, the container ends, or
    /// one of the kernel interface's calls waits for the lock (`Interrupted`; the turn running
    /// then is ended). While another runner holds the lock, none can run on this one: it returns
    /// `Idle` with no time.
    ///
    /// A turn runs until its thread traps: a time slice the kernel asks for is not kept, as
    /// an execution takes none.
    fn run(&mut self, executor: &mut C) -> interface::StopReason {
        let mut timer_told = false;
        loop {
            let Some(mut inner) = self.shared.try_lock_for_runner() else {
                return if self.shared.waiting.load(Ordering::SeqCst) == 0 {
                    interface::StopReason::Idle { until: None }
                } else {
                    interface::StopReason::Interrupted
                };
            };
            let Inner {
                kernel,
                executor: machine,
            } = &mut *inner;
            match kernel.next() {
                Next::Run { thread, resume, .. } => {
                    kernel.ready_turn(machine);
                    let (_, registers, memory) = machine.thread_parts(thread);
                    let exit = running::turn(executor, registers, memory, resume, |turn| {
                        self.shared.turn_started(turn);
                    });
                    self.shared.turn_ended();
                    kernel.exit(machine, &self.system, exit);
                    if exit == Exit::Stop {
                        return interface::StopReason::Interrupted;
                    }
                }
                // The kernel's waits to settle, or the time it waits for may have come.
                Next::Idle {
                    until_nanoseconds: Some(0),
                } => kernel.fire_timers(machine, &self.system),
                Next::Idle {
                    until_nanoseconds: Some(_),
                } if !timer_told => {
                    timer_told = true;
                    kernel.fire_timers(machine, &self.system);
                }
                Next::Idle { until_nanoseconds } => {
                    return interface::StopReason::Idle {
                        until: until_nanoseconds.map(Duration::from_nanos),
                    };
                }
                Next::Finished { status } => {
                    return interface::StopReason::Finished(process_end(
                        status,
                        kernel.start_failure(),
                    ));
                }
            }
        }
    }
}

/// How the container's first process ended, from its wait status `status` and the error that kept
/// its program from starting, if one did.
fn process_end(
    status: i32,
    start_failure: Option<interface::errno::Errno>,
) -> interface::ProcessEnd {
    if let Some(error) = start_failure {
        return interface::ProcessEnd::NotStarted { error };
    }
    match crate::wait::decode(status) {
        crate::wait::Ended::Exited(code) => interface::ProcessEnd::Exited {
            code: u8::try_from(code).expect("an exit status is its low byte"),
        },
        crate::wait::Ended::Killed {
            signal,
            core_dumped,
        } => interface::ProcessEnd::Signaled {
            signal: u32::try_from(signal)
                .ok()
                .and_then(crate::siginfo::signal)
                .expect("a process ends by a signal Linux names"),
            core_dumped,
        },
    }
}

/// One process of a [`KernelWithExecutor`], holding the kernel's lock for as long as it lasts.
pub struct KernelProcess<'a, C: Executor, State, S> {
    inner: spin::MutexGuard<'a, Inner<C>>,
    shared: &'a Arc<Shared<C>>,
    pid: Pid,
    types: PhantomData<fn() -> (State, S)>,
}

impl<C: Executor + 'static, State: Clone + 'static, S> KernelProcess<'_, C, State, S> {
    fn handle(&mut self) -> crate::Process<'_, C> {
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

impl<C: Executor + 'static, State: Clone + 'static, S> interface::ProcessInfo
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
    C: Cpu
        + Executor<TranslatorConfig: Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>>
        + Send
        + 'static,
    State: Clone + Send + 'static,
    S: crate::System + Send,
{
    type State = State;
    type Kernel = KernelWithExecutor<C, State, S>;
    type SuspendedTasks = alloc::vec::IntoIter<(TaskId, Held<SuspendedTask, C>)>;

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

    /// Hands the range to `reader` as one part.
    fn read_memory(
        &mut self,
        range: Range<u64>,
        reader: &mut impl interface::MemoryReader,
    ) -> Result<(), MemoryError> {
        reader.read_part(&self.handle().read_memory(range)?);
        Ok(())
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

    /// Sets the watch in the kernel itself, which needs no executor's watch of one thread.
    ///
    /// The kernel runs one thread of a memory at a time, and before each turn its memory's pages
    /// allow what that thread's watches leave.
    fn watch_thread_memory(&mut self, thread: TaskId, range: Range<u64>, watches: WatchesFor)
    where
        C: PerThreadWatch,
    {
        self.handle().watch_thread_memory(thread, range, watches);
    }

    fn take_dirty_memory(&mut self) -> Vec<Range<u64>> {
        self.handle().take_dirty_memory()
    }

    fn send_signal(&mut self, thread: TaskId, signal: SignalInformation) {
        self.handle().send_signal(thread, signal);
    }

    fn suspend_threads(&mut self, threads: &[TaskId]) -> Self::SuspendedTasks {
        let shared = self.shared;
        let taken: Vec<_> = self
            .handle()
            .suspend_tasks(threads)
            .map(|(id, task)| (id, shared.hold(task)))
            .collect();
        taken.into_iter()
    }

    fn suspend_process(mut self) -> (Held<ProcessContext, C>, Self::SuspendedTasks) {
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
    C: Cpu
        + Executor<TranslatorConfig: Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>>
        + Send
        + 'static,
    State: Clone + Send + 'static,
    S: crate::System + Send,
{
}

#[cfg(test)]
mod tests {
    use super::*;

    use wcontainers_behavior_source::BehaviorSource;
    use wcontainers_byte_sink::ByteSink;
    use wcontainers_byte_source::ByteSource;
    use wcontainers_executor::memory_space::{
        CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
    };
    use wcontainers_processor_architecture::{MemorySink, MemorySnapshot};
    use wcontainers_processor_architecture_arm64::{Arm64, User};
    use wcontainers_volatile_bytes::VolatileBytes;

    /// The processor's state of a thread at EL0.
    type Processor = wcontainers_processor_architecture_arm64::Processor<()>;

    /// An executor with nothing to configure, which has no memory to run on.
    struct Plain;

    /// What `Plain` has no instance of: its memory, bytes, pages, execution and interrupter.
    enum Absent {}

    impl Absent {
        /// Any value at all, as there is no `Absent` to ask.
        fn anything<T>(&self) -> T {
            match *self {}
        }
    }

    impl MemorySpace for Absent {
        type Bytes<'space> = Absent;
        type PageTable = Absent;

        fn borrow(&self, _addresses: Range<u64>) -> Result<Absent, InvalidAddressError> {
            match *self {}
        }
    }

    impl PageTable for Absent {
        fn set_permissions(
            &mut self,
            _addresses: Range<u64>,
            _allowed: wcontainers_abi::Permissions,
        ) -> Result<(), InvalidAddressError> {
            match *self {}
        }

        fn map_from(
            &mut self,
            _addresses: Range<u64>,
            _other: &Self,
            _other_start: u64,
        ) -> Result<(), MapMemoryError> {
            match *self {}
        }

        fn unmap_from(&mut self, _addresses: Range<u64>) -> Result<(), InvalidAddressError> {
            match *self {}
        }

        fn copy_from(
            &mut self,
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

        fn read_to<Sink: ByteSink>(&self, _sink: &mut Sink) -> Result<usize, Sink::Error> {
            match *self {}
        }

        fn write_from<Source: ByteSource>(
            &self,
            _source: &mut Source,
        ) -> Result<usize, Source::Error> {
            match *self {}
        }
    }

    impl wcontainers_executor::Execution for Absent {
        type ProcessorState = Processor;
        type Wait = core::future::Ready<()>;

        fn run(
            self,
            _handler: &mut impl wcontainers_executor::TrapHandler,
        ) -> wcontainers_executor::ExecutionStop<Processor, Self::Wait> {
            match self {}
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

    impl BehaviorSource for Plain {
        type Architecture = Arm64<User>;
        type Error = core::convert::Infallible;

        fn execute_single_instruction_in_place(
            &mut self,
            _processor: &mut Processor,
            _memory: MemorySnapshot<'_, '_, Self::Architecture>,
            _mutation_hook: &mut impl MemorySink<Self::Architecture>,
        ) -> Result<(), Self::Error> {
            unreachable!("the kernel's tests use Plain only for its translator settings")
        }
    }

    impl Executor for Plain {
        type TranslatorConfig = ();
        type GuestMemory = Absent;
        type Interrupter = Absent;
        type CreateGuestMemoryError = NoMemory;
        type Execution<'execution> = Absent;

        fn create_guest_memory(&self) -> Result<(Absent, Absent), NoMemory> {
            Err(NoMemory)
        }

        fn prepare_execution<'execution>(
            &'execution mut self,
            _processor: Processor,
            memory: &'execution Absent,
        ) -> (Absent, Absent) {
            (memory.anything(), memory.anything())
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
            config: Arc::new(config(
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
        assert!(Arc::ptr_eq(&original.config, &copied.config));
    }
}
