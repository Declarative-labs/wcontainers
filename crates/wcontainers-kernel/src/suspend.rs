//! Taking tasks and process contexts off a kernel, and putting them on a kernel again.

use alloc::vec::Vec;

use wcontainers_abi::{Cpu, RegisterBlock, Resume, SYSTEM_CALL_NUMBER_REGISTER};

use crate::counters::reason;
use crate::process::{Call, Channel, Pid, Restart, Thread, Tid};
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
    /// Parked in its call, which runs again when it is put back.
    Waiting,
    /// Its wait had ended; its call runs again first.
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
}

impl ProcessContext {
    pub fn id(&self) -> TaskId {
        TaskId::of(self.process.pid)
    }
}

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
        } = context;
        process.execution = config;
        process.threads.clear();
        process
            .map
            .borrow_mut()
            .note_to(alloc::rc::Rc::clone(&self.map_notes));
        self.shared_announced.extend(announced);
        self.processes.insert(process.pid, process);
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

    /// Takes a process's context and its tasks; with a task the kernel cannot take now, the
    /// process stays and the answer holds a copy of its context and the tasks taken.
    pub(crate) fn take_process(
        &mut self,
        cpu: &mut dyn Cpu,
        pid: Pid,
    ) -> Option<(ProcessContext, Vec<(TaskId, SuspendedTask)>)> {
        self.processes
            .get(&pid)
            .filter(|record| record.ended.is_none())?;
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
        self.update_watching();
        Some((ProcessContext { process, announced }, tasks))
    }

    /// A copy of process `pid`'s context, with no tasks.
    pub(crate) fn context_copy(&self, pid: Pid) -> ProcessContext {
        let mut process = self.processes[&pid].clone();
        process.threads.clear();
        let announced = self
            .shared_announced
            .iter()
            .filter(|((_, mapper), _)| *mapper == pid)
            .map(|(key, pieces)| (*key, pieces.clone()))
            .collect();
        ProcessContext { process, announced }
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

    /// Takes one task off the kernel. A call waiting in a futex's, a semaphore set's or a message
    /// queue's line leaves it and is made again when the task is put back, as Linux restarts a
    /// call a stop interrupted (`restart_syscall` for a futex wait with a timeout); any other
    /// waiting call keeps its place in the call and runs again when the task is put back.
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
        let in_line = wait == Some(Channel::Futex)
            || syscall::sem::queued(self, tid)
            || syscall::mqueue::queued(self, tid);
        let before_call = self.stepping == Some(tid);
        if before_call {
            self.stepping = None;
        }
        if in_line || before_call {
            syscall::futex::forget(self, tid);
            syscall::sem::forget(self, tid);
            syscall::mqueue::forget(self, tid);
            let thread = self.thread_mut(tid);
            let restart_block = in_line
                && wait == Some(Channel::Futex)
                && matches!(thread.restart, Some(Restart::Futex { .. }));
            registers.pc -= size_of::<u32>() as u64;
            if restart_block {
                registers.x[SYSTEM_CALL_NUMBER_REGISTER] =
                    u64::from(crate::linux::general::__NR_restart_syscall);
            }
            thread.call = Call::default();
            thread.resume = Resume::FromRegisters;
            place = Place::Ready;
        }
        let thread = self.threads.remove(&tid).expect("thread exists");
        cpu.thread_drop(thread.cpu);
        self.processes
            .get_mut(&thread.process)
            .expect("process exists")
            .threads
            .retain(|other| *other != tid);
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
        let Some(process) = self
            .processes
            .get(&task.thread.process)
            .filter(|process| process.ended.is_none())
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
        let threads = &mut self
            .processes
            .get_mut(&pid)
            .expect("process exists")
            .threads;
        if tid == pid {
            threads.insert(0, tid);
        } else {
            threads.push(tid);
        }
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
        if task.signalled && !self.signalled.contains(&tid) {
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
        let (context, tasks) = self
            .kernel
            .take_process(self.cpu, self.pid)
            .expect("a hook's process is on its kernel");
        (context, tasks.into_iter())
    }

    /// As [`Kernel::copy_process_context`] for this process.
    pub fn copy_process_context(&self) -> ProcessContext {
        self.kernel.context_copy(self.pid)
    }
}
