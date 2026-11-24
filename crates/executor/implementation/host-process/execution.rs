//! How the kernel's loop drives form P (plan 4.6): the one place that implements an executor trait, so the owner's
//! interface, when it is merged, changes only this module.

use core::convert::Infallible;
use std::time::Duration;

use wcontainers_abi::{Exit, ThreadId, Warning};
use wcontainers_runtime_single_kernel::{Answer, Carry, Execution, Turn};

use super::HostProcessExecutor;

impl HostProcessExecutor {
    /// The count at which `turn`, starting on `thread` now, ends, and the time that bounds each entry into its code:
    /// a unit of progress is taken as a nanosecond, since a host process cannot be stopped at an exact count.
    fn turn_end(&self, thread: ThreadId, turn: Turn) -> (Option<u64>, Option<Duration>) {
        match turn {
            Turn::Counted { limit: None } | Turn::Free => (None, None),
            Turn::Counted { limit: Some(limit) } => (
                Some(self.guest_count(thread) + limit),
                Some(Duration::from_nanos(limit)),
            ),
        }
    }

    fn guest_count(&self, thread: ThreadId) -> u64 {
        self.guests[thread.0 as usize]
            .as_ref()
            .expect("a thread the kernel holds")
            .count()
    }
}

impl Execution for HostProcessExecutor {
    type Failure = Infallible;

    /// Counter reads are told no progress; the progress made before one is handed with the exit that follows.
    fn run(
        &mut self,
        thread: ThreadId,
        mut resume: wcontainers_abi::Resume,
        turn: Turn,
        answer: &mut dyn Answer,
    ) -> Result<(), Infallible> {
        let (mut until, mut slice) = self.turn_end(thread, turn);
        loop {
            let before = self.guest_count(thread);
            let exit = self.run_translated(thread, resume, until, slice, &mut || answer.clock(0));
            for (reason, count, detail) in self.take_warnings() {
                answer.warn(Warning {
                    reason,
                    count,
                    detail,
                });
            }
            let progress = self.guest_count(thread) - before;
            if exit != Exit::Stop {
                self.counters.kernel_entries += 1;
            }
            match answer.exit(self, exit, progress) {
                Carry::On {
                    resume: next,
                    turn: next_turn,
                } => {
                    (until, slice) = self.turn_end(thread, next_turn);
                    resume = next;
                }
                Carry::Leave(_) => return Ok(()),
            }
        }
    }

    fn restored(&self) -> Self {
        unimplemented!("a save of form P is not built")
    }
}
