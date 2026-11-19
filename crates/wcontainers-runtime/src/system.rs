//! The local system for an operating system: its clocks and randomness, the container's standard
//! input, output and error (or its terminal), a directory of the host as the file backend, the host's network, free turns ended by a
//! timer thread at the kernel's slice or by an arrival from outside, and waiting in real time.

use core::ops::ControlFlow;
use std::io::{ErrorKind, Read, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wcontainers_abi::{
    BackendRequestId, ConnectionId, Cpu, End, Event, Host, HostError, PAGE_SIZE, Warning,
};
use wcontainers_kernel::{Kernel, Standard};

use crate::arrivals::{Arrival, Arrivals};
use crate::files::Files;
use crate::net::Network;
use crate::{Execution, StopFlag, System, TerminalSize, Turn};

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

/// The ends this system names.
#[repr(u32)]
enum Named {
    Input,
    Output,
    Error,
    Terminal,
}

const INPUT: End = End(Named::Input as u32);
const OUTPUT: End = End(Named::Output as u32);
const ERROR: End = End(Named::Error as u32);
const TERMINAL: End = End(Named::Terminal as u32);

/// The most the input's reader reads at once: a page, the unit a pipe's room is granted in.
const INPUT_PIECE: u64 = PAGE_SIZE;

/// Why the standard input stopped early, and why output was lost.
const INPUT_NOT_READ: &str = "standard input not read";
const OUTPUT_NOT_WRITTEN: &str = "standard output not written";
const ERROR_NOT_WRITTEN: &str = "standard error not written";

/// The names of the system's threads.
const TIMER_THREAD: &str = "wcontainers timer";
const INPUT_THREAD: &str = "wcontainers input";

/// The local system.
pub struct LocalSystem {
    started: Instant,
    output: Box<dyn Write + Send>,
    error: Box<dyn Write + Send>,
    /// Output ends not yet given their credit: all the system will ever take, as it writes
    /// everything as it comes.
    credit_owed: Vec<End>,
    /// Ends the kernel closed, which the system lets go of in turn.
    closing: Vec<End>,
    /// The container's standard input, until its reader starts.
    input: Option<Box<dyn Read + Send>>,
    /// The end the input goes in by, once named, and whether the system has let go of it.
    input_end: Option<End>,
    input_closed: bool,
    input_credit: Arc<InputCredit>,
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
            credit_owed: Vec::new(),
            closing: Vec::new(),
            input: None,
            input_end: None,
            input_closed: false,
            input_credit: Arc::default(),
            warnings: Vec::new(),
            timer: None,
            files: None,
            replies: Vec::new(),
            arrivals: Arc::default(),
            network: None,
            published: Vec::new(),
        }
    }

    /// Gives the container's first process `input` as its standard input, as `docker run --interactive`
    /// does; without it, descriptor 0 is `/dev/null`. Read on a thread of its own, never more than
    /// the kernel has room for.
    pub fn input(&mut self, input: Box<dyn Read + Send>) {
        self.input = Some(input);
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

    /// Hands the kernel what arrived: the output ends' credit and closes, the file backend's
    /// replies and the network's events. Whether anything did.
    fn deliver(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let credit_owed = std::mem::take(&mut self.credit_owed);
        let closing = std::mem::take(&mut self.closing);
        let replies = std::mem::take(&mut self.replies);
        let arrivals = self.arrivals.take();
        let any = !credit_owed.is_empty()
            || !closing.is_empty()
            || !replies.is_empty()
            || !arrivals.is_empty();
        for end in credit_owed {
            kernel.event(
                cpu,
                self,
                Event::StreamCredit {
                    end,
                    bytes: u64::MAX,
                },
            );
        }
        for end in closing {
            kernel.event(cpu, self, Event::StreamClosed { end });
        }
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
        for arrival in arrivals {
            match arrival {
                Arrival::Input(bytes) => self.give_input(kernel, cpu, &bytes),
                Arrival::InputEnded(error) => self.input_ended(kernel, cpu, error),
                arrival => {
                    let Some(network) = &mut self.network else {
                        continue;
                    };
                    if let Some(event) = network.event(&arrival) {
                        kernel.event(cpu, self, event);
                    }
                }
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

    /// Hands the kernel bytes of the input, unless one side has let go of its end.
    fn give_input(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu, bytes: &[u8]) {
        if let Some(end) = self.input_end.filter(|_| !self.input_closed) {
            kernel.event(cpu, self, Event::StreamData { end, bytes });
        }
    }

    /// The input ended: the system lets go of a pipe's end, so the guest reads end of file. A
    /// terminal's end stays, since its output goes on; the guest reads nothing more from it.
    fn input_ended(
        &mut self,
        kernel: &mut Kernel,
        cpu: &mut dyn Cpu,
        error: Option<std::io::Error>,
    ) {
        if let Some(error) = error {
            let detail = error.raw_os_error().map_or(u64::MAX, |code| code as u64);
            let count = self
                .warnings
                .iter()
                .find(|w| w.reason == INPUT_NOT_READ)
                .map_or(1, |w| w.count + 1);
            self.keep(
                Warning {
                    reason: INPUT_NOT_READ,
                    detail,
                    count,
                },
                false,
            );
        }
        if self.input_end == Some(INPUT) && !self.input_closed {
            self.input_closed = true;
            kernel.event(cpu, self, Event::StreamClosed { end: INPUT });
        }
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
    /// Stops the timer thread, and the input's reader once it next waits for room (one blocked
    /// reading is left to end with the process).
    fn drop(&mut self) {
        self.input_credit.stop();
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

/// The room the kernel gave for input and the reader has not used, shared with the reader.
#[derive(Default)]
struct InputCredit {
    state: Mutex<Credit>,
    changed: Condvar,
}

#[derive(Default)]
struct Credit {
    bytes: u64,
    stopped: bool,
}

impl InputCredit {
    fn give(&self, bytes: u64) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.bytes = state.bytes.saturating_add(bytes);
        self.changed.notify_all();
    }

    fn stop(&self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .stopped = true;
        self.changed.notify_all();
    }

    /// Waits for room, and takes up to `most` bytes of it; none once stopped.
    fn wait(&self, most: u64) -> Option<u64> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if state.stopped {
                return None;
            }
            if state.bytes > 0 {
                let taken = state.bytes.min(most);
                state.bytes -= taken;
                return Some(taken);
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Gives back room taken and not used.
    fn unused(&self, bytes: u64) {
        self.give(bytes);
    }
}

/// The input's reader: reads within the room the kernel gave, and hands each piece over.
fn read_input(mut input: Box<dyn Read + Send>, credit: &InputCredit, arrivals: &Arrivals) {
    let mut buffer = vec![0; INPUT_PIECE as usize];
    while let Some(room) = credit.wait(INPUT_PIECE) {
        match input.read(&mut buffer[..room as usize]) {
            Ok(0) => {
                arrivals.push(Arrival::InputEnded(None));
                return;
            }
            Ok(read) => {
                credit.unused(room - read as u64);
                arrivals.push(Arrival::Input(buffer[..read].to_vec()));
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => credit.unused(room),
            Err(error) => {
                arrivals.push(Arrival::InputEnded(Some(error)));
                return;
            }
        }
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

    fn stream_send(&mut self, end: End, bytes: &[u8]) {
        let (writer, name) = match end {
            ERROR => (&mut self.error, ERROR_NOT_WRITTEN),
            _ => (&mut self.output, OUTPUT_NOT_WRITTEN),
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

    /// Room for input; the kernel gives none on an end that takes nothing.
    fn stream_credit(&mut self, end: End, bytes: u64) {
        if Some(end) == self.input_end {
            self.input_credit.give(bytes);
        }
    }

    fn stream_close(&mut self, end: End) {
        if Some(end) == self.input_end {
            self.input_credit.stop();
            self.input_closed = true;
        }
        self.closing.push(end);
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
                .name(TIMER_THREAD.to_string())
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

    /// The terminal asked for, or pipes; with an input, its reader starts.
    fn standard(&mut self, terminal: Option<TerminalSize>) -> Standard {
        let standard = match terminal {
            Some(size) => {
                self.credit_owed = vec![TERMINAL];
                self.input_end = self.input.is_some().then_some(TERMINAL);
                Standard::Terminal {
                    end: TERMINAL,
                    columns: size.columns,
                    rows: size.rows,
                }
            }
            None => {
                self.credit_owed = vec![OUTPUT, ERROR];
                self.input_end = self.input.is_some().then_some(INPUT);
                Standard::Pipes {
                    input: self.input_end,
                    output: OUTPUT,
                    error: ERROR,
                }
            }
        };
        if let Some(input) = self.input.take() {
            let credit = self.input_credit.clone();
            let arrivals = self.arrivals.clone();
            std::thread::Builder::new()
                .name(INPUT_THREAD.to_string())
                .spawn(move || read_input(input, &credit, &arrivals))
                .expect("an input thread");
        }
        standard
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
    const PAST_IT: Duration = LONG_SLICE.saturating_mul(2);

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
