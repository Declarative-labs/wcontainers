//! The local system for an operating system: its clocks and randomness, the container's standard
//! input, output and error (or its terminal), a directory of the host as the file backend, the host's network, and a
//! thread that hands the kernel what arrives from outside as it comes.

use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use std::task::{Poll, Waker};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use wcontainers_abi::{HostError, PAGE_SIZE, Warning};
use wcontainers_kernel_linux::Standard;
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor, UnconditionalEvents};
use wcontainers_system::StreamTransport;
use wcontainers_volatile_bytes::VolatileBytes;

use crate::arrivals::{Arrival, Arrivals};
use crate::files::Files;
use crate::net::Network;
use crate::timer::nanoseconds_since;
use crate::{StopFlag, System, TerminalSize};

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

/// The name of the thread that reads the input.
const INPUT_THREAD: &str = "wcontainers input";

/// The name of the thread that hands the kernel what arrives.
const DELIVERY_THREAD: &str = "wcontainers delivery";

/// The local system, serving a kernel whose threads run on the executor `E`.
pub struct LocalSystem<E = ()> {
    state: Mutex<LocalState>,
    /// The memory programs are told the machine has, set when the container is made.
    pub(crate) machine_memory: OnceLock<u64>,
    /// Made at the container's first use of each.
    pub(crate) icmp: OnceLock<Result<wcontainers_icmp_sockets_host::HostIcmpSockets, HostError>>,
    pub(crate) raw: OnceLock<Result<wcontainers_raw_sockets_host::HostRawSockets, HostError>>,
    pub(crate) routes: OnceLock<wcontainers_route_resolver_host::HostRouteResolver>,
    executor: PhantomData<fn() -> E>,
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

impl LocalSystem {
    /// A system writing the container's standard output and error to these streams.
    pub fn new(output: Box<dyn Write + Send>, error: Box<dyn Write + Send>) -> Self {
        Self::serving(output, error)
    }
}

impl<E> LocalSystem<E> {
    /// A system for a kernel on the executor `E`, writing the container's standard output and error to these streams.
    pub fn serving(output: Box<dyn Write + Send>, error: Box<dyn Write + Send>) -> Self {
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
            machine_memory: OnceLock::new(),
            icmp: OnceLock::new(),
            raw: OnceLock::new(),
            routes: OnceLock::new(),
            executor: PhantomData,
        }
    }

    /// Sets the memory size in bytes programs are told the machine has, as the container is made; else the size set before.
    pub fn describe_memory(&self, bytes: u64) -> Result<(), u64> {
        self.machine_memory.set(bytes)
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

    /// Keeps a slow path of the executor, apart from the kernel's fallbacks.
    pub fn cpu_warning(&self, warning: Warning<'_>) {
        self.state().keep(warning, true);
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, LocalState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn state_mut(&mut self) -> &mut LocalState {
        self.state.get_mut().unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes what arrived: the input, and the network's readiness, the state held for each in turn.
    fn deliver(&self) {
        let arrivals = self.state().arrivals.take();
        for arrival in arrivals {
            let mut state = self.state();
            match arrival {
                Arrival::Input(bytes) => state.input_arrived(&bytes),
                Arrival::InputEnded(error) => state.input_ended(error),
                arrival => {
                    if let Some(network) = &mut state.network {
                        network.arrived(&arrival);
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
    }
}

/// Hands what arrives to the system `system` points to, as it comes, until the system is gone.
fn deliver_arrivals<E>(system: &Weak<LocalSystem<E>>) {
    loop {
        let Some(arrivals) = system
            .upgrade()
            .map(|system| Arc::clone(&system.state().arrivals))
        else {
            return;
        };
        arrivals.wait(None);
        if arrivals.closed() {
            return;
        }
        match system.upgrade() {
            Some(system) => system.deliver(),
            None => return,
        }
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

    pub(crate) fn keep(&mut self, warning: Warning<'_>, cpu: bool) {
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

impl<E> Drop for LocalSystem<E> {
    /// Stops the input's reader once it next waits for room (one blocked reading is left to end with
    /// the process), and the delivery thread.
    fn drop(&mut self) {
        let state = self.state_mut();
        state.input_credit.stop();
        state.arrivals.close();
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
            // A terminal with no input is one nobody types on, as `docker run --tty` without `--interactive` gives.
            return Err(match peer {
                TERMINAL => HostError::WouldBlock,
                _ => HostError::NotSupported,
            });
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
        impl<E> wcontainers_clock::Clock for $host {
            fn monotonic_nanoseconds(&self) -> u64 {
                let $this = self;
                $state.monotonic_nanoseconds()
            }

            fn realtime_nanoseconds(&self) -> u64 {
                let $this = self;
                $state.realtime_nanoseconds()
            }
        }

        impl<E> wcontainers_random_source::RandomSource for $host {
            fn fill_random_bytes(&self, into: &impl VolatileBytes) -> Result<(), HostError> {
                let $this = self;
                crate::host_call::receive_into(into, |bytes| {
                    $state.random(bytes);
                    Ok(bytes.len())
                })?;
                Ok(())
            }
        }

        impl<E> StreamTransport for $host {
            type Endpoint = u32;

            fn send_stream(
                &self,
                peer: &u32,
                payload: &impl VolatileBytes,
            ) -> Result<usize, HostError> {
                let $this = self;
                crate::host_call::send_from(payload, |bytes| $state.send_stream(*peer, bytes))
            }

            fn receive_stream(
                &self,
                peer: &u32,
                into: &impl VolatileBytes,
            ) -> Result<usize, HostError> {
                let $this = self;
                crate::host_call::receive_into(into, |bytes| $state.receive_stream(*peer, bytes))
            }

            fn close_stream(&self, peer: &u32) {
                let $this = self;
                $state.close_stream(*peer);
            }
        }

        impl<E> Readiness<u32> for $host {
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
        impl<E> wcontainers_system::TaskIdAllocator for $host {
            fn free_task_id(&self, _: wcontainers_kernel_linux::TaskId) {}
        }

        /// No hook does anything.
        impl<E: 'static> wcontainers_kernel_linux::System for $host {}
    };
}

host_on_state!(LocalSystem<E>, system => system.state());

impl<E> wcontainers_system::FuseServer for LocalSystem<E> {
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

impl<E> wcontainers_system::DomainNameResolver for LocalSystem<E> {
    type AnswerBytes = Vec<u8>;

    async fn resolve_domain(
        &self,
        question: domain::base::Question<&domain::base::Name<[u8]>>,
    ) -> Result<crate::names::NameAnswer, HostError> {
        let arrivals = self.state().arrivals.clone();
        crate::names::resolve(arrivals, question).await
    }
}

impl<E: 'static> System for LocalSystem<E> {
    /// Starts the thread that hands the kernel what arrives, the arrivals before the start among them.
    fn on_container_made(self: &Arc<Self>) {
        let system = Arc::downgrade(self);
        std::thread::Builder::new()
            .name(DELIVERY_THREAD.to_string())
            .spawn(move || deliver_arrivals(&system))
            .expect("a delivery thread");
    }

    /// Arrivals end a free turn.
    fn stop_flag(&self, flag: StopFlag) {
        self.state().arrivals.set_flag(flag);
    }

    fn serve_directory(&self, source: &Path) -> std::io::Result<()> {
        self.state().files = Some(Files::new(source)?);
        Ok(())
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
