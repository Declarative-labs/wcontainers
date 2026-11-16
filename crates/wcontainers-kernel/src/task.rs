//! The kernel's main loop between the driver's calls: choosing who runs, the waiting rule, and
//! processes ending.
//!
//! A call that cannot finish parks its thread on a [`Channel`] and returns. When what it waits for
//! happens, its deadline comes or a signal arrives, the thread is woken and the same call runs again
//! from the thread's saved registers; nothing ever waits on the kernel's own stack.

use wcontainers_abi::{Next, RegisterBlock, Resume, Syscall};

use crate::counters::reason;
use crate::linux::general::*;
use crate::process::{Call, Channel, Pid, Report, Tid};
use crate::schedule;
use crate::{Context, Kernel, syscall};

impl Kernel {
    /// Chooses the thread to run, or says why none can run.
    pub(crate) fn pick(&mut self) -> Next {
        let tid = match self.current.filter(|_| !self.resched) {
            Some(tid) => tid,
            None => {
                if let Some(previous) = self.current.take() {
                    let thread = &self.threads[&previous];
                    self.run_queue.insert(
                        previous,
                        thread.vruntime,
                        schedule::weight(&thread.schedule),
                    );
                }
                self.resched = false;
                let threads = &self.threads;
                let Some(tid) = self
                    .run_queue
                    .pop(|tid| schedule::weight(&threads[&tid].schedule))
                else {
                    return Next::Idle {
                        until_nanoseconds: self.next_deadline(),
                    };
                };
                let thread = self.threads.get_mut(&tid).expect("queued thread exists");
                self.picked_runtime = thread.runtime;
                if let Some(woken) = thread.woken_at.take() {
                    self.counters.measure(
                        reason::WAKE_TO_RUN,
                        0,
                        self.last_now.saturating_sub(woken),
                    );
                }
                tid
            }
        };
        self.current = Some(tid);
        self.running = Some(tid);
        let thread = &self.threads[&tid];
        let slice_nanoseconds = (self.run_queue.len() > 0).then(|| {
            let weight = schedule::weight(&thread.schedule);
            let ideal = schedule::slice(
                weight,
                self.run_queue.load + weight,
                self.run_queue.len() + 1,
            );
            ideal
                .saturating_sub(thread.runtime - self.picked_runtime)
                .max(1)
        });
        Next::Run {
            thread: thread.cpu,
            resume: thread.resume,
            slice_nanoseconds,
        }
    }

    /// The earliest time a parked call ends anyway.
    fn next_deadline(&self) -> Option<u64> {
        self.threads
            .values()
            .filter(|thread| thread.call.wait.is_some())
            .filter_map(|thread| thread.call.deadline)
            .min()
    }

    /// Charges the thread that ran for the time since it was handed out.
    pub(crate) fn account(&mut self, cx: &mut Context<'_>, tid: Tid) {
        let now = cx.host.monotonic_nanoseconds();
        self.entry_now = now;
        let delta = now.saturating_sub(self.last_now);
        let thread = self.thread_mut(tid);
        thread.runtime += delta;
        thread.vruntime += schedule::scale(delta, schedule::weight(&thread.schedule));
        let vruntime = thread.vruntime;
        self.run_queue.update_min(Some(vruntime));
    }

    /// Deals with why a thread came back.
    pub(crate) fn came_back(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        exit: wcontainers_abi::Exit,
    ) {
        use wcontainers_abi::Exit;
        match exit {
            Exit::Syscall(call) => self.system_call(cx, tid, call, false),
            Exit::Stop => {
                // The slice timer, or a signal or event that needed the thread back.
                let thread = &self.threads[&tid];
                let weight = schedule::weight(&thread.schedule);
                let ideal = schedule::slice(
                    weight,
                    self.run_queue.load + weight,
                    self.run_queue.len() + 1,
                );
                if self.run_queue.len() > 0 && thread.runtime - self.picked_runtime >= ideal {
                    self.resched = true;
                }
                self.return_to_user(cx, tid, Resume::Continue);
            }
            Exit::MemoryFault { address, access } => self.memory_fault(cx, tid, address, access),
            Exit::MemoryFaultWithoutRegisters => self.fatal_fault(cx, tid, SIGSEGV),
            Exit::AlignmentFault { address } => {
                self.force_fault(cx, tid, SIGBUS, BUS_ADRALN as i32, address)
            }
            Exit::IllegalInstruction => {
                let pc = self.thread_pc(cx, tid);
                self.force_fault(cx, tid, SIGILL, ILL_ILLOPC as i32, pc)
            }
            Exit::Breakpoint { .. } => {
                let pc = self.thread_pc(cx, tid);
                self.force_fault(cx, tid, SIGTRAP, TRAP_BRKPT as i32, pc)
            }
        }
    }

    fn thread_pc(&mut self, cx: &mut Context<'_>, tid: Tid) -> u64 {
        let mut registers = RegisterBlock::ZERO;
        cx.cpu.registers(self.threads[&tid].cpu, &mut registers);
        registers.pc
    }

    /// Runs a system call, the first time or again after its thread was woken.
    fn system_call(&mut self, cx: &mut Context<'_>, tid: Tid, call: Syscall, again: bool) {
        let outcome = syscall::dispatch(self, cx, tid, call, again);
        if let Some(thread) = self.threads.get(&tid) {
            syscall::ipc::settle(self, cx, thread.process);
        }
        // A call may have ended its own thread's process (a signal it sent itself).
        if !self.threads.contains_key(&tid) {
            return;
        }
        match outcome {
            syscall::Outcome::Return(value) => {
                self.finish_call(tid);
                self.return_to_user(cx, tid, Resume::Return(value));
            }
            syscall::Outcome::Restart => {
                self.finish_call(tid);
                self.return_to_user(cx, tid, Resume::FromRegisters);
            }
            syscall::Outcome::Parked => {
                debug_assert!(self.threads[&tid].call.wait.is_some());
            }
            syscall::Outcome::Gone => {}
        }
    }

    /// Clears a finished call's slot; a named pipe's open still waiting gives up its end.
    fn finish_call(&mut self, tid: Tid) {
        let thread = self.thread_mut(tid);
        thread.call = Call::default();
        let opening = thread.opening.take();
        self.give_up_opening(opening);
    }

    /// Closes the end a named pipe's waiting open held, which the other end may wait on.
    fn give_up_opening(&mut self, opening: Option<crate::file::FileRef>) {
        if let Some(file) = opening {
            let node = file.borrow().node.clone();
            drop(file);
            self.wake(crate::syscall::pipe::channel(&node));
        }
    }

    /// On the thread's way back to the guest: acts on its signals, then it continues, or waits for
    /// its turn if it is not the current thread.
    pub(crate) fn return_to_user(&mut self, cx: &mut Context<'_>, tid: Tid, resume: Resume) {
        // A call may have ended its own thread's process (a signal it sent itself).
        if !self.threads.contains_key(&tid) {
            return;
        }
        let Some(resume) = self.deliver(cx, tid, resume) else {
            return;
        };
        if self.current == Some(tid) {
            self.thread_mut(tid).resume = resume;
        } else {
            self.make_runnable(tid, resume);
        }
    }

    /// Runs again the calls of woken threads and acts on signals sent to threads waiting for their
    /// turn, until nothing is left; then notes the time and reports what was counted.
    pub(crate) fn settle(&mut self, cx: &mut Context<'_>) {
        loop {
            if !self.files.terminals.is_empty() {
                syscall::tty::released(self, cx);
            }
            if let Some(tid) = self.woken.pop_front() {
                let Some(thread) = self.threads.get(&tid) else {
                    continue;
                };
                if thread.call.wait.is_some() {
                    continue;
                }
                let mut registers = RegisterBlock::ZERO;
                cx.cpu.registers(thread.cpu, &mut registers);
                let x = &registers.x;
                let call = Syscall {
                    number: x[8],
                    arguments: [x[0], x[1], x[2], x[3], x[4], x[5]],
                };
                self.system_call(cx, tid, call, true);
                continue;
            }
            if let Some(tid) = self.signalled.pop_front() {
                let Some(thread) = self.threads.get(&tid) else {
                    continue;
                };
                let stopped = self.processes[&thread.process].stopped;
                if thread.call.wait.is_some() || stopped || self.woken.contains(&tid) {
                    continue;
                }
                // A thread waiting its turn, the current one, or one of a process just continued
                // after a stop acts on its signals and goes back in line.
                let resume = thread.resume;
                self.take_out_of_line(tid);
                self.return_to_user(cx, tid, resume);
                continue;
            }
            break;
        }
        self.last_now = cx.host.monotonic_nanoseconds();
        self.counters.flush(cx.host);
    }

    /// Parks a thread in its call until `channel` changes, its deadline comes or a signal arrives.
    pub(crate) fn park(&mut self, tid: Tid, channel: Channel) {
        self.thread_mut(tid).call.wait = Some(channel);
        if self.current == Some(tid) {
            self.current = None;
        }
    }

    /// Wakes every thread parked on `channel`, and for a change to an object also those waiting on
    /// any file's readiness.
    pub(crate) fn wake(&mut self, channel: Channel) {
        let woken: alloc::vec::Vec<Tid> = self
            .threads
            .iter()
            .filter(|(_, thread)| match thread.call.wait {
                Some(Channel::Readiness) => matches!(channel, Channel::Object(_)),
                Some(wait) => wait == channel,
                None => false,
            })
            .map(|(tid, _)| *tid)
            .collect();
        for tid in woken {
            self.wake_thread(tid);
        }
    }

    /// Wakes the threads of `pid`'s process waiting on files' readiness, which signal descriptors
    /// depend on (Linux's `signalfd_notify`).
    pub(crate) fn wake_readiness_of(&mut self, pid: Pid) {
        let Some(process) = self.processes.get(&pid) else {
            return;
        };
        for tid in process.threads.clone() {
            if self.threads[&tid].call.wait == Some(Channel::Readiness) {
                self.wake_thread(tid);
            }
        }
    }

    /// Wakes the threads of `pid`'s process parked on its children.
    pub(crate) fn wake_waiters_of(&mut self, pid: Pid) {
        let Some(process) = self.processes.get(&pid) else {
            return;
        };
        for tid in process.threads.clone() {
            if self.threads[&tid].call.wait == Some(Channel::Children) {
                self.wake_thread(tid);
            }
        }
    }

    /// Ends a thread's wait; its call runs again before the kernel hands back control.
    pub(crate) fn wake_thread(&mut self, tid: Tid) {
        let entry_now = self.entry_now;
        let thread = self.thread_mut(tid);
        if thread.call.wait.take().is_some() {
            thread.woken_at = Some(entry_now);
            self.woken.push_back(tid);
        }
    }

    /// Wakes the threads whose deadline has come.
    pub(crate) fn wake_due(&mut self, cx: &mut Context<'_>) {
        let now = cx.host.monotonic_nanoseconds();
        self.entry_now = now;
        let due: alloc::vec::Vec<Tid> = self
            .threads
            .iter()
            .filter(|(_, thread)| {
                thread.call.wait.is_some() && thread.call.deadline.is_some_and(|at| at <= now)
            })
            .map(|(tid, _)| *tid)
            .collect();
        for tid in due {
            self.wake_thread(tid);
        }
    }

    /// Puts a thread that is not the current one in line to run, with how it continues. A thread
    /// of a stopped process waits aside until it is continued.
    pub(crate) fn make_runnable(&mut self, tid: Tid, resume: Resume) {
        let entry_now = self.entry_now;
        let thread = self.thread_mut(tid);
        thread.resume = resume;
        let pid = thread.process;
        if self.processes[&pid].stopped || self.run_queue.contains(tid) {
            return;
        }
        let thread = self.threads.get_mut(&tid).expect("thread exists");
        thread.vruntime = self.run_queue.place_woken(thread.vruntime);
        thread.woken_at.get_or_insert(entry_now);
        let (vruntime, schedule) = (thread.vruntime, thread.schedule);
        self.run_queue
            .insert(tid, vruntime, schedule::weight(&schedule));
        self.check_preempt(vruntime, &schedule);
    }

    /// Puts a new child in line to run (`wake_up_new_task`).
    pub(crate) fn start_new(&mut self, tid: Tid) {
        let entry_now = self.entry_now;
        let running = self.run_queue.len() + usize::from(self.current.is_some());
        let thread = self.threads.get_mut(&tid).expect("thread exists");
        let weight = schedule::weight(&thread.schedule);
        thread.vruntime = self.run_queue.place_new(weight, running);
        thread.woken_at = Some(entry_now);
        let (vruntime, schedule) = (thread.vruntime, thread.schedule);
        self.run_queue.insert(tid, vruntime, weight);
        self.check_preempt(vruntime, &schedule);
    }

    fn check_preempt(&mut self, vruntime: u64, schedule: &crate::process::Schedule) {
        if let Some(current) = self.current {
            let thread = &self.threads[&current];
            if schedule::preempts(thread.vruntime, &thread.schedule, vruntime, schedule) {
                self.resched = true;
            }
        }
    }

    /// Takes a thread out of line; returns whether it was waiting for its turn.
    pub(crate) fn take_out_of_line(&mut self, tid: Tid) -> bool {
        let weight = schedule::weight(&self.threads[&tid].schedule);
        self.run_queue.remove(tid, weight)
    }

    /// Takes a thread off the CPU and out of line, as when its process stops or it ends.
    pub(crate) fn unschedule(&mut self, tid: Tid) {
        self.take_out_of_line(tid);
        if self.current == Some(tid) {
            self.current = None;
        }
        self.woken.retain(|other| *other != tid);
        self.signalled.retain(|other| *other != tid);
    }

    /// Ends one thread of a process that has others.
    pub(crate) fn end_thread(&mut self, cx: &mut Context<'_>, tid: Tid) {
        self.unschedule(tid);
        syscall::sem::forget(self, tid);
        let thread = self.threads.remove(&tid).expect("thread exists");
        cx.cpu.thread_drop(thread.cpu);
        self.give_up_opening(thread.opening);
        let process = self
            .processes
            .get_mut(&thread.process)
            .expect("process exists");
        process.threads.retain(|other| *other != tid);
        process.ended_time += thread.runtime;
    }

    /// Ends a whole process with a wait status, as `exit_group` and fatal signals do: its threads
    /// and memory go, its children go to a reaper, and its parent is told. It stays as a zombie
    /// until its parent reaps it, unless the parent does not want it.
    pub(crate) fn end_process(&mut self, cx: &mut Context<'_>, pid: Pid, wait_status: i32) {
        if self.processes[&pid].ended.is_some() {
            return;
        }
        for tid in self.processes[&pid].threads.clone() {
            self.unschedule(tid);
            syscall::sem::forget(self, tid);
            let thread = self.threads.remove(&tid).expect("thread exists");
            cx.cpu.thread_drop(thread.cpu);
            let process = self.processes.get_mut(&pid).expect("process exists");
            process.ended_time += thread.runtime;
            if tid == pid {
                process.ended_schedule = crate::process::Schedule {
                    io_priority: 0,
                    ..thread.schedule
                };
            }
        }
        self.release_vfork(pid);
        syscall::sem::exit_sem(self, cx, pid);
        let process = self.processes.get_mut(&pid).expect("process exists");
        process.threads.clear();
        process.ended = Some(wait_status);
        let pidfd = process.pidfd.clone();
        process.stopped = false;
        process.report = None;
        process.pending = Default::default();
        process.files.close_all();
        let memory = process.memory;
        let empty = crate::memory::AddressMap::new(cx.cpu.address_space().end);
        let map = core::mem::replace(&mut process.map, empty);
        process.peak_mapped = map.iter().map(|mapping| mapping.end - mapping.start).sum();
        if !self.memory_in_use_elsewhere(pid, memory) {
            cx.cpu.memory_drop(memory);
        }
        if let Some(node) = pidfd {
            self.wake(syscall::pidfd::channel(&node));
        }
        syscall::tty::disassociate(self, cx, pid, true);
        syscall::ipc::settle(self, cx, pid);
        crate::shared::release(self, cx);
        self.files_released(cx);
        if self.init == Some(pid) {
            // The container ends with its first process; Docker kills the rest.
            self.finished = Some(wait_status);
            return;
        }
        self.orphan_children(cx, pid);
        self.notify_parent(cx, pid);
    }

    /// Whether another live process shows the same memory (a `vfork` parent or child).
    fn memory_in_use_elsewhere(&self, pid: Pid, memory: wcontainers_abi::MemoryId) -> bool {
        self.processes
            .values()
            .any(|other| other.pid != pid && other.ended.is_none() && other.memory == memory)
    }

    /// A `vfork` child lets go of its parent's memory: the parent sees the mappings as the child
    /// left them, and its wait ends.
    pub(crate) fn release_vfork(&mut self, child: Pid) {
        let Some(parent_thread) = self
            .processes
            .get_mut(&child)
            .and_then(|process| process.vfork_parent.take())
        else {
            return;
        };
        let map = self.processes[&child].map.clone();
        let Some(parent) = self
            .threads
            .get(&parent_thread)
            .map(|thread| thread.process)
        else {
            return;
        };
        let memory = self.processes[&child].memory;
        if let Some(parent) = self.processes.get_mut(&parent)
            && parent.memory == memory
        {
            parent.map = map;
        }
        self.wake(Channel::Vfork(child));
    }

    /// Hands a dying process's children to the nearest subreaper above it or the first process
    /// (`forget_original_parent`), sends each its parent death signal, and treats process groups
    /// this leaves orphaned with stopped members as Linux does.
    fn orphan_children(&mut self, cx: &mut Context<'_>, pid: Pid) {
        let children: alloc::vec::Vec<Pid> = self
            .processes
            .values()
            .filter(|process| process.parent == pid)
            .map(|process| process.pid)
            .collect();
        if children.is_empty() {
            return;
        }
        let mut reaper = self.processes[&pid].parent;
        while let Some(process) = self.processes.get(&reaper) {
            if process.child_subreaper && process.ended.is_none() {
                break;
            }
            reaper = process.parent;
        }
        let reaper = if self.processes.contains_key(&reaper) {
            reaper
        } else {
            self.init.expect("the container has started")
        };
        for child in children {
            self.processes.get_mut(&child).expect("child").parent = reaper;
            let threads = self.processes[&child].threads.clone();
            for tid in threads {
                let signal = self.threads[&tid].parent_death_signal;
                if signal > 0 {
                    let info = crate::signal::kernel_info(signal as u32);
                    self.send_to_thread(cx, tid, signal as u32, info);
                }
            }
            if self.processes[&child].ended.is_some() {
                // A zombie the new parent does not want goes at once; otherwise it is told.
                self.notify_parent_of_zombie(cx, child);
            }
            self.kill_orphaned_group(cx, pid, child);
        }
        self.wake_waiters_of(reaper);
    }

    /// When a process's going leaves its child's group orphaned with stopped members, the group is
    /// sent SIGHUP then SIGCONT (`kill_orphaned_pgrp`).
    fn kill_orphaned_group(&mut self, cx: &mut Context<'_>, dying: Pid, child: Pid) {
        let (group, session) = {
            let process = &self.processes[&child];
            (process.group, process.session)
        };
        let dying_process = &self.processes[&dying];
        if dying_process.group == group || dying_process.session != session {
            return;
        }
        if self.group_orphaned(group) && self.group_has_stopped(group) {
            self.send_to_group(cx, group, SIGHUP, crate::signal::kernel_info(SIGHUP));
            self.send_to_group(cx, group, SIGCONT, crate::signal::kernel_info(SIGCONT));
        }
    }

    /// Whether no member of a process group has a parent in another group of the same session
    /// (`will_become_orphaned_pgrp` with no one leaving).
    pub(crate) fn group_orphaned(&self, group: Pid) -> bool {
        !self.processes.values().any(|process| {
            process.group == group
                && process.ended.is_none()
                && self.init != Some(process.parent)
                && self.processes.get(&process.parent).is_some_and(|parent| {
                    parent.group != group && parent.session == process.session
                })
        })
    }

    fn group_has_stopped(&self, group: Pid) -> bool {
        self.processes
            .values()
            .any(|process| process.group == group && process.stopped)
    }

    /// Tells a dead process's parent (`do_notify_parent`): its exit signal, and wakes its waits. A
    /// parent that ignores SIGCHLD or asked not to wait gets no zombie.
    fn notify_parent(&mut self, cx: &mut Context<'_>, pid: Pid) {
        let process = &self.processes[&pid];
        let parent = process.parent;
        let exit_signal = process.exit_signal;
        let status = process.ended.expect("the process has ended");
        let Some(parent_process) = self.processes.get(&parent) else {
            self.processes.remove(&pid);
            return;
        };
        let action = parent_process.actions[SIGCHLD as usize - 1];
        let unwanted = exit_signal == SIGCHLD
            && (action.sa_handler_kernel == crate::linux_extra::SIG_IGN
                || action.sa_flags & u64::from(SA_NOCLDWAIT) != 0);
        if exit_signal != 0 {
            let info = self.child_info(pid, status);
            self.send_to_process(cx, parent, exit_signal, info);
        }
        self.wake_waiters_of(parent);
        if unwanted {
            self.reap(pid);
        }
    }

    /// A zombie handed to a new parent: reaped at once if that parent does not want it, else the
    /// parent is told as `reparent_leader` does.
    fn notify_parent_of_zombie(&mut self, cx: &mut Context<'_>, pid: Pid) {
        self.notify_parent(cx, pid);
    }

    /// The `siginfo` of SIGCHLD for a child that ended or changed state.
    pub(crate) fn child_info(&self, pid: Pid, status: i32) -> [u8; 128] {
        let process = &self.processes[&pid];
        let (code, value) = if status & 0x7f == 0 {
            (CLD_EXITED, (status >> 8) & 0xff)
        } else if status & 0x80 != 0 {
            (CLD_DUMPED, status & 0x7f)
        } else {
            (CLD_KILLED, status & 0x7f)
        };
        let time = process.ended_time / 10_000_000;
        crate::signal::child_info(pid, process.credentials.uid, code as i32, value, time)
    }

    /// Takes a zombie away for good, adding its times to its parent's children's.
    pub(crate) fn reap(&mut self, pid: Pid) {
        let Some(process) = self.processes.remove(&pid) else {
            return;
        };
        if let Some(node) = &process.pidfd {
            self.wake(syscall::pidfd::channel(node));
        }
        if let Some(parent) = self.processes.get_mut(&process.parent) {
            parent.children_time += process.ended_time + process.children_time;
            parent.children_maxrss = parent.children_maxrss.max(process.children_maxrss);
        }
    }

    /// The report a stopped or continued child leaves for its parent, and the parent told.
    pub(crate) fn report_to_parent(&mut self, cx: &mut Context<'_>, pid: Pid, report: Report) {
        let process = self.processes.get_mut(&pid).expect("process exists");
        process.report = Some(report);
        let parent = process.parent;
        let Some(parent_process) = self.processes.get(&parent) else {
            return;
        };
        let action = parent_process.actions[SIGCHLD as usize - 1];
        if action.sa_flags & u64::from(SA_NOCLDSTOP) == 0 {
            let (code, value) = match report {
                Report::Stopped(signal) => (CLD_STOPPED, signal as i32),
                Report::Continued => (CLD_CONTINUED, SIGCONT as i32),
            };
            let process = &self.processes[&pid];
            let info =
                crate::signal::child_info(pid, process.credentials.uid, code as i32, value, 0);
            self.send_to_process(cx, parent, SIGCHLD, info);
        }
        self.wake_waiters_of(parent);
    }

    /// Descriptors were closed: locks may have been let go and pipes and terminals may have lost an
    /// end, so every wait on a file looks again.
    pub(crate) fn files_released(&mut self, cx: &mut Context<'_>) {
        crate::syscall::pipe::released(self, cx);
        let woken: alloc::vec::Vec<Tid> = self
            .threads
            .iter()
            .filter(|(_, thread)| {
                matches!(
                    thread.call.wait,
                    Some(Channel::Object(_) | Channel::Readiness | Channel::Locks)
                )
            })
            .map(|(tid, _)| *tid)
            .collect();
        for tid in woken {
            self.wake_thread(tid);
        }
    }
}
