//! A Linux kernel for ARM64 guests.
//!
//! The kernel is a state machine driven from outside through `wcontainers-abi`. It never waits on its own
//! stack: the driver asks [`Kernel::next`] what to do, runs that thread on the CPU side until it comes
//! back, and hands the [`Exit`] to [`Kernel::exit`]. Answers from outside arrive through
//! [`Kernel::event`]. The kernel reaches the CPU side only through [`Cpu`] and everything else through
//! [`Host`].

#![no_std]

extern crate alloc;

mod counters;
mod errno;
pub mod linux;
mod loader;
mod process;
mod syscall;

use alloc::collections::{BTreeMap, VecDeque};

use wcontainers_abi::{CounterReading, Cpu, Event, Exit, Host, Next, Resume};

pub use counters::reason;
pub use errno::Errno;

use counters::Counters;
use process::{Pid, Process, Thread, Tid};

/// What a call into the kernel may use besides the kernel's own state.
pub(crate) struct Context<'a> {
    pub cpu: &'a mut dyn Cpu,
    pub host: &'a mut dyn Host,
}

/// All kernel state. It is plain data, so a container is saved by copying it with the guest memories.
#[derive(Default)]
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
}

impl Kernel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts the container's first process from the bytes of a program file.
    pub fn start(
        &mut self,
        cpu: &mut dyn Cpu,
        host: &mut dyn Host,
        program: &[u8],
        arguments: &[&[u8]],
        environment: &[&[u8]],
    ) -> Result<(), Errno> {
        assert!(self.init.is_none(), "the container has already started");
        let mut cx = Context { cpu, host };
        let pid = self.allocate_id();
        let image = loader::load(self, &mut cx, program, arguments, environment)?;
        let thread = cx.cpu.thread_new(image.memory).map_err(errno::from_cpu)?;
        cx.cpu.set_registers(thread, &image.registers);
        self.processes
            .insert(pid, Process::new(image.memory, pid, image.mappings));
        self.threads.insert(
            pid,
            Thread {
                process: pid,
                cpu: thread,
                resume: Resume::FromRegisters,
            },
        );
        self.runnable.push_back(pid);
        self.init = Some(pid);
        Ok(())
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
                until_nanoseconds: None,
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
                    self.thread_mut(tid).resume = Resume::Return(value);
                    self.runnable.push_front(tid);
                }
                syscall::Outcome::Gone => {}
            },
            Exit::Stop => {
                self.thread_mut(tid).resume = Resume::Continue;
                self.runnable.push_back(tid);
            }
            Exit::MemoryFault { .. } | Exit::MemoryFaultWithoutRegisters => {
                self.kill_by_signal(&mut cx, tid, linux::general::SIGSEGV)
            }
            Exit::IllegalInstruction => self.kill_by_signal(&mut cx, tid, linux::general::SIGILL),
            Exit::Breakpoint { .. } => self.kill_by_signal(&mut cx, tid, linux::general::SIGTRAP),
        }
    }

    /// Something happened outside. No thread can wait for anything yet, so every event is unexpected.
    pub fn event(&mut self, _cpu: &mut dyn Cpu, host: &mut dyn Host, event: Event<'_>) {
        let detail = match event {
            Event::Timer => 0,
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

    fn allocate_id(&mut self) -> i32 {
        self.last_id += 1;
        self.last_id
    }

    fn thread_mut(&mut self, tid: Tid) -> &mut Thread {
        self.threads.get_mut(&tid).expect("thread exists")
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

    /// A fault with no handler: the process dies of the signal, as Linux's default action does.
    fn kill_by_signal(&mut self, cx: &mut Context<'_>, tid: Tid, signal: u32) {
        let pid = self.threads[&tid].process;
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
