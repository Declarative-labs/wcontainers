//! Taking tasks and process contexts off a kernel, and putting them on a kernel again.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use wcontainers_abi::{Cpu, MemoryId, RegisterBlock, Resume};

use crate::counters::reason;
use crate::linux::general::{SIGCHLD, SIGRTMIN};
use crate::process::{Away, Call, Channel, Leader, Map, Pending, Pid, Thread, Tid};
use crate::system::{ExecutionConfig, Process, TaskId};
use crate::{Kernel, sharing, syscall};

/// A task taken off a kernel: its own records, its registers, and where it was in line.
pub struct SuspendedTask {
    id: Tid,
    thread: alloc::boxed::Box<Thread>,
    registers: alloc::boxed::Box<RegisterBlock>,
    place: Place,
    /// It had a signal to act on before it runs.
    signalled: bool,
}

impl SuspendedTask {
    pub fn id(&self) -> TaskId {
        TaskId::of(self.id)
    }

    /// The process whose context the task needs.
    pub fn process(&self) -> TaskId {
        TaskId::of(self.thread.process)
    }
}

/// Where a task was in line when it was taken.
#[derive(Clone, Copy)]
enum Place {
    /// Parked in its call, which runs again, interrupted, when it is put back; or waiting outside
    /// a call, which goes on waiting.
    Waiting,
    /// Its wait had ended; its call runs again first, interrupted.
    Woken,
    /// Waiting for its turn, or having it.
    Ready,
}

/// What the tasks of one process share, as the kernel keeps it: the memory and its map, the
/// signal handlers, the limits and the rest of the process's record, and what the embedder has
/// been told the process maps of shared memory.
pub struct ProcessContext {
    process: crate::process::Process,
    announced: Vec<((crate::MemoryObject, Pid), sharing::Pieces)>,
    origin: ContextOrigin,
}

/// How a process context was made: the process left the kernel with it, or it is a copy and the
/// process stayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextOrigin {
    Suspended,
    Copied,
}

impl ProcessContext {
    pub fn id(&self) -> TaskId {
        TaskId::of(self.process.pid)
    }

    pub fn origin(&self) -> ContextOrigin {
        self.origin
    }
}

/// What a kernel keeps of a process whose context was taken off it, so that its parent's wait
/// sees a live child.
pub(crate) struct ProcessAway {
    pub parent: Pid,
    pub parent_thread: Tid,
    pub group: Pid,
    pub exit_signal: u32,
    /// The `vfork` parent its first thread keeps waiting.
    pub vfork_parent: Option<Tid>,
    /// Signals the kernel sent the process while it was away (a child's end), sent again when
    /// it is back.
    pub signals: Vec<(u32, [u8; SIGNAL_INFORMATION_SIZE])>,
}

const SIGNAL_INFORMATION_SIZE: usize = size_of::<crate::system::SignalInformation>();

impl Kernel {
    /// Takes the tasks named, as of one moment; a task not on this kernel is absent from the
    /// answer.
    pub fn suspend_tasks(
        &mut self,
        cpu: &mut dyn Cpu,
        tasks: &[TaskId],
    ) -> alloc::vec::IntoIter<(TaskId, SuspendedTask)> {
        assert!(
            self.running.is_none(),
            "tasks suspended while a thread runs"
        );
        self.take_tasks(cpu, None, tasks).into_iter()
    }

    /// Takes a process's context and every task of it on this kernel, which then has none of
    /// it; none for a process not here.
    pub fn suspend_process(
        &mut self,
        cpu: &mut dyn Cpu,
        process: TaskId,
    ) -> Option<(
        ProcessContext,
        alloc::vec::IntoIter<(TaskId, SuspendedTask)>,
    )> {
        assert!(
            self.running.is_none(),
            "a process suspended while a thread runs"
        );
        let (context, tasks) = self.take_process(cpu, process.pid())?;
        Some((context, tasks.into_iter()))
    }

    /// A copy of a process's context; nothing stops.
    pub fn copy_process_context(&self, process: TaskId) -> Option<ProcessContext> {
        let pid = process.pid();
        self.processes
            .get(&pid)
            .filter(|record| record.ended.is_none())?;
        Some(self.context_copy(pid))
    }

    /// Puts a process's context on this kernel, run as `config` says; hands it back if that
    /// process is already here or the kernel cannot run it so.
    pub fn resume_process(
        &mut self,
        context: ProcessContext,
        config: ExecutionConfig,
    ) -> Option<ProcessContext> {
        if self.processes.contains_key(&context.process.pid) || config.supported().is_err() {
            return Some(context);
        }
        let ProcessContext {
            mut process,
            announced,
            ..
        } = context;
        process.execution = config;
        process.threads.clear();
        // A parent that ended while it was away gave it to a reaper.
        if let Some(away) = self.processes_away.remove(&process.pid) {
            process.parent = away.parent;
            process.parent_thread = away.parent_thread;
            process.exit_signal = away.exit_signal;
            let pid = process.pid;
            self.signals_to_returning.extend(
                away.signals
                    .into_iter()
                    .map(|(signal, info)| (pid, signal, info)),
            );
            self.unsettled = true;
        }
        process
            .map
            .borrow_mut()
            .note_to(alloc::rc::Rc::clone(&self.map_notes));
        self.maps_away
            .borrow_mut()
            .retain(|(_, map)| map.strong_count() > 0 && map.as_ptr() != Rc::as_ptr(&process.map));
        // What it maps of shared memory is compared with what it was told when the kernel next
        // announces.
        self.map_notes.sharing_changed.set(true);
        self.shared_announced.extend(announced);
        let pid = process.pid;
        let unwanted = Kernel::children_unwanted(&process);
        self.processes.insert(pid, process);
        // Children that ended while it was away and that it does not want are reaped now.
        if unwanted {
            let zombies: Vec<Pid> = self
                .processes
                .values()
                .filter(|child| {
                    child.parent == pid && child.ended.is_some() && child.exit_signal == SIGCHLD
                })
                .map(|child| child.pid)
                .collect();
            for zombie in zombies {
                self.reap(zombie);
            }
        }
        self.update_watching();
        None
    }

    /// Puts tasks back on this kernel, each going on as it was taken; hands back those whose
    /// process context is not here.
    pub fn resume_tasks(
        &mut self,
        cpu: &mut dyn Cpu,
        tasks: impl IntoIterator<Item = SuspendedTask>,
    ) -> alloc::vec::IntoIter<SuspendedTask> {
        assert!(self.running.is_none(), "tasks resumed while a thread runs");
        let mut left = Vec::new();
        for task in tasks {
            if let Err(task) = self.place_task(cpu, task) {
                left.push(task);
            }
        }
        left.into_iter()
    }

    /// Whether a hook may take `tid` now: not a task the kernel is in the middle of a step for
    /// (a thread's call or signal, a task it starts, a process it ends), except a thread whose
    /// call has not begun.
    fn takeable(&self, tid: Tid) -> bool {
        let Some(thread) = self.threads.get(&tid) else {
            return false;
        };
        let pid = thread.process;
        let process = &self.processes[&pid];
        process.ended.is_none()
            && !self.dying.contains_key(&pid)
            && self.ending_process != Some(pid)
            && self.starting_task != Some(tid)
            && (self.stepping != Some(tid) || self.call_not_begun)
    }

    /// Takes the tasks of `tasks` that are on this kernel (of process `only`, if given); one the
    /// kernel cannot take now is counted.
    pub(crate) fn take_tasks(
        &mut self,
        cpu: &mut dyn Cpu,
        only: Option<Pid>,
        tasks: &[TaskId],
    ) -> Vec<(TaskId, SuspendedTask)> {
        let mut taken = Vec::new();
        for &task in tasks {
            let tid = task.pid();
            let Some(thread) = self.threads.get(&tid) else {
                continue;
            };
            if only.is_some_and(|pid| pid != thread.process) {
                continue;
            }
            if !self.takeable(tid) {
                if self.processes[&thread.process].ended.is_none() {
                    self.counters
                        .note(reason::TASK_IN_A_STEP_NOT_SUSPENDED, task.0.into());
                    self.unsettled = true;
                }
                continue;
            }
            taken.push((task, self.take_task(cpu, tid)));
        }
        taken
    }

    /// Whether process `pid` is being ended, so that it cannot leave the kernel.
    fn ending(&self, pid: Pid) -> bool {
        self.dying.contains_key(&pid) || self.ending_process == Some(pid)
    }

    /// Takes a process's context and its tasks; none for a process not here or being ended. With
    /// a task the kernel cannot take now, the process stays and the answer holds a copy of its
    /// context and the tasks taken.
    pub(crate) fn take_process(
        &mut self,
        cpu: &mut dyn Cpu,
        pid: Pid,
    ) -> Option<(ProcessContext, Vec<(TaskId, SuspendedTask)>)> {
        self.processes
            .get(&pid)
            .filter(|record| record.ended.is_none())?;
        if self.ending(pid) {
            return None;
        }
        let ids: Vec<TaskId> = self.processes[&pid]
            .threads
            .iter()
            .map(|&tid| TaskId::of(tid))
            .collect();
        let tasks = self.take_tasks(cpu, Some(pid), &ids);
        if !self.processes[&pid].threads.is_empty() {
            return Some((self.context_copy(pid), tasks));
        }
        let process = self.processes.remove(&pid).expect("process exists");
        let announced = self.take_announced(pid);
        // Its map stops noting to this kernel unless a process still here runs in it.
        let shared = self
            .processes
            .values()
            .any(|other| Rc::ptr_eq(&other.map, &process.map));
        if !shared {
            process.map.borrow_mut().stop_noting_to(&self.map_notes);
        }
        self.hold_map(&process);
        self.processes_away.insert(
            pid,
            ProcessAway {
                parent: process.parent,
                parent_thread: process.parent_thread,
                group: process.group,
                exit_signal: process.exit_signal,
                vfork_parent: process.away.get(&pid).and_then(|away| away.vfork_parent),
                signals: Vec::new(),
            },
        );
        self.update_watching();
        let context = ProcessContext {
            process,
            announced,
            origin: ContextOrigin::Suspended,
        };
        Some((context, tasks))
    }

    /// A copy of process `pid`'s context, with no tasks: its own map and signal handlers, every
    /// task of the process counted as away from it, and no pending signal, which stay with the
    /// process.
    pub(crate) fn context_copy(&self, pid: Pid) -> ProcessContext {
        let mut process = self.processes[&pid].clone();
        let mut map = process.map.borrow().clone();
        map.notes_to_none();
        process.map = Rc::new(RefCell::new(map));
        let actions = *process.actions.borrow();
        process.actions = Rc::new(RefCell::new(actions));
        self.hold_map(&process);
        process.pending = Pending::default();
        for away in process.away.values_mut() {
            away.pending = Pending::default();
        }
        for tid in core::mem::take(&mut process.threads) {
            let thread = &self.threads[&tid];
            if tid == pid {
                process.ended_leader = Leader::of(thread);
            }
            // It goes on running here: an end of the copy makes no last stores for it into the
            // memory it shares with the original.
            process.away.insert(
                tid,
                Away {
                    last_stores: None,
                    ..Away::of(thread)
                },
            );
        }
        let announced = self
            .shared_announced
            .iter()
            .filter(|((_, mapper), _)| *mapper == pid)
            .map(|(key, pieces)| (*key, pieces.clone()))
            .collect();
        ProcessContext {
            process,
            announced,
            origin: ContextOrigin::Copied,
        }
    }

    /// Keeps what a context's map shows in use while the context is off this kernel.
    fn hold_map(&self, process: &crate::process::Process) {
        let mut held = self.maps_away.borrow_mut();
        held.retain(|(_, map)| map.strong_count() > 0);
        held.push((process.memory, Rc::downgrade(&process.map)));
    }

    /// The maps of contexts off this kernel that still live, with their memory.
    pub(crate) fn live_maps_away(&self) -> Vec<(MemoryId, Map)> {
        self.maps_away
            .borrow()
            .iter()
            .filter_map(|(memory, map)| Some((*memory, map.upgrade()?)))
            .collect()
    }

    /// What the embedder was told process `pid` maps, taken out to go with its context.
    fn take_announced(&mut self, pid: Pid) -> Vec<((crate::MemoryObject, Pid), sharing::Pieces)> {
        let keys: Vec<_> = self
            .shared_announced
            .keys()
            .filter(|(_, mapper)| *mapper == pid)
            .copied()
            .collect();
        keys.into_iter()
            .map(|key| {
                let pieces = self.shared_announced.remove(&key).expect("announced");
                (key, pieces)
            })
            .collect()
    }

    /// Takes one task off the kernel. A task in a call is treated as Linux treats a stop arriving
    /// in it: the call's next run answers as interrupted by a signal with no handler, and a waiter
    /// in a line a wake chooses from (a futex's, a semaphore set's, a message queue's) leaves it
    /// now. A thread taken before its call began makes the call again.
    fn take_task(&mut self, cpu: &mut dyn Cpu, tid: Tid) -> SuspendedTask {
        let mut registers = RegisterBlock::ZERO;
        cpu.registers(self.threads[&tid].cpu, &mut registers);
        let wait = self.threads[&tid].call.wait;
        let mut place = if wait.is_some() {
            Place::Waiting
        } else if self.woken.contains(&tid) {
            Place::Woken
        } else {
            Place::Ready
        };
        let signalled = self.signalled.contains(&tid);
        self.unschedule(tid);
        if self.stepping == Some(tid) {
            self.stepping = None;
            registers.pc -= size_of::<u32>() as u64;
            let thread = self.thread_mut(tid);
            thread.call = Call::default();
            thread.resume = Resume::FromRegisters;
            place = Place::Ready;
        }
        if matches!(place, Place::Waiting) {
            if wait == Some(Channel::Futex) {
                syscall::futex::leave(self, tid);
            }
            syscall::sem::leave(self, tid);
            if syscall::mqueue::queued(self, tid) {
                syscall::mqueue::forget(self, tid);
            }
        }
        // A thread waiting outside a call (its own access, or its signal's frame, met a watch or
        // pages not here) has no call to interrupt: it touches the memory again when put back.
        if !matches!(place, Place::Ready) && !self.threads[&tid].filling {
            self.thread_mut(tid).suspended_in_call = true;
        }
        let thread = self.threads.remove(&tid).expect("thread exists");
        cpu.thread_drop(thread.cpu);
        let process = self
            .processes
            .get_mut(&thread.process)
            .expect("process exists");
        process.threads.retain(|other| *other != tid);
        if tid == thread.process {
            process.ended_leader = Leader::of(&thread);
        }
        process.away.insert(tid, Away::of(&thread));
        SuspendedTask {
            id: tid,
            thread: alloc::boxed::Box::new(thread),
            registers: alloc::boxed::Box::new(registers),
            place,
            signalled,
        }
    }

    /// Puts one task on the kernel, going on as it was taken; gives it back if its process
    /// context is not here, its number is taken, or the executor cannot make its thread.
    fn place_task(
        &mut self,
        cpu: &mut dyn Cpu,
        mut task: SuspendedTask,
    ) -> Result<(), SuspendedTask> {
        let tid = task.id;
        // A task whose process ended or ran a new program while it was away ended with it.
        let Some(process) = self
            .processes
            .get(&task.thread.process)
            .filter(|process| process.ended.is_none() && process.away.contains_key(&tid))
        else {
            return Err(task);
        };
        if self.threads.contains_key(&tid) {
            return Err(task);
        }
        let cpu_thread = match cpu.thread_new(process.memory) {
            Ok(thread) => thread,
            Err(error) => {
                if error == wcontainers_abi::CpuError::NotSupported {
                    self.counters.note(reason::HOST_CANNOT, 0);
                    self.unsettled = true;
                }
                return Err(task);
            }
        };
        cpu.set_registers(cpu_thread, &task.registers);
        task.thread.cpu = cpu_thread;
        let pid = task.thread.process;
        let process = self.processes.get_mut(&pid).expect("process exists");
        if tid == pid {
            process.threads.insert(0, tid);
        } else {
            process.threads.push(tid);
        }
        // What it was sent while away comes after what it had; a standard signal once.
        if let Some(away) = process.away.remove(&tid) {
            let pending = &mut task.thread.pending;
            for queued in away.pending.queue {
                let bit = crate::signal::bit(queued.signal);
                if queued.signal >= SIGRTMIN || pending.set & bit == 0 {
                    pending.queue.push_back(queued);
                }
            }
            pending.set |= away.pending.set;
        }
        let deliverable =
            (task.thread.pending.set | process.pending.set) & !task.thread.signal_mask != 0;
        let channel = task.thread.call.wait;
        let deadline = task.thread.call.deadline;
        let resume = task.thread.resume;
        self.threads.insert(tid, *task.thread);
        match (task.place, channel) {
            (Place::Waiting, Some(channel)) => {
                self.parked.insert(tid, channel, deadline);
                self.wake_thread(tid);
            }
            (Place::Woken, _) => {
                self.woken.push_back(tid);
                self.make_runnable(tid, resume);
            }
            _ => self.make_runnable(tid, resume),
        }
        let ready = matches!(task.place, Place::Ready);
        if (task.signalled || (ready && deliverable)) && !self.signalled.contains(&tid) {
            self.signalled.push_back(tid);
        }
        self.unsettled = true;
        Ok(())
    }
}

impl Process<'_> {
    /// As [`Kernel::suspend_tasks`] for this process's tasks.
    pub fn suspend_tasks(
        &mut self,
        tasks: &[TaskId],
    ) -> alloc::vec::IntoIter<(TaskId, SuspendedTask)> {
        self.kernel
            .take_tasks(self.cpu, Some(self.pid), tasks)
            .into_iter()
    }

    /// As [`Kernel::suspend_process`] for this process.
    pub fn suspend_process(
        self,
    ) -> (
        ProcessContext,
        alloc::vec::IntoIter<(TaskId, SuspendedTask)>,
    ) {
        // A process being ended stays, and its tasks with it, each counted.
        let (context, tasks) = match self.kernel.take_process(self.cpu, self.pid) {
            Some(taken) => taken,
            None => {
                let ids: Vec<TaskId> = self.kernel.processes[&self.pid]
                    .threads
                    .iter()
                    .map(|&tid| TaskId::of(tid))
                    .collect();
                let tasks = self.kernel.take_tasks(self.cpu, Some(self.pid), &ids);
                (self.kernel.context_copy(self.pid), tasks)
            }
        };
        (context, tasks.into_iter())
    }

    /// As [`Kernel::copy_process_context`] for this process.
    pub fn copy_process_context(&self) -> ProcessContext {
        self.kernel.context_copy(self.pid)
    }
}
