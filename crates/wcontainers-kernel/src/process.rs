//! Processes and threads.

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

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
    /// The parent's thread it belongs to (Linux's `real_parent`): the one that made it, or the
    /// thread that took it on when that one ended. A wait with __WNOTHREAD sees only its own.
    pub parent_thread: Tid,
    pub group: Pid,
    pub session: Pid,
    pub memory: MemoryId,
    /// The address map, shared by the processes made with CLONE_VM (Linux's `mm_struct`).
    pub map: Map,
    pub threads: Vec<Tid>,
    pub limits: [Limit; LIMITS],
    /// The program file and the path it was started by.
    pub executable: (Node, Vec<u8>),
    pub starting_stack: StartingStack,
    /// Signal handlers as `rt_sigaction` set them, shared by the processes made with CLONE_SIGHAND.
    pub actions: Handlers,
    /// The number the next POSIX timer it makes gets.
    pub next_timer: i32,
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
    /// Job control: whether the process is stopped, and what a parent's wait has yet to see.
    pub stopped: bool,
    pub report: Option<Report>,
    /// Continued and the parent not yet sent its SIGCHLD, which the process sends when it next runs
    /// (`SIGNAL_CLD_CONTINUED`).
    pub continued_unsent: bool,
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
    /// Which mappings a core dump would hold (`/proc/<pid>/coredump_filter`, the memory's
    /// `MMF_DUMP_FILTER_MASK` bits), which a fork and an exec keep.
    pub coredump_filter: u32,
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
    /// Sent by a POSIX timer (Linux's preallocated `sigqueue` of the timer), which decides at
    /// delivery whether it still counts.
    pub timer: bool,
}

/// One queued signal in a user's count (Linux's `UCOUNT_RLIMIT_SIGPENDING`), which
/// RLIMIT_SIGPENDING bounds: the count goes up when one is made or copied and down when it goes.
#[derive(Debug)]
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
                timer: false,
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

impl Thread {
    /// Sets the `comm` name from a path's last part, as `execve` does.
    pub fn set_name(&mut self, name: &[u8]) {
        let name = name.rsplit(|byte| *byte == b'/').next().unwrap_or(name);
        self.name = name[..name.len().min(NAME_LENGTH)].to_vec();
    }

    /// Sets the `comm` name to the bytes up to the first zero, as `PR_SET_NAME` does.
    pub fn set_name_exact(&mut self, name: &[u8]) {
        let end = name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len());
        self.name = name[..end.min(NAME_LENGTH)].to_vec();
    }
}

/// An address map, shared by the processes made with CLONE_VM.
pub(crate) type Map = Rc<RefCell<AddressMap>>;

/// Signal handlers by signal number less one, shared by the threads and processes made with
/// CLONE_SIGHAND (Linux's `sighand_struct`).
pub(crate) type Handlers = Rc<RefCell<[kernel_sigaction; SIGNALS]>>;

/// A descriptor table, shared by the threads and processes made with CLONE_FILES (Linux's
/// `files_struct`).
pub(crate) type Files = Rc<RefCell<FdTable>>;

/// The root, working directory and umask, shared by the threads and processes made with CLONE_FS
/// (Linux's `fs_struct`).
#[derive(Clone)]
pub(crate) struct FsContext {
    pub root: Node,
    pub cwd: Node,
    pub umask: u32,
}

pub(crate) type Fs = Rc<RefCell<FsContext>>;

#[derive(Clone)]
pub(crate) struct Thread {
    pub process: Pid,
    pub cpu: ThreadId,
    pub files: Files,
    pub fs: Fs,
    /// The thread's credentials (Linux's `cred`, which each thread holds).
    pub credentials: Credentials,
    /// `personality`'s value; kept across `execve`.
    pub personality: u32,
    /// The name `comm` shows, at most 15 bytes.
    pub name: Vec<u8>,
    /// How the thread continues the next time it runs.
    pub resume: Resume,
    /// Made with CLONE_VFORK: the thread that made it, which waits until this one starts a program
    /// or ends (`vfork_done`).
    pub vfork_parent: Option<Tid>,
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
    /// What the call took from its arguments, kept while it waits so each run takes the same.
    pub taken: RefCell<Taken>,
    /// The futexes the call waits on, while it is a futex wait.
    pub futex: crate::syscall::futex::Wait,
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
    /// The inode a thread other than its process's first gives its process descriptors.
    pub pidfd: Option<Node>,
}

/// The timer slack a thread starts with (Linux's `init_task` value, which children inherit).
pub(crate) const TIMER_SLACK: u64 = 50_000;

/// What a process's first thread keeps for `/proc`, the scheduling calls and permission checks once
/// it has ended, until the process is reaped: its name, credentials, scheduling values (its I/O
/// priority is gone with its I/O context), its blocked signals and how many seccomp filters it
/// installed.
#[derive(Clone, Debug, Default)]
pub(crate) struct Leader {
    pub name: Vec<u8>,
    pub credentials: Credentials,
    pub schedule: Schedule,
    pub signal_mask: u64,
    pub filters: usize,
}

impl Leader {
    pub fn of(thread: &Thread) -> Self {
        Self {
            name: thread.name.clone(),
            credentials: thread.credentials.clone(),
            schedule: Schedule {
                io_priority: 0,
                ..thread.schedule
            },
            signal_mask: thread.signal_mask,
            filters: thread.filters.len(),
        }
    }
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
    /// Whether its deadline is a time on the wall clock (`HRTIMER_BASE_REALTIME`), moved when the
    /// wall clock is set.
    pub wall: bool,
    /// How much of the call is done, such as bytes of a large write.
    pub progress: u64,
    /// How many messages of sendmmsg or recvmmsg are done; `progress` is the current one's.
    pub messages: u64,
    /// For F_SETLKW's wait: the descriptor table asking, and the table holding the lock in the way
    /// (none for an open file's lock), as Linux's `blocked_hash` finds them.
    pub lock_wait: Option<(u64, Option<u64>)>,
    /// The locks the call holds until it ends, while it waits too (see `Kernel::take_lock`): a
    /// pipe's, as `splice_file_to_pipe` holds it around a terminal's read, an open file's
    /// position, a backend file's writes.
    pub holds: [Option<Channel>; 2],
}

/// What a call took from its arguments, in order: the open files its descriptors named (`fdget`)
/// and the paths it copied (`getname`). Linux takes each once per call; a call here runs again
/// after each wait, and its later runs take these instead of reading the table and memory again.
#[derive(Clone, Default)]
pub(crate) struct Taken {
    items: Vec<TakenItem>,
    used: usize,
}

#[derive(Clone)]
enum TakenItem {
    File(i32, Result<crate::file::FileRef, crate::Errno>),
    Path(u64, Result<Vec<u8>, crate::Errno>),
    Value(u64),
}

impl Taken {
    /// A run of the call starts: it takes from the start again.
    pub fn rewind(&mut self) {
        self.used = 0;
    }

    /// The call ended: what it took goes, its files given back (see `Kernel::give_back`).
    pub fn clear(&mut self) -> Vec<crate::file::FileRef> {
        self.used = 0;
        self.items
            .drain(..)
            .filter_map(|item| match item {
                TakenItem::File(_, Ok(file)) => Some(file),
                _ => None,
            })
            .collect()
    }

    /// What the next item taken is, if an earlier run took it for the same argument.
    fn next(&mut self, same: impl Fn(&TakenItem) -> bool) -> Option<TakenItem> {
        let item = self.items.get(self.used).filter(|item| same(item))?.clone();
        self.used += 1;
        Some(item)
    }

    fn push(&mut self, item: TakenItem) {
        self.items.truncate(self.used);
        self.items.push(item);
        self.used += 1;
    }

    /// The open file descriptor `fd` named when the call first took it, else `look` now.
    pub fn file(
        &mut self,
        fd: i32,
        look: impl FnOnce() -> Result<crate::file::FileRef, crate::Errno>,
    ) -> Result<crate::file::FileRef, crate::Errno> {
        if let Some(TakenItem::File(_, file)) =
            self.next(|item| matches!(item, TakenItem::File(known, _) if *known == fd))
        {
            return file;
        }
        let file = look();
        self.push(TakenItem::File(fd, file.clone()));
        file
    }

    /// A value the call settles once, such as where an append goes: the one an earlier run
    /// settled, else `value`.
    pub fn value(&mut self, value: u64) -> u64 {
        if let Some(TakenItem::Value(known)) = self.next(|item| matches!(item, TakenItem::Value(_)))
        {
            return known;
        }
        self.push(TakenItem::Value(value));
        value
    }

    /// The path copied from `address` when the call first took it, else `read` now.
    pub fn path(
        &mut self,
        address: u64,
        read: impl FnOnce() -> Result<Vec<u8>, crate::Errno>,
    ) -> Result<Vec<u8>, crate::Errno> {
        if let Some(TakenItem::Path(_, path)) =
            self.next(|item| matches!(item, TakenItem::Path(known, _) if *known == address))
        {
            return path;
        }
        let path = read();
        self.push(TakenItem::Path(address, path.clone()));
        path
    }
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
    /// A thread made with CLONE_VFORK starting a program or ending.
    Vfork(Tid),
    /// A futex wake, requeue or lock handed over (the futexes know which).
    Futex,
    /// An answer from the file backend; no signal ends this wait, as Linux's FUSE waits once a
    /// request is sent.
    Backend,
    /// A lock that another call holds while it waits (the address of what it guards); no signal
    /// ends this wait, as none ends Linux's `mutex_lock`.
    Held(usize),
    /// An event for an epoll instance (its address), which wakes one waiter at a time.
    Epoll(usize),
}

/// What `restart_syscall` runs again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Restart {
    /// A sleep until this time on the monotonic clock, writing what is left to this address.
    Sleep { until: u64, left: u64 },
    /// A sleep until a CPU clock (its id) reaches a time, writing what is left to this address.
    CpuSleep { clock: i32, until: u64, left: u64 },
    /// A futex wait with a timeout: the futex, the value it must hold, the bits it waits for, and
    /// when it ends on the monotonic clock.
    Futex {
        address: u64,
        value: u32,
        bitset: u32,
        shared: bool,
        until: u64,
        /// Whether `until` is a time on the wall clock.
        wall: bool,
    },
}

impl Thread {
    pub fn new(
        process: Pid,
        cpu: ThreadId,
        files: Files,
        fs: Fs,
        credentials: Credentials,
    ) -> Self {
        Self {
            process,
            cpu,
            files,
            fs,
            credentials,
            personality: 0,
            name: Vec::new(),
            resume: Resume::FromRegisters,
            vfork_parent: None,
            clear_child_tid: 0,
            robust_list: (0, 0),
            signal_mask: 0,
            saved_mask: None,
            pending: Pending::default(),
            alternate_stack: (0, 0, 0),
            call: Call::default(),
            taken: RefCell::default(),
            futex: Default::default(),
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
            pidfd: None,
        }
    }
}
