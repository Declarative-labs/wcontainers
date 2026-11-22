//! The kernel test crate's policy (docs/native.md section 6.2, R3): the seeded choices of a test run, built
//! wherever the runtime's loop runs, with the host's services behind any [`Host`].

#![no_std]

extern crate alloc;

mod tcp;
mod udp;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::convert::Infallible;
use core::ops::{ControlFlow, Range};
use core::time::Duration;

use alloc::sync::Arc;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use spin::Mutex;
use wcontainers_abi::{Cpu, Event, Host, HostError, Warning};
use wcontainers_kernel_linux::{Kernel, Standard, TaskId};
use wcontainers_random_source::RandomSource;
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor};
use wcontainers_runtime_single_kernel::{Execution, System, TerminalSize, Turn};
use wcontainers_system::StreamTransport;

/// The services' stream endpoints this policy names for the first process's standard output and
/// error; it gives no input.
pub const OUTPUT: u32 = 1;
pub const ERROR: u32 = 2;

/// Mixed into the run's seed for the schedule's draws, so that they are not the host's random bytes.
pub const SCHEDULE_STREAM: u64 = 0x5eed;

/// The monotonic clock's reading when a repeatable run starts.
pub const REPEATABLE_START_NANOSECONDS: u64 = wcontainers_abi::NANOSECONDS_PER_SECOND;

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
/// What changes as the run goes is behind a lock, as the kernel holds its system by shared reference.
pub struct TestPolicy<H> {
    pub services: H,
    /// When set, every turn ends after this many counted events, in place of the schedule's limit.
    pub turn_events: Option<u64>,
    state: Mutex<PolicyState>,
}

/// What a policy's run changes.
struct PolicyState {
    schedule: Schedule,
    clock: RepeatableClock,
    /// Turns ended so far.
    turns: u64,
    /// The CPU side's slow paths, and what the policy refused.
    warnings: Vec<Warned>,
    refused: u64,
}

impl<H: Host> TestPolicy<H> {
    pub fn new(services: H, seed: u64, wall_start: u64) -> Self {
        Self {
            services,
            turn_events: None,
            state: Mutex::new(PolicyState {
                schedule: Schedule::new(seed),
                clock: RepeatableClock::new(wall_start),
                turns: 0,
                warnings: Vec::new(),
                refused: 0,
            }),
        }
    }

    /// Turns ended so far.
    pub fn turns(&self) -> u64 {
        self.state.lock().turns
    }

    /// The CPU side's slow paths, and what the policy refused, in order.
    pub fn warnings(&self) -> Vec<Warned> {
        self.state.lock().warnings.clone()
    }

    /// The host's services, once the run is over.
    pub fn into_services(self) -> H {
        self.services
    }

    fn refuse(&self, detail: u64) -> Result<(), HostError> {
        let mut state = self.state.lock();
        state.refused += 1;
        let count = state.refused;
        state.warnings.push(Warned {
            reason: OUTSIDE_NOT_CARRIED.to_string(),
            count,
            detail,
        });
        Err(HostError::NotSupported)
    }
}

/// The policy's own seeded clock.
impl<H> wcontainers_clock::Clock for TestPolicy<H> {
    fn monotonic_nanoseconds(&self) -> u64 {
        self.state.lock().clock.monotonic()
    }

    fn realtime_nanoseconds(&self) -> u64 {
        self.state.lock().clock.realtime()
    }
}

/// Randomness is the services' own.
impl<H: RandomSource> RandomSource for TestPolicy<H> {
    fn fill_random_bytes(&self, into: &mut [u8]) -> Result<(), HostError> {
        self.services.fill_random_bytes(into)
    }
}

impl<H: Host> Host for TestPolicy<H> {
    fn warn(&self, warning: Warning<'_>) {
        self.services.warn(warning);
    }
}

/// The standard streams are the services' own.
impl<H: StreamTransport<Endpoint = u32>> StreamTransport for TestPolicy<H> {
    type Endpoint = u32;

    fn send_stream(&self, peer: &u32, bytes: &[u8]) -> Result<usize, HostError> {
        self.services.send_stream(peer, bytes)
    }

    fn receive_stream(&self, peer: &u32, into: &mut [u8]) -> Result<usize, HostError> {
        self.services.receive_stream(peer, into)
    }

    fn close_stream(&self, peer: &u32) {
        self.services.close_stream(peer);
    }
}

impl<H: StreamTransport<Endpoint = u32>> Readiness<u32> for TestPolicy<H> {
    async fn wait_until_ready(
        &self,
        peer: &u32,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        self.services.wait_until_ready(peer, interest).await
    }
}

/// The file backend is refused, as every other request to the outside.
impl<H: Host> wcontainers_system::FuseServer for TestPolicy<H> {
    type Reply = Vec<u8>;
    type Notice = Vec<u8>;

    async fn send_request(&self, request: &[u8]) -> Result<Vec<u8>, HostError> {
        self.refuse(request.len() as u64).map(|()| Vec::new())
    }

    /// Refused, as every other request to the outside.
    async fn receive_notice(
        &self,
        applied: Option<Result<(), wcontainers_kernel::errno::Errno>>,
    ) -> Result<Vec<u8>, HostError> {
        let _ = applied;
        self.refuse(0).map(|()| Vec::new())
    }
}

/// Names are refused, as every other request to the outside.
impl<H: Host> wcontainers_system::DomainNameResolver for TestPolicy<H> {
    type AnswerBytes = &'static [u8];

    async fn resolve_domain(
        &self,
        question: domain::base::Question<&domain::base::Name<[u8]>>,
    ) -> Result<domain::base::Message<Self::AnswerBytes>, HostError> {
        let kind = u64::from(question.qtype().to_int());
        self.refuse(kind)?;
        Err(HostError::NotSupported)
    }
}

/// Every task id is free.
impl<H> wcontainers_system::TaskIdAllocator for TestPolicy<H> {
    fn free_task_id(&self, _id: TaskId) {}
}

impl<H: Host + RandomSource + StreamTransport<Endpoint = u32> + 'static>
    wcontainers_kernel_linux::System for TestPolicy<H>
{
}

impl<H: Host + RandomSource + StreamTransport<Endpoint = u32> + 'static> System for TestPolicy<H> {
    type End = Infallible;

    fn started(self: &Arc<Self>, _kernel: &mut Kernel, _cpu: &mut dyn Cpu) {}

    fn turn(&self, slice_nanoseconds: Option<u64>) -> Turn {
        Turn::Counted {
            limit: self
                .turn_events
                .or_else(|| self.state.lock().schedule.turn_limit(slice_nanoseconds)),
        }
    }

    fn ran(&self, progress: u64) {
        let mut state = self.state.lock();
        let nanoseconds = state.schedule.turn_nanoseconds(progress);
        state.clock.advance(nanoseconds);
    }

    fn arrived(self: &Arc<Self>, _kernel: &mut Kernel, _cpu: &mut dyn Cpu) -> bool {
        self.state.lock().turns += 1;
        false
    }

    fn stopped<E: Execution>(
        self: &Arc<Self>,
        _kernel: &mut Kernel,
        _cpu: &mut E,
    ) -> ControlFlow<Infallible> {
        ControlFlow::Continue(())
    }

    fn idle(
        self: &Arc<Self>,
        kernel: &mut Kernel,
        cpu: &mut dyn Cpu,
        until_nanoseconds: Option<u64>,
    ) {
        let Some(until) = until_nanoseconds else {
            panic!("the kernel is idle with nothing to wake it, and this policy has no events");
        };
        self.state.lock().clock.skip_to(until);
        kernel.event(cpu, self, Event::Timer);
    }

    fn cpu_warning(&self, warning: Warning<'_>) {
        self.state.lock().warnings.push(Warned {
            reason: warning.reason.to_string(),
            count: warning.count,
            detail: warning.detail,
        });
    }

    fn standard(&self, _terminal: Option<TerminalSize>) -> Standard<u32> {
        Standard::Pipes {
            input: None,
            output: OUTPUT,
            error: ERROR,
        }
    }
}
