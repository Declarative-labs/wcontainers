//! Tracing with `ptrace`: the link between a traced thread and the thread tracing it, the stop a
//! traced thread makes for each signal it is about to act on (Linux's signal-delivery-stop,
//! `ptrace_signal`), how its tracer is told and resumes it, and what happens when the tracer goes.

use wcontainers_executor::Executor;

use crate::linux::general::*;
use crate::process::{Pid, Queued, Tid};
use crate::{Context, Kernel};

/// A traced thread's link to its tracer (Linux's `ptrace` flags and `parent`).
#[derive(Clone)]
pub(crate) struct Trace {
    /// The thread tracing it, the only one whose requests it answers.
    pub tracer: Tid,
    /// The stop it is in, if it stopped for its tracer.
    pub stop: Option<Stop>,
}

/// A traced thread's stop for a signal it took (Linux's `TASK_TRACED` with `exit_code` and
/// `last_siginfo`).
#[derive(Clone)]
pub(crate) struct Stop {
    /// The signal it stopped for, which a wait reports.
    pub signal: u32,
    /// The signal's information, which `PTRACE_GETSIGINFO` reads and `PTRACE_SETSIGINFO` replaces.
    pub info: [u8; 128],
    /// A wait has reported the stop (Linux clears `exit_code`), so another does not.
    pub reported: bool,
}

/// How a stopped thread goes on: the signal it acts on (0 for none) and the tracer that chose it,
/// whose information a signal other than the one it stopped for carries.
pub(crate) struct Going {
    pub signal: u32,
    pub tracer: Tid,
}

impl Kernel {
    /// Whether `tid` is traced.
    pub(crate) fn traced(&self, tid: Tid) -> bool {
        self.threads
            .get(&tid)
            .is_some_and(|thread| thread.trace.is_some())
    }

    /// Whether `tid` is stopped for its tracer.
    pub(crate) fn trace_stopped(&self, tid: Tid) -> bool {
        self.threads
            .get(&tid)
            .and_then(|thread| thread.trace.as_ref())
            .is_some_and(|trace| trace.stop.is_some())
    }

    /// A traced thread took `queued` on its way back to its program (`ptrace_signal`): it stops
    /// where it is, keeping how it goes on, and its tracer is told.
    pub(crate) fn stop_for_tracer<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        queued: &Queued,
    ) {
        self.take_out_of_line(tid);
        if self.current == Some(tid) {
            self.current = None;
        }
        self.signalled.retain(|other| *other != tid);
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

    /// Sends the tracer's process SIGCHLD for a stop (CLD_TRAPPED), unless it asked not to be told
    /// of stops, and wakes its waits (`do_notify_parent_cldstop` for ptrace).
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
            let info = crate::signal::child_info(tid, uid, CLD_TRAPPED as i32, signal as i32, 0);
            self.send_to_process(cx, process, SIGCHLD, info);
        }
        self.wake_waiters_of(process);
    }

    /// A thread stopped for its tracer goes on (`ptrace_resume`, `ptrace_detach`): it acts on
    /// `going`'s signal, if any, and then on its other signals, from where it stopped. A signal it
    /// now blocks is sent to it again instead; one other than the signal it stopped for carries
    /// the tracer's own information (SI_USER).
    pub(crate) fn resume_traced<S: crate::System, C: Executor + 'static>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        stop: Stop,
        going: &Going,
    ) {
        let signal = going.signal;
        if signal != 0 {
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

    /// Thread `tracer` ends (`exit_ptrace`): every thread it traces is let go of, and one stopped
    /// for it goes on without the signal it stopped for, as Linux's does (MEASURED: a tracee stopped
    /// for SIGUSR1 with a handler goes on without running it once its tracer ends).
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
                let going = Going { signal: 0, tracer };
                self.resume_traced(cx, tid, stop, &going);
            }
        }
    }

    /// The threads of `pid` a fatal signal ends leave their stops, so they run to their end.
    pub(crate) fn leave_trace_stops(&mut self, pid: Pid) {
        for tid in self.processes[&pid].threads.clone() {
            if let Some(trace) = self.thread_mut(tid).trace.as_mut() {
                trace.stop = None;
            }
        }
    }

    /// What a wait by thread `tid` finds of the threads its process traces: each traced thread
    /// `which` admits and whose tracer the wait looks at (only `tid` itself with `__WNOTHREAD`),
    /// with its unreported stop's signal, if any (`ptrace_do_wait`).
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

    /// A wait reported `tid`'s stop; another wait does not see it again.
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

    /// Whether a thread of process `pid` is traced (by another process's thread, as no thread may
    /// trace its own process).
    pub(crate) fn process_traced(&self, pid: Pid) -> bool {
        self.processes[&pid]
            .threads
            .iter()
            .any(|tid| self.traced(*tid))
    }
}
