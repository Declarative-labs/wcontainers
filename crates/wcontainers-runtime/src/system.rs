//! The shipped outside for an operating system: its clocks and randomness, the container's output
//! streams, free turns ended by a timer thread at the kernel's slice, and waiting in real time.
//! Everything else the kernel may ask of a host (the file backend, the network, terminal input) it
//! answers as not supported, which the kernel turns into the errors Linux gives without them.

use core::ops::ControlFlow;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wcontainers_abi::{
    BackendRequestId, ConnectionId, Cpu, Event, Host, HostError, Stream, Warning,
};
use wcontainers_kernel::Kernel;

use crate::{Execution, Outside, StopFlag, Turn};

/// A warning as the outside keeps it: the kernel's or the back end's, by reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warned {
    pub reason: String,
    /// The detail it was last warned with.
    pub detail: u64,
    /// How many times so far, as the last warning counted.
    pub count: u64,
    /// From the back end, not the kernel.
    pub cpu: bool,
}

/// The shipped outside.
pub struct System {
    started: Instant,
    output: Box<dyn Write + Send>,
    error: Box<dyn Write + Send>,
    warnings: Vec<Warned>,
    timer: Option<Timer>,
}

/// Raises the stop flag at the deadline the last turn set, on its own thread.
struct Timer {
    shared: Arc<Deadline>,
    thread: Option<JoinHandle<()>>,
}

struct Deadline {
    /// Nanoseconds after `started` at which the running turn ends; `u64::MAX` for never.
    at: AtomicU64,
    /// The deadline the thread sleeps towards: a turn with an earlier one wakes it.
    sleeping: AtomicU64,
    /// Whether the outside is gone.
    gone: Mutex<bool>,
    wake: Condvar,
}

impl System {
    /// An outside writing the container's standard output and error to these streams.
    pub fn new(output: Box<dyn Write + Send>, error: Box<dyn Write + Send>) -> Self {
        Self {
            started: Instant::now(),
            output,
            error,
            warnings: Vec::new(),
            timer: None,
        }
    }

    /// Every warning so far, the kernel's and the back end's.
    pub fn warnings(&self) -> &[Warned] {
        &self.warnings
    }

    fn now(&self) -> u64 {
        self.started.elapsed().as_nanos() as u64
    }

    fn keep(&mut self, warning: Warning<'_>, cpu: bool) {
        match self
            .warnings
            .iter_mut()
            .find(|w| w.cpu == cpu && w.reason == warning.reason)
        {
            Some(kept) => {
                kept.count = kept.count.max(warning.count);
                kept.detail = warning.detail;
            }
            None => self.warnings.push(Warned {
                reason: warning.reason.to_string(),
                detail: warning.detail,
                count: warning.count,
                cpu,
            }),
        }
    }
}

impl Drop for System {
    fn drop(&mut self) {
        if let Some(timer) = &mut self.timer {
            *timer
                .shared
                .gone
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = true;
            timer.shared.wake.notify_one();
            if let Some(thread) = timer.thread.take() {
                let _ = thread.join();
            }
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
        let now = started.elapsed().as_nanos() as u64;
        if at <= now {
            flag.raise();
            let _ = shared
                .at
                .compare_exchange(at, u64::MAX, Ordering::SeqCst, Ordering::SeqCst);
            continue;
        }
        gone = if at == u64::MAX {
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

impl Host for System {
    fn monotonic_nanoseconds(&mut self) -> u64 {
        self.now()
    }

    fn realtime_nanoseconds(&mut self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos() as u64)
    }

    fn random(&mut self, into: &mut [u8]) {
        getrandom::fill(into).expect("the system's randomness");
    }

    fn warn(&mut self, warning: Warning<'_>) {
        self.keep(warning, false);
    }

    fn backend_send(&mut self, _: BackendRequestId, _: &[u8]) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn output(&mut self, stream: Stream, bytes: &[u8]) {
        let (writer, name) = match stream {
            Stream::Output => (&mut self.output, "standard output not written"),
            Stream::Error => (&mut self.error, "standard error not written"),
        };
        if let Err(error) = writer.write_all(bytes).and_then(|()| writer.flush()) {
            let detail = error.raw_os_error().map_or(u64::MAX, |code| code as u64);
            let count = self
                .warnings
                .iter()
                .find(|w| w.reason == name)
                .map_or(1, |w| w.count + 1);
            self.keep(
                Warning {
                    reason: name,
                    detail,
                    count,
                },
                false,
            );
        }
    }

    fn connect(&mut self, _: ConnectionId, _: &[u8], _: u16, _: bool) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn send(&mut self, _: ConnectionId, _: &[u8]) -> Result<usize, HostError> {
        Err(HostError::NotSupported)
    }

    fn receive(&mut self, _: ConnectionId, _: &mut [u8]) -> Result<usize, HostError> {
        Err(HostError::NotSupported)
    }

    fn finish(&mut self, _: ConnectionId) {}

    fn close(&mut self, _: ConnectionId) {}

    fn look_up(&mut self, _: u64, _: &str) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn listen(&mut self, _: ConnectionId, _: u16) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn accept(&mut self, _: ConnectionId, _: ConnectionId) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}

impl Outside for System {
    /// The shipped outside never ends a run itself.
    type End = core::convert::Infallible;

    fn stop_flag(&mut self, flag: StopFlag) {
        let shared = Arc::new(Deadline {
            at: AtomicU64::new(u64::MAX),
            sleeping: AtomicU64::new(u64::MAX),
            gone: Mutex::new(false),
            wake: Condvar::new(),
        });
        let started = self.started;
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("wcontainers timer".to_string())
                .spawn(move || time(&shared, started, &flag))
                .expect("a timer thread")
        };
        self.timer = Some(Timer {
            shared,
            thread: Some(thread),
        });
    }

    /// A free turn, ended by the timer thread when the kernel's slice has passed.
    fn turn(&mut self, slice_nanoseconds: Option<u64>) -> Turn {
        let at = slice_nanoseconds.map_or(u64::MAX, |slice| self.now().saturating_add(slice));
        if let Some(timer) = &self.timer {
            let shared = &timer.shared;
            shared.at.store(at, Ordering::SeqCst);
            // The thread sleeps to a later deadline or none: it must wake for this one.
            if at < shared.sleeping.load(Ordering::SeqCst) {
                let _gone = shared.gone.lock().unwrap_or_else(PoisonError::into_inner);
                shared.wake.notify_one();
            }
        }
        Turn::Free
    }

    /// Time is the host's clock, not progress.
    fn ran(&mut self, _: u64) {}

    /// Nothing arrives from outside: this outside has no file backend, network or terminal input.
    fn arrived(&mut self, _: &mut Kernel, _: &mut dyn Cpu) -> bool {
        false
    }

    fn stopped<E: Execution>(&mut self, _: &mut Kernel, _: &mut E) -> ControlFlow<Self::End> {
        ControlFlow::Continue(())
    }

    /// Sleeps until the kernel's next timer. With none, nothing can wake the container: it waits for
    /// ever, as it would on Linux.
    fn idle(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu, until_nanoseconds: Option<u64>) {
        match until_nanoseconds {
            Some(until) => {
                let now = self.now();
                if until > now {
                    std::thread::sleep(Duration::from_nanos(until - now));
                }
                kernel.event(cpu, self, Event::Timer);
            }
            None => loop {
                std::thread::park();
            },
        }
    }

    fn cpu_warning(&mut self, warning: Warning<'_>) {
        self.keep(warning, true);
    }
}
