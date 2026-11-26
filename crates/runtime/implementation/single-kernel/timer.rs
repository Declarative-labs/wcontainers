//! Free turns ended on the machine's clock: a thread that raises the executor's stop flag once the running
//! turn's slice has passed.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::StopFlag;

/// The name of the timer's thread.
const TIMER_THREAD: &str = "wcontainers timer";

/// A deadline that never comes.
const NEVER: u64 = u64::MAX;

/// Raises a stop flag at the deadline the last turn set, on its own thread, which ends when this is dropped.
pub struct SliceTimer {
    started: Instant,
    shared: Arc<Deadline>,
    thread: Option<JoinHandle<()>>,
}

struct Deadline {
    /// Nanoseconds after `started` at which the running turn ends; `NEVER` for no end.
    at: AtomicU64,
    /// The deadline the thread sleeps towards: a turn with an earlier one wakes it.
    sleeping: AtomicU64,
    /// Whether the timer is gone.
    gone: Mutex<bool>,
    wake: Condvar,
}

impl SliceTimer {
    pub fn new(flag: StopFlag) -> Self {
        let started = Instant::now();
        let shared = Arc::new(Deadline {
            at: AtomicU64::new(NEVER),
            sleeping: AtomicU64::new(NEVER),
            gone: Mutex::new(false),
            wake: Condvar::new(),
        });
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name(TIMER_THREAD.to_string())
                .spawn(move || time(&shared, started, &flag))
                .expect("a timer thread")
        };
        Self {
            started,
            shared,
            thread: Some(thread),
        }
    }

    /// A turn starts now and ends once `slice_nanoseconds` have passed, or never without a slice.
    pub fn turn(&self, slice_nanoseconds: Option<u64>) {
        let at = slice_nanoseconds.map_or(NEVER, |slice| {
            nanoseconds_since(self.started).saturating_add(slice)
        });
        let shared = &self.shared;
        shared.at.store(at, Ordering::SeqCst);
        // The thread sleeps to a later deadline or none: it must wake for this one.
        if at < shared.sleeping.load(Ordering::SeqCst) {
            let _gone = shared.gone.lock().unwrap_or_else(PoisonError::into_inner);
            shared.wake.notify_one();
        }
    }

    /// The turn ended: its deadline is dropped, so a wait past it raises nothing.
    pub fn turn_over(&self) {
        self.shared.at.store(NEVER, Ordering::SeqCst);
    }

    /// Whether no turn's deadline is waiting.
    #[cfg(test)]
    pub(crate) fn idle(&self) -> bool {
        self.shared.at.load(Ordering::SeqCst) == NEVER
    }
}

impl Drop for SliceTimer {
    fn drop(&mut self) {
        *self
            .shared
            .gone
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
        self.shared.wake.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The timer thread: sleeps to the deadline, raises the flag when it has passed, and sleeps again
/// when the deadline moved on meanwhile.
///
/// A turn stores its deadline and then reads the one slept towards; the thread stores the one it sleeps
/// towards and then reads the deadline again, holding the lock until it waits. So either the thread
/// sees the new deadline, or the turn sees an earlier one slept towards and wakes the thread once it
/// waits.
fn time(shared: &Deadline, started: Instant, flag: &StopFlag) {
    let mut gone = shared.gone.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        if *gone {
            return;
        }
        let at = shared.at.load(Ordering::SeqCst);
        shared.sleeping.store(at, Ordering::SeqCst);
        if shared.at.load(Ordering::SeqCst) != at {
            continue;
        }
        let now = nanoseconds_since(started);
        if at <= now {
            // A deadline a turn ended before, or moved, is not raised.
            if shared
                .at
                .compare_exchange(at, NEVER, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                flag.raise();
            }
            continue;
        }
        gone = if at == NEVER {
            shared
                .wake
                .wait(gone)
                .unwrap_or_else(PoisonError::into_inner)
        } else {
            let wait = Duration::from_nanos(at - now);
            match shared.wake.wait_timeout(gone, wait) {
                Ok((guard, _)) => guard,
                Err(poisoned) => poisoned.into_inner().0,
            }
        };
    }
}

pub(crate) fn nanoseconds_since(started: Instant) -> u64 {
    started.elapsed().as_nanos() as u64
}
