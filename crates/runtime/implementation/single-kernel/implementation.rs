//! The runtime: a container from an OCI configuration and an image, run by its kernel's runners on host threads.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
#[cfg(feature = "std")]
use std::sync::{Mutex, PoisonError};

extern crate alloc;
use alloc::sync::Arc;

use wcontainers_abi::{Cpu, Exit, Resume, ThreadId, Warning};

/// The value of a raised flag word: true, as a number.
#[cfg(feature = "std")]
const RAISED: u32 = true as u32;
pub use wcontainers_kernel_linux::Kernel;

#[cfg(feature = "std")]
mod arrivals;
#[cfg(feature = "std")]
mod container;
#[cfg(feature = "std")]
mod described;
#[cfg(feature = "std")]
mod errno;
#[cfg(feature = "std")]
mod files;
#[cfg(feature = "std")]
mod host_call;
#[cfg(feature = "std")]
pub mod image;
#[cfg(feature = "std")]
mod interruptible;
#[cfg(feature = "std")]
pub mod names;
#[cfg(feature = "std")]
mod net;
#[cfg(feature = "std")]
mod runners;
#[cfg(feature = "std")]
mod spec;
#[cfg(feature = "std")]
mod system;
#[cfg(feature = "std")]
mod tcp;
#[cfg(feature = "std")]
mod timer;
#[cfg(feature = "std")]
pub mod udp;

#[cfg(feature = "std")]
pub use container::{
    Container, ContainerKernel, Error, GUEST_ARCHITECTURE, ImageSource, MakeExecutor,
    MakeRunnerExecutor, Runtime, RuntimeConfig,
};
#[cfg(feature = "std")]
pub use image::Image;
#[cfg(feature = "std")]
pub use interruptible::{Interruptible, SharedInterrupter};
#[cfg(feature = "std")]
pub use oci_spec;
#[cfg(feature = "std")]
use runners::KernelContainer;
#[cfg(feature = "std")]
pub use runners::RunInterrupter;
#[cfg(feature = "std")]
pub use spec::{MASKED_PATHS_FIELD, READONLY_PATHS_FIELD, base_spec, bind_mount, spec_from_json};
#[cfg(feature = "std")]
pub use system::{LocalSystem, Warned};
#[cfg(feature = "std")]
pub use timer::SliceTimer;

/// A terminal's size in characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalSize {
    pub columns: u16,
    pub rows: u16,
}

/// How a turn ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// A turn ended after `limit` units of progress, or never.
    Counted { limit: Option<u64> },
    /// A turn ended at the next stop check after a raise of the [`StopFlag`].
    Free,
}

/// A raiser of the running thread's stop flag, or the next one's, from any host thread.
#[cfg(feature = "std")]
#[derive(Clone, Default)]
pub struct StopFlag(Arc<Raising>);

#[cfg(feature = "std")]
#[derive(Default)]
struct Raising {
    raised: AtomicBool,
    running: Mutex<Option<&'static AtomicU32>>,
}

#[cfg(feature = "std")]
impl StopFlag {
    pub fn raise(&self) {
        self.0.raised.store(true, Ordering::SeqCst);
        let running = self
            .0
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(flag) = *running {
            flag.store(RAISED, Ordering::SeqCst);
        }
    }

    /// Records `flag` as the running thread's, raised at once if a raise waits.
    pub fn entered(&self, flag: &'static AtomicU32) {
        let mut running = self
            .0
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *running = Some(flag);
        if self.0.raised.load(Ordering::SeqCst) {
            flag.store(RAISED, Ordering::SeqCst);
        }
    }

    /// Records that no thread runs.
    pub fn left(&self) {
        *self
            .0
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// Returns whether it was raised since this was last asked.
    pub fn take(&self) -> bool {
        self.0.raised.swap(false, Ordering::SeqCst)
    }
}

/// What an executor does with its thread once an exit is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carry {
    /// Going on in place with this resume and turn.
    On { resume: Resume, turn: Turn },
    /// Leaving `run` for the loop.
    Leave(Leaving),
}

/// Why a thread leaves `run`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaving {
    /// No thread can run for now.
    Waits,
    /// A switch to another thread.
    AnotherThread,
    /// The thread goes on from new registers.
    NewRegisters,
    /// The container's first process ended.
    Finished,
    /// The system asked for the loop.
    SystemAsked,
}

/// What an executor of the old loop calls while its thread runs.
pub trait Answer {
    /// Answers the thread's `exit` after `progress` since its last exit, handing `cpu` on to the kernel.
    fn exit(&mut self, cpu: &mut dyn Cpu, exit: Exit, progress: u64) -> Carry;
    /// Returns the system's monotonic clock after telling it `progress`.
    fn clock(&mut self, progress: u64) -> u64;
    /// Records a slow path the executor took.
    fn warn(&mut self, warning: Warning<'_>);
}

/// The executor: runs guest threads behind the kernel's [`Cpu`] side.
pub trait Execution: Cpu + Sized {
    type Failure;
    /// Runs a thread, handing every exit to `answer`, until `answer` says leave.
    fn run(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        turn: Turn,
        answer: &mut dyn Answer,
    ) -> Result<(), Self::Failure>;
    /// Returns an executor holding the same memories and threads, with nothing translated.
    fn restored(&self) -> Self;
}

/// What surrounds the kernel, and what a container asks of it as it is made, by shared reference.
pub trait System: wcontainers_kernel_linux::System {
    /// Starts handing the kernel what arrives from outside, once the container is made.
    fn on_container_made(self: &Arc<Self>) {}
    /// Serves a host directory as the kernel's file backend for a bind mount, or answers `Unsupported`.
    #[cfg(feature = "std")]
    fn serve_directory(&self, _source: &std::path::Path) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}
