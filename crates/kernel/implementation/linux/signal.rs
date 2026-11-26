//! Signals: sending them to threads, processes and groups, acting on them on a thread's way back to
//! the guest, the frame a handler is given (laid out exactly as arm64 Linux lays it out), and the
//! return from a handler.

use wcontainers_abi::{Access, RegisterBlock, Resume};
use wcontainers_executor::Executor;

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

    /// Whether a thread has a signal it does not block waiting for it, or was taken off its kernel
    /// in its call (Linux's `signal_pending`).
    pub(crate) fn signal_pending(&self, tid: Tid) -> bool {
        let thread = &self.threads[&tid];
        let process = &self.processes[&thread.process];
        (thread.pending.set | process.pending.set) & !thread.signal_mask != 0
            || thread.suspended_in_call
    }

    /// Sends a signal to one thread (`tkill`, a fault, a timer); one that does not fit the user's
    /// queue is lost as [`Self::queue_to_thread`] says.
    pub(crate) fn send_to_thread<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        signal: u32,
        info: [u8; 128],
    ) {
        let _ = self.send(cx, Target::Thread(tid), signal, info);
    }

    /// As [`Self::send_to_thread`], telling the sender (EAGAIN) of a real-time signal with
    /// information of its own that does not fit the receiving user's queue.
    pub(crate) fn queue_to_thread<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        signal: u32,
        info: [u8; 128],
    ) -> Result<(), crate::Errno> {
        self.send(cx, Target::Thread(tid), signal, info)
    }

    /// A signal the embedder sent to thread `tid` of process `pid` through its handle, sent as
    /// `rt_tgsigqueueinfo` sends one; counted and not sent when it cannot be.
    pub(crate) fn send_from_embedder<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        pid: Pid,
        tid: Tid,
        info: crate::siginfo::Bytes,
    ) {
        let signal = signal_of(&info);
        // A task of the process taken off the kernel is still one of its tasks.
        let ours = match self.threads.get(&tid) {
            Some(thread) => thread.process == pid,
            None => self
                .processes
                .get(&pid)
                .is_some_and(|process| process.away.contains_key(&tid)),
        };
        if !ours
            || !(1..=_NSIG).contains(&signal)
            || self.queue_to_thread(cx, tid, signal, info).is_err()
        {
            self.counters
                .count(cx.host, reason::SIGNAL_NOT_SENT, u64::from(signal));
        }
    }

    /// SIGXFSZ for a write past the file size limit, as `send_sig` sends it.
    pub(crate) fn send_file_size_signal<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
    ) {
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
    pub(crate) fn send_to_process<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        pid: Pid,
        signal: u32,
        info: [u8; 128],
    ) {
        let _ = self.send(cx, Target::Process(pid), signal, info);
    }

    /// A POSIX timer's signal, to one thread or (with none) the process: it goes in the timer's own
    /// place in the user's count, taken when the timer was made, so it always fits.
    pub(crate) fn send_from_timer<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
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
        let process = &self.processes[&pid];
        let mask = match thread.or_else(|| process.threads.first().copied()) {
            Some(tid) => match self.threads.get(&tid) {
                Some(thread) => thread.signal_mask,
                None => process.away[&tid].signal_mask,
            },
            None => match process.away.values().next() {
                Some(away) => away.signal_mask,
                None => return false,
            },
        };
        mask & bit(signal) == 0 && self.dropped(pid, signal)
    }

    /// Whether `tid` is a task of process `pid`, on this kernel or taken off it.
    pub(crate) fn task_of(&self, pid: Pid, tid: Tid) -> bool {
        self.threads.contains_key(&tid)
            || self
                .processes
                .get(&pid)
                .is_some_and(|process| process.away.contains_key(&tid))
    }

    /// The caller's count of pipe pages (by its real user id), and whether it is free of the limits
    /// on them (`pipe_is_unprivileged_user`: neither CAP_SYS_RESOURCE nor CAP_SYS_ADMIN).
    pub(crate) fn pipe_pages(
        &mut self,
        tid: Tid,
    ) -> (
        alloc::sync::Arc<crossbeam_utils::atomic::AtomicCell<u64>>,
        bool,
    ) {
        let credentials = self.credentials(tid);
        let privileged = credentials.has(CAP_SYS_RESOURCE) || credentials.has(CAP_SYS_ADMIN);
        let uid = credentials.uid;
        (self.pipe_users.entry(uid).or_default().clone(), privileged)
    }

    /// Linux's `posixtimer_init_sigqueue`: a place in the calling user's count for a new timer's
    /// signal, or none (EAGAIN) when the count is at the caller's RLIMIT_SIGPENDING.
    pub(crate) fn timer_charge(&mut self, tid: Tid) -> Option<crate::process::Charge> {
        let uid = self.credentials(tid).uid;
        let limit = self.process_of(tid).limits[RLIMIT_SIGPENDING as usize].current;
        let count = self.signal_users.entry(uid).or_default().clone();
        (count.load() < limit).then(|| crate::process::Charge::new(&count))
    }

    /// As [`Self::send_to_process`], telling the sender as [`Self::queue_to_thread`] does.
    pub(crate) fn queue_to_process<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        pid: Pid,
        signal: u32,
        info: [u8; 128],
    ) -> Result<(), crate::Errno> {
        self.send(cx, Target::Process(pid), signal, info)
    }

    /// Linux's `send_sigio`: the I/O signal of an open file with `FASYNC` set, to its owner's tasks
    /// that its namer may signal (`sigio_perm`), for `reason` (`POLL_IN`, `POLL_OUT`, ...). With no
    /// signal chosen it is SIGIO from the kernel; otherwise it carries the band and the descriptor.
    pub(crate) fn send_io_signal<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        file: &crate::file::FileRef,
        reason: u32,
    ) {
        let (owner, fd) = {
            let file = file.borrow();
            match (file.owner, file.async_fd) {
                (Some(owner), Some(fd)) if file.flags & FASYNC != 0 && owner.pid != 0 => {
                    (owner, fd)
                }
                _ => return,
            }
        };
        self.signal_owner(cx, &owner, fd, reason);
    }

    /// Linux's `send_sigio` itself, for descriptor `fd` of a file with `owner`.
    pub(crate) fn signal_owner<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        owner: &crate::file::Owner,
        fd: i32,
        reason: u32,
    ) {
        let owner = *owner;
        for (target, pid) in self.owner_targets(&owner) {
            if !self.may_signal_owner(&owner, pid) {
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

    /// Linux's `send_sigurg`: SIGURG to an open file's owner's tasks that its namer may signal,
    /// with or without `FASYNC`; false if it has no owner.
    pub(crate) fn send_urgent_signal<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        file: &crate::file::FileRef,
    ) -> bool {
        let Some(owner) = file.borrow().owner.filter(|owner| owner.pid != 0) else {
            return false;
        };
        for (target, pid) in self.owner_targets(&owner) {
            if self.may_signal_owner(&owner, pid) {
                let _ = self.send(cx, target, SIGURG, kernel_info(SIGURG));
            }
        }
        true
    }

    /// The live tasks a file's owner names, with their processes.
    fn owner_targets(&self, owner: &crate::file::Owner) -> alloc::vec::Vec<(Target, Pid)> {
        use crate::file::OwnerKind;
        let live = |process: &crate::process::Process| process.ended.is_none();
        match owner.kind {
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
        }
    }

    /// `sigio_perm`: whether a file's owner, by who named it, may be sent its signals.
    fn may_signal_owner(&self, owner: &crate::file::Owner, pid: Pid) -> bool {
        let credentials = self.task_credentials(pid);
        owner.euid == 0
            || [owner.euid, owner.uid]
                .iter()
                .any(|user| *user == credentials.suid || *user == credentials.uid)
    }

    /// Sends a signal to every live process of a group; false if it has none.
    pub(crate) fn send_to_group<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
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
    fn send<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        target: Target,
        signal: u32,
        info: [u8; 128],
    ) -> Result<(), crate::Errno> {
        self.send_queued(cx, target, signal, info, false)
    }

    /// As `send`; a timer's signal (`timer`) is queued in the timer's place, not charged again.
    fn send_queued<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        target: Target,
        signal: u32,
        info: [u8; 128],
        timer: bool,
    ) -> Result<(), crate::Errno> {
        if signal == 0 {
            return Ok(());
        }
        // What the task named blocks; a task taken off the kernel keeps what it is sent, as a
        // stopped one does.
        let (pid, mask) = match target {
            Target::Thread(tid) => match self.threads.get(&tid) {
                Some(thread) => (thread.process, thread.signal_mask),
                None => {
                    let pid = self.away_process_of(tid).expect("a task of a process");
                    (pid, self.processes[&pid].away[&tid].signal_mask)
                }
            },
            Target::Process(pid) => {
                let process = &self.processes[&pid];
                match process.threads.first() {
                    Some(tid) => (pid, self.threads[tid].signal_mask),
                    None => match process.away.get(&pid).or(process.away.values().next()) {
                        Some(away) => (pid, away.signal_mask),
                        None => return Ok(()),
                    },
                }
            }
        };
        // `prepare_signal`: a process dumping core takes only SIGKILL, which ends the dump.
        if crate::syscall::coredump::writer(self, pid).is_some() && signal != SIGKILL {
            return Ok(());
        }
        if STOPPING & bit(signal) != 0 {
            self.drop_pending(pid, bit(SIGCONT));
        } else if signal == SIGCONT {
            self.drop_pending(pid, STOPPING);
            self.continue_process(cx, pid);
        }
        let blocked = mask & bit(signal) != 0;
        if !blocked && self.dropped(pid, signal) {
            // A wait for this signal still sees it (`sigtimedwait` blocks what it waits for).
            return Ok(());
        }
        // A standard signal already pending is not queued again (`legacy_queue`).
        if signal < SIGRTMIN && self.pending_of(target).set & bit(signal) != 0 {
            return Ok(());
        }
        let (queued, charge) = if timer {
            (true, None)
        } else {
            let charge = self.charge_signal(pid, signal, &info)?;
            (charge.is_some(), charge)
        };
        let pending = self.pending_of_mut(target);
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

    /// The process a task taken off this kernel belongs to.
    pub(crate) fn away_process_of(&self, tid: Tid) -> Option<Pid> {
        self.processes
            .values()
            .find(|process| process.away.contains_key(&tid))
            .map(|process| process.pid)
    }

    fn pending_of(&self, target: Target) -> &crate::process::Pending {
        match target {
            Target::Thread(tid) => match self.threads.get(&tid) {
                Some(thread) => &thread.pending,
                None => {
                    let pid = self.away_process_of(tid).expect("a task of a process");
                    &self.processes[&pid].away[&tid].pending
                }
            },
            Target::Process(pid) => &self.processes[&pid].pending,
        }
    }

    fn pending_of_mut(&mut self, target: Target) -> &mut crate::process::Pending {
        match target {
            Target::Thread(tid) if !self.threads.contains_key(&tid) => {
                let pid = self.away_process_of(tid).expect("a task of a process");
                let process = self.processes.get_mut(&pid).expect("process");
                &mut process.away.get_mut(&tid).expect("a task away").pending
            }
            Target::Thread(tid) => &mut self.thread_mut(tid).pending,
            Target::Process(pid) => &mut self.processes.get_mut(&pid).expect("process").pending,
        }
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
        if overrides || count.load() < limit {
            return Ok(Some(crate::process::Charge::new(&count)));
        }
        if signal >= SIGRTMIN && !special && code != SI_USER as i32 {
            return Err(crate::Errno::EAGAIN);
        }
        Ok(None)
    }

    /// Linux's `complete_signal`: a fatal signal that does not dump core and that a thread does not
    /// block ends the process when one of its threads next runs; otherwise a thread that does not
    /// block it is woken to act on it.
    fn complete<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        pid: Pid,
        target: Target,
        signal: u32,
    ) {
        let candidates: alloc::vec::Vec<Tid> = match target {
            Target::Thread(tid) if self.threads.contains_key(&tid) => alloc::vec![tid],
            Target::Thread(_) => alloc::vec::Vec::new(),
            Target::Process(_) => self.processes[&pid].threads.clone(),
        };
        // SIGKILL ends a process whose tasks are taken off the kernel, as it ends a stopped one;
        // any other signal waits for a task to come back (`wants_signal`).
        if candidates.is_empty() {
            if signal == SIGKILL {
                self.drop_pending(pid, bit(signal));
                self.die_of(cx, pid, signal);
            }
            return;
        }
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
        // One that dumps core is left to the thread that takes it, as Linux leaves it to
        // `get_signal`.
        let handler = self.handler(pid, signal);
        let fatal = handler == SIG_DFL
            && default_action(signal) == Default::Terminate
            && !self.dropped(pid, signal);
        if fatal || signal == SIGKILL {
            self.drop_pending(pid, bit(signal));
            self.die_when_met(pid, signal);
            return;
        }
        let wait = self.threads[&tid].call.wait;
        if wait.is_some_and(|channel| {
            !matches!(
                channel,
                Channel::Backend | Channel::Held(_) | Channel::Watch(_)
            )
        }) {
            self.wake_thread(tid);
        } else if wait.is_none() && self.running != Some(tid) && !self.signalled.contains(&tid) {
            self.signalled.push_back(tid);
        }
    }

    /// Whether a fatal signal is ending process `pid`.
    pub(crate) fn is_dying(&self, pid: Pid) -> bool {
        self.dying.contains_key(&pid)
    }

    /// Leaves a fatal signal to the process's own threads, as Linux's threads end themselves: each
    /// is put in line, and the first whose turn comes meets the watches on what the kernel stores
    /// at the end, then ends the process. None of them runs its program or a call again.
    fn die_when_met(&mut self, pid: Pid, signal: u32) {
        self.dying.insert(pid, signal);
        self.processes.get_mut(&pid).expect("process").stopped = false;
        for tid in self.processes[&pid].threads.clone() {
            match self.threads[&tid].call.wait {
                // It goes on once the watches change.
                Some(Channel::Watch(_)) => {}
                Some(_) => self.wake_thread(tid),
                None if self.current != Some(tid) => {
                    let resume = self.threads[&tid].resume;
                    self.make_runnable(tid, resume);
                }
                None => {}
            }
        }
    }

    /// Drops the signals in `set` from every pending set of a process.
    fn drop_pending(&mut self, pid: Pid, set: u64) {
        let process = self.processes.get_mut(&pid).expect("process");
        process.pending.drop_set(set);
        for away in process.away.values_mut() {
            away.pending.drop_set(set);
        }
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
        let choose = |set: u64| first_ready(set, allowed);
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
    pub(crate) fn deliver<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        resume: Resume,
    ) -> Option<Resume> {
        if !self.dying.is_empty() {
            let pid = self.threads[&tid].process;
            if let Some(&signal) = self.dying.get(&pid) {
                if self.watched(tid)
                    && !self.walk_watched(cx, |kernel, cx| kernel.ends_touched(cx, tid))
                {
                    self.wait_on_watch(tid, Some(resume), true);
                } else {
                    self.die_of(cx, pid, signal);
                }
                return None;
            }
        }
        let mut restart = match resume {
            Resume::Return(value) => Errno::restart_of(value),
            _ => None,
        };
        let mut resume = resume;
        // Taken off the CPU in its own code: its restartable sequence first.
        if self.threads[&tid].rseq_event {
            if self.watched(tid)
                && !self.walk_watched(cx, |kernel, cx| kernel.rseq_touched(cx, tid))
            {
                self.wait_on_watch(tid, Some(resume), true);
                return None;
            }
            self.thread_mut(tid).rseq_event = false;
            if self.rseq_preempted(cx, tid)? == Resume::FromRegisters {
                resume = Resume::FromRegisters;
            }
        }
        let mut handled = false;
        loop {
            let pid = self.threads[&tid].process;
            let queued = match self.thread_mut(tid).signal_taken.take() {
                Some(queued) => queued,
                None => {
                    let mask = self.threads[&tid].signal_mask;
                    let Some(queued) = self.dequeue_signal(tid, !mask) else {
                        break;
                    };
                    let signal = crate::siginfo::decode(&queued.info);
                    self.announce(cx, pid, |system, process| {
                        system.on_signal_delivering(process, crate::system::task_id(tid), &signal)
                    });
                    queued
                }
            };
            let signal = queued.signal;
            if self.watched(tid)
                && !self.walk_watched(cx, |kernel, cx| kernel.frame_touched(cx, tid, signal))
            {
                self.thread_mut(tid).signal_taken = Some(queued);
                self.wait_on_watch(tid, Some(resume), true);
                return None;
            }
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
                    Default::Terminate => {
                        self.die_of(cx, pid, signal);
                        return None;
                    }
                    Default::Core => {
                        // The dump shows the registers as arm64's `do_signal` left them before
                        // `get_signal`: an interrupted call to be restarted is set to run again,
                        // another call's answer is in x0.
                        if let Resume::Return(value) = resume {
                            let mut registers = RegisterBlock::ZERO;
                            cx.cpu
                                .read_registers(self.threads[&tid].cpu, &mut registers);
                            if restart.is_some() {
                                registers.pc -= 4;
                            } else {
                                registers.x[0] = value;
                            }
                            cx.cpu.set_registers(self.threads[&tid].cpu, &registers);
                        }
                        self.die_with(cx, tid, signal, queued.info);
                        return None;
                    }
                }
            }
            // A handler runs: the interrupted call answers EINTR or is set to run again after it.
            let mut registers = RegisterBlock::ZERO;
            cx.cpu
                .read_registers(self.threads[&tid].cpu, &mut registers);
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
                self.die_with(cx, tid, SIGSEGV, kernel_info(SIGSEGV));
                return None;
            }
            cx.cpu.set_registers(self.threads[&tid].cpu, &registers);
            resume = Resume::FromRegisters;
            handled = true;
        }
        if let Some(code) = restart {
            // No handler ran: the call runs again, from `restart_syscall` for one that keeps state.
            let mut registers = RegisterBlock::ZERO;
            cx.cpu
                .read_registers(self.threads[&tid].cpu, &mut registers);
            registers.pc -= 4;
            if code == Errno::ERESTART_RESTARTBLOCK {
                registers.x[wcontainers_abi::SYSTEM_CALL_NUMBER_REGISTER] =
                    u64::from(__NR_restart_syscall);
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
    pub(crate) fn rseq_preempted<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
    ) -> Option<Resume> {
        if self.threads[&tid].rseq.is_none() {
            return Some(Resume::Continue);
        }
        let mut registers = RegisterBlock::ZERO;
        cx.cpu
            .read_registers(self.threads[&tid].cpu, &mut registers);
        let before = registers.pc;
        if self.rseq_fixup(cx, tid, &mut registers).is_err() {
            self.die_with(cx, tid, SIGSEGV, kernel_info(SIGSEGV));
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
    fn rseq_fixup<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        registers: &mut RegisterBlock,
    ) -> Result<(), Errno> {
        let Some((area, _, signature)) = self.threads[&tid].rseq else {
            return Ok(());
        };
        let pc = registers.pc;
        let mut guest = crate::syscall::guest(self, cx, tid);
        let sequence: u64 = guest.read_value(area + RSEQ_CS_FIELD)?;
        if sequence == 0 {
            return Ok(());
        }
        let fields: crate::linux_extra::rseq_cs = guest.read_value(sequence)?;
        let (version, start, post_commit, abort) = (
            fields.version,
            fields.start_ip,
            fields.post_commit_offset,
            fields.abort_ip,
        );
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
        guest.put(area + RSEQ_CS_FIELD, &0u64.to_le_bytes())?;
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
    fn stop_process<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        pid: Pid,
        signal: u32,
    ) {
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
    fn continue_process<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        pid: Pid,
    ) {
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
    pub(crate) fn tell_continued<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
    ) {
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
    fn setup_frame<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        queued: &Queued,
        action: &kernel_sigaction,
        registers: &mut RegisterBlock,
    ) -> Result<(), Errno> {
        let signal = queued.signal;
        let flags = action.sa_flags;
        let thread = &self.threads[&tid];
        let (stack_base, stack_size, stack_flags) = thread.alternate_stack;
        let (at, record, top) = frame_bounds(thread.alternate_stack, flags, registers.sp);
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
    pub(crate) fn sigreturn<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
    ) -> Outcome {
        let cpu = self.threads[&tid].cpu;
        let mut registers = RegisterBlock::ZERO;
        cx.cpu.read_registers(cpu, &mut registers);
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

    fn restore_frame<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
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
    pub(crate) fn force_fault<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
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

    fn thread_pc_now<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
    ) -> u64 {
        let mut registers = RegisterBlock::ZERO;
        cx.cpu
            .read_registers(self.threads[&tid].cpu, &mut registers);
        registers.pc
    }

    /// SIGSEGV at an address, as `arm64_notify_segfault` sends it for a bad frame.
    fn segfault_at(&mut self, tid: Tid, address: u64) {
        let mapped = self.process_of(tid).map.borrow().find(address).is_some();
        let code = if mapped { SEGV_ACCERR } else { SEGV_MAPERR };
        self.force_signal(tid, fault_info(SIGSEGV, code as i32, address), (0, 0));
    }

    /// Meets the embedder's watches on what acting on the signal taken touches: the frame and
    /// restartable sequence of a handler, or the thread ends of a fatal one; false if the thread
    /// must wait.
    fn frame_touched<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        signal: u32,
    ) -> bool {
        let thread = &self.threads[&tid];
        let pid = thread.process;
        let action = self.processes[&pid].actions.borrow()[signal as usize - 1];
        if action.sa_handler_kernel == SIG_IGN {
            return true;
        }
        if action.sa_handler_kernel == SIG_DFL {
            let fatal = matches!(default_action(signal), Default::Terminate | Default::Core)
                && (self.init != Some(pid) || self.forced(tid, signal));
            return !fatal || self.ends_touched(cx, tid);
        }
        let mut registers = RegisterBlock::ZERO;
        cx.cpu.read_registers(thread.cpu, &mut registers);
        let (at, _, top) = frame_bounds(thread.alternate_stack, action.sa_flags, registers.sp);
        if crate::user::access_ok(at, top.wrapping_sub(at)) {
            let mut memory = crate::system::CallMemory::new(self, cx, tid);
            if !memory.touch(at, top - at, crate::system::AccessKind::Write) {
                return false;
            }
        }
        self.rseq_touched(cx, tid)
    }

    /// Meets the embedder's watches on what the ends of thread `tid`'s process store; false if the
    /// thread must wait.
    fn ends_touched<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
    ) -> bool {
        let ends = self.thread_ends(tid, crate::syscall::touch::Ending::Process);
        let mut memory = crate::system::CallMemory::new(self, cx, tid);
        crate::syscall::touch::thread_ends(&mut memory, &ends)
    }

    /// Meets the embedder's watches on what `rseq_fixup` touches; false if the thread must wait.
    fn rseq_touched<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
    ) -> bool {
        use crate::system::AccessKind::{Read, Write};
        let Some((area, _, _)) = self.threads[&tid].rseq else {
            return true;
        };
        let mut registers = RegisterBlock::ZERO;
        cx.cpu
            .read_registers(self.threads[&tid].cpu, &mut registers);
        let mut memory = crate::system::CallMemory::new(self, cx, tid);
        let field = area + RSEQ_CS_FIELD;
        let mut sequence = [0; size_of::<u64>()];
        if !memory.touch(field, sequence.len() as u64, Write) {
            return false;
        }
        if !memory.read(field, &mut sequence) || u64::from_le_bytes(sequence) == 0 {
            return true;
        }
        let sequence = u64::from_le_bytes(sequence);
        let mut fields = [0; RSEQ_CS_SIZE];
        if !memory.touch(sequence, RSEQ_CS_SIZE as u64, Read) {
            return false;
        }
        if !memory.read(sequence, &mut fields) {
            return true;
        }
        let section =
            <crate::linux_extra::rseq_cs as zerocopy::FromBytes>::read_from_bytes(&fields)
                .expect("the size of a section");
        let (start, post_commit, abort) = (
            section.start_ip,
            section.post_commit_offset,
            section.abort_ip,
        );
        let signature = size_of::<u32>() as u64;
        registers.pc.wrapping_sub(start) >= post_commit
            || memory.touch(abort.wrapping_sub(signature), signature, Read)
    }

    /// A fault the embedder's watches or its record of written pages made, on an access the page's
    /// mapping allows: the hook, or the page noted as written. Whether it was such a fault.
    fn embedder_fault<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        address: u64,
        access: Access,
    ) -> bool {
        use crate::system::WatchesFor;
        let pid = self.threads[&tid].process;
        // The fault names an access's first byte; the access reached into the next page only when
        // the CPU side allows it all of the first.
        let last = address.saturating_add(LARGEST_ACCESS - 1);
        let (watch, at, held_here) = {
            let map = self.processes[&pid].map.borrow();
            let page = crate::memory::page_down(address);
            let held_here = map.watches.holds_back_write(address)
                || map
                    .find(address)
                    .is_some_and(|mapping| mapping.copies_on_write(address))
                || map.readable_whole(page..page + wcontainers_abi::PAGE_SIZE)
                    == Err(crate::system::MemoryError::NotRead);
            match map.watches.watch_at(tid, address) {
                WatchesFor::None if !held_here => (map.watches.watch_at(tid, last), last, false),
                watch => (watch, address, true),
            }
        };
        // A page whose reading the CPU side allows faults only on a write, whatever the CPU side
        // calls it (an atomic's read and write in one).
        let touch = match (access, watch) {
            (Access::Read, WatchesFor::Writes) | (Access::Write, _) => {
                crate::system::AccessKind::Write
            }
            (Access::Read, _) => crate::system::AccessKind::Read,
            (Access::Execute, _) => crate::system::AccessKind::Execute,
        };
        let watched = match watch {
            WatchesFor::All => true,
            WatchesFor::Writes => touch == crate::system::AccessKind::Write,
            WatchesFor::None => false,
        };
        if watched {
            if self.watch_touched(cx, tid, pid, at, touch) {
                self.return_to_user(cx, tid, Resume::FromRegisters);
            } else {
                self.wait_on_watch(tid, Some(Resume::FromRegisters), false);
            }
            return true;
        }
        let process = &self.processes[&pid];
        // The access's next page only once its first page is all allowed.
        if access != Access::Execute && {
            let mut map = process.map.borrow_mut();
            map.written_fault(cx.cpu, process.memory, address)
                || (!held_here
                    && crate::memory::page_down(last) != crate::memory::page_down(address)
                    && map.written_fault(cx.cpu, process.memory, last))
        } {
            self.return_to_user(cx, tid, Resume::FromRegisters);
            return true;
        }
        false
    }

    /// A memory fault: the stack grows if that is what it is; otherwise the signal Linux sends, with
    /// the fault's address and an approximated syndrome (which translation level faulted is Linux's
    /// page tables' own).
    pub(crate) fn memory_fault<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        address: u64,
        access: Access,
    ) {
        let pid = self.threads[&tid].process;
        let may_lock_any = self.credentials(tid).has(CAP_IPC_LOCK);
        let wanted = match access {
            Access::Read => wcontainers_abi::Permissions::READ,
            Access::Write => wcontainers_abi::Permissions::WRITE,
            Access::Execute => wcontainers_abi::Permissions::EXECUTE,
        };
        // A guard page has no entry and never gets one (`handle_pte_marker`'s VM_FAULT_SIGSEGV).
        let guarded = self.processes[&pid].map.borrow().guarded(address);
        let mapping = if guarded {
            None
        } else {
            self.processes[&pid].map.borrow().find(address).cloned()
        };
        let allowed = mapping.as_ref().is_some_and(|mapping| {
            crate::memory::cpu_permissions(mapping.permissions).contains(wanted)
        });
        if self.watching && allowed && self.embedder_fault(cx, tid, address, access) {
            return;
        }
        // A page of a backend file the mapping allows this access to, read when first touched.
        if let Some(mapping) = &mapping
            && let Backing::File {
                node,
                offset,
                file_end,
                ..
            } = &mapping.backing
            && address < *file_end
            && crate::memory::cpu_permissions(mapping.permissions).contains(wanted)
            && crate::fs::backend::remote(node).is_some()
        {
            let page = (offset + (address - mapping.start)) / wcontainers_abi::PAGE_SIZE;
            if !crate::shared::unfilled(node, page..page + 1).is_empty() {
                // Linux's read-around: the pages about the one touched (`do_sync_mmap_readahead`).
                let around = crate::shared::readahead_pages(node);
                let first = page.saturating_sub(around / 2);
                self.begin_asking(tid);
                let result = crate::shared::fill(node, first..(first + around).max(page + 1));
                self.end_asking(tid, result == Err(Errno::PARKED));
                match result {
                    Err(Errno::PARKED) => {
                        let thread = self.thread_mut(tid);
                        thread.filling = true;
                        thread.resume = Resume::FromRegisters;
                        return;
                    }
                    Err(_) => {
                        self.force_signal(
                            tid,
                            fault_info(SIGBUS, BUS_ADRERR as i32, address),
                            (address, 0),
                        );
                        self.return_to_user(cx, tid, Resume::FromRegisters);
                        return;
                    }
                    Ok(()) => {}
                }
            }
            let process = &self.processes[&pid];
            if !mapping.copies_on_write(address) || access != Access::Write {
                // The pages read around it, as far as the mapping reaches.
                let around = crate::shared::readahead_pages(node) * wcontainers_abi::PAGE_SIZE;
                let refreshed = process.map.borrow_mut().refresh(
                    cx.cpu,
                    process.memory,
                    address.saturating_sub(around / 2).max(mapping.start),
                    address.saturating_add(around),
                );
                if refreshed.is_ok() {
                    self.return_to_user(cx, tid, Resume::FromRegisters);
                    return;
                }
            }
        }
        let process = self.processes.get_mut(&pid).expect("process exists");
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
        // A page that allows nothing has no valid entry: a translation fault even inside a mapping.
        let present = mapping
            .as_ref()
            .is_some_and(|mapping| !crate::memory::cpu_permissions(mapping.permissions).is_empty());
        let (signal, code) = match mapping {
            None => {
                if !guarded
                    && process
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

    /// Ends thread `tid`'s process, killed by `signal` with `info`: one whose default action dumps
    /// core writes one first where the process may (`do_coredump`).
    pub(crate) fn die_with<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        tid: Tid,
        signal: u32,
        info: [u8; 128],
    ) {
        let pid = self.threads[&tid].process;
        if default_action(signal) == Default::Core && crate::syscall::coredump::wanted(self, pid) {
            crate::syscall::coredump::start(self, cx, tid, signal, info);
        } else {
            self.end_process(cx, pid, signal as i32);
        }
    }

    /// Ends a process killed by a signal where no thread's registers are left to dump: a signal
    /// whose default action dumps core would have Linux write a core file and mark the wait
    /// status, when the core size limit allows one and the process is dumpable; neither is done
    /// here (counted).
    pub(crate) fn die_of<S: crate::System, C: Executor>(
        &mut self,
        cx: &mut Context<'_, S, C>,
        pid: Pid,
        signal: u32,
    ) {
        // A dump SIGKILL stops ends the process with the dump's signal and no core bit.
        if let Some(tid) = crate::syscall::coredump::writer(self, pid) {
            let dumped = self.dumps.remove(&tid).expect("a dump").signal();
            self.end_process(cx, pid, dumped as i32);
            return;
        }
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

/// The lowest signal of `set` that is `allowed`, the synchronous ones first, as Linux's
/// `next_signal` picks.
fn first_ready(set: u64, allowed: u64) -> Option<u32> {
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
}

/// Where a handler's frame goes for a thread with this alternate stack, the handler's flags and
/// stack pointer: its start, its record of the frame pointer and link, and the stack's top.
fn frame_bounds(alternate_stack: (u64, u64, i32), flags: u64, sp: u64) -> (u64, u64, u64) {
    let (stack_base, stack_size, _) = alternate_stack;
    let on_alternate = on_stack(alternate_stack, sp);
    let top = if flags & u64::from(SA_ONSTACK) != 0 && stack_size != 0 && !on_alternate {
        stack_base.wrapping_add(stack_size)
    } else {
        sp
    };
    let aligned = |address: u64| address & !(STACK_ALIGNMENT - 1);
    let record = aligned(top.wrapping_sub(frame::RECORD_SIZE));
    let at = aligned(record).wrapping_sub(frame::SIZE);
    (at, record, top)
}

/// What ARM64 keeps its stack pointer a multiple of (`sp_el0` alignment checking).
pub(crate) use wcontainers_abi::STACK_ALIGNMENT;

/// The most bytes one ARM64 instruction accesses: four vector registers (`st4`, `ld4`).
const LARGEST_ACCESS: u64 = 4 * size_of::<u128>() as u64;

/// Where `struct rseq` holds its current sequence's address.
const RSEQ_CS_FIELD: u64 = core::mem::offset_of!(crate::linux_extra::rseq, rseq_cs) as u64;
const RSEQ_CS_SIZE: usize = size_of::<crate::linux_extra::rseq_cs>();
