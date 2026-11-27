//! A container run through its kernel's runners: each on a host thread of its own with an executor of its own.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use wcontainers_abi::Warning;
use wcontainers_clock::Clock;
use wcontainers_kernel::{Kernel, Runner, StopReason};
use wcontainers_system::FallbackObserver;

/// Why an idle runner was run again before the time the kernel gave; its detail is the idle poll in nanoseconds.
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
    /// The kernel itself, held as long as its runners.
    _kernel: K,
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
        let signals = Arc::new(Signals {
            state: Mutex::default(),
            changed: Condvar::new(),
            idle_polls: AtomicU64::default(),
            idle_polls_told: AtomicU64::default(),
            idle_poll,
            stop,
            resume,
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
        (self.signals.stop)();
    }
}

/// Runs one runner until the container's run ends, again after each `Interrupted`, `Idle` or `Suspended`.
fn drive<K: Kernel>(
    runner: &mut K::Runner,
    executor: &mut K::Executor,
    signals: &Signals,
    clock: &impl Clock,
) {
    loop {
        // An end asked after this check still ends this run: the executions stay ended until every runner is back.
        if signals.leaving() {
            return;
        }
        let stopped =
            catch_unwind(AssertUnwindSafe(|| runner.run(executor))).unwrap_or(StopReason::Panicked);
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

    /// Ends the run for every runner; a panic outranks a finish, a finish an interrupt, and the first finish stands.
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
        (self.stop)();
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

    use super::{IDLE_WITHOUT_WAKE, KernelContainer, RunInterrupter, RunnerStops};

    /// The longest a container's run may take here: every run of these tests ends as soon as its runners are ended.
    const RUN_BOUND: Duration = Duration::from_secs(30);

    /// An idle poll far longer than [`RUN_BOUND`], so a runner the runtime leaves waiting is seen.
    const LONG_POLL: Duration = Duration::from_secs(3600);

    /// An idle poll short enough that a runner waiting it out is run again well within [`RUN_BOUND`].
    const SHORT_POLL: Duration = Duration::from_millis(20);

    /// How long a step waits after its gate holds, so that a runner which answered `Idle` is asleep in the runtime.
    const SETTLE: Duration = Duration::from_millis(300);

    /// The way the finishing runner's process ended.
    const FINISHED: StopReason = StopReason::Finished(ProcessEnd::Exited { code: 0 });

    /// What every runner of a double kernel sees: whether its executions are ended, and what the runners did so far.
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
        /// How many times a runner answered `Idle` or `Suspended` at once.
        idled: usize,
        /// How many runs began, over every runner.
        runs: usize,
    }

    impl Board {
        fn lock(&self) -> MutexGuard<'_, BoardState> {
            self.state.lock().unwrap_or_else(PoisonError::into_inner)
        }

        /// Waits until `gate` holds of the board.
        fn wait_until(&self, gate: Gate) {
            let mut state = self.lock();
            while !gate.holds(&state) {
                state = self
                    .changed
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        }

        /// Waits until `gate` holds of the board, at most `bound`, and returns whether it held.
        fn wait_until_within(&self, gate: Gate, bound: Duration) -> bool {
            let deadline = Instant::now() + bound;
            let mut state = self.lock();
            while !gate.holds(&state) {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return false;
                }
                state = self
                    .changed
                    .wait_timeout(state, left)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
            true
        }

        /// Changes the board, and wakes every runner.
        fn change(&self, change: impl FnOnce(&mut BoardState)) {
            change(&mut self.lock());
            self.changed.notify_all();
        }
    }

    /// What a step waits for before it answers.
    #[derive(Clone, Copy)]
    enum Gate {
        /// Nothing.
        Now,
        /// At least this many runners are running until their executions are ended.
        Running(usize),
        /// Runners answered `Idle` or `Suspended` at least this many times, and then [`SETTLE`] passed.
        IdledThenSettled(usize),
        /// The runtime ended the executions.
        Ended,
    }

    impl Gate {
        fn holds(self, state: &BoardState) -> bool {
            match self {
                Gate::Now => true,
                Gate::Running(running) => state.running >= running,
                Gate::IdledThenSettled(idled) => state.idled >= idled,
                Gate::Ended => state.ended,
            }
        }
    }

    /// What one run of a double kernel's runner does.
    #[derive(Clone, Copy)]
    enum Step {
        /// Returns `Finished` with the given end once the gate holds.
        Finishes(ProcessEnd, Gate),
        /// Runs until its execution is ended, then returns `Interrupted`.
        RunsThenInterrupted,
        /// Runs until its execution is ended, then returns `Idle` with no time to be run again at.
        RunsThenIdle,
        /// Runs until its execution is ended, then its host thread panics.
        RunsThenPanics,
        /// Returns `Idle` at once, with the given time to be run again at.
        Idles(Option<Duration>),
        /// Returns `Suspended` at once.
        Suspends,
        /// Its host thread panics at once.
        Panics,
    }

    /// The step that finishes at once.
    const FINISHES_NOW: Step = Step::Finishes(ProcessEnd::Exited { code: 0 }, Gate::Now);

    /// A runner of a double kernel, which ignores the executor it is given.
    struct ScriptedRunner {
        board: Arc<Board>,
        /// One step for each run, in order; the last is repeated.
        steps: Vec<Step>,
        next: usize,
    }

    impl ScriptedRunner {
        /// Counts itself running and waits until its execution is ended.
        fn run_until_ended(&self) {
            self.board.change(|state| state.running += 1);
            self.board.wait_until(Gate::Ended);
        }
    }

    impl Runner<StandIn> for ScriptedRunner {
        fn run(&mut self, _executor: &mut StandIn) -> StopReason {
            let step = self.steps[self.next.min(self.steps.len() - 1)];
            self.next += 1;
            self.board.change(|state| state.runs += 1);
            match step {
                Step::Finishes(end, gate) => {
                    self.board.wait_until(gate);
                    if let Gate::IdledThenSettled(_) = gate {
                        thread::sleep(SETTLE);
                    }
                    StopReason::Finished(end)
                }
                Step::RunsThenInterrupted => {
                    self.run_until_ended();
                    StopReason::Interrupted
                }
                Step::RunsThenIdle => {
                    self.run_until_ended();
                    StopReason::Idle { until: None }
                }
                Step::RunsThenPanics => {
                    self.run_until_ended();
                    panic!("the runner's host thread dies after its execution ended")
                }
                Step::Idles(until) => {
                    self.board.change(|state| state.idled += 1);
                    StopReason::Idle { until }
                }
                Step::Suspends => {
                    self.board.change(|state| state.idled += 1);
                    StopReason::Suspended
                }
                Step::Panics => panic!("the runner's host thread dies at once"),
            }
        }
    }

    /// A kernel with no tasks whose runners follow their steps.
    struct ScriptedKernel;

    impl Kernel for ScriptedKernel {
        type Executor = StandIn;
        type Fallback = ();
        type Config = (Arc<Board>, Vec<Vec<Step>>);
        type Runner = ScriptedRunner;
        type SuspendedTask = ();
        type ProcessContext = ();
        type Runners = std::vec::IntoIter<ScriptedRunner>;
        type SuspendedTasks<'kernel> = Empty<(TaskId, ())>;
        type UnresumedTasks<'kernel> = Empty<()>;

        /// Gives each runner the next list of steps, in order; `parallelism` must be the number of lists.
        fn new(config: Self::Config, parallelism: NonZeroUsize) -> (Self, Self::Runners) {
            let (board, scripts) = config;
            assert_eq!(scripts.len(), parallelism.get(), "one script per runner");
            let runners: Vec<ScriptedRunner> = scripts
                .into_iter()
                .map(|steps| ScriptedRunner {
                    board: Arc::clone(&board),
                    steps,
                    next: 0,
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

    /// A system whose clock stands at zero and which keeps every warning it is told of.
    #[derive(Default)]
    struct StillSystem {
        warnings: Mutex<Vec<(String, u64, u64)>>,
    }

    impl StillSystem {
        /// Returns each warning told so far: its reason, count and detail.
        fn warnings(&self) -> Vec<(String, u64, u64)> {
            self.warnings
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    impl Clock for StillSystem {
        fn monotonic_nanoseconds(&self) -> u64 {
            0
        }

        fn realtime_nanoseconds(&self) -> u64 {
            0
        }
    }

    impl FallbackObserver<Warning<'static>> for StillSystem {
        fn on_fallback_used(&self, which: &Warning<'static>) {
            self.warnings
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((which.reason.to_owned(), which.count, which.detail));
        }
    }

    type Double = KernelContainer<ScriptedKernel, StillSystem>;

    /// A container on a double kernel whose runners follow `scripts`, with its interrupter, board and system.
    struct Setup {
        container: Double,
        interrupter: RunInterrupter,
        board: Arc<Board>,
        system: Arc<StillSystem>,
    }

    fn setup(scripts: Vec<Vec<Step>>, idle_poll: Duration) -> Setup {
        let board = Arc::new(Board::default());
        let parallelism = NonZeroUsize::new(scripts.len()).expect("at least one script");
        let executors = scripts.iter().map(|_| StandIn::new()).collect();
        let (kernel, runners) = ScriptedKernel::new((Arc::clone(&board), scripts), parallelism);
        let (ended, resumed) = (Arc::clone(&board), Arc::clone(&board));
        let stops = RunnerStops {
            idle_poll,
            stop: Box::new(move || ended.change(|state| state.ended = true)),
            resume: Box::new(move || resumed.change(|state| state.ended = false)),
        };
        let system = Arc::new(StillSystem::default());
        let (container, interrupter) =
            KernelContainer::new(kernel, runners, executors, stops, Arc::clone(&system));
        Setup {
            container,
            interrupter,
            board,
            system,
        }
    }

    /// Runs `container` on a host thread of its own; returns its stop and the container, or none past [`RUN_BOUND`].
    fn run_bounded(mut container: Double) -> Option<(StopReason, Double)> {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let stopped = container.run();
            let _ = sender.send((stopped, container));
        });
        receiver.recv_timeout(RUN_BOUND).ok()
    }

    /// Returns what `container`'s run returned, or none past [`RUN_BOUND`].
    fn stopped(container: Double) -> Option<StopReason> {
        run_bounded(container).map(|(stopped, _)| stopped)
    }

    /// Interrupts the run from a host thread once `gate` holds of `board`; the thread says whether it held in time.
    fn interrupt_when(
        interrupter: RunInterrupter,
        board: Arc<Board>,
        gate: Gate,
    ) -> thread::JoinHandle<bool> {
        thread::spawn(move || {
            if !board.wait_until_within(gate, RUN_BOUND) {
                return false;
            }
            if let Gate::IdledThenSettled(_) = gate {
                thread::sleep(SETTLE);
            }
            interrupter.interrupt();
            true
        })
    }

    #[test]
    fn a_finish_ends_a_runner_that_runs_until_its_execution_is_ended() {
        let setup = setup(
            vec![
                vec![Step::Finishes(
                    ProcessEnd::Exited { code: 0 },
                    Gate::Running(1),
                )],
                vec![Step::RunsThenInterrupted],
            ],
            LONG_POLL,
        );
        assert_eq!(stopped(setup.container), Some(FINISHED));
    }

    #[test]
    fn a_finish_ends_a_runner_that_answers_idle_once_its_execution_is_ended() {
        let setup = setup(
            vec![
                vec![Step::Finishes(
                    ProcessEnd::Exited { code: 0 },
                    Gate::Running(1),
                )],
                vec![Step::RunsThenIdle],
            ],
            LONG_POLL,
        );
        assert_eq!(stopped(setup.container), Some(FINISHED));
    }

    #[test]
    fn a_finish_wakes_a_runner_that_waits_idle() {
        let setup = setup(
            vec![
                vec![Step::Finishes(
                    ProcessEnd::Exited { code: 0 },
                    Gate::IdledThenSettled(1),
                )],
                vec![Step::Idles(None), Step::RunsThenInterrupted],
            ],
            LONG_POLL,
        );
        assert_eq!(stopped(setup.container), Some(FINISHED));
    }

    #[test]
    fn an_interrupt_ends_runners_that_run_until_their_executions_are_ended() {
        let setup = setup(
            vec![vec![Step::RunsThenInterrupted], vec![Step::RunsThenIdle]],
            LONG_POLL,
        );
        let interrupting = interrupt_when(setup.interrupter, setup.board, Gate::Running(2));
        assert_eq!(stopped(setup.container), Some(StopReason::Interrupted));
        assert!(interrupting.join().expect("the interrupting thread"));
    }

    #[test]
    fn an_interrupt_wakes_runners_that_wait_idle() {
        let idle_then_running = vec![Step::Idles(None), Step::RunsThenInterrupted];
        let setup = setup(
            vec![idle_then_running.clone(), idle_then_running],
            LONG_POLL,
        );
        let interrupting =
            interrupt_when(setup.interrupter, setup.board, Gate::IdledThenSettled(2));
        assert_eq!(stopped(setup.container), Some(StopReason::Interrupted));
        assert!(interrupting.join().expect("the interrupting thread"));
    }

    #[test]
    fn a_run_after_an_interrupt_runs_the_runners_again() {
        let setup = setup(
            vec![vec![Step::RunsThenInterrupted, FINISHES_NOW]],
            LONG_POLL,
        );
        let interrupting = interrupt_when(setup.interrupter, setup.board, Gate::Running(1));
        let (first, container) = run_bounded(setup.container).expect("the first run ends");
        assert!(interrupting.join().expect("the interrupting thread"));
        assert_eq!(first, StopReason::Interrupted);
        assert_eq!(stopped(container), Some(FINISHED));
    }

    #[test]
    fn a_panic_after_a_finish_ends_the_container_as_panicked() {
        let setup = setup(
            vec![
                vec![Step::Finishes(
                    ProcessEnd::Exited { code: 0 },
                    Gate::Running(1),
                )],
                vec![Step::RunsThenPanics],
            ],
            LONG_POLL,
        );
        assert_eq!(stopped(setup.container), Some(StopReason::Panicked));
    }

    #[test]
    fn a_finish_after_a_panic_ends_the_container_as_panicked() {
        let setup = setup(
            vec![
                vec![Step::Panics],
                vec![Step::Finishes(ProcessEnd::Exited { code: 0 }, Gate::Ended)],
            ],
            LONG_POLL,
        );
        assert_eq!(stopped(setup.container), Some(StopReason::Panicked));
    }

    #[test]
    fn the_first_finish_stands() {
        let setup = setup(
            vec![
                vec![Step::Finishes(ProcessEnd::Exited { code: 1 }, Gate::Now)],
                vec![Step::Finishes(ProcessEnd::Exited { code: 2 }, Gate::Ended)],
            ],
            LONG_POLL,
        );
        assert_eq!(
            stopped(setup.container),
            Some(StopReason::Finished(ProcessEnd::Exited { code: 1 }))
        );
    }

    #[test]
    fn a_container_that_panicked_is_not_run_again() {
        let setup = setup(vec![vec![Step::Panics]], LONG_POLL);
        let (first, container) = run_bounded(setup.container).expect("the first run ends");
        assert_eq!(first, StopReason::Panicked);
        assert_eq!(stopped(container), Some(StopReason::Panicked));
        assert_eq!(setup.board.lock().runs, 1);
    }

    #[test]
    fn a_suspended_runner_is_run_again() {
        let setup = setup(vec![vec![Step::Suspends, FINISHES_NOW]], SHORT_POLL);
        assert_eq!(stopped(setup.container), Some(FINISHED));
    }

    #[test]
    fn an_idle_runner_is_run_again_at_the_time_it_was_given() {
        let setup = setup(
            vec![vec![Step::Idles(Some(SHORT_POLL)), FINISHES_NOW]],
            LONG_POLL,
        );
        assert_eq!(stopped(setup.container), Some(FINISHED));
        assert_eq!(setup.system.warnings(), vec![]);
    }

    #[test]
    fn an_idle_runner_run_again_at_the_idle_poll_itself_is_not_early() {
        let setup = setup(
            vec![vec![Step::Idles(Some(SHORT_POLL)), FINISHES_NOW]],
            SHORT_POLL,
        );
        assert_eq!(stopped(setup.container), Some(FINISHED));
        assert_eq!(setup.system.warnings(), vec![]);
    }

    #[test]
    fn an_idle_runner_with_no_time_is_run_again_after_the_idle_poll_and_told() {
        let setup = setup(vec![vec![Step::Idles(None), FINISHES_NOW]], SHORT_POLL);
        assert_eq!(stopped(setup.container), Some(FINISHED));
        assert_eq!(
            setup.system.warnings(),
            vec![(
                IDLE_WITHOUT_WAKE.to_owned(),
                1,
                SHORT_POLL.as_nanos() as u64
            )]
        );
    }

    #[test]
    fn an_idle_runner_whose_time_is_past_the_idle_poll_is_run_again_early_and_told() {
        let setup = setup(
            vec![vec![
                Step::Idles(Some(LONG_POLL)),
                Step::Idles(Some(LONG_POLL)),
                FINISHES_NOW,
            ]],
            SHORT_POLL,
        );
        assert_eq!(stopped(setup.container), Some(FINISHED));
        assert_eq!(
            setup.system.warnings(),
            vec![(
                IDLE_WITHOUT_WAKE.to_owned(),
                2,
                SHORT_POLL.as_nanos() as u64
            )]
        );
    }

    #[test]
    fn the_idle_polls_are_told_again_only_when_they_grew() {
        let setup = setup(
            vec![vec![
                Step::RunsThenInterrupted,
                Step::Idles(None),
                FINISHES_NOW,
            ]],
            SHORT_POLL,
        );
        let interrupting = interrupt_when(
            setup.interrupter.clone(),
            Arc::clone(&setup.board),
            Gate::Running(1),
        );
        let (first, container) = run_bounded(setup.container).expect("the first run ends");
        assert!(interrupting.join().expect("the interrupting thread"));
        assert_eq!(first, StopReason::Interrupted);
        assert_eq!(setup.system.warnings(), vec![]);
        assert_eq!(stopped(container), Some(FINISHED));
        let told = vec![(
            IDLE_WITHOUT_WAKE.to_owned(),
            1,
            SHORT_POLL.as_nanos() as u64,
        )];
        assert_eq!(setup.system.warnings(), told);
    }
}
