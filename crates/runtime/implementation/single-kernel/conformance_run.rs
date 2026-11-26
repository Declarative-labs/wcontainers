//! The runtime suite on the local system, with each executor the runtime is tested on.
//!
//! Whoever runs it gives the image, the number of runners and the idle poll in the environment.

use std::io::Write;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;

use wcontainers_executor::Executor;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_kernel::{ExecutionConfig, ReplayDeterminism, ThreadOrdering};
use wcontainers_runtime::Runtime as _;
use wcontainers_runtime_conformance::{Parts, Subject};
use wcontainers_runtime_single_kernel::{
    ImageSource, LocalSystem, MakeExecutor, MakeRunnerExecutor, Runtime, RuntimeConfig,
};

/// The variable naming a plain tar of a root file system that holds `sh`, `cat`, `cmp` and `sleep`.
const IMAGE_VARIABLE: &str = "WCONTAINERS_RUNTIME_SUITE_IMAGE";

/// The variable giving how many runners each container has.
const RUNNERS_VARIABLE: &str = "WCONTAINERS_RUNTIME_SUITE_RUNNERS";

/// The variable giving the longest an idle runner waits, in microseconds.
const IDLE_POLL_VARIABLE: &str = "WCONTAINERS_RUNTIME_SUITE_IDLE_POLL_MICROSECONDS";

/// The runtime on the executor `E` and the local system.
type LocalRuntime<E> = Runtime<E, LocalSystem<E>>;

/// Returns the value of the environment variable `name`, which the suite cannot run without.
fn parameter(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("the suite's parameter {name} is set"))
}

/// Returns a system whose standard output goes to `output` and whose standard error is dropped.
fn parts<E>(output: Box<dyn Write + Send>) -> Parts<LocalRuntime<E>>
where
    E: Executor<
            TranslatorConfig: Clone + Send + Sync + 'static,
            GuestMemory: Send + MemorySpace<PageTable: Send>,
        > + Send
        + 'static,
{
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

/// Returns the runtime on the executors the makers give, its containers translating as `translator` says, made as the environment says.
fn subject<E>(
    make_executor: MakeExecutor<E>,
    make_runner_executor: MakeRunnerExecutor<E>,
    translator: E::TranslatorConfig,
) -> Subject<LocalRuntime<E>>
where
    E: Executor<
            TranslatorConfig: Clone + Send + Sync + 'static,
            GuestMemory: Send + MemorySpace<PageTable: Send>,
        > + Send
        + 'static,
{
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
        make_executor,
        make_runner_executor,
        runners,
        idle_poll,
    };
    Subject {
        runtime: LocalRuntime::new(config),
        runners,
        image: Box::new(move || ImageSource::Tar(image.clone())),
        execution: Box::new(move || ExecutionConfig {
            thread_ordering: ThreadOrdering::Serial,
            replay_determinism: ReplayDeterminism::Exact,
            translator: translator.clone(),
        }),
        state: Box::new(|| ()),
        parts: Box::new(|| parts::<E>(Box::new(std::io::sink()))),
        failing_parts: Box::new(|| parts::<E>(Box::new(PanickingOutput))),
    }
}

/// The suite on the unicorn stand-in.
mod on_the_stand_in {
    use std::sync::Arc;

    use wcontainers_executor_unicorn::standin::StandIn;

    /// Returns the runtime on the stand-in, whose runners' stand-ins share the kernel's engine.
    ///
    /// A stand-in runs only on the memories of its own engine.
    fn subject() -> wcontainers_runtime_conformance::Subject<super::LocalRuntime<StandIn>> {
        super::subject(
            Arc::new(|_: &()| StandIn::new()),
            Arc::new(StandIn::restored),
            (),
        )
    }

    wcontainers_runtime_conformance::tests!(subject());
}
