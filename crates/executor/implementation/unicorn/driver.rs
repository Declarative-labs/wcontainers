//! The driver loop and a host for tests: runs a program on the kernel with the stand-in CPU.

use std::collections::BTreeMap;
use std::ops::{ControlFlow, Range};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};
use std::task::{Poll, Waker};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use wcontainers_abi::{
    CounterReading, Cpu, Event, Host, HostError, Next, PAGE_SIZE, SystemCallRequest, Warning,
};
use wcontainers_clock::Clock as _;
use wcontainers_kernel::errno::Errno;
use wcontainers_kernel::signal::Signal;
use wcontainers_kernel_linux::{
    AccessKind, FileRecord, Kernel, MemoryObject, Process, SignalInformation, Standard, Start,
    System, SystemCallResponse, TaskId,
};
use wcontainers_runtime_single_kernel::{Ending, Execution, TerminalSize, Turn};
use wcontainers_system_seeded_test_policy::{
    PROGRESS_PICOSECONDS, REPEATABLE_START_NANOSECONDS, Schedule, TURN_NANOSECONDS,
};

use crate::hooks::{BetweenSteps, Hooks, NoHooks};
use crate::replay::{Entry, MemoryDigests, OwnedEvent, Tape};
use crate::standin::Unsupported;
use crate::streams::{StreamClient, StreamHost, Streams};
use crate::{image, reference, streams};

#[path = "tcp.rs"]
mod tcp;
#[path = "udp.rs"]
mod udp;
use tcp::Readied;

/// Why a run on our kernel gave no outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The kernel could not start the program.
    Start(Errno),
    /// The program did something the stand-in CPU cannot run as the reference does.
    StandIn(Unsupported),
}

impl From<Unsupported> for Failure {
    fn from(unsupported: Unsupported) -> Self {
        Self::StandIn(unsupported)
    }
}

/// What runs the guest's instructions behind the kernel: the stand-in CPU.
#[cfg(not(feature = "runtime"))]
pub(crate) type Back = crate::standin::StandIn;

/// What runs the guest's instructions behind the kernel: the translator's WebAssembly.
#[cfg(feature = "runtime")]
pub(crate) type Back = wcontainers_executor_wasmtime::CpuSide<wcontainers_arm64::Arm64>;

#[cfg(not(feature = "runtime"))]
fn back_end(_seed: u64, address_space: u64, wall_clock: bool) -> Back {
    let stand_in = crate::standin::StandIn::with_space(address_space);
    if wall_clock {
        stand_in.on_wall_clock()
    } else {
        stand_in
    }
}

/// One compiler for the test process, so a second container finds the first one's translations, with
/// its cache's directory under the target directory, so every test binary of a build shares them.
/// The address space ends at the back end's own ceiling, from its output's widths, whatever the
/// container asks.
#[cfg(feature = "runtime")]
pub(crate) fn back_end(seed: u64, _address_space: u64, _wall_clock: bool) -> Back {
    use wcontainers_executor_wasmtime::{Compiler, CpuSide, Optimise};
    static COMPILER: std::sync::OnceLock<std::sync::Arc<Compiler<wcontainers_arm64::Arm64>>> =
        std::sync::OnceLock::new();
    let compiler = COMPILER
        .get_or_init(|| Compiler::new(Some(crate::backend::out().join(TRANSLATIONS))))
        .clone();
    CpuSide::new(compiler, look_ahead(seed), Optimise::Speed, ())
}

/// The folder of the translation cache, in the tests' output folder.
#[cfg(feature = "runtime")]
const TRANSLATIONS: &str = "translations";

/// Mixed into the run's seed for the look-ahead's draw, so that it is not the schedule's first draw.
#[cfg(feature = "runtime")]
const LOOK_AHEAD_STREAM: u64 = 0x1007_a4ead;

/// The variable naming the look-ahead: `full` (also when unset), `none`, or `bounded`.
#[cfg(feature = "runtime")]
const LOOK_AHEAD_VARIABLE: &str = "LOOK_AHEAD";

/// The variable mixed into the run's seed for a bounded look-ahead's N (0 if unset).
#[cfg(feature = "runtime")]
const LOOK_AHEAD_SEED_VARIABLE: &str = "LOOK_AHEAD_SEED";

/// The look-ahead [`LOOK_AHEAD_VARIABLE`] names; at `bounded` its N is drawn from 1 to the
/// translator's default bound from the run's seed and [`LOOK_AHEAD_SEED_VARIABLE`], and printed.
#[cfg(feature = "runtime")]
fn look_ahead(seed: u64) -> wcontainers_executor_wasmtime::LookAhead {
    use wcontainers_executor_wasmtime::LookAhead;
    match std::env::var(LOOK_AHEAD_VARIABLE).as_deref() {
        Err(_) | Ok("full") => LookAhead::Full,
        Ok("none") => LookAhead::None,
        Ok("bounded") => {
            let knob = std::env::var(LOOK_AHEAD_SEED_VARIABLE).map_or(0, |seed| {
                seed.parse().unwrap_or_else(|_| {
                    panic!("{LOOK_AHEAD_SEED_VARIABLE} is an unsigned number, not {seed}")
                })
            });
            let n = StdRng::seed_from_u64(seed ^ knob ^ LOOK_AHEAD_STREAM)
                .random_range(1..=wcontainers_executor_wasmtime::BOUND);
            eprintln!(
                "look-ahead bounded {n} (run seed {seed}, {LOOK_AHEAD_SEED_VARIABLE} {knob})"
            );
            LookAhead::Bounded(n)
        }
        Ok(other) => panic!("{LOOK_AHEAD_VARIABLE} is full, none or bounded, not {other}"),
    }
}

#[cfg(feature = "runtime")]
impl From<std::convert::Infallible> for Failure {
    fn from(never: std::convert::Infallible) -> Self {
        match never {}
    }
}

/// What the WebAssembly back end translated for a run, on the standard error.
#[cfg(feature = "runtime")]
fn report(cpu: &Back, container: &Container<'_>, took: Duration) {
    let counters: Vec<_> = cpu
        .counters()
        .iter()
        .map(|c| format!("{} {}", c.reason, c.count))
        .collect();
    eprintln!(
        "cpu side: run of {:?}: {}",
        container.arguments,
        counters.join(", ")
    );
    let translated = cpu.translated();
    let at_run_time = translated.iter().filter(|t| t.at_run_time).count();
    use wcontainers_executor_wasmtime::Origin;
    let made: Vec<(u64, u64)> = translated
        .iter()
        .filter_map(|t| match t.origin {
            Origin::Made {
                translating,
                compiling,
            } => Some((translating, compiling)),
            _ => None,
        })
        .collect();
    let (translating, compiling) = made
        .iter()
        .fold((0, 0), |(t, c), (more_t, more_c)| (t + more_t, c + more_c));
    let disk: Vec<u64> = translated
        .iter()
        .filter_map(|t| match t.origin {
            Origin::Disk(nanoseconds) => Some(nanoseconds),
            _ => None,
        })
        .collect();
    eprintln!(
        "cpu side: {} translations ({} at run time), {} made ({:.1?} translating, {:.1?} compiling), {} from disk ({:.1?}), {} from memory; run {:.1?}",
        translated.len(),
        at_run_time,
        made.len(),
        Duration::from_nanos(translating),
        Duration::from_nanos(compiling),
        disk.len(),
        Duration::from_nanos(disk.iter().sum::<u64>()),
        translated.len() - made.len() - disk.len(),
        took
    );
    for t in translated.iter().filter(|t| !t.at_run_time) {
        eprintln!(
            "cpu side: range {:#x}+{:#x} at {:?}: {} starts {:?}, {} pieces, {:?}",
            t.start, t.length, t.look_ahead, t.starts, t.sources, t.pieces, t.origin
        );
    }
}

/// What a program did, in a form both runners produce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// The exit code as `docker run` reports it: the program's status, or 128 plus the signal that
    /// killed it.
    pub exit_code: i32,
}

/// One warning the kernel gave, with its reason and detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warned {
    pub reason: String,
    pub count: u64,
    pub detail: u64,
}

/// A run on our kernel: what the program did, and what the kernel reported along the way.
#[derive(Debug)]
pub struct Run {
    pub outcome: Outcome,
    pub warnings: Vec<Warned>,
    /// The CPU side's own slow paths, apart from the kernel's.
    pub cpu_warnings: Vec<Warned>,
    pub counters: Vec<CounterReading>,
    /// The root file system when the container ended.
    pub files: Vec<FileRecord>,
    /// How many times it was saved and brought back.
    pub saved: usize,
    /// How many turns on the CPU side it took.
    pub turns: u64,
    /// The most input the kernel asked for at once.
    pub most_input_asked: u64,
    /// What the kernel did against the rules on its ends.
    pub broken: Vec<String>,
    /// Whether the kernel closed every output end once the container ended.
    pub outputs_closed: bool,
    /// Whether the kernel closed the input end before the outside did.
    pub input_closed: bool,
    /// The record of what the host gave, for a run that kept one.
    pub tape: Option<Tape>,
    /// What every guest memory held, for a run that kept a record.
    pub memories: MemoryDigests,
}

/// The host used in tests. Randomness comes from a seed so runs repeat exactly. When the kernel is idle
/// until a time, the host's clocks jump to it instead of waiting, unless the host waits. With
/// `NoHooks` the kernel's own provided defaults answer every hook and task id call.
pub struct TestHost<H = NoHooks> {
    /// The instant the monotonic clock is read against.
    origin: Instant,
    /// The time from `origin` to the start this host's monotonic clock counts from.
    started: AtomicU64,
    /// How far the wall clock has jumped ahead.
    skipped: AtomicU64,
    random: Mutex<StdRng>,
    /// The other sides of the container's ends.
    pub(crate) streams: Mutex<Streams>,
    warnings: Mutex<Vec<Warned>>,
    /// A clock that moves only when the driver says, so a run repeats exactly; none for the wall
    /// clock (layer 1, where the reference runs beside it).
    virtual_now: Option<AtomicU64>,
    /// With a clock that moves only when the driver says, the wall clock's reading when the
    /// monotonic clock read 0: the wall clock moves with it.
    wall_base: AtomicU64,
    /// What a run's choices come from: how long each turn on the CPU side lasts, and when the
    /// backend's answers arrive.
    schedule: Mutex<Schedule>,
    /// With a repeatable clock, the time the kernel's work on a turn's end takes, passed at the
    /// next reading of the clock.
    kernel_work: AtomicU64,
    /// The file backend, for a container with a volume.
    pub(crate) backend: Mutex<Option<crate::backend::Backend>>,
    /// The backend's replies the kernel's calls wait for.
    replies: Mutex<Replies>,
    /// The resolver's answers the kernel's calls wait for, by the number of each question.
    names: Mutex<Replies>,
    /// The number of the last name question.
    last_name: AtomicU64,
    /// What the sockets became ready for, for the kernel's waits on them.
    ready: Mutex<Readied>,
    /// What the stream endpoints became ready for, by number, for the kernel's waits on them.
    stream_ready: Mutex<Readied>,
    /// The number of the last socket: a TCP connection or listener, or a UDP socket.
    last_socket: AtomicU32,
    /// Connections to the outside and lookups.
    network: Mutex<crate::network::Network>,
    /// The CPU side's own slow paths.
    cpu_warnings: Mutex<Vec<Warned>>,
    /// What a driven run has done to it between turns; none outside one.
    injecting: Mutex<Option<Injecting>>,
    /// Whether an idle kernel's waits take their time on the machine's clock, as the reference's
    /// do, rather than being skipped.
    waits: bool,
    /// What the test does at the kernel's hooks.
    hooks: Mutex<H>,
    /// The record of what this host gives the kernel, kept or answered from.
    tape: Option<Mutex<Tape>>,
}

/// The backend's replies that arrived for the kernel's calls and are not taken yet, and the waker of
/// each call that waits for its reply, by request number.
#[derive(Default)]
struct Replies {
    arrived: BTreeMap<u64, Vec<u8>>,
    waiting: BTreeMap<u64, Waker>,
}

/// What a driven run has done to it at moments drawn from its seed, and where it is.
struct Injecting {
    seed: u64,
    injections: Injections,
    /// Turn counts at which it is saved.
    at: Vec<u64>,
    signal: Signal,
    signal_at: Vec<u64>,
    notice_at: Vec<u64>,
    notices: StdRng,
    turns: u64,
    saved: usize,
    /// In a copy, where its run is reported to the process it was copied from.
    report: Option<i32>,
}

impl Injecting {
    fn new(seed: u64, injections: Injections) -> Self {
        let (signal, signals) = injections.signal.unwrap_or((Signal::SIGKILL, 0));
        Self {
            seed,
            injections,
            at: save_moments(seed, injections),
            signal,
            signal_at: moments(seed ^ SIGNALS_SALT, signals),
            notice_at: moments_apart(seed ^ NOTICES_SALT, injections.notices, NOTICES_APART),
            notices: StdRng::seed_from_u64(seed ^ NOTICE_BYTES_SALT),
            turns: 0,
            saved: 0,
            report: None,
        }
    }
}

impl TestHost {
    /// A host whose clock is the wall clock's.
    pub fn new(seed: u64) -> Self {
        Self {
            origin: Instant::now(),
            started: AtomicU64::new(0),
            skipped: AtomicU64::new(0),
            random: Mutex::new(StdRng::seed_from_u64(seed)),
            streams: Mutex::default(),
            warnings: Mutex::default(),
            virtual_now: None,
            wall_base: AtomicU64::new(0),
            schedule: Mutex::new(Schedule::new(seed)),
            kernel_work: AtomicU64::default(),
            backend: Mutex::default(),
            replies: Mutex::default(),
            names: Mutex::default(),
            last_name: AtomicU64::default(),
            ready: Mutex::default(),
            stream_ready: Mutex::default(),
            last_socket: AtomicU32::default(),
            network: Default::default(),
            cpu_warnings: Mutex::default(),
            injecting: Mutex::default(),
            waits: false,
            hooks: Mutex::new(NoHooks),
            tape: None,
        }
    }

    /// A host whose clocks move only with the work run and the waits skipped, both from the seed,
    /// so the kernel's choices repeat exactly for a seed, with the wall clock reading `wall_start`
    /// (the machine's clock when none) at the start.
    pub fn repeatable(seed: u64, wall_start: Option<u64>) -> Self {
        let start = REPEATABLE_START_NANOSECONDS;
        let mut host = Self {
            virtual_now: Some(AtomicU64::new(start)),
            ..Self::new(seed)
        };
        *host.wall_base.get_mut() = wall_start.unwrap_or_else(wall_clock) - start;
        host
    }

    /// The same host with the test's side of the kernel's hooks.
    pub fn with_hooks(self, hooks: Box<dyn Hooks>) -> TestHost<Box<dyn Hooks>> {
        TestHost {
            origin: self.origin,
            started: self.started,
            skipped: self.skipped,
            random: self.random,
            streams: self.streams,
            warnings: self.warnings,
            virtual_now: self.virtual_now,
            wall_base: self.wall_base,
            schedule: self.schedule,
            kernel_work: self.kernel_work,
            backend: self.backend,
            replies: self.replies,
            names: self.names,
            last_name: self.last_name,
            ready: self.ready,
            stream_ready: self.stream_ready,
            last_socket: self.last_socket,
            network: self.network,
            cpu_warnings: self.cpu_warnings,
            injecting: self.injecting,
            waits: self.waits,
            hooks: Mutex::new(hooks),
            tape: self.tape,
        }
    }
}

/// The machine's wall clock, in nanoseconds since 1970.
fn wall_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after the epoch")
        .as_nanos() as u64
}

impl<H: Send> TestHost<H> {
    /// Brings a saved container's host back as a new one. What the container wrote has already
    /// left; the monotonic clock reads from an earlier start than this one's (a kernel that did not
    /// carry its clock over would see it go back) and the wall clock goes on from the save (a
    /// repeatable one without the time the save took, moved on by `wall_step`); randomness and the
    /// length of turns come from `seed`. The backend and the answers it owes, the streams and the
    /// test's hooks go on.
    fn move_on(&self, seed: u64, wall_step: u64) {
        let wall = self.realtime_nanoseconds();
        self.started
            .store(self.origin.elapsed().as_nanos() as u64, Ordering::Relaxed);
        *lock(&self.random) = StdRng::seed_from_u64(seed);
        if let Some(now) = &self.virtual_now {
            now.store(0, Ordering::Relaxed);
        }
        *lock(&self.schedule) = Schedule::new(seed);
        self.kernel_work.store(0, Ordering::Relaxed);
        // A new host has none of the old one's connections; its published ports go on.
        {
            let mut network = lock(&self.network);
            let moved = network.moved();
            *network = moved;
        }
        *lock(&self.ready) = tcp::Readied::default();
        *lock(&self.stream_ready) = tcp::Readied::default();
        self.wall_base.store(0, Ordering::Relaxed);
        let base = wall + wall_step - self.monotonic_nanoseconds();
        self.wall_base.store(base, Ordering::Relaxed);
    }

    /// The test's hooks for a call the kernel makes; one made while another is running nested in
    /// it, which is reported rather than waited on.
    pub fn hook(&self) -> MutexGuard<'_, H> {
        match self.hooks.try_lock() {
            Ok(hooks) => hooks,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => panic!("a hook nested in another"),
        }
    }

    /// Moves the clocks forward to a time on the monotonic clock, as if the host had waited.
    pub fn skip_to(&self, until: u64) {
        let now = self.monotonic_nanoseconds();
        self.skipped
            .fetch_add(until.saturating_sub(now), Ordering::Relaxed);
    }
}

/// What a lock holds, also after a thread panicked holding it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a lock holds, reached through its only owner.
pub(crate) fn held<T>(mutex: &mut Mutex<T>) -> &mut T {
    mutex.get_mut().unwrap_or_else(PoisonError::into_inner)
}

impl wcontainers_system::TaskIdAllocator for TestHost {
    fn free_task_id(&self, _: TaskId) {}
}

impl System for TestHost {}

impl wcontainers_system::TaskIdAllocator for TestHost<Box<dyn Hooks>> {
    fn allocate_task_id(&self, previous: TaskId) -> TaskId {
        self.hook().allocate_task_id(previous)
    }
    fn free_task_id(&self, id: TaskId) {
        self.hook().free_task_id(id);
    }
}

impl System for TestHost<Box<dyn Hooks>> {
    fn intercept_system_call_request(
        &self,
        process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        self.hook()
            .intercept_system_call_request(process, thread, request)
    }
    fn intercept_system_call_response(
        &self,
        process: Process<'_>,
        thread: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse {
        self.hook()
            .intercept_system_call_response(process, thread, request, response)
    }
    fn on_process_starting(&self, process: Process<'_>) {
        self.hook().on_process_starting(process);
    }
    fn on_program_starting(&self, process: Process<'_>) {
        self.hook().on_program_starting(process);
    }
    fn on_thread_starting(&self, process: Process<'_>, thread: TaskId) {
        self.hook().on_thread_starting(process, thread);
    }
    fn on_process_exiting(&self, process: Process<'_>) {
        self.hook().on_process_exiting(process);
    }
    fn on_process_outgrown(&self, process: Process<'_>, bytes: u64) {
        self.hook().on_process_outgrown(process, bytes);
    }
    fn on_watched_memory_accessing(
        &self,
        process: Process<'_>,
        thread: TaskId,
        address: u64,
        kind: AccessKind,
    ) {
        self.hook()
            .on_watched_memory_accessing(process, thread, address, kind);
    }
    fn on_memory_written_by_kernel(&self, process: Process<'_>, thread: TaskId, range: Range<u64>) {
        self.hook()
            .on_memory_written_by_kernel(process, thread, range);
    }
    fn on_signal_delivering(
        &self,
        process: Process<'_>,
        thread: TaskId,
        signal: &SignalInformation,
    ) {
        self.hook().on_signal_delivering(process, thread, signal);
    }
    fn on_shared_memory_mapping(
        &self,
        process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        self.hook()
            .on_shared_memory_mapping(process, source, offset, range);
    }
    fn on_shared_memory_unmapped(
        &self,
        process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        self.hook()
            .on_shared_memory_unmapped(process, source, offset, range);
    }
}

impl<H> TestHost<H> {
    /// A host that keeps a record of what it gives the kernel, or answers from one.
    pub fn taped(mut self, tape: Tape) -> Self {
        self.tape = Some(Mutex::new(tape));
        self
    }

    /// Whether this host keeps a record or answers from one.
    pub fn taped_run(&self) -> bool {
        self.tape.is_some()
    }

    /// Whether this host answers from a record.
    pub fn replaying(&self) -> bool {
        self.tape
            .as_ref()
            .is_some_and(|tape| lock(tape).is_replaying())
    }

    /// The next answer of a replay, taken by `take`; none when not replaying.
    fn replayed<T>(&self, take: impl FnOnce(Entry) -> Result<T, Entry>) -> Option<T> {
        let tape = self.tape.as_ref()?;
        let mut tape = lock(tape);
        if !tape.is_replaying() {
            return None;
        }
        let entry = tape.take();
        Some(take(entry).unwrap_or_else(|entry| tape.out_of_step(&entry)))
    }

    /// Keeps what the host gave the kernel, unless the driver's own work asked.
    fn keep(&self, entry: impl FnOnce() -> Entry) {
        if let Some(tape) = &self.tape {
            let mut tape = lock(tape);
            if !tape.outside {
                tape.keep(entry());
            }
        }
    }

    /// An answer: from the record when replaying, else as `live` gives it, kept.
    fn answer<T: Clone>(
        &self,
        kept: impl FnOnce(T) -> Entry,
        take: impl FnOnce(Entry) -> Result<T, Entry>,
        live: impl FnOnce() -> T,
    ) -> T {
        if let Some(value) = self.replayed(take) {
            return value;
        }
        let value = live();
        self.keep(|| kept(value.clone()));
        value
    }

    /// Marks the driver's own work as running or not; what it was.
    fn set_outside(&self, outside: bool) -> bool {
        self.tape.as_ref().is_some_and(|tape| {
            let mut tape = lock(tape);
            std::mem::replace(&mut tape.outside, outside)
        })
    }

    /// Whether the backend owes an answer.
    pub(crate) fn backend_waiting(&self) -> bool {
        if let Some(waiting) = self.replayed(|entry| match entry {
            Entry::BackendWaiting(waiting) => Ok(waiting),
            other => Err(other),
        }) {
            return waiting;
        }
        let waiting = self
            .backend()
            .as_ref()
            .is_some_and(|backend| backend.waiting());
        self.keep(|| Entry::BackendWaiting(waiting));
        waiting
    }

    /// What the record holds for a run that kept one, which then has no more to keep.
    pub fn take_tape(&self) -> Option<Tape> {
        self.tape
            .as_ref()
            .map(|tape| std::mem::take(&mut *lock(tape)))
    }
}

impl<H: Send> TestHost<H>
where
    Self: System,
{
    /// Does what the outside does at one point of the driver loop: in a replay, hands the kernel
    /// what the record holds up to that point's end; else does it with `live`, keeping its end.
    fn outside_part<T: Copy>(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut dyn Cpu,
        ending: fn(T) -> Entry,
        ended: fn(&Entry) -> Option<T>,
        live: impl FnOnce(&Arc<Self>, &mut Kernel, &mut dyn Cpu) -> T,
    ) -> T {
        if self.replaying() {
            let tape = self.tape.as_ref().expect("a record");
            loop {
                let entry = lock(tape).take();
                match entry {
                    Entry::Event(event) => kernel.event(cpu, self, event.event()),
                    Entry::Kill(signal) => kernel.kill(cpu, self, signal),
                    Entry::BackendAnswered(unique, reply) => self.reply_arrived(unique, reply),
                    Entry::NameAnswered(number, answer) => self.name_arrived(number, answer),
                    Entry::SocketReady(number, ready) => self.socket_ready(number, ready),
                    Entry::StreamReady(end, ready) => self.stream_readied(end, ready),
                    other => match ended(&other) {
                        Some(value) => return value,
                        None if other == Entry::Delivered => {}
                        None => lock(tape).out_of_step(&other),
                    },
                }
            }
        }
        let was = self.set_outside(true);
        let value = live(self, kernel, cpu);
        self.set_outside(was);
        if let Some(tape) = &self.tape {
            lock(tape).keep(ending(value));
        }
        value
    }

    /// Hands the kernel an event from the outside, kept in the record; in a replay, one the
    /// driver's own work hands must be the one the record holds.
    pub(crate) fn hand(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut dyn Cpu, event: Event<'_>) {
        self.kept_in_step(Entry::Event(OwnedEvent::of(&event)));
        let was = self.set_outside(false);
        kernel.event(cpu, self, event);
        self.set_outside(was);
    }

    /// Sends a signal from the outside to the first process, kept in the record.
    fn kill(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut dyn Cpu, signal: Signal) {
        self.kept_in_step(Entry::Kill(signal));
        let was = self.set_outside(false);
        kernel.kill(cpu, self, signal);
        self.set_outside(was);
    }

    /// Keeps what the outside hands the kernel, or in a replay checks it against the record.
    fn kept_in_step(&self, entry: Entry) {
        let Some(tape) = &self.tape else {
            return;
        };
        let mut tape = lock(tape);
        if !tape.is_replaying() {
            tape.keep(entry);
            return;
        }
        let kept = tape.take();
        assert_eq!(
            kept,
            entry,
            "the replay went out of step at entry {}",
            tape.taken()
        );
    }
}

impl<H: Send> TestHost<H> {
    /// The monotonic clock as this host keeps it.
    fn monotonic_live(&self) -> u64 {
        match &self.virtual_now {
            Some(now) => {
                let work = self.kernel_work.swap(0, Ordering::Relaxed);
                now.fetch_add(work, Ordering::Relaxed) + self.skipped.load(Ordering::Relaxed)
            }
            None => {
                (self.origin.elapsed().as_nanos() as u64)
                    .wrapping_sub(self.started.load(Ordering::Relaxed))
                    + self.skipped.load(Ordering::Relaxed)
            }
        }
    }
}

impl<H: Send> wcontainers_clock::Clock for TestHost<H> {
    fn monotonic_nanoseconds(&self) -> u64 {
        self.answer(
            Entry::Monotonic,
            |entry| match entry {
                Entry::Monotonic(now) => Ok(now),
                other => Err(other),
            },
            || self.monotonic_live(),
        )
    }

    fn realtime_nanoseconds(&self) -> u64 {
        self.answer(
            Entry::Realtime,
            |entry| match entry {
                Entry::Realtime(now) => Ok(now),
                other => Err(other),
            },
            || match &self.virtual_now {
                Some(_) => self.wall_base.load(Ordering::Relaxed) + self.monotonic_live(),
                None => wall_clock() + self.skipped.load(Ordering::Relaxed),
            },
        )
    }
}

impl<H: Send> wcontainers_random_source::RandomSource for TestHost<H> {
    fn fill_random_bytes(&self, into: &mut [u8]) -> Result<(), HostError> {
        let bytes = self.answer(
            Entry::Random,
            |entry| match entry {
                Entry::Random(bytes) if bytes.len() == into.len() => Ok(bytes),
                other => Err(other),
            },
            || {
                let mut bytes = vec![0; into.len()];
                lock(&self.random).fill_bytes(&mut bytes);
                bytes
            },
        );
        into.copy_from_slice(&bytes);
        Ok(())
    }
}

impl<H: Send> Host for TestHost<H> {
    fn warn(&self, warning: Warning<'_>) {
        // Only a missing call's detail is a call number.
        let name = if warning.reason == wcontainers_kernel_linux::reason::NOT_IMPLEMENTED {
            wcontainers_kernel_linux::system_call_name(warning.detail).unwrap_or("")
        } else {
            ""
        };
        // Printed at counts 1, 2, 4, 8 and on, as a program can give millions.
        if warning.count.is_power_of_two() {
            eprintln!(
                "kernel warning: {} (detail {} {name}, count {})",
                warning.reason, warning.detail, warning.count
            );
        }
        lock(&self.warnings).push(Warned {
            reason: warning.reason.to_string(),
            count: warning.count,
            detail: warning.detail,
        });
    }
}

/// The standard streams, each send and receive kept in the record.
impl<H: Send> wcontainers_system::StreamTransport for TestHost<H> {
    type Endpoint = crate::streams::End;

    /// In a replay the client still shows what the record says was taken.
    fn send_stream(&self, peer: &crate::streams::End, bytes: &[u8]) -> Result<usize, HostError> {
        let result = self.answer(
            Entry::StreamSent,
            |entry| match entry {
                Entry::StreamSent(result) => Ok(result),
                other => Err(other),
            },
            || lock(&self.streams).send(*peer, bytes),
        );
        if let Ok(taken) = result
            && self.replaying()
        {
            lock(&self.streams).show(*peer, &bytes[..taken]);
        }
        result
    }

    fn receive_stream(
        &self,
        peer: &crate::streams::End,
        into: &mut [u8],
    ) -> Result<usize, HostError> {
        let (result, bytes) = self.answer(
            |(result, bytes)| Entry::StreamReceived(result, bytes),
            |entry| match entry {
                Entry::StreamReceived(result, bytes) if bytes.len() <= into.len() => {
                    Ok((result, bytes))
                }
                other => Err(other),
            },
            || {
                let mut bytes = vec![u8::MIN; into.len()];
                let result = lock(&self.streams).receive(*peer, &mut bytes);
                bytes.truncate(*result.as_ref().unwrap_or(&usize::MIN));
                (result, bytes)
            },
        );
        into[..bytes.len()].copy_from_slice(&bytes);
        result
    }

    fn close_stream(&self, peer: &crate::streams::End) {
        lock(&self.streams).close(*peer);
    }
}

impl<H: Send> wcontainers_readiness::Readiness<crate::streams::End> for TestHost<H> {
    async fn wait_until_ready(
        &self,
        peer: &crate::streams::End,
        interest: wcontainers_readiness::ReadyFor,
    ) -> Result<wcontainers_readiness::ReadinessReport, HostError> {
        if !self.replaying() {
            lock(&self.streams).arm(*peer, interest);
        }
        Ok(Readied::wait(&self.stream_ready, peer.0).await)
    }
}

/// The backend's messages, each sent (kept in the record) and waited on until its reply arrives:
/// a message FUSE gives no reply, or one this host answers itself, finishes once sent.
impl<H: Send> wcontainers_system::FuseServer for TestHost<H> {
    type Reply = Vec<u8>;
    type Notice = Vec<u8>;

    /// Not built: this system hands its notices to the kernel as the abi's
    /// `Event::BackendNotice`, until the kernel keeps a wait for them.
    async fn receive_notice(
        &self,
        applied: Option<Result<(), Errno>>,
    ) -> Result<Vec<u8>, HostError> {
        let _ = applied;
        Err(HostError::NotSupported)
    }

    async fn send_request(&self, request: &[u8]) -> Result<Vec<u8>, HostError> {
        use fuse_abi::linux::{fuse_in_header, fuse_opcode};
        use zerocopy::FromBytes;
        let (header, _) =
            fuse_in_header::read_from_prefix(request).expect("a request starts with its header");
        let unique = header.unique;
        // The answer to a RETRIEVE this host sent in the server's place stays here.
        let retrieved = unique & RETRIEVED == RETRIEVED;
        self.answer(
            Entry::BackendSent,
            |entry| match entry {
                Entry::BackendSent(result) => Ok(result),
                other => Err(other),
            },
            || {
                let mut backend = lock(&self.backend);
                let backend = backend.as_mut().ok_or(HostError::NotSupported)?;
                if !retrieved {
                    backend.send(unique, request);
                }
                Ok(())
            },
        )?;
        let unanswered = [
            fuse_opcode::FUSE_FORGET,
            fuse_opcode::FUSE_BATCH_FORGET,
            fuse_opcode::FUSE_NOTIFY_REPLY,
        ]
        .iter()
        .any(|opcode| *opcode as u32 == header.opcode);
        if retrieved || unanswered {
            return Ok(Vec::new());
        }
        Ok(Replies::wait(&self.replies, unique).await)
    }
}

/// Each name question is looked up by this machine's resolver (kept in the record) and waited on
/// until its answer arrives, as the network's answers do.
impl<H: Send> wcontainers_system::DomainNameResolver for TestHost<H> {
    type AnswerBytes = Vec<u8>;

    async fn resolve_domain(
        &self,
        question: domain::base::Question<&domain::base::Name<[u8]>>,
    ) -> Result<wcontainers_runtime_single_kernel::names::NameAnswer, HostError> {
        let number = self.last_name.fetch_add(1, Ordering::Relaxed) + 1;
        if !self.replaying() {
            lock(&self.network).resolve(number, question);
        }
        wcontainers_runtime_single_kernel::names::to_name_answer(
            Replies::wait(&self.names, number).await,
        )
    }
}

impl Replies {
    /// The reply to `key` once it has arrived; one let go before is forgotten.
    async fn wait(replies: &Mutex<Replies>, key: u64) -> Vec<u8> {
        /// Forgets a reply waited for, when the wait is let go.
        struct Forget<'a>(&'a Mutex<Replies>, u64);
        impl Drop for Forget<'_> {
            fn drop(&mut self) {
                let mut replies = lock(self.0);
                replies.arrived.remove(&self.1);
                replies.waiting.remove(&self.1);
            }
        }
        let _forget = Forget(replies, key);
        std::future::poll_fn(|context| {
            let mut replies = lock(replies);
            match replies.arrived.remove(&key) {
                Some(reply) => Poll::Ready(reply),
                None => {
                    replies.waiting.insert(key, context.waker().clone());
                    Poll::Pending
                }
            }
        })
        .await
    }

    /// The reply to `key` arrived: the call waiting for it is woken.
    fn arrived(replies: &Mutex<Replies>, key: u64, reply: Vec<u8>) {
        let waiting = {
            let mut replies = lock(replies);
            replies.arrived.insert(key, reply);
            replies.waiting.remove(&key)
        };
        if let Some(waker) = waiting {
            waker.wake();
        }
    }
}

impl<H> TestHost<H> {
    /// The backend's reply to request `unique` arrived: the call waiting for it is woken.
    fn reply_arrived(&self, unique: u64, reply: Vec<u8>) {
        Replies::arrived(&self.replies, unique, reply);
    }

    /// The resolver's answer to question `number` arrived: the call waiting for it is woken.
    fn name_arrived(&self, number: u64, answer: Vec<u8>) {
        Replies::arrived(&self.names, number, answer);
    }

    /// Socket `number` became ready: the wait on it is woken.
    /// Ends the kernel's wait on stream endpoint `end` with what it is ready for.
    pub(crate) fn stream_readied(
        &self,
        end: crate::streams::End,
        ready: wcontainers_readiness::ReadinessReport,
    ) {
        Readied::arrive(&self.stream_ready, end.0, ready);
    }

    pub(crate) fn socket_ready(&self, number: u32, ready: wcontainers_readiness::ReadinessReport) {
        Readied::arrive(&self.ready, number, ready);
    }
}

impl<H: Send> StreamHost for TestHost<H>
where
    Self: System,
{
    fn streams(&self) -> MutexGuard<'_, Streams> {
        lock(&self.streams)
    }

    fn choices(&self) -> MutexGuard<'_, Schedule> {
        lock(&self.schedule)
    }

    fn change_served(&self, path: &str, bytes: &[u8]) {
        self.backend()
            .as_ref()
            .expect("a container with a backend")
            .change(path, bytes);
    }

    fn hand(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut dyn Cpu, event: Event<'_>) {
        TestHost::hand(self, kernel, cpu, event);
    }

    /// Kept in the record, then the kernel settles, as for a socket.
    fn ready(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut dyn Cpu,
        end: crate::streams::End,
        ready: wcontainers_readiness::ReadinessReport,
    ) {
        if self.taped_run() {
            self.kept_in_step(Entry::StreamReady(end, ready));
        }
        self.stream_readied(end, ready);
        TestHost::hand(self, kernel, cpu, Event::Timer);
    }
}

/// The exit code `docker run` gives for a wait status.
pub fn exit_code(wait_status: i32) -> i32 {
    match wcontainers_kernel_linux::wait::decode(wait_status) {
        wcontainers_kernel_linux::wait::Ended::Exited(code) => code,
        wcontainers_kernel_linux::wait::Ended::Killed { signal, .. } => {
            wcontainers_abi::SIGNALLED_EXIT_BASE + signal
        }
    }
}

/// How many turns on the CPU side this process's runs have ended, backend answers they have
/// taken and steps of deadlines they have waited for, for a watchdog that tells a run that is
/// slow from one stuck inside a single call or turn.
pub static PROGRESS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// What a container starts with: its files and its first process.
pub struct Container<'a> {
    pub files: &'a [image::File],
    pub path: &'a str,
    pub arguments: &'a [&'a str],
    pub environment: &'a [&'a str],
    pub working_directory: &'a str,
    /// Where the file backend is mounted: `/` makes it the root, with the files unpacked into
    /// it; any other path is a volume, empty at the start. None keeps the root in memory.
    pub backend: Option<&'a str>,
    /// With the root on the backend, an image holding the root's files where the backend serves
    /// them, served in place of `files`; the files a run leaves are then not read.
    pub root_image: Option<&'a str>,
    /// Whether the backend opens files for the page cache rather than for direct reads and writes.
    pub cached: bool,
    /// Ports of this machine's loopback that reach the container's ports, as (container port,
    /// outside port), as `docker run -p` publishes them.
    pub published: &'a [(u16, u16)],
    /// Where each process's address space ends: the host's memories are 32 or 64 bit
    /// ([`crate::standin::SPACE_32_BIT`], [`crate::standin::SPACE_64_BIT`]).
    pub address_space: u64,
}

/// As [`run_injected`], with the client of the container's standard streams.
pub fn run_with_client(
    container: &Container<'_>,
    client: StreamClient,
    seed: u64,
    injections: Injections,
) -> Result<Run, Failure> {
    let prepared = Prepared {
        client,
        ..prepare(container)
    };
    run_prepared(prepared, container, seed, injections)
}

/// Runs a container on our kernel and the stand-in CPU until its first process ends.
pub fn run(container: &Container<'_>, seed: u64) -> Result<Run, Failure> {
    run_injected(container, seed, Injections::default())
}

/// What a run has done to it at moments drawn from its seed, besides where its turns end.
#[derive(Clone, Copy, Debug)]
pub struct Injections {
    /// How many times it is saved and brought back.
    pub saves: usize,
    /// The most turns between two saves (and before the first): a long run spreads its saves
    /// over more turns.
    pub save_gap: u64,
    /// How much more the host's wall clock moves on than its monotonic clock at each save.
    pub wall_step: u64,
    /// A signal sent to the first process from outside, and how many times.
    pub signal: Option<(Signal, usize)>,
    /// How many notices the file backend sends unasked, of kinds and about nodes drawn from the
    /// seed, as a backend whose files changed underneath would.
    pub notices: usize,
    /// The wall clock's reading at the start, given again to repeat a run whose guest reads it; the
    /// machine's clock when none.
    pub wall_start: Option<u64>,
}

impl Default for Injections {
    fn default() -> Self {
        Self {
            saves: 0,
            save_gap: MOMENTS_APART,
            wall_step: 0,
            signal: None,
            notices: 0,
            wall_start: None,
        }
    }
}

/// The most turns between two injected moments, unless a run asks for another spacing.
const MOMENTS_APART: u64 = 100;

/// Turn counts at which something is injected: `count` of them, each 1 to [`MOMENTS_APART`] turns
/// after the last.
fn moments(seed: u64, count: usize) -> Vec<u64> {
    moments_apart(seed, count, MOMENTS_APART)
}

/// The turn counts at which a run of `seed` with `injections` is saved, as far as it lasts.
pub fn save_moments(seed: u64, injections: Injections) -> Vec<u64> {
    moments_apart(seed ^ SAVES_SALT, injections.saves, injections.save_gap)
}

/// Turn counts at which something is injected: `count` of them, each 1 to `apart` turns after the
/// last.
fn moments_apart(seed: u64, count: usize, apart: u64) -> Vec<u64> {
    let mut moments = StdRng::seed_from_u64(seed);
    let mut last = 0;
    (0..count)
        .map(|_| {
            last += moments.random_range(1..apart);
            last
        })
        .collect()
}

/// As [`run`], with what `injections` says: saving the container up to `saves` times at moments
/// drawn from the seed and bringing it back each time from the copy alone, and sending signals. A save is a new
/// process holding a copy of this one's memory, as a host that saves copies the kernel's memory
/// and every guest memory: the copy goes on with a new CPU side made from the saved memories and
/// registers and a new host whose clock reads differently, while this process only waits for the
/// copy's run. Each time the host's wall clock has moved on `wall_step` nanoseconds more than its
/// monotonic clock, as on a host whose wall clock was set while the container was saved.
pub fn run_injected(
    container: &Container<'_>,
    seed: u64,
    injections: Injections,
) -> Result<Run, Failure> {
    run_prepared(prepare(container), container, seed, injections)
}

/// A kernel holding a container's files before its first process starts, and the groups that
/// process starts in.
pub struct Prepared {
    pub kernel: Kernel,
    pub groups: Vec<u32>,
    /// Whether the container's output also goes to this process's streams as it comes.
    pub echo: bool,
    /// The client of its standard streams.
    pub client: StreamClient,
    /// What the test does at the kernel's hooks; with none, the kernel's provided defaults.
    pub hooks: Option<Box<dyn Hooks>>,
    /// The executor's ceiling given at start, if below the address space's end.
    pub ceiling: Option<u64>,
    /// Whether the guest's clocks are the machine's and its waits take their time, so a test that
    /// runs for a span of time takes that span, rather than moving only with the work run.
    pub wall_clock: bool,
    /// A record to keep of what the host gives the kernel, or to answer from in its place.
    pub tape: Option<Tape>,
    /// What the container's configuration sets beyond its program.
    pub options: wcontainers_kernel_linux::StartOptions<'static>,
}

/// Puts a container's files in a new kernel's root, unless the root is on the backend.
pub fn prepare(container: &Container<'_>) -> Prepared {
    let mut kernel = Kernel::new();
    if container.backend != Some(reference::ROOT_DIRECTORY) {
        image::add_files(&mut kernel, container.files);
    }
    Prepared {
        kernel,
        groups: image::root_groups(container.files),
        echo: false,
        client: StreamClient::default(),
        hooks: None,
        ceiling: None,
        wall_clock: false,
        tape: None,
        options: Default::default(),
    }
}

/// Held alone by a run that saves and shared by every other run of this test process, while the run
/// has a back end: a save copies the process with fork, which carries only the saving thread, so no
/// other thread may be inside the back end then, holding locks of wasmtime or of the system's
/// unwinder that the copy needs to compile.
#[cfg(feature = "runtime")]
static SAVING: std::sync::RwLock<()> = std::sync::RwLock::new(());

/// As [`run_injected`], on a kernel [`prepare`] made for the container.
pub fn run_prepared(
    mut prepared: Prepared,
    container: &Container<'_>,
    seed: u64,
    injections: Injections,
) -> Result<Run, Failure> {
    let mut host = if prepared.wall_clock {
        TestHost {
            waits: true,
            ..TestHost::new(seed)
        }
    } else {
        TestHost::repeatable(seed, injections.wall_start)
    };
    if let Some(tape) = prepared.tape.take() {
        assert_eq!(
            injections.saves, 0,
            "a run that keeps a record is not saved"
        );
        host = host.taped(tape);
    }
    match prepared.hooks.take() {
        None => run_hosted(prepared, host, container, seed, injections),
        Some(hooks) => run_hosted(
            prepared,
            host.with_hooks(hooks),
            container,
            seed,
            injections,
        ),
    }
}

/// A kernel with the container started on the stand-in, as the kernel interface sees it: its first
/// process, with `state`, is loaded and has not run.
pub fn started_with_executor<State: Clone + 'static>(
    prepared: Prepared,
    container: &Container<'_>,
    state: State,
) -> (
    wcontainers_kernel_linux::KernelWithExecutor<crate::standin::StandIn, State, TestHost>,
    Arc<TestHost>,
) {
    let host = TestHost::repeatable(0, None);
    *host.streams() = Streams::new(prepared.client);
    let arguments: Vec<&[u8]> = container
        .arguments
        .iter()
        .map(|argument| argument.as_bytes())
        .collect();
    let environment: Vec<&[u8]> = container
        .environment
        .iter()
        .map(|variable| variable.as_bytes())
        .collect();
    let cpu = crate::standin::StandIn::with_space(container.address_space);
    let start = Start {
        path: container.path.as_bytes(),
        arguments: &arguments,
        environment: &environment,
        working_directory: container.working_directory.as_bytes(),
        hostname: reference::HOSTNAME.as_bytes(),
        groups: &prepared.groups,
        backend: None,
        standard: host.standard(),
        address_space_bytes: wcontainers_kernel_linux::ARM64_ADDRESS_SPACE_BYTES,
        memory_bytes: cpu.address_space().end,
        ceiling: prepared.ceiling,
        options: prepared.options,
    };
    let config = wcontainers_kernel::ExecutionConfig {
        thread_ordering: wcontainers_kernel::ThreadOrdering::Serial,
        replay_determinism: wcontainers_kernel::ReplayDeterminism::Exact,
        translator: (),
    };
    let host = Arc::new(host);
    // SAFETY: the test host's waiting calls hold only what may move between host threads (see
    // `waiting_calls_move_between_threads`).
    let kernel = unsafe {
        wcontainers_kernel_linux::KernelWithExecutor::start(
            prepared.kernel,
            cpu,
            &host,
            start,
            config,
            state,
        )
    }
    .expect("the kernel runs a process serially with exact replay");
    (kernel, host)
}

/// Fails to build unless every waiting call of the test host may move between host threads, as
/// the kernel's start asks of a system.
fn waiting_calls_move_between_threads(
    host: &TestHost,
    end: &crate::streams::End,
    tcp: &tcp::TcpHandle,
    udp: &udp::UdpHandle,
    question: domain::base::Question<&domain::base::Name<[u8]>>,
) {
    use wcontainers_readiness::{Readiness, ReadyFor};
    use wcontainers_system::{DomainNameResolver, FuseServer};
    fn movable(_: &impl Send) {}
    movable(&Readiness::<crate::streams::End>::wait_until_ready(
        host,
        end,
        ReadyFor::READABLE,
    ));
    movable(&Readiness::<tcp::TcpHandle>::wait_until_ready(
        host,
        tcp,
        ReadyFor::READABLE,
    ));
    movable(&Readiness::<udp::UdpHandle>::wait_until_ready(
        host,
        udp,
        ReadyFor::READABLE,
    ));
    movable(&host.send_request(&[]));
    movable(&host.receive_notice(None));
    movable(&host.resolve_domain(question));
}

/// The test host's waiting calls, as [`waiting_calls_move_between_threads`] takes them.
type WaitingCallsOf = fn(
    &TestHost,
    &crate::streams::End,
    &tcp::TcpHandle,
    &udp::UdpHandle,
    domain::base::Question<&domain::base::Name<[u8]>>,
);

const _: WaitingCallsOf = waiting_calls_move_between_threads;

/// The id of a container's first process, as the kernel numbers it inside the container.
pub const FIRST_PROCESS: wcontainers_kernel::TaskId = wcontainers_kernel::TaskId(1);

/// A kernel on the stand-in with a container started with the guest program `name`, built with
/// `link_flags` into `out`, whose first process has not run and holds a number as the embedder's
/// state; the kernel interface's suite takes it.
pub fn program_not_run(
    name: &str,
    link_flags: &[&str],
    out: &std::path::Path,
) -> wcontainers_kernel_linux::KernelWithExecutor<crate::standin::StandIn, u64, TestHost> {
    program_started(name, link_flags, out).0
}

/// As [`program_not_run`], run until every thread waits with no deadline.
pub fn program_waiting(
    name: &str,
    link_flags: &[&str],
    out: &std::path::Path,
) -> wcontainers_kernel_linux::KernelWithExecutor<crate::standin::StandIn, u64, TestHost> {
    let (mut started, host) = program_started(name, link_flags, out);
    started.with_parts(|kernel, cpu| {
        loop {
            match kernel.next() {
                Next::Run {
                    thread,
                    resume,
                    slice_nanoseconds,
                } => {
                    let exit = cpu
                        .run(thread, resume, slice_nanoseconds)
                        .expect("the stand-in runs the program");
                    kernel.exit(cpu, &host, exit);
                }
                Next::Idle {
                    until_nanoseconds: Some(0),
                } => kernel.event(cpu, &host, Event::Timer),
                Next::Idle {
                    until_nanoseconds: None,
                } => break,
                other => panic!("the program waits for ever, not {other:?}"),
            }
        }
    });
    started
}

/// A container started with the guest program `name`, built with `link_flags` into `out`, and
/// its system.
fn program_started(
    name: &str,
    link_flags: &[&str],
    out: &std::path::Path,
) -> (
    wcontainers_kernel_linux::KernelWithExecutor<crate::standin::StandIn, u64, TestHost>,
    Arc<TestHost>,
) {
    let program = crate::guest::build(name, link_flags, out);
    let mut files = image::reference_image().to_vec();
    files.extend(image::host_directory(
        program.parent().expect("a directory"),
        crate::guest_directory_entry(),
    ));
    let path = reference::in_guest_directory(name);
    let arguments = [path.as_str()];
    let container = Container {
        files: &files,
        path: &path,
        arguments: &arguments,
        environment: reference::ENVIRONMENT,
        working_directory: reference::ROOT_DIRECTORY,
        backend: None,
        cached: false,
        published: &[],
        root_image: None,
        address_space: crate::standin::SPACE_32_BIT,
    };
    started_with_executor(prepare(&container), &container, u64::MIN)
}

fn run_hosted<H: BetweenSteps + Send>(
    prepared: Prepared,
    mut host: TestHost<H>,
    container: &Container<'_>,
    seed: u64,
    injections: Injections,
) -> Result<Run, Failure>
where
    TestHost<H>: System + wcontainers_system::StreamTransport<Endpoint = crate::streams::End>,
{
    let Prepared {
        mut kernel,
        groups,
        echo,
        client,
        ceiling,
        wall_clock,
        options,
        ..
    } = prepared;
    #[cfg(feature = "runtime")]
    let (_alone, _shared);
    #[cfg(feature = "runtime")]
    if injections.saves > 0 {
        _alone = SAVING
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    } else {
        _shared = SAVING
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }
    let mut cpu = back_end(seed, container.address_space, wall_clock);
    #[cfg(not(feature = "runtime"))]
    let taped = host.tape.is_some();
    #[cfg(not(feature = "runtime"))]
    if taped {
        crate::replay::DigestMemories::keep_digests(&mut cpu);
    }
    *host
        .injecting
        .get_mut()
        .unwrap_or_else(PoisonError::into_inner) = Some(Injecting::new(seed, injections));
    *host.streams() = Streams::new(client);
    host.streams().echo = echo;
    held(&mut host.network).published = container.published.iter().copied().collect();
    let root_on_backend = container.backend == Some(reference::ROOT_DIRECTORY);
    // A replay has only its record of what the backend answered.
    if container.backend.is_some() && !host.replaying() {
        let served = match container.root_image {
            _ if !root_on_backend => crate::backend::Served::Empty,
            Some(image) => crate::backend::Served::Image(image),
            None => crate::backend::Served::Files(container.files),
        };
        *host.backend() = Some(crate::backend::Backend::start(
            &crate::backend::out(),
            served,
            container.cached,
        ));
    }
    let arguments: Vec<&[u8]> = container
        .arguments
        .iter()
        .map(|argument| argument.as_bytes())
        .collect();
    let environment: Vec<&[u8]> = container
        .environment
        .iter()
        .map(|variable| variable.as_bytes())
        .collect();
    let start = Start {
        path: container.path.as_bytes(),
        arguments: &arguments,
        environment: &environment,
        working_directory: container.working_directory.as_bytes(),
        hostname: reference::HOSTNAME.as_bytes(),
        groups: &groups,
        backend: container.backend.map(str::as_bytes),
        standard: host.standard(),
        address_space_bytes: wcontainers_kernel_linux::ARM64_ADDRESS_SPACE_BYTES,
        memory_bytes: cpu.address_space().end,
        ceiling,
        options,
    };
    #[cfg(feature = "runtime")]
    let started = Instant::now();
    let host = Arc::new(host);
    kernel.start(&mut cpu, &host, start);
    wcontainers_runtime_single_kernel::System::started(&host, &mut kernel, &mut cpu);
    let ending = wcontainers_runtime_single_kernel::drive(&mut kernel, &mut cpu, &host);
    let status = match ending {
        Ok(Ending::Finished { status }) => status,
        Ok(Ending::Ended(run)) => return Ok(run),
        // A copy cannot hand a failure back; it says what it was and ends.
        Err(failure) if host.injecting().report.is_some() => {
            panic!("the CPU side failed: {failure:?}")
        }
        Err(failure) => return Err(failure.into()),
    };
    #[cfg(feature = "runtime")]
    report(&cpu, container, started.elapsed());
    let input_closed = host.streams().input_closed_by_kernel();
    let outputs_closed = host.outside_part(
        &mut kernel,
        &mut cpu,
        Entry::Finished,
        |entry| match entry {
            Entry::Finished(closed) => Some(*closed),
            _ => None,
        },
        |host, kernel, cpu| streams::finish(kernel, cpu, host),
    );
    if let Some(error) = kernel.start_failure() {
        // Never in a copy: no save is made before the first program runs.
        return Err(Failure::Start(error));
    }
    let files = match &*host.backend() {
        Some(_) if container.root_image.is_some() => Vec::new(),
        Some(backend) if root_on_backend => image::records(&backend.files()),
        _ => kernel.root_files(&mut cpu),
    };
    #[cfg(not(feature = "runtime"))]
    let memories = if taped {
        crate::replay::DigestMemories::digests(&mut cpu)
    } else {
        Vec::new()
    };
    #[cfg(feature = "runtime")]
    let memories = Vec::new();
    let tape = host.take_tape();
    if let Some(tape) = &tape {
        assert!(
            tape.used_up(),
            "the replay left {} entries of its record",
            tape.len()
        );
    }
    let (report, saved, turns) = {
        let injecting = host.injecting();
        (injecting.report, injecting.saved, injecting.turns)
    };
    let streams = std::mem::take(&mut *host.streams());
    let run = Run {
        outcome: Outcome {
            stdout: streams.stdout,
            stderr: streams.stderr,
            exit_code: exit_code(status),
        },
        warnings: std::mem::take(&mut *lock(&host.warnings)),
        cpu_warnings: std::mem::take(&mut *lock(&host.cpu_warnings)),
        counters: kernel.counters().collect(),
        files,
        saved,
        turns,
        most_input_asked: streams.most_input_asked,
        broken: streams.broken,
        outputs_closed,
        input_closed,
        tape,
        memories,
    };
    Ok(finish(run, report))
}

impl<H> TestHost<H> {
    /// The file backend, for a container with a volume.
    pub(crate) fn backend(&self) -> MutexGuard<'_, Option<crate::backend::Backend>> {
        lock(&self.backend)
    }

    fn injecting(&self) -> Held<'_, Injecting> {
        Held(lock(&self.injecting))
    }
}

/// What a lock holds that is there only for a driven run, held.
struct Held<'a, T>(MutexGuard<'a, Option<T>>);

impl<T> std::ops::Deref for Held<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0.as_ref().expect("a driven run")
    }
}

impl<T> std::ops::DerefMut for Held<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0.as_mut().expect("a driven run")
    }
}

impl<H: BetweenSteps + Send> TestHost<H>
where
    Self: System,
{
    /// How the next turn ends, drawn from the seed.
    fn turn_live(&self, slice_nanoseconds: Option<u64>) -> Turn {
        // The seed chooses where in its slice each turn ends, so a run repeats for a seed and
        // threads interleave differently under different seeds.
        let mut limit = lock(&self.schedule).turn_limit(slice_nanoseconds);
        // An answer owed arrives during the turn, at a moment drawn from the seed: the host raises
        // the running thread's stop flag then, as the boundary says.
        if self
            .backend()
            .as_ref()
            .is_some_and(|backend| backend.waiting())
        {
            let arrival = lock(&self.schedule)
                .draws()
                .random_range(1..=ANSWER_ARRIVES_WITHIN);
            limit = Some(limit.map_or(arrival, |l| l.min(arrival)));
        }
        Turn::Counted { limit }
    }

    fn arrived_live(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let turns = {
            let mut injecting = self.injecting();
            injecting.turns += 1;
            injecting.turns
        };
        PROGRESS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        network_events(kernel, cpu, self, Duration::ZERO);
        streams::act(kernel, cpu, self, false);
        // The backend's answers arrive at moments and in an order drawn from the seed.
        while self
            .backend()
            .as_ref()
            .is_some_and(|backend| backend.waiting())
            && lock(&self.schedule).draws().random_bool(ANSWER_NOW_CHANCE)
        {
            deliver(kernel, cpu, self);
        }
        while self.injecting().notice_at.first() == Some(&turns) {
            let notice = {
                let mut injecting = self.injecting();
                injecting.notice_at.remove(0);
                notice(&mut injecting.notices)
            };
            self.hand(kernel, cpu, Event::BackendNotice { notice: &notice });
        }
        while self.injecting().signal_at.first() == Some(&turns) {
            let signal = {
                let mut injecting = self.injecting();
                injecting.signal_at.remove(0);
                injecting.signal
            };
            TestHost::kill(self, kernel, cpu, signal);
        }
        self.injecting().at.first() == Some(&turns)
    }

    fn stopped_live<E: Execution>(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut E,
    ) -> ControlFlow<Run> {
        loop {
            {
                let mut injecting = self.injecting();
                if injecting.at.first() != Some(&injecting.turns) {
                    return ControlFlow::Continue(());
                }
                injecting.at.remove(0);
                injecting.saved += 1;
            }
            let mut ends = [0; 2];
            // SAFETY: plain system calls; the copy goes on in this thread alone and ends with
            // `_exit` once it has reported.
            unsafe {
                assert_eq!(libc::pipe(ends.as_mut_ptr()), 0, "a pipe for the copy");
                match libc::fork() {
                    copy if copy < 0 => panic!("fork failed"),
                    0 => {
                        libc::close(ends[0]);
                        std::panic::set_hook(Box::new(|info| panic_in_copy(&info.to_string())));
                        if let Some(earlier) = self.injecting().report.replace(ends[1]) {
                            libc::close(earlier);
                        }
                    }
                    copy => {
                        libc::close(ends[1]);
                        // The copy alone holds the network from here on.
                        drop(std::mem::take(&mut *lock(&self.network)));
                        let report = self.injecting().report;
                        return ControlFlow::Break(finish(copied_run(copy, ends[0]), report));
                    }
                }
            }
            *cpu = cpu.restored();
            let (seed, wall_step) = {
                let injecting = self.injecting();
                (
                    injecting.seed ^ ((injecting.saved as u64) << u32::BITS),
                    injecting.injections.wall_step,
                )
            };
            self.move_on(seed, wall_step);
            kernel.restore(cpu, self);
        }
    }

    fn idle_live(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut dyn Cpu,
        until_nanoseconds: Option<u64>,
    ) {
        if lock(&self.hooks).between_steps(kernel, cpu) {
            return;
        }
        // An answer owed comes before any time passes.
        if self
            .backend()
            .as_ref()
            .is_some_and(|backend| backend.waiting())
        {
            deliver(kernel, cpu, self);
            return;
        }
        // The outside answers in real time: a wait for it goes on as long as the kernel's next
        // timer, and at most ten seconds.
        if lock(&self.network).waiting() {
            let now = self.monotonic_nanoseconds();
            let wait = until_nanoseconds.map_or(NETWORK_PATIENCE, |until| {
                Duration::from_nanos(until.saturating_sub(now)).min(NETWORK_PATIENCE)
            });
            if network_events(kernel, cpu, self, wait) {
                return;
            }
        }
        // Coming back at once is the kernel settling what it was woken for, not a container that
        // can go on no other way.
        let settling = until_nanoseconds == Some(0);
        if streams::act(kernel, cpu, self, !settling) {
            return;
        }
        let Some(until) = until_nanoseconds else {
            let streams = self.streams();
            panic!(
                "the kernel is idle with nothing to wake it, and this host has no events \
                 (the client waits at {:?}); output so far:\n{}{}",
                streams.stuck_at(),
                String::from_utf8_lossy(&streams.stdout),
                String::from_utf8_lossy(&streams.stderr)
            );
        };
        let left = until.saturating_sub(self.monotonic_nanoseconds());
        if self.waits {
            // A wait for the kernel's own deadline is no hang: it counts as progress as it goes.
            let mut left = Duration::from_nanos(left);
            while !left.is_zero() {
                let step = left.min(WAIT_STEP);
                std::thread::sleep(step);
                left -= step;
                PROGRESS.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            self.skipped.fetch_add(left, Ordering::Relaxed);
        }
        self.hand(kernel, cpu, Event::Timer);
    }
}

impl<H: BetweenSteps + Send> TestHost<H>
where
    Self: System,
{
    /// What descriptors 0 to 2 of the first process are: the ends this host holds the other sides of.
    pub fn standard(&self) -> Standard<crate::streams::End> {
        lock(&self.streams).client.standard()
    }
}

/// What the loop asks of the test host between turns.
impl<H: BetweenSteps + Send + 'static> wcontainers_runtime_single_kernel::System for TestHost<H>
where
    Self: System + wcontainers_system::StreamTransport<Endpoint = crate::streams::End>,
{
    /// The run of a copy made at a save, as the process it was copied from gets it.
    type End = Run;

    /// What the test's client is; a test that asks for a terminal says so there too.
    fn standard(&self, terminal: Option<TerminalSize>) -> Standard<crate::streams::End> {
        assert_eq!(
            terminal.map(|size| (size.columns, size.rows)),
            lock(&self.streams).client.terminal,
            "the terminal asked for is the client's"
        );
        TestHost::standard(self)
    }

    fn started(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut dyn Cpu) {
        self.outside_part(
            kernel,
            cpu,
            |()| Entry::Begun,
            |entry| (*entry == Entry::Begun).then_some(()),
            |host, kernel, cpu| streams::begin(kernel, cpu, host),
        );
    }

    fn turn(&self, slice_nanoseconds: Option<u64>) -> Turn {
        if let Some(turn) = self.replayed(|entry| match entry {
            Entry::Turn(turn) => Ok(turn),
            other => Err(other),
        }) {
            return turn;
        }
        let turn = self.turn_live(slice_nanoseconds);
        self.keep(|| Entry::Turn(turn));
        turn
    }

    /// Advances a repeatable clock by the time one turn on the CPU side took: between 0.5 and 1.5
    /// nanoseconds for each unit of progress (an instruction on the stand-in; a CPU's speed varies
    /// with its caches and its load), and between 0.5 and 1.5 microseconds more for the kernel's
    /// work on what came back, both drawn from the seed. The kernel's part passes after the first
    /// reading of the clock, which the kernel takes as it is entered. The speed moves the points
    /// where the scheduler switches threads, so seeds give different interleavings.
    fn ran(&self, progress: u64) {
        if self.replaying() {
            return;
        }
        let (picoseconds, kernel) = {
            let mut schedule = lock(&self.schedule);
            let picoseconds = schedule.draws().random_range(PROGRESS_PICOSECONDS);
            (picoseconds, schedule.draws().random_range(TURN_NANOSECONDS))
        };
        if let Some(now) = &self.virtual_now {
            now.fetch_add(
                progress.saturating_mul(picoseconds) / PICOSECONDS_PER_NANOSECOND,
                Ordering::Relaxed,
            );
            self.kernel_work.store(kernel, Ordering::Relaxed);
        }
    }

    fn arrived(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        self.outside_part(
            kernel,
            cpu,
            Entry::Arrived,
            |entry| match entry {
                Entry::Arrived(asked) => Some(*asked),
                _ => None,
            },
            |host, kernel, cpu| host.arrived_live(kernel, cpu),
        )
    }

    /// Saves the container where the seed says: a new process holding a copy of this one's memory,
    /// as a host that saves copies the kernel's memory and every guest memory. The copy goes on with
    /// a new CPU side made from the saved memories and registers and a new host whose clock reads
    /// differently, while this process only waits for the copy's run.
    fn stopped<E: Execution>(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut E,
    ) -> ControlFlow<Run> {
        self.stopped_live(kernel, cpu)
    }

    fn idle(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut dyn Cpu,
        until_nanoseconds: Option<u64>,
    ) {
        self.outside_part(
            kernel,
            cpu,
            |()| Entry::Idled,
            |entry| (*entry == Entry::Idled).then_some(()),
            |host, kernel, cpu| host.idle_live(kernel, cpu, until_nanoseconds),
        );
    }

    fn cpu_warning(&self, warning: Warning<'_>) {
        // Printed at counts 1, 2, 4, 8 and on, as the kernel's are.
        if warning.count.is_power_of_two() {
            eprintln!(
                "cpu warning: {} (detail {:#x}, count {})",
                warning.reason, warning.detail, warning.count
            );
        }
        lock(&self.cpu_warnings).push(Warned {
            reason: warning.reason.to_string(),
            count: warning.count,
            detail: warning.detail,
        });
    }
}

/// Mixed into a run's seed for the moments of saves and signals, so that each draws apart from the
/// host's own randomness (the schedule's choices are the policy's, `SCHEDULE_STREAM`).
const SAVES_SALT: u64 = 0x5a7e;
const SIGNALS_SALT: u64 = 0x5197;
const NOTICES_SALT: u64 = 0x6e07;
const NOTICE_BYTES_SALT: u64 = 0x6e08;

/// Turns at least between the backend's notices, close together so that a short program meets
/// many of them.
const NOTICES_APART: u64 = 10;

/// The chance that one more owed answer arrives in the same turn.
const ANSWER_NOW_CHANCE: f64 = 0.5;

/// Picoseconds in a nanosecond, for one instruction's time on the CPU side (the policy's
/// `PROGRESS_PICOSECONDS`); the kernel's work on a turn's end takes the policy's `TURN_NANOSECONDS`.
const PICOSECONDS_PER_NANOSECOND: u64 = 1000;

/// How much of a waiting host's wait for a deadline passes between its marks of progress.
const WAIT_STEP: Duration = Duration::from_secs(1);

/// The longest the driver waits for the outside when nothing else can happen.
const NETWORK_PATIENCE: Duration = Duration::from_secs(10);

/// Hands the kernel what the outside sent, after waiting up to `wait` for something; whether
/// anything came.
fn network_events<H: Send>(
    kernel: &mut Kernel,
    cpu: &mut dyn wcontainers_abi::Cpu,
    host: &Arc<TestHost<H>>,
    wait: Duration,
) -> bool
where
    TestHost<H>: System,
{
    if !lock(&host.network).waiting() {
        return false;
    }
    let owed = lock(&host.network).take(wait);
    for owed in owed.iter() {
        match owed {
            // The call waiting for it takes it when the kernel next settles, as at once.
            crate::network::Owed::Answered(number, answer) => {
                if host.taped_run() {
                    host.kept_in_step(Entry::NameAnswered(*number, answer.clone()));
                }
                host.name_arrived(*number, answer.clone());
                host.hand(kernel, cpu, Event::Timer);
            }
            crate::network::Owed::Ready(number, ready) => {
                if host.taped_run() {
                    host.kept_in_step(Entry::SocketReady(*number, *ready));
                }
                host.socket_ready(*number, *ready);
                host.hand(kernel, cpu, Event::Timer);
            }
        }
    }
    !owed.is_empty()
}

/// The longest a turn runs while the backend owes an answer, in nanoseconds of the CPU side.
const ANSWER_ARRIVES_WITHIN: u64 = 50_000;

/// The request numbers of RETRIEVE notices this host sends: above any the kernel numbers.
const RETRIEVED: u64 = 1 << 62;

/// The kinds of notice a backend sends unasked, but RESEND, which only answers the kernel's requests.
const NOTICE_KINDS: &[fuse_abi::linux::fuse_notify_code] = {
    use fuse_abi::linux::fuse_notify_code::*;
    &[
        FUSE_NOTIFY_POLL,
        FUSE_NOTIFY_INVAL_INODE,
        FUSE_NOTIFY_INVAL_ENTRY,
        FUSE_NOTIFY_DELETE,
        FUSE_NOTIFY_RETRIEVE,
        FUSE_NOTIFY_STORE,
    ]
};

/// The nodes notices are about: the first few the backend numbers, some never looked up.
const NOTICE_NODES: std::ops::Range<u64> = 1..12;

/// The name INVAL_ENTRY and DELETE notices carry, and the bytes STORE notices write.
const NOTICE_NAME: &std::ffi::CStr = c"backend-file";

/// One notice in this many is cut short, which Linux refuses.
const CUT_SHORT_ONE_IN: u32 = 8;

/// A notice a backend sends unasked (`fuse_notify`), of a kind and about a node drawn from `random`,
/// sometimes cut short. The kernel's NOTIFY_REPLY to a RETRIEVE comes back to this host, which sent
/// it, not to the server ([`RETRIEVED`]).
fn notice(random: &mut StdRng) -> Vec<u8> {
    use fuse_abi::linux::*;
    use rand::seq::IndexedRandom;
    use zerocopy::IntoBytes;
    let node = random.random_range(NOTICE_NODES);
    let namelen = NOTICE_NAME.count_bytes() as u32;
    let code = *NOTICE_KINDS.choose(random).expect("a kind of notice");
    let mut body = match code {
        fuse_notify_code::FUSE_NOTIFY_POLL => {
            fuse_notify_poll_wakeup_out { kh: node }.as_bytes().to_vec()
        }
        fuse_notify_code::FUSE_NOTIFY_INVAL_INODE => fuse_notify_inval_inode_out {
            ino: node,
            off: 0,
            len: -1,
        }
        .as_bytes()
        .to_vec(),
        fuse_notify_code::FUSE_NOTIFY_INVAL_ENTRY => {
            let mut body = fuse_notify_inval_entry_out {
                parent: node,
                namelen,
                flags: 0,
            }
            .as_bytes()
            .to_vec();
            body.extend_from_slice(NOTICE_NAME.to_bytes_with_nul());
            body
        }
        fuse_notify_code::FUSE_NOTIFY_DELETE => {
            let mut body = fuse_notify_delete_out {
                parent: FUSE_ROOT_ID,
                child: node,
                namelen,
                padding: 0,
            }
            .as_bytes()
            .to_vec();
            body.extend_from_slice(NOTICE_NAME.to_bytes_with_nul());
            body
        }
        fuse_notify_code::FUSE_NOTIFY_RETRIEVE => fuse_notify_retrieve_out {
            notify_unique: RETRIEVED | node,
            nodeid: node,
            offset: 0,
            size: PAGE_SIZE as u32,
            padding: 0,
        }
        .as_bytes()
        .to_vec(),
        fuse_notify_code::FUSE_NOTIFY_STORE => {
            let mut body = fuse_notify_store_out {
                nodeid: node,
                offset: 0,
                size: NOTICE_NAME.count_bytes() as u32,
                padding: 0,
            }
            .as_bytes()
            .to_vec();
            body.extend_from_slice(NOTICE_NAME.to_bytes());
            body
        }
        other => unreachable!("{other:?} is not one of NOTICE_KINDS"),
    };
    if random.random_ratio(1, CUT_SHORT_ONE_IN) {
        body.truncate(random.random_range(0..body.len()));
    }
    let header = fuse_out_header {
        len: (size_of::<fuse_out_header>() + body.len()) as u32,
        error: code as i32,
        unique: 0,
    };
    let mut notice = header.as_bytes().to_vec();
    notice.extend_from_slice(&body);
    notice
}

/// Hands the kernel one of the backend's answers or notices, in an order drawn from the seed,
/// waiting for it.
pub(crate) fn deliver<H: Send>(
    kernel: &mut Kernel,
    cpu: &mut dyn wcontainers_abi::Cpu,
    host: &Arc<TestHost<H>>,
) where
    TestHost<H>: System,
{
    host.outside_part(
        kernel,
        cpu,
        |()| Entry::Delivered,
        |entry| (*entry == Entry::Delivered).then_some(()),
        |host, kernel, cpu| deliver_live(kernel, cpu, host),
    );
}

fn deliver_live<H: Send>(
    kernel: &mut Kernel,
    cpu: &mut dyn wcontainers_abi::Cpu,
    host: &Arc<TestHost<H>>,
) where
    TestHost<H>: System,
{
    let message = {
        let mut backend = host.backend();
        let mut schedule = lock(&host.schedule);
        backend
            .as_mut()
            .and_then(|backend| backend.next(schedule.draws()))
    };
    let Some(message) = message else {
        return;
    };
    PROGRESS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    match message {
        // The call waiting for it takes it when the kernel next settles, as at once.
        crate::backend::Message::Answer(unique, reply) => {
            if host.taped_run() {
                host.kept_in_step(Entry::BackendAnswered(unique, reply.clone()));
            }
            host.reply_arrived(unique, reply);
            host.hand(kernel, cpu, Event::Timer);
        }
        crate::backend::Message::Notice(notice) => {
            host.hand(kernel, cpu, Event::BackendNotice { notice: &notice });
        }
    }
}

/// A run's end: returned, or in a copy made at a save reported to the process it was copied from,
/// which ends the copy.
fn finish(run: Run, report: Option<i32>) -> Run {
    if let Some(fd) = report {
        let bytes = encode(&run);
        // SAFETY: writes to the pipe this copy was given, then ends the copy without unwinding.
        unsafe {
            let mut written = 0;
            while written < bytes.len() {
                let count =
                    libc::write(fd, bytes[written..].as_ptr().cast(), bytes.len() - written);
                assert!(count > 0, "reporting a copy's run");
                written += count as usize;
            }
            libc::_exit(0);
        }
    }
    run
}

/// Ends a copy made at a save that panics, after saying why on the standard error, since a copy
/// must not unwind into the test harness it was copied with.
fn panic_in_copy(why: &str) -> ! {
    let message = format!("a copy made at a save failed: {why}\n");
    // SAFETY: a write to the standard error, then the copy ends.
    unsafe {
        libc::write(2, message.as_ptr().cast(), message.len());
        libc::_exit(1)
    }
}

/// The run of a copy made at a save, read from its pipe once it has ended.
fn copied_run(copy: libc::pid_t, fd: i32) -> Run {
    use std::io::Read;
    use std::os::fd::{FromRawFd, OwnedFd};
    let mut bytes = Vec::new();
    // SAFETY: the read end of the copy's pipe, which only this function uses and closes.
    std::fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) })
        .read_to_end(&mut bytes)
        .expect("the copy's pipe reads to its end");
    let mut status = 0;
    // SAFETY: reaps the copy, a child of this process.
    unsafe { libc::waitpid(copy, &mut status, 0) };
    assert!(
        !bytes.is_empty() && status == 0,
        "the copy made at a save ended without a run (wait status {status}); why is above"
    );
    decode(&bytes)
}

/// A run as bytes, for a copy to hand it to the process it was copied from.
fn encode(run: &Run) -> Vec<u8> {
    fn bytes(out: &mut Vec<u8>, value: &[u8]) {
        out.extend_from_slice(&(value.len() as u64).to_le_bytes());
        out.extend_from_slice(value);
    }
    fn number(out: &mut Vec<u8>, value: u64) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    let mut out = Vec::new();
    bytes(&mut out, &run.outcome.stdout);
    bytes(&mut out, &run.outcome.stderr);
    number(&mut out, run.outcome.exit_code as u64);
    number(&mut out, run.saved as u64);
    number(&mut out, run.turns);
    number(&mut out, run.most_input_asked);
    number(&mut out, run.outputs_closed.into());
    number(&mut out, run.input_closed.into());
    number(&mut out, run.broken.len() as u64);
    for broken in &run.broken {
        bytes(&mut out, broken.as_bytes());
    }
    for warnings in [&run.warnings, &run.cpu_warnings] {
        number(&mut out, warnings.len() as u64);
        for warned in warnings {
            bytes(&mut out, warned.reason.as_bytes());
            number(&mut out, warned.count);
            number(&mut out, warned.detail);
        }
    }
    number(&mut out, run.counters.len() as u64);
    for counter in &run.counters {
        bytes(&mut out, counter.reason.as_bytes());
        number(&mut out, counter.detail);
        number(&mut out, counter.count);
        number(&mut out, counter.nanoseconds);
    }
    number(&mut out, run.files.len() as u64);
    for file in &run.files {
        bytes(&mut out, &file.path);
        number(&mut out, file.mode.into());
        number(&mut out, file.uid.into());
        number(&mut out, file.gid.into());
        bytes(&mut out, &file.contents);
        number(&mut out, file.rdev);
    }
    out
}

/// Reads what [`encode`] wrote.
fn decode(input: &[u8]) -> Run {
    struct Reader<'a>(&'a [u8]);
    impl Reader<'_> {
        fn number(&mut self) -> u64 {
            let (value, rest) = self.0.split_first_chunk().expect("a whole number");
            self.0 = rest;
            u64::from_le_bytes(*value)
        }
        fn bytes(&mut self) -> Vec<u8> {
            let length = self.number() as usize;
            let (value, rest) = self.0.split_at(length);
            self.0 = rest;
            value.to_vec()
        }
        fn text(&mut self) -> String {
            String::from_utf8(self.bytes()).expect("a reason's name")
        }
    }
    let mut reader = Reader(input);
    let stdout = reader.bytes();
    let stderr = reader.bytes();
    let exit_code = reader.number() as i32;
    let saved = reader.number() as usize;
    let turns = reader.number();
    let most_input_asked = reader.number();
    let outputs_closed = reader.number() != 0;
    let input_closed = reader.number() != 0;
    let broken = (0..reader.number()).map(|_| reader.text()).collect();
    let mut warned = || -> Vec<Warned> {
        (0..reader.number())
            .map(|_| Warned {
                reason: reader.text(),
                count: reader.number(),
                detail: reader.number(),
            })
            .collect()
    };
    let warnings = warned();
    let cpu_warnings = warned();
    let counters = (0..reader.number())
        .map(|_| CounterReading {
            // The kernel's reasons are fixed names; the copy's are leaked to stay for the run.
            reason: reader.text().leak(),
            detail: reader.number(),
            count: reader.number(),
            nanoseconds: reader.number(),
        })
        .collect();
    let files = (0..reader.number())
        .map(|_| FileRecord {
            path: reader.bytes(),
            mode: reader.number() as u32,
            uid: reader.number() as u32,
            gid: reader.number() as u32,
            contents: reader.bytes(),
            rdev: reader.number(),
        })
        .collect();
    Run {
        outcome: Outcome {
            stdout,
            stderr,
            exit_code,
        },
        warnings,
        cpu_warnings,
        counters,
        files,
        saved,
        turns,
        most_input_asked,
        broken,
        outputs_closed,
        input_closed,
        // A copy made at a save keeps no record.
        tape: None,
        memories: Vec::new(),
    }
}
