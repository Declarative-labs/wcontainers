//! The library surface: a runtime makes a container from what to run (the OCI runtime configuration),
//! its root's files, how its translator works (the executor's translator configuration) and what machine it sees, with a system.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use oci_spec::runtime::Spec;
use wcontainers_abi::Cpu;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_kernel::{ExecutionConfig, ReplayDeterminism, StopReason, ThreadOrdering};
use wcontainers_kernel_linux::{
    DOCKER_MOUNTS, DockerMount, Kernel, KernelConfig, KernelWithExecutor, Start, StartOptions,
};

use crate::{Image, KernelContainer, RunInterrupter, StopFlag, System, spec};

/// The start of the variable in `process.env` holding the user's home directory.
const HOME_ASSIGNMENT: &str = "HOME=";

/// How many runners a container's kernel has: one, so that its threads run one at a time on one host thread.
const RUNNERS: NonZeroUsize = NonZeroUsize::MIN;

/// The configuration of a runtime, shared by its containers.
#[derive(Clone, Debug, Default)]
pub struct RuntimeConfig {
    /// Where compiled translations are kept between runs and processes; none keeps them in memory.
    pub cache: Option<PathBuf>,
}

/// What runs a container's guest threads; the runtime makes one for its kernel and one for each runner.
pub trait Executor:
    Cpu
    + wcontainers_executor::Executor<
        TranslatorConfig: Clone + Send + Sync,
        GuestMemory: Send + MemorySpace<PageTable: Send>,
    > + Send
    + 'static
{
    /// What the containers of one runtime share, such as compiled translations.
    type Shared;
    /// The CPU features the guest is told it has.
    type Offer;
    /// The architecture of the guests it runs, as an OCI image configuration names it.
    const ARCHITECTURE: &'static str;

    fn shared(config: &RuntimeConfig) -> Self::Shared;
    fn new(
        shared: &Self::Shared,
        translator_config: &Self::TranslatorConfig,
        offer: &Self::Offer,
    ) -> Self;
    /// What ends a free turn of this executor.
    fn stop_flag(&self) -> StopFlag;
}

/// What machine the container appears to run on: the same wherever it runs, since the guest sees it.
#[derive(Clone, Debug, Default)]
pub struct GuestMachine<Offer> {
    /// The CPU features the guest is told it has.
    pub offer: Offer,
}

/// Why a container was not made.
#[derive(Debug)]
pub enum Error {
    /// A field of the configuration the runtime cannot honour, named as the standard names it, with
    /// its value and what honouring it would take.
    Refused { field: String, reason: String },
    /// The configuration's JSON is not one the runtime specification describes.
    Configuration(String),
    /// The image is not one this runtime reads.
    Image(String),
    /// `process.args[0]` has no slash and is in no directory of the process's `PATH`, which runc
    /// refuses too.
    NotFound(String),
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Refused { field, reason } => write!(f, "{field} is not supported: {reason}"),
            Error::Configuration(what) => write!(f, "the configuration: {what}"),
            Error::Image(what) => write!(f, "the image: {what}"),
            Error::NotFound(name) => write!(
                f,
                "process.args[0] {name:?}: executable file not found in $PATH"
            ),
            Error::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for Error {}

/// Makes containers on one kind of executor.
pub struct Runtime<E: Executor> {
    shared: E::Shared,
}

impl<E: Executor> Runtime<E> {
    pub fn new(config: &RuntimeConfig) -> Self {
        Self {
            shared: E::shared(config),
        }
    }

    /// A container ready to run `spec`'s process on `image`'s files, or the first field of `spec` it
    /// cannot honour.
    pub fn container<S: System>(
        &self,
        spec: &Spec,
        image: &Image,
        translator_config: &E::TranslatorConfig,
        machine: &GuestMachine<E::Offer>,
        system: S,
    ) -> Result<Container<E, S>, Error> {
        let accepted = spec::accept(spec)?;
        if let Some(architecture) = image.architecture()
            && architecture != E::ARCHITECTURE
        {
            return Err(Error::Image(format!(
                "it is for {architecture}; this executor runs {}",
                E::ARCHITECTURE
            )));
        }
        let mut env = accepted.env;
        // As runc: a process without HOME gets its user's home directory.
        if !env
            .iter()
            .any(|variable| variable.starts_with(HOME_ASSIGNMENT))
        {
            env.push(format!("{HOME_ASSIGNMENT}{}", image.home(accepted.uid)));
        }
        let path = image
            .program(&accepted.args[0], &env, &accepted.cwd)
            .ok_or_else(|| Error::NotFound(accepted.args[0].clone()))?;
        let mut kernel = Kernel::new();
        image.add_to(&mut kernel, &accepted.hostname)?;
        if let Some(volume) = &accepted.volume {
            system
                .serve_directory(&volume.source)
                .map_err(|error| match error.kind() {
                    std::io::ErrorKind::Unsupported => Error::Refused {
                        field: spec::mount_field(volume.index),
                        reason: "this system serves no directory of the host".to_string(),
                    },
                    _ => Error::Io {
                        path: volume.source.clone(),
                        error,
                    },
                })?;
        }
        let make_executor = || E::new(&self.shared, translator_config, &machine.offer);
        let executor = make_executor();
        let runner_executors: Vec<E> = (0..RUNNERS.get()).map(|_| make_executor()).collect();
        let stop_flags: Vec<StopFlag> = runner_executors.iter().map(E::stop_flag).collect();
        if let Some(flag) = stop_flags.first() {
            system.stop_flag(flag.clone());
        }
        let system = Arc::new(system);
        let standard = system.standard(accepted.terminal);
        let memory_bytes = Cpu::address_space(&executor).end;
        // Of Docker's default mounts, those the configuration lists (accepted only as Docker gives
        // them); the kernel makes no other.
        let mounts: Vec<DockerMount> = DOCKER_MOUNTS
            .into_iter()
            .filter(|docker| {
                spec.mounts()
                    .iter()
                    .flatten()
                    .any(|listed| listed.destination().as_os_str() == docker.point)
            })
            .collect();
        let arguments: Vec<&'static [u8]> = accepted
            .args
            .into_iter()
            .map(|argument| kept(argument.into_bytes()))
            .collect();
        let environment: Vec<&'static [u8]> = env
            .into_iter()
            .map(|variable| kept(variable.into_bytes()))
            .collect();
        let start = Start {
            path: kept(path.into_bytes()),
            arguments: kept(arguments),
            environment: kept(environment),
            working_directory: kept(accepted.cwd.into_bytes()),
            hostname: kept(accepted.hostname.into_bytes()),
            groups: kept(accepted.groups),
            backend: accepted
                .volume
                .map(|volume| kept(volume.destination.into_bytes())),
            standard,
            address_space_bytes: wcontainers_kernel_linux::ARM64_ADDRESS_SPACE_BYTES,
            memory_bytes,
            ceiling: None,
            options: StartOptions {
                mounts: kept(mounts),
                ..StartOptions::default()
            },
        };
        let execution = ExecutionConfig {
            thread_ordering: ThreadOrdering::Serial,
            replay_determinism: ReplayDeterminism::Exact,
            translator: translator_config.clone(),
        };
        let config = KernelConfig::new(kernel, executor, Arc::clone(&system), start, execution, ());
        let (kernel, runners) =
            <KernelWithExecutor<E, (), S> as wcontainers_kernel::Kernel>::new(config, RUNNERS);
        let mut runner_executors = runner_executors.into_iter();
        let (container, interrupter) = KernelContainer::new(
            kernel,
            runners,
            || {
                runner_executors
                    .next()
                    .expect("an executor was made for each runner")
            },
            move || stop_flags.iter().for_each(StopFlag::raise),
            Arc::clone(&system),
        );
        system.started();
        Ok(Container {
            container,
            interrupter,
        })
    }
}

/// Keeps `values` for as long as the process lasts, since the kernel's configuration holds its start as `'static`.
fn kept<T>(values: Vec<T>) -> &'static [T] {
    values.leak()
}

/// A running container.
pub struct Container<E: Executor, S: System> {
    container: KernelContainer<KernelWithExecutor<E, (), S>, S>,
    interrupter: RunInterrupter,
}

impl<E: Executor, S: System> Container<E, S> {
    /// Runs the kernel's runners until the first process ends, a runner dies, or the run is interrupted.
    pub fn run(&mut self) -> StopReason {
        self.container.run()
    }

    /// Ends a run from any host thread.
    pub fn interrupter(&self) -> RunInterrupter {
        self.interrupter.clone()
    }

    pub fn system(&self) -> &S {
        self.container.system()
    }
}
