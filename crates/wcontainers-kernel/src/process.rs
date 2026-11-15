//! Processes and threads.

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

/// What `/proc/<pid>/cmdline`, `environ` and `auxv` read from: ranges of the starting stack.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StartingStack {
    pub arguments: (u64, u64),
    pub environment: (u64, u64),
    pub auxiliary: (u64, u64),
}

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
    /// `sigaltstack`: base, size and the flags it was given; no stack is a size of zero.
    pub alternate_stack: (u64, u64, i32),
    /// The call the thread is parked in, if it is waiting.
    pub parked: Option<Parked>,
}

/// What a parked thread waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Parked {
    /// A sleep until this time on the monotonic clock; the call is finished when it comes.
    Until { nanoseconds: u64 },
    /// A wait for something that nothing here can bring yet; only a signal could end it (K2).
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
            alternate_stack: (0, 0, 0),
            parked: None,
        }
    }
}
