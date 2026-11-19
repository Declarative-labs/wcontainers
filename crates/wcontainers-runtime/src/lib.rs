//! The runtime: one loop that drives the kernel, a back end that runs the guest's instructions
//! ([`Execution`]) and what surrounds them ([`System`]). Both are type parameters, resolved at compile
//! time; the loop knows no back end and no host.

use core::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use wcontainers_abi::{Cpu, Exit, Host, Next, Resume, ThreadId, Warning};
pub use wcontainers_kernel::Kernel;

mod container;
mod errno;
pub mod files;
mod image;
mod net;
mod spec;
mod system;

pub use container::{Container, Error, Executor, GuestMachine, Runtime, RuntimeSettings};
pub use image::Image;
pub use oci_spec;
pub use spec::{base_spec, bind_mount, spec_from_json};
pub use system::{LocalSystem, Warned};

/// How a turn ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// The back end counts progress and ends the turn after `limit` units, or never with no limit.
    Counted { limit: Option<u64> },
    /// Nothing is counted: the turn ends at the guest's next stop check after the system raises the
    /// [`StopFlag`].
    Free,
}

/// Ends a free turn from any host thread: raises the stop flag of the thread running, or of the next
/// one to run when none runs.
#[derive(Clone, Default)]
pub struct StopFlag(Arc<Raising>);

#[derive(Default)]
struct Raising {
    raised: AtomicBool,
    /// The address of the running thread's flag word, 0 while none runs.
    running: Mutex<usize>,
}

impl StopFlag {
    pub fn raise(&self) {
        self.0.raised.store(true, Ordering::SeqCst);
        let running = self
            .0
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if *running != 0 {
            // SAFETY: the back end keeps the word valid while it is entered (`entered`).
            unsafe { &*(*running as *const AtomicU32) }.store(1, Ordering::SeqCst);
        }
    }

    /// For the back end: a thread whose stop flag is `flag` runs until [`StopFlag::left`]; raised at once
    /// if a raise is waiting.
    ///
    /// # Safety
    /// `flag` stays valid until [`StopFlag::left`].
    pub unsafe fn entered(&self, flag: &AtomicU32) {
        let mut running = self
            .0
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *running = flag as *const AtomicU32 as usize;
        if self.0.raised.load(Ordering::SeqCst) {
            flag.store(1, Ordering::SeqCst);
        }
    }

    /// For the back end: no thread runs.
    pub fn left(&self) {
        *self
            .0
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = 0;
    }

    /// For the back end: whether it was raised since this was last asked.
    pub fn take(&self) -> bool {
        self.0.raised.swap(false, Ordering::SeqCst)
    }
}

/// What a back end does with its thread once an exit has been answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carry {
    /// Carry on in place with this resume, for this next turn.
    On { resume: Resume, turn: Turn },
    /// Leave `run`: the loop has something else to do first.
    Leave(Leaving),
}

/// Why a thread leaves `run`, for a back end that counts leaving as a slow path.
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

/// What a back end calls while its thread runs. The loop implements it.
pub trait Answer {
    /// The thread came back with `exit`, having made `progress` since its last exit. `cpu` is the back
    /// end itself, handed on to the kernel.
    fn exit(&mut self, cpu: &mut dyn Cpu, exit: Exit, progress: u64) -> Carry;
    /// The guest reads the clock without a system call, having made `progress` since its last exit:
    /// the system's monotonic clock, after the system is told the progress.
    fn clock(&mut self, progress: u64) -> u64;
    /// A slow path the back end took.
    fn warn(&mut self, warning: Warning<'_>);
}

/// The back end: runs guest threads behind the kernel's [`Cpu`] side.
pub trait Execution: Cpu + Sized {
    /// What the back end cannot run at all.
    type Failure;
    /// Runs a thread, handing every exit to `answer`, until `answer` says leave.
    fn run(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        turn: Turn,
        answer: &mut dyn Answer,
    ) -> Result<(), Self::Failure>;
    /// A new back end of the same kind holding the same memories, blocks of shared pages and thread
    /// blocks under the same handles, with nothing translated or cached, for a copy of this process
    /// made at a save.
    fn restored(&self) -> Self;
}

/// What surrounds the kernel: its [`Host`], and what the loop asks between turns.
pub trait System: Host {
    /// What a run ended by the system gives back.
    type End;
    /// How the next turn ends, for the kernel's slice.
    fn turn(&mut self, slice_nanoseconds: Option<u64>) -> Turn;
    /// How far the running thread got since it was last heard from.
    fn ran(&mut self, progress: u64);
    /// Hands the kernel what arrived since the last exit; whether something needs the thread out of
    /// `run` before the kernel is asked what is next.
    fn arrived(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool;
    /// No thread is in `run`: does what [`System::arrived`] asked for, and may end the run.
    fn stopped<E: Execution>(&mut self, kernel: &mut Kernel, cpu: &mut E)
    -> ControlFlow<Self::End>;
    /// Nothing can run: waits for an event, or until the time on the monotonic clock, and hands it to
    /// the kernel.
    fn idle(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu, until_nanoseconds: Option<u64>);
    /// A slow path of the back end, apart from the kernel's warnings.
    fn cpu_warning(&mut self, warning: Warning<'_>);
    /// The back end's stop flag, before the first turn, for a system that gives free turns; one that
    /// counts every turn needs none.
    fn stop_flag(&mut self, _flag: StopFlag) {}
    /// Serves a directory of the host as the kernel's file backend, for a bind mount of the
    /// configuration, before the first turn. A system that cannot answers `Unsupported`.
    fn serve_directory(&mut self, _source: &std::path::Path) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

/// What one step did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step<End> {
    /// A thread ran.
    Ran,
    /// Nothing can run until an event arrives, or until this time on the monotonic clock.
    Idle { until_nanoseconds: Option<u64> },
    /// The container's first process ended with this wait status.
    Finished { status: i32 },
    /// The system ended the run.
    Ended(End),
}

/// How a whole run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending<End> {
    Finished { status: i32 },
    Ended(End),
}

/// The loop, one step at a time, for a caller with its own event loop.
#[derive(Default)]
pub struct Stepper {
    /// The kernel's answer kept from the last step, when a thread left with one in hand.
    kept: Option<Next>,
}

impl Stepper {
    pub fn step<E: Execution, S: System>(
        &mut self,
        kernel: &mut Kernel,
        cpu: &mut E,
        system: &mut S,
    ) -> Result<Step<S::End>, E::Failure> {
        match self.kept.take().unwrap_or_else(|| kernel.next()) {
            Next::Run {
                thread,
                resume,
                slice_nanoseconds,
            } => {
                let turn = system.turn(slice_nanoseconds);
                let mut answer = Answering {
                    kernel,
                    system,
                    thread,
                    kept: &mut self.kept,
                };
                cpu.run(thread, resume, turn, &mut answer)?;
                Ok(match system.stopped(kernel, cpu) {
                    ControlFlow::Break(end) => Step::Ended(end),
                    ControlFlow::Continue(()) => Step::Ran,
                })
            }
            Next::Idle { until_nanoseconds } => Ok(Step::Idle { until_nanoseconds }),
            Next::Finished { status } => Ok(Step::Finished { status }),
        }
    }

    /// Runs the container to its end: [`Stepper::step`] again and again, waiting in
    /// [`System::idle`].
    pub fn drive<E: Execution, S: System>(
        &mut self,
        kernel: &mut Kernel,
        cpu: &mut E,
        system: &mut S,
    ) -> Result<Ending<S::End>, E::Failure> {
        loop {
            match self.step(kernel, cpu, system)? {
                Step::Ran => {}
                Step::Idle { until_nanoseconds } => system.idle(kernel, cpu, until_nanoseconds),
                Step::Finished { status } => return Ok(Ending::Finished { status }),
                Step::Ended(end) => return Ok(Ending::Ended(end)),
            }
        }
    }
}

/// Runs the container to its end from its first step.
pub fn drive<E: Execution, S: System>(
    kernel: &mut Kernel,
    cpu: &mut E,
    system: &mut S,
) -> Result<Ending<S::End>, E::Failure> {
    Stepper::default().drive(kernel, cpu, system)
}

/// The loop's answer to the running thread's exits.
struct Answering<'a, S> {
    kernel: &'a mut Kernel,
    system: &'a mut S,
    thread: ThreadId,
    kept: &'a mut Option<Next>,
}

impl<S: System> Answer for Answering<'_, S> {
    fn exit(&mut self, cpu: &mut dyn Cpu, exit: Exit, progress: u64) -> Carry {
        self.system.ran(progress);
        self.kernel.exit(cpu, &mut *self.system, exit);
        if self.system.arrived(self.kernel, cpu) {
            return Carry::Leave(Leaving::SystemAsked);
        }
        let next = self.kernel.next();
        let why = match next {
            Next::Run {
                thread,
                resume: resume @ (Resume::Return(_) | Resume::Continue),
                slice_nanoseconds,
            } if thread == self.thread => {
                return Carry::On {
                    resume,
                    turn: self.system.turn(slice_nanoseconds),
                };
            }
            Next::Run { thread, .. } if thread == self.thread => Leaving::NewRegisters,
            Next::Run { .. } => Leaving::AnotherThread,
            Next::Idle { .. } => Leaving::Waits,
            Next::Finished { .. } => Leaving::Finished,
        };
        *self.kept = Some(next);
        Carry::Leave(why)
    }

    fn clock(&mut self, progress: u64) -> u64 {
        self.system.ran(progress);
        self.system.monotonic_nanoseconds()
    }

    fn warn(&mut self, warning: Warning<'_>) {
        self.system.cpu_warning(warning);
    }
}
