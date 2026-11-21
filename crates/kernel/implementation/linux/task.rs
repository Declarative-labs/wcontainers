//! The kernel's main loop between the driver's calls: choosing who runs, the waiting rule, and
//! processes ending.
//!
//! A call that cannot finish parks its thread on a [`Channel`] and returns. When what it waits for
//! happens, its deadline comes or a signal arrives, the thread is woken and the same call runs again
//! from the thread's saved registers; nothing ever waits on the kernel's own stack.

use wcontainers_abi::{
    Next, RegisterBlock, Resume, SYSTEM_CALL_ARGUMENT_COUNT, SYSTEM_CALL_NUMBER_REGISTER,
    SystemCallNumber, SystemCallRequest,
};

use crate::counters::reason;
use crate::linux::general::*;
use crate::process::{Call, Channel, Leader, Pid, Report, Tid};
use crate::schedule;
use crate::{Context, Errno, Kernel, syscall};

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
        self.charge(tid, now.saturating_sub(self.last_now));
    }

    /// Charges a thread for CPU time, as its scheduler time too.
    fn charge(&mut self, tid: Tid, delta: u64) {
        let thread = self.thread_mut(tid);
        thread.runtime += delta;
        thread.vruntime += schedule::scale(delta, schedule::weight(&thread.schedule));
        let vruntime = thread.vruntime;
        self.run_queue.update_min(Some(vruntime));
    }

    /// Charges a thread that came back for the time the kernel then worked until it settled, as
    /// time in the kernel (`stime`).
    pub(crate) fn account_system(&mut self, tid: Tid, entered: u64) {
        if !self.threads.contains_key(&tid) {
            return;
        }
        let delta = self.last_now.saturating_sub(entered);
        self.charge(tid, delta);
        self.thread_mut(tid).system += delta;
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
                if self.resched && self.threads[&tid].rseq.is_some() {
                    self.thread_mut(tid).rseq_event = true;
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
    fn system_call(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        mut call: SystemCallRequest,
        again: bool,
    ) {
        // A thread of a process a fatal signal is ending does not run its call again.
        if !self.dying.is_empty() && self.dying.contains_key(&self.threads[&tid].process) {
            self.return_to_user(cx, tid, Resume::FromRegisters);
            return;
        }
        // The embedder sees each call it made once, before anything of it happens: it may answer
        // it, or change it, which changes the thread's registers as a tracer's change does.
        if !again {
            let pid = self.threads[&tid].process;
            let thread = crate::system::task_id(tid);
            let mut decision = core::ops::ControlFlow::Continue(call);
            self.call_not_begun = true;
            self.announce(cx, pid, |system, process| {
                decision = system.intercept_system_call_request(process, thread, call);
            });
            self.call_not_begun = false;
            // The embedder took the thread, which makes the call again where it is put back.
            if self.stepping.is_none() {
                return;
            }
            match decision {
                core::ops::ControlFlow::Break(response) => {
                    let value = crate::system::return_of(response);
                    self.return_to_user(cx, tid, Resume::Return(value));
                    return;
                }
                core::ops::ControlFlow::Continue(request) => {
                    if request != call {
                        let mut registers = RegisterBlock::ZERO;
                        cx.cpu.registers(self.threads[&tid].cpu, &mut registers);
                        registers.x[SYSTEM_CALL_NUMBER_REGISTER] = request.number.0;
                        registers.x[..request.arguments.len()].copy_from_slice(&request.arguments);
                        cx.cpu.set_registers(self.threads[&tid].cpu, &registers);
                        call = request;
                    }
                }
            }
        }
        // The watched memory the call will touch is met before it acts, and its copies are
        // checked against what its description named; so is another process's memory it reads
        // through a file.
        let other = if self.watching {
            self.reads_watched_process(tid, call)
        } else {
            None
        };
        let watched = self.watched(tid) || other.is_some();
        let mut unmet = None;
        if watched
            && !self.walk_watched(cx, |kernel, cx| {
                // A copy into this memory by another process's call left nothing for this one.
                let own = kernel.threads[&tid].process;
                for pid in core::iter::once(own).chain(other) {
                    let watches = &mut kernel.processes[&pid].map.borrow_mut().watches;
                    watches.touched = None;
                    watches.described = Some(alloc::vec::Vec::new());
                }
                let mut memory = crate::system::CallMemory::new(kernel, cx, tid);
                let met = syscall::touch::touch(&mut memory, call);
                unmet = memory.unmet;
                met
            })
        {
            let memory = unmet.unwrap_or(self.process_of(tid).memory);
            self.park(tid, Channel::Watch(memory.0));
            return;
        }
        self.threads[&tid].taken.borrow_mut().rewind();
        self.begin_asking(tid);
        let mut outcome = syscall::dispatch(self, cx, tid, call, again);
        if again && let Some(thread) = self.threads.get_mut(&tid) {
            thread.suspended_in_call = false;
        }
        // A copy that reached pages of a mapped backend file not read yet found them not there:
        // a call that failed for it runs again once they have come, as Linux's copy waits for
        // the fault. One that did part of its work first keeps its short result (counted).
        if let Some(client) = self.files.backend.clone()
            && client.waits_for_pages(tid)
        {
            match outcome {
                syscall::Outcome::Return(value) if !crate::errno::Errno::is_error_return(value) => {
                    client.stop_waiting_for_pages(tid);
                    self.counters
                        .count(cx.host, reason::COPY_SHORT_OF_PAGES, call.number.0);
                }
                syscall::Outcome::Return(_) | syscall::Outcome::Restart => {
                    outcome = syscall::Outcome::Parked;
                }
                syscall::Outcome::Parked | syscall::Outcome::Gone => {}
            }
        }
        // A copy outside the call's description is counted. If it met a watched page, a call that
        // failed for it runs again once the watch is met, doing again what it did before the
        // copy; one that did part of its work keeps its short result (both counted).
        let mut touched = None;
        let mut undescribed = false;
        if watched && let Some(thread) = self.threads.get(&tid) {
            for pid in core::iter::once(thread.process).chain(other) {
                let Some(process) = self.processes.get(&pid) else {
                    continue;
                };
                let watches = &mut process.map.borrow_mut().watches;
                watches.described = None;
                undescribed |= watches.undescribed.take().is_some();
                if let Some((address, access)) = watches.touched.take() {
                    touched.get_or_insert((pid, process.memory, address, access));
                }
            }
        }
        if undescribed {
            self.counters
                .count(cx.host, reason::COPY_UNDESCRIBED, call.number.0);
        }
        let mut runs_again = false;
        if let Some((pid, memory, address, access)) = touched {
            let met = self.watch_touched(cx, tid, pid, address, access);
            match outcome {
                syscall::Outcome::Return(value) if !crate::errno::Errno::is_error_return(value) => {
                    self.counters.count(
                        cx.host,
                        reason::CALL_SHORT_OF_WATCHED_MEMORY,
                        call.number.0,
                    );
                }
                syscall::Outcome::Return(_) | syscall::Outcome::Restart => {
                    self.counters
                        .count(cx.host, reason::WATCH_MET_AT_A_COPY, call.number.0);
                    if met {
                        // The thread runs its system call instruction again.
                        let mut registers = RegisterBlock::ZERO;
                        cx.cpu.registers(self.threads[&tid].cpu, &mut registers);
                        registers.pc -= size_of::<u32>() as u64;
                        cx.cpu.set_registers(self.threads[&tid].cpu, &registers);
                        outcome = syscall::Outcome::Restart;
                        runs_again = true;
                    } else {
                        self.park(tid, Channel::Watch(memory.0));
                        outcome = syscall::Outcome::Parked;
                    }
                }
                syscall::Outcome::Parked | syscall::Outcome::Gone => {}
            }
        }
        // A request outgrew the executor's ceiling. It failed before it had any effect, and its
        // result reaches the thread only once the hook has returned.
        if let Some(bytes) = self.map_notes.outgrown.take()
            && let Some(thread) = self.threads.get(&tid)
        {
            let pid = thread.process;
            self.counters
                .count(cx.host, reason::PROCESS_OUTGROWN, call.number.0);
            self.announce(cx, pid, |system, process| {
                system.on_process_outgrown(process, bytes)
            });
        }
        self.end_asking(tid, matches!(outcome, syscall::Outcome::Parked));
        if !self.dnotified.is_empty() {
            syscall::dnotify::send(self, cx);
        }
        if let Some(thread) = self.threads.get(&tid) {
            syscall::ipc::settle(self, cx, thread.process);
        }
        // A call may have ended its own thread's process (a signal it sent itself).
        if !self.threads.contains_key(&tid) {
            return;
        }
        match outcome {
            syscall::Outcome::Return(value) => {
                self.finish_call(cx, tid);
                let pid = self.threads[&tid].process;
                let thread = crate::system::task_id(tid);
                let mut response = crate::system::response_of(value);
                self.announce(cx, pid, |system, process| {
                    response =
                        system.intercept_system_call_response(process, thread, &call, response);
                });
                // The embedder's answer may have ended the thread's process (suspended, later).
                if !self.threads.contains_key(&tid) {
                    return;
                }
                let value = crate::system::return_of(response);
                self.return_to_user(cx, tid, Resume::Return(value));
            }
            syscall::Outcome::Restart => {
                self.finish_call(cx, tid);
                if !runs_again {
                    self.answer_in_registers(cx, tid, &call);
                }
                self.return_to_user(cx, tid, Resume::FromRegisters);
            }
            // A call woken within its own run (what it waits for came while it parked) is in
            // line and runs again, as Linux's task woken between `prepare_to_wait` and
            // `schedule` does.
            syscall::Outcome::Parked => {
                debug_assert!(
                    self.threads[&tid].call.wait.is_some() || self.in_line(tid),
                    "a parked call neither waits nor is in line"
                );
            }
            syscall::Outcome::Gone => {}
        }
    }

    /// Asks the embedder for its answer to a call that replaced its thread's registers (a new
    /// program, a handler's return): the result is the thread's x0, and the answer goes there.
    pub(crate) fn answer_in_registers(
        &mut self,
        cx: &mut Context<'_>,
        tid: Tid,
        call: &SystemCallRequest,
    ) {
        let Some(thread) = self.threads.get(&tid) else {
            return;
        };
        let (pid, cpu) = (thread.process, thread.cpu);
        let mut registers = RegisterBlock::ZERO;
        cx.cpu.registers(cpu, &mut registers);
        let given = crate::system::response_of(registers.x[0]);
        let mut response = given;
        let thread = crate::system::task_id(tid);
        self.announce(cx, pid, |system, process| {
            response = system.intercept_system_call_response(process, thread, call, response);
        });
        if response != given && self.threads.contains_key(&tid) {
            registers.x[0] = crate::system::return_of(response);
            cx.cpu.set_registers(cpu, &registers);
        }
    }

    /// A run of `tid`'s call starts: the backend's questions are asked as from its thread.
    pub(crate) fn begin_asking(&self, tid: Tid) {
        if let Some(client) = &self.files.backend {
            let thread = &self.threads[&tid];
            client.begin(crate::fs::backend::Asker {
                tid,
                uid: thread.credentials.fsuid,
                gid: thread.credentials.fsgid,
                pid: thread.process as u32,
            });
        }
    }

    /// The run of `tid`'s call ended, `parked` or not: a parked call that waits for the backend
    /// waits on it, and a call that ended forgets its answers.
    pub(crate) fn end_asking(&mut self, tid: Tid, parked: bool) {
        let Some(client) = self.files.backend.clone() else {
            return;
        };
        let waits = parked && client.waits(tid);
        client.end(tid, !parked);
        if waits
            && self
                .threads
                .get(&tid)
                .is_some_and(|thread| thread.call.wait.is_none())
            && !self.run_queue.contains(tid)
        {
            self.park(tid, Channel::Backend);
        }
    }

    /// Whether a thread that does not wait is in line to run, or waits aside in its stopped process.
    fn in_line(&self, tid: Tid) -> bool {
        self.run_queue.contains(tid) || self.processes[&self.threads[&tid].process].stopped
    }

    /// Takes a lock a call keeps until it ends, while it waits too (Linux's mutexes a sleeping
    /// call holds), on `channel`: the address of what it guards, under `Channel::Held`, or a pipe's
    /// own channel. `holder` is the thread that took it last, which holds it while its call still
    /// does. Parks while another thread's call holds it.
    pub(crate) fn take_lock(
        &mut self,
        tid: Tid,
        holder: Option<Tid>,
        channel: Channel,
    ) -> Result<(), Errno> {
        if self.held_by_other(tid, holder, channel) {
            self.park(tid, channel);
            return Err(Errno::PARKED);
        }
        self.hold_lock(tid, channel);
        Ok(())
    }

    /// Whether another thread's call holds the lock on `channel` that `holder` took last.
    pub(crate) fn held_by_other(&self, tid: Tid, holder: Option<Tid>, channel: Channel) -> bool {
        holder.is_some_and(|holder| {
            holder != tid
                && self
                    .threads
                    .get(&holder)
                    .is_some_and(|thread| thread.call.holds.contains(&Some(channel)))
        })
    }

    /// Marks the lock on `channel` as held by `tid`'s call until it ends.
    pub(crate) fn hold_lock(&mut self, tid: Tid, channel: Channel) {
        let holds = &mut self.thread_mut(tid).call.holds;
        if !holds.contains(&Some(channel)) {
            let free = holds
                .iter_mut()
                .find(|slot| slot.is_none())
                .expect("a call holds at most two locks");
            *free = Some(channel);
        }
    }

    /// Lets go of the locks a call held: whoever waits for one tries again.
    fn let_go(&mut self, holds: [Option<Channel>; 2]) {
        for channel in holds.into_iter().flatten() {
            self.wake(channel);
        }
    }

    /// Clears a finished call's slot; a named pipe's open still waiting gives up its end.
    pub(crate) fn finish_call(&mut self, cx: &mut Context<'_>, tid: Tid) {
        let thread = self.thread_mut(tid);
        let holds = core::mem::take(&mut thread.call).holds;
        let taken = thread.taken.get_mut().clear();
        self.give_back(taken);
        self.let_go(holds);
        let thread = self.thread_mut(tid);
        let opening = thread.opening.take();
        self.give_up_opening(opening);
        if !self.released.is_empty() {
            self.files_released(cx);
        }
    }

    /// A call's files go: those only it held (their descriptors were closed meanwhile) are let
    /// go as closed descriptors' are.
    fn give_back(&mut self, taken: alloc::vec::Vec<crate::file::FileRef>) {
        let only_held = taken
            .into_iter()
            .filter(|file| alloc::rc::Rc::strong_count(file) == 1);
        self.released.extend(only_held);
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
        if !self.dnotified.is_empty() {
            syscall::dnotify::send(self, cx);
        }
        loop {
            if !self.files.terminals.is_empty() {
                syscall::tty::released(self, cx);
            }
            if core::mem::take(&mut self.aio_polls_due) {
                syscall::aio::polled(self, cx);
                continue;
            }
            // A woken thread's call runs again when its turn comes, as Linux's waiter goes on in its
            // call once it is scheduled.
            if let Some(tid) = self.next_to_run()
                && let Some(index) = self.woken.iter().position(|woken| *woken == tid)
            {
                self.woken.remove(index);
                self.take_out_of_line(tid);
                // The container's start has no registers: it runs on from where it got.
                if self.is_starting(tid) {
                    self.run_start(cx);
                    continue;
                }
                if self.dumps.contains_key(&tid) {
                    syscall::coredump::run(self, cx, tid);
                    continue;
                }
                let thread = &self.threads[&tid];
                let mut registers = RegisterBlock::ZERO;
                cx.cpu.registers(thread.cpu, &mut registers);
                let x = &registers.x;
                let mut arguments = [0; SYSTEM_CALL_ARGUMENT_COUNT];
                arguments.copy_from_slice(&x[..SYSTEM_CALL_ARGUMENT_COUNT]);
                let call = SystemCallRequest {
                    number: SystemCallNumber(x[SYSTEM_CALL_NUMBER_REGISTER]),
                    arguments,
                };
                self.stepping = Some(tid);
                self.system_call(cx, tid, call, true);
                self.stepping = None;
                // A call that went back to its program has its thread's turn.
                if self.run_queue.contains(tid) {
                    self.take_out_of_line(tid);
                    self.give_way();
                    self.take_turn(tid);
                }
                continue;
            }
            if let Some(client) = self
                .files
                .backend
                .as_ref()
                .filter(|client| client.has_dropped())
            {
                for node in client.take_dropped() {
                    let _ = crate::shared::backend_dropped(self, cx, &node);
                }
                continue;
            }
            if self
                .files
                .backend
                .as_ref()
                .is_some_and(|client| client.has_outgoing())
            {
                self.send_to_backend(cx);
                continue;
            }
            if self.serve_ends(cx) {
                continue;
            }
            if !self.kernel_writes.get_mut().is_empty() {
                self.announce_kernel_writes(cx);
                continue;
            }
            if self.map_notes.sharing_changed.get() {
                self.announce_sharing(cx);
                continue;
            }
            if let Some((pid, tid, info)) = self.signals_sent.pop_front() {
                self.send_from_embedder(cx, pid, tid, info);
                continue;
            }
            if let Some((pid, signal, info)) = self.signals_to_returning.pop_front() {
                if self.processes.contains_key(&pid) {
                    self.send_to_process(cx, pid, signal, info);
                }
                continue;
            }
            // A thread of a process a fatal signal is ending ends it when its turn comes.
            if !self.dying.is_empty()
                && let Some(tid) = self.next_to_run()
                && self.dying.contains_key(&self.threads[&tid].process)
            {
                let resume = self.threads[&tid].resume;
                self.take_out_of_line(tid);
                self.return_to_user(cx, tid, resume);
                continue;
            }
            if let Some(tid) = self.signalled.pop_front() {
                let Some(thread) = self.threads.get(&tid) else {
                    continue;
                };
                let process = thread.process;
                if thread.call.wait.is_some()
                    || self.processes[&process].stopped
                    || self.dying.contains_key(&process)
                {
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
                self.stepping = Some(tid);
                self.return_to_user(cx, tid, resume);
                self.stepping = None;
                continue;
            }
            break;
        }
        if !self.ids_let_go.is_empty() {
            self.release_ids(cx);
        }
        self.unsettled = false;
        self.last_now = cx.monotonic();
        for _ in 0..core::mem::take(&mut cx.refused) {
            self.counters.count(cx.host, reason::HOST_CANNOT, 0);
        }
        let unasked = self
            .files
            .backend
            .as_ref()
            .map_or(0, |client| client.unasked());
        for _ in 0..unasked {
            self.counters
                .count(cx.host, reason::BACKEND_PAGES_UNASKED, 0);
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
    pub(crate) fn next_to_run(&self) -> Option<Tid> {
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
        self.wake_poll(channel, 0);
    }

    /// As [`Kernel::wake`] for a change to an object's readiness with the events in `key` (0 for
    /// any), which epoll items watching it take (`wake_up_poll`).
    pub(crate) fn wake_poll(&mut self, channel: Channel, key: u32) {
        self.ends.woken(channel);
        let readiness = matches!(channel, Channel::Object(_)).then_some(Channel::Readiness);
        for tid in self.parked.on_either(channel, readiness) {
            self.wake_thread(tid);
        }
        if let Channel::Object(object) = channel {
            syscall::epoll::notify(self, syscall::epoll::Queue::Object(object), key);
            self.aio_polls_due |=
                syscall::aio::notify(self, syscall::epoll::Queue::Object(object), key);
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
        syscall::epoll::notify(self, syscall::epoll::Queue::Signals(pid), 0);
        self.aio_polls_due |= syscall::aio::notify(self, syscall::epoll::Queue::Signals(pid), 0);
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
        // A task suspended off the kernel runs its call again when it is put back.
        let Some(thread) = self.threads.get_mut(&tid) else {
            return;
        };
        if let Some(channel) = thread.call.wait.take() {
            thread.woken_at = Some(entry_now);
            let deadline = thread.call.deadline;
            let filling = core::mem::take(&mut thread.filling);
            let delivering = core::mem::take(&mut thread.delivering);
            let resume = thread.resume;
            self.parked.remove(tid, channel, deadline);
            // Its registers still hold the call, which runs again before it goes on, or the
            // instruction that touched pages it waited for, which touches them again; a frame
            // that waited is written when it acts on its signals.
            if !filling {
                self.woken.push_back(tid);
            }
            // A thread of a process a fatal signal is ending acts on it first.
            let dying =
                !self.dying.is_empty() && self.dying.contains_key(&self.threads[&tid].process);
            if delivering {
                self.make_runnable(tid, resume);
            } else {
                self.make_runnable(tid, Resume::FromRegisters);
            }
            // A thread that touched watched memory outside a call acts on a signal sent while it
            // waited before it touches the memory again.
            let signal_waits = filling && self.signal_pending(tid);
            if (delivering || dying || signal_waits) && !self.signalled.contains(&tid) {
                self.signalled.push_back(tid);
            }
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
        self.thread_mut(tid).call.holds = Default::default();
        let taken = self.thread_mut(tid).taken.get_mut().clear();
        self.give_back(taken);
        self.let_go(call.holds);
        if self.current == Some(tid) {
            self.current = None;
        }
        self.woken.retain(|other| *other != tid);
        self.signalled.retain(|other| *other != tid);
    }

    /// Ends one thread of a process that has others.
    pub(crate) fn end_thread(&mut self, cx: &mut Context<'_>, tid: Tid) {
        syscall::futex::thread_end(self, cx, tid, true);
        syscall::timer::thread_end(self, tid);
        if let Some(client) = &self.files.backend {
            client.drop_call(tid);
        }
        self.unschedule(tid);
        self.release_vfork(tid);
        syscall::sem::forget(self, tid);
        syscall::sem::exit_sem(self, cx, self.threads[&tid].process, tid);
        syscall::mqueue::forget(self, tid);
        let thread = self.threads.remove(&tid).expect("thread exists");
        self.ids_let_go.push(tid);
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
        process.ended_system += thread.system;
        if let Some(leader) = leader {
            process.ended_leader = leader;
        }
        // With its other tasks suspended, its children wait for one to come back.
        if let Some(&heir) = process.threads.first() {
            self.hand_children_to(cx, tid, heir);
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
        // Its tasks taken off the kernel end too, as stopped threads do in Linux; the caller still
        // uses the memory.
        self.end_tasks_away(cx, pid, |_| true);
        if tid == pid {
            return tid;
        }
        let mut thread = self.threads.remove(&tid).expect("thread exists");
        thread.call = Call::default();
        let taken = thread.taken.get_mut().clear();
        self.give_back(taken);
        // Its own number goes with the first thread, and the descriptors naming it with it.
        if let Some(node) = thread.pidfd.take() {
            self.wake(syscall::pidfd::channel(&node));
        }
        self.threads.insert(pid, thread);
        self.ids_let_go.push(tid);
        syscall::sem::renumber(self, tid, pid);
        for process in self.processes.values_mut() {
            if process.parent_thread == tid {
                process.parent_thread = pid;
            }
        }
        for away in self.processes_away.values_mut() {
            if away.parent_thread == tid {
                away.parent_thread = pid;
            }
        }
        self.processes
            .get_mut(&pid)
            .expect("process exists")
            .threads = alloc::vec![pid];
        if self.current == Some(tid) {
            self.current = Some(pid);
        }
        pid
    }

    /// Ends the tasks of process `pid` taken off the kernel, as Linux ends stopped threads: their
    /// last stores into the memory (`others_use_memory` of each in turn), the semaphore
    /// adjustments and waits they leave, the `vfork` parent they kept waiting; their numbers are
    /// let go of, and putting one back hands it back.
    fn end_tasks_away(
        &mut self,
        cx: &mut Context<'_>,
        pid: Pid,
        others_use_memory: impl Fn(usize) -> bool,
    ) {
        let away = core::mem::take(&mut self.processes.get_mut(&pid).expect("process exists").away);
        for (index, (tid, task)) in away.into_iter().enumerate() {
            if let Some(end) = syscall::futex::TaskEnd::of_away(pid, tid, &task) {
                syscall::futex::task_end(self, cx, &end, others_use_memory(index));
            }
            syscall::timer::thread_end(self, tid);
            if task.vfork_parent.is_some() {
                self.wake(Channel::Vfork(tid));
            }
            syscall::sem::forget(self, tid);
            syscall::sem::exit_sem(self, cx, pid, tid);
            syscall::mqueue::forget(self, tid);
            self.ids_let_go.push(tid);
        }
    }

    /// Ends a whole process with a wait status, as `exit_group` and fatal signals do: its threads
    /// and memory go, its children go to a reaper, and its parent is told. It stays as a zombie
    /// until its parent reaps it, unless the parent does not want it.
    pub(crate) fn end_process(&mut self, cx: &mut Context<'_>, pid: Pid, wait_status: i32) {
        if self.processes[&pid].ended.is_some() {
            return;
        }
        self.dying.remove(&pid);
        let threads = self.processes[&pid].threads.clone();
        let memory = self.processes[&pid].memory;
        let shared = self.memory_in_use_elsewhere(pid, memory);
        // The kernel's last stores into the process's memory, as Linux's `mm_release`, come
        // before the embedder hears of the end, so a watch it sets then has nothing left to meet.
        // Its tasks taken off the kernel end first, as stopped threads do; each end sees the
        // memory still used by those yet to end.
        let away = self.processes[&pid].away.len();
        self.end_tasks_away(cx, pid, |index| {
            shared || index + 1 < away || !threads.is_empty()
        });
        for (index, &tid) in threads.iter().enumerate() {
            syscall::futex::thread_end(self, cx, tid, shared || index + 1 < threads.len());
        }
        let ending = self.ending_process.replace(pid);
        self.announce(cx, pid, |system, process| {
            system.on_process_exiting(process)
        });
        self.withdraw_sharing(cx, pid);
        self.ending_process = ending;
        // The container ends with its first process, and the rest are killed then, as Linux's
        // first process of a namespace kills them on its way out (`zap_pid_ns_processes`).
        if self.init == Some(pid) {
            let others: alloc::vec::Vec<Pid> = self
                .processes
                .iter()
                .filter(|(other, process)| **other != pid && process.ended.is_none())
                .map(|(other, _)| *other)
                .collect();
            for other in others {
                self.end_process(cx, other, SIGKILL as i32);
            }
        }
        self.ids_let_go.extend(&threads);
        // The threads' descriptor tables, closed where the process's files close.
        let mut tables = alloc::vec::Vec::new();
        for tid in threads {
            if self.dumps.remove(&tid).is_some()
                && let Some(client) = &self.files.backend
            {
                client.drop_call(tid);
            }
            syscall::timer::thread_end(self, tid);
            self.unschedule(tid);
            self.release_vfork(tid);
            syscall::sem::forget(self, tid);
            syscall::sem::exit_sem(self, cx, pid, tid);
            syscall::mqueue::forget(self, tid);
            let thread = self.threads.remove(&tid).expect("thread exists");
            cx.cpu.thread_drop(thread.cpu);
            if let Some(node) = &thread.pidfd {
                self.wake(syscall::pidfd::channel(node));
            }
            let process = self.processes.get_mut(&pid).expect("process exists");
            process.ended_time += thread.runtime;
            process.ended_system += thread.system;
            if tid == pid {
                process.ended_leader = Leader::of(&thread);
            }
            tables.push(thread.files);
        }
        syscall::timer::process_end(self, pid, false);
        // No hook comes before the process is marked ended.
        self.sharing_left.remove(&pid);
        let process = self.processes.get_mut(&pid).expect("process exists");
        process.threads.clear();
        process.ended = Some(wait_status);
        let pidfd = process.pidfd.clone();
        process.stopped = false;
        process.report = None;
        process.pending = Default::default();
        let memory = process.memory;
        let empty = crate::memory::AddressMap::new(
            self.machine.address_space_bytes,
            self.ceiling,
            self.map_notes.clone(),
        );
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
        // A reader of this process's memory that waits on a watch finds it gone.
        self.wake(Channel::Watch(memory.0));
        self.update_watching();
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
            // Nothing carries any number once the container has ended.
            self.finished = Some(wait_status);
            let carried: alloc::vec::Vec<Pid> = self
                .processes
                .values()
                .flat_map(|process| [process.pid, process.group, process.session])
                .chain(
                    self.processes_away
                        .iter()
                        .flat_map(|(pid, away)| [*pid, away.group]),
                )
                .collect();
            self.ids_let_go.extend(carried);
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
        // A task taken off the kernel counts, as a stopped one does; a first thread among them is
        // counted above.
        let away = self
            .processes
            .values()
            .flat_map(|process| {
                process
                    .away
                    .iter()
                    .filter(move |(tid, _)| **tid != process.pid)
            })
            .filter(|(_, away)| away.shown.credentials.uid == uid)
            .count();
        let tasks = (live + ended + away) as i64;
        i128::from(tasks + more) > i128::from(limit.min(i64::MAX as u64))
    }

    /// Whether another live process shows the same memory (made with CLONE_VM), or a context
    /// off this kernel does.
    pub(crate) fn memory_in_use_elsewhere(
        &self,
        pid: Pid,
        memory: wcontainers_abi::MemoryId,
    ) -> bool {
        self.processes
            .values()
            .any(|other| other.pid != pid && other.ended.is_none() && other.memory == memory)
            || self
                .maps_away
                .borrow()
                .iter()
                .any(|(away, map)| *away == memory && map.strong_count() > 0)
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

    /// An ended thread's children go to `heir`, a live thread of its process, each sent its
    /// parent death signal (`forget_original_parent` finding a live thread of the group).
    fn hand_children_to(&mut self, cx: &mut Context<'_>, tid: Tid, heir: Tid) {
        let children: alloc::vec::Vec<Pid> = self
            .processes
            .values()
            .filter(|process| process.parent_thread == tid)
            .map(|process| process.pid)
            .collect();
        for child in children {
            self.processes.get_mut(&child).expect("child").parent_thread = heir;
            self.send_parent_death_signals(cx, child);
        }
    }

    /// Sends each thread of `child` that asked for one its parent death signal.
    fn send_parent_death_signals(&mut self, cx: &mut Context<'_>, child: Pid) {
        for tid in self.processes[&child].threads.clone() {
            let signal = self.threads[&tid].parent_death_signal;
            if signal > 0 {
                let info = crate::signal::kernel_info(signal as u32);
                self.send_to_thread(cx, tid, signal as u32, info);
            }
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
        let away = self.processes_away.values().any(|away| away.parent == pid);
        if children.is_empty() && !away {
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
        // The reaper's first live thread takes them (`find_alive_thread`).
        let heir = self.processes[&reaper]
            .threads
            .first()
            .copied()
            .unwrap_or(0);
        for away in self.processes_away.values_mut() {
            if away.parent == pid {
                away.parent = reaper;
                away.parent_thread = heir;
                away.exit_signal = SIGCHLD;
            }
        }
        for child in children {
            let process = self.processes.get_mut(&child).expect("child");
            process.parent = reaper;
            process.parent_thread = heir;
            // A new parent always hears of the end by SIGCHLD (`reparent_leader`).
            process.exit_signal = SIGCHLD;
            self.send_parent_death_signals(cx, child);
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

    /// Linux's `ptrace_may_access` with PTRACE_MODE_READ_FSCREDS ([`crate::fs::proc::ptrace_may_read`]).
    pub(crate) fn ptrace_may_read(&self, caller: Tid, target: Pid) -> bool {
        crate::fs::proc::ptrace_may_read(&self.processes, &self.threads, caller, target)
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
        // A parent whose context is away reaps it when it is back, and is told then.
        let away = self.processes_away.contains_key(&parent);
        if !away && !self.processes.contains_key(&parent) {
            if let Some(process) = self.processes.remove(&pid) {
                self.let_go_of_ids(&process);
            }
            return;
        }
        if exit_signal != 0 {
            let info = self.child_info(pid, status);
            match self.processes_away.get_mut(&parent) {
                Some(away) => away.signals.push((exit_signal, info)),
                None => self.send_to_process(cx, parent, exit_signal, info),
            }
        }
        if away {
            return;
        }
        let unwanted = exit_signal == SIGCHLD && Self::children_unwanted(&self.processes[&parent]);
        self.wake_waiters_of(parent);
        if unwanted {
            self.reap(pid);
        }
    }

    /// Whether a process takes no zombies of its children that end with SIGCHLD.
    pub(crate) fn children_unwanted(process: &crate::process::Process) -> bool {
        let action = process.actions.borrow()[SIGCHLD as usize - 1];
        action.sa_handler_kernel == crate::linux_extra::SIG_IGN
            || action.sa_flags & u64::from(SA_NOCLDWAIT) != 0
    }

    /// A zombie handed to a new parent: reaped at once if that parent does not want it, else the
    /// parent is told as `reparent_leader` does.
    fn notify_parent_of_zombie(&mut self, cx: &mut Context<'_>, pid: Pid) {
        self.notify_parent(cx, pid);
    }

    /// The `siginfo` of SIGCHLD for a child that ended or changed state.
    pub(crate) fn child_info(&self, pid: Pid, status: i32) -> [u8; 128] {
        let process = &self.processes[&pid];
        let (code, value) = match crate::wait::decode(status) {
            crate::wait::Ended::Exited(code) => (CLD_EXITED, code),
            crate::wait::Ended::Killed {
                signal,
                core_dumped: true,
            } => (CLD_DUMPED, signal),
            crate::wait::Ended::Killed { signal, .. } => (CLD_KILLED, signal),
        };
        let time = process.ended_time / crate::linux_extra::NANOSECONDS_PER_TICK;
        let uid = self.task_credentials(pid).uid;
        crate::signal::child_info(pid, uid, code as i32, value, time)
    }

    /// Takes a zombie away for good, adding its times to its parent's children's.
    pub(crate) fn reap(&mut self, pid: Pid) {
        let Some(process) = self.processes.remove(&pid) else {
            return;
        };
        self.let_go_of_ids(&process);
        if let Some(node) = &process.pidfd {
            if let crate::fs::Content::Pidfd(_, exit) = &mut node.borrow_mut().content {
                *exit = process.ended;
            }
            self.wake(syscall::pidfd::channel(node));
        }
        if let Some(parent) = self.processes.get_mut(&process.parent) {
            parent.children_time += process.ended_time + process.children_time;
            parent.children_system += process.ended_system + process.children_system;
            parent.children_maxrss = parent
                .children_maxrss
                .max(process.peak_mapped.max(process.children_maxrss));
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
        // What a release lets go of (a socket's queue, a collected cycle) is released in turn.
        // Cycles are looked for when a local socket is released (`unix_release_sock`).
        loop {
            let sockets = self.release_round(cx);
            let collected = sockets && crate::syscall::socket::collect(self, cx);
            if !collected && self.released.is_empty() {
                break;
            }
        }
    }

    /// One round of [`Self::files_released`]; returns whether a local socket was released.
    fn release_round(&mut self, cx: &mut Context<'_>) -> bool {
        let mut files = core::mem::take(&mut self.released);
        let mut notices = core::mem::take(&mut self.notices);
        let mut let_go = false;
        for thread in self.threads.values() {
            let mut table = thread.files.borrow_mut();
            files.extend(table.take_released());
            notices.append(&mut table.notices);
            let_go |= table.take_let_go_locks();
        }
        for (socket, cookie) in notices {
            crate::syscall::socket::netlink::send_from_kernel(self, cx, &socket, cookie);
        }
        let sockets = files.iter().any(|file| {
            alloc::rc::Rc::strong_count(file) == 1
                && crate::syscall::socket::of(file).is_some_and(|socket| socket.borrow().local())
        });
        let nodes: alloc::vec::Vec<_> = files
            .iter()
            .map(|file| file.borrow().node.clone())
            .collect();
        // A socket whose last file went is released (`sock_close`).
        for file in &files {
            if alloc::rc::Rc::strong_count(file) == 1
                && let Some(socket) = crate::syscall::socket::of(file)
            {
                crate::syscall::socket::released(self, cx, &socket);
            }
        }
        crate::syscall::socket::flush_outside(self, cx);
        // An open file's last close (`__fput`): inotify's close event, and an instance lets go of
        // its watches.
        for file in &files {
            if alloc::rc::Rc::strong_count(file) != 1 {
                continue;
            }
            crate::syscall::lease::released(self, file);
            let instance = crate::syscall::inotify::of_file(file);
            match instance {
                Some(instance) => crate::syscall::inotify::released(&instance),
                None => {
                    let writer = file.borrow().flags & O_ACCMODE != O_RDONLY;
                    let path = file.borrow().flags & O_PATH != 0;
                    if !path {
                        let mask = if writer {
                            IN_CLOSE_WRITE
                        } else {
                            IN_CLOSE_NOWRITE
                        };
                        crate::syscall::inotify::file_changed(self, file, mask);
                    }
                    crate::syscall::inotify::closed(self, file);
                }
            }
        }
        crate::syscall::pipe::released(self, cx, files);
        // An open file's last close lets go of its record and flock locks.
        for node in nodes {
            let mut node = node.borrow_mut();
            let_go |= crate::locks::release_dead(&mut node.record_locks);
            let flocks = node.flocks.len();
            node.flocks.retain(|(holder, ..)| holder.strong_count() > 0);
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
        sockets
    }
}
