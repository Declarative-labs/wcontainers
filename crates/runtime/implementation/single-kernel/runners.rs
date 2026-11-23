//! A container run through its kernel's runners: each on a host thread of its own with an executor of its own.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use wcontainers_clock::Clock;
use wcontainers_kernel::{Kernel, Runner, StopReason};

/// The longest an idle runner waits before it is run again, since nothing tells the runtime that a wait of the kernel's ended.
const IDLE_POLL: Duration = Duration::from_millis(1);

/// Why an idle runner was run again before the time the kernel gave.
pub const IDLE_WITHOUT_WAKE: &str =
    "an idle runner is run again after a pause, since the kernel tells of no wake";

/// A container on one kernel, run by that kernel's runners.
pub struct KernelContainer<K: Kernel, S> {
    kernel: K,
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
    /// Ends the executors' runs in progress.
    stop: Arc<dyn Fn() + Send + Sync>,
}

/// What the runners of one container tell each other and the interrupter.
#[derive(Default)]
struct Signals {
    state: Mutex<Signalled>,
    changed: Condvar,
    idle_polls: AtomicU64,
}

#[derive(Default)]
struct Signalled {
    interrupted: bool,
    /// Why the run in progress ends, once a runner said.
    stopped: Option<StopReason>,
}

impl<K, S> KernelContainer<K, S>
where
    K: Kernel,
    K::Executor: Send,
    S: Clock + Sync,
{
    /// A container whose runners each get an executor `executor` makes; `stop` ends every run in progress of those executors.
    pub fn new(
        kernel: K,
        runners: impl Iterator<Item = K::Runner>,
        mut executor: impl FnMut() -> K::Executor,
        stop: impl Fn() + Send + Sync + 'static,
        system: Arc<S>,
    ) -> (Self, RunInterrupter) {
        let signals = Arc::new(Signals::default());
        let container = Self {
            kernel,
            runners: runners.map(|runner| (runner, executor())).collect(),
            signals: Arc::clone(&signals),
            system,
            ended: None,
        };
        let interrupter = RunInterrupter {
            signals,
            stop: Arc::new(stop),
        };
        (container, interrupter)
    }

    pub fn kernel(&self) -> &K {
        &self.kernel
    }

    pub fn system(&self) -> &S {
        &self.system
    }

    /// How many times an idle runner was run again for [`IDLE_WITHOUT_WAKE`].
    pub fn idle_polls(&self) -> u64 {
        self.signals.idle_polls.load(Ordering::Relaxed)
    }

    /// Runs every runner on its own host thread until one finishes the container or dies, or the run is interrupted.
    pub fn run(&mut self) -> StopReason {
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
        let mut state = signals.lock();
        let stopped = match state.stopped.take() {
            Some(StopReason::Interrupted) | None => {
                state.interrupted = false;
                StopReason::Interrupted
            }
            Some(stopped) => stopped,
        };
        if matches!(stopped, StopReason::Finished(_) | StopReason::Panicked) {
            self.ended = Some(stopped);
        }
        stopped
    }
}

impl<K, S> wcontainers_runtime::Container for KernelContainer<K, S>
where
    K: Kernel,
    K::Executor: Send,
    S: Clock + Sync,
{
    fn run(&mut self) -> StopReason {
        KernelContainer::run(self)
    }
}

impl wcontainers_runtime::Interrupter for RunInterrupter {
    fn interrupt(&self) {
        self.signals.lock().interrupted = true;
        self.signals.changed.notify_all();
        (self.stop)();
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

    /// Ends the run for every runner; a panic outranks a finish, and a finish an interrupt.
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
    }

    /// Waits until `until` on `clock`'s monotonic time, at most [`IDLE_POLL`], or until the run ends.
    fn wait(&self, until: Option<Duration>, clock: &impl Clock) {
        let now = Duration::from_nanos(clock.monotonic_nanoseconds());
        let left = until.map(|until| until.saturating_sub(now));
        let pause = match left {
            Some(left) if left <= IDLE_POLL => left,
            _ => {
                self.idle_polls.fetch_add(1, Ordering::Relaxed);
                IDLE_POLL
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
