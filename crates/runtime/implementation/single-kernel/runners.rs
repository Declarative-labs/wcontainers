//! A container run through its kernel's runners: each on a host thread of its own with an executor of its own.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use wcontainers_abi::Warning;
use wcontainers_clock::Clock;
use wcontainers_kernel::{Kernel, Runner, StopReason};
use wcontainers_system::FallbackObserver;

/// Why an idle runner was run again before the time the kernel gave.
pub(crate) const IDLE_WITHOUT_WAKE: &str =
    "an idle runner is run again after a pause, since the kernel tells of no wake";

/// How a container's runners poll when idle, and what ends and resumes their executions.
pub(crate) struct RunnerStops {
    pub(crate) idle_poll: Duration,
    pub(crate) stop: Box<dyn Fn() + Send + Sync>,
    pub(crate) resume: Box<dyn Fn() + Send + Sync>,
}

/// A container on one kernel, run by that kernel's runners.
pub(crate) struct KernelContainer<K: Kernel, S> {
    /// The kernel itself, held as long as its runners' container; its interrupter calls it to end their runs.
    _kernel: Arc<K>,
    runners: Vec<(K::Runner, K::Executor)>,
    signals: Arc<Signals>,
    system: Arc<S>,
    /// What every later run returns, once the kernel can be run no more.
    ended: Option<StopReason>,
}

/// Ends a [`KernelContainer`]'s run from any host thread.
#[derive(Clone)]
pub struct RunInterrupter {
    signals: Arc<Signals>,
}

/// What the runners of one container tell each other and the interrupter.
struct Signals {
    state: Mutex<Signalled>,
    changed: Condvar,
    idle_polls: AtomicU64,
    /// How many idle polls the system has been told of.
    idle_polls_told: AtomicU64,
    /// The longest an idle runner waits before it is run again, since nothing tells the runtime that a wait of the kernel's ended.
    idle_poll: Duration,
    /// Ends the executors' runs in progress, and every later one until `resume`.
    stop: Box<dyn Fn() + Send + Sync>,
    /// Lets the executors run again once a run is over.
    resume: Box<dyn Fn() + Send + Sync>,
    /// How many runners are inside the kernel's `run` now.
    inside: AtomicUsize,
    /// Makes one call of the kernel's interface, which ends every runner's run once it waits for the kernel's lock.
    call_kernel: Box<dyn Fn() + Send + Sync>,
}

#[derive(Default)]
struct Signalled {
    interrupted: bool,
    /// Why the run in progress ends, once a runner said.
    stopped: Option<StopReason>,
}

impl<K, S> KernelContainer<K, S>
where
    K: Kernel + Send + 'static,
    K::Executor: Send,
    S: Clock + FallbackObserver<Warning<'static>> + Sync,
{
    /// Returns a container whose runners each get one of `executors`, whose executions `stops` ends.
    pub(crate) fn new(
        kernel: K,
        runners: impl Iterator<Item = K::Runner>,
        executors: Vec<K::Executor>,
        stops: RunnerStops,
        system: Arc<S>,
    ) -> (Self, RunInterrupter) {
        let RunnerStops {
            idle_poll,
            stop,
            resume,
        } = stops;
        let kernel = Arc::new(kernel);
        let called = Arc::clone(&kernel);
        let signals = Arc::new(Signals {
            state: Mutex::default(),
            changed: Condvar::new(),
            idle_polls: AtomicU64::default(),
            idle_polls_told: AtomicU64::default(),
            idle_poll,
            stop,
            resume,
            inside: AtomicUsize::new(usize::MIN),
            // Taking no task, it changes nothing: only its wait for the lock ends the runs.
            call_kernel: Box::new(move || drop(called.suspend_tasks(&[]))),
        });
        let container = Self {
            _kernel: kernel,
            runners: runners.zip(executors).collect(),
            signals: Arc::clone(&signals),
            system,
            ended: None,
        };
        (container, RunInterrupter { signals })
    }

    pub(crate) fn system(&self) -> &S {
        &self.system
    }

    /// Tells the system how many times an idle runner was run again for [`IDLE_WITHOUT_WAKE`], when that grew.
    fn tell_idle_polls(&self) {
        let polls = self.signals.idle_polls.load(Ordering::Relaxed);
        if self.signals.idle_polls_told.swap(polls, Ordering::Relaxed) != polls {
            self.system.on_fallback_used(&Warning {
                reason: IDLE_WITHOUT_WAKE,
                count: polls,
                detail: self.signals.idle_poll.as_nanos() as u64,
            });
        }
    }

    /// Runs every runner on its own host thread until one finishes the container or dies, or the run is interrupted.
    pub(crate) fn run(&mut self) -> StopReason {
        if let Some(ended) = self.ended {
            return ended;
        }
        let signals = &*self.signals;
        let clock = &*self.system;
        thread::scope(|scope| {
            for (runner, executor) in &mut self.runners {
                thread::Builder::new()
                    .spawn_scoped(scope, move || drive::<K>(runner, executor, signals, clock))
                    .expect("a host thread for a runner");
            }
        });
        (signals.resume)();
        let mut state = signals.lock();
        let stopped = match state.stopped.take() {
            Some(StopReason::Interrupted) | None => {
                state.interrupted = false;
                StopReason::Interrupted
            }
            Some(stopped) => stopped,
        };
        drop(state);
        self.tell_idle_polls();
        if matches!(stopped, StopReason::Finished(_) | StopReason::Panicked) {
            self.ended = Some(stopped);
        }
        stopped
    }
}

impl<K, S> wcontainers_runtime::Container for KernelContainer<K, S>
where
    K: Kernel + Send + 'static,
    K::Executor: Send,
    S: Clock + FallbackObserver<Warning<'static>> + Sync,
{
    fn run(&mut self) -> StopReason {
        KernelContainer::run(self)
    }
}

impl wcontainers_runtime::Interrupter for RunInterrupter {
    fn interrupt(&self) {
        self.signals.lock().interrupted = true;
        self.signals.changed.notify_all();
        self.signals.end_runs();
    }
}

/// Runs one runner until the container's run ends.
fn drive<K: Kernel>(
    runner: &mut K::Runner,
    executor: &mut K::Executor,
    signals: &Signals,
    clock: &impl Clock,
) {
    loop {
        // Counted inside first, so an interrupt either sees this runner inside or is seen here.
        signals.inside.fetch_add(1, Ordering::SeqCst);
        if signals.leaving() {
            signals.inside.fetch_sub(1, Ordering::SeqCst);
            return;
        }
        let stopped =
            catch_unwind(AssertUnwindSafe(|| runner.run(executor))).unwrap_or(StopReason::Panicked);
        signals.inside.fetch_sub(1, Ordering::SeqCst);
        match stopped {
            StopReason::Interrupted => {}
            StopReason::Idle { until } => signals.wait(until, clock),
            StopReason::Suspended => signals.wait(None, clock),
            StopReason::Finished(_) | StopReason::Panicked => {
                signals.stop(stopped);
                return;
            }
        }
    }
}

impl Signals {
    fn lock(&self) -> MutexGuard<'_, Signalled> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn leaving(&self) -> bool {
        let state = self.lock();
        state.interrupted || state.stopped.is_some()
    }

    /// Ends the run for every runner, those in the executor too; a panic outranks a finish, and a finish an interrupt.
    fn stop(&self, stopped: StopReason) {
        let mut state = self.lock();
        let outranks = match state.stopped {
            None | Some(StopReason::Interrupted) => true,
            Some(StopReason::Finished(_)) => stopped == StopReason::Panicked,
            Some(_) => false,
        };
        if outranks {
            state.stopped = Some(stopped);
        }
        drop(state);
        self.changed.notify_all();
        self.end_runs();
    }

    /// Ends the executions running and calls the kernel until no runner is inside its `run`.
    ///
    /// The kernel's runner goes on to its next turn after an execution's interrupt alone, and leaves `run` only while one
    /// of the kernel's calls waits for its lock.
    fn end_runs(&self) {
        (self.stop)();
        while self.inside.load(Ordering::SeqCst) != 0 {
            (self.call_kernel)();
            thread::yield_now();
        }
    }

    /// Waits until `until` on `clock`'s monotonic time, at most the idle poll, or until the run ends.
    fn wait(&self, until: Option<Duration>, clock: &impl Clock) {
        let now = Duration::from_nanos(clock.monotonic_nanoseconds());
        let left = until.map(|until| until.saturating_sub(now));
        let pause = match left {
            Some(left) if left <= self.idle_poll => left,
            _ => {
                self.idle_polls.fetch_add(1, Ordering::Relaxed);
                self.idle_poll
            }
        };
        let state = self.lock();
        if state.interrupted || state.stopped.is_some() {
            return;
        }
        let _woken = self
            .changed
            .wait_timeout(state, pause)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// The runtime's own end of a run, against a kernel that does only what the kernel's interface promises.
///
/// The interface promises no time within which a runner returns once another finished or an interrupt was asked, so
/// the double's runners never return by a clock of their own: each runs until the execution it runs is ended through
/// the runner stops the runtime was given, as an execution ends on its executor's interrupter.
#[cfg(test)]
mod tests {
    use std::iter::Empty;
    use std::num::NonZeroUsize;
    use std::sync::mpsc;
    use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
    use std::thread;
    use std::time::{Duration, Instant};

    use wcontainers_abi::Warning;
    use wcontainers_clock::Clock;
    use wcontainers_executor_unicorn::standin::StandIn;
    use wcontainers_kernel::{ExecutionConfig, Kernel, ProcessEnd, Runner, StopReason, TaskId};
    use wcontainers_runtime::Interrupter;
    use wcontainers_system::FallbackObserver;

    use super::{KernelContainer, RunnerStops};

    /// The longest a container's run may take here: every run of these tests ends as soon as its runners are ended.
    const RUN_BOUND: Duration = Duration::from_secs(30);

    /// An idle poll far longer than [`RUN_BOUND`], so a runner the runtime leaves waiting is seen.
    const IDLE_POLL: Duration = Duration::from_secs(3600);

    /// The way the finishing runner's process ended.
    const FINISHED: StopReason = StopReason::Finished(ProcessEnd::Exited { code: 0 });

    /// What every runner of a double kernel sees: whether its executions are ended, and how many runners are running.
    #[derive(Default)]
    struct Board {
        state: Mutex<BoardState>,
        changed: Condvar,
    }

    #[derive(Default)]
    struct BoardState {
        /// Whether every execution ends at once until the runtime lets them run again.
        ended: bool,
        /// How many runners have begun a run that only an end of their execution ends.
        running: usize,
    }

    impl Board {
        fn lock(&self) -> MutexGuard<'_, BoardState> {
            self.state.lock().unwrap_or_else(PoisonError::into_inner)
        }

        /// Waits until `done` holds of the board.
        fn wait_until(&self, done: impl Fn(&BoardState) -> bool) {
            let mut state = self.lock();
            while !done(&state) {
                state = self
                    .changed
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        }

        /// Sets whether executions end, and wakes every runner.
        fn set_ended(&self, ended: bool) {
            self.lock().ended = ended;
            self.changed.notify_all();
        }
    }

    /// What one runner of a double kernel does in each run.
    #[derive(Clone, Copy)]
    enum Script {
        /// Returns [`FINISHED`] once this many other runners are running.
        Finishes { after_running: usize },
        /// Runs until its execution is ended, then returns `Interrupted`.
        RunsThenInterrupted,
        /// Runs until its execution is ended, then returns `Idle` with no time to be run again at.
        RunsThenIdle,
        /// Runs until its execution is ended, then its host thread panics.
        RunsThenPanics,
    }

    /// A runner of a double kernel, which ignores the executor it is given.
    struct ScriptedRunner {
        board: Arc<Board>,
        script: Script,
    }

    impl ScriptedRunner {
        /// Counts itself running and waits until its execution is ended.
        fn run_until_ended(&self) {
            self.board.lock().running += 1;
            self.board.changed.notify_all();
            self.board.wait_until(|state| state.ended);
        }
    }

    impl Runner<StandIn> for ScriptedRunner {
        fn run(&mut self, _executor: &mut StandIn) -> StopReason {
            match self.script {
                Script::Finishes { after_running } => {
                    self.board
                        .wait_until(|state| state.running >= after_running);
                    FINISHED
                }
                Script::RunsThenInterrupted => {
                    self.run_until_ended();
                    StopReason::Interrupted
                }
                Script::RunsThenIdle => {
                    self.run_until_ended();
                    StopReason::Idle { until: None }
                }
                Script::RunsThenPanics => {
                    self.run_until_ended();
                    panic!("the runner's host thread dies after its execution ended")
                }
            }
        }
    }

    /// A kernel with no tasks whose runners follow their scripts.
    struct ScriptedKernel;

    impl Kernel for ScriptedKernel {
        type Executor = StandIn;
        type Fallback = ();
        type Config = (Arc<Board>, Vec<Script>);
        type Runner = ScriptedRunner;
        type SuspendedTask = ();
        type ProcessContext = ();
        type Runners = std::vec::IntoIter<ScriptedRunner>;
        type SuspendedTasks<'kernel> = Empty<(TaskId, ())>;
        type UnresumedTasks<'kernel> = Empty<()>;

        /// Gives each runner the next script, in order; `parallelism` must be the number of scripts.
        fn new(config: Self::Config, parallelism: NonZeroUsize) -> (Self, Self::Runners) {
            let (board, scripts) = config;
            assert_eq!(scripts.len(), parallelism.get(), "one script per runner");
            let runners: Vec<ScriptedRunner> = scripts
                .into_iter()
                .map(|script| ScriptedRunner {
                    board: Arc::clone(&board),
                    script,
                })
                .collect();
            (ScriptedKernel, runners.into_iter())
        }

        fn suspend_tasks(&self, _tasks: &[TaskId]) -> Self::SuspendedTasks<'_> {
            std::iter::empty()
        }

        fn suspend_process(&self, _process: TaskId) -> Option<((), Self::SuspendedTasks<'_>)> {
            None
        }

        fn copy_process_context(&self, _process: TaskId) -> Option<()> {
            None
        }

        fn resume_process(&self, context: (), _config: ExecutionConfig<StandIn>) -> Option<()> {
            Some(context)
        }

        fn resume_tasks(&self, _tasks: impl IntoIterator<Item = ()>) -> Self::UnresumedTasks<'_> {
            std::iter::empty()
        }
    }

    /// A system that only tells the time.
    struct HostClock {
        start: Instant,
    }

    impl Clock for HostClock {
        fn monotonic_nanoseconds(&self) -> u64 {
            self.start.elapsed().as_nanos() as u64
        }

        fn realtime_nanoseconds(&self) -> u64 {
            self.monotonic_nanoseconds()
        }
    }

    impl FallbackObserver<Warning<'static>> for HostClock {}

    /// Returns a container on a double kernel whose runners follow `scripts`, its interrupter, and their board.
    fn container(
        scripts: Vec<Script>,
    ) -> (
        KernelContainer<ScriptedKernel, HostClock>,
        super::RunInterrupter,
        Arc<Board>,
    ) {
        let board = Arc::new(Board::default());
        let parallelism = NonZeroUsize::new(scripts.len()).expect("at least one script");
        let executors = scripts.iter().map(|_| StandIn::new()).collect();
        let (kernel, runners) = ScriptedKernel::new((Arc::clone(&board), scripts), parallelism);
        let (ended, resumed) = (Arc::clone(&board), Arc::clone(&board));
        let stops = RunnerStops {
            idle_poll: IDLE_POLL,
            stop: Box::new(move || ended.set_ended(true)),
            resume: Box::new(move || resumed.set_ended(false)),
        };
        let system = Arc::new(HostClock {
            start: Instant::now(),
        });
        let (container, interrupter) =
            KernelContainer::new(kernel, runners, executors, stops, system);
        (container, interrupter, board)
    }

    /// Runs `container` on a host thread of its own and returns what its run returned, or none past [`RUN_BOUND`].
    fn run_bounded(
        mut container: KernelContainer<ScriptedKernel, HostClock>,
    ) -> Option<StopReason> {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let stopped = container.run();
            let _ = sender.send(stopped);
        });
        receiver.recv_timeout(RUN_BOUND).ok()
    }

    #[test]
    fn a_finish_ends_a_runner_that_runs_until_its_execution_is_ended() {
        let (container, _interrupter, _board) = container(vec![
            Script::Finishes { after_running: 1 },
            Script::RunsThenInterrupted,
        ]);
        assert_eq!(run_bounded(container), Some(FINISHED));
    }

    #[test]
    fn a_finish_ends_a_runner_that_answers_idle_once_its_execution_is_ended() {
        let (container, _interrupter, _board) = container(vec![
            Script::Finishes { after_running: 1 },
            Script::RunsThenIdle,
        ]);
        assert_eq!(run_bounded(container), Some(FINISHED));
    }

    #[test]
    fn an_interrupt_ends_runners_that_run_until_their_executions_are_ended() {
        let (container, interrupter, board) =
            container(vec![Script::RunsThenInterrupted, Script::RunsThenIdle]);
        let interrupting = thread::spawn(move || {
            board.wait_until(|state| state.running == 2);
            interrupter.interrupt();
        });
        assert_eq!(run_bounded(container), Some(StopReason::Interrupted));
        interrupting.join().expect("the interrupting thread");
    }

    #[test]
    fn a_panic_after_a_finish_ends_the_container_as_panicked() {
        let (container, _interrupter, _board) = container(vec![
            Script::Finishes { after_running: 1 },
            Script::RunsThenPanics,
        ]);
        assert_eq!(run_bounded(container), Some(StopReason::Panicked));
    }
}
