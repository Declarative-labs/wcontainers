//! Sending signals within one process and acting on them when they need no handler. Handlers and
//! stopping a process come with signal delivery (K2); a signal that would need either is refused where
//! it would become deliverable, and counted.

use crate::counters::reason;
use crate::linux::general::*;
use crate::linux_extra::{SIG_DFL, SIG_IGN};
use crate::process::{Pid, Tid};
use crate::{Context, Errno, Kernel};

/// The bit for a signal in a signal set.
pub(crate) fn bit(signal: u32) -> u64 {
    1 << (signal - 1)
}

/// Signals that cannot be blocked or caught.
pub(crate) const UNBLOCKABLE: u64 = (1 << (SIGKILL - 1)) | (1 << (SIGSTOP - 1));

/// Signals whose default action stops the process.
const STOPPING: u64 =
    (1 << (SIGSTOP - 1)) | (1 << (SIGTSTP - 1)) | (1 << (SIGTTIN - 1)) | (1 << (SIGTTOU - 1));

/// Signals whose default action is to do nothing (Linux's `sig_kernel_ignore`).
fn ignored_by_default(signal: u32) -> bool {
    matches!(signal, SIGCHLD | SIGURG | SIGWINCH | SIGCONT)
}

/// Whether an action ignores a signal, explicitly or by default (`sig_handler_ignored`).
pub(crate) fn action_ignores(handler: u64, signal: u32) -> bool {
    handler == SIG_IGN || (handler == SIG_DFL && ignored_by_default(signal))
}

impl Kernel {
    fn handler(&self, pid: Pid, signal: u32) -> u64 {
        self.processes[&pid].actions[signal as usize - 1].sa_handler_kernel
    }

    /// Whether a signal not blocked would be dropped by `pid` at once (Linux's `sig_task_ignored`):
    /// ignored, or one the container's first process has no handler for, which Linux keeps from it.
    fn dropped(&self, pid: Pid, signal: u32) -> bool {
        let handler = self.handler(pid, signal);
        action_ignores(handler, signal) || (self.init == Some(pid) && handler == SIG_DFL)
    }

    /// What acting on a signal needs that is not built yet: running its handler, or stopping the
    /// process. Counted when found.
    fn not_built(&mut self, cx: &mut Context<'_>, pid: Pid, signal: u32) -> Option<Errno> {
        if self.dropped(pid, signal) {
            return None;
        }
        if self.handler(pid, signal) != SIG_DFL {
            self.counters
                .count(cx.host, reason::SIGNAL_HANDLER_NOT_RUN, signal.into());
            return Some(Errno::ENOSYS);
        }
        if STOPPING & bit(signal) != 0 {
            self.counters
                .count(cx.host, reason::PROCESS_STOP_NOT_IMPLEMENTED, signal.into());
            return Some(Errno::ENOSYS);
        }
        None
    }

    /// Sends a signal to the calling thread itself, as Linux's `send_signal` does for a caller in the
    /// same process: a continue signal drops pending stops and the reverse, an ignored one is dropped
    /// unless blocked, and the rest become pending. Refused, and counted, when it would have to run a
    /// handler or stop the process.
    pub(crate) fn send_to_self(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        signal: u32,
    ) -> Result<(), Errno> {
        if signal == 0 {
            return Ok(());
        }
        let pid = self.threads[&tid].process;
        let blocked = self.threads[&tid].signal_mask & bit(signal) != 0;
        if !blocked && let Some(error) = self.not_built(cx, pid, signal) {
            return Err(error);
        }
        let thread = self.thread_mut(tid);
        if signal == SIGCONT {
            thread.pending &= !STOPPING;
        } else if STOPPING & bit(signal) != 0 {
            thread.pending &= !bit(SIGCONT);
        }
        if blocked || !self.dropped(pid, signal) {
            self.thread_mut(tid).pending |= bit(signal);
        }
        Ok(())
    }

    /// Checks a new signal mask for pending signals it would let through that need what is not built.
    pub(crate) fn check_unblocking(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        mask: u64,
    ) -> Result<(), Errno> {
        let pid = self.threads[&tid].process;
        let freed = self.threads[&tid].pending & !mask;
        for signal in 1..=64u32 {
            if freed & bit(signal) != 0
                && let Some(error) = self.not_built(cx, pid, signal)
            {
                return Err(error);
            }
        }
        Ok(())
    }

    /// Drops a signal from every pending set of a process, as setting an action that ignores it does.
    pub(crate) fn flush_pending(&mut self, pid: Pid, signal: u32) {
        for tid in self.processes[&pid].threads.clone() {
            self.thread_mut(tid).pending &= !bit(signal);
        }
    }

    /// On a thread's way back to the guest: acts on its pending signals that are not blocked, lowest
    /// first. Dropped ones are forgotten; a fatal one ends the process. Returns whether the thread
    /// still exists.
    pub(crate) fn deliver_signals(&mut self, cx: &mut Context<'_>, tid: Tid) -> bool {
        loop {
            let thread = &self.threads[&tid];
            let ready = thread.pending & !thread.signal_mask;
            if ready == 0 {
                return true;
            }
            let signal = ready.trailing_zeros() + 1;
            self.thread_mut(tid).pending &= !bit(signal);
            let pid = self.threads[&tid].process;
            if self.dropped(pid, signal) {
                continue;
            }
            // Signals needing a handler or a stop never reach here: they are refused before they can be
            // pending and not blocked.
            self.end_process(cx, pid, signal as i32);
            return false;
        }
    }
}
