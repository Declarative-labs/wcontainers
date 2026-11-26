//! The runtime: a container made from an OCI configuration and an image, run by its kernel's runners, each on a host
//! thread of its own with an executor of its own, and served by what surrounds the kernel ([`System`]).
//!
//! Without the `std` feature only the types an executor of the old loop names are built ([`Execution`], [`Answer`]),
//! for an executor that runs where there is no standard library; they go once no executor runs that way.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
#[cfg(feature = "std")]
use std::sync::{Mutex, PoisonError};

extern crate alloc;
use alloc::sync::Arc;

use wcontainers_abi::{Cpu, Exit, Resume, ThreadId, Warning};
pub use wcontainers_kernel_linux::{Kernel, Standard};

#[cfg(feature = "std")]
mod arrivals;
#[cfg(feature = "std")]
mod container;
#[cfg(feature = "std")]
mod described;
#[cfg(feature = "std")]
mod errno;
#[cfg(feature = "std")]
pub mod files;
#[cfg(feature = "std")]
mod host_call;
#[cfg(feature = "std")]
pub mod image;
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
pub use container::{Container, Error, Executor, GuestMachine, Runtime, RuntimeConfig};
#[cfg(feature = "std")]
pub use described::LocalKernel;
#[cfg(feature = "std")]
pub use image::Image;
#[cfg(feature = "std")]
pub use oci_spec;
#[cfg(feature = "std")]
pub use runners::{IDLE_WITHOUT_WAKE, KernelContainer, RunInterrupter};
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
    /// The executor counts progress and ends the turn after `limit` units, or never with no limit.
    Counted { limit: Option<u64> },
    /// Nothing is counted: the turn ends at the guest's next stop check after the system raises the
    /// [`StopFlag`].
    Free,
}

/// Ends a free turn from any host thread: raises the stop flag of the thread running, or of the next
/// one to run when none runs.
#[cfg(feature = "std")]
#[derive(Clone, Default)]
pub struct StopFlag(Arc<Raising>);

#[cfg(feature = "std")]
#[derive(Default)]
struct Raising {
    raised: AtomicBool,
    /// The running thread's flag word.
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
            flag.store(1, Ordering::SeqCst);
        }
    }

    /// For the executor: a thread whose stop flag is `flag` runs until [`StopFlag::left`]; raised at once
    /// if a raise is waiting.
    pub fn entered(&self, flag: &'static AtomicU32) {
        let mut running = self
            .0
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *running = Some(flag);
        if self.0.raised.load(Ordering::SeqCst) {
            flag.store(1, Ordering::SeqCst);
        }
    }

    /// For the executor: no thread runs.
    pub fn left(&self) {
        *self
            .0
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// For the executor: whether it was raised since this was last asked.
    pub fn take(&self) -> bool {
        self.0.raised.swap(false, Ordering::SeqCst)
    }
}

/// What an executor does with its thread once an exit has been answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carry {
    /// Carry on in place with this resume, for this next turn.
    On { resume: Resume, turn: Turn },
    /// Leave `run`: the loop has something else to do first.
    Leave(Leaving),
}

/// Why a thread leaves `run`, for an executor that counts leaving as a slow path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaving {
    /// No thread can run until an event arrives or time passes.
    Waits,
    /// Another thread runs next.
    AnotherThread,
    /// The thread goes on from new registers.
    NewRegisters,
    /// The container's first process ended.
    Finished,
    /// The system needs no thread in `run` before the kernel is asked what is next.
    SystemAsked,
}

/// What an executor of the old loop calls while its thread runs.
pub trait Answer {
    /// The thread came back with `exit`, having made `progress` since its last exit. `cpu` is the back
    /// end itself, handed on to the kernel.
    fn exit(&mut self, cpu: &mut dyn Cpu, exit: Exit, progress: u64) -> Carry;
    /// The guest reads the clock without a system call, having made `progress` since its last exit:
    /// the system's monotonic clock, after the system is told the progress.
    fn clock(&mut self, progress: u64) -> u64;
    /// A slow path the executor took.
    fn warn(&mut self, warning: Warning<'_>);
}

/// The executor: runs guest threads behind the kernel's [`Cpu`] side.
pub trait Execution: Cpu + Sized {
    /// What the executor cannot run at all.
    type Failure;
    /// Runs a thread, handing every exit to `answer`, until `answer` says leave.
    fn run(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        turn: Turn,
        answer: &mut dyn Answer,
    ) -> Result<(), Self::Failure>;
    /// A new executor of the same kind holding the same memories, blocks of shared pages and thread
    /// blocks under the same handles, with nothing translated or cached, for a copy of this process
    /// made at a save.
    fn restored(&self) -> Self;
}

/// What surrounds the kernel: its [`wcontainers_kernel_linux::System`], and what a container asks of
/// it as it is made, by shared reference so that the system synchronizes what it changes.
pub trait System: wcontainers_kernel_linux::System {
    /// Starts handing the kernel what arrives from outside, once the container is made.
    fn on_container_made(self: &Arc<Self>) {}
    /// Names the ends of the first process's descriptors 0 to 2, whose other sides the system holds:
    /// a terminal of this size when the configuration asks for one, else pipes.
    fn standard(
        &self,
        terminal: Option<TerminalSize>,
    ) -> Standard<<Self as wcontainers_system::StreamTransport>::Endpoint>;
    /// The executor's stop flag, before the first turn, for a system that gives free turns; one that
    /// counts every turn needs none.
    #[cfg(feature = "std")]
    fn stop_flag(&self, _flag: StopFlag) {}
    /// Serves a directory of the host as the kernel's file backend, for a bind mount of the
    /// configuration, before the first turn. A system that cannot answers `Unsupported`.
    #[cfg(feature = "std")]
    fn serve_directory(&self, _source: &std::path::Path) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}
