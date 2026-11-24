//! A container's first process run in the kernel's loop with the seeded test policy, on any executor of the loop.

use core::convert::Infallible;
use std::sync::{Arc, Mutex, PoisonError};

use wcontainers_abi::{Host, HostError, Warning};
use wcontainers_byte_source::ByteSource;
use wcontainers_kernel_linux::{ARM64_ADDRESS_SPACE_BYTES, FileRecord, Start, StartOptions};
use wcontainers_runtime_single_kernel::{Ending, Execution, Kernel, System, drive};
use wcontainers_system_seeded_test_policy::{ERROR, TestPolicy};
use wcontainers_volatile_bytes::VolatileBytes;

/// The clocks the test host reads; the wall clock's is also a run's start where the guest is not shown the machine's.
const MONOTONIC_NANOSECONDS: u64 = 1_234_567;
pub const REALTIME_NANOSECONDS: u64 = 1_763_500_000_000_000_000;

/// Why an output end's bytes can always be kept.
const OUTPUT_KEPT: &str = "the test's output fits in memory";

/// What the host was asked to write to each output end.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// A host with fixed clocks and seeded random bytes that keeps each output end's bytes, with no network or backend.
struct Recording {
    output: Mutex<Output>,
    random: Mutex<fastrand::Rng>,
}

impl wcontainers_clock::Clock for Recording {
    fn monotonic_nanoseconds(&self) -> u64 {
        MONOTONIC_NANOSECONDS
    }
    fn realtime_nanoseconds(&self) -> u64 {
        REALTIME_NANOSECONDS
    }
}

/// The seeded random bytes, which fill whatever they are read into.
struct Seeded<'random>(&'random mut fastrand::Rng);

impl ByteSource for Seeded<'_> {
    type Error = Infallible;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, Infallible> {
        self.0.fill(into);
        Ok(into.len())
    }
}

impl wcontainers_random_source::RandomSource for Recording {
    fn fill_random_bytes(&self, into: &impl VolatileBytes) -> Result<(), HostError> {
        let mut random = self.random.lock().unwrap_or_else(PoisonError::into_inner);
        let Ok(_) = into.write_from(&mut Seeded(&mut random));
        Ok(())
    }
}

impl Host for Recording {
    fn warn(&self, _warning: Warning<'_>) {}
}

impl wcontainers_system::StreamTransport for Recording {
    type Endpoint = u32;

    fn send_stream(&self, peer: &u32, payload: &impl VolatileBytes) -> Result<usize, HostError> {
        let mut output = self.output.lock().unwrap_or_else(PoisonError::into_inner);
        let end = match *peer {
            ERROR => &mut output.stderr,
            _ => &mut output.stdout,
        };
        Ok(payload.read_to(end).expect(OUTPUT_KEPT))
    }
    fn receive_stream(&self, _peer: &u32, _into: &impl VolatileBytes) -> Result<usize, HostError> {
        Err(HostError::NotSupported)
    }
    fn close_stream(&self, _peer: &u32) {}
}

impl wcontainers_readiness::Readiness<u32> for Recording {
    async fn wait_until_ready(
        &self,
        _peer: &u32,
        interest: wcontainers_readiness::ReadyFor,
    ) -> Result<wcontainers_readiness::ReadinessReport, HostError> {
        if interest.contains(wcontainers_readiness::ReadyFor::WRITABLE) {
            return Ok(wcontainers_readiness::ReadinessReport {
                ready_for: wcontainers_readiness::ReadyFor::WRITABLE,
                unconditional: wcontainers_readiness::UnconditionalEvents::empty(),
            });
        }
        std::future::pending().await
    }
}

/// The first process of a container: what it runs and the world it starts in.
pub struct Program<'a> {
    pub path: &'a [u8],
    pub arguments: &'a [&'a [u8]],
    pub environment: &'a [&'a [u8]],
    pub hostname: &'a [u8],
    pub groups: &'a [u32],
    /// The container's ceiling and the memory size it is told, the same for every executor compared; where none, the
    /// executor's own ceiling is both.
    pub limit: Option<u64>,
}

/// How a container's run ended: its first process's wait status, its output, its root's files and the policy's turns.
pub struct Ran {
    pub status: i32,
    pub output: Output,
    pub files: Vec<FileRecord>,
    pub turns: u64,
}

/// Runs `program` in `kernel`, which holds the container's files, on `cpu` at the root, until its first process ends.
pub fn run_container<E: Execution<Failure = core::convert::Infallible>>(
    cpu: &mut E,
    mut kernel: Kernel,
    seed: u64,
    wall_start: u64,
    program: &Program<'_>,
) -> Ran {
    let recording = Recording {
        output: Mutex::default(),
        random: Mutex::new(fastrand::Rng::with_seed(seed)),
    };
    let policy = Arc::new(TestPolicy::new(recording, seed, wall_start));
    let memory_bytes = program.limit.unwrap_or(cpu.address_space().end);
    let start = Start {
        path: program.path,
        arguments: program.arguments,
        environment: program.environment,
        working_directory: b"/",
        hostname: program.hostname,
        groups: program.groups,
        backend: None,
        standard: policy.standard(None),
        address_space_bytes: ARM64_ADDRESS_SPACE_BYTES,
        memory_bytes,
        ceiling: program.limit,
        options: StartOptions::default(),
    };
    kernel.start(cpu, &policy, start);
    policy.started(&mut kernel, cpu);
    let status = match drive(&mut kernel, cpu, &policy) {
        Ok(Ending::Finished { status }) => status,
        Ok(Ending::Ended(never)) => match never {},
        Err(never) => match never {},
    };
    let files = kernel.root_files(cpu);
    drop(kernel);
    let policy = Arc::into_inner(policy).expect("the kernel keeps no call on the policy");
    let turns = policy.turns();
    let output = policy
        .into_services()
        .output
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner);
    Ran {
        status,
        output,
        files,
        turns,
    }
}
