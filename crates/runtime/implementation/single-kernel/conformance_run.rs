//! The runtime suite on the local system, with the unicorn stand-in as the executor.
//!
//! Whoever runs it gives the image, the number of runners and the idle poll in the environment.

use std::io::Write;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use wcontainers_executor_unicorn::standin::StandIn;
use wcontainers_kernel::{ExecutionConfig, ReplayDeterminism, ThreadOrdering};
use wcontainers_runtime::Runtime as _;
use wcontainers_runtime_conformance::{Parts, Subject};
use wcontainers_runtime_single_kernel::{ImageSource, LocalSystem, Runtime, RuntimeConfig};

/// The variable naming a plain tar of a root file system that holds `sh`, `cat` and `cmp`.
const IMAGE_VARIABLE: &str = "WCONTAINERS_RUNTIME_SUITE_IMAGE";

/// The variable giving how many runners each container has.
const RUNNERS_VARIABLE: &str = "WCONTAINERS_RUNTIME_SUITE_RUNNERS";

/// The variable giving the longest an idle runner waits, in microseconds.
const IDLE_POLL_VARIABLE: &str = "WCONTAINERS_RUNTIME_SUITE_IDLE_POLL_MICROSECONDS";

type StandInRuntime = Runtime<StandIn, LocalSystem<StandIn>>;

/// Returns the value of the environment variable `name`, which the suite cannot run without.
fn parameter(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("the suite's parameter {name} is set"))
}

/// Returns a system whose standard output goes to `output` and whose standard error is dropped.
fn parts(output: Box<dyn Write + Send>) -> Parts<StandInRuntime> {
    let system = LocalSystem::serving(output, Box::new(std::io::sink()));
    Parts {
        standard_output: system.output_end(),
        standard_error: system.error_end(),
        system,
    }
}

/// A standard output that panics when the container writes to it.
struct PanickingOutput;

impl Write for PanickingOutput {
    fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
        panic!("the failing system's standard output was written to")
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Returns the runtime on the stand-in, made as the environment says.
fn subject() -> Subject<StandInRuntime> {
    let runners: NonZeroUsize = parameter(RUNNERS_VARIABLE)
        .parse()
        .expect("the number of runners is a positive whole number");
    let idle_poll = Duration::from_micros(
        parameter(IDLE_POLL_VARIABLE)
            .parse()
            .expect("the idle poll is a whole number of microseconds"),
    );
    let image = PathBuf::from(parameter(IMAGE_VARIABLE));
    let config = RuntimeConfig {
        make_executor: Arc::new(|_: &()| StandIn::new()),
        // A stand-in runs only on the memories of its own engine, which a restored one shares.
        make_runner_executor: Arc::new(StandIn::restored),
        runners,
        idle_poll,
    };
    Subject {
        runtime: StandInRuntime::new(config),
        runners,
        image: Box::new(move || ImageSource::Tar(image.clone())),
        execution: Box::new(|| ExecutionConfig {
            thread_ordering: ThreadOrdering::Serial,
            replay_determinism: ReplayDeterminism::Exact,
            translator: (),
        }),
        state: Box::new(|| ()),
        parts: Box::new(|| parts(Box::new(std::io::sink()))),
        failing_parts: Box::new(|| parts(Box::new(PanickingOutput))),
    }
}

wcontainers_runtime_conformance::tests!(subject());
