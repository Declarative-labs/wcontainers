//! The local system for an operating system: its clocks and randomness, the container's standard
//! input, output and error (or its terminal), a directory of the host as the file backend, the host's network, free turns ended by a
//! timer thread at the kernel's slice or by an arrival from outside, and waiting in real time.

use core::ops::ControlFlow;
use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::task::{Poll, Waker};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wcontainers_abi::{Cpu, Event, Host, HostError, PAGE_SIZE, Warning};
use wcontainers_kernel_linux::{Kernel, Standard};
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor, UnconditionalEvents};
use wcontainers_system::StreamTransport;

use crate::arrivals::{Arrival, Arrivals};
use crate::files::Files;
use crate::net::Network;
use crate::{Execution, StopFlag, System, TerminalSize, Turn};

/// A warning as the system keeps it: the kernel's or the executor's, by reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warned {
    pub reason: String,
    /// The detail it was last warned with.
    pub detail: u64,
    /// How many times so far, as the last warning counted.
    pub count: u64,
    /// From the executor, not the kernel.
    pub cpu: bool,
}

/// The stream endpoints this system names.
#[repr(u32)]
enum Named {
    Input,
    Output,
    Error,
    Terminal,
}

const INPUT: u32 = Named::Input as u32;
const OUTPUT: u32 = Named::Output as u32;
const ERROR: u32 = Named::Error as u32;
const TERMINAL: u32 = Named::Terminal as u32;

/// The most the input's reader reads ahead of what the kernel took: a page.
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
    state: Mutex<LocalState>,
    /// Set with the stop flag, before the first turn.
    timer: OnceLock<Timer>,
}

/// What the local system changes as the kernel asks.
pub(crate) struct LocalState {
    started: Instant,
    output: Box<dyn Write + Send>,
    error: Box<dyn Write + Send>,
    /// The container's standard input, until its reader starts.
    input: Option<Box<dyn Read + Send>>,
    /// The endpoint the input goes in by, once named, and whether the kernel has let go of it.
    input_end: Option<u32>,
    input_closed: bool,
    input_credit: Arc<InputCredit>,
    /// What the reader read that the kernel has not taken, and whether the input ended.
    input_held: Vec<u8>,
    input_ended: bool,
    /// The waker of each endpoint's wait.
    stream_wakers: BTreeMap<u32, Waker>,
    warnings: Vec<Warned>,
    files: Option<Files>,
    arrivals: Arc<Arrivals>,
    /// Made at the container's first use of the network.
    network: Option<Network>,
    published: Vec<(u16, SocketAddr)>,
}

/// Raises the stop flag at the deadline the last turn set, on its own thread.
struct Timer {
    started: Instant,
    shared: Arc<Deadline>,
    thread: Option<JoinHandle<()>>,
}

/// A deadline that never comes.
const NEVER: u64 = u64::MAX;

struct Deadline {
    /// Nanoseconds after `started` at which the running turn ends; `NEVER` for no end.
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
        let state = LocalState {
            started: Instant::now(),
            output,
            error,
            input: None,
            input_end: None,
            input_closed: false,
            input_credit: Arc::default(),
            input_held: Vec::new(),
            input_ended: false,
            stream_wakers: BTreeMap::new(),
            warnings: Vec::new(),
            files: None,
            arrivals: Arc::default(),
            network: None,
            published: Vec::new(),
        };
        Self {
            state: Mutex::new(state),
            timer: OnceLock::new(),
        }
    }

    /// Gives the container's first process `input` as its standard input, as `docker run --interactive`
    /// does; without it, descriptor 0 is `/dev/null`. Read on a thread of its own, never more than
    /// the kernel has room for.
    pub fn input(&mut self, input: Box<dyn Read + Send>) {
        self.state_mut().input = Some(input);
    }

    /// Lets connections from the host at `at` reach the container's TCP port `port`, as `docker run
    /// --publish` does. Nothing outside reaches a port not published.
    pub fn publish(&mut self, port: u16, at: SocketAddr) {
        let state = self.state_mut();
        state.published.push((port, at));
        if let Some(network) = &mut state.network {
            network.publish(port, at);
        }
    }

    /// Every warning so far, the kernel's and the executor's.
    pub fn warnings(&self) -> Vec<Warned> {
        self.state().warnings.clone()
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, LocalState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn state_mut(&mut self) -> &mut LocalState {
        self.state.get_mut().unwrap_or_else(PoisonError::into_inner)
    }

    /// Drops the deadline of the turn that ended.
    fn turn_over(&self) {
        if let Some(timer) = self.timer.get() {
            timer.shared.at.store(NEVER, Ordering::SeqCst);
        }
    }
}

impl LocalSystem {
    /// Takes what arrived: the input, and the network's readiness, none of it while the state is
    /// held. Whether anything did.
    fn deliver(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let arrivals = self.state().arrivals.take();
        let any = !arrivals.is_empty();
        for arrival in arrivals {
            match arrival {
                Arrival::Input(bytes) => self.state().input_arrived(&bytes),
                Arrival::InputEnded(error) => self.state().input_ended(error),
                arrival => {
                    let event = match &mut self.state().network {
                        Some(network) => network.event(&arrival),
                        None => continue,
                    };
                    if let Some(event) = event {
                        kernel.event(cpu, self, event);
                    }
                }
            }
        }
        let mut state = self.state();
        let warnings: Vec<_> = state
            .files
            .as_mut()
            .map(|files| files.warnings())
            .unwrap_or_default();
        for (reason, detail, count) in warnings {
            state.keep(
                Warning {
                    reason,
                    detail,
                    count,
                },
                false,
            );
        }
        any
    }
}

impl LocalState {
    /// Bytes of the input, held until the kernel takes them, unless it has let go of its end.
    fn input_arrived(&mut self, bytes: &[u8]) {
        if !self.input_closed {
            self.input_held.extend_from_slice(bytes);
            self.wake_streams();
        }
    }

    /// The input ended: once the kernel has taken what is held, a pipe's endpoint gives no more,
    /// so the guest reads end of file; a terminal's stays, since its output goes on.
    fn input_ended(&mut self, error: Option<std::io::Error>) {
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
        self.input_ended = true;
        self.wake_streams();
    }

    fn wake_streams(&mut self) {
        for (_, waker) in std::mem::take(&mut self.stream_wakers) {
            waker.wake();
        }
    }

    /// What an endpoint is ready for: output is written as it comes, input once some is held or
    /// it ended.
    fn stream_ready(&self, peer: u32) -> ReadinessReport {
        let mut ready = ReadinessReport {
            ready_for: ReadyFor::empty(),
            unconditional: UnconditionalEvents::empty(),
        };
        if peer != INPUT {
            ready.ready_for |= ReadyFor::WRITABLE;
        }
        if Some(peer) == self.input_end {
            // A terminal's input ending leaves the terminal, whose output goes on.
            let ended = self.input_ended && peer == INPUT;
            if !self.input_held.is_empty() || ended {
                ready.ready_for |= ReadyFor::READABLE;
            }
            if ended {
                ready.unconditional |= UnconditionalEvents::HANG_UP;
            }
        }
        ready
    }
}

impl LocalState {
    pub(crate) fn network(&mut self) -> &mut Network {
        self.network.get_or_insert_with(|| {
            let mut network = Network::new(self.arrivals.clone());
            for &(port, at) in &self.published {
                network.publish(port, at);
            }
            network
        })
    }

    fn now(&self) -> u64 {
        nanoseconds_since(self.started)
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
        self.state_mut().input_credit.stop();
        if let Some(timer) = self.timer.get_mut() {
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

fn nanoseconds_since(started: Instant) -> u64 {
    started.elapsed().as_nanos() as u64
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

/// What the kernel asks of the host, done on the state the local system holds.
impl LocalState {
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

    /// Served at once; a message with no reply has an empty one.
    fn serve(&mut self, message: &[u8]) -> Result<Vec<u8>, HostError> {
        let files = self.files.as_mut().ok_or(HostError::NotSupported)?;
        Ok(files.serve(message).unwrap_or_default())
    }

    fn send_stream(&mut self, peer: u32, bytes: &[u8]) -> Result<usize, HostError> {
        let (writer, name) = match peer {
            ERROR => (&mut self.error, ERROR_NOT_WRITTEN),
            _ => (&mut self.output, OUTPUT_NOT_WRITTEN),
        };
        let Err(error) = writer.write_all(bytes).and_then(|()| writer.flush()) else {
            return Ok(bytes.len());
        };
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
        Err(HostError::Closed)
    }

    /// Input held for the kernel; the reader may read as much again.
    fn receive_stream(&mut self, peer: u32, into: &mut [u8]) -> Result<usize, HostError> {
        if Some(peer) != self.input_end {
            return Err(HostError::NotSupported);
        }
        if self.input_held.is_empty() {
            return Err(match self.input_ended && peer == INPUT {
                true => HostError::Closed,
                false => HostError::WouldBlock,
            });
        }
        let count = into.len().min(self.input_held.len());
        into[..count].copy_from_slice(&self.input_held[..count]);
        self.input_held.drain(..count);
        self.input_credit.give(count as u64);
        Ok(count)
    }

    fn close_stream(&mut self, peer: u32) {
        if Some(peer) == self.input_end {
            self.input_credit.stop();
            self.input_closed = true;
            self.input_held.clear();
        }
        self.stream_wakers.remove(&peer);
    }
}

/// The kernel's calls, each made on the state held for it.
macro_rules! host_on_state {
    ($host:ty, $this:ident => $state:expr) => {
        impl wcontainers_clock::Clock for $host {
            fn monotonic_nanoseconds(&self) -> u64 {
                let $this = self;
                $state.monotonic_nanoseconds()
            }

            fn realtime_nanoseconds(&self) -> u64 {
                let $this = self;
                $state.realtime_nanoseconds()
            }
        }

        impl wcontainers_random_source::RandomSource for $host {
            fn fill_random_bytes(&self, into: &mut [u8]) -> Result<(), HostError> {
                let $this = self;
                $state.random(into);
                Ok(())
            }
        }

        impl Host for $host {
            fn warn(&self, warning: Warning<'_>) {
                let $this = self;
                $state.warn(warning);
            }
        }

        impl StreamTransport for $host {
            type Endpoint = u32;

            fn send_stream(&self, peer: &u32, bytes: &[u8]) -> Result<usize, HostError> {
                let $this = self;
                $state.send_stream(*peer, bytes)
            }

            fn receive_stream(&self, peer: &u32, into: &mut [u8]) -> Result<usize, HostError> {
                let $this = self;
                $state.receive_stream(*peer, into)
            }

            fn close_stream(&self, peer: &u32) {
                let $this = self;
                $state.close_stream(*peer);
            }
        }

        impl Readiness<u32> for $host {
            async fn wait_until_ready(
                &self,
                peer: &u32,
                interest: ReadyFor,
            ) -> Result<ReadinessReport, HostError> {
                let $this = self;
                std::future::poll_fn(|context| {
                    let mut state = $state;
                    let ready = state.stream_ready(*peer);
                    if ready.ready_for.intersects(interest) || !ready.unconditional.is_empty() {
                        return Poll::Ready(Ok(ready));
                    }
                    state.stream_wakers.insert(*peer, context.waker().clone());
                    Poll::Pending
                })
                .await
            }
        }

        /// Every task id is free.
        impl wcontainers_system::TaskIdAllocator for $host {
            fn free_task_id(&self, _: wcontainers_kernel_linux::TaskId) {}
        }

        /// No hook does anything.
        impl wcontainers_kernel_linux::System for $host {}
    };
}

host_on_state!(LocalSystem, system => system.state());

impl wcontainers_system::FuseServer for LocalSystem {
    type Reply = Vec<u8>;
    type Notice = Vec<u8>;

    /// This system's file server sends no notices.
    async fn receive_notice(
        &self,
        applied: Option<Result<(), wcontainers_kernel::errno::Errno>>,
    ) -> Result<Vec<u8>, HostError> {
        let _ = applied;
        Err(HostError::NotSupported)
    }

    async fn send_request(&self, request: &[u8]) -> Result<Vec<u8>, HostError> {
        self.state().serve(request)
    }
}

impl wcontainers_system::DomainNameResolver for LocalSystem {
    type AnswerBytes = Vec<u8>;

    async fn resolve_domain(
        &self,
        question: domain::base::Question<&domain::base::Name<[u8]>>,
    ) -> Result<crate::names::NameAnswer, HostError> {
        let arrivals = self.state().arrivals.clone();
        crate::names::resolve(arrivals, question).await
    }
}

impl System for LocalSystem {
    /// The local system never ends a run itself.
    type End = core::convert::Infallible;

    /// Takes what arrived before the start.
    fn started(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut dyn Cpu) {
        self.deliver(kernel, cpu);
    }

    fn stop_flag(&self, flag: StopFlag) {
        let state = self.state();
        state.arrivals.set_flag(flag.clone());
        let shared = Arc::new(Deadline {
            at: AtomicU64::new(NEVER),
            sleeping: AtomicU64::new(NEVER),
            gone: Mutex::new(false),
            wake: Condvar::new(),
        });
        let started = state.started;
        self.timer.get_or_init(|| {
            let thread = {
                let shared = shared.clone();
                std::thread::Builder::new()
                    .name(TIMER_THREAD.to_string())
                    .spawn(move || time(&shared, started, &flag))
                    .expect("a timer thread")
            };
            Timer {
                started,
                shared,
                thread: Some(thread),
            }
        });
    }

    /// A free turn, ended by the timer thread when the kernel's slice has passed.
    fn turn(&self, slice_nanoseconds: Option<u64>) -> Turn {
        if let Some(timer) = self.timer.get() {
            let at = slice_nanoseconds.map_or(NEVER, |slice| {
                nanoseconds_since(timer.started).saturating_add(slice)
            });
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
    fn ran(&self, _: u64) {}

    fn arrived(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        self.deliver(kernel, cpu);
        false
    }

    /// The turn is over, so its deadline is dropped: an idle past it raises nothing.
    fn stopped<E: Execution>(
        self: &Arc<Self>,
        _: &mut Kernel,
        _: &mut E,
    ) -> ControlFlow<Self::End> {
        self.turn_over();
        ControlFlow::Continue(())
    }

    /// Hands over what arrived, or else waits until the kernel's next timer or an arrival. With
    /// neither to come, the container waits for ever, as it would on Linux. The state is not held
    /// while waiting.
    fn idle(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut dyn Cpu,
        until_nanoseconds: Option<u64>,
    ) {
        if self.deliver(kernel, cpu) {
            return;
        }
        let (arrivals, now) = {
            let state = self.state();
            (state.arrivals.clone(), state.now())
        };
        let left = until_nanoseconds.map(|until| Duration::from_nanos(until.saturating_sub(now)));
        if left != Some(Duration::ZERO) {
            arrivals.wait(left);
            self.deliver(kernel, cpu);
        }
        if until_nanoseconds.is_some_and(|until| self.state().now() >= until) {
            kernel.event(cpu, self, Event::Timer);
        }
    }

    fn serve_directory(&self, source: &Path) -> std::io::Result<()> {
        self.state().files = Some(Files::new(source)?);
        Ok(())
    }

    fn cpu_warning(&self, warning: Warning<'_>) {
        self.state().keep(warning, true);
    }

    /// The terminal asked for, or pipes; with an input, its reader starts.
    fn standard(&self, terminal: Option<TerminalSize>) -> Standard<u32> {
        let mut state = self.state();
        let standard = match terminal {
            Some(size) => {
                state.input_end = state.input.is_some().then_some(TERMINAL);
                Standard::Terminal {
                    end: TERMINAL,
                    columns: size.columns,
                    rows: size.rows,
                }
            }
            None => {
                state.input_end = state.input.is_some().then_some(INPUT);
                Standard::Pipes {
                    input: state.input_end,
                    output: OUTPUT,
                    error: ERROR,
                }
            }
        };
        if let Some(input) = state.input.take() {
            let credit = state.input_credit.clone();
            credit.give(INPUT_PIECE);
            let arrivals = state.arrivals.clone();
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
    use super::{LocalSystem, NEVER, Ordering};
    use crate::{StopFlag, System};
    use std::time::Duration;

    const SLICE: Duration = Duration::from_millis(1);
    /// Long enough that the turn surely ends before it.
    const LONG_SLICE: Duration = Duration::from_millis(200);
    const PAST_IT: Duration = LONG_SLICE.saturating_mul(2);

    fn system() -> (LocalSystem, StopFlag) {
        let flag = StopFlag::default();
        let system = LocalSystem::new(Box::new(std::io::sink()), Box::new(std::io::sink()));
        system.stop_flag(flag.clone());
        (system, flag)
    }

    #[test]
    fn a_turn_past_its_deadline_is_stopped() {
        let (system, flag) = system();
        system.turn(Some(SLICE.as_nanos() as u64));
        std::thread::sleep(PAST_IT);
        assert!(flag.take());
    }

    #[test]
    fn an_idle_past_an_ended_turns_deadline_raises_nothing() {
        let (system, flag) = system();
        system.turn(Some(LONG_SLICE.as_nanos() as u64));
        system.turn_over();
        std::thread::sleep(PAST_IT);
        assert!(!flag.take());
        let timer = system.timer.get().expect("a timer");
        assert_eq!(timer.shared.at.load(Ordering::SeqCst), NEVER);
    }
}
