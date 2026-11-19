//! The library surface: a runtime makes a container from what to run (the OCI runtime configuration),
//! its root's files, how its translator works (the executor's translator configuration) and what machine it sees, with a system.

use std::path::PathBuf;

use oci_spec::runtime::Spec;
use wcontainers_kernel::{Errno, Kernel, Start};

use crate::{Ending, Execution, Image, Step, Stepper, StopFlag, System, spec};

/// The start of the variable in `process.env` holding the user's home directory.
const HOME_ASSIGNMENT: &str = "HOME=";

/// The configuration of a runtime, shared by its containers.
#[derive(Clone, Debug, Default)]
pub struct RuntimeConfig {
    /// Where compiled translations are kept between runs and processes; none keeps them in memory.
    pub cache: Option<PathBuf>,
}

/// An executor: the back end the runtime makes for each container.
pub trait Executor: Execution {
    /// What the containers of one runtime share, such as compiled translations.
    type Shared;
    /// How the translator works for a container: it changes speed, never behaviour.
    type TranslatorConfig;
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
    /// What ends a free turn of this back end.
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

/// Makes containers on one kind of back end.
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
        mut system: S,
    ) -> Result<Container<E, S>, Error> {
        let accepted = spec::accept(spec)?;
        if let Some(architecture) = image.architecture()
            && architecture != E::ARCHITECTURE
        {
            return Err(Error::Image(format!(
                "it is for {architecture}; this back end runs {}",
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
        let mut cpu = E::new(&self.shared, translator_config, &machine.offer);
        system.stop_flag(cpu.stop_flag());
        let arguments: Vec<&[u8]> = accepted.args.iter().map(|a| a.as_bytes()).collect();
        let environment: Vec<&[u8]> = env.iter().map(|v| v.as_bytes()).collect();
        let standard = system.standard(accepted.terminal);
        kernel.start(
            &mut cpu,
            &mut system,
            &Start {
                path: path.as_bytes(),
                arguments: &arguments,
                environment: &environment,
                working_directory: accepted.cwd.as_bytes(),
                hostname: accepted.hostname.as_bytes(),
                groups: &accepted.groups,
                backend: accepted.volume.as_ref().map(|v| v.destination.as_bytes()),
                standard,
            },
        );
        Ok(Container {
            kernel,
            cpu,
            system,
            stepper: Stepper::default(),
        })
    }
}

/// A running container.
pub struct Container<E: Executor, S: System> {
    kernel: Kernel,
    cpu: E,
    system: S,
    stepper: Stepper,
}

impl<E: Executor, S: System> Container<E, S> {
    /// One step, for a caller with its own event loop: after [`Step::Idle`] the caller waits as it
    /// likes, or calls [`Container::idle`].
    pub fn step(&mut self) -> Result<Step<S::End>, E::Failure> {
        self.stepper
            .step(&mut self.kernel, &mut self.cpu, &mut self.system)
    }

    /// Waits in the system until the time an idle step gave, or for an event.
    pub fn idle(&mut self, until_nanoseconds: Option<u64>) {
        self.system
            .idle(&mut self.kernel, &mut self.cpu, until_nanoseconds);
    }

    /// Runs to the end: the first process's wait status, or what the system ended it with.
    pub fn run(&mut self) -> Result<Ending<S::End>, E::Failure> {
        self.stepper
            .drive(&mut self.kernel, &mut self.cpu, &mut self.system)
    }

    /// Why the first program could not be started, once the container has finished.
    pub fn start_failure(&self) -> Option<Errno> {
        self.kernel.start_failure()
    }

    pub fn system(&self) -> &S {
        &self.system
    }

    pub fn system_mut(&mut self) -> &mut S {
        &mut self.system
    }

    pub fn back_end(&self) -> &E {
        &self.cpu
    }
}
