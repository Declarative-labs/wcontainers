//! The kernel's main loop between the driver's calls: choosing who runs, the waiting rule, and
//! processes ending.
//!
//! A call that cannot finish parks its thread on a [`Channel`] and returns. When what it waits for
//! happens, its deadline comes or a signal arrives, the thread is woken and the same call runs again
//! from the thread's saved registers; nothing ever waits on the kernel's own stack.

use wcontainers_abi::{Next, RegisterBlock, Resume, Syscall};

use crate::counters::reason;
use crate::linux::general::*;
use crate::process::{Call, Channel, Leader, Pid, Report, Tid};
use crate::schedule;
use crate::{Context, Kernel, syscall};

impl Kernel {
    /// Chooses the thread to run, or says why none can run.
    pub(crate) fn pick(&mut self) -> Next {
        let tid = match self.current.filter(|_| !self.resched) {
            Some(tid) => tid,
            None => {
                self.give_way();
                let threads = &self.threads;
                let Some(tid) = self
                    .run_queue
                    .pop(|tid| schedule::weight(&threads[&tid].schedule))
                else {
                    // The host's clock, which the kernel's runs ahead of by the offset.
                    return Next::Idle {
                        until_nanoseconds: self
                            .next_deadline()
                            .map(|at| at.wrapping_sub(self.monotonic_offset)),
                    };
                };
                debug_assert!(!self.woken.contains(&tid), "a woken call runs first");
                self.take_turn(tid);
                tid
            }
        };
        self.current = Some(tid);
        self.running = Some(tid);
        let thread = &self.threads[&tid];
        let fair = (self.run_queue.len() > 0).then(|| {
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
        // A parked call's deadline ends the turn as Linux's timer interrupt would.
        let timer = self
            .next_deadline()
            .map(|at| at.saturating_sub(self.last_now).max(1));
        // So does a CPU timer of the thread's or its process's.
        let cpu = syscall::timer::cpu_slice(self, tid);
        let slice_nanoseconds = [fair, timer, cpu].into_iter().flatten().min();
        Next::Run {
            thread: thread.cpu,
            resume: thread.resume,
            slice_nanoseconds,
        }
    }

    /// The earliest time a parked call ends anyway.
    fn next_deadline(&self) -> Option<u64> {
        match (self.parked.earliest(), self.timers.earliest()) {
            (Some(parked), Some(timer)) => Some(parked.min(timer)),
            (parked, timer) => parked.or(timer),
        }
    }

    /// Charges the thread that ran for the time since it was handed out.
    pub(crate) fn account(&mut self, cx: &mut Context<'_>, tid: Tid) {
        let now = cx.monotonic();
        self.entry_now = now;
        let delta = now.saturating_sub(self.last_now);
        let thread = self.thread_mut(tid);
        thread.runtime += delta;
        thread.vruntime += schedule::scale(delta, schedule::weight(&thread.schedule));
        let vruntime = thread.vruntime;
        self.run_queue.update_min(Some(vruntime));
    }

    /// Linux's scheduler tick (`check_preempt_tick`), at every return from the CPU side whatever
    /// brought the thread back: the current thread gives way once it has had its share while others
    /// wait.
    pub(crate) fn tick(&mut self, tid: Tid) {
        if self.current != Some(tid) || self.run_queue.len() == 0 {
            return;
        }
        let thread = &self.threads[&tid];
        let weight = schedule::weight(&thread.schedule);
        let ideal = schedule::slice(
            weight,
            self.run_queue.load + weight,
            self.run_queue.len() + 1,
        );
        if thread.runtime - self.picked_runtime >= ideal {
            self.resched = true;
        }
    }

    /// `sched_yield`: the current thread goes behind every thread waiting for its turn, as the
    /// reference's scheduler (EEVDF, which forfeits the caller's deadline) lets them run first.
    pub(crate) fn yield_turn(&mut self, tid: Tid) {
        if self.current != Some(tid) {
            return;
        }
        let Some(last) = self.run_queue.rightmost() else {
            return;
        };
        let thread = self.thread_mut(tid);
        thread.vruntime = thread.vruntime.max(last);
        self.resched = true;
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
            // The slice timer, or a signal or event that needed the thread back; one that gives
            // way leaves its restartable sequence.
            Exit::Stop => {
                let resume = if self.resched {
                    self.rseq_preempted(cx, tid)
                } else {
                    Some(Resume::Continue)
                };
                if let Some(resume) = resume {
                    self.return_to_user(cx, tid, resume);
                }
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
        let holds = core::mem::take(&mut thread.call).holds;
        if let Some(address) = holds {
            self.wake(Channel::Object(address));
        }
        let thread = self.thread_mut(tid);
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
            // A woken thread's call runs again when its turn comes, as Linux's waiter goes on in its
            // call once it is scheduled.
            if let Some(tid) = self.next_to_run()
                && let Some(index) = self.woken.iter().position(|woken| *woken == tid)
            {
                self.woken.remove(index);
                self.take_out_of_line(tid);
                let thread = &self.threads[&tid];
                let mut registers = RegisterBlock::ZERO;
                cx.cpu.registers(thread.cpu, &mut registers);
                let x = &registers.x;
                let call = Syscall {
                    number: x[8],
                    arguments: [x[0], x[1], x[2], x[3], x[4], x[5]],
                };
                self.system_call(cx, tid, call, true);
                // A call that went back to its program has its thread's turn.
                if self.run_queue.contains(tid) {
                    self.take_out_of_line(tid);
                    self.give_way();
                    self.take_turn(tid);
                }
                continue;
            }
            if let Some(tid) = self.signalled.pop_front() {
                let Some(thread) = self.threads.get(&tid) else {
                    continue;
                };
                let stopped = self.processes[&thread.process].stopped;
                if thread.call.wait.is_some() || stopped {
                    continue;
                }
                // A woken thread of a process just continued goes back in line to run its call.
                if self.woken.contains(&tid) {
                    self.make_runnable(tid, Resume::FromRegisters);
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
        self.last_now = cx.monotonic();
        for _ in 0..core::mem::take(&mut cx.refused) {
            self.counters.count(cx.host, reason::HOST_CANNOT, 0);
        }
        self.counters.flush(cx.host);
    }

    /// The current thread goes back in line.
    fn give_way(&mut self) {
        if let Some(previous) = self.current.take() {
            let thread = &self.threads[&previous];
            self.run_queue.insert(
                previous,
                thread.vruntime,
                schedule::weight(&thread.schedule),
            );
        }
        self.resched = false;
    }

    /// Gives `tid`, out of line, the CPU, measuring how long it waited since it was woken.
    fn take_turn(&mut self, tid: Tid) {
        let thread = self.threads.get_mut(&tid).expect("thread exists");
        self.picked_runtime = thread.runtime;
        if let Some(woken) = thread.woken_at.take() {
            self.counters
                .measure(reason::WAKE_TO_RUN, 0, self.last_now.saturating_sub(woken));
        }
        self.current = Some(tid);
    }

    /// The thread `pick` would run now: the current one unless it is to give way, else the one in
    /// line with the least time, the current one going back in line behind those with as little.
    fn next_to_run(&self) -> Option<Tid> {
        if let Some(current) = self.current.filter(|_| !self.resched) {
            return Some(current);
        }
        match (self.run_queue.leftmost(), self.current) {
            (Some((vruntime, _)), Some(current)) if vruntime > self.threads[&current].vruntime => {
                Some(current)
            }
            (Some((_, tid)), _) => Some(tid),
            (None, current) => current,
        }
    }

    /// Parks a thread in its call until `channel` changes, its deadline comes or a signal arrives.
    pub(crate) fn park(&mut self, tid: Tid, channel: Channel) {
        let call = &mut self.thread_mut(tid).call;
        let (before, deadline) = (call.wait.replace(channel), call.deadline);
        if let Some(before) = before {
            self.parked.remove(tid, before, deadline);
        }
        self.parked.insert(tid, channel, deadline);
        if self.current == Some(tid) {
            self.current = None;
        }
    }

    /// Wakes every thread parked on `channel`, and for a change to an object also those waiting on
    /// any file's readiness.
    pub(crate) fn wake(&mut self, channel: Channel) {
        let readiness = matches!(channel, Channel::Object(_)).then_some(Channel::Readiness);
        for tid in self.parked.on_either(channel, readiness) {
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

    /// Ends a thread's wait; it goes in line and its call runs again when its turn comes.
    pub(crate) fn wake_thread(&mut self, tid: Tid) {
        let entry_now = self.entry_now;
        let thread = self.thread_mut(tid);
        if let Some(channel) = thread.call.wait.take() {
            thread.woken_at = Some(entry_now);
            let deadline = thread.call.deadline;
            self.parked.remove(tid, channel, deadline);
            self.woken.push_back(tid);
            // Its registers still hold the call, which runs again before it goes on.
            self.make_runnable(tid, Resume::FromRegisters);
        }
    }

    /// Wakes the threads whose deadline has come by the time the kernel was entered.
    pub(crate) fn wake_due(&mut self) {
        for tid in self.parked.due(self.entry_now) {
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
        let call = self.threads[&tid].call;
        if let Some(channel) = call.wait {
            self.parked.remove(tid, channel, call.deadline);
        }
        if let Some(address) = call.holds {
            self.thread_mut(tid).call.holds = None;
            self.wake(Channel::Object(address));
        }
        if self.current == Some(tid) {
            self.current = None;
        }
        self.woken.retain(|other| *other != tid);
        self.signalled.retain(|other| *other != tid);
    }

    /// Ends one thread of a process that has others.
    pub(crate) fn end_thread(&mut self, cx: &mut Context<'_>, tid: Tid) {
        syscall::futex::thread_end(self, cx, tid, true);
        self.unschedule(tid);
        self.release_vfork(tid);
        syscall::sem::forget(self, tid);
        syscall::sem::exit_sem(self, cx, tid);
        syscall::mqueue::forget(self, tid);
        let thread = self.threads.remove(&tid).expect("thread exists");
        cx.cpu.thread_drop(thread.cpu);
        if let Some(node) = &thread.pidfd {
            self.wake(syscall::pidfd::channel(node));
        }
        let leader = (tid == thread.process).then(|| Leader::of(&thread));
        self.put_files(thread.files, thread.process);
        self.give_up_opening(thread.opening);
        let process = self
            .processes
            .get_mut(&thread.process)
            .expect("process exists");
        process.threads.retain(|other| *other != tid);
        process.ended_time += thread.runtime;
        if let Some(leader) = leader {
            process.ended_leader = leader;
        }
        if !self.released.is_empty() {
            self.files_released(cx);
        }
    }

    /// `de_thread`: ends every other thread of the caller's process, and the caller takes the
    /// process's number if it did not have it. Returns the caller's number now.
    pub(crate) fn only_thread(&mut self, cx: &mut Context<'_>, pid: Pid, tid: Tid) -> Tid {
        for other in self.processes[&pid].threads.clone() {
            if other != tid {
                self.end_thread(cx, other);
            }
        }
        if tid == pid {
            return tid;
        }
        let mut thread = self.threads.remove(&tid).expect("thread exists");
        thread.call = Call::default();
        // Its own number goes with the first thread, and the descriptors naming it with it.
        if let Some(node) = thread.pidfd.take() {
            self.wake(syscall::pidfd::channel(&node));
        }
        self.threads.insert(pid, thread);
        syscall::sem::renumber(self, tid, pid);
        self.processes
            .get_mut(&pid)
            .expect("process exists")
            .threads = alloc::vec![pid];
        if self.current == Some(tid) {
            self.current = Some(pid);
        }
        pid
    }

    /// Ends a whole process with a wait status, as `exit_group` and fatal signals do: its threads
    /// and memory go, its children go to a reaper, and its parent is told. It stays as a zombie
    /// until its parent reaps it, unless the parent does not want it.
    pub(crate) fn end_process(&mut self, cx: &mut Context<'_>, pid: Pid, wait_status: i32) {
        if self.processes[&pid].ended.is_some() {
            return;
        }
        let threads = self.processes[&pid].threads.clone();
        // The threads' descriptor tables, closed where the process's files close.
        let mut tables = alloc::vec::Vec::new();
        let memory = self.processes[&pid].memory;
        let shared = self.memory_in_use_elsewhere(pid, memory);
        for (index, &tid) in threads.iter().enumerate() {
            // Each thread's end sees the memory still used by those yet to end.
            syscall::futex::thread_end(self, cx, tid, shared || index + 1 < threads.len());
        }
        for tid in threads {
            self.unschedule(tid);
            self.release_vfork(tid);
            syscall::sem::forget(self, tid);
            syscall::sem::exit_sem(self, cx, tid);
            syscall::mqueue::forget(self, tid);
            let thread = self.threads.remove(&tid).expect("thread exists");
            cx.cpu.thread_drop(thread.cpu);
            if let Some(node) = &thread.pidfd {
                self.wake(syscall::pidfd::channel(node));
            }
            let process = self.processes.get_mut(&pid).expect("process exists");
            process.ended_time += thread.runtime;
            if tid == pid {
                process.ended_leader = Leader::of(&thread);
            }
            tables.push(thread.files);
        }
        syscall::timer::process_end(self, pid, false);
        let process = self.processes.get_mut(&pid).expect("process exists");
        process.threads.clear();
        process.ended = Some(wait_status);
        let pidfd = process.pidfd.clone();
        process.stopped = false;
        process.report = None;
        process.pending = Default::default();
        let memory = process.memory;
        let empty = crate::memory::AddressMap::new(cx.cpu.address_space().end);
        let map = core::mem::replace(
            &mut process.map,
            alloc::rc::Rc::new(core::cell::RefCell::new(empty)),
        );
        process.peak_mapped = map
            .borrow()
            .iter()
            .map(|mapping| mapping.end - mapping.start)
            .sum();
        if !self.memory_in_use_elsewhere(pid, memory) {
            cx.cpu.memory_drop(memory);
        }
        for files in tables {
            self.put_files(files, pid);
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
        // Its own group may be left orphaned by its going (`kill_orphaned_pgrp` at exit).
        let parent = self.processes[&pid].parent;
        self.kill_orphaned_group(cx, parent, pid);
        self.notify_parent(cx, pid);
    }

    /// Whether a user's tasks, with `more` added (or taken away), are over a process's RLIMIT_NPROC
    /// (`is_rlimit_overlimit` on `UCOUNT_RLIMIT_NPROC`): every live thread of the user's processes
    /// and every one ended and not yet reaped counts. Root's are never over it.
    pub(crate) fn over_process_limit(&self, pid: Pid, uid: u32, more: i64) -> bool {
        let limit = self.processes[&pid].limits[RLIMIT_NPROC as usize].current;
        if uid == 0 || limit == RLIM_INFINITY as u64 {
            return false;
        }
        let live = self
            .threads
            .values()
            .filter(|thread| thread.credentials.uid == uid)
            .count();
        // A process's first thread counts until it is reaped.
        let ended = self
            .processes
            .values()
            .filter(|process| {
                !self.threads.contains_key(&process.pid)
                    && process.ended_leader.credentials.uid == uid
            })
            .count();
        let tasks = (live + ended) as i64;
        i128::from(tasks + more) > i128::from(limit.min(i64::MAX as u64))
    }

    /// Whether another live process shows the same memory (made with CLONE_VM).
    pub(crate) fn memory_in_use_elsewhere(
        &self,
        pid: Pid,
        memory: wcontainers_abi::MemoryId,
    ) -> bool {
        self.processes
            .values()
            .any(|other| other.pid != pid && other.ended.is_none() && other.memory == memory)
    }

    /// A `vfork` child thread starts a program or ends (`mm_release`): the parent thread's wait
    /// ends.
    pub(crate) fn release_vfork(&mut self, child: Tid) {
        let Some(thread) = self.threads.get_mut(&child) else {
            return;
        };
        if thread.vfork_parent.take().is_some() {
            self.wake(Channel::Vfork(child));
        }
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
            let process = self.processes.get_mut(&child).expect("child");
            process.parent = reaper;
            // A new parent always hears of the end by SIGCHLD (`reparent_leader`).
            process.exit_signal = SIGCHLD;
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

    /// When a process's going leaves `member`'s group orphaned with stopped members, the group is
    /// sent SIGHUP then SIGCONT (`kill_orphaned_pgrp`): `outside` is the member's link outside the
    /// group, the dying process for its child or the parent of the dying process itself.
    fn kill_orphaned_group(&mut self, cx: &mut Context<'_>, outside: Pid, member: Pid) {
        let (group, session) = {
            let process = &self.processes[&member];
            (process.group, process.session)
        };
        let Some(outside) = self.processes.get(&outside) else {
            return;
        };
        if outside.group == group || outside.session != session {
            return;
        }
        if self.group_orphaned(group) && self.group_has_stopped(group) {
            self.send_to_group(cx, group, SIGHUP, crate::signal::kernel_info(SIGHUP));
            self.send_to_group(cx, group, SIGCONT, crate::signal::kernel_info(SIGCONT));
        }
    }

    /// Whether no live member of a process group has a parent in another group of the same session
    /// (`will_become_orphaned_pgrp`; the container's first process is not the machine's init, so
    /// it counts as a parent like any other).
    pub(crate) fn group_orphaned(&self, group: Pid) -> bool {
        !self.processes.values().any(|process| {
            process.group == group
                && process.ended.is_none()
                && self.processes.get(&process.parent).is_some_and(|parent| {
                    parent.group != group && parent.session == process.session
                })
        })
    }

    /// Linux's `ptrace_may_access` with PTRACE_MODE_READ_FSCREDS: the caller's own process, or one
    /// whose ids all match the caller's file system ids, that may be dumped and whose permitted
    /// powers the caller's effective ones cover (`cap_ptrace_access_check`), unless the caller has
    /// CAP_SYS_PTRACE.
    pub(crate) fn ptrace_may_read(&self, caller: Tid, target: Pid) -> bool {
        if self.threads[&caller].process == target {
            return true;
        }
        let Some(target_process) = self.processes.get(&target) else {
            return false;
        };
        let (c, t) = (self.credentials(caller), self.task_credentials(target));
        let traces = c.has(CAP_SYS_PTRACE);
        let same = [t.uid, t.euid, t.suid].iter().all(|id| *id == c.fsuid)
            && [t.gid, t.egid, t.sgid].iter().all(|id| *id == c.fsgid);
        if !same && !traces {
            return false;
        }
        if target_process.ended.is_none() && !target_process.map.borrow().dumpable && !traces {
            return false;
        }
        t.permitted & !c.effective == 0 || traces
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
        let action = parent_process.actions.borrow()[SIGCHLD as usize - 1];
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
        let uid = self.task_credentials(pid).uid;
        crate::signal::child_info(pid, uid, code as i32, value, time)
    }

    /// Takes a zombie away for good, adding its times to its parent's children's.
    pub(crate) fn reap(&mut self, pid: Pid) {
        let Some(process) = self.processes.remove(&pid) else {
            return;
        };
        if let Some(node) = &process.pidfd {
            if let crate::fs::Content::Pidfd(_, exit) = &mut node.borrow_mut().content {
                *exit = process.ended;
            }
            self.wake(syscall::pidfd::channel(node));
        }
        if let Some(parent) = self.processes.get_mut(&process.parent) {
            parent.children_time += process.ended_time + process.children_time;
            parent.children_maxrss = parent.children_maxrss.max(process.children_maxrss);
        }
    }

    /// The report a stopped child leaves for its parent, and the parent told.
    pub(crate) fn report_to_parent(&mut self, cx: &mut Context<'_>, pid: Pid, report: Report) {
        self.processes.get_mut(&pid).expect("process exists").report = Some(report);
        self.tell_parent(cx, pid, report);
    }

    /// Sends a stopped or continued child's parent SIGCHLD, unless it asked not to be told, and
    /// wakes its waits (`do_notify_parent_cldstop`).
    pub(crate) fn tell_parent(&mut self, cx: &mut Context<'_>, pid: Pid, report: Report) {
        let parent = self.processes[&pid].parent;
        let Some(parent_process) = self.processes.get(&parent) else {
            return;
        };
        let action = parent_process.actions.borrow()[SIGCHLD as usize - 1];
        if action.sa_flags & u64::from(SA_NOCLDSTOP) == 0 {
            let (code, value) = match report {
                Report::Stopped(signal) => (CLD_STOPPED, signal as i32),
                Report::Continued => (CLD_CONTINUED, SIGCONT as i32),
            };
            let uid = self.task_credentials(pid).uid;
            let info = crate::signal::child_info(pid, uid, code as i32, value, 0);
            self.send_to_process(cx, parent, SIGCHLD, info);
        }
        self.wake_waiters_of(parent);
    }

    /// Descriptors were closed: pipes and terminals may have lost an end, so every wait on a file
    /// looks again, and waits for locks do if locks were let go.
    pub(crate) fn files_released(&mut self, cx: &mut Context<'_>) {
        let mut files = core::mem::take(&mut self.released);
        let mut let_go = false;
        for thread in self.threads.values() {
            let mut table = thread.files.borrow_mut();
            files.extend(table.take_released());
            let_go |= table.take_let_go_locks();
        }
        let nodes: alloc::vec::Vec<_> = files
            .iter()
            .map(|file| file.borrow().node.clone())
            .collect();
        crate::syscall::pipe::released(self, cx, files);
        // An open file's last close lets go of its record and flock locks.
        for node in nodes {
            let mut node = node.borrow_mut();
            let_go |= crate::locks::release_dead(&mut node.record_locks);
            let flocks = node.flocks.len();
            node.flocks.retain(|(holder, _)| holder.strong_count() > 0);
            let_go |= node.flocks.len() != flocks;
        }
        if let_go {
            self.wake(Channel::Locks);
        }
        let woken = self
            .parked
            .on(|channel| matches!(channel, Channel::Object(_) | Channel::Readiness));
        for tid in woken {
            self.wake_thread(tid);
        }
    }
}
