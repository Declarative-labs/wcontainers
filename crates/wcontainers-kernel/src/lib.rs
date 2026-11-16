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
mod locks;
mod memory;
mod pipe;
mod process;
mod schedule;
mod seccomp;
mod shared;
mod signal;
mod syscall;
mod task;
mod tty;
mod user;
mod vdso;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::rc::Rc;
use alloc::vec::Vec;

use wcontainers_abi::{CounterReading, Cpu, Event, Exit, Host, Next};

pub use container::{Attributes, FileRecord, NewFile, Start};
pub use counters::reason;
pub use errno::Errno;

use counters::Counters;
use fs::{FileSystem, Node, Numbers, Time};
use process::{Pid, Process, Thread, Tid};

/// What a call into the kernel may use besides the kernel's own state.
pub(crate) struct Context<'a> {
    pub cpu: &'a mut dyn Cpu,
    pub host: &'a mut dyn Host,
    /// What is added to the host's monotonic clock to give the kernel's (see
    /// [`Kernel::restore`]).
    monotonic_offset: u64,
    /// Requests the CPU side answered it cannot do at all, counted when the kernel hands back control.
    refused: u32,
}

impl Context<'_> {
    /// What a refused request to make a memory, thread or shared block means to the guest; a host
    /// that cannot do it at all is counted.
    pub fn cpu_error(&mut self, error: wcontainers_abi::CpuError) -> Errno {
        if error == wcontainers_abi::CpuError::NotSupported {
            self.refused += 1;
        }
        errno::from_cpu(error)
    }

    /// The kernel's monotonic clock, which never goes backwards, also across a restore on another
    /// host.
    pub fn monotonic(&mut self) -> u64 {
        self.host
            .monotonic_nanoseconds()
            .wrapping_add(self.monotonic_offset)
    }
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
    /// The internal tmpfs memory files and shared anonymous memory live on (Linux's `shm_mnt`, mounted
    /// first at boot, so it has the first anonymous device number).
    pub memfd_fs: FileSystem,
    /// The internal file system secret memory files live on, made at the first `memfd_secret`.
    pub secretmem_fs: Option<FileSystem>,
    /// The file system process descriptors are on, once one is made.
    pub pidfs: Option<FileSystem>,
    /// The anonymous inode, once a file needs it.
    pub anon: Option<crate::fs::Node>,
    /// Files whose bytes are partly in a block of shared pages, while a mapping shares them.
    pub shared: Vec<Node>,
    /// The key tmpfs inodes' generation numbers are made from, as Linux draws each at random.
    pub generation_key: [u8; 16],
    /// Changed attributes of the `/proc` entries that keep them.
    pub proc_kept: BTreeMap<u64, fs::proc::Kept>,
    /// Every tmpfs's count of what it holds, for the machine's memory in use.
    pub spaces: Vec<Rc<fs::Space>>,
    /// Pseudo-terminal pairs by number, until both sides have closed.
    pub terminals: BTreeMap<u32, Rc<core::cell::RefCell<tty::Pty>>>,
}

/// All kernel state. It is plain data, so a container is saved by copying it with the guest memories.
pub struct Kernel {
    processes: BTreeMap<Pid, Process>,
    threads: BTreeMap<Tid, Thread>,
    /// Threads that can run and are not the current one.
    run_queue: schedule::RunQueue,
    /// The thread that has the CPU: it runs again after its call unless another should take over.
    current: Option<Tid>,
    /// Whether the current thread should give way at the next choice (Linux's `need_resched`).
    resched: bool,
    /// The thread the driver is running, between [`Kernel::next`] and [`Kernel::exit`].
    running: Option<Tid>,
    /// Parked threads by channel and deadline.
    parked: schedule::Parked,
    /// Files closed with a descriptor table whose last holder has gone, for the kernel to finish
    /// releasing.
    released: Vec<file::FileRef>,
    /// Parked threads whose wait ended, whose calls are run again before anything else happens.
    woken: VecDeque<Tid>,
    /// Threads not running that were sent a signal, which they act on before they run.
    signalled: VecDeque<Tid>,
    /// What is added to the host's monotonic clock to give the kernel's: zero until a restore on a
    /// host whose clock reads differently.
    monotonic_offset: u64,
    /// The monotonic clock when the kernel was last entered.
    entry_now: u64,
    /// The monotonic clock when the kernel last handed control back, which is when the thread it
    /// picks next starts running.
    last_now: u64,
    /// The current thread's CPU time when it was picked, for how much of its slice is used.
    picked_runtime: u64,
    /// The last process or thread number handed out. Both share one space, as in Linux.
    last_id: i32,
    /// The container's first process; when it ends the container ends.
    init: Option<Pid>,
    /// The first process's wait status once it has ended.
    finished: Option<i32>,
    /// The last serial given to committed credentials; 0 is the credentials of whoever opened the
    /// container's first descriptors.
    last_credentials: u64,
    counters: Counters,
    files: Files,
    /// Each user's count of queued signals, by real user id.
    signal_users: BTreeMap<u32, Rc<core::cell::Cell<u64>>>,
    /// The container's System V IPC objects.
    ipc: syscall::ipc::Ipc,
    /// Every futex with waiters.
    futexes: syscall::futex::Futexes,
    /// Every timer.
    timers: syscall::timer::Timers,
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
        let memfd_space = fs::Space::new(0, 0);
        let spaces = [root.borrow().fs.space.clone(), Some(memfd_space.clone())]
            .into_iter()
            .flatten()
            .collect();
        Self {
            processes: BTreeMap::new(),
            threads: BTreeMap::new(),
            run_queue: schedule::RunQueue::default(),
            current: None,
            resched: false,
            running: None,
            parked: schedule::Parked::default(),
            released: Vec::new(),
            woken: VecDeque::new(),
            signalled: VecDeque::new(),
            monotonic_offset: 0,
            entry_now: 0,
            last_now: 0,
            picked_runtime: 0,
            last_id: 0,
            init: None,
            finished: None,
            last_credentials: 0,
            counters: Counters::default(),
            ipc: Default::default(),
            futexes: Default::default(),
            timers: Default::default(),
            signal_users: BTreeMap::new(),
            files: Files {
                numbers,
                root,
                proc_fs,
                pipe_fs,
                memfd_fs: FileSystem {
                    kind: fs::Kind::Tmpfs,
                    device: fs::encode_device(0, 1),
                    flags: 0,
                    space: Some(memfd_space),
                },
                secretmem_fs: None,
                pidfs: None,
                anon: None,
                shared: Vec::new(),
                generation_key: [0; 16],
                proc_kept: BTreeMap::new(),
                spaces,
                terminals: BTreeMap::new(),
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
        self.pick()
    }

    /// The thread from the last [`Next::Run`] has come back.
    pub fn exit(&mut self, cpu: &mut dyn Cpu, host: &mut dyn Host, exit: Exit) {
        let tid = self
            .running
            .take()
            .expect("an exit was handed back with no thread running");
        let mut cx = Context {
            cpu,
            host,
            monotonic_offset: self.monotonic_offset,
            refused: 0,
        };
        self.account(&mut cx, tid);
        self.tick(tid);
        self.wake_due();
        syscall::timer::fire_due(self, &mut cx);
        syscall::timer::cpu_check(self, &mut cx, tid);
        self.tell_continued(&mut cx, tid);
        self.came_back(&mut cx, tid, exit);
        self.settle(&mut cx);
    }

    /// Something happened outside.
    pub fn event(&mut self, cpu: &mut dyn Cpu, host: &mut dyn Host, event: Event<'_>) {
        let mut cx = Context {
            cpu,
            host,
            monotonic_offset: self.monotonic_offset,
            refused: 0,
        };
        let detail = match event {
            Event::Timer => {
                self.entry_now = cx.monotonic();
                self.wake_due();
                syscall::timer::fire_due(self, &mut cx);
                self.settle(&mut cx);
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
        self.counters
            .count(cx.host, reason::EVENT_UNEXPECTED, detail);
        self.settle(&mut cx);
    }

    /// Brings back a saved container, on a CPU side restored from the same save (every memory, block
    /// and thread's registers under the same handles, nothing else) and a host whose clocks may read
    /// anything: the kernel's monotonic clock goes on from where it was saved, every thread starts
    /// again from its saved registers, and every parked call runs again, its deadline on the clock
    /// it had.
    pub fn restore(&mut self, cpu: &mut dyn Cpu, host: &mut dyn Host) {
        assert!(self.running.is_none(), "restored while a thread runs");
        self.monotonic_offset = self.last_now.wrapping_sub(host.monotonic_nanoseconds());
        let mut cx = Context {
            cpu,
            host,
            monotonic_offset: self.monotonic_offset,
            refused: 0,
        };
        self.entry_now = cx.monotonic();
        let tids: Vec<Tid> = self.threads.keys().copied().collect();
        for tid in tids {
            let thread = self.threads.get_mut(&tid).expect("thread");
            // A parked thread's registers still hold its call; it gets its value when the call ends.
            if thread.call.wait.is_some() {
                self.wake_thread(tid);
                continue;
            }
            if let wcontainers_abi::Resume::Return(value) = thread.resume {
                let mut registers = wcontainers_abi::RegisterBlock::ZERO;
                cx.cpu.registers(thread.cpu, &mut registers);
                registers.x[0] = value;
                cx.cpu.set_registers(thread.cpu, &registers);
            }
            thread.resume = wcontainers_abi::Resume::FromRegisters;
        }
        self.settle(&mut cx);
    }

    /// Every counter that has been bumped.
    pub fn counters(&self) -> impl Iterator<Item = CounterReading> + '_ {
        self.counters.readings()
    }

    /// Gives a thread's credentials a new serial after a change Linux commits (`commit_creds`).
    pub(crate) fn commit_credentials(&mut self, tid: Tid) {
        self.last_credentials += 1;
        let serial = self.last_credentials;
        self.thread_mut(tid).credentials.serial = serial;
    }

    /// Thread `tid`'s credentials (`current_cred` for the calling thread).
    pub(crate) fn credentials(&self, tid: Tid) -> &credentials::Credentials {
        &self.threads[&tid].credentials
    }

    /// The credentials of the task a number names (`__task_cred`): a live thread's, or what a
    /// process's first thread kept at its end.
    pub(crate) fn task_credentials(&self, pid: Pid) -> &credentials::Credentials {
        fs::proc::task_credentials(&self.processes, &self.threads, pid)
    }

    /// The next free process and thread number, wrapping at `pid_max` past the low numbers
    /// (`alloc_pid`'s `RESERVED_PIDS`).
    fn allocate_id(&mut self) -> i32 {
        loop {
            self.last_id += 1;
            if self.last_id >= PID_MAX {
                self.last_id = RESERVED_PIDS;
            }
            let taken = self.processes.contains_key(&self.last_id)
                || self.threads.contains_key(&self.last_id)
                || self.processes.values().any(|process| {
                    process.group == self.last_id || process.session == self.last_id
                });
            if !taken {
                return self.last_id;
            }
        }
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

    /// Thread `tid`'s descriptor table.
    pub(crate) fn files(&self, tid: Tid) -> core::cell::Ref<'_, file::FdTable> {
        self.threads[&tid].files.borrow()
    }

    pub(crate) fn files_mut(&self, tid: Tid) -> core::cell::RefMut<'_, file::FdTable> {
        self.threads[&tid].files.borrow_mut()
    }

    /// Thread `tid`'s root, working directory and umask.
    pub(crate) fn fs(&self, tid: Tid) -> core::cell::Ref<'_, process::FsContext> {
        self.threads[&tid].fs.borrow()
    }

    pub(crate) fn fs_mut(&self, tid: Tid) -> core::cell::RefMut<'_, process::FsContext> {
        self.threads[&tid].fs.borrow_mut()
    }

    /// Lets go of a thread's hold on a descriptor table; the last holder's going closes it, in its
    /// process `closer` (`put_files_struct`).
    pub(crate) fn put_files(&mut self, files: process::Files, closer: process::Pid) {
        if let Some(table) = Rc::into_inner(files) {
            let mut table = table.into_inner();
            table.close_all(closer);
            self.released.extend(table.take_released());
        }
    }

    /// Gives thread `tid` a copy of its descriptor table if it shares it (`unshare_fd`); the copy's
    /// record locks are its own.
    pub(crate) fn unshare_files(&mut self, tid: Tid) {
        let thread = self.thread_mut(tid);
        if Rc::strong_count(&thread.files) > 1 {
            let copy = thread.files.borrow().duplicate(tid as u64);
            thread.files = Rc::new(core::cell::RefCell::new(copy));
        }
    }

    /// Runs `f` with a path walker from the point of view of thread `tid`.
    pub(crate) fn with_walk<R>(
        &self,
        tid: Tid,
        f: impl FnOnce(&fs::Walk<'_>, &process::FsContext) -> Result<R, Errno>,
    ) -> Result<R, Errno> {
        let thread = &self.threads[&tid];
        let pid = thread.process;
        let context = thread.fs.borrow();
        let view = fs::proc::View {
            processes: &self.processes,
            threads: &self.threads,
            caller: pid,
            caller_thread: tid,
            fs: self.files.proc_fs.clone(),
            boot: self.boot,
            kept: &self.files.proc_kept,
            counters: &self.counters,
        };
        let walk = fs::Walk {
            root: &context.root,
            credentials: &thread.credentials,
            proc: &view,
        };
        f(&walk, &context)
    }

    /// The time now on the wall clock.
    /// The memory in use, in bytes, and how much of it tmpfs holds: every mapped page and every
    /// tmpfs page, counted as present, since which pages a program has touched is not known.
    pub(crate) fn memory_in_use(&self) -> (u64, u64) {
        let tmpfs: u64 = self
            .files
            .spaces
            .iter()
            .map(|space| space.used_blocks() * wcontainers_abi::PAGE_SIZE)
            .sum();
        // A map shared by processes made with CLONE_VM counts once.
        let mut maps: alloc::vec::Vec<&process::Map> = self
            .processes
            .values()
            .map(|process| &process.map)
            .collect();
        maps.sort_by_key(|map| Rc::as_ptr(map));
        maps.dedup_by(|a, b| Rc::ptr_eq(a, b));
        let mapped: u64 = maps
            .iter()
            .flat_map(|map| {
                map.borrow()
                    .iter()
                    .map(|mapping| mapping.end - mapping.start)
                    .collect::<alloc::vec::Vec<_>>()
            })
            .sum();
        ((tmpfs + mapped).min(self.machine.memory_bytes), tmpfs)
    }

    pub(crate) fn now(&self, cx: &mut Context<'_>) -> Time {
        Time::from_nanoseconds(cx.host.realtime_nanoseconds())
    }
}

/// The largest process number plus one (`pid_max`'s default of `PID_MAX_DEFAULT`, which
/// `/proc/sys/kernel/pid_max` reports), and the numbers skipped when they wrap.
const PID_MAX: i32 = 4_194_304;
const RESERVED_PIDS: i32 = 300;

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
