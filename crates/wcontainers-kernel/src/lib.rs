//! A Linux kernel for ARM64 guests.
//!
//! The kernel is a state machine driven from outside through `wcontainers-abi`. It never waits on its own
//! stack: the driver asks [`Kernel::next`] what to do, runs that thread on the CPU side until it comes
//! back, and hands the [`Exit`] to [`Kernel::exit`]. Answers from outside arrive through
//! [`Kernel::event`]. The kernel reaches the CPU side only through [`Cpu`] and everything else through
//! [`Host`].

#![no_std]

extern crate alloc;

mod container;
mod counters;
mod credentials;
mod errno;
mod file;
mod fs;
pub mod linux;
mod linux_extra;
mod loader;
mod memory;
mod process;
mod seccomp;
mod signal;
mod syscall;
mod user;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use wcontainers_abi::{Access, CounterReading, Cpu, Event, Exit, Host, Next, Resume};

pub use container::{Attributes, FileRecord, NewFile, Start};
pub use counters::reason;
pub use errno::Errno;

use counters::Counters;
use fs::{FileSystem, Node, Numbers, Time};
use memory::Backing;
use process::{Parked, Pid, Process, Thread, Tid};

/// What a call into the kernel may use besides the kernel's own state.
pub(crate) struct Context<'a> {
    pub cpu: &'a mut dyn Cpu,
    pub host: &'a mut dyn Host,
}

/// What the guest is told about the machine besides its CPU features.
pub(crate) struct Machine {
    /// `uname`'s release and version.
    pub release: &'static str,
    pub version: &'static str,
    /// The memory size reported, the host's cap on one process's addresses.
    pub memory_bytes: u64,
}

/// The kernel's file systems that exist from the start.
pub(crate) struct Files {
    pub numbers: Numbers,
    pub root: Node,
    pub proc_fs: FileSystem,
    pub pipe_fs: FileSystem,
}

/// All kernel state. It is plain data, so a container is saved by copying it with the guest memories.
pub struct Kernel {
    processes: BTreeMap<Pid, Process>,
    threads: BTreeMap<Tid, Thread>,
    /// Threads that can run, the next one first.
    runnable: VecDeque<Tid>,
    /// The thread the driver is running, between [`Kernel::next`] and [`Kernel::exit`].
    running: Option<Tid>,
    /// The last process or thread number handed out. Both share one space, as in Linux.
    last_id: i32,
    /// The container's first process; when it ends the container ends.
    init: Option<Pid>,
    /// The first process's wait status once it has ended.
    finished: Option<i32>,
    counters: Counters,
    files: Files,
    machine: Machine,
    hostname: Vec<u8>,
    domainname: Vec<u8>,
    /// The wall clock time and the monotonic clock when the container started.
    boot: Time,
    boot_monotonic: u64,
    boot_id: [u8; 16],
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}

impl Kernel {
    pub fn new() -> Self {
        let mut numbers = Numbers::default();
        let root = container::new_root(&mut numbers);
        use linux_extra::{ST_NODEV, ST_NOEXEC, ST_NOSUID, ST_RELATIME};
        let proc_fs = numbers.file_system(
            fs::Kind::Proc,
            ST_NOSUID | ST_NODEV | ST_NOEXEC | ST_RELATIME,
        );
        let pipe_fs = numbers.file_system(fs::Kind::Pipe, 0);
        Self {
            processes: BTreeMap::new(),
            threads: BTreeMap::new(),
            runnable: VecDeque::new(),
            running: None,
            last_id: 0,
            init: None,
            finished: None,
            counters: Counters::default(),
            files: Files {
                numbers,
                root,
                proc_fs,
                pipe_fs,
            },
            machine: Machine {
                release: "7.0.0-wcontainers",
                version: "#1 SMP PREEMPT",
                memory_bytes: 0,
            },
            hostname: Vec::new(),
            domainname: b"(none)".to_vec(),
            boot: Time::default(),
            boot_monotonic: 0,
            boot_id: [0; 16],
        }
    }

    /// What the driver should do next.
    #[allow(
        clippy::should_implement_trait,
        reason = "named after the abi's `Next`"
    )]
    pub fn next(&mut self) -> Next {
        assert!(
            self.running.is_none(),
            "next asked while a thread is running; hand its exit back first"
        );
        if let Some(status) = self.finished {
            return Next::Finished { status };
        }
        match self.runnable.pop_front() {
            Some(tid) => {
                self.running = Some(tid);
                let thread = &self.threads[&tid];
                Next::Run {
                    thread: thread.cpu,
                    resume: thread.resume,
                    slice_nanoseconds: None,
                }
            }
            None => Next::Idle {
                until_nanoseconds: self
                    .threads
                    .values()
                    .filter_map(|thread| match thread.parked {
                        Some(Parked::Until { nanoseconds }) => Some(nanoseconds),
                        _ => None,
                    })
                    .min(),
            },
        }
    }

    /// The thread from the last [`Next::Run`] has come back.
    pub fn exit(&mut self, cpu: &mut dyn Cpu, host: &mut dyn Host, exit: Exit) {
        let tid = self
            .running
            .take()
            .expect("an exit was handed back with no thread running");
        let mut cx = Context { cpu, host };
        match exit {
            Exit::Syscall(call) => match syscall::dispatch(self, &mut cx, tid, call) {
                syscall::Outcome::Return(value) => {
                    if self.deliver_signals(&mut cx, tid) {
                        self.resume(tid, Resume::Return(value))
                    }
                }
                syscall::Outcome::Restart => {
                    if self.deliver_signals(&mut cx, tid) {
                        self.resume(tid, Resume::FromRegisters)
                    }
                }
                syscall::Outcome::Parked => {}
                syscall::Outcome::Gone => {}
            },
            Exit::Stop => self.resume(tid, Resume::Continue),
            Exit::MemoryFault { address, access } => {
                self.memory_fault(&mut cx, tid, address, access)
            }
            Exit::MemoryFaultWithoutRegisters => {
                self.kill_by_signal(&mut cx, tid, linux::general::SIGSEGV)
            }
            Exit::IllegalInstruction => self.kill_by_signal(&mut cx, tid, linux::general::SIGILL),
            Exit::Breakpoint { .. } => self.kill_by_signal(&mut cx, tid, linux::general::SIGTRAP),
        }
    }

    /// Something happened outside.
    pub fn event(&mut self, _cpu: &mut dyn Cpu, host: &mut dyn Host, event: Event<'_>) {
        let detail = match event {
            Event::Timer => {
                let now = host.monotonic_nanoseconds();
                let due: Vec<Tid> = self
                    .threads
                    .iter()
                    .filter(|(_, thread)| {
                        matches!(thread.parked, Some(Parked::Until { nanoseconds }) if nanoseconds <= now)
                    })
                    .map(|(tid, _)| *tid)
                    .collect();
                for tid in due {
                    self.threads.get_mut(&tid).expect("thread").parked = None;
                    self.resume(tid, Resume::Return(0));
                }
                return;
            }
            Event::BackendReply { request, .. } => request.0,
            Event::BackendNotice { .. } => 0,
            Event::Connected { connection, .. } | Event::ConnectionReady { connection } => {
                u64::from(connection.0)
            }
            Event::LookedUp { request, .. } => request,
            Event::TerminalInput { .. } | Event::TerminalResized { .. } => 0,
        };
        self.counters.count(host, reason::EVENT_UNEXPECTED, detail);
    }

    /// Every counter that has been bumped.
    pub fn counters(&self) -> impl Iterator<Item = CounterReading> + '_ {
        self.counters.readings()
    }

    fn resume(&mut self, tid: Tid, resume: Resume) {
        self.thread_mut(tid).resume = resume;
        self.runnable.push_front(tid);
    }

    fn allocate_id(&mut self) -> i32 {
        self.last_id += 1;
        self.last_id
    }

    pub(crate) fn thread_mut(&mut self, tid: Tid) -> &mut Thread {
        self.threads.get_mut(&tid).expect("thread exists")
    }

    pub(crate) fn process_of(&self, tid: Tid) -> &Process {
        &self.processes[&self.threads[&tid].process]
    }

    pub(crate) fn process_of_mut(&mut self, tid: Tid) -> &mut Process {
        let pid = self.threads[&tid].process;
        self.processes.get_mut(&pid).expect("process exists")
    }

    /// Runs `f` with a path walker from the point of view of process `pid`.
    pub(crate) fn with_walk<R>(
        &self,
        pid: Pid,
        f: impl FnOnce(&fs::Walk<'_>, &Process) -> Result<R, Errno>,
    ) -> Result<R, Errno> {
        let process = &self.processes[&pid];
        let view = fs::proc::View {
            processes: &self.processes,
            caller: pid,
            fs: self.files.proc_fs,
            boot: self.boot,
        };
        let walk = fs::Walk {
            root: &process.root,
            credentials: &process.credentials,
            proc: &view,
        };
        f(&walk, process)
    }

    /// The time now on the wall clock.
    pub(crate) fn now(&self, cx: &mut Context<'_>) -> Time {
        Time::from_nanoseconds(cx.host.realtime_nanoseconds())
    }

    /// Ends a whole process with a wait status, as `exit_group` and fatal signals do.
    pub(crate) fn end_process(&mut self, cx: &mut Context<'_>, pid: Pid, wait_status: i32) {
        let process = self.processes.remove(&pid).expect("process exists");
        for tid in &process.threads {
            let thread = self.threads.remove(tid).expect("thread exists");
            cx.cpu.thread_drop(thread.cpu);
            self.runnable.retain(|other| other != tid);
        }
        cx.cpu.memory_drop(process.memory);
        if self.init == Some(pid) {
            self.finished = Some(wait_status);
        }
    }

    /// A fault: grow the stack if that is what it is, or kill the process as Linux's default action does
    /// for the signal the fault raises.
    fn memory_fault(&mut self, cx: &mut Context<'_>, tid: Tid, address: u64, access: Access) {
        let pid = self.threads[&tid].process;
        let process = self.processes.get_mut(&pid).expect("process exists");
        let mapping = process.map.find(address).cloned();
        let signal = match mapping {
            None => {
                if process
                    .map
                    .grow_stack(cx.cpu, process.memory, address, &process.limits)
                    .unwrap_or(false)
                {
                    self.resume(tid, Resume::FromRegisters);
                    return;
                }
                linux::general::SIGSEGV
            }
            Some(mapping) => match mapping.backing {
                // Allowed by the mapping but past the end of its file.
                Backing::File { file_end, .. }
                    if address >= file_end
                        && memory::cpu_permissions(mapping.permissions).contains(
                            match access {
                                Access::Read => wcontainers_abi::Permissions::READ,
                                Access::Write => wcontainers_abi::Permissions::WRITE,
                                Access::Execute => wcontainers_abi::Permissions::EXECUTE,
                            },
                        ) =>
                {
                    linux::general::SIGBUS
                }
                _ => linux::general::SIGSEGV,
            },
        };
        self.kill_by_signal(cx, tid, signal);
    }

    /// A signal with no handler that kills: the process dies of it, as Linux's default action does. A
    /// handler the program set cannot run yet, which is counted.
    fn kill_by_signal(&mut self, cx: &mut Context<'_>, tid: Tid, signal: u32) {
        let pid = self.threads[&tid].process;
        let handler = self.processes[&pid].actions[signal as usize - 1].sa_handler_kernel;
        if handler != linux_extra::SIG_DFL {
            self.counters
                .count(cx.host, reason::SIGNAL_HANDLER_NOT_RUN, signal.into());
        }
        self.end_process(cx, pid, signal as i32);
    }
}

/// One ARM64 Linux system call and whether the kernel has built it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemCall {
    pub number: u64,
    pub name: &'static str,
    pub implemented: bool,
}

/// Every ARM64 Linux system call, by number.
pub fn system_calls() -> impl Iterator<Item = SystemCall> {
    syscall::all()
}

/// The name of an ARM64 Linux system call, if Linux has one with this number.
pub fn system_call_name(number: u64) -> Option<&'static str> {
    syscall::name(number)
}
