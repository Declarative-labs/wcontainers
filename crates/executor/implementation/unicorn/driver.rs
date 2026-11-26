//! The driver loop and a host for tests: runs a program on the kernel with the stand-in CPU.

use std::collections::BTreeMap;
use std::ops::{ControlFlow, Range};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};
use std::task::{Poll, Waker};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use wcontainers_abi::{CounterReading, HostError, PAGE_SIZE, SystemCallRequest, Warning};
use wcontainers_clock::Clock as _;
use wcontainers_executor::Executor;
use wcontainers_kernel::errno::Errno;
use wcontainers_kernel::signal::Signal;
use wcontainers_kernel::{ProcessEnd, StopReason};
use wcontainers_kernel_linux::{
    AccessKind, FileRecord, Kernel, MemoryObject, Process, SignalInformation, Standard, Start,
    System, SystemCallResponse, TaskId,
};
use wcontainers_system_seeded_test_policy::{
    REPEATABLE_START_NANOSECONDS, Schedule, TURN_NANOSECONDS,
};

use crate::hooks::Hooks;
use crate::replay::{Entry, MemoryDigests, Tape};
use crate::standin::Unsupported;
use crate::streams::{StreamClient, Streams};
use crate::{image, reference, streams};
use wcontainers_volatile_bytes::VolatileBytes;

#[path = "tcp.rs"]
mod tcp;
#[path = "udp.rs"]
mod udp;
use tcp::Readied;

/// The number of bytes a system call's buffer holds.
fn length(bytes: &impl VolatileBytes) -> usize {
    usize::try_from(bytes.len()).expect("a buffer fits in memory")
}

/// A copy of a system call's buffer.
fn copied(bytes: &impl VolatileBytes) -> Vec<u8> {
    let mut copy = vec![u8::MIN; length(bytes)];
    bytes.read(&mut copy);
    copy
}

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
pub(crate) type Back = crate::standin::StandIn;

/// The kernel's executor, the stand-in, with the memories and registers of its guest threads.
pub type Machine = wcontainers_kernel_linux::GuestMachine<Back>;

/// A kernel with its executor, the stand-in, as the driver runs it.
type Driven = wcontainers_kernel_linux::KernelWithExecutor<Back, (), TestHost>;

fn back_end(address_space: u64, wall_clock: bool) -> Back {
    let stand_in = crate::standin::StandIn::with_space(address_space);
    if wall_clock {
        stand_in.on_wall_clock()
    } else {
        stand_in
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
    /// What the stand-in refused in a copy made at a save, which ends without dropping it, handed
    /// back so the process it was copied from fails the test as its own stand-in goes.
    pub refused: Vec<crate::standin::Refused>,
}

/// The host used in tests. Randomness comes from a seed so runs repeat exactly. When the kernel is idle
/// until a time, the host's clocks jump to it instead of waiting, unless the host waits. With
/// no hooks the kernel's own provided defaults answer every hook and task id call.
pub struct TestHost {
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
    /// The backend's notices, for the kernel's wait on them.
    notices: Mutex<Notices>,
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
    /// What the test does at the kernel's hooks; none for the kernel's defaults.
    hooks: Mutex<Option<Box<dyn Hooks>>>,
    /// The record of what this host gives the kernel, kept or answered from.
    tape: Option<Mutex<Tape>>,
}

/// The backend's replies that arrived for the kernel's calls and are not taken yet, and the waker of
/// each call that waits for its reply, by request number. A reply completes only once the kernel
/// has applied every notice sent before it arrived, as the FUSE server interface's rule says.
#[derive(Default)]
struct Replies {
    /// Each reply, with how many notices had been sent when it arrived.
    arrived: BTreeMap<u64, (u64, Vec<u8>)>,
    waiting: BTreeMap<u64, Waker>,
    /// How many of the backend's notices the kernel has applied.
    applied: u64,
}

/// The backend's notices sent and not yet taken by the kernel's wait, and that wait's waker.
#[derive(Default)]
struct Notices {
    queued: std::collections::VecDeque<Vec<u8>>,
    waker: Option<Waker>,
    /// How many were sent.
    sent: u64,
    /// Whether the kernel holds one it has not yet said it applied.
    held: bool,
}

/// What a driven run has done to it at moments drawn from its seed, and where it is.
struct Injecting {
    signal: Signal,
    signal_at: Vec<u64>,
    notice_at: Vec<u64>,
    notices: StdRng,
    turns: u64,
}

impl Injecting {
    fn new(seed: u64, injections: Injections) -> Self {
        let (signal, signals) = injections.signal.unwrap_or((Signal::SIGKILL, 0));
        Self {
            signal,
            signal_at: moments(seed ^ SIGNALS_SALT, signals),
            notice_at: moments_apart(seed ^ NOTICES_SALT, injections.notices, NOTICES_APART),
            notices: StdRng::seed_from_u64(seed ^ NOTICE_BYTES_SALT),
            turns: 0,
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
            notices: Mutex::default(),
            names: Mutex::default(),
            last_name: AtomicU64::default(),
            ready: Mutex::default(),
            stream_ready: Mutex::default(),
            last_socket: AtomicU32::default(),
            network: Default::default(),
            cpu_warnings: Mutex::default(),
            injecting: Mutex::default(),
            waits: false,
            hooks: Mutex::new(None),
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
    pub fn with_hooks(mut self, hooks: Box<dyn Hooks>) -> Self {
        *held(&mut self.hooks) = Some(hooks);
        self
    }
}

/// The machine's wall clock, in nanoseconds since 1970.
fn wall_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after the epoch")
        .as_nanos() as u64
}

impl TestHost {
    /// The test's hooks for a call the kernel makes; one made while another is running nested in
    /// it, which is reported rather than waited on.
    pub fn hook(&self) -> MutexGuard<'_, Option<Box<dyn Hooks>>> {
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
    fn allocate_task_id(&self, previous: TaskId) -> TaskId {
        match &mut *self.hook() {
            Some(hooks) => hooks.allocate_task_id(previous),
            None => wcontainers_kernel_linux::task_id_after(previous),
        }
    }
    fn free_task_id(&self, id: TaskId) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.free_task_id(id);
        }
    }
}

/// A hook's process, on the kernel this host is given, which runs on the stand-in.
fn on_stand_in<C: Executor + 'static>(process: Process<'_, C>) -> crate::hooks::Process<'_> {
    process
        .on_executor()
        .expect("this host's kernel runs on the stand-in")
}

impl System for TestHost {
    fn intercept_system_call_request<C: Executor + 'static>(
        &self,
        process: Process<'_, C>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        match &mut *self.hook() {
            Some(hooks) => {
                hooks.intercept_system_call_request(on_stand_in(process), thread, request)
            }
            None => ControlFlow::Continue(request),
        }
    }
    fn intercept_system_call_response<C: Executor + 'static>(
        &self,
        process: Process<'_, C>,
        thread: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse {
        match &mut *self.hook() {
            Some(hooks) => hooks.intercept_system_call_response(
                on_stand_in(process),
                thread,
                request,
                response,
            ),
            None => response,
        }
    }
    fn on_process_starting<C: Executor + 'static>(&self, process: Process<'_, C>) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_process_starting(on_stand_in(process));
        }
    }
    fn on_program_starting<C: Executor + 'static>(&self, process: Process<'_, C>) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_program_starting(on_stand_in(process));
        }
    }
    fn on_thread_starting<C: Executor + 'static>(&self, process: Process<'_, C>, thread: TaskId) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_thread_starting(on_stand_in(process), thread);
        }
    }
    fn on_process_exiting<C: Executor + 'static>(&self, process: Process<'_, C>) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_process_exiting(on_stand_in(process));
        }
    }
    fn on_process_outgrown<C: Executor + 'static>(&self, process: Process<'_, C>, bytes: u64) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_process_outgrown(on_stand_in(process), bytes);
        }
    }
    fn on_watched_memory_accessing<C: Executor + 'static>(
        &self,
        process: Process<'_, C>,
        thread: TaskId,
        address: u64,
        kind: AccessKind,
    ) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_watched_memory_accessing(on_stand_in(process), thread, address, kind);
        }
    }
    fn on_memory_written_by_kernel<C: Executor + 'static>(
        &self,
        process: Process<'_, C>,
        thread: TaskId,
        range: Range<u64>,
    ) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_memory_written_by_kernel(on_stand_in(process), thread, range);
        }
    }
    fn on_signal_delivering<C: Executor + 'static>(
        &self,
        process: Process<'_, C>,
        thread: TaskId,
        signal: &SignalInformation,
    ) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_signal_delivering(on_stand_in(process), thread, signal);
        }
    }
    fn on_shared_memory_mapping<C: Executor + 'static>(
        &self,
        process: Process<'_, C>,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_shared_memory_mapping(on_stand_in(process), source, offset, range);
        }
    }
    fn on_shared_memory_unmapped<C: Executor + 'static>(
        &self,
        process: Process<'_, C>,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_shared_memory_unmapped(on_stand_in(process), source, offset, range);
        }
    }
    fn on_inspection_starting<C: Executor + 'static>(&self, process: Process<'_, C>) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_inspection_starting(on_stand_in(process));
        }
    }
    fn on_inspection_ended<C: Executor + 'static>(&self, process: Process<'_, C>) {
        if let Some(hooks) = &mut *self.hook() {
            hooks.on_inspection_ended(on_stand_in(process));
        }
    }
}

impl TestHost {
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

    /// What the record holds for a run that kept one, which then has no more to keep.
    pub fn take_tape(&self) -> Option<Tape> {
        self.tape
            .as_ref()
            .map(|tape| std::mem::take(&mut *lock(tape)))
    }
}

impl TestHost {
    /// Does what the outside does at one point of the driver loop: in a replay, hands the kernel
    /// what the record holds up to that point's end; else does it with `live`, keeping its end.
    fn outside_part<T: Copy>(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut Machine,
        ending: fn(T) -> Entry,
        ended: fn(&Entry) -> Option<T>,
        live: impl FnOnce(&Arc<Self>, &mut Kernel, &mut Machine) -> T,
    ) -> T {
        if self.replaying() {
            let tape = self.tape.as_ref().expect("a record");
            loop {
                let entry = lock(tape).take();
                match entry {
                    Entry::Resized(columns, rows) => {
                        kernel.resize_terminal(cpu, self, columns, rows);
                    }
                    Entry::Kill(signal) => kernel.kill(cpu, self, signal),
                    Entry::BackendAnswered(unique, reply) => self.reply_arrived(unique, reply),
                    Entry::NoticeSent(notice) => self.notice_sent(notice),
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

    /// Resizes the first process's terminal from the outside, kept in the record; in a replay,
    /// one the driver's own work hands must be the one the record holds.
    pub(crate) fn resize(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut Machine,
        columns: u16,
        rows: u16,
    ) {
        self.kept_in_step(Entry::Resized(columns, rows));
        let was = self.set_outside(false);
        kernel.resize_terminal(cpu, self, columns, rows);
        self.set_outside(was);
    }

    /// The backend sends a notice, kept in the record; the kernel's wait for one is woken, and the
    /// kernel settles as its runner next runs.
    fn send_notice(&self, notice: Vec<u8>) {
        if self.taped_run() {
            self.kept_in_step(Entry::NoticeSent(notice.clone()));
        }
        self.notice_sent(notice);
    }

    /// Sends a signal from the outside to the first process, kept in the record.
    fn kill(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut Machine, signal: Signal) {
        self.kept_in_step(Entry::Kill(signal));
        let was = self.set_outside(false);
        kernel.kill(cpu, self, signal);
        self.set_outside(was);
    }

    /// What the client does after the container's end ([`streams::finish`]), the kernel settling
    /// with `settle` after each step, kept in the record; in a replay, the record's steps and
    /// settlings: whether every output end was closed.
    fn finished(self: &Arc<Self>, mut settle: impl FnMut()) -> bool {
        // The kernel's own calls on the host while it settles are its, and kept.
        let mut settled = |host: &Self| {
            let was = host.set_outside(false);
            settle();
            host.set_outside(was);
        };
        if self.replaying() {
            let tape = self.tape.as_ref().expect("a record");
            loop {
                let entry = lock(tape).take();
                match entry {
                    Entry::StreamReady(end, ready) => self.stream_readied(end, ready),
                    Entry::Settled => settled(self),
                    Entry::Finished(closed) => return closed,
                    other => lock(tape).out_of_step(&other),
                }
            }
        }
        let was = self.set_outside(true);
        let closed = streams::finish(self, || {
            if let Some(tape) = &self.tape {
                lock(tape).keep(Entry::Settled);
            }
            settled(self);
        });
        self.set_outside(was);
        if let Some(tape) = &self.tape {
            lock(tape).keep(Entry::Finished(closed));
        }
        closed
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

impl TestHost {
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

impl wcontainers_clock::Clock for TestHost {
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

impl wcontainers_random_source::RandomSource for TestHost {
    fn fill_random_bytes(&self, into: &impl VolatileBytes) -> Result<(), HostError> {
        let bytes = self.answer(
            Entry::Random,
            |entry| match entry {
                Entry::Random(bytes) if bytes.len() == length(into) => Ok(bytes),
                other => Err(other),
            },
            || {
                let mut bytes = vec![0; length(into)];
                lock(&self.random).fill_bytes(&mut bytes);
                bytes
            },
        );
        into.write(&bytes);
        Ok(())
    }
}

impl wcontainers_system::FallbackObserver<Warning<'static>> for TestHost {
    fn on_fallback_used(&self, warning: &Warning<'static>) {
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
impl wcontainers_system::StreamTransport for TestHost {
    type Endpoint = crate::streams::End;

    /// In a replay the client still shows what the record says was taken.
    fn send_stream(
        &self,
        peer: &crate::streams::End,
        payload: &impl VolatileBytes,
    ) -> Result<usize, HostError> {
        let bytes = copied(payload);
        let result = self.answer(
            Entry::StreamSent,
            |entry| match entry {
                Entry::StreamSent(result) => Ok(result),
                other => Err(other),
            },
            || lock(&self.streams).send(*peer, &bytes),
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
        into: &impl VolatileBytes,
    ) -> Result<usize, HostError> {
        let (result, bytes) = self.answer(
            |(result, bytes)| Entry::StreamReceived(result, bytes),
            |entry| match entry {
                Entry::StreamReceived(result, bytes) if bytes.len() <= length(into) => {
                    Ok((result, bytes))
                }
                other => Err(other),
            },
            || {
                let mut bytes = vec![u8::MIN; length(into)];
                let result = lock(&self.streams).receive(*peer, &mut bytes);
                bytes.truncate(*result.as_ref().unwrap_or(&usize::MIN));
                (result, bytes)
            },
        );
        into.write(&bytes);
        result
    }

    fn close_stream(&self, peer: &crate::streams::End) {
        lock(&self.streams).close(*peer);
    }
}

impl wcontainers_readiness::Readiness<crate::streams::End> for TestHost {
    async fn wait_until_ready(
        &self,
        peer: &crate::streams::End,
        interest: wcontainers_readiness::ReadyFor,
    ) -> Result<wcontainers_readiness::ReadinessReport, HostError> {
        if !self.replaying() {
            lock(&self.streams).arm(*peer, interest);
        }
        Ok(Readied::wait(&self.stream_ready, peer.0, || None).await)
    }
}

/// The backend's messages, each sent (kept in the record) and waited on until its reply arrives:
/// a message FUSE gives no reply, or one this host answers itself, finishes once sent.
impl wcontainers_system::FuseServer for TestHost {
    type Reply = Vec<u8>;
    type Notice = Vec<u8>;

    /// The next notice sent, once the kernel has said how the one before was applied, which lets
    /// through the replies that waited for it.
    async fn receive_notice(
        &self,
        applied: Option<Result<(), Errno>>,
    ) -> Result<Vec<u8>, HostError> {
        let held = std::mem::take(&mut lock(&self.notices).held);
        if held != applied.is_some() {
            let broken = if held {
                format!("a notice's wait told {applied:?} where it was given a notice")
            } else {
                format!("a notice's wait told {applied:?} where it was given none")
            };
            lock(&self.streams).broken.push(broken);
        }
        if held {
            Replies::notice_applied(&self.replies);
        }
        std::future::poll_fn(|context| {
            let mut notices = lock(&self.notices);
            match notices.queued.pop_front() {
                Some(notice) => {
                    notices.held = true;
                    Poll::Ready(Ok(notice))
                }
                None => {
                    notices.waker = Some(context.waker().clone());
                    Poll::Pending
                }
            }
        })
        .await
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
impl wcontainers_system::DomainNameResolver for TestHost {
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
            let applied = replies.applied;
            match replies.arrived.entry(key) {
                std::collections::btree_map::Entry::Occupied(reply) if reply.get().0 <= applied => {
                    Poll::Ready(reply.remove().1)
                }
                _ => {
                    replies.waiting.insert(key, context.waker().clone());
                    Poll::Pending
                }
            }
        })
        .await
    }

    /// The reply to `key` arrived, after `notices` notices were sent: the call waiting for it is
    /// woken once they are applied.
    fn arrived(replies: &Mutex<Replies>, key: u64, notices: u64, reply: Vec<u8>) {
        let waiting = {
            let mut replies = lock(replies);
            replies.arrived.insert(key, (notices, reply));
            (notices <= replies.applied)
                .then(|| replies.waiting.remove(&key))
                .flatten()
        };
        if let Some(waker) = waiting {
            waker.wake();
        }
    }

    /// The kernel applied one more notice: the calls whose replies waited for it are woken.
    fn notice_applied(replies: &Mutex<Replies>) {
        let woken: Vec<Waker> = {
            let mut replies = lock(replies);
            replies.applied += 1;
            let Replies {
                arrived,
                waiting,
                applied,
            } = &mut *replies;
            arrived
                .iter()
                .filter(|(_, (notices, _))| notices == applied)
                .filter_map(|(key, _)| waiting.remove(key))
                .collect()
        };
        for waker in woken {
            waker.wake();
        }
    }
}

impl TestHost {
    /// The backend's reply to request `unique` arrived: the call waiting for it is woken.
    fn reply_arrived(&self, unique: u64, reply: Vec<u8>) {
        let notices = lock(&self.notices).sent;
        Replies::arrived(&self.replies, unique, notices, reply);
    }

    /// The resolver's answer to question `number` arrived: the call waiting for it is woken.
    fn name_arrived(&self, number: u64, answer: Vec<u8>) {
        Replies::arrived(&self.names, number, 0, answer);
    }

    /// The backend sent a notice: the kernel's wait for one is woken.
    fn notice_sent(&self, notice: Vec<u8>) {
        let waker = {
            let mut notices = lock(&self.notices);
            notices.sent += 1;
            notices.queued.push_back(notice);
            notices.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
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

    /// What socket `number` is ready for now, asked of the socket as a host's wait asks when it is
    /// polled; none in a run that keeps or answers from a record, which hears the outside only
    /// between turns, where its record keeps what it heard.
    pub(crate) fn socket_ready_now(
        &self,
        number: u32,
    ) -> Option<wcontainers_readiness::ReadinessReport> {
        if self.taped_run() {
            return None;
        }
        lock(&self.network).ready_now(number)
    }
}

impl TestHost {
    pub(crate) fn streams(&self) -> MutexGuard<'_, Streams> {
        lock(&self.streams)
    }

    pub(crate) fn choices(&self) -> MutexGuard<'_, Schedule> {
        lock(&self.schedule)
    }

    /// Changes a file the backend serves underneath the kernel.
    pub(crate) fn change_served(&self, path: &str, bytes: &[u8]) {
        self.backend()
            .as_ref()
            .expect("a container with a backend")
            .change(path, bytes);
    }

    /// Ends the kernel's wait on an endpoint with what it is ready for, kept in the record; the
    /// kernel settles as its runner next runs, as for a socket.
    pub(crate) fn ready(
        &self,
        end: crate::streams::End,
        ready: wcontainers_readiness::ReadinessReport,
    ) {
        if self.taped_run() {
            self.kept_in_step(Entry::StreamReady(end, ready));
        }
        self.stream_readied(end, ready);
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
/// taken, steps of deadlines they have waited for and backends they have started, for a watchdog
/// that tells a run that is slow from one stuck inside a single call or turn.
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
    /// The container's address-space size given at start.
    pub address_space_bytes: u64,
    /// The memory size the guest is told, if not the executor's address space's end.
    pub memory_bytes: Option<u64>,
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
        address_space_bytes: wcontainers_kernel_linux::ARM64_ADDRESS_SPACE_BYTES,
        memory_bytes: None,
        wall_clock: false,
        tape: None,
        options: Default::default(),
    }
}

/// As [`run_injected`], on a kernel [`prepare`] made for the container.
///
/// # Panics
///
/// If the run is to be saved: a save copied the test process with `fork`, which no call without
/// `unsafe` makes.
pub fn run_prepared(
    mut prepared: Prepared,
    container: &Container<'_>,
    seed: u64,
    injections: Injections,
) -> Result<Run, Failure> {
    assert_eq!(
        injections.saves, 0,
        "a save copied the test process with fork(2), which every crate that offers it marks unsafe; saving is not built on the kernel's runners"
    );
    let mut host = if prepared.wall_clock {
        TestHost {
            waits: true,
            ..TestHost::new(seed)
        }
    } else {
        TestHost::repeatable(seed, injections.wall_start)
    };
    if let Some(tape) = prepared.tape.take() {
        host = host.taped(tape);
    }
    if let Some(hooks) = prepared.hooks.take() {
        host = host.with_hooks(hooks);
    }
    run_hosted(prepared, host, container, seed, injections)
}

/// A kernel interface's kernel on the stand-in, with the embedder's state `State`.
pub type InterfaceKernel<State> =
    wcontainers_kernel_linux::KernelWithExecutor<crate::standin::StandIn, State, TestHost>;

/// A runner of an [`InterfaceKernel`].
pub type InterfaceRunner<State> =
    wcontainers_kernel_linux::KernelRunner<crate::standin::StandIn, State, TestHost>;

/// A kernel with the container started on the stand-in, made as the kernel interface makes one,
/// with `parallelism` runners, and those runners: its first process, with `state`, is loaded and
/// has not run. The stand-in stays on the host thread that made it, so each runner runs with
/// [`wcontainers_kernel_linux::KernelRunner::run_on`] on a stand-in of that thread.
pub fn started_with_executor<State: Clone + Send + 'static>(
    prepared: Prepared,
    container: &Container<'_>,
    state: State,
    parallelism: std::num::NonZeroUsize,
) -> (
    InterfaceKernel<State>,
    Vec<InterfaceRunner<State>>,
    Arc<TestHost>,
) {
    /// The configuration holds a start that lives as long as the kernel: what it names lives as
    /// long as the test.
    fn kept(bytes: &[u8]) -> &'static [u8] {
        Box::leak(bytes.into())
    }
    let host = TestHost::repeatable(0, None);
    *host.streams() = Streams::new(prepared.client);
    let arguments: Vec<&[u8]> = container
        .arguments
        .iter()
        .map(|argument| kept(argument.as_bytes()))
        .collect();
    let environment: Vec<&[u8]> = container
        .environment
        .iter()
        .map(|variable| kept(variable.as_bytes()))
        .collect();
    let cpu = crate::standin::StandIn::with_space(container.address_space);
    let start = Start {
        path: kept(container.path.as_bytes()),
        arguments: Box::leak(arguments.into_boxed_slice()),
        environment: Box::leak(environment.into_boxed_slice()),
        working_directory: kept(container.working_directory.as_bytes()),
        hostname: reference::HOSTNAME.as_bytes(),
        groups: Box::leak(prepared.groups.into_boxed_slice()),
        backend: None,
        standard: host.standard(),
        address_space_bytes: prepared.address_space_bytes,
        memory_bytes: prepared
            .memory_bytes
            .unwrap_or(wcontainers_abi::Cpu::address_space(&cpu).end),
        ceiling: prepared.ceiling,
        options: prepared.options,
    };
    let host = Arc::new(host);
    let config = wcontainers_kernel_linux::KernelConfig::new(
        prepared.kernel,
        cpu,
        Arc::clone(&host),
        start,
        serial_and_exact(),
        state,
    );
    let (kernel, runners) =
        wcontainers_kernel_linux::KernelWithExecutor::with_runners(config, parallelism);
    (kernel, runners, host)
}

/// The execution configuration the stand-in runs: one thread at a time, replayed exactly.
fn serial_and_exact() -> wcontainers_kernel::ExecutionConfig<Back> {
    wcontainers_kernel::ExecutionConfig {
        thread_ordering: wcontainers_kernel::ThreadOrdering::Serial,
        replay_determinism: wcontainers_kernel::ReplayDeterminism::Exact,
        translator: (),
    }
}

/// The id of a container's first process, as the kernel numbers it inside the container.
pub const FIRST_PROCESS: wcontainers_kernel::TaskId = wcontainers_kernel::TaskId(1);

/// A kernel on the stand-in with a container started with the guest program `name`, built with
/// `link_flags` into `out`, whose first process has not run and holds a number as the embedder's
/// state.
pub fn program_not_run(
    name: &str,
    link_flags: &[&str],
    out: &std::path::Path,
) -> InterfaceKernel<u64> {
    program_started(
        name,
        link_flags,
        out,
        std::num::NonZeroUsize::MIN,
        Default::default(),
    )
    .0
}

/// A kernel on the stand-in made with `parallelism` runners, with a container started with the
/// guest program `name`, built with `link_flags` into `out`, whose first process has not run; its
/// runners; and its system.
pub fn program_on_runners(
    name: &str,
    link_flags: &[&str],
    out: &std::path::Path,
    parallelism: std::num::NonZeroUsize,
) -> (
    InterfaceKernel<u64>,
    Vec<InterfaceRunner<u64>>,
    Arc<TestHost>,
) {
    program_started(name, link_flags, out, parallelism, Default::default())
}

/// As [`program_not_run`], run until every thread waits with no deadline.
pub fn program_waiting(
    name: &str,
    link_flags: &[&str],
    out: &std::path::Path,
) -> InterfaceKernel<u64> {
    let (started, mut runners, _) = program_started(
        name,
        link_flags,
        out,
        std::num::NonZeroUsize::MIN,
        Default::default(),
    );
    let finished = run_started(&mut runners[0]);
    assert_eq!(finished, None, "the program waits for ever");
    started
}

/// The exit code of the guest program `name`, built with `link_flags` into `out`, run to its end
/// on a kernel made with `parallelism` runners.
pub fn program_exit_code(
    name: &str,
    link_flags: &[&str],
    out: &std::path::Path,
    parallelism: std::num::NonZeroUsize,
) -> i32 {
    program_exit_code_with(name, link_flags, out, parallelism, Default::default())
}

/// As [`program_exit_code`], with what the container's configuration sets beyond its program.
pub fn program_exit_code_with(
    name: &str,
    link_flags: &[&str],
    out: &std::path::Path,
    parallelism: std::num::NonZeroUsize,
    options: wcontainers_kernel_linux::StartOptions<'static>,
) -> i32 {
    let (_started, mut runners, _) = program_started(name, link_flags, out, parallelism, options);
    let end = run_started(&mut runners[0]).expect("the program ends");
    end_code(end).unwrap_or_else(|failure| panic!("the program did not start: {failure:?}"))
}

/// Runs a started kernel's threads with `runner`, on a stand-in of this host thread, until its
/// container ends, with how its first process ended, or every thread waits with no deadline.
fn run_started(runner: &mut InterfaceRunner<u64>) -> Option<ProcessEnd> {
    let mut stand_in = crate::standin::StandIn::new();
    loop {
        match runner.run_on(&mut stand_in) {
            StopReason::Finished(end) => return Some(end),
            StopReason::Idle { until: None } => return None,
            StopReason::Interrupted => {}
            other => panic!("the program waits for its deadline, not {other:?}"),
        }
    }
}

/// The exit code `docker run` gives for how the first process ended, or why it did not start.
fn end_code(end: ProcessEnd) -> Result<i32, Failure> {
    match end {
        ProcessEnd::Exited { code } => Ok(i32::from(code)),
        ProcessEnd::Signaled { signal, .. } => {
            Ok(wcontainers_abi::SIGNALLED_EXIT_BASE + signal as i32)
        }
        ProcessEnd::NotStarted { error } => Err(Failure::Start(error)),
    }
}

/// A container started with the guest program `name`, built with `link_flags` into `out`, and
/// `options`, on a kernel with `parallelism` runners, and its system.
fn program_started(
    name: &str,
    link_flags: &[&str],
    out: &std::path::Path,
    parallelism: std::num::NonZeroUsize,
    options: wcontainers_kernel_linux::StartOptions<'static>,
) -> (
    InterfaceKernel<u64>,
    Vec<InterfaceRunner<u64>>,
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
    let prepared = Prepared {
        options,
        ..prepare(&container)
    };
    started_with_executor(prepared, &container, u64::MIN, parallelism)
}

/// A runner of the kernel the driver runs.
type DrivenRunner = wcontainers_kernel_linux::KernelRunner<Back, (), TestHost>;

fn run_hosted(
    prepared: Prepared,
    mut host: TestHost,
    container: &Container<'_>,
    seed: u64,
    injections: Injections,
) -> Result<Run, Failure> {
    let Prepared {
        kernel,
        groups,
        echo,
        client,
        ceiling,
        address_space_bytes,
        memory_bytes,
        wall_clock,
        options,
        ..
    } = prepared;
    // The kernel's own stand-in makes its memories; the runner runs its turns on another of this
    // host thread, as an executor stays on the thread that made it.
    let cpu = back_end(container.address_space, wall_clock);
    let mut stand_in = back_end(container.address_space, wall_clock);
    *held(&mut host.injecting) = Some(Injecting::new(seed, injections));
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
        PROGRESS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
        address_space_bytes,
        memory_bytes: memory_bytes.unwrap_or(wcontainers_abi::Cpu::address_space(&cpu).end),
        ceiling,
        options,
    };
    let host = Arc::new(host);
    let mut driven = Driven::start(kernel, cpu, &host, start, serial_and_exact(), ())
        .unwrap_or_else(|unsupported| panic!("the stand-in's configuration: {unsupported:?}"));
    let mut runner = driven.runner(&host);
    driven.with_parts(|kernel, cpu| host.begin(kernel, cpu));
    let end = drive(&mut driven, &mut runner, &mut stand_in, &host);
    let input_closed = host.streams().input_closed_by_kernel();
    let outputs_closed = host.finished(|| {
        let _ = runner.run_on(&mut stand_in);
    });
    let exit_code = end_code(end)?;
    let files = match &*host.backend() {
        Some(_) if container.root_image.is_some() => Vec::new(),
        Some(backend) if root_on_backend => image::records(&backend.files()),
        _ => driven.with_parts(|kernel, cpu| kernel.root_files(cpu)),
    };
    let tape = host.take_tape();
    if let Some(tape) = &tape {
        assert!(
            tape.used_up(),
            "the replay left {} entries of its record",
            tape.len()
        );
    }
    let turns = host.injecting().turns;
    let streams = std::mem::take(&mut *host.streams());
    let counters = driven.with_parts(|kernel, _| kernel.counters().collect());
    Ok(Run {
        outcome: Outcome {
            stdout: streams.stdout,
            stderr: streams.stderr,
            exit_code,
        },
        warnings: std::mem::take(&mut *lock(&host.warnings)),
        cpu_warnings: std::mem::take(&mut *lock(&host.cpu_warnings)),
        counters,
        files,
        saved: 0,
        turns,
        most_input_asked: streams.most_input_asked,
        broken: streams.broken,
        outputs_closed,
        input_closed,
        tape,
        memories: Vec::new(),
        refused: Vec::new(),
    })
}

/// Runs the container's threads with `runner` on `stand_in` until the container ends, doing what
/// the outside does each time the runner comes back: how the first process ended.
fn drive(
    driven: &mut Driven,
    runner: &mut DrivenRunner,
    stand_in: &mut Back,
    host: &Arc<TestHost>,
) -> ProcessEnd {
    loop {
        let stop = runner.run_on(stand_in);
        match stop {
            StopReason::Finished(end) => return end,
            StopReason::Interrupted => {}
            StopReason::Idle { until } => {
                let until = until.map(|until| {
                    u64::try_from(until.as_nanos()).expect("a deadline fits in 64 bits")
                });
                driven.with_parts(|kernel, cpu| host.idle(kernel, cpu, until));
            }
            StopReason::Suspended | StopReason::Panicked => {
                panic!("the kernel's runner stopped: {stop:?}")
            }
        }
        driven.with_parts(|kernel, cpu| host.arrived(kernel, cpu));
    }
}

impl TestHost {
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

impl TestHost {
    /// What the outside does each time the kernel's runner comes back, which counts as a turn of
    /// the run: the time the runner's turns took passes, the outside's answers and the client's
    /// steps arrive, and the signals and notices due at this turn are sent.
    fn arrived_live(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut Machine) {
        let turns = {
            let mut injecting = self.injecting();
            injecting.turns += 1;
            injecting.turns
        };
        PROGRESS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // The stand-in tells no count of what the runner's turns ran, so a repeatable clock moves
        // on by one turn's work of the kernel, drawn from the seed.
        if let Some(now) = &self.virtual_now {
            let work = lock(&self.schedule).draws().random_range(TURN_NANOSECONDS);
            now.fetch_add(work, Ordering::Relaxed);
        }
        network_events(self, Duration::ZERO);
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
            self.send_notice(notice);
        }
        while self.injecting().signal_at.first() == Some(&turns) {
            let signal = {
                let mut injecting = self.injecting();
                injecting.signal_at.remove(0);
                injecting.signal
            };
            TestHost::kill(self, kernel, cpu, signal);
        }
    }

    /// What the outside does when the kernel's runner comes back with nothing to run until
    /// `until_nanoseconds`: the test's hooks act, an answer owed arrives, the outside is heard,
    /// the client takes a step, or the time waited for passes. The runner wakes the kernel's
    /// waits and fires its timers as it next runs.
    fn idle_live(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut Machine,
        until_nanoseconds: Option<u64>,
    ) {
        if let Some(hooks) = &mut *lock(&self.hooks)
            && hooks.between_steps(kernel, cpu)
        {
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
            if network_events(self, wait) {
                return;
            }
        }
        if streams::act(kernel, cpu, self, true) {
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
    }

    /// What descriptors 0 to 2 of the first process are: the ends this host holds the other sides of.
    pub fn standard(&self) -> Standard<crate::streams::End> {
        lock(&self.streams).client.standard()
    }

    /// What the client does at the start, before the first process runs, kept in the record.
    fn begin(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut Machine) {
        self.outside_part(
            kernel,
            cpu,
            |()| Entry::Begun,
            |entry| (*entry == Entry::Begun).then_some(()),
            |host, kernel, cpu| streams::begin(kernel, cpu, host),
        );
    }

    /// [`TestHost::arrived_live`], kept in the record.
    fn arrived(self: &Arc<Self>, kernel: &mut Kernel, cpu: &mut Machine) {
        self.outside_part(
            kernel,
            cpu,
            |()| Entry::Arrived,
            |entry| (*entry == Entry::Arrived).then_some(()),
            |host, kernel, cpu| host.arrived_live(kernel, cpu),
        );
    }

    /// [`TestHost::idle_live`], kept in the record.
    fn idle(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut Machine,
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

/// How much of a waiting host's wait for a deadline passes between its marks of progress.
const WAIT_STEP: Duration = Duration::from_secs(1);

/// The longest the driver waits for the outside when nothing else can happen.
const NETWORK_PATIENCE: Duration = Duration::from_secs(10);

/// Hands the kernel what the outside sent, after waiting up to `wait` for something; whether
/// anything came.
fn network_events(host: &TestHost, wait: Duration) -> bool {
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
            }
            crate::network::Owed::Ready(number, ready) => {
                if host.taped_run() {
                    host.kept_in_step(Entry::SocketReady(*number, *ready));
                }
                host.socket_ready(*number, *ready);
            }
        }
    }
    !owed.is_empty()
}

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
pub(crate) fn deliver(kernel: &mut Kernel, cpu: &mut Machine, host: &Arc<TestHost>) {
    host.outside_part(
        kernel,
        cpu,
        |()| Entry::Delivered,
        |entry| (*entry == Entry::Delivered).then_some(()),
        |host, _, _| deliver_live(host),
    );
}

fn deliver_live(host: &TestHost) {
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
        }
        crate::backend::Message::Notice(notice) => host.send_notice(notice),
    }
}
