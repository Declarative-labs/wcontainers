//! Signals: sending them to threads, processes and groups, acting on them on a thread's way back to
//! the guest, the frame a handler is given (laid out exactly as arm64 Linux lays it out), and the
//! return from a handler.

use wcontainers_abi::{Access, RegisterBlock, Resume};

use crate::counters::reason;
use crate::linux::general::*;
use crate::linux_extra::frame;
use crate::linux_extra::{SIG_DFL, SIG_IGN};
use crate::memory::Backing;
use crate::process::{Channel, Pid, Queued, Report, Tid};
use crate::syscall::Outcome;
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

/// Signals a fault raises, taken before others (`SYNCHRONOUS_MASK`).
const SYNCHRONOUS: u64 = (1 << (SIGSEGV - 1))
    | (1 << (SIGBUS - 1))
    | (1 << (SIGILL - 1))
    | (1 << (SIGTRAP - 1))
    | (1 << (SIGFPE - 1))
    | (1 << (SIGSYS - 1));

/// What a signal does when its handler is the default.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Default {
    Ignore,
    Terminate,
    /// Terminate, writing a core file where one is allowed.
    Core,
    Stop,
    Continue,
}

fn default_action(signal: u32) -> Default {
    match signal {
        SIGCHLD | SIGURG | SIGWINCH => Default::Ignore,
        SIGCONT => Default::Continue,
        SIGSTOP | SIGTSTP | SIGTTIN | SIGTTOU => Default::Stop,
        SIGQUIT | SIGILL | SIGTRAP | SIGABRT | SIGBUS | SIGFPE | SIGSEGV | SIGXCPU | SIGXFSZ
        | SIGSYS => Default::Core,
        _ => Default::Terminate,
    }
}

/// Whether an action ignores a signal, explicitly or by default (`sig_handler_ignored`).
pub(crate) fn action_ignores(handler: u64, signal: u32) -> bool {
    handler == SIG_IGN
        || (handler == SIG_DFL
            && matches!(default_action(signal), Default::Ignore | Default::Continue))
}

/// A `siginfo` with its number and code.
fn info(signal: u32, code: i32) -> [u8; 128] {
    let mut info = [0; 128];
    info[0..4].copy_from_slice(&signal.to_le_bytes());
    info[8..12].copy_from_slice(&code.to_le_bytes());
    info
}

fn put(info: &mut [u8; 128], at: usize, bytes: &[u8]) {
    info[at..at + bytes.len()].copy_from_slice(bytes);
}

/// The information of a signal the kernel itself sends (`SEND_SIG_PRIV`).
pub(crate) fn kernel_info(signal: u32) -> [u8; 128] {
    info(signal, SI_KERNEL as i32)
}

/// The information of a signal sent by a process: `kill` (SI_USER) or `tkill` (SI_TKILL).
pub(crate) fn sender_info(signal: u32, code: i32, pid: Pid, uid: u32) -> [u8; 128] {
    let mut info = info(signal, code);
    put(&mut info, 16, &pid.to_le_bytes());
    put(&mut info, 20, &uid.to_le_bytes());
    info
}

/// SIGCHLD's information: the child, its user, how it changed and its CPU time in ticks.
pub(crate) fn child_info(pid: Pid, uid: u32, code: i32, status: i32, ticks: u64) -> [u8; 128] {
    let mut info = info(SIGCHLD, code);
    put(&mut info, 16, &pid.to_le_bytes());
    put(&mut info, 20, &uid.to_le_bytes());
    put(&mut info, 24, &status.to_le_bytes());
    put(&mut info, 32, &ticks.to_le_bytes());
    put(&mut info, 40, &0u64.to_le_bytes());
    info
}

/// A fault's information: the address it was at.
pub(crate) fn fault_info(signal: u32, code: i32, address: u64) -> [u8; 128] {
    let mut info = info(signal, code);
    put(&mut info, 16, &address.to_le_bytes());
    info
}

/// The signal number a `siginfo` carries.
fn signal_of(info: &[u8; 128]) -> u32 {
    u32::from_le_bytes(info[0..4].try_into().expect("four bytes"))
}

/// Where a signal goes once sent.
#[derive(Clone, Copy)]
enum Target {
    Thread(Tid),
    Process(Pid),
}

impl Kernel {
    fn handler(&self, pid: Pid, signal: u32) -> u64 {
        self.processes[&pid].actions.borrow()[signal as usize - 1].sa_handler_kernel
    }

    /// Whether `pid` drops a signal at once (`sig_task_ignored`): ignored, or one the container's
    /// first process has no handler for, which Linux keeps from it.
    fn dropped(&self, pid: Pid, signal: u32) -> bool {
        let handler = self.handler(pid, signal);
        action_ignores(handler, signal) || (self.init == Some(pid) && handler == SIG_DFL)
    }

    /// Whether a thread has a signal it does not block waiting for it (Linux's `signal_pending`).
    pub(crate) fn signal_pending(&self, tid: Tid) -> bool {
        let thread = &self.threads[&tid];
        let process = &self.processes[&thread.process];
        (thread.pending.set | process.pending.set) & !thread.signal_mask != 0
    }

    /// Sends a signal to one thread (`tkill`, a fault, a timer); one that does not fit the user's
    /// queue is lost as [`Self::queue_to_thread`] says.
    pub(crate) fn send_to_thread(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        signal: u32,
        info: [u8; 128],
    ) {
        let _ = self.send(cx, Target::Thread(tid), signal, info);
    }

    /// As [`Self::send_to_thread`], telling the sender (EAGAIN) of a real-time signal with
    /// information of its own that does not fit the receiving user's queue.
    pub(crate) fn queue_to_thread(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        signal: u32,
        info: [u8; 128],
    ) -> Result<(), crate::Errno> {
        self.send(cx, Target::Thread(tid), signal, info)
    }

    /// SIGXFSZ for a write past the file size limit, as `send_sig` sends it.
    pub(crate) fn send_file_size_signal(&mut self, cx: &mut Context<'_>, tid: Tid) {
        let process = self.process_of(tid);
        let info = sender_info(
            SIGXFSZ,
            SI_USER as i32,
            process.pid,
            self.credentials(tid).uid,
        );
        self.send_to_thread(cx, tid, SIGXFSZ, info);
    }

    /// Sends a signal to a whole process (`kill`).
    pub(crate) fn send_to_process(
        &mut self,
        cx: &mut Context<'_>,
        pid: Pid,
        signal: u32,
        info: [u8; 128],
    ) {
        let _ = self.send(cx, Target::Process(pid), signal, info);
    }

    /// A POSIX timer's signal, to one thread or (with none) the process: it goes in the timer's own
    /// place in the user's count, taken when the timer was made, so it always fits.
    pub(crate) fn send_from_timer(
        &mut self,
        cx: &mut Context<'_>,
        thread: Option<Tid>,
        pid: Pid,
        signal: u32,
        info: [u8; 128],
    ) {
        let target = thread.map_or(Target::Process(pid), Target::Thread);
        let _ = self.send_queued(cx, target, signal, info, true);
    }

    /// Whether a timer's signal to one thread or (with none) the process would be dropped as
    /// ignored (`prepare_signal` failing in `posixtimer_send_sigqueue`).
    pub(crate) fn timer_signal_ignored(&self, thread: Option<Tid>, pid: Pid, signal: u32) -> bool {
        let Some(tid) = thread.or_else(|| self.processes[&pid].threads.first().copied()) else {
            return false;
        };
        self.threads[&tid].signal_mask & bit(signal) == 0 && self.dropped(pid, signal)
    }

    /// Linux's `posixtimer_init_sigqueue`: a place in the calling user's count for a new timer's
    /// signal, or none (EAGAIN) when the count is at the caller's RLIMIT_SIGPENDING.
    pub(crate) fn timer_charge(&mut self, tid: Tid) -> Option<crate::process::Charge> {
        let uid = self.credentials(tid).uid;
        let limit = self.process_of(tid).limits[RLIMIT_SIGPENDING as usize].current;
        let count = self.signal_users.entry(uid).or_default().clone();
        (count.get() < limit).then(|| crate::process::Charge::new(&count))
    }

    /// As [`Self::send_to_process`], telling the sender as [`Self::queue_to_thread`] does.
    pub(crate) fn queue_to_process(
        &mut self,
        cx: &mut Context<'_>,
        pid: Pid,
        signal: u32,
        info: [u8; 128],
    ) -> Result<(), crate::Errno> {
        self.send(cx, Target::Process(pid), signal, info)
    }

    /// Linux's `send_sigio`: the I/O signal of an open file with `FASYNC` set, to its owner's tasks
    /// that its namer may signal (`sigio_perm`), for `reason` (`POLL_IN`, `POLL_OUT`, ...). With no
    /// signal chosen it is SIGIO from the kernel; otherwise it carries the band and the descriptor.
    pub(crate) fn send_io_signal(
        &mut self,
        cx: &mut Context<'_>,
        file: &crate::file::FileRef,
        reason: u32,
    ) {
        use crate::file::OwnerKind;
        let (owner, fd) = {
            let file = file.borrow();
            match (file.owner, file.async_fd) {
                (Some(owner), Some(fd)) if file.flags & FASYNC != 0 && owner.pid != 0 => {
                    (owner, fd)
                }
                _ => return,
            }
        };
        let live = |process: &crate::process::Process| process.ended.is_none();
        let targets: alloc::vec::Vec<(Target, Pid)> = match owner.kind {
            OwnerKind::Thread => self
                .threads
                .get(&owner.pid)
                .map(|thread| (Target::Thread(owner.pid), thread.process))
                .into_iter()
                .collect(),
            OwnerKind::Process => self
                .processes
                .get(&owner.pid)
                .filter(|process| live(process))
                .map(|process| (Target::Process(process.pid), process.pid))
                .into_iter()
                .collect(),
            OwnerKind::Group => self
                .processes
                .values()
                .filter(|process| process.group == owner.pid && live(process))
                .map(|process| (Target::Process(process.pid), process.pid))
                .collect(),
        };
        for (target, pid) in targets {
            let credentials = self.task_credentials(pid);
            let allowed = owner.euid == 0
                || [owner.euid, owner.uid]
                    .iter()
                    .any(|user| *user == credentials.suid || *user == credentials.uid);
            if !allowed {
                continue;
            }
            let (signal, info) = if owner.signal == 0 {
                (SIGIO, kernel_info(SIGIO))
            } else {
                let signal = owner.signal as u32;
                // The poll codes belong to SIGPOLL; a signal with codes of its own gets SI_SIGIO.
                let own_codes = matches!(
                    signal,
                    SIGILL | SIGFPE | SIGSEGV | SIGBUS | SIGTRAP | SIGCHLD | SIGSYS
                );
                let code = if own_codes { SI_SIGIO } else { reason as i32 };
                let band = match reason {
                    POLL_IN => POLLIN | POLLRDNORM,
                    POLL_OUT => POLLOUT | POLLWRNORM | POLLWRBAND,
                    POLL_MSG => POLLIN | POLLRDNORM | POLLMSG,
                    POLL_ERR => POLLERR,
                    POLL_PRI => POLLPRI | POLLRDBAND,
                    _ => POLLHUP | POLLERR,
                };
                let mut info = info(signal, code);
                put(&mut info, 16, &u64::from(band).to_le_bytes());
                put(&mut info, 24, &fd.to_le_bytes());
                (signal, info)
            };
            // A real-time signal that does not fit becomes SIGIO (`send_sigio_to_task`).
            if self.send(cx, target, signal, info).is_err() {
                let _ = self.send(cx, target, SIGIO, kernel_info(SIGIO));
            }
        }
    }

    /// Sends a signal to every live process of a group; false if it has none.
    pub(crate) fn send_to_group(
        &mut self,
        cx: &mut Context<'_>,
        group: Pid,
        signal: u32,
        info: [u8; 128],
    ) -> bool {
        let members: alloc::vec::Vec<Pid> = self
            .processes
            .values()
            .filter(|process| process.group == group && process.ended.is_none())
            .map(|process| process.pid)
            .collect();
        for pid in &members {
            self.send_to_process(cx, *pid, signal, info);
        }
        !members.is_empty()
    }

    /// Linux's `send_signal_locked`: the side effects of stopping and continuing, dropping what is
    /// ignored, queueing, then waking or ending what it concerns.
    fn send(
        &mut self,
        cx: &mut Context<'_>,
        target: Target,
        signal: u32,
        info: [u8; 128],
    ) -> Result<(), crate::Errno> {
        self.send_queued(cx, target, signal, info, false)
    }

    /// As `send`; a timer's signal (`timer`) is queued in the timer's place, not charged again.
    fn send_queued(
        &mut self,
        cx: &mut Context<'_>,
        target: Target,
        signal: u32,
        info: [u8; 128],
        timer: bool,
    ) -> Result<(), crate::Errno> {
        if signal == 0 {
            return Ok(());
        }
        let (pid, leader) = match target {
            Target::Thread(tid) => (self.threads[&tid].process, tid),
            Target::Process(pid) => match self.processes[&pid].threads.first() {
                Some(tid) => (pid, *tid),
                None => return Ok(()),
            },
        };
        // `prepare_signal`.
        if STOPPING & bit(signal) != 0 {
            self.drop_pending(pid, bit(SIGCONT));
        } else if signal == SIGCONT {
            self.drop_pending(pid, STOPPING);
            self.continue_process(cx, pid);
        }
        let blocked = self.threads[&leader].signal_mask & bit(signal) != 0;
        if !blocked && self.dropped(pid, signal) {
            // A wait for this signal still sees it (`sigtimedwait` blocks what it waits for).
            return Ok(());
        }
        // A standard signal already pending is not queued again (`legacy_queue`).
        let pending = match target {
            Target::Thread(tid) => &self.threads[&tid].pending,
            Target::Process(pid) => &self.processes[&pid].pending,
        };
        if signal < SIGRTMIN && pending.set & bit(signal) != 0 {
            return Ok(());
        }
        let (queued, charge) = if timer {
            (true, None)
        } else {
            let charge = self.charge_signal(pid, signal, &info)?;
            (charge.is_some(), charge)
        };
        let pending = match target {
            Target::Thread(tid) => &mut self.thread_mut(tid).pending,
            Target::Process(pid) => &mut self.processes.get_mut(&pid).expect("process").pending,
        };
        pending.set |= bit(signal);
        if queued {
            pending.queue.push_back(Queued {
                signal,
                info,
                charge,
                timer,
            });
        }
        self.wake_readiness_of(pid);
        self.complete(cx, pid, target, signal);
        Ok(())
    }

    /// Linux's `__sigqueue_alloc` against the receiving user's RLIMIT_SIGPENDING: a place in the
    /// user's count, or none when the queue is full and the signal goes without its information.
    /// Standard signals from the kernel or `kill` always fit; a real-time one with information of
    /// its own that does not fit is not sent (EAGAIN). The kernel's own information
    /// (`SEND_SIG_PRIV`) is the one with SI_KERNEL.
    fn charge_signal(
        &mut self,
        pid: Pid,
        signal: u32,
        info: &[u8; 128],
    ) -> Result<Option<crate::process::Charge>, crate::Errno> {
        let uid = self.task_credentials(pid).uid;
        let process = &self.processes[&pid];
        let limit = process.limits[RLIMIT_SIGPENDING as usize].current;
        let count = self.signal_users.entry(uid).or_default().clone();
        let code = i32::from_le_bytes(info[8..12].try_into().expect("four bytes"));
        let special = code == SI_KERNEL as i32;
        let overrides = signal < SIGRTMIN && (special || code >= 0);
        if overrides || count.get() < limit {
            return Ok(Some(crate::process::Charge::new(&count)));
        }
        if signal >= SIGRTMIN && !special && code != SI_USER as i32 {
            return Err(crate::Errno::EAGAIN);
        }
        Ok(None)
    }

    /// Linux's `complete_signal`: a fatal signal that a thread does not block ends the process now;
    /// otherwise a thread that does not block it is woken to act on it.
    fn complete(&mut self, cx: &mut Context<'_>, pid: Pid, target: Target, signal: u32) {
        let candidates: alloc::vec::Vec<Tid> = match target {
            Target::Thread(tid) => alloc::vec![tid],
            Target::Process(_) => self.processes[&pid].threads.clone(),
        };
        let wanting = candidates
            .iter()
            .copied()
            .find(|tid| self.threads[tid].signal_mask & bit(signal) == 0);
        // Threads waiting for a signal they block (`sigtimedwait`) check their set again.
        for tid in &candidates {
            if self.threads[tid].call.wait == Some(Channel::Signals) {
                self.wake_thread(*tid);
            }
        }
        let Some(tid) = wanting else {
            return;
        };
        let handler = self.handler(pid, signal);
        let fatal = handler == SIG_DFL
            && matches!(default_action(signal), Default::Terminate | Default::Core)
            && !self.dropped(pid, signal);
        if fatal || signal == SIGKILL {
            self.drop_pending(pid, bit(signal));
            self.die_of(cx, pid, signal);
            return;
        }
        let wait = self.threads[&tid].call.wait;
        if wait.is_some_and(|channel| !matches!(channel, Channel::Backend | Channel::Held(_))) {
            self.wake_thread(tid);
        } else if wait.is_none() && self.running != Some(tid) && !self.signalled.contains(&tid) {
            self.signalled.push_back(tid);
        }
    }

    /// Drops the signals in `set` from every pending set of a process.
    fn drop_pending(&mut self, pid: Pid, set: u64) {
        self.processes
            .get_mut(&pid)
            .expect("process")
            .pending
            .drop_set(set);
        for tid in self.processes[&pid].threads.clone() {
            self.thread_mut(tid).pending.drop_set(set);
        }
    }

    /// Drops a signal from every pending set of a process, as setting an action that ignores it does.
    pub(crate) fn flush_pending(&mut self, pid: Pid, signal: u32) {
        self.drop_pending(pid, bit(signal));
    }

    /// Takes the next signal a thread does not block: its own before its process's, a fault's
    /// before others, then the lowest number (`dequeue_signal`, `next_signal`).
    pub(crate) fn dequeue_signal(&mut self, tid: Tid, allowed: u64) -> Option<Queued> {
        let pid = self.threads[&tid].process;
        let choose = |set: u64| -> Option<u32> {
            let ready = set & allowed;
            if ready == 0 {
                return None;
            }
            let first = if ready & SYNCHRONOUS != 0 {
                ready & SYNCHRONOUS
            } else {
                ready
            };
            Some(first.trailing_zeros() + 1)
        };
        loop {
            let mut queued = if let Some(signal) = choose(self.threads[&tid].pending.set) {
                self.thread_mut(tid).pending.take(signal)
            } else {
                let signal = choose(self.processes[&pid].pending.set)?;
                self.processes
                    .get_mut(&pid)
                    .expect("process")
                    .pending
                    .take(signal)
            }?;
            // A stale timer's signal is dropped and the next one taken.
            if crate::syscall::timer::taken(self, pid, &mut queued) {
                return Some(queued);
            }
        }
    }

    /// On a thread's way back to the guest, after a call (`resume` carries its answer), a stop or a
    /// fault: acts on the signals it does not block, as arm64's `do_signal` does. A call a signal
    /// interrupted is restarted or answers EINTR. Returns how the thread continues, or none if it
    /// stopped or is gone.
    pub(crate) fn deliver(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        resume: Resume,
    ) -> Option<Resume> {
        let mut restart = match resume {
            Resume::Return(value) => Errno::restart_of(value),
            _ => None,
        };
        let mut resume = resume;
        let mut handled = false;
        loop {
            let mask = self.threads[&tid].signal_mask;
            let Some(queued) = self.dequeue_signal(tid, !mask) else {
                break;
            };
            let signal = queued.signal;
            let pid = self.threads[&tid].process;
            let action = self.processes[&pid].actions.borrow()[signal as usize - 1];
            let handler = action.sa_handler_kernel;
            if handler == SIG_IGN {
                continue;
            }
            if handler == SIG_DFL {
                match default_action(signal) {
                    Default::Ignore | Default::Continue => continue,
                    _ if self.init == Some(pid) && !self.forced(tid, signal) => continue,
                    Default::Stop => {
                        // Terminal stops do nothing to an orphaned group (`is_current_pgrp_orphaned`).
                        if signal != SIGSTOP && self.group_orphaned(self.processes[&pid].group) {
                            continue;
                        }
                        self.thread_mut(tid).resume = resume;
                        self.stop_process(cx, pid, signal);
                        return None;
                    }
                    Default::Terminate | Default::Core => {
                        self.die_of(cx, pid, signal);
                        return None;
                    }
                }
            }
            // A handler runs: the interrupted call answers EINTR or is set to run again after it.
            let mut registers = RegisterBlock::ZERO;
            cx.cpu.registers(self.threads[&tid].cpu, &mut registers);
            if let Resume::Return(value) = resume {
                let original = registers.x[0];
                registers.x[0] = value;
                if let Some(code) = restart.take() {
                    let again = code == Errno::ERESTARTNOINTR
                        || (code == Errno::ERESTARTSYS
                            && action.sa_flags & u64::from(SA_RESTART) != 0);
                    if again {
                        registers.x[0] = original;
                        registers.pc -= 4;
                    } else {
                        registers.x[0] = Errno::EINTR.to_return();
                    }
                }
            }
            if self.rseq_fixup(cx, tid, &mut registers).is_err()
                || self
                    .setup_frame(cx, tid, &queued, &action, &mut registers)
                    .is_err()
            {
                // A frame that cannot be written is SIGSEGV (`force_sigsegv`).
                let pid = self.threads[&tid].process;
                if signal == SIGSEGV {
                    self.processes[&pid].actions.borrow_mut()[SIGSEGV as usize - 1]
                        .sa_handler_kernel = SIG_DFL;
                }
                self.die_of(cx, pid, SIGSEGV);
                return None;
            }
            cx.cpu.set_registers(self.threads[&tid].cpu, &registers);
            resume = Resume::FromRegisters;
            handled = true;
        }
        if let Some(code) = restart {
            // No handler ran: the call runs again, from `restart_syscall` for one that keeps state.
            let mut registers = RegisterBlock::ZERO;
            cx.cpu.registers(self.threads[&tid].cpu, &mut registers);
            registers.pc -= 4;
            if code == Errno::ERESTART_RESTARTBLOCK {
                registers.x[8] = u64::from(__NR_restart_syscall);
            }
            cx.cpu.set_registers(self.threads[&tid].cpu, &registers);
            resume = Resume::FromRegisters;
        }
        if !handled && let Some(saved) = self.thread_mut(tid).saved_mask.take() {
            self.thread_mut(tid).signal_mask = saved;
        }
        Some(resume)
    }

    /// A thread taken off the CPU while it ran its own code (`rseq_sched_switch_event`): inside its
    /// restartable sequence it continues at the abort handler. Returns how it continues, or none if
    /// a bad area killed its process.
    pub(crate) fn rseq_preempted(&mut self, cx: &mut Context<'_>, tid: Tid) -> Option<Resume> {
        if self.threads[&tid].rseq.is_none() {
            return Some(Resume::Continue);
        }
        let mut registers = RegisterBlock::ZERO;
        cx.cpu.registers(self.threads[&tid].cpu, &mut registers);
        let before = registers.pc;
        if self.rseq_fixup(cx, tid, &mut registers).is_err() {
            let pid = self.threads[&tid].process;
            self.die_of(cx, pid, SIGSEGV);
            return None;
        }
        if registers.pc == before {
            return Some(Resume::Continue);
        }
        cx.cpu.set_registers(self.threads[&tid].cpu, &registers);
        Some(Resume::FromRegisters)
    }

    /// Linux's `rseq_ip_fixup` before a handler runs: a thread inside its registered restartable
    /// sequence's critical section goes to the sequence's abort handler, whose signature must match;
    /// the area's current sequence is cleared. A bad area is an error (SIGSEGV).
    fn rseq_fixup(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        registers: &mut RegisterBlock,
    ) -> Result<(), Errno> {
        let Some((area, _, signature)) = self.threads[&tid].rseq else {
            return Ok(());
        };
        let pc = registers.pc;
        let mut guest = crate::syscall::guest(self, cx, tid);
        // `struct rseq`'s `rseq_cs` and `struct rseq_cs`'s fields.
        let sequence: u64 = guest.read_value(area + 8)?;
        if sequence == 0 {
            return Ok(());
        }
        let fields = guest.read(sequence, 32)?;
        let word = |at: usize| u64::from_le_bytes(fields[at..at + 8].try_into().expect("8 bytes"));
        let version = u32::from_le_bytes(fields[0..4].try_into().expect("4 bytes"));
        let (start, post_commit, abort) = (word(8), word(16), word(24));
        if version > 0
            || start
                .checked_add(post_commit)
                .is_none_or(|end| end > crate::user::ADDRESS_LIMIT)
        {
            return Err(Errno::EINVAL);
        }
        let inside = pc >= start && pc - start < post_commit;
        if inside {
            if abort >= start && abort - start < post_commit {
                return Err(Errno::EINVAL);
            }
            let found: u32 = guest.read_value(abort.wrapping_sub(4))?;
            if found != signature {
                return Err(Errno::EINVAL);
            }
        }
        guest.put(area + 8, &0u64.to_le_bytes())?;
        if inside {
            registers.pc = abort;
        }
        Ok(())
    }

    /// Whether a signal pending for the thread was forced (a fault's), which even the first process
    /// cannot ignore.
    fn forced(&self, tid: Tid, signal: u32) -> bool {
        self.threads[&tid].forced & bit(signal) != 0
    }

    /// Stops a process for job control (`do_signal_stop`): its threads stop where they are and its
    /// parent is told.
    fn stop_process(&mut self, cx: &mut Context<'_>, pid: Pid, signal: u32) {
        // Threads in line or on the CPU wait aside; parked ones stay parked, and any that finish
        // their call meanwhile wait aside too.
        for tid in self.processes[&pid].threads.clone() {
            self.take_out_of_line(tid);
            if self.current == Some(tid) {
                self.current = None;
            }
            self.signalled.retain(|other| *other != tid);
        }
        let process = self.processes.get_mut(&pid).expect("process");
        process.stopped = true;
        process.continued_unsent = false;
        self.report_to_parent(cx, pid, Report::Stopped(signal));
    }

    /// Continues a stopped process (SIGCONT's `prepare_signal`): its threads go back to acting on
    /// their signals and running; a wait sees it at once and the parent is sent SIGCHLD when one of
    /// its threads next comes back from running (`get_signal`), or now if every one is parked.
    fn continue_process(&mut self, cx: &mut Context<'_>, pid: Pid) {
        let process = self.processes.get_mut(&pid).expect("process");
        if !process.stopped {
            return;
        }
        process.stopped = false;
        process.report = Some(Report::Continued);
        let mut runs = false;
        for tid in process.threads.clone() {
            if self.threads[&tid].call.wait.is_none() {
                runs = true;
                if !self.signalled.contains(&tid) {
                    self.signalled.push_back(tid);
                }
            }
        }
        if runs {
            self.processes
                .get_mut(&pid)
                .expect("process")
                .continued_unsent = true;
        } else {
            self.tell_parent(cx, pid, Report::Continued);
        }
    }

    /// A thread back from running tells its parent of its process's continuing, if it has not been.
    pub(crate) fn tell_continued(&mut self, cx: &mut Context<'_>, tid: Tid) {
        let pid = self.threads[&tid].process;
        if core::mem::take(
            &mut self
                .processes
                .get_mut(&pid)
                .expect("process")
                .continued_unsent,
        ) {
            self.tell_parent(cx, pid, Report::Continued);
        }
    }

    /// Writes a handler's frame below the thread's stack (or on its alternate stack) and points the
    /// registers at the handler, as arm64's `setup_rt_frame` does.
    fn setup_frame(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        queued: &Queued,
        action: &kernel_sigaction,
        registers: &mut RegisterBlock,
    ) -> Result<(), Errno> {
        let signal = queued.signal;
        let flags = action.sa_flags;
        let thread = &self.threads[&tid];
        let (stack_base, stack_size, stack_flags) = thread.alternate_stack;
        let on_alternate = on_stack(thread.alternate_stack, registers.sp);
        let top = if flags & u64::from(SA_ONSTACK) != 0 && stack_size != 0 && !on_alternate {
            stack_base.wrapping_add(stack_size)
        } else {
            registers.sp
        };
        let record = (top.wrapping_sub(frame::RECORD_SIZE)) & !15;
        let at = (record & !15).wrapping_sub(frame::SIZE);
        if !crate::user::access_ok(at, top.wrapping_sub(at)) {
            return Err(Errno::EFAULT);
        }
        let mask = thread.saved_mask.unwrap_or(thread.signal_mask);
        let (fault_address, esr) = thread.fault;
        // The frame keeps the flags the stack was set with (`__save_altstack`).
        let stack_flags_saved = stack_flags;
        let mut writes = Writes::default();
        writes.u64(at + frame::UC_FLAGS, 0);
        writes.u64(at + frame::UC_LINK, 0);
        writes.u64(at + frame::UC_STACK, stack_base);
        writes.u32(at + frame::UC_STACK + 8, stack_flags_saved as u32);
        writes.u64(at + frame::UC_STACK + 16, stack_size);
        writes.u64(record, registers.x[29]);
        writes.u64(record + 8, registers.x[30]);
        for (index, value) in registers.x.iter().enumerate() {
            writes.u64(at + frame::REGS + 8 * index as u64, *value);
        }
        writes.u64(at + frame::SP, registers.sp);
        writes.u64(at + frame::PC, registers.pc);
        writes.u64(at + frame::PSTATE, registers.pstate);
        writes.u64(at + frame::FAULT_ADDRESS, fault_address);
        writes.u64(at + frame::UC_SIGMASK, mask);
        // The records: the vector registers, the fault's syndrome if it has one, then the end.
        let mut fpsimd = alloc::vec::Vec::with_capacity(frame::FPSIMD_SIZE as usize);
        fpsimd.extend_from_slice(&frame::FPSIMD_MAGIC.to_le_bytes());
        fpsimd.extend_from_slice(&frame::FPSIMD_SIZE.to_le_bytes());
        fpsimd.extend_from_slice(&registers.fpsr.to_le_bytes());
        fpsimd.extend_from_slice(&registers.fpcr.to_le_bytes());
        for vector in &registers.v {
            fpsimd.extend_from_slice(&vector[0].to_le_bytes());
            fpsimd.extend_from_slice(&vector[1].to_le_bytes());
        }
        let mut next = at + frame::RESERVED;
        writes.0.push((next, fpsimd));
        next += u64::from(frame::FPSIMD_SIZE);
        // A memory fault's status (whether the page was present, and at which level a table was
        // missing) is that of Linux's page tables, which are not kept here.
        let class = esr >> frame::ESR_EC_SHIFT;
        if class == frame::ESR_EC_DABT_LOW || class == frame::ESR_EC_IABT_LOW {
            self.counters
                .count(cx.host, reason::FAULT_SYNDROME_APPROXIMATED, esr);
        }
        if esr != 0 {
            let mut record = alloc::vec::Vec::with_capacity(16);
            record.extend_from_slice(&frame::ESR_MAGIC.to_le_bytes());
            record.extend_from_slice(&frame::ESR_SIZE.to_le_bytes());
            record.extend_from_slice(&esr.to_le_bytes());
            writes.0.push((next, record));
            next += u64::from(frame::ESR_SIZE);
        }
        writes.0.push((next, alloc::vec![0; 8]));
        if flags & u64::from(SA_SIGINFO) != 0 {
            writes.0.push((at, queued.info.to_vec()));
        }
        let mut guest = crate::syscall::guest(self, cx, tid);
        for (address, bytes) in &writes.0 {
            guest.write(*address, bytes)?;
        }
        // The handler starts with the signal in x0, the frame as its stack and the return through the
        // program's restorer or the vDSO's.
        let restorer = if flags & u64::from(SA_RESTORER) != 0 {
            action.sa_restorer
        } else {
            self.processes[&self.threads[&tid].process].vdso
                + crate::vdso::VVAR_PAGES * wcontainers_abi::PAGE_SIZE
                + crate::vdso::SIGTRAMP
        };
        registers.x[0] = u64::from(signal);
        registers.sp = at;
        registers.x[29] = record;
        registers.pc = action.sa_handler_kernel;
        registers.x[30] = restorer;
        if flags & u64::from(SA_SIGINFO) != 0 {
            registers.x[1] = at;
            registers.x[2] = at + frame::UC_FLAGS;
        }
        // `signal_setup_done`: the stack is given up if asked, the mask grows, a one-shot handler
        // goes back to the default.
        let thread = self.thread_mut(tid);
        if stack_flags & SS_AUTODISARM as i32 != 0 {
            thread.alternate_stack = (0, 0, 0);
        }
        thread.saved_mask = None;
        thread.forced &= !bit(signal);
        let mut blocked = action.sa_mask.sig[0];
        if flags & u64::from(SA_NODEFER) == 0 {
            blocked |= bit(signal);
        }
        thread.signal_mask |= blocked & !UNBLOCKABLE;
        if flags & u64::from(SA_RESETHAND) != 0 {
            let pid = thread.process;
            self.processes[&pid].actions.borrow_mut()[signal as usize - 1].sa_handler_kernel =
                SIG_DFL;
        }
        Ok(())
    }

    /// `rt_sigreturn`: puts back the registers, mask and alternate stack a handler's frame holds. A
    /// frame that is not one is SIGSEGV.
    pub(crate) fn sigreturn(&mut self, cx: &mut Context<'_>, tid: Tid) -> Outcome {
        let cpu = self.threads[&tid].cpu;
        let mut registers = RegisterBlock::ZERO;
        cx.cpu.registers(cpu, &mut registers);
        let at = registers.sp;
        match self.restore_frame(cx, tid, at, &mut registers) {
            Ok(()) => {
                cx.cpu.set_registers(cpu, &registers);
                Outcome::Restart
            }
            Err(()) => {
                self.segfault_at(tid, at);
                Outcome::Restart
            }
        }
    }

    fn restore_frame(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        at: u64,
        registers: &mut RegisterBlock,
    ) -> Result<(), ()> {
        if at & 15 != 0 {
            return Err(());
        }
        let mut guest = crate::syscall::guest(self, cx, tid);
        let bytes = guest.read(at, frame::SIZE).map_err(|_| ())?;
        let word = |offset: u64| {
            u64::from_le_bytes(
                bytes[offset as usize..offset as usize + 8]
                    .try_into()
                    .expect("eight bytes"),
            )
        };
        let half = |offset: u64| {
            u32::from_le_bytes(
                bytes[offset as usize..offset as usize + 4]
                    .try_into()
                    .expect("four bytes"),
            )
        };
        let mask = word(frame::UC_SIGMASK);
        for index in 0..31 {
            registers.x[index] = word(frame::REGS + 8 * index as u64);
        }
        registers.sp = word(frame::SP);
        registers.pc = word(frame::PC);
        let pstate = word(frame::PSTATE);
        // `valid_native_regs`: only the flags of a user mode state; anything else is a bad frame.
        let valid = pstate & !wcontainers_abi::pstate::NZCV == 0;
        registers.pstate = pstate & wcontainers_abi::pstate::NZCV;
        // `parse_user_sigframe`: the records up to the end, which must include the vectors.
        let mut offset = frame::RESERVED;
        let limit = frame::RESERVED + frame::RESERVED_SIZE;
        let mut fpsimd = None;
        loop {
            if limit - offset < 8 {
                return Err(());
            }
            let magic = half(offset);
            let size = u64::from(half(offset + 4));
            match magic {
                0 => {
                    if size != 0 {
                        return Err(());
                    }
                    break;
                }
                frame::FPSIMD_MAGIC => {
                    if fpsimd.is_some() || size != u64::from(frame::FPSIMD_SIZE) {
                        return Err(());
                    }
                    fpsimd = Some(offset);
                }
                frame::ESR_MAGIC => {}
                _ => return Err(()),
            }
            if size < 8 || limit - offset < size {
                return Err(());
            }
            offset += size;
        }
        let Some(fpsimd) = fpsimd else {
            return Err(());
        };
        registers.fpsr = half(fpsimd + 8);
        registers.fpcr = half(fpsimd + 12);
        for (index, vector) in registers.v.iter_mut().enumerate() {
            vector[0] = word(fpsimd + 16 + 16 * index as u64);
            vector[1] = word(fpsimd + 24 + 16 * index as u64);
        }
        if !valid {
            return Err(());
        }
        let thread = self.thread_mut(tid);
        thread.signal_mask = mask & !UNBLOCKABLE;
        thread.restart = None;
        // `restore_altstack`: the stack the frame names, errors other than a fault ignored.
        let stack = (
            word(frame::UC_STACK),
            word(frame::UC_STACK + 16),
            half(frame::UC_STACK + 8) as i32,
        );
        self.restore_alternate_stack(tid, stack, registers.sp);
        Ok(())
    }

    /// Sets the alternate stack from a frame, as `do_sigaltstack` would, ignoring a refusal.
    fn restore_alternate_stack(&mut self, tid: Tid, (base, size, flags): (u64, u64, i32), sp: u64) {
        let thread = self.thread_mut(tid);
        if on_stack(thread.alternate_stack, sp) {
            return;
        }
        let mode = flags & !(SS_AUTODISARM as i32);
        if mode == SS_DISABLE as i32 {
            thread.alternate_stack = (0, 0, flags);
        } else if (mode == 0 || mode == SS_ONSTACK as i32) && size >= MIN_SIGNAL_STACK {
            thread.alternate_stack = (base, size, flags);
        }
    }

    /// A fault the program's own handler may catch (`force_sig_fault`): a blocked or ignored signal
    /// is let through with the default action; the thread then acts on it on its way back.
    pub(crate) fn force_signal(&mut self, tid: Tid, info: [u8; 128], fault: (u64, u64)) {
        let signal = signal_of(&info);
        let pid = self.threads[&tid].process;
        let thread = self.thread_mut(tid);
        thread.fault = fault;
        let blocked = thread.signal_mask & bit(signal) != 0;
        if blocked {
            thread.signal_mask &= !bit(signal);
        }
        thread.forced |= bit(signal);
        {
            let action = &mut self.processes[&pid].actions.borrow_mut()[signal as usize - 1];
            if blocked || action.sa_handler_kernel == SIG_IGN {
                action.sa_handler_kernel = SIG_DFL;
            }
        }
        if self.threads[&tid].pending.set & bit(signal) == 0 {
            // A fault's information is the kernel's own and always fits.
            let charge = self.charge_signal(pid, signal, &info).ok().flatten();
            let pending = &mut self.thread_mut(tid).pending;
            pending.set |= bit(signal);
            pending.queue.push_back(Queued {
                signal,
                info,
                charge,
                timer: false,
            });
        }
    }

    /// A fault the stand-in reported: the signal Linux sends for it, acted on at once.
    pub(crate) fn force_fault(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        signal: u32,
        code: i32,
        address: u64,
    ) {
        let esr = match signal {
            SIGBUS if code == BUS_ADRALN as i32 => {
                let class = if address == self.thread_pc_now(cx, tid) {
                    frame::ESR_EC_PC_ALIGN
                } else {
                    frame::ESR_EC_SP_ALIGN
                };
                (class << frame::ESR_EC_SHIFT) | frame::ESR_IL
            }
            _ => 0,
        };
        // An alignment fault keeps no fault address in the frame (`arm64_notify_die`).
        self.force_signal(tid, fault_info(signal, code, address), (0, esr));
        self.return_to_user(cx, tid, Resume::FromRegisters);
    }

    fn thread_pc_now(&mut self, cx: &mut Context<'_>, tid: Tid) -> u64 {
        let mut registers = RegisterBlock::ZERO;
        cx.cpu.registers(self.threads[&tid].cpu, &mut registers);
        registers.pc
    }

    /// A fault after which the thread's registers are lost: the process dies of the signal.
    pub(crate) fn fatal_fault(&mut self, cx: &mut Context<'_>, tid: Tid, signal: u32) {
        let pid = self.threads[&tid].process;
        self.die_of(cx, pid, signal);
    }

    /// SIGSEGV at an address, as `arm64_notify_segfault` sends it for a bad frame.
    fn segfault_at(&mut self, tid: Tid, address: u64) {
        let mapped = self.process_of(tid).map.borrow().find(address).is_some();
        let code = if mapped { SEGV_ACCERR } else { SEGV_MAPERR };
        self.force_signal(tid, fault_info(SIGSEGV, code as i32, address), (0, 0));
    }

    /// A memory fault: the stack grows if that is what it is; otherwise the signal Linux sends, with
    /// the fault's address and an approximated syndrome (which translation level faulted is Linux's
    /// page tables' own).
    pub(crate) fn memory_fault(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        address: u64,
        access: Access,
    ) {
        let pid = self.threads[&tid].process;
        let may_lock_any = self.credentials(tid).has(CAP_IPC_LOCK);
        let process = self.processes.get_mut(&pid).expect("process exists");
        let mapping = process.map.borrow().find(address).cloned();
        // A first write to a private file page: it becomes the process's own copy.
        if access == Access::Write
            && mapping
                .as_ref()
                .is_some_and(|mapping| mapping.copies_on_write(address))
            && process
                .map
                .borrow_mut()
                .copy_on_write(cx.cpu, process.memory, address, address + 1)
                .is_ok()
        {
            self.return_to_user(cx, tid, Resume::FromRegisters);
            return;
        }
        let wanted = match access {
            Access::Read => wcontainers_abi::Permissions::READ,
            Access::Write => wcontainers_abi::Permissions::WRITE,
            Access::Execute => wcontainers_abi::Permissions::EXECUTE,
        };
        // A page that allows nothing has no valid entry: a translation fault even inside a mapping.
        let present = mapping
            .as_ref()
            .is_some_and(|mapping| !crate::memory::cpu_permissions(mapping.permissions).is_empty());
        let (signal, code) = match mapping {
            None => {
                if process
                    .map
                    .borrow_mut()
                    .grow_stack(
                        cx.cpu,
                        process.memory,
                        address,
                        &process.limits,
                        may_lock_any,
                    )
                    .unwrap_or(false)
                {
                    self.return_to_user(cx, tid, Resume::FromRegisters);
                    return;
                }
                (SIGSEGV, SEGV_MAPERR)
            }
            Some(mapping) => match mapping.backing {
                // Allowed by the mapping but past the end of its file.
                Backing::File { file_end, .. }
                    if address >= file_end
                        && crate::memory::cpu_permissions(mapping.permissions).contains(wanted) =>
                {
                    (SIGBUS, BUS_ADRERR)
                }
                _ => (SIGSEGV, SEGV_ACCERR),
            },
        };
        let class = if access == Access::Execute {
            frame::ESR_EC_IABT_LOW
        } else {
            frame::ESR_EC_DABT_LOW
        };
        let status = if present {
            frame::ESR_FSC_PERMISSION_3
        } else {
            frame::ESR_FSC_TRANSLATION_3
        };
        let write = if access == Access::Write {
            frame::ESR_WNR
        } else {
            0
        };
        let esr = (class << frame::ESR_EC_SHIFT) | frame::ESR_IL | write | status;
        self.force_signal(
            tid,
            fault_info(signal, code as i32, address),
            (address, esr),
        );
        self.return_to_user(cx, tid, Resume::FromRegisters);
    }

    /// Ends a process killed by a signal. A signal whose default action dumps core would have Linux
    /// write a core file and mark the wait status, when the core size limit allows one and the process
    /// is dumpable; neither is done here (counted).
    pub(crate) fn die_of(&mut self, cx: &mut Context<'_>, pid: Pid, signal: u32) {
        let process = &self.processes[&pid];
        if default_action(signal) == Default::Core
            && process.limits[RLIMIT_CORE as usize].current > 0
            && process.map.borrow().dumpable
        {
            self.counters
                .count(cx.host, reason::CORE_DUMP_NOT_WRITTEN, signal.into());
        }
        self.end_process(cx, pid, signal as i32);
    }
}

/// The smallest alternate signal stack ARM64 Linux accepts (`MINSIGSTKSZ`).
pub(crate) const MIN_SIGNAL_STACK: u64 = 5120;

/// Whether `sp` is on an alternate stack (`on_sig_stack`), unless the stack is given up on entry to a
/// handler (`SS_AUTODISARM`).
pub(crate) fn on_stack((base, size, flags): (u64, u64, i32), sp: u64) -> bool {
    flags & SS_AUTODISARM as i32 == 0 && sp > base && sp - base <= size
}

/// The bytes a frame is made of, each at its address, written in Linux's order.
#[derive(Default)]
struct Writes(alloc::vec::Vec<(u64, alloc::vec::Vec<u8>)>);

impl Writes {
    fn u64(&mut self, at: u64, value: u64) {
        self.0.push((at, value.to_le_bytes().to_vec()));
    }

    fn u32(&mut self, at: u64, value: u32) {
        self.0.push((at, value.to_le_bytes().to_vec()));
    }
}
