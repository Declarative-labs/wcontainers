//! The runtime: one loop that drives the kernel, a back end that runs the guest's instructions
//! ([`Execution`]) and what surrounds them ([`Outside`]). Both are type parameters, resolved at compile
//! time; the loop knows no back end and no host.

use core::ops::ControlFlow;

use wcontainers_abi::{Cpu, Exit, Host, Next, Resume, ThreadId, Warning};
pub use wcontainers_kernel::Kernel;

/// How a turn ends: after `limit` units of progress, or never by counting when there is no limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Turn {
    pub limit: Option<u64>,
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
    /// The outside needs no thread in `run` before the kernel is asked what is next.
    OutsideAsked,
}

/// What a back end calls while its thread runs. The loop implements it.
pub trait Answer {
    /// The thread came back with `exit`, having made `progress` since its last exit. `cpu` is the back
    /// end itself, handed on to the kernel.
    fn exit(&mut self, cpu: &mut dyn Cpu, exit: Exit, progress: u64) -> Carry;
    /// The guest reads the clock without a system call, having made `progress` since its last exit:
    /// the outside's monotonic clock, after the outside is told the progress.
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
pub trait Outside: Host {
    /// What a run ended by the outside gives back.
    type End;
    /// How the next turn ends, for the kernel's slice.
    fn turn(&mut self, slice_nanoseconds: Option<u64>) -> Turn;
    /// How far the running thread got since it was last heard from.
    fn ran(&mut self, progress: u64);
    /// Hands the kernel what arrived since the last exit; whether something needs the thread out of
    /// `run` before the kernel is asked what is next.
    fn arrived(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool;
    /// No thread is in `run`: does what [`Outside::arrived`] asked for, and may end the run.
    fn stopped<E: Execution>(&mut self, kernel: &mut Kernel, cpu: &mut E)
    -> ControlFlow<Self::End>;
    /// Nothing can run: waits for an event, or until the time on the monotonic clock, and hands it to
    /// the kernel.
    fn idle(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu, until_nanoseconds: Option<u64>);
    /// A slow path of the back end, apart from the kernel's warnings.
    fn cpu_warning(&mut self, warning: Warning<'_>);
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
    /// The outside ended the run.
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
    pub fn step<E: Execution, O: Outside>(
        &mut self,
        kernel: &mut Kernel,
        cpu: &mut E,
        outside: &mut O,
    ) -> Result<Step<O::End>, E::Failure> {
        match self.kept.take().unwrap_or_else(|| kernel.next()) {
            Next::Run {
                thread,
                resume,
                slice_nanoseconds,
            } => {
                let turn = outside.turn(slice_nanoseconds);
                let mut answer = Answering {
                    kernel,
                    outside,
                    thread,
                    kept: &mut self.kept,
                };
                cpu.run(thread, resume, turn, &mut answer)?;
                Ok(match outside.stopped(kernel, cpu) {
                    ControlFlow::Break(end) => Step::Ended(end),
                    ControlFlow::Continue(()) => Step::Ran,
                })
            }
            Next::Idle { until_nanoseconds } => Ok(Step::Idle { until_nanoseconds }),
            Next::Finished { status } => Ok(Step::Finished { status }),
        }
    }
}

/// Runs the container to its end: [`Stepper::step`] again and again, waiting in [`Outside::idle`].
pub fn drive<E: Execution, O: Outside>(
    kernel: &mut Kernel,
    cpu: &mut E,
    outside: &mut O,
) -> Result<Ending<O::End>, E::Failure> {
    let mut stepper = Stepper::default();
    loop {
        match stepper.step(kernel, cpu, outside)? {
            Step::Ran => {}
            Step::Idle { until_nanoseconds } => outside.idle(kernel, cpu, until_nanoseconds),
            Step::Finished { status } => return Ok(Ending::Finished { status }),
            Step::Ended(end) => return Ok(Ending::Ended(end)),
        }
    }
}

/// The loop's answer to the running thread's exits.
struct Answering<'a, O> {
    kernel: &'a mut Kernel,
    outside: &'a mut O,
    thread: ThreadId,
    kept: &'a mut Option<Next>,
}

impl<O: Outside> Answer for Answering<'_, O> {
    fn exit(&mut self, cpu: &mut dyn Cpu, exit: Exit, progress: u64) -> Carry {
        self.outside.ran(progress);
        self.kernel.exit(cpu, &mut *self.outside, exit);
        if self.outside.arrived(self.kernel, cpu) {
            return Carry::Leave(Leaving::OutsideAsked);
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
                    turn: self.outside.turn(slice_nanoseconds),
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
        self.outside.ran(progress);
        self.outside.monotonic_nanoseconds()
    }

    fn warn(&mut self, warning: Warning<'_>) {
        self.outside.cpu_warning(warning);
    }
}
