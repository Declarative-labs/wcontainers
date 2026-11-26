//! A Linux kernel for ARM64 guests.
//!
//! The kernel is a state machine driven from outside through `wcontainers-abi`. It never waits on its own
//! stack: the driver asks [`Kernel::next`] what to do, runs that thread on the CPU side until it comes
//! back, and hands the [`Exit`] to [`Kernel::exit`]. Answers from outside arrive through
//! [`Kernel::event`]. The kernel reaches the CPU side only through [`Cpu`] and everything else through
//! [`Host`].

#![no_std]
// The kernel keeps the system's waiting calls and is used from any host thread, so it asks each
// `async fn` of the system it waits on to return a `Send` future, which only return type notation
// can say of a generic system.
#![feature(return_type_notation)]

extern crate alloc;

mod cgroup;
mod container;
mod counters;
mod credentials;
mod ends;
mod errno;
mod executing;
mod file;
mod fs;
mod guest_machine;
mod hugetlb;
pub mod linux;
mod linux_extra;
mod loader;
mod locks;
mod memory;
pub mod network;
mod pipe;
mod process;
mod running;
mod schedule;
mod seccomp;
mod shared;
mod sharing;
mod siginfo;
mod signal;
mod suspend;
mod syscall;
mod sysctl;
mod system;
mod task;
mod tty;
mod user;
mod vdso;
pub mod wait;
mod waiting;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;
use alloc::vec::Vec;

use wcontainers_abi::{CounterReading, Cpu, Event, Exit, Next};

pub use cgroup::MOUNT as CGROUP_MOUNT;
pub use container::{
    Attributes, DEV_MOUNT, DEVPTS_MOUNT, DEVPTS_PATH, DOCKER_DOMAIN_NAME, DOCKER_MASKED_PATHS,
    DOCKER_MOUNTS, DOCKER_READ_ONLY_PATHS, DOCKER_UMASK, DockerMount, EXIT_NOT_FOUND,
    EXIT_NOT_RUNNABLE, EXIT_START_FAILED, FileRecord, MQUEUE_MOUNT, NULL_PATH, NewFile, PROC_MOUNT,
    SHM_MOUNT, SYS_MOUNT, Seccomp, Start, StartOptions, TimeOffsets,
};
pub use counters::reason;
pub use credentials::{
    CapabilitySets, DOCKER_CAPABILITY_SETS, DOCKER_DEFAULT as DOCKER_CAPABILITIES, ROOT_GID,
    ROOT_UID,
};
pub use ends::Standard;
pub(crate) use errno::Errno;
pub use executing::{KernelConfig, KernelProcess, KernelRunner, KernelWithExecutor};
pub use guest_machine::GuestMachine;
pub use linux_extra::{
    ADDR_NO_RANDOMIZE, IOPRIO_CLASS_BE, IOPRIO_CLASS_SHIFT, IPC_NOWAIT, MAX_RW_COUNT, sched_attr,
};
pub use memory::PLACEMENT_TIERS;
pub use process::DOCKER_LIMITS;
pub use suspend::{ContextOrigin, ProcessContext, SuspendedTask};
pub use syscall::tty::PTMX_PATH;
pub use system::{
    AccessKind, AnonymousMemoryId, DeviceId, InodeNumber, MemoryError, MemoryObject, Process,
    ProcessInfo, ReplayDeterminism, SignalInformation, System, SystemCallResponse,
    SystemVSegmentId, TaskId, ThreadOrdering, Unsupported, WatchesFor, task_id_after,
};
pub use user::ADDRESS_LIMIT as ARM64_ADDRESS_SPACE_BYTES;

use counters::Counters;
use fs::{FileSystem, Node, Numbers, Time};
use process::{Pid, Thread, Tid};

/// What a call into the kernel may use besides the kernel's own state.
pub(crate) struct Context<'a, S> {
    pub cpu: &'a mut dyn Cpu,
    pub host: &'a S,
    /// The same system, for the calls the kernel keeps while it waits on them.
    pub system: &'a alloc::sync::Arc<S>,
    /// What is added to the host's monotonic clock to give the kernel's (see
    /// [`Kernel::restore`]).
    monotonic_offset: u64,
    /// Requests the CPU side answered it cannot do at all, counted when the kernel hands back control.
    refused: u32,
}

impl<S: System> Context<'_, S> {
    /// What a refused request to make a memory, thread or shared block means to the guest; a host
    /// that cannot do it at all is counted.
    pub fn cpu_error(&mut self, error: wcontainers_abi::CpuError) -> Errno {
        if error == wcontainers_abi::CpuError::NotSupported {
            self.refused += 1;
        }
        errno::from_cpu(error)
    }

    /// Fills `into` from the system's random source. Linux's randomness never fails, so neither
    /// may the kernel's: a failed source stops the kernel.
    pub fn random(&self, into: &mut [u8]) {
        if let Err(error) = self.host.fill_random_bytes(&core::cell::RefCell::new(into)) {
            panic!("the system's random source failed: {error:?}");
        }
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
    /// The memory size reported.
    pub memory_bytes: u64,
    /// How many processors programs are told are online, numbered from 0: the kernel's
    /// parallelism.
    pub processors: core::num::NonZeroUsize,
    /// The container's address-space size: one past the highest address a process may use.
    pub address_space_bytes: u64,
    /// The top of the lowest placement tier this executor gives, which layouts are made under.
    pub lowest_tier_top: u64,
}

impl Machine {
    /// `kernel.threads-max`: as many threads as an eighth of memory holds stacks of
    /// `THREAD_SIZE` (`set_max_threads`), at least `MIN_THREADS` and at most `FUTEX_TID_MASK`.
    pub fn threads_max(&self) -> u64 {
        use crate::linux_extra::{MIN_THREADS, THREAD_SIZE};
        (self.memory_bytes / (THREAD_SIZE * 8)).clamp(
            MIN_THREADS,
            u64::from(crate::linux::general::FUTEX_TID_MASK),
        )
    }
}

/// The kernel's file systems that exist from the start.
pub(crate) struct Files {
    pub numbers: Numbers,
    pub root: Node,
    pub proc_fs: FileSystem,
    /// nsfs, mounted at boot with the fourth anonymous device number (measured).
    pub nsfs: FileSystem,
    pub pipe_fs: FileSystem,
    /// The internal tmpfs memory files and shared anonymous memory live on (Linux's `shm_mnt`, mounted
    /// first at boot, so it has the first anonymous device number).
    pub memfd_fs: FileSystem,
    /// The internal file system secret memory files live on, made at the first `memfd_secret`.
    pub secretmem_fs: Option<FileSystem>,
    /// The internal hugetlbfs of each huge page size, by page shift, made at its first file.
    pub hugetlb_fs: alloc::collections::BTreeMap<u32, FileSystem>,
    /// The file system process descriptors are on, once one is made.
    pub pidfs: Option<FileSystem>,
    /// The anonymous inode, once a file needs it.
    pub anon: Option<crate::fs::Node>,
    /// The file system sockets are on, once one is made.
    pub sockfs: Option<FileSystem>,
    /// Files whose bytes are partly in a block of shared pages, while a mapping shares them.
    pub shared: Vec<Node>,
    /// The key tmpfs inodes' generation numbers are made from, as Linux draws each at random.
    pub generation_key: [u8; 16],
    /// Every event descriptor's counter, for the lowest free number a new one takes.
    pub event_counters:
        Vec<alloc::sync::Weak<atomic_refcell::AtomicRefCell<syscall::eventfd::Counter>>>,
    /// Changed attributes of the `/proc` entries that keep them.
    pub proc_kept: BTreeMap<u64, fs::proc::Kept>,
    /// What Docker mounts over entries of `/proc`'s root, by name.
    pub proc_masked: Vec<(&'static [u8], fs::Node)>,
    /// Every tmpfs's count of what it holds, for the machine's memory in use.
    pub spaces: Vec<Arc<fs::Space>>,
    /// Pseudo-terminal pairs by number, until both sides have closed.
    pub terminals: BTreeMap<u32, Arc<atomic_refcell::AtomicRefCell<tty::Pty>>>,
    /// The connection to the file backend, once one is mounted.
    pub backend: Option<Arc<fs::backend::Client>>,
    /// The directories file systems are mounted on.
    pub mount_points: Vec<Node>,
    /// The mount table, in the order the mounts were made.
    pub mounts: Vec<fs::mounts::Mount>,
}

/// All kernel state. It is plain data, so a container is saved by copying it with the guest memories.
pub struct Kernel {
    processes: BTreeMap<Pid, process::Process>,
    threads: BTreeMap<Tid, Thread>,
    /// Threads that can run and are not the current one.
    run_queue: schedule::RunQueue,
    /// The thread that has the CPU: it runs again after its call unless another should take over.
    current: Option<Tid>,
    /// Whether the current thread should give way at the next choice (Linux's `need_resched`).
    resched: bool,
    /// The thread the driver is running, between [`Kernel::next`] and [`Kernel::exit`].
    running: Option<Tid>,
    /// The thread whose call, signal or fault the kernel is in the middle of, which a hook cannot
    /// take unless its call has not begun.
    stepping: Option<Tid>,
    /// The stepping thread's call has not begun: a hook may take it, to make the call again.
    call_not_begun: bool,
    /// A task the kernel is starting, which a hook cannot take.
    starting_task: Option<Tid>,
    /// A process the kernel is ending, whose tasks a hook cannot take.
    ending_process: Option<Pid>,
    /// Parked threads by channel and deadline.
    parked: schedule::Parked,
    /// Files closed with a descriptor table whose last holder has gone, for the kernel to finish
    /// releasing.
    released: Vec<file::FileRef>,
    /// Notices that such closes took message queue requests away, for the kernel to send.
    notices: Vec<(syscall::socket::SocketRef, alloc::vec::Vec<u8>)>,
    /// The open files and descriptors whose owners F_NOTIFY requests owe a signal, in order.
    dnotified: Vec<(
        alloc::sync::Weak<atomic_refcell::AtomicRefCell<file::OpenFile>>,
        i32,
    )>,
    /// Every process's asynchronous I/O contexts.
    aio: Vec<syscall::aio::Ring>,
    /// A file's readiness changed while asynchronous poll requests wait, for the kernel to look
    /// at them again.
    aio_polls_due: bool,
    /// Parked threads whose wait ended, in line to run, whose calls are run again when their turn
    /// comes.
    woken: VecDeque<Tid>,
    /// Threads not running that were sent a signal, which they act on before they run.
    signalled: VecDeque<Tid>,
    /// Signals the embedder sent to a thread of a process, with their information, sent when the
    /// kernel settles.
    signals_sent: VecDeque<(Pid, Tid, siginfo::Bytes)>,
    /// Signals the kernel sent a process while its context was away, sent to it when the kernel
    /// settles after it is back.
    signals_to_returning: VecDeque<(Pid, u32, siginfo::Bytes)>,
    /// Processes sent a fatal signal while their memory is watched, with the signal: their
    /// threads end of it on their way back to the program, once the kernel's stores at the end
    /// have met the watches (`SIGNAL_GROUP_EXIT`).
    dying: BTreeMap<Pid, u32>,
    /// What the kernel wrote into processes' memory and has not yet told the embedder.
    kernel_writes: atomic_refcell::AtomicRefCell<Vec<system::KernelWrite>>,
    /// What is added to the host's monotonic clock to give the kernel's: zero until a restore on a
    /// host whose clock reads differently.
    monotonic_offset: u64,
    /// The wall clock less the kernel's monotonic clock (`ktime_mono_to_real`), as at the start or
    /// the last restore; a restore that finds it changed has seen the wall clock set.
    wall_offset: u64,
    /// The monotonic clock when the kernel was last entered.
    entry_now: u64,
    /// The monotonic clock when the kernel last handed control back, which is when the thread it
    /// picks next starts running.
    last_now: u64,
    /// The current thread's CPU time when it was picked, for how much of its slice is used.
    picked_runtime: u64,
    /// The last process or thread number handed out. Both share one space, as in Linux.
    last_id: i32,
    /// The highest address the executor can give a process now (at most the address space's end).
    ceiling: u64,
    /// Some process has watched pages or a record of written pages: a call, a signal or a fault
    /// then looks at its own process's.
    watching: bool,
    /// How many times any page's watch was raised (set, or widened to `All`): a walk over what a
    /// call will touch starts again when this moves under it.
    watches_raised: u64,
    /// The embedder's watches changed outside a call into the kernel and woke threads, which the
    /// kernel settles before anything runs.
    unsettled: bool,
    /// Numbers that may no longer be carried, given back to the embedder when the kernel settles.
    ids_let_go: Vec<i32>,
    /// What every address map notes for the kernel: a placement that found no room below the
    /// ceiling, a shared mapping put in or taken out.
    map_notes: Arc<memory::MapNotes>,
    /// What the embedder has been told each process maps of shared memory.
    shared_announced: sharing::Announced,
    /// Counts the kernel's rounds of shared memory announcements, so a round sees one begun
    /// inside its hooks.
    sharing_round: u32,
    /// Processes whose context was taken off this kernel and not put back on it.
    processes_away: BTreeMap<Pid, suspend::ProcessAway>,
    /// The address maps of contexts taken or copied off this kernel, with their memory: what
    /// they map stays in use while they live, until one is put back here.
    maps_away: atomic_refcell::AtomicRefCell<
        Vec<(
            wcontainers_abi::MemoryId,
            alloc::sync::Weak<atomic_refcell::AtomicRefCell<memory::AddressMap>>,
        )>,
    >,
    /// Processes on their way out whose shared memory has been announced ended.
    sharing_left: alloc::collections::BTreeSet<Pid>,
    /// The container's first process; when it ends the container ends.
    init: Option<Pid>,
    /// The first process's wait status once it has ended.
    finished: Option<i32>,
    /// The container's start while it waits for the backend.
    starting: Option<alloc::boxed::Box<container::Starting>>,
    /// Why the container's start failed.
    start_failure: Option<Errno>,
    /// Every inotify instance made, with the user it is charged to.
    inotify_instances: Vec<(
        u32,
        alloc::sync::Weak<atomic_refcell::AtomicRefCell<syscall::inotify::Instance>>,
    )>,
    /// The core dumps being written, by the thread that writes each.
    dumps: BTreeMap<process::Tid, syscall::coredump::Dump>,
    /// The last serial given to committed credentials; 0 is the credentials of whoever opened the
    /// container's first descriptors.
    last_credentials: u64,
    counters: Counters,
    files: Files,
    /// Each user's count of queued signals, by real user id.
    signal_users: BTreeMap<u32, Arc<crossbeam_utils::atomic::AtomicCell<u64>>>,
    /// Each user's count of pages its pipes may hold (`user_struct`'s `pipe_bufs`), by real user id.
    pipe_users: BTreeMap<u32, Arc<crossbeam_utils::atomic::AtomicCell<u64>>>,
    /// The container's System V IPC objects.
    ipc: syscall::ipc::Ipc,
    /// Every futex with waiters.
    futexes: syscall::futex::Futexes,
    /// Every timer.
    timers: syscall::timer::Timers,
    /// The epoll items watching each file's readiness.
    poll_watches: syscall::epoll::Watches,
    /// The names local sockets are bound to.
    sockets: syscall::socket::Names,
    /// The byte streams to the embedder.
    ends: ends::Ends,
    machine: Machine,
    hostname: Vec<u8>,
    domainname: Vec<u8>,
    /// Whether Docker's default seccomp profile answers every call before the threads' filters.
    docker_profile: bool,
    /// The offsets of the container's own time namespace, when it has one.
    time_offsets: Option<TimeOffsets>,
    /// The wall clock time and the monotonic clock when the container started.
    boot: Time,
    boot_monotonic: u64,
    boot_id: [u8; 16],
    /// The last cookie that ties a rename's two inotify events together.
    move_cookie: u32,
    /// The last number given to a kernel object as its identity (see [`Kernel::next_object`]).
    last_object: u64,
    /// The calls the kernel waits on its system for.
    waiting: waiting::Waiting,
    /// The system the backend's requests under way were handed to, by its address: a restore on
    /// another drops them and asks again.
    backend_system: usize,
    /// The kernel's wait for the backend's notices.
    backend_notices: fs::backend::NoticeWait,
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
            stepping: None,
            call_not_begun: false,
            starting_task: None,
            ending_process: None,
            parked: schedule::Parked::default(),
            released: Vec::new(),
            notices: Vec::new(),
            dnotified: Vec::new(),
            aio: Vec::new(),
            aio_polls_due: false,
            woken: VecDeque::new(),
            signalled: VecDeque::new(),
            dying: BTreeMap::new(),
            kernel_writes: Default::default(),
            monotonic_offset: 0,
            wall_offset: 0,
            entry_now: 0,
            last_now: 0,
            picked_runtime: 0,
            last_id: 0,
            ceiling: 0,
            watching: false,
            watches_raised: 0,
            unsettled: false,
            signals_sent: VecDeque::new(),
            signals_to_returning: VecDeque::new(),
            ids_let_go: Vec::new(),
            map_notes: Arc::default(),
            shared_announced: sharing::Announced::new(),
            sharing_round: 0,
            processes_away: BTreeMap::new(),
            maps_away: atomic_refcell::AtomicRefCell::default(),
            sharing_left: alloc::collections::BTreeSet::new(),
            init: None,
            finished: None,
            starting: None,
            start_failure: None,
            dumps: BTreeMap::new(),
            inotify_instances: Vec::new(),
            last_credentials: 0,
            counters: Counters::default(),
            ipc: Default::default(),
            futexes: Default::default(),
            timers: Default::default(),
            poll_watches: Default::default(),
            sockets: Default::default(),
            ends: Default::default(),
            signal_users: BTreeMap::new(),
            pipe_users: BTreeMap::new(),
            files: Files {
                numbers,
                root,
                proc_fs,
                nsfs: FileSystem {
                    kind: fs::Kind::Nsfs,
                    device: fs::encode_device(0, 4),
                    flags: 0,
                    space: None,
                },
                pipe_fs,
                memfd_fs: FileSystem {
                    kind: fs::Kind::Tmpfs,
                    device: fs::encode_device(0, 1),
                    flags: 0,
                    space: Some(memfd_space),
                },
                secretmem_fs: None,
                hugetlb_fs: Default::default(),
                pidfs: None,
                anon: None,
                sockfs: None,
                shared: Vec::new(),
                generation_key: [0; 16],
                event_counters: Vec::new(),
                proc_kept: BTreeMap::new(),
                proc_masked: Vec::new(),
                spaces,
                terminals: BTreeMap::new(),
                backend: None,
                mount_points: Vec::new(),
                mounts: Vec::new(),
            },
            machine: Machine {
                release: "7.0.0-wcontainers",
                version: "#1 SMP PREEMPT",
                memory_bytes: 0,
                processors: core::num::NonZeroUsize::MIN,
                address_space_bytes: 0,
                lowest_tier_top: 0,
            },
            hostname: Vec::new(),
            domainname: container::DOCKER_DOMAIN_NAME.to_vec(),
            docker_profile: true,
            time_offsets: None,
            boot: Time::default(),
            boot_monotonic: 0,
            boot_id: [0; 16],
            move_cookie: 0,
            last_object: 0,
            waiting: waiting::Waiting::default(),
            backend_system: 0,
            backend_notices: fs::backend::NoticeWait::NotYet,
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
        // Come back at once ([`Event::Timer`]), so the woken threads, and the answers of the calls
        // the kernel waits on, are settled first.
        if self.unsettled || self.waiting.woken() {
            return Next::Idle {
                until_nanoseconds: Some(0),
            };
        }
        self.pick()
    }

    /// The thread from the last [`Next::Run`] has come back.
    pub fn exit<S: System>(&mut self, cpu: &mut dyn Cpu, system: &alloc::sync::Arc<S>, exit: Exit) {
        let tid = self
            .running
            .take()
            .expect("an exit was handed back with no thread running");
        let mut cx = Context {
            cpu,
            host: &**system,
            system,
            monotonic_offset: self.monotonic_offset,
            refused: 0,
        };
        self.account(&mut cx, tid);
        let entered = self.entry_now;
        self.tick(tid);
        self.wake_due();
        syscall::timer::fire_due(self, &mut cx);
        // A timer's signal or the CPU time limit may have killed the thread's process.
        if self.threads.contains_key(&tid) {
            syscall::timer::cpu_check(self, &mut cx, tid);
        }
        if self.threads.contains_key(&tid) {
            self.stepping = Some(tid);
            self.tell_continued(&mut cx, tid);
            self.came_back(&mut cx, tid, exit);
            self.stepping = None;
        }
        self.settle(&mut cx);
        self.account_system(tid, entered);
        // The kernel's time may give the turn to a woken thread, whose call runs before it goes on.
        if self
            .next_to_run()
            .is_some_and(|next| self.woken.contains(&next))
        {
            self.settle(&mut cx);
        }
    }

    /// Something happened outside.
    pub fn event<S: System>(
        &mut self,
        cpu: &mut dyn Cpu,
        system: &alloc::sync::Arc<S>,
        event: Event,
    ) {
        let mut cx = Context {
            cpu,
            host: &**system,
            system,
            monotonic_offset: self.monotonic_offset,
            refused: 0,
        };
        self.entry_now = cx.monotonic();
        match event {
            Event::Timer => {
                self.wake_due();
                syscall::timer::fire_due(self, &mut cx);
            }
            Event::TerminalResized { columns, rows } => {
                self.terminal_resized(&mut cx, columns, rows);
            }
        }
        self.settle(&mut cx);
    }

    /// Sends a signal to the container's first process from outside the container, as `docker
    /// kill` does: from no process the container can see, and SIGKILL ends it even though Linux keeps
    /// other signals with no handler from that process.
    pub fn kill<S: System>(
        &mut self,
        cpu: &mut dyn Cpu,
        system: &alloc::sync::Arc<S>,
        signal: wcontainers_kernel::signal::Signal,
    ) {
        let signal = signal as u32;
        assert!(self.running.is_none(), "a signal sent while a thread runs");
        let mut cx = Context {
            cpu,
            host: &**system,
            system,
            monotonic_offset: self.monotonic_offset,
            refused: 0,
        };
        self.entry_now = cx.monotonic();
        if let Some(pid) = self.init.filter(|_| self.finished.is_none()) {
            if signal == linux::general::SIGKILL {
                self.die_of(&mut cx, pid, signal);
            } else {
                let info = signal::sender_info(signal, linux::general::SI_USER as i32, 0, 0);
                self.send_to_process(&mut cx, pid, signal, info);
            }
        }
        self.settle(&mut cx);
    }

    /// Brings back a saved container, on a CPU side restored from the same save (every memory, block
    /// and thread's registers under the same handles, nothing else) and a host whose clocks may read
    /// anything: the kernel's monotonic clock goes on from where it was saved, every thread starts
    /// again from its saved registers, and every parked call runs again, its deadline on the clock
    /// it had.
    pub fn restore<S: System>(&mut self, cpu: &mut dyn Cpu, system: &alloc::sync::Arc<S>) {
        assert!(self.running.is_none(), "restored while a thread runs");
        self.monotonic_offset = self.last_now.wrapping_sub(system.monotonic_nanoseconds());
        let mut cx = Context {
            cpu,
            host: &**system,
            system,
            monotonic_offset: self.monotonic_offset,
            refused: 0,
        };
        self.entry_now = cx.monotonic();
        self.backend_restored(&mut cx);
        // The wall clock moved against the monotonic clock while saved, as across a suspend, which
        // Linux's resume treats as a setting of the wall clock (`timerfd_resume`).
        let wall_offset = cx.host.realtime_nanoseconds().wrapping_sub(self.entry_now);
        let moved = wall_offset.wrapping_sub(self.wall_offset) as i64;
        self.wall_offset = wall_offset;
        let tids: Vec<Tid> = self.threads.keys().copied().collect();
        for &tid in &tids {
            let thread = self.threads.get_mut(&tid).expect("thread");
            // A parked thread's registers still hold its call; it gets its value when the call ends.
            if thread.call.wait.is_some() {
                self.wake_thread(tid);
                continue;
            }
            if let wcontainers_abi::Resume::Return(value) = thread.resume {
                // A call a stop interrupted keeps its restart code: the signals acted on when the
                // process is continued turn it into the call again or EINTR, and set the registers.
                if errno::Errno::restart_of(value).is_some() {
                    continue;
                }
                let mut registers = wcontainers_abi::RegisterBlock::ZERO;
                cx.cpu.registers(thread.cpu, &mut registers);
                registers.x[0] = value;
                cx.cpu.set_registers(thread.cpu, &registers);
            }
            thread.resume = wcontainers_abi::Resume::FromRegisters;
        }
        if moved != 0 {
            self.clock_was_set(moved, &tids);
        }
        // The new host knows none of the old one's connections, and none of its waits.
        syscall::socket::restored(self, &mut cx);
        self.ends_restored();
        self.settle(&mut cx);
    }

    /// `clock_was_set`: the wall clock moved by `delta` against the monotonic clock. Calls and
    /// timers whose times are on it move with it, so those whose time has come end.
    fn clock_was_set(&mut self, delta: i64, tids: &[Tid]) {
        let shift =
            |at: u64| (i128::from(at) - i128::from(delta)).clamp(0, i128::from(u64::MAX)) as u64;
        for tid in tids {
            let thread = self.thread_mut(*tid);
            if thread.call.wall {
                thread.call.deadline = thread.call.deadline.map(shift);
            }
            if let Some(process::Restart::Futex {
                until, wall: true, ..
            }) = &mut thread.restart
            {
                *until = shift(*until);
            }
        }
        syscall::timer::clock_was_set(self, delta);
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

    /// A free process and thread number from the embedder's task ids, skipping any still carried
    /// as Linux's `alloc_pid` does (counted); EAGAIN for a number out of range, or once every
    /// number has been tried.
    fn allocate_id<S: crate::System>(&mut self, cx: &mut Context<'_, S>) -> Result<i32, Errno> {
        for _ in RESERVED_PIDS..PID_MAX {
            let id = crate::system::pid_of(
                cx.host
                    .allocate_task_id(crate::system::task_id(self.last_id)),
            );
            if !(1..PID_MAX).contains(&id) {
                self.counters
                    .count(cx.host, reason::TASK_ID_OUT_OF_RANGE, id as u32 as u64);
                return Err(Errno::EAGAIN);
            }
            self.last_id = id;
            if !self.carried(id) {
                return Ok(id);
            }
            self.counters
                .count(cx.host, reason::TASK_ID_CARRIED, id as u64);
        }
        Err(Errno::EAGAIN)
    }

    /// Whether a process, thread, process group or session carries a number: a task or process
    /// taken off the kernel counts.
    fn carried(&self, id: i32) -> bool {
        self.processes.contains_key(&id)
            || self.threads.contains_key(&id)
            || self.processes_away.contains_key(&id)
            || self.processes.values().any(|process| {
                process.group == id || process.session == id || process.away.contains_key(&id)
            })
            || self.processes_away.values().any(|away| away.group == id)
    }

    /// Gives back to the embedder the numbers let go of that nothing carries any more.
    pub(crate) fn release_ids<S: crate::System>(&mut self, cx: &mut Context<'_, S>) {
        let mut ids = core::mem::take(&mut self.ids_let_go);
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            if self.finished.is_some() || !self.carried(id) {
                cx.host.free_task_id(crate::system::task_id(id));
            }
        }
    }

    /// Notes the numbers a process carried, which may be let go of with it.
    pub(crate) fn let_go_of_ids(&mut self, process: &process::Process) {
        self.ids_let_go
            .extend([process.pid, process.group, process.session]);
    }

    pub(crate) fn thread_mut(&mut self, tid: Tid) -> &mut Thread {
        self.threads.get_mut(&tid).expect("thread exists")
    }

    pub(crate) fn process_of(&self, tid: Tid) -> &process::Process {
        &self.processes[&self.threads[&tid].process]
    }

    pub(crate) fn process_of_mut(&mut self, tid: Tid) -> &mut process::Process {
        let pid = self.threads[&tid].process;
        self.processes.get_mut(&pid).expect("process exists")
    }

    /// Thread `tid`'s descriptor table.
    pub(crate) fn files(&self, tid: Tid) -> atomic_refcell::AtomicRef<'_, file::FdTable> {
        self.threads[&tid].files.borrow()
    }

    /// The open file descriptor `fd` names, taken once per call (`fdget`): a call that waits
    /// takes the same file in its later runs, whatever the table holds by then.
    pub(crate) fn call_file(&self, tid: Tid, fd: i32) -> Result<file::FileRef, Errno> {
        let thread = &self.threads[&tid];
        thread
            .taken
            .borrow_mut()
            .file(fd, || thread.files.borrow().file(fd))
    }

    /// A number for a new kernel object, unique in the container: what a guest sees of the object
    /// where Linux shows its address, so nothing it sees depends on where the host put it.
    pub(crate) fn next_object(&mut self) -> u64 {
        self.last_object += 1;
        self.last_object
    }

    /// The last number `next_object` gave.
    pub(crate) fn last_object(&self) -> u64 {
        self.last_object
    }

    /// A value `tid`'s call settles once, such as where an append goes (Linux computes it once
    /// under the inode's lock): an earlier run's, else `value`.
    pub(crate) fn settle_once(&self, tid: Tid, value: u64) -> u64 {
        self.threads[&tid].taken.borrow_mut().value(value)
    }

    pub(crate) fn files_mut(&self, tid: Tid) -> atomic_refcell::AtomicRefMut<'_, file::FdTable> {
        self.threads[&tid].files.borrow_mut()
    }

    /// Thread `tid`'s root, working directory and umask.
    pub(crate) fn fs(&self, tid: Tid) -> atomic_refcell::AtomicRef<'_, process::FsContext> {
        self.threads[&tid].fs.borrow()
    }

    pub(crate) fn fs_mut(&self, tid: Tid) -> atomic_refcell::AtomicRefMut<'_, process::FsContext> {
        self.threads[&tid].fs.borrow_mut()
    }

    /// Lets go of a thread's hold on a descriptor table; the last holder's going closes it, in its
    /// process `closer` (`put_files_struct`).
    pub(crate) fn put_files(&mut self, files: process::Files, closer: process::Pid) {
        if let Some(table) = Arc::into_inner(files) {
            let mut table = table.into_inner();
            table.close_all(closer);
            self.released.extend(table.take_released());
            self.notices.append(&mut table.notices);
            if table.take_let_go_locks() {
                self.wake(process::Channel::Locks);
            }
        }
    }

    /// Gives thread `tid` a copy of its descriptor table if it shares it (`unshare_fd`); the copy's
    /// record locks are its own.
    pub(crate) fn unshare_files(&mut self, tid: Tid) {
        let thread = self.thread_mut(tid);
        if Arc::strong_count(&thread.files) > 1 {
            let copy = thread.files.borrow().duplicate(tid as u64);
            thread.files = Arc::new(atomic_refcell::AtomicRefCell::new(copy));
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
            nsfs: self.files.nsfs.clone(),
            boot: self.boot,
            kept: &self.files.proc_kept,
            masked: &self.files.proc_masked,
            counters: &self.counters,
            docker_profile: self.docker_profile,
            time_offsets: self.time_offsets,
        };
        let walk = fs::Walk {
            root: &context.root,
            credentials: &thread.credentials,
            proc: &view,
            internal: core::cell::Cell::new(false),
        };
        f(&walk, &context)
    }

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
        maps.sort_by_key(|map| Arc::as_ptr(map));
        maps.dedup_by(|a, b| Arc::ptr_eq(a, b));
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

    /// The time now on the wall clock.
    pub(crate) fn now<S: crate::System>(&self, cx: &mut Context<'_, S>) -> Time {
        Time::from_nanoseconds(cx.host.realtime_nanoseconds())
    }
}

/// The largest process number plus one (`pid_max`'s default of `PID_MAX_DEFAULT`, which
/// `/proc/sys/kernel/pid_max` reports), and the numbers skipped when they wrap.
pub(crate) const PID_MAX: i32 = 4_194_304;
pub(crate) const RESERVED_PIDS: i32 = 300;

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
