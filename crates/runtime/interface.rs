use std::path::PathBuf;

use oci_spec::runtime::Spec;
use wcontainers_kernel::errno::Errno;
use wcontainers_kernel::{ExecutionConfig, Kernel, ReplayDeterminism, StopReason, ThreadOrdering};
use wcontainers_system::{ProcessTracer, StreamTransport, System};

/// A maker of containers that run on one kind of executor for one kind of system.
pub trait Runtime {
    type System: System;
    type Image: Image;
    type ImageSource;
    type ImageError;
    type Container: Container;
    type Interrupter: Interrupter + 'static;

    fn open_image(&self, source: Self::ImageSource) -> Result<Self::Image, Self::ImageError>;
    fn make_container(
        &self,
        config: ContainerConfig<'_, Self>,
    ) -> Result<(Self::Container, Self::Interrupter), ContainerError>;
}

pub trait Image {
    /// Returns the runtime configuration that the image's own configuration [converts](https://github.com/opencontainers/image-spec/blob/main/conversion.md) to.
    fn to_runtime_spec(&self) -> Result<Spec, Refusal>;
}

/// A field the runtime cannot honour, with what honouring it would take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The field's path in the configuration's JSON, such as `mounts[2].options`.
    pub field: String,
    pub reason: String,
}

pub trait Container {
    fn run(&mut self) -> StopReason;
}

/// A handle that interrupts a container's run, idempotently.
pub trait Interrupter: Send + Sync {
    fn interrupt(&self);
}

pub struct ContainerConfig<'a, R: Runtime + ?Sized> {
    pub spec: &'a Spec,
    pub image: &'a R::Image,
    pub execution: ExecutionConfig<ExecutorOf<R>>,
    pub state: <R::System as ProcessTracer>::ProcessState,
    pub standard_input: Option<EndpointOf<R>>,
    pub standard_output: EndpointOf<R>,
    pub standard_error: EndpointOf<R>,
    pub system: R::System,
}

pub type ExecutorOf<R> = <<<R as Runtime>::System as ProcessTracer>::Kernel as Kernel>::Executor;

pub type EndpointOf<R> = <<R as Runtime>::System as StreamTransport>::Endpoint;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContainerError {
    Refused(Refusal),
    /// No directory of the process's `PATH` holds `process.args[0]`, which names no path.
    ProgramNotFound,
    ThreadOrderingUnsupported(ThreadOrdering),
    ReplayDeterminismUnsupported(ReplayDeterminism),
    /// A file of the image that could not be added to the container's root file system.
    FileRefused {
        path: PathBuf,
        error: Errno,
    },
}

/// A runtime built from a configuration given once.
pub trait StandaloneRuntime: Runtime {
    type Config;

    fn new(config: Self::Config) -> Self;
}
