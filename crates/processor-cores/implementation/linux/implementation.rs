//! The logical CPUs Linux lists as online, each held by pinning the calling thread to it alone.

#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::io;
use std::path::PathBuf;

use rustix::io::Errno;
use wcontainers_processor_cores::ProcessorCores;

#[cfg(target_arch = "aarch64")]
mod arm64_identity;
#[cfg(target_arch = "x86_64")]
mod cpuid;
mod placement;
mod system_files;
#[cfg(target_arch = "x86_64")]
mod x86_64_identity;

#[cfg(target_arch = "aarch64")]
use arm64_identity::list_identified_cores;
#[cfg(target_arch = "aarch64")]
use wcontainers_processor_architecture_arm64::CoreIdentity;
#[cfg(target_arch = "x86_64")]
use wcontainers_processor_architecture_x86_64::CoreIdentity;
#[cfg(target_arch = "x86_64")]
use x86_64_identity::list_identified_cores;

/// The online logical CPUs of the machine, each named by the number Linux gives it.
pub struct LinuxProcessorCores;

/// A reason the cores could not be listed.
#[derive(Debug)]
pub enum ListCoresError {
    FileUnreadable {
        path: PathBuf,
        error: io::Error,
    },
    /// A file held text the kernel does not write there.
    FileMalformed {
        path: PathBuf,
        contents: String,
    },
    /// The CPU has no section in /proc/cpuinfo.
    CpuInformationMissing {
        core: usize,
    },
    /// The core and its section of /proc/cpuinfo disagree on the field.
    IdentityDisagrees {
        core: usize,
        field: String,
        from_core: String,
        from_cpu_information: String,
    },
    CoreNotHeld {
        core: usize,
        error: RunOnCoreError,
    },
}

/// A reason the work could not be run on the core.
#[derive(Debug)]
pub enum RunOnCoreError {
    /// The number is at or above CPU_SETSIZE.
    CoreOutOfRange {
        core: usize,
    },
    PlacementUnreadable(Errno),
    /// Linux refused the CPU, as it does one that is offline or outside the thread's cpuset.
    PinRefused(Errno),
    /// The thread was on another CPU before the work, which did not run.
    MovedBeforeWork {
        found: usize,
    },
    /// The thread was on another CPU after the work, whose output is dropped.
    MovedDuringWork {
        found: usize,
    },
    /// The thread's earlier CPUs could not be given back.
    PlacementNotRestored(Errno),
}

impl ProcessorCores for LinuxProcessorCores {
    type Core = usize;
    type CoreIdentity = CoreIdentity;
    type ListCoresError = ListCoresError;
    type RunOnCoreError = RunOnCoreError;

    fn list_cores(&self) -> Result<Vec<(Self::Core, Self::CoreIdentity)>, Self::ListCoresError> {
        list_identified_cores(self)
    }

    fn run_on_core<Output>(
        &self,
        core: &Self::Core,
        work: impl FnOnce() -> Output,
    ) -> Result<Output, Self::RunOnCoreError> {
        placement::run_pinned(*core, work)
    }
}
