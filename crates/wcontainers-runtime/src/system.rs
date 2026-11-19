//! The local system for an operating system: its clocks and randomness, the container's output
//! streams, a directory of the host as the file backend, the host's network, free turns ended by a
//! timer thread at the kernel's slice or by an arrival from outside, and waiting in real time.

use core::ops::ControlFlow;
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wcontainers_abi::{
    BackendRequestId, ConnectionId, Cpu, Event, Host, HostError, Stream, Warning,
};
use wcontainers_kernel::Kernel;

use crate::files::Files;
use crate::net::{Arrivals, Network};
use crate::{Execution, StopFlag, System, Turn};

/// A warning as the system keeps it: the kernel's or the back end's, by reason.
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

/// The local system.
pub struct LocalSystem {
    started: Instant,
    output: Box<dyn Write + Send>,
    error: Box<dyn Write + Send>,
    warnings: Vec<Warned>,
    timer: Option<Timer>,
    files: Option<Files>,
    /// The file backend's replies not yet handed to the kernel.
    replies: Vec<(BackendRequestId, Vec<u8>)>,
    arrivals: Arc<Arrivals>,
    /// Made at the container's first use of the network.
    network: Option<Network>,
    published: Vec<(u16, SocketAddr)>,
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
    /// Whether the system is gone.
    gone: Mutex<bool>,
    wake: Condvar,
}

impl LocalSystem {
    /// A system writing the container's standard output and error to these streams.
    pub fn new(output: Box<dyn Write + Send>, error: Box<dyn Write + Send>) -> Self {
        Self {
            started: Instant::now(),
            output,
            error,
            warnings: Vec::new(),
            timer: None,
            files: None,
            replies: Vec::new(),
            arrivals: Arc::default(),
            network: None,
            published: Vec::new(),
        }
    }

    /// Lets connections from the host at `at` reach the container's TCP port `port`, as `docker run
    /// --publish` does. Nothing outside reaches a port not published.
    pub fn publish(&mut self, port: u16, at: SocketAddr) {
        self.published.push((port, at));
        if let Some(network) = &mut self.network {
            network.publish(port, at);
        }
    }

    fn network(&mut self) -> &mut Network {
        self.network.get_or_insert_with(|| {
            let mut network = Network::new(self.arrivals.clone());
            for &(port, at) in &self.published {
                network.publish(port, at);
            }
            network
        })
    }

    /// Hands the kernel what arrived: the file backend's replies and the network's events.
    /// Whether anything did.
    fn deliver(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let replies = std::mem::take(&mut self.replies);
        let arrivals = self.arrivals.take();
        let any = !replies.is_empty() || !arrivals.is_empty();
        for (request, reply) in replies {
            kernel.event(
                cpu,
                self,
                Event::BackendReply {
                    request,
                    reply: &reply,
                },
            );
        }
        for arrival in &arrivals {
            let Some(network) = &mut self.network else {
                break;
            };
            if let Some(event) = network.event(arrival) {
                kernel.event(cpu, self, event);
            }
        }
        if let Some(files) = &mut self.files {
            for (reason, detail, count) in files.warnings() {
                self.keep(
                    Warning {
                        reason,
                        detail,
                        count,
                    },
                    false,
                );
            }
        }
        any
    }

    /// Every warning so far, the kernel's and the back end's.
    pub fn warnings(&self) -> &[Warned] {
        &self.warnings
    }

    fn now(&self) -> u64 {
        self.started.elapsed().as_nanos() as u64
    }

    /// Drops the deadline of the turn that ended.
    fn turn_over(&self) {
        if let Some(timer) = &self.timer {
            timer.shared.at.store(u64::MAX, Ordering::SeqCst);
        }
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

impl Drop for LocalSystem {
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
            // A deadline a turn ended before, or moved, is not raised.
            if shared
                .at
                .compare_exchange(at, u64::MAX, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                flag.raise();
            }
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

impl Host for LocalSystem {
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

    /// Served at once: the reply is handed over at the next exit or idle.
    fn backend_send(&mut self, request: BackendRequestId, message: &[u8]) -> Result<(), HostError> {
        let files = self.files.as_mut().ok_or(HostError::NotSupported)?;
        if let Some(reply) = files.serve(message) {
            self.replies.push((request, reply));
        }
        Ok(())
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

    fn connect(
        &mut self,
        connection: ConnectionId,
        address: &[u8],
        port: u16,
        stream: bool,
    ) -> Result<(), HostError> {
        self.network().connect(connection.0, address, port, stream)
    }

    fn send(&mut self, connection: ConnectionId, bytes: &[u8]) -> Result<usize, HostError> {
        self.network().send(connection.0, bytes)
    }

    fn receive(&mut self, connection: ConnectionId, into: &mut [u8]) -> Result<usize, HostError> {
        self.network().receive(connection.0, into)
    }

    fn finish(&mut self, connection: ConnectionId) {
        self.network().finish(connection.0);
    }

    fn close(&mut self, connection: ConnectionId) {
        self.network().close(connection.0);
    }

    fn look_up(&mut self, request: u64, name: &str) -> Result<(), HostError> {
        self.network().look_up(request, name);
        Ok(())
    }

    fn listen(&mut self, listener: ConnectionId, port: u16) -> Result<(), HostError> {
        self.network().listen(listener.0, port)
    }

    fn accept(
        &mut self,
        listener: ConnectionId,
        connection: ConnectionId,
    ) -> Result<(), HostError> {
        self.network().accept(listener.0, connection.0)
    }
}

impl System for LocalSystem {
    /// The local system never ends a run itself.
    type End = core::convert::Infallible;

    fn stop_flag(&mut self, flag: StopFlag) {
        self.arrivals.set_flag(flag.clone());
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

    fn arrived(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        self.deliver(kernel, cpu);
        false
    }

    /// The turn is over, so its deadline is dropped: an idle past it raises nothing.
    fn stopped<E: Execution>(&mut self, _: &mut Kernel, _: &mut E) -> ControlFlow<Self::End> {
        self.turn_over();
        ControlFlow::Continue(())
    }

    /// Hands over what arrived, or else waits until the kernel's next timer or an arrival. With
    /// neither to come, the container waits for ever, as it would on Linux.
    fn idle(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu, until_nanoseconds: Option<u64>) {
        if self.deliver(kernel, cpu) {
            return;
        }
        let left =
            until_nanoseconds.map(|until| Duration::from_nanos(until.saturating_sub(self.now())));
        if left != Some(Duration::ZERO) {
            self.arrivals.wait(left);
            self.deliver(kernel, cpu);
        }
        if until_nanoseconds.is_some_and(|until| self.now() >= until) {
            kernel.event(cpu, self, Event::Timer);
        }
    }

    fn serve_directory(&mut self, source: &Path) -> std::io::Result<()> {
        self.files = Some(Files::new(source)?);
        Ok(())
    }

    fn cpu_warning(&mut self, warning: Warning<'_>) {
        self.keep(warning, true);
    }
}

#[cfg(test)]
mod tests {
    use super::{LocalSystem, Ordering};
    use crate::{StopFlag, System};
    use std::time::Duration;

    const SLICE: Duration = Duration::from_millis(1);
    /// Long enough that the turn surely ends before it.
    const LONG_SLICE: Duration = Duration::from_millis(200);
    const PAST_IT: Duration = Duration::from_millis(400);

    fn system() -> (LocalSystem, StopFlag) {
        let flag = StopFlag::default();
        let mut system = LocalSystem::new(Box::new(std::io::sink()), Box::new(std::io::sink()));
        system.stop_flag(flag.clone());
        (system, flag)
    }

    #[test]
    fn a_turn_past_its_deadline_is_stopped() {
        let (mut system, flag) = system();
        system.turn(Some(SLICE.as_nanos() as u64));
        std::thread::sleep(PAST_IT);
        assert!(flag.take());
    }

    #[test]
    fn an_idle_past_an_ended_turns_deadline_raises_nothing() {
        let (mut system, flag) = system();
        system.turn(Some(LONG_SLICE.as_nanos() as u64));
        system.turn_over();
        std::thread::sleep(PAST_IT);
        assert!(!flag.take());
        let timer = system.timer.as_ref().expect("a timer");
        assert_eq!(timer.shared.at.load(Ordering::SeqCst), u64::MAX);
    }
}
