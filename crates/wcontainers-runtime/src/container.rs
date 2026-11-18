//! The library surface: a runtime makes a container from what to run (the OCI runtime configuration),
//! its root's files, how it runs (the back end's tuning) and what machine it sees, with a system.

use std::path::PathBuf;

use oci_spec::runtime::Spec;
use wcontainers_kernel::{Errno, Kernel, Start};

use crate::{Ending, Execution, Image, Step, Stepper, StopFlag, System, spec};

/// Settings of a runtime, shared by its containers.
#[derive(Clone, Debug, Default)]
pub struct RuntimeSettings {
    /// Where compiled translations are kept between runs and processes; none keeps them in memory.
    pub cache: Option<PathBuf>,
}

/// A back end the runtime makes for each container.
pub trait BackEnd: Execution {
    /// What the containers of one runtime share, such as compiled translations.
    type Shared;
    /// How the back end runs a container: it changes speed, never behaviour.
    type Tuning;
    /// The CPU features the guest is told it has.
    type Offer;
    /// The architecture of the guests it runs, as an OCI image configuration names it.
    const ARCHITECTURE: &'static str;

    fn shared(settings: &RuntimeSettings) -> Self::Shared;
    fn new(shared: &Self::Shared, tuning: &Self::Tuning, offer: &Self::Offer) -> Self;
    /// What ends a free turn of this back end.
    fn stop_flag(&self) -> StopFlag;
}

/// What machine the container appears to run on: the same wherever it runs, since the guest sees it.
#[derive(Clone, Debug, Default)]
pub struct Machine<Offer> {
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
pub struct Runtime<B: BackEnd> {
    shared: B::Shared,
}

impl<B: BackEnd> Runtime<B> {
    pub fn new(settings: &RuntimeSettings) -> Self {
        Self {
            shared: B::shared(settings),
        }
    }

    /// A container ready to run `spec`'s process on `image`'s files, or the first field of `spec` it
    /// cannot honour.
    pub fn container<S: System>(
        &self,
        spec: &Spec,
        image: &Image,
        tuning: &B::Tuning,
        machine: &Machine<B::Offer>,
        mut system: S,
    ) -> Result<Container<B, S>, Error> {
        let accepted = spec::accept(spec)?;
        if let Some(architecture) = image.architecture()
            && architecture != B::ARCHITECTURE
        {
            return Err(Error::Image(format!(
                "it is for {architecture}; this back end runs {}",
                B::ARCHITECTURE
            )));
        }
        let mut env = accepted.env;
        // As runc: a process without HOME gets its user's home directory.
        if !env.iter().any(|variable| variable.starts_with("HOME=")) {
            env.push(format!("HOME={}", image.home(accepted.uid)));
        }
        let path = image
            .program(&accepted.args[0], &env, &accepted.cwd)
            .ok_or_else(|| Error::NotFound(accepted.args[0].clone()))?;
        let mut kernel = Kernel::new();
        image.add_to(&mut kernel, &accepted.hostname)?;
        let mut cpu = B::new(&self.shared, tuning, &machine.offer);
        system.stop_flag(cpu.stop_flag());
        let arguments: Vec<&[u8]> = accepted.args.iter().map(|a| a.as_bytes()).collect();
        let environment: Vec<&[u8]> = env.iter().map(|v| v.as_bytes()).collect();
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
                backend: None,
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
pub struct Container<B: BackEnd, S: System> {
    kernel: Kernel,
    cpu: B,
    system: S,
    stepper: Stepper,
}

impl<B: BackEnd, S: System> Container<B, S> {
    /// One step, for a caller with its own event loop: after [`Step::Idle`] the caller waits as it
    /// likes, or calls [`Container::idle`].
    pub fn step(&mut self) -> Result<Step<S::End>, B::Failure> {
        self.stepper
            .step(&mut self.kernel, &mut self.cpu, &mut self.system)
    }

    /// Waits in the system until the time an idle step gave, or for an event.
    pub fn idle(&mut self, until_nanoseconds: Option<u64>) {
        self.system
            .idle(&mut self.kernel, &mut self.cpu, until_nanoseconds);
    }

    /// Runs to the end: the first process's wait status, or what the system ended it with.
    pub fn run(&mut self) -> Result<Ending<S::End>, B::Failure> {
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

    pub fn back_end(&self) -> &B {
        &self.cpu
    }
}
