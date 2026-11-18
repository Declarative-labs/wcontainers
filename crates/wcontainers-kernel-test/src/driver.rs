//! The driver loop and a host for tests: runs a program on the kernel with the stand-in CPU.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use wcontainers_abi::{
    BackendRequestId, ConnectionId, CounterReading, End, Event, Host, HostError, Next, Warning,
};
use wcontainers_kernel::{Errno, FileRecord, Kernel, Start};

use crate::standin::{StandIn, Unsupported};
use crate::streams::{Outside, StreamHost, Streams};
use crate::{image, reference, streams};

/// Why a run on our kernel gave no outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The kernel could not start the program.
    Start(Errno),
    /// The program did something the stand-in CPU cannot run as the reference does.
    StandIn(Unsupported),
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
    pub counters: Vec<CounterReading>,
    /// The root file system when the container ended.
    pub files: Vec<FileRecord>,
    /// How many times it was saved and brought back.
    pub saved: usize,
    /// How many turns on the CPU side it took.
    pub turns: u64,
    /// The most input credit the kernel had given and not seen used.
    pub most_input_credit: u64,
    /// What the kernel did against the credit rules on its ends.
    pub broken: Vec<String>,
    /// Whether the kernel closed every output end once the container ended.
    pub outputs_closed: bool,
}

/// The host used in tests. Randomness comes from a seed so runs repeat exactly. When the kernel is idle
/// until a time, the host's clocks jump to it instead of waiting.
pub struct TestHost {
    started: Instant,
    /// How far the clocks have jumped ahead.
    skipped: u64,
    random: StdRng,
    /// The other sides of the container's ends.
    pub(crate) streams: Streams,
    warnings: Vec<Warned>,
    /// A clock that moves only when the driver says, so a run repeats exactly; none for the wall
    /// clock (layer 1, where the reference runs beside it).
    virtual_now: Option<u64>,
    /// With a clock that moves only when the driver says, the wall clock's reading when the
    /// monotonic clock read 0: the wall clock moves with it.
    wall_base: u64,
    /// What a run's choices come from: how long each turn on the CPU side lasts, and when the
    /// backend's answers arrive.
    schedule: StdRng,
    /// The file backend, for a container with a volume.
    pub(crate) backend: Option<crate::backend::Backend>,
    /// Connections to the outside and lookups.
    network: crate::network::Network,
}

impl TestHost {
    /// A host whose clock is the wall clock's.
    pub fn new(seed: u64) -> Self {
        Self {
            started: Instant::now(),
            skipped: 0,
            random: StdRng::seed_from_u64(seed),
            streams: Streams::default(),
            warnings: Vec::new(),
            virtual_now: None,
            wall_base: 0,
            schedule: StdRng::seed_from_u64(seed ^ 0x5eed),
            backend: None,
            network: Default::default(),
        }
    }

    /// A host whose clocks move only with the work run and the waits skipped, both from the seed,
    /// so the kernel's choices repeat exactly for a seed.
    pub fn repeatable(seed: u64) -> Self {
        let mut host = Self {
            virtual_now: Some(1_000_000_000),
            ..Self::new(seed)
        };
        host.wall_base = wall_clock() - 1_000_000_000;
        host
    }

    /// Advances a repeatable clock by the time one turn on the CPU side took: between 0.5 and 1.5
    /// nanoseconds for each instruction it ran (a CPU's speed varies with its caches and its load),
    /// and between 0.5 and 1.5 microseconds more, both drawn from the seed. The speed moves the
    /// points where the scheduler switches threads, so seeds give different interleavings.
    fn ran(&mut self, instructions: u64) {
        let picoseconds = self.schedule.random_range(500..1500);
        let step =
            instructions.saturating_mul(picoseconds) / 1000 + self.schedule.random_range(500..1500);
        if let Some(now) = &mut self.virtual_now {
            *now += step;
        }
    }
}

/// The machine's wall clock, in nanoseconds since 1970.
fn wall_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_nanos() as u64
}

impl TestHost {
    /// The host a saved container is brought back on. What the container wrote has already left;
    /// the monotonic clock reads from an earlier start than this one's (a kernel that did not carry
    /// its clock over would see it go back) and the wall clock goes on from the save (a repeatable
    /// one without the time the save took, moved on by `wall_step`); randomness and the length of
    /// turns come from `seed`.
    fn moved(&mut self, seed: u64, wall_step: u64) -> Self {
        let wall = self.realtime_nanoseconds();
        let mut host = Self {
            started: Instant::now(),
            skipped: self.skipped,
            random: StdRng::seed_from_u64(seed),
            streams: self.streams.clone(),
            warnings: self.warnings.clone(),
            virtual_now: self.virtual_now.map(|_| 0),
            wall_base: 0,
            schedule: StdRng::seed_from_u64(seed ^ 0x5eed),
            // The backend and the answers it owes go on with the copy.
            backend: self.backend.take(),
            // A new host has none of the old one's connections; its published ports go on.
            network: self.network.moved(),
        };
        host.wall_base = wall + wall_step - host.monotonic_nanoseconds();
        host
    }

    /// Moves the clocks forward to a time on the monotonic clock, as if the host had waited.
    pub fn skip_to(&mut self, until: u64) {
        let now = self.monotonic_nanoseconds();
        self.skipped += until.saturating_sub(now);
    }
}

impl Host for TestHost {
    fn monotonic_nanoseconds(&mut self) -> u64 {
        match self.virtual_now {
            Some(now) => now + self.skipped,
            None => self.started.elapsed().as_nanos() as u64 + self.skipped,
        }
    }

    fn realtime_nanoseconds(&mut self) -> u64 {
        match self.virtual_now {
            Some(_) => self.wall_base + self.monotonic_nanoseconds(),
            None => wall_clock() + self.skipped,
        }
    }

    fn random(&mut self, into: &mut [u8]) {
        self.random.fill_bytes(into);
    }

    fn warn(&mut self, warning: Warning<'_>) {
        // Only a missing call's detail is a call number.
        let name = if warning.reason == wcontainers_kernel::reason::NOT_IMPLEMENTED {
            wcontainers_kernel::system_call_name(warning.detail).unwrap_or("")
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
        self.warnings.push(Warned {
            reason: warning.reason.to_string(),
            count: warning.count,
            detail: warning.detail,
        });
    }

    fn backend_send(&mut self, request: BackendRequestId, message: &[u8]) -> Result<(), HostError> {
        let backend = self.backend.as_mut().ok_or(HostError::NotSupported)?;
        // The answer to a RETRIEVE this host sent in the server's place stays here.
        if request.0 & RETRIEVED == RETRIEVED {
            return Ok(());
        }
        backend.send(request.0, message);
        Ok(())
    }

    fn stream_send(&mut self, end: End, bytes: &[u8]) {
        self.streams.send(end, bytes);
    }

    fn stream_credit(&mut self, end: End, bytes: u64) {
        self.streams.credit(end, bytes);
    }

    fn stream_close(&mut self, end: End) {
        self.streams.close(end);
    }

    fn connect(
        &mut self,
        connection: ConnectionId,
        address: &[u8],
        port: u16,
        stream: bool,
    ) -> Result<(), HostError> {
        self.network.connect(connection.0, address, port, stream)
    }

    fn send(&mut self, connection: ConnectionId, bytes: &[u8]) -> Result<usize, HostError> {
        self.network.send(connection.0, bytes)
    }

    fn receive(&mut self, connection: ConnectionId, into: &mut [u8]) -> Result<usize, HostError> {
        self.network.receive(connection.0, into)
    }

    fn finish(&mut self, connection: ConnectionId) {
        self.network.finish(connection.0);
    }

    fn close(&mut self, connection: ConnectionId) {
        self.network.close(connection.0);
    }

    fn look_up(&mut self, request: u64, name: &str) -> Result<(), HostError> {
        self.network.look_up(request, name);
        Ok(())
    }

    fn listen(&mut self, listener: ConnectionId, port: u16) -> Result<(), HostError> {
        self.network.listen(listener.0, port)
    }

    fn accept(
        &mut self,
        listener: ConnectionId,
        connection: ConnectionId,
    ) -> Result<(), HostError> {
        self.network.accept(listener.0, connection.0)
    }
}

impl StreamHost for TestHost {
    fn streams(&mut self) -> &mut Streams {
        &mut self.streams
    }

    fn choices(&mut self) -> &mut StdRng {
        &mut self.schedule
    }
}

/// The exit code `docker run` gives for a wait status.
pub fn exit_code(wait_status: i32) -> i32 {
    let signal = wait_status & 0x7f;
    if signal == 0 {
        (wait_status >> 8) & 0xff
    } else {
        128 + signal
    }
}

/// How many turns on the CPU side this process's runs have ended, for a watchdog that tells a
/// run that is slow from one stuck inside a single call or turn.
pub static TURNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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
    /// Whether the backend opens files for the page cache rather than for direct reads and writes.
    pub cached: bool,
    /// Ports of this machine's loopback that reach the container's ports, as (container port,
    /// outside port), as `docker run -p` publishes them.
    pub published: &'a [(u16, u16)],
    /// Where each process's address space ends: the host's memories are 32 or 64 bit
    /// ([`crate::standin::SPACE_32_BIT`], [`crate::standin::SPACE_64_BIT`]).
    pub address_space: u64,
}

/// As [`run_injected`], with the outside of the container's standard streams.
pub fn run_outside(
    container: &Container<'_>,
    outside: Outside,
    seed: u64,
    injections: Injections,
) -> Result<Run, Failure> {
    let prepared = Prepared {
        outside,
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
    pub signal: Option<(u32, usize)>,
    /// How many notices the file backend sends unasked, of kinds and about nodes drawn from the
    /// seed, as a backend whose files changed underneath would.
    pub notices: usize,
}

impl Default for Injections {
    fn default() -> Self {
        Self {
            saves: 0,
            save_gap: 100,
            wall_step: 0,
            signal: None,
            notices: 0,
        }
    }
}

/// Turn counts at which something is injected: `count` of them, each 1 to 100 turns after the last.
fn moments(seed: u64, count: usize) -> Vec<u64> {
    moments_apart(seed, count, 100)
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
    /// The outside of its standard streams.
    pub outside: Outside,
}

/// Puts a container's files in a new kernel's root, unless the root is on the backend.
pub fn prepare(container: &Container<'_>) -> Prepared {
    let mut kernel = Kernel::new();
    if container.backend != Some("/") {
        image::add_files(&mut kernel, container.files);
    }
    Prepared {
        kernel,
        groups: image::root_groups(container.files),
        echo: false,
        outside: Outside::default(),
    }
}

/// As [`run_injected`], on a kernel [`prepare`] made for the container.
pub fn run_prepared(
    prepared: Prepared,
    container: &Container<'_>,
    seed: u64,
    injections: Injections,
) -> Result<Run, Failure> {
    let Prepared {
        mut kernel,
        groups,
        echo,
        outside,
    } = prepared;
    let mut at = moments_apart(seed ^ 0x5a7e, injections.saves, injections.save_gap);
    let (signal, signals) = injections.signal.unwrap_or((0, 0));
    let mut signal_at = moments(seed ^ 0x5197, signals);
    // Notices come close together, so that a short program meets many of them.
    let mut notice_at = moments_apart(seed ^ 0x6e07, injections.notices, 10);
    let mut notices = StdRng::seed_from_u64(seed ^ 0x6e08);
    let mut turns = 0u64;
    let mut saved = 0;
    // In a copy, where its run is reported to the process it was copied from.
    let mut report: Option<i32> = None;
    let mut cpu = StandIn::with_space(container.address_space);
    let mut host = TestHost::repeatable(seed);
    let standard = outside.standard();
    host.streams = Streams::new(outside);
    host.streams.echo = echo;
    host.network.published = container.published.iter().copied().collect();
    let root_on_backend = container.backend == Some("/");
    if container.backend.is_some() {
        let image = root_on_backend.then_some(container.files);
        host.backend = Some(crate::backend::Backend::start(
            &crate::backend::out(),
            image,
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
        standard,
    };
    kernel.start(&mut cpu, &mut host, &start);
    streams::begin(&mut kernel, &mut cpu, &mut host);
    let status = loop {
        match kernel.next() {
            Next::Run {
                thread,
                resume,
                slice_nanoseconds,
            } => {
                // The seed chooses where in its slice each turn ends, so a run repeats for a seed
                // and threads interleave differently under different seeds.
                let mut slice_nanoseconds =
                    slice_nanoseconds.map(|slice| host.schedule.random_range(1..=slice.max(1)));
                // An answer owed arrives during the turn, at a moment drawn from the seed: the
                // host raises the running thread's stop flag then, as the boundary says.
                if host
                    .backend
                    .as_ref()
                    .is_some_and(|backend| backend.waiting())
                {
                    let arrival = host.schedule.random_range(1..=ANSWER_ARRIVES_WITHIN);
                    slice_nanoseconds = Some(slice_nanoseconds.map_or(arrival, |s| s.min(arrival)));
                }
                let exit = match cpu.run(thread, resume, slice_nanoseconds) {
                    Ok(exit) => exit,
                    // A copy cannot hand a failure back; it says what it was and ends.
                    Err(unsupported) if report.is_some() => {
                        panic!("the stand-in failed: {unsupported:?}")
                    }
                    Err(unsupported) => return Err(Failure::StandIn(unsupported)),
                };
                host.ran(cpu.last_instructions);

                kernel.exit(&mut cpu, &mut host, exit);
                turns += 1;
                TURNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                network_events(&mut kernel, &mut cpu, &mut host, Duration::ZERO);
                streams::act(&mut kernel, &mut cpu, &mut host, false);
                // The backend's answers arrive at moments and in an order drawn from the seed.
                while host
                    .backend
                    .as_ref()
                    .is_some_and(|backend| backend.waiting())
                    && host.schedule.random_bool(0.5)
                {
                    deliver(&mut kernel, &mut cpu, &mut host);
                }
                while notice_at.first() == Some(&turns) {
                    notice_at.remove(0);
                    let notice = notice(&mut notices);
                    kernel.event(
                        &mut cpu,
                        &mut host,
                        Event::BackendNotice { notice: &notice },
                    );
                }
                while signal_at.first() == Some(&turns) {
                    signal_at.remove(0);
                    kernel.kill(&mut cpu, &mut host, signal);
                }
                while at.first() == Some(&turns) {
                    at.remove(0);
                    saved += 1;
                    let mut ends = [0; 2];
                    // SAFETY: plain system calls; the copy goes on in this thread alone and ends with
                    // `_exit` once it has reported.
                    unsafe {
                        assert_eq!(libc::pipe(ends.as_mut_ptr()), 0, "a pipe for the copy");
                        match libc::fork() {
                            -1 => panic!("fork failed"),
                            0 => {
                                libc::close(ends[0]);
                                std::panic::set_hook(Box::new(|info| {
                                    panic_in_copy(&info.to_string())
                                }));
                                if let Some(earlier) = report.replace(ends[1]) {
                                    libc::close(earlier);
                                }
                            }
                            copy => {
                                libc::close(ends[1]);
                                // The copy alone holds the network from here on.
                                drop(std::mem::take(&mut host.network));
                                return Ok(finish(copied_run(copy, ends[0]), report));
                            }
                        }
                    }
                    cpu = cpu.restored();
                    host = host.moved(seed ^ ((saved as u64) << 32), injections.wall_step);
                    kernel.restore(&mut cpu, &mut host);
                }
            }
            Next::Idle { until_nanoseconds } => {
                // An answer owed comes before any time passes.
                if host
                    .backend
                    .as_ref()
                    .is_some_and(|backend| backend.waiting())
                {
                    deliver(&mut kernel, &mut cpu, &mut host);
                    continue;
                }
                // The outside answers in real time: a wait for it goes on as long as the kernel's
                // next timer, and at most ten seconds.
                if host.network.waiting() {
                    let now = host.monotonic_nanoseconds();
                    let wait = until_nanoseconds.map_or(NETWORK_PATIENCE, |until| {
                        Duration::from_nanos(until.saturating_sub(now)).min(NETWORK_PATIENCE)
                    });
                    if network_events(&mut kernel, &mut cpu, &mut host, wait) {
                        continue;
                    }
                }
                if streams::act(&mut kernel, &mut cpu, &mut host, true) {
                    continue;
                }
                let Some(until) = until_nanoseconds else {
                    panic!(
                        "the kernel is idle with nothing to wake it, and this host has no events \
                         (the outside waits at {:?}); output so far:\n{}{}",
                        host.streams.stuck_at(),
                        String::from_utf8_lossy(&host.streams.stdout),
                        String::from_utf8_lossy(&host.streams.stderr)
                    );
                };
                let now = host.monotonic_nanoseconds();
                host.skipped += until.saturating_sub(now);
                kernel.event(&mut cpu, &mut host, Event::Timer);
            }
            Next::Finished { status } => break status,
        }
    };
    let outputs_closed = streams::finish(&mut kernel, &mut cpu, &mut host);
    if let Some(error) = kernel.start_failure() {
        // Never in a copy: no save is made before the first program runs.
        return Err(Failure::Start(error));
    }
    let files = match &host.backend {
        Some(backend) if root_on_backend => image::records(&backend.files()),
        _ => kernel.root_files(&mut cpu),
    };
    let run = Run {
        outcome: Outcome {
            stdout: host.streams.stdout,
            stderr: host.streams.stderr,
            exit_code: exit_code(status),
        },
        warnings: host.warnings,
        counters: kernel.counters().collect(),
        files,
        saved,
        turns,
        most_input_credit: host.streams.most_input_credit,
        broken: host.streams.broken,
        outputs_closed,
    };
    Ok(finish(run, report))
}

/// The longest the driver waits for the outside when nothing else can happen.
const NETWORK_PATIENCE: Duration = Duration::from_secs(10);

/// Hands the kernel what the outside sent, after waiting up to `wait` for something; whether
/// anything came.
fn network_events(
    kernel: &mut Kernel,
    cpu: &mut dyn wcontainers_abi::Cpu,
    host: &mut TestHost,
    wait: Duration,
) -> bool {
    if !host.network.waiting() {
        return false;
    }
    let owed = host.network.take(wait);
    for event in &owed {
        event.with(|event| kernel.event(cpu, host, event));
    }
    !owed.is_empty()
}

/// The longest a turn runs while the backend owes an answer, in nanoseconds of the CPU side.
const ANSWER_ARRIVES_WITHIN: u64 = 50_000;

/// The request numbers of RETRIEVE notices this host sends: above any the kernel numbers.
const RETRIEVED: u64 = 1 << 62;

/// A notice a backend sends unasked (`fuse_notify`), of a kind and about a node (one of the first
/// few the backend numbers, some never looked up) drawn from `random`, sometimes cut short. The kernel's NOTIFY_REPLY to
/// a RETRIEVE comes back to this host, which sent it, not to the server ([`RETRIEVED`]).
fn notice(random: &mut StdRng) -> Vec<u8> {
    use fuse_abi::linux::*;
    use zerocopy::IntoBytes;
    let node: u64 = random.random_range(1..12);
    let name = b"backend-file\0";
    let namelen = (name.len() - 1) as u32;
    let (code, mut body): (fuse_notify_code, Vec<u8>) = match random.random_range(0..6) {
        0 => (
            fuse_notify_code::FUSE_NOTIFY_POLL,
            fuse_notify_poll_wakeup_out { kh: node }.as_bytes().to_vec(),
        ),
        1 => (
            fuse_notify_code::FUSE_NOTIFY_INVAL_INODE,
            fuse_notify_inval_inode_out {
                ino: node,
                off: 0,
                len: -1,
            }
            .as_bytes()
            .to_vec(),
        ),
        2 => {
            let mut body = fuse_notify_inval_entry_out {
                parent: node,
                namelen,
                flags: 0,
            }
            .as_bytes()
            .to_vec();
            body.extend_from_slice(name);
            (fuse_notify_code::FUSE_NOTIFY_INVAL_ENTRY, body)
        }
        3 => {
            let mut body = fuse_notify_delete_out {
                parent: 1,
                child: node,
                namelen,
                padding: 0,
            }
            .as_bytes()
            .to_vec();
            body.extend_from_slice(name);
            (fuse_notify_code::FUSE_NOTIFY_DELETE, body)
        }
        4 => (
            fuse_notify_code::FUSE_NOTIFY_RETRIEVE,
            fuse_notify_retrieve_out {
                notify_unique: RETRIEVED | node,
                nodeid: node,
                offset: 0,
                size: 4096,
                padding: 0,
            }
            .as_bytes()
            .to_vec(),
        ),
        _ => {
            let mut body = fuse_notify_store_out {
                nodeid: node,
                offset: 0,
                size: 4,
                padding: 0,
            }
            .as_bytes()
            .to_vec();
            body.extend_from_slice(b"data");
            (fuse_notify_code::FUSE_NOTIFY_STORE, body)
        }
    };
    // One in eight is cut short, which Linux refuses.
    if random.random_ratio(1, 8) {
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
pub(crate) fn deliver(
    kernel: &mut Kernel,
    cpu: &mut dyn wcontainers_abi::Cpu,
    host: &mut TestHost,
) {
    let Some(message) = host
        .backend
        .as_mut()
        .and_then(|backend| backend.next(&mut host.schedule))
    else {
        return;
    };
    match message {
        crate::backend::Message::Answer(unique, reply) => {
            let request = BackendRequestId(unique);
            kernel.event(
                cpu,
                host,
                Event::BackendReply {
                    request,
                    reply: &reply,
                },
            );
        }
        crate::backend::Message::Notice(notice) => {
            kernel.event(cpu, host, Event::BackendNotice { notice: &notice });
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
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 65536];
    // SAFETY: reads the copy's pipe until it closes, then reaps the copy.
    let status = unsafe {
        loop {
            let count = libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len());
            if count <= 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..count as usize]);
        }
        libc::close(fd);
        let mut status = 0;
        libc::waitpid(copy, &mut status, 0);
        status
    };
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
    number(&mut out, run.most_input_credit);
    number(&mut out, run.outputs_closed.into());
    number(&mut out, run.broken.len() as u64);
    for broken in &run.broken {
        bytes(&mut out, broken.as_bytes());
    }
    number(&mut out, run.warnings.len() as u64);
    for warned in &run.warnings {
        bytes(&mut out, warned.reason.as_bytes());
        number(&mut out, warned.count);
        number(&mut out, warned.detail);
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
            let (value, rest) = self.0.split_at(8);
            self.0 = rest;
            u64::from_le_bytes(value.try_into().expect("eight bytes"))
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
    let most_input_credit = reader.number();
    let outputs_closed = reader.number() != 0;
    let broken = (0..reader.number()).map(|_| reader.text()).collect();
    let warnings = (0..reader.number())
        .map(|_| Warned {
            reason: reader.text(),
            count: reader.number(),
            detail: reader.number(),
        })
        .collect();
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
        counters,
        files,
        saved,
        turns,
        most_input_credit,
        broken,
        outputs_closed,
    }
}
