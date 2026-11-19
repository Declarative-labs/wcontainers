//! The kernel test crate's policy (docs/native.md section 6.2, R3): the seeded choices of a test run, built
//! wherever the runtime's loop runs, with the host's services behind any [`Host`].

#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::convert::Infallible;
use core::ops::{ControlFlow, Range};
use core::time::Duration;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use wcontainers_abi::{
    BackendRequestId, ConnectionId, Cpu, Event, Host, HostError, Stream, Warning,
};
use wcontainers_kernel::Kernel;
use wcontainers_runtime::{Execution, System, Turn};

/// Mixed into the run's seed for the schedule's draws, so that they are not the host's random bytes.
pub const SCHEDULE_STREAM: u64 = 0x5eed;

/// The monotonic clock's reading when a repeatable run starts.
pub const REPEATABLE_START_NANOSECONDS: u64 = 1_000_000_000;

/// How long one unit of progress takes, drawn per turn (an instruction on the stand-in; a CPU's speed varies
/// with its caches and its load).
pub const PROGRESS_PICOSECONDS: Range<u64> = 500..1500;

/// What a turn costs besides its progress, drawn per turn.
pub const TURN_NANOSECONDS: Range<u64> = 500..1500;

/// A connection, a lookup or a file backend asked of the services while the outside's events cannot reach
/// the policy: refused as unsupported.
pub const OUTSIDE_NOT_CARRIED: &str = "outside's events not carried to the policy";

/// The draws a run's choices come from: how long each turn lasts and when a backend's answers arrive.
pub struct Schedule {
    draws: StdRng,
}

impl Schedule {
    pub fn new(seed: u64) -> Self {
        Self {
            draws: StdRng::seed_from_u64(seed ^ SCHEDULE_STREAM),
        }
    }

    /// The generator itself, for the host's own draws between turns.
    pub fn draws(&mut self) -> &mut StdRng {
        &mut self.draws
    }

    /// Where in its slice a turn ends, so a run repeats for a seed and threads interleave differently under
    /// different seeds; no limit for no slice.
    pub fn turn_limit(&mut self, slice_nanoseconds: Option<u64>) -> Option<u64> {
        slice_nanoseconds.map(|slice| self.draws.random_range(1..=slice.max(1)))
    }

    /// The time one turn took for `progress` units of it.
    pub fn turn_nanoseconds(&mut self, progress: u64) -> u64 {
        let picoseconds = self.draws.random_range(PROGRESS_PICOSECONDS);
        // Picoseconds in a nanosecond: the same thousandfold step as nanoseconds in a microsecond.
        let picoseconds_per_nanosecond = Duration::from_micros(1).as_nanos() as u64;
        progress.saturating_mul(picoseconds) / picoseconds_per_nanosecond
            + self.draws.random_range(TURN_NANOSECONDS)
    }
}

/// Clocks that move only with the work run and the waits skipped, so the kernel's choices repeat for a seed.
#[derive(Clone, Copy, Debug)]
pub struct RepeatableClock {
    now: u64,
    skipped: u64,
    /// The wall clock's reading when the monotonic clock read 0.
    wall_base: u64,
}

impl RepeatableClock {
    /// Clocks with the wall clock reading `wall_start` at the start.
    pub fn new(wall_start: u64) -> Self {
        Self {
            now: REPEATABLE_START_NANOSECONDS,
            skipped: 0,
            wall_base: wall_start - REPEATABLE_START_NANOSECONDS,
        }
    }

    /// The clocks of a saved run brought back: the monotonic clock reads from an earlier start, keeping the
    /// waits skipped, and the wall clock reads `wall`.
    pub fn moved(&self, wall: u64) -> Self {
        let mut clock = Self {
            now: 0,
            skipped: self.skipped,
            wall_base: 0,
        };
        clock.wall_base = wall - clock.monotonic();
        clock
    }

    pub fn monotonic(&self) -> u64 {
        self.now + self.skipped
    }

    pub fn realtime(&self) -> u64 {
        self.wall_base + self.monotonic()
    }

    /// The work of a turn.
    pub fn advance(&mut self, nanoseconds: u64) {
        self.now += nanoseconds;
    }

    /// Moves the clocks forward to a time on the monotonic clock, as if the host had waited.
    pub fn skip_to(&mut self, until: u64) {
        self.skipped += until.saturating_sub(self.monotonic());
    }
}

/// One slow path or refusal, with its count and detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warned {
    pub reason: String,
    pub count: u64,
    pub detail: u64,
}

/// The test crate's policy over the host's services `H`, for a run with no file backend, no network and
/// nothing injected: its clocks are repeatable, its turns come from the seed, and a wait for a time skips to it.
pub struct TestPolicy<H> {
    pub services: H,
    pub schedule: Schedule,
    pub clock: RepeatableClock,
    /// Turns ended so far.
    pub turns: u64,
    /// The CPU side's slow paths, and what the policy refused.
    pub warnings: Vec<Warned>,
    refused: u64,
}

impl<H: Host> TestPolicy<H> {
    pub fn new(services: H, seed: u64, wall_start: u64) -> Self {
        Self {
            services,
            schedule: Schedule::new(seed),
            clock: RepeatableClock::new(wall_start),
            turns: 0,
            warnings: Vec::new(),
            refused: 0,
        }
    }

    fn refuse(&mut self, detail: u64) -> Result<(), HostError> {
        self.refused += 1;
        self.warnings.push(Warned {
            reason: OUTSIDE_NOT_CARRIED.to_string(),
            count: self.refused,
            detail,
        });
        Err(HostError::NotSupported)
    }
}

impl<H: Host> Host for TestPolicy<H> {
    fn monotonic_nanoseconds(&mut self) -> u64 {
        self.clock.monotonic()
    }

    fn realtime_nanoseconds(&mut self) -> u64 {
        self.clock.realtime()
    }

    fn random(&mut self, into: &mut [u8]) {
        self.services.random(into);
    }

    fn warn(&mut self, warning: Warning<'_>) {
        self.services.warn(warning);
    }

    fn backend_send(
        &mut self,
        request: BackendRequestId,
        _message: &[u8],
    ) -> Result<(), HostError> {
        self.refuse(request.0)
    }

    fn output(&mut self, stream: Stream, bytes: &[u8]) {
        self.services.output(stream, bytes);
    }

    fn connect(
        &mut self,
        connection: ConnectionId,
        _address: &[u8],
        _port: u16,
        _stream: bool,
    ) -> Result<(), HostError> {
        self.refuse(connection.0.into())
    }

    fn send(&mut self, connection: ConnectionId, bytes: &[u8]) -> Result<usize, HostError> {
        self.services.send(connection, bytes)
    }

    fn receive(&mut self, connection: ConnectionId, into: &mut [u8]) -> Result<usize, HostError> {
        self.services.receive(connection, into)
    }

    fn finish(&mut self, connection: ConnectionId) {
        self.services.finish(connection);
    }

    fn close(&mut self, connection: ConnectionId) {
        self.services.close(connection);
    }

    fn look_up(&mut self, request: u64, _name: &str) -> Result<(), HostError> {
        self.refuse(request)
    }

    fn listen(&mut self, listener: ConnectionId, _port: u16) -> Result<(), HostError> {
        self.refuse(listener.0.into())
    }

    fn accept(
        &mut self,
        listener: ConnectionId,
        connection: ConnectionId,
    ) -> Result<(), HostError> {
        self.services.accept(listener, connection)
    }
}

impl<H: Host> System for TestPolicy<H> {
    type End = Infallible;

    fn turn(&mut self, slice_nanoseconds: Option<u64>) -> Turn {
        Turn::Counted {
            limit: self.schedule.turn_limit(slice_nanoseconds),
        }
    }

    fn ran(&mut self, progress: u64) {
        let nanoseconds = self.schedule.turn_nanoseconds(progress);
        self.clock.advance(nanoseconds);
    }

    fn arrived(&mut self, _kernel: &mut Kernel, _cpu: &mut dyn Cpu) -> bool {
        self.turns += 1;
        false
    }

    fn stopped<E: Execution>(
        &mut self,
        _kernel: &mut Kernel,
        _cpu: &mut E,
    ) -> ControlFlow<Infallible> {
        ControlFlow::Continue(())
    }

    fn idle(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu, until_nanoseconds: Option<u64>) {
        let Some(until) = until_nanoseconds else {
            panic!("the kernel is idle with nothing to wake it, and this policy has no events");
        };
        self.clock.skip_to(until);
        kernel.event(cpu, self, Event::Timer);
    }

    fn cpu_warning(&mut self, warning: Warning<'_>) {
        self.warnings.push(Warned {
            reason: warning.reason.to_string(),
            count: warning.count,
            detail: warning.detail,
        });
    }
}
