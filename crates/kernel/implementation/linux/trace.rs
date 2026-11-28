//! The link between a traced thread and its tracer, and the stop it makes for each signal.
//!
//! Linux's signal-delivery-stop (`ptrace_signal`): how the tracer is told and resumes it, and what
//! happens when the tracer goes.

use wcontainers_executor::Executor;

use crate::linux::general::*;
use crate::process::{Pid, Queued, Tid};
use crate::{Context, Kernel};

/// The signal a stopped thread goes on with when it is given none.
const NO_SIGNAL: u32 = 0;

/// The CPU time a stop's SIGCHLD carries, 0 here as for a job control stop's.
const STOP_TICKS: u64 = 0;

/// A traced thread's link to its tracer (Linux's `ptrace` flags and `parent`).
#[derive(Clone)]
pub(crate) struct Trace {
    /// The thread tracing it, the only one whose requests it answers.
    pub tracer: Tid,
    /// The stop it is in, if it stopped for its tracer.
    pub stop: Option<Stop>,
}

/// A traced thread's stop for a signal it took (Linux's `TASK_TRACED` and `last_siginfo`).
#[derive(Clone)]
pub(crate) struct Stop {
    /// The signal it stopped for, which a wait reports.
    pub signal: u32,
    /// The signal's information, which `PTRACE_GETSIGINFO` reads and `PTRACE_SETSIGINFO` replaces.
    pub info: crate::siginfo::Bytes,
    /// A wait has reported the stop (Linux clears `exit_code`), so another does not.
    pub reported: bool,
}

/// How a stopped thread goes on: the signal it acts on (0 for none) and the tracer choosing it.
///
/// A signal other than the one it stopped for carries the tracer's information.
pub(crate) struct Going {
    pub signal: u32,
    pub tracer: Tid,
}

impl Kernel {
    /// Tells whether `tid` is traced.
    pub(crate) fn traced(&self, tid: Tid) -> bool {
        self.threads
            .get(&tid)
            .is_some_and(|thread| thread.trace.is_some())
    }

    /// Tells whether `tid` is stopped for its tracer.
    pub(crate) fn trace_stopped(&self, tid: Tid) -> bool {
        self.threads
            .get(&tid)
            .and_then(|thread| thread.trace.as_ref())
            .is_some_and(|trace| trace.stop.is_some())
    }

    /// Stops traced thread `tid`, which took `queued` on its way back, and tells its tracer.
    ///
    /// It keeps how it goes on (`ptrace_signal`).
    pub(crate) fn stop_for_tracer<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        queued: &Queued,
    ) {
        // Out of line and off the CPU; a turn it still has in the line of signalled threads is
        // skipped while it is stopped.
        self.take_out_of_line(tid);
        if self.current == Some(tid) {
            self.current = None;
        }
        let thread = self.thread_mut(tid);
        let Some(trace) = thread.trace.as_mut() else {
            return;
        };
        trace.stop = Some(Stop {
            signal: queued.signal,
            info: queued.info,
            reported: false,
        });
        let tracer = trace.tracer;
        let uid = thread.credentials.uid;
        self.tell_tracer(cx, tracer, tid, uid, queued.signal);
    }

    /// Sends the tracer's process SIGCHLD for a stop and wakes its waits.
    ///
    /// CLD_TRAPPED, unless it asked not to be told of stops (`do_notify_parent_cldstop`).
    fn tell_tracer<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tracer: Tid,
        tid: Tid,
        uid: u32,
        signal: u32,
    ) {
        let Some(process) = self.threads.get(&tracer).map(|thread| thread.process) else {
            return;
        };
        let action = self.processes[&process].actions.borrow()[SIGCHLD as usize - 1];
        if action.sa_flags & u64::from(SA_NOCLDSTOP) == 0 {
            let info =
                crate::signal::child_info(tid, uid, CLD_TRAPPED as i32, signal as i32, STOP_TICKS);
            self.send_to_process(cx, process, SIGCHLD, info);
        }
        self.wake_waiters_of(process);
    }

    /// Lets thread `tid`, stopped for its tracer, go on with `going`'s signal, if any.
    ///
    /// It then acts on its other signals, from where it stopped (`ptrace_resume`, `ptrace_detach`).
    /// A signal it now blocks is sent to it again instead; one other than the signal it stopped for
    /// carries the tracer's own information (SI_USER).
    pub(crate) fn resume_traced<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        stop: Stop,
        going: &Going,
    ) {
        let signal = going.signal;
        if signal != NO_SIGNAL {
            let info = if signal == stop.signal {
                stop.info
            } else {
                let uid = self.task_credentials(going.tracer).uid;
                crate::signal::sender_info(signal, SI_USER as i32, going.tracer, uid)
            };
            if self.threads[&tid].signal_mask & crate::signal::bit(signal) != 0 {
                self.send_to_thread(cx, tid, signal, info);
            } else {
                self.thread_mut(tid).signal_taken = Some(Queued {
                    signal,
                    info,
                    charge: None,
                    timer: false,
                });
            }
        }
        if !self.signalled.contains(&tid) {
            self.signalled.push_back(tid);
        }
    }

    /// Lets go of every thread `tracer` traces, as it ends (`exit_ptrace`).
    ///
    /// One stopped for it goes on without the signal it stopped for, as on Linux (MEASURED: a
    /// tracee stopped for SIGUSR1 goes on without running its handler once its tracer ends).
    pub(crate) fn tracer_ended<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tracer: Tid,
    ) {
        let traced: alloc::vec::Vec<Tid> = self
            .threads
            .iter()
            .filter(|(_, thread)| {
                thread
                    .trace
                    .as_ref()
                    .is_some_and(|trace| trace.tracer == tracer)
            })
            .map(|(tid, _)| *tid)
            .collect();
        for tid in traced {
            let trace = self.thread_mut(tid).trace.take();
            if let Some(stop) = trace.and_then(|trace| trace.stop) {
                let going = Going {
                    signal: NO_SIGNAL,
                    tracer,
                };
                self.resume_traced(cx, tid, stop, &going);
            }
        }
    }

    /// Takes the threads of `pid`, which a fatal signal ends, out of their stops to run to it.
    pub(crate) fn leave_trace_stops(&mut self, pid: Pid) {
        for tid in self.processes[&pid].threads.clone() {
            if let Some(trace) = self.thread_mut(tid).trace.as_mut() {
                trace.stop = None;
            }
        }
    }

    /// Returns the traced threads a wait by `tid` finds, each with its unreported stop's signal.
    ///
    /// Those `admits` takes and whose tracer the wait looks at: one of its process, or only `tid`
    /// itself with `__WNOTHREAD` (`ptrace_do_wait`).
    pub(crate) fn traced_for_wait(
        &self,
        tid: Tid,
        own_thread_only: bool,
        admits: impl Fn(Tid, Pid) -> bool,
    ) -> alloc::vec::Vec<(Tid, Option<u32>)> {
        let pid = self.threads[&tid].process;
        self.threads
            .iter()
            .filter_map(|(traced, thread)| {
                let trace = thread.trace.as_ref()?;
                let tracer_process = self.threads.get(&trace.tracer)?.process;
                let looked_at = if own_thread_only {
                    trace.tracer == tid
                } else {
                    tracer_process == pid
                };
                (looked_at && admits(*traced, thread.process)).then(|| {
                    let unreported = trace
                        .stop
                        .as_ref()
                        .filter(|stop| !stop.reported)
                        .map(|stop| stop.signal);
                    (*traced, unreported)
                })
            })
            .collect()
    }

    /// Marks `tid`'s stop as reported by a wait, so another wait does not see it again.
    pub(crate) fn trace_stop_reported(&mut self, tid: Tid) {
        if let Some(stop) = self
            .thread_mut(tid)
            .trace
            .as_mut()
            .and_then(|trace| trace.stop.as_mut())
        {
            stop.reported = true;
        }
    }

    /// Tells whether a thread of process `pid` is traced (by another process's, as no other can).
    pub(crate) fn process_traced(&self, pid: Pid) -> bool {
        self.processes[&pid]
            .threads
            .iter()
            .any(|tid| self.traced(*tid))
    }
}
