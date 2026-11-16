//! The driver loop and a host for tests: runs a program on the kernel with the stand-in CPU.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use wcontainers_abi::{
    BackendRequestId, ConnectionId, CounterReading, Event, Host, HostError, Next, Stream, Warning,
};
use wcontainers_kernel::{Errno, FileRecord, Kernel, Start};

use crate::standin::{StandIn, Unsupported};
use crate::{image, reference};

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
}

/// The host used in tests. Randomness comes from a seed so runs repeat exactly. When the kernel is idle
/// until a time, the host's clocks jump to it instead of waiting.
pub struct TestHost {
    started: Instant,
    /// How far the clocks have jumped ahead.
    skipped: u64,
    random: StdRng,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    warnings: Vec<Warned>,
    /// A clock that moves only when the driver says, so a run repeats exactly; none for the wall
    /// clock (layer 1, where the reference runs beside it).
    virtual_now: Option<u64>,
    /// What a run's choices come from: how long each turn on the CPU side lasts.
    schedule: StdRng,
}

impl TestHost {
    /// A host whose clock is the wall clock's.
    pub fn new(seed: u64) -> Self {
        Self {
            started: Instant::now(),
            skipped: 0,
            random: StdRng::seed_from_u64(seed),
            stdout: Vec::new(),
            stderr: Vec::new(),
            warnings: Vec::new(),
            virtual_now: None,
            schedule: StdRng::seed_from_u64(seed ^ 0x5eed),
        }
    }

    /// A host whose clock moves only with the work run and the waits skipped, both from the seed,
    /// so the kernel's choices repeat exactly for a seed.
    pub fn repeatable(seed: u64) -> Self {
        Self {
            virtual_now: Some(1_000_000_000),
            ..Self::new(seed)
        }
    }

    /// Advances a repeatable clock by the time one turn on the CPU side took: between 0.5 and 1.5
    /// microseconds, drawn from the seed.
    fn ran(&mut self) {
        let step = self.schedule.random_range(500..1500);
        if let Some(now) = &mut self.virtual_now {
            *now += step;
        }
    }
}

impl TestHost {
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
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock is after 1970")
            .as_nanos() as u64
            + self.skipped
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
        eprintln!(
            "kernel warning: {} (detail {} {name}, count {})",
            warning.reason, warning.detail, warning.count
        );
        self.warnings.push(Warned {
            reason: warning.reason.to_string(),
            count: warning.count,
            detail: warning.detail,
        });
    }

    fn backend_send(&mut self, _: BackendRequestId, _: &[u8]) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn output(&mut self, stream: Stream, bytes: &[u8]) {
        match stream {
            Stream::Output => self.stdout.extend_from_slice(bytes),
            Stream::Error => self.stderr.extend_from_slice(bytes),
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

/// The exit code `docker run` gives for a wait status.
pub fn exit_code(wait_status: i32) -> i32 {
    let signal = wait_status & 0x7f;
    if signal == 0 {
        (wait_status >> 8) & 0xff
    } else {
        128 + signal
    }
}

/// What a container starts with: its files and its first process.
pub struct Container<'a> {
    pub files: &'a [image::File],
    pub path: &'a str,
    pub arguments: &'a [&'a str],
    pub environment: &'a [&'a str],
    pub working_directory: &'a str,
}

/// Runs a container on our kernel and the stand-in CPU until its first process ends.
pub fn run(container: &Container<'_>, seed: u64) -> Result<Run, Failure> {
    run_saving(container, seed, 0)
}

/// As [`run`], saving the container up to `saves` times at moments drawn from the seed (each 1 to
/// 100 turns after the last) and bringing it back each time on a new CPU side, as a host that saves
/// memories does.
pub fn run_saving(container: &Container<'_>, seed: u64, saves: usize) -> Result<Run, Failure> {
    let mut moments = StdRng::seed_from_u64(seed ^ 0x5a7e);
    let mut last = 0;
    let mut at: Vec<u64> = (0..saves)
        .map(|_| {
            last += moments.random_range(1..100);
            last
        })
        .collect();
    let mut turns = 0u64;
    let mut saved = 0;
    let mut cpu = StandIn::new();
    let mut host = TestHost::repeatable(seed);
    let mut kernel = Kernel::new();
    image::add_files(&mut kernel, container.files);
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
    let groups = image::root_groups(container.files);
    let start = Start {
        path: container.path.as_bytes(),
        arguments: &arguments,
        environment: &environment,
        working_directory: container.working_directory.as_bytes(),
        hostname: reference::HOSTNAME.as_bytes(),
        groups: &groups,
    };
    kernel
        .start(&mut cpu, &mut host, &start)
        .map_err(Failure::Start)?;
    let status = loop {
        match kernel.next() {
            Next::Run {
                thread,
                resume,
                slice_nanoseconds,
            } => {
                let exit = cpu
                    .run(thread, resume, slice_nanoseconds)
                    .map_err(Failure::StandIn)?;
                host.ran();

                kernel.exit(&mut cpu, &mut host, exit);
                turns += 1;
                while at.first() == Some(&turns) {
                    at.remove(0);
                    saved += 1;
                    cpu = cpu.restored();
                    kernel.restore(&mut cpu, &mut host);
                }
            }
            Next::Idle { until_nanoseconds } => {
                let Some(until) = until_nanoseconds else {
                    panic!(
                        "the kernel is idle with nothing to wake it, and this host has no events; \
                         output so far:\n{}{}",
                        String::from_utf8_lossy(&host.stdout),
                        String::from_utf8_lossy(&host.stderr)
                    );
                };
                let now = host.monotonic_nanoseconds();
                host.skipped += until.saturating_sub(now);
                kernel.event(&mut cpu, &mut host, Event::Timer);
            }
            Next::Finished { status } => break status,
        }
    };
    Ok(Run {
        outcome: Outcome {
            stdout: host.stdout,
            stderr: host.stderr,
            exit_code: exit_code(status),
        },
        warnings: host.warnings,
        counters: kernel.counters().collect(),
        files: kernel.root_files(&mut cpu),
        saved,
    })
}
