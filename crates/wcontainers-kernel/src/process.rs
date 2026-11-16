//! Processes and threads.

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::vec::Vec;

use wcontainers_abi::{MemoryId, Resume, ThreadId};

use crate::credentials::Credentials;
use crate::file::FdTable;
use crate::fs::Node;
use crate::linux::general::kernel_sigaction;
use crate::memory::AddressMap;

pub(crate) type Pid = i32;
pub(crate) type Tid = i32;

/// Linux's number of resource limits (`RLIM_NLIMITS`).
pub(crate) const LIMITS: usize = 16;

/// Linux's number of signals on ARM64.
pub(crate) const SIGNALS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Limit {
    pub current: u64,
    pub maximum: u64,
}

/// The resource limits a default Docker container's first process starts with.
pub(crate) fn docker_limits() -> [Limit; LIMITS] {
    use crate::linux::general::*;
    let unlimited = Limit {
        current: RLIM_INFINITY as u64,
        maximum: RLIM_INFINITY as u64,
    };
    let mut limits = [unlimited; LIMITS];
    let set = |limits: &mut [Limit; LIMITS], resource: u32, current: u64, maximum: u64| {
        limits[resource as usize] = Limit { current, maximum };
    };
    set(&mut limits, RLIMIT_STACK, 8 << 20, RLIM_INFINITY as u64);
    set(&mut limits, RLIMIT_CORE, 0, RLIM_INFINITY as u64);
    set(&mut limits, RLIMIT_NOFILE, 1 << 20, 1 << 20);
    set(&mut limits, RLIMIT_NICE, 0, 0);
    set(&mut limits, RLIMIT_RTPRIO, 0, 0);
    set(&mut limits, RLIMIT_SIGPENDING, 128_302, 128_302);
    set(&mut limits, RLIMIT_MSGQUEUE, 819_200, 819_200);
    limits
}

/// What `/proc/<pid>/cmdline`, `environ` and `auxv` read from: ranges of the starting stack, and the
/// auxiliary vector as the kernel keeps it (`saved_auxv`).
#[derive(Clone, Debug, Default)]
pub(crate) struct StartingStack {
    pub arguments: (u64, u64),
    pub environment: (u64, u64),
    pub auxiliary: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct Process {
    pub pid: Pid,
    pub parent: Pid,
    pub group: Pid,
    pub session: Pid,
    pub memory: MemoryId,
    pub map: AddressMap,
    pub threads: Vec<Tid>,
    pub files: FdTable,
    pub root: Node,
    pub cwd: Node,
    pub umask: u32,
    pub credentials: Credentials,
    pub limits: [Limit; LIMITS],
    /// The name `comm` shows, at most 15 bytes.
    pub name: Vec<u8>,
    /// The program file and the path it was started by.
    pub executable: (Node, Vec<u8>),
    pub starting_stack: StartingStack,
    /// `personality`'s value; kept across `execve`.
    pub personality: u32,
    /// `PR_SET_DUMPABLE`; cleared when the process's ids change.
    pub dumpable: bool,
    /// Signal handlers as `rt_sigaction` set them, by signal number less one.
    pub actions: [kernel_sigaction; SIGNALS],
    /// `PR_SET_CHILD_SUBREAPER`: orphaned descendants come to this process.
    pub child_subreaper: bool,
    /// `PR_SET_THP_DISABLE`'s value as `PR_GET_THP_DISABLE` reports it; kept across `execve`.
    pub thp_disable: u64,
    /// Signals sent to the whole process and not yet taken by one of its threads.
    pub pending: Pending,
    /// The signal the parent is sent when this process ends (clone's low byte); 0 for none.
    pub exit_signal: u32,
    /// Once every thread has ended: the wait status a parent's wait will reap (a zombie).
    pub ended: Option<i32>,
    /// What a zombie's first thread keeps until it is reaped.
    pub ended_leader: Leader,
    /// The inode its process descriptors share, once one is made.
    pub pidfd: Option<Node>,
    /// A `vfork` child: the parent's thread that waits until the child lets go of the memory it
    /// borrows by starting a program or ending.
    pub vfork_parent: Option<Tid>,
    /// Job control: whether the process is stopped, and what a parent's wait has yet to see.
    pub stopped: bool,
    pub report: Option<Report>,
    /// CPU time of threads that have ended, and of reaped children with their own reaped children
    /// (`cutime`), in nanoseconds.
    pub ended_time: u64,
    pub children_time: u64,
    /// The largest memory of reaped children, for `getrusage(RUSAGE_CHILDREN)`.
    pub children_maxrss: u64,
    /// The memory mapped when the process ended, as its largest.
    pub peak_mapped: u64,
    /// Where the process's time starts on the monotonic clock.
    pub started: u64,
    /// Where the vDSO is mapped; its signal return code is there.
    pub vdso: u64,
    /// A change of user left the new user over RLIMIT_NPROC (`PF_NPROC_EXCEEDED`), which the next
    /// `execve` refuses if it still is.
    pub nproc_exceeded: bool,
    /// Whether it has started a program since it was made (`setpgid` from its parent is refused).
    pub execed: bool,
    /// How many programs it has run (Linux's `self_exec_id`, which a fork copies).
    pub self_exec_id: u64,
    /// The session's controlling terminal, if the process has one: a pair whose slave it is.
    pub terminal: Option<Rc<core::cell::RefCell<crate::tty::Pty>>>,
    /// For a session leader whose terminal was hung up, the foreground group it had
    /// (`tty_old_pgrp`), which is sent SIGHUP when the leader ends.
    pub terminal_old_group: Option<Pid>,
}

/// A change in a child's state that `wait` with `WUNTRACED` or `WCONTINUED` reports once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Report {
    Stopped(u32),
    Continued,
}

/// A signal waiting to be acted on, with the `siginfo` a handler or `sigtimedwait` is given.
#[derive(Clone)]
pub(crate) struct Queued {
    pub signal: u32,
    pub info: [u8; 128],
    /// Its place in the count of the receiving user's queued signals, held until it goes; none for
    /// a signal whose information was lost to a full queue.
    #[allow(dead_code, reason = "held for what its drop does")]
    pub charge: Option<Charge>,
}

/// One queued signal in a user's count (Linux's `UCOUNT_RLIMIT_SIGPENDING`), which
/// RLIMIT_SIGPENDING bounds: the count goes up when one is made or copied and down when it goes.
pub(crate) struct Charge(Rc<core::cell::Cell<u64>>);

impl Charge {
    pub fn new(count: &Rc<core::cell::Cell<u64>>) -> Self {
        count.set(count.get() + 1);
        Self(count.clone())
    }
}

impl Clone for Charge {
    fn clone(&self) -> Self {
        Self::new(&self.0)
    }
}

impl Drop for Charge {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

/// Signals sent and not yet taken: one set, and their information in the order sent. A standard
/// signal is queued once however often it is sent; real-time ones each time.
#[derive(Clone, Default)]
pub(crate) struct Pending {
    pub set: u64,
    pub queue: VecDeque<Queued>,
}

impl Pending {
    /// Takes the first queued instance of `signal`, clearing its bit when no other is left. A
    /// signal pending with no information left (its queue was full) comes as if sent by `kill` from
    /// nobody (`collect_signal`).
    pub fn take(&mut self, signal: u32) -> Option<Queued> {
        let bit = crate::signal::bit(signal);
        if self.set & bit == 0 {
            return None;
        }
        let Some(index) = self.queue.iter().position(|queued| queued.signal == signal) else {
            self.set &= !bit;
            return Some(Queued {
                signal,
                info: crate::signal::sender_info(
                    signal,
                    crate::linux::general::SI_USER as i32,
                    0,
                    0,
                ),
                charge: None,
            });
        };
        let queued = self.queue.remove(index);
        if !self.queue.iter().any(|other| other.signal == signal) {
            self.set &= !crate::signal::bit(signal);
        }
        queued
    }

    /// Drops every instance of the signals in `set`.
    pub fn drop_set(&mut self, set: u64) {
        self.set &= !set;
        self.queue
            .retain(|queued| set & crate::signal::bit(queued.signal) == 0);
    }
}

/// Linux's `TASK_COMM_LEN` less the terminating zero.
const NAME_LENGTH: usize = 15;

impl Process {
    /// Sets the `comm` name from a path's last part, as `execve` does.
    pub fn set_name(&mut self, name: &[u8]) {
        let name = name.rsplit(|byte| *byte == b'/').next().unwrap_or(name);
        self.name = name[..name.len().min(NAME_LENGTH)].to_vec();
    }

    pub fn set_name_exact(&mut self, name: &[u8]) {
        let end = name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len());
        self.name = name[..end.min(NAME_LENGTH)].to_vec();
    }
}

#[derive(Clone)]
pub(crate) struct Thread {
    pub process: Pid,
    pub cpu: ThreadId,
    /// How the thread continues the next time it runs.
    pub resume: Resume,
    /// `set_tid_address`: where to clear and wake when the thread ends.
    pub clear_child_tid: u64,
    /// `set_robust_list`: the head and its length.
    pub robust_list: (u64, u64),
    /// Blocked signals.
    pub signal_mask: u64,
    /// A mask to put back once the call that set a temporary one (sigsuspend, ppoll) has had its
    /// signal handled.
    pub saved_mask: Option<u64>,
    /// Signals sent to this thread and not yet acted on.
    pub pending: Pending,
    /// `sigaltstack`: base, size and the flags it was given; no stack is a size of zero.
    pub alternate_stack: (u64, u64, i32),
    /// The call the thread is in, while it has not finished: what it waits for and how far it got.
    pub call: Call,
    /// What `restart_syscall` continues, for a call interrupted by a handler.
    pub restart: Option<Restart>,
    /// The CPU time the thread has used, and its scheduler time weighted by its priority.
    pub runtime: u64,
    pub vruntime: u64,
    /// When the thread was woken and has not yet run, on the monotonic clock.
    pub woken_at: Option<u64>,
    /// The address and syndrome of the thread's last fault, which every later frame shows.
    pub fault: (u64, u64),
    /// Signals a fault forced on the thread, which even the first process acts on.
    pub forced: u64,
    /// How the thread is scheduled: its policy, real-time priority, nice value and whether children
    /// go back to the default.
    pub schedule: Schedule,
    /// `rseq`: the registered area, its length and its signature.
    pub rseq: Option<(u64, u32, u32)>,
    /// The seccomp filters the thread installed, oldest first; Docker's profile is not among them.
    pub filters: Vec<Rc<crate::seccomp::Filter>>,
    /// `PR_SET_PDEATHSIG`: the signal sent when the parent ends; cleared when the ids change.
    pub parent_death_signal: i32,
    /// `PR_SET_TIMERSLACK`: how late a timer may end, in nanoseconds.
    pub timer_slack: u64,
    /// An open of a named pipe waiting for the other end: the file it makes, already counted as an
    /// end (Linux's `wait_for_partner`).
    pub opening: Option<crate::file::FileRef>,
}

/// The timer slack a thread starts with (Linux's `init_task` value, which children inherit).
pub(crate) const TIMER_SLACK: u64 = 50_000;

/// What an ended process's first thread keeps for `/proc` and the scheduling calls until it is
/// reaped: its scheduling values (its I/O priority is gone with its I/O context), its blocked signals
/// and how many seccomp filters it installed.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Leader {
    pub schedule: Schedule,
    pub signal_mask: u64,
    pub filters: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Schedule {
    pub policy: u32,
    pub priority: u32,
    pub nice: i32,
    pub reset_on_fork: bool,
    /// A time slice the thread asked for with `sched_setattr`, in nanoseconds; 0 for the default.
    pub slice: u64,
    /// The deadline policy's runtime, deadline and period.
    pub deadline: (u64, u64, u64),
    /// `ioprio_set`'s value; 0 (no class) until one is set.
    pub io_priority: u32,
}

/// The one slot for the call a thread is in. Its registers hold the call and its arguments, so a
/// parked call is run again from them when what it waits for happens; the slot keeps what the call
/// must not forget between runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Call {
    /// What the thread is parked on; none while it runs or can run.
    pub wait: Option<Channel>,
    /// When the call ends anyway, on the monotonic clock, set by its first run.
    pub deadline: Option<u64>,
    /// Whether the call has run before (so its deadline and progress are its own).
    pub started: bool,
    /// How much of the call is done, such as bytes of a large write.
    pub progress: u64,
    /// For F_SETLKW's wait: the descriptor table asking, and the table holding the lock in the way
    /// (none for an open file's lock), as Linux's `blocked_hash` finds them.
    pub lock_wait: Option<(u64, Option<u64>)>,
}

/// What a parked call waits for, besides its deadline and a signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Channel {
    /// Only its deadline or a signal ends the wait.
    Nothing,
    /// A change to one object, such as a pipe, a terminal or a lock's file (its address).
    Object(usize),
    /// A change to whether any file is ready, for the calls that wait on many.
    Readiness,
    /// A change in the state of a child of the thread's process.
    Children,
    /// Any signal sent to the thread or its process (`sigtimedwait` waits for ones it blocks).
    Signals,
    /// A record lock or `flock` lock let go anywhere.
    Locks,
    /// A `vfork` child letting go of the memory.
    Vfork(Pid),
}

/// What `restart_syscall` runs again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Restart {
    /// A sleep until this time on the monotonic clock, writing what is left to this address.
    Sleep { until: u64, left: u64 },
    /// A sleep on a clock that never ends by time (a CPU clock that does not move).
    Forever,
}

impl Thread {
    pub fn new(process: Pid, cpu: ThreadId) -> Self {
        Self {
            process,
            cpu,
            resume: Resume::FromRegisters,
            clear_child_tid: 0,
            robust_list: (0, 0),
            signal_mask: 0,
            saved_mask: None,
            pending: Pending::default(),
            alternate_stack: (0, 0, 0),
            call: Call::default(),
            restart: None,
            runtime: 0,
            vruntime: 0,
            woken_at: None,
            fault: (0, 0),
            forced: 0,
            schedule: Schedule::default(),
            rseq: None,
            filters: Vec::new(),
            parent_death_signal: 0,
            timer_slack: TIMER_SLACK,
            opening: None,
        }
    }
}
