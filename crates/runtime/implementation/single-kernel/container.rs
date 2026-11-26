//! The library surface: a runtime makes a container from an OCI configuration, an image, the standard endpoints and a system.
//!
//! What the executors are and how many runners a container has are the runtime's configuration.

use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use wcontainers_abi::Cpu;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_kernel::errno::Errno;
use wcontainers_kernel::{ReplayDeterminism, StopReason, ThreadOrdering};
use wcontainers_kernel_linux::{
    DOCKER_MOUNTS, DockerMount, Kernel, KernelConfig, KernelWithExecutor, Standard, Start,
    StartOptions,
};
use wcontainers_runtime::{ContainerConfig, ContainerError, Refusal};
use wcontainers_system::ProcessTracer;

use crate::interruptible::Interruptible;
use crate::runners::RunnerStops;
use crate::{Image, KernelContainer, RunInterrupter, System, spec};

/// The start of the variable in `process.env` holding the user's home directory.
const HOME_ASSIGNMENT: &str = "HOME=";

/// Why a bind mount is refused on a system with no file backend.
const NO_DIRECTORY_SERVED: &str = "this system serves no directory of the host";

/// The field that asks for a terminal, and why one is refused when the endpoints differ.
const TERMINAL_FIELD: &str = "process.terminal";
const ONE_TERMINAL_END: &str =
    "a terminal is one endpoint, so standard output, error and any input must name the same one";

/// The architecture of the only guests the kernel runs, as an [OCI image configuration](https://github.com/opencontainers/image-spec/blob/v1.1.1/config.md#properties) names it.
pub const GUEST_ARCHITECTURE: &str = "arm64";

/// The kernel a container on the executor `E` and the system `S` runs on.
pub type ContainerKernel<E, S> =
    KernelWithExecutor<Interruptible<E>, <S as ProcessTracer>::ProcessState, S>;

/// Makes a kernel's executor from the translator's configuration.
pub type MakeExecutor<E> =
    Arc<dyn Fn(&<E as wcontainers_executor::Executor>::TranslatorConfig) -> E + Send + Sync>;

/// Makes a runner's executor, which runs on the memories the kernel's executor given makes.
pub type MakeRunnerExecutor<E> = Arc<dyn Fn(&E) -> E + Send + Sync>;

/// The configuration of a runtime, shared by its containers.
pub struct RuntimeConfig<E: wcontainers_executor::Executor> {
    /// Makes the executor of a container's kernel, whose memories its runners run on.
    pub make_executor: MakeExecutor<E>,
    /// Makes the executor of each runner from the kernel's.
    pub make_runner_executor: MakeRunnerExecutor<E>,
    /// How many runners a container's kernel has, each on a host thread of its own.
    pub runners: NonZeroUsize,
    /// The longest an idle runner waits before it is run again, as nothing tells the runtime that a wait of the kernel's ended.
    pub idle_poll: Duration,
}

impl<E: wcontainers_executor::Executor> Clone for RuntimeConfig<E> {
    fn clone(&self) -> Self {
        Self {
            make_executor: Arc::clone(&self.make_executor),
            make_runner_executor: Arc::clone(&self.make_runner_executor),
            runners: self.runners,
            idle_poll: self.idle_poll,
        }
    }
}

/// Where an image is read from.
#[derive(Clone, Debug)]
pub enum ImageSource {
    /// A plain tar of a root file system, with no configuration.
    Tar(PathBuf),
    /// A directory in the OCI image layout, of which the image for [`GUEST_ARCHITECTURE`] is read.
    Oci(PathBuf),
}

/// Why a container or an image was not made.
#[derive(Debug)]
pub enum Error {
    /// A field of the configuration the runtime cannot honour, named as the standard names it, with its value and what it would take.
    Refused { field: String, reason: String },
    /// The configuration's JSON is not one the runtime specification describes.
    Configuration(String),
    /// The image is not one this runtime reads.
    Image(String),
    /// A `process.args[0]` with no slash that is in no directory of the process's `PATH`, which runc refuses too.
    NotFound(String),
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
    /// A file of the image the kernel refused to add to the container's root file system.
    FileRefused { path: PathBuf, error: Errno },
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
            Error::FileRefused { path, error } => {
                write!(f, "{}: the kernel refused it ({error:?})", path.display())
            }
        }
    }
}

impl std::error::Error for Error {}

/// Returns the runtime interface's error for `error`; one with no field of its own names the whole configuration.
fn to_container_error(error: Error) -> ContainerError {
    let refused =
        |field: String, reason: String| ContainerError::Refused(Refusal { field, reason });
    match error {
        Error::Refused { field, reason } => refused(field, reason),
        Error::NotFound(_) => ContainerError::ProgramNotFound,
        Error::FileRefused { path, error } => ContainerError::FileRefused { path, error },
        other @ (Error::Configuration(_) | Error::Image(_) | Error::Io { .. }) => {
            refused(String::new(), other.to_string())
        }
    }
}

/// A maker of containers on the executor `E` for the system `S`.
pub struct Runtime<E: wcontainers_executor::Executor, S> {
    config: RuntimeConfig<E>,
    system: PhantomData<fn() -> S>,
}

impl<E, S> wcontainers_runtime::Runtime for Runtime<E, S>
where
    E: Cpu
        + wcontainers_executor::Executor<
            TranslatorConfig: Send + Sync,
            GuestMemory: Send + MemorySpace<PageTable: Send>,
        > + Send
        + 'static,
    S: wcontainers_system::System<Kernel = ContainerKernel<E, S>> + System,
    <S as ProcessTracer>::ProcessState: 'static,
{
    type System = S;
    type Image = Image;
    type ImageSource = ImageSource;
    type ImageError = Error;
    type Container = Container<E, S>;
    type Interrupter = RunInterrupter;
    type Config = RuntimeConfig<E>;

    fn new(config: RuntimeConfig<E>) -> Self {
        Self {
            config,
            system: PhantomData,
        }
    }

    fn open_image(&self, source: ImageSource) -> Result<Image, Error> {
        match source {
            ImageSource::Tar(path) => Image::tar(&path),
            ImageSource::Oci(directory) => Image::oci(&directory, GUEST_ARCHITECTURE),
        }
    }

    /// Returns a container ready to run the configuration's process on the image's files, or the first field it cannot honour.
    fn make_container(
        &self,
        config: ContainerConfig<'_, Self>,
    ) -> Result<(Container<E, S>, RunInterrupter), ContainerError> {
        let ContainerConfig {
            spec,
            image,
            execution,
            state,
            standard_input,
            standard_output,
            standard_error,
            system,
        } = config;
        if execution.thread_ordering != ThreadOrdering::Serial {
            return Err(ContainerError::ThreadOrderingUnsupported(
                execution.thread_ordering,
            ));
        }
        if execution.replay_determinism != ReplayDeterminism::Exact {
            return Err(ContainerError::ReplayDeterminismUnsupported(
                execution.replay_determinism,
            ));
        }
        let accepted = spec::accept(spec).map_err(to_container_error)?;
        if let Some(architecture) = image.architecture()
            && architecture != GUEST_ARCHITECTURE
        {
            return Err(to_container_error(Error::Image(format!(
                "it is for {architecture}; the kernel runs {GUEST_ARCHITECTURE}"
            ))));
        }
        let standard = match accepted.terminal {
            Some(size) => {
                if standard_error != standard_output
                    || standard_input
                        .as_ref()
                        .is_some_and(|input| *input != standard_output)
                {
                    return Err(ContainerError::Refused(Refusal {
                        field: TERMINAL_FIELD.to_string(),
                        reason: ONE_TERMINAL_END.to_string(),
                    }));
                }
                Standard::Terminal {
                    end: standard_output,
                    columns: size.columns,
                    rows: size.rows,
                }
            }
            None => Standard::Pipes {
                input: standard_input,
                output: standard_output,
                error: standard_error,
            },
        };
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
            .ok_or(ContainerError::ProgramNotFound)?;
        let mut kernel = Kernel::new();
        image
            .add_to(&mut kernel, &accepted.hostname)
            .map_err(to_container_error)?;
        if let Some(volume) = &accepted.volume {
            system.serve_directory(&volume.source).map_err(|error| {
                let reason = match error.kind() {
                    std::io::ErrorKind::Unsupported => NO_DIRECTORY_SERVED.to_string(),
                    _ => format!("{}: {error}", volume.source.display()),
                };
                ContainerError::Refused(Refusal {
                    field: spec::mount_field(volume.index),
                    reason,
                })
            })?;
        }
        let kernel_executor = (self.config.make_executor)(&execution.translator);
        let runner_executors: Vec<Interruptible<E>> = (0..self.config.runners.get())
            .map(|_| Interruptible::new((self.config.make_runner_executor)(&kernel_executor)))
            .collect();
        let executor = Interruptible::new(kernel_executor);
        let slots: Arc<[_]> = runner_executors.iter().map(Interruptible::slot).collect();
        let lowered = Arc::clone(&slots);
        let stops = RunnerStops {
            idle_poll: self.config.idle_poll,
            stop: Box::new(move || slots.iter().for_each(|slot| slot.raise())),
            resume: Box::new(move || lowered.iter().for_each(|slot| slot.lower())),
        };
        let system = Arc::new(system);
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
        let config = KernelConfig::new(
            kernel,
            executor,
            Arc::clone(&system),
            start,
            execution,
            state,
        );
        let (kernel, runners) =
            <ContainerKernel<E, S> as wcontainers_kernel::Kernel>::new(config, self.config.runners);
        let (container, interrupter) = KernelContainer::new(
            kernel,
            runners,
            runner_executors,
            stops,
            Arc::clone(&system),
        );
        system.on_container_made();
        Ok((Container { container }, interrupter))
    }
}

/// Keeps `values` for as long as the process lasts, since the kernel's configuration holds its start as `'static`.
fn kept<T>(values: Vec<T>) -> &'static [T] {
    values.leak()
}

/// A container, run by its kernel's runners.
pub struct Container<E, S>
where
    E: Cpu
        + wcontainers_executor::Executor<
            TranslatorConfig: Send + Sync,
            GuestMemory: Send + MemorySpace<PageTable: Send>,
        > + Send
        + 'static,
    S: wcontainers_system::System<Kernel = ContainerKernel<E, S>> + System,
    <S as ProcessTracer>::ProcessState: 'static,
{
    container: KernelContainer<ContainerKernel<E, S>, S>,
}

impl<E, S> Container<E, S>
where
    E: Cpu
        + wcontainers_executor::Executor<
            TranslatorConfig: Send + Sync,
            GuestMemory: Send + MemorySpace<PageTable: Send>,
        > + Send
        + 'static,
    S: wcontainers_system::System<Kernel = ContainerKernel<E, S>> + System,
    <S as ProcessTracer>::ProcessState: 'static,
{
    /// Returns the system the container was made with.
    pub fn system(&self) -> &S {
        self.container.system()
    }
}

impl<E, S> wcontainers_runtime::Container for Container<E, S>
where
    E: Cpu
        + wcontainers_executor::Executor<
            TranslatorConfig: Send + Sync,
            GuestMemory: Send + MemorySpace<PageTable: Send>,
        > + Send
        + 'static,
    S: wcontainers_system::System<Kernel = ContainerKernel<E, S>> + System,
    <S as ProcessTracer>::ProcessState: 'static,
{
    /// Runs the kernel's runners until the first process ends, a runner dies, or the run is interrupted.
    fn run(&mut self) -> StopReason {
        self.container.run()
    }
}
