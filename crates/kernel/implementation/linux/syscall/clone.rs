//! Making child processes and waiting for them: `clone` in its fork and vfork forms, `wait4` and
//! `waitid`.

use crate::guest_machine::GuestMachine;
use wcontainers_abi::{RegisterBlock, Resume};
use wcontainers_executor::Executor;
use zerocopy::FromZeros;

use super::{Args, Outcome, guest};
use crate::linux::general::*;
use crate::linux_extra::PIDFD_THREAD;
use crate::memory::Lock;
use crate::process::{Channel, Pid, Report, Tid};
use crate::{Context, Errno, Kernel, errno};

/// `clone(flags, stack, parent_tid, tls, child_tid)`, arm64's argument order.
pub(super) fn clone<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    [flags, stack, parent_tid, tls, child_tid, _]: Args,
) -> Outcome {
    // A vfork parent runs its call again when the child lets go of the memory.
    if kernel.threads[&tid].call.started {
        let child = kernel.threads[&tid].call.progress as Pid;
        return vfork_wait(kernel, tid, child);
    }
    // The low byte of clone's flags is the signal sent to the parent at the end.
    let exit_signal = flags & u64::from(CSIGNAL);
    let flags = (flags as u32) & !CSIGNAL;
    // Legacy clone gives the descriptor CLONE_PIDFD makes where the parent's thread id would go.
    if flags & CLONE_PIDFD != 0 && flags & CLONE_PARENT_SETTID != 0 {
        return Err(Errno::EINVAL).into();
    }
    match make_child(
        kernel,
        cx,
        tid,
        flags,
        exit_signal as u32,
        stack,
        tls,
        child_tid,
        parent_tid,
    ) {
        Ok(child) => {
            // The parent's copy of the child's number, written after the child exists and
            // before the embedder hears of it.
            if flags & CLONE_PARENT_SETTID != 0 {
                let _ = guest(kernel, cx, tid).put(parent_tid, &child.to_le_bytes());
            }
            let process = kernel.threads[&child].process;
            if flags & CLONE_THREAD != 0 {
                let thread = crate::system::task_id(child);
                kernel.announce(cx, process, |system, process| {
                    system.on_thread_starting(process, thread)
                });
            } else {
                kernel.announce_start(cx, process, |system, process| {
                    system.on_process_starting(process)
                });
            }
            if flags & CLONE_VFORK != 0 {
                let call = &mut kernel.thread_mut(tid).call;
                call.started = true;
                call.progress = child as u64;
                return vfork_wait(kernel, tid, child);
            }
            Outcome::Return(child as u64)
        }
        Err(error) => Err(error).into(),
    }
}

/// `sched_fork` for a parent with SCHED_FLAG_RESET_ON_FORK: a real-time or deadline policy becomes
/// SCHED_OTHER at nice 0, another keeps its policy with a negative nice raised to 0; the slice asked
/// for and the flag go.
fn reset_on_fork(schedule: &mut crate::process::Schedule) {
    if !schedule.reset_on_fork {
        return;
    }
    if matches!(schedule.policy, SCHED_FIFO | SCHED_RR | SCHED_DEADLINE) {
        schedule.policy = SCHED_NORMAL;
        schedule.priority = 0;
        schedule.nice = 0;
    } else {
        schedule.nice = schedule.nice.max(0);
    }
    schedule.slice = 0;
    schedule.reset_on_fork = false;
}

/// A `vfork` parent waits, not to be interrupted, until its child starts a program or ends.
fn vfork_wait(kernel: &mut Kernel, tid: Tid, child: Tid) -> Outcome {
    // A child taken off the kernel, or whose context is, still borrows the memory.
    let borrowing = match kernel.threads.get(&child) {
        Some(thread) => thread.vfork_parent,
        None => match kernel.processes.get(&child) {
            Some(process) => process.away.get(&child).and_then(|away| away.vfork_parent),
            None => kernel
                .processes_away
                .get(&child)
                .and_then(|away| away.vfork_parent),
        },
    } == Some(tid);
    if borrowing {
        kernel.park(tid, Channel::Vfork(child));
        return Outcome::Parked;
    }
    Outcome::Return(child as u64)
}

/// Linux's `copy_process` for a child process: its checks in order, then a copy of the caller's
/// process and thread, which starts in line to run.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn make_child<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    flags: u32,
    exit_signal: u32,
    stack: u64,
    tls: u64,
    child_tid: u64,
    pidfd_out: u64,
) -> Result<Pid, Errno> {
    let pid = kernel.threads[&tid].process;
    if flags & (CLONE_NEWNS | CLONE_FS) == CLONE_NEWNS | CLONE_FS
        || flags & (CLONE_NEWUSER | CLONE_FS) == CLONE_NEWUSER | CLONE_FS
        || (flags & CLONE_THREAD != 0 && flags & CLONE_SIGHAND == 0)
        || (flags & CLONE_SIGHAND != 0 && flags & CLONE_VM == 0)
        || (flags & CLONE_PARENT != 0 && kernel.init == Some(pid))
        || (flags & CLONE_PIDFD != 0 && flags & CLONE_DETACHED != 0)
        || (flags & CLONE_THREAD != 0 && flags & (CLONE_NEWUSER | CLONE_NEWPID) != 0)
    {
        return Err(Errno::EINVAL);
    }
    let thread = flags & CLONE_THREAD != 0;
    // RLIMIT_NPROC counts the child among its user's tasks; root and a caller with the powers to
    // pass it are not held (`copy_process`).
    let credentials = kernel.task_credentials(pid);
    if kernel.over_process_limit(pid, credentials.uid, 1)
        && !credentials.has(CAP_SYS_RESOURCE)
        && !credentials.has(CAP_SYS_ADMIN)
    {
        return Err(Errno::EAGAIN);
    }
    kernel
        .processes
        .get_mut(&pid)
        .expect("process exists")
        .nproc_exceeded = false;
    // No more threads than `kernel.threads-max`, whoever asks.
    if kernel.threads.len() as u64 >= kernel.machine.threads_max() {
        return Err(Errno::EAGAIN);
    }
    // The parent's descriptor for the child: its number is written before the child exists, and
    // nothing is made if that faults.
    if flags & CLONE_PIDFD != 0 {
        let fd = super::pidfd::next_descriptor(kernel, tid)?;
        guest(kernel, cx, tid).put(pidfd_out, &fd.to_le_bytes())?;
    }
    if thread {
        let child = make_thread(kernel, cx, tid, flags, stack, tls, child_tid)?;
        if flags & CLONE_PIDFD != 0 {
            super::pidfd::give_child_descriptor(kernel, cx, tid, child, PIDFD_THREAD)?;
        }
        return Ok(child);
    }
    // The memory: a copy, or for vfork the parent's own until the child lets go of it.
    let borrow = flags & CLONE_VM != 0;
    let parent = &kernel.processes[&pid];
    let (memory, map) = if borrow {
        (parent.memory, parent.map.clone())
    } else {
        let memory = cx
            .cpu
            .duplicate_memory(parent.memory)
            .map_err(|error| cx.cpu_error(error))?;
        match copy_map(cx.cpu, &parent.map.borrow(), memory) {
            Ok(map) => (
                memory,
                alloc::sync::Arc::new(atomic_refcell::AtomicRefCell::new(map)),
            ),
            Err(error) => {
                cx.cpu.drop_memory(memory);
                return Err(error);
            }
        }
    };
    // As `copy_process`, the number is taken last, so no failure before it holds one.
    let made = match cx.cpu.make_thread(memory) {
        Ok(thread_cpu) => match kernel.allocate_id(cx) {
            Ok(child) => Ok((thread_cpu, child)),
            Err(error) => {
                cx.cpu.drop_thread(thread_cpu);
                Err(error)
            }
        },
        Err(error) => Err(cx.cpu_error(error)),
    };
    let (thread_cpu, child) = match made {
        Ok(made) => made,
        Err(error) => {
            if !borrow {
                cx.cpu.drop_memory(memory);
            }
            return Err(error);
        }
    };
    let parent = &kernel.processes[&pid];
    let mut process = parent.clone();
    if flags & CLONE_SIGHAND == 0 {
        let actions = *process.actions.borrow();
        process.actions = alloc::sync::Arc::new(atomic_refcell::AtomicRefCell::new(actions));
    }
    process.pid = child;
    process.memory = memory;
    process.map = map;
    process.map.borrow().note_sharers_changed();
    process.threads = alloc::vec![child];
    process.pending = Default::default();
    process.away = Default::default();
    process.ended = None;
    process.stopped = false;
    process.report = None;
    process.continued_unsent = false;
    process.next_timer = 0;
    process.ended_time = 0;
    process.children_time = 0;
    process.ended_system = 0;
    process.children_system = 0;
    process.children_maxrss = 0;
    process.peak_mapped = 0;
    process.child_subreaper = false;
    process.started = kernel.entry_now;
    process.execed = false;
    process.terminal_old_group = None;
    process.pidfd = None;
    if flags & CLONE_PARENT != 0 {
        process.parent = parent.parent;
        process.parent_thread = parent.parent_thread;
        process.exit_signal = parent.exit_signal;
    } else {
        process.parent = pid;
        process.parent_thread = tid;
        process.exit_signal = exit_signal;
    }
    let mut registers = RegisterBlock::ZERO;
    cx.cpu
        .read_registers(kernel.threads[&tid].cpu, &mut registers);
    registers.x[0] = 0;
    if stack != 0 {
        registers.sp = stack;
    }
    if flags & CLONE_SETTLS != 0 {
        registers.tpidr_el0 = tls;
    }
    cx.cpu.set_registers(thread_cpu, &registers);
    let mut thread = kernel.threads[&tid].clone();
    share_or_copy(&mut thread, flags, child);
    thread.vfork_parent = (flags & CLONE_VFORK != 0).then_some(tid);
    thread.process = child;
    thread.pidfd = None;
    thread.cpu = thread_cpu;
    thread.resume = Resume::FromRegisters;
    thread.pending = Default::default();
    thread.saved_mask = None;
    thread.call = Default::default();
    thread.restart = None;
    thread.runtime = 0;
    thread.system = 0;
    thread.forced = 0;
    thread.parent_death_signal = 0;
    thread.default_timer_slack = thread.timer_slack;
    thread.robust_list = (0, 0);
    thread.clear_child_tid = if flags & CLONE_CHILD_CLEARTID != 0 {
        child_tid
    } else {
        0
    };
    // A process on the caller's memory has no registered area, nor, unless the caller waits, its
    // alternate stack (`copy_process`, `rseq_fork`).
    if borrow {
        thread.rseq = None;
        if flags & CLONE_VFORK == 0 {
            thread.alternate_stack = (0, 0, 0);
        }
    }
    reset_on_fork(&mut thread.schedule);
    kernel.processes.insert(child, process);
    kernel.threads.insert(child, thread);
    super::sem::copy_undo(kernel, tid, child, flags & CLONE_SYSVSEM != 0);
    if flags & CLONE_PIDFD != 0 {
        super::pidfd::give_child_descriptor(kernel, cx, tid, child, 0)?;
    }
    // `schedule_tail`: the child writes its own number where it was asked to.
    if flags & CLONE_CHILD_SETTID != 0 {
        let _ = guest(kernel, cx, child).put(child_tid, &child.to_le_bytes());
    }
    kernel.start_new(child);
    Ok(child)
}

/// `dup_mmap`: a fork's child's map over its copy of the memory. What the child does not get goes,
/// what it gets wiped is zeroed, and nothing stays locked.
fn copy_map<C: Executor + 'static>(
    cpu: &mut GuestMachine<C>,
    parent: &crate::memory::AddressMap,
    memory: wcontainers_abi::MemoryId,
) -> Result<crate::memory::AddressMap, Errno> {
    let mut map = parent.clone();
    let mappings: alloc::vec::Vec<_> = map.iter().cloned().collect();
    for mapping in mappings {
        let wipe =
            mapping.on_fork.wipe || matches!(mapping.backing, crate::memory::Backing::Droppable);
        let length = mapping.end - mapping.start;
        // The child's guard pages are its parent's, but where nothing is copied.
        let unguarded = (mapping.on_fork.skip || wipe)
            && !map.take_guards(mapping.start, mapping.end).is_empty();
        if mapping.on_fork.skip {
            map.remove(mapping.start, mapping.end);
            // The copy maps the same shared pages as the parent; the child lets go of them before
            // its range is cleared, or the clearing would reach the parent's pages.
            if mapping.shared || matches!(mapping.backing, crate::memory::Backing::File { .. }) {
                cpu.unmap_views(memory, mapping.start, length)
                    .map_err(errno::from_cpu)?;
            }
            cpu.set_permissions(
                memory,
                mapping.start,
                length,
                wcontainers_abi::Permissions::empty(),
            )
            .map_err(errno::from_cpu)?;
            cpu.zero(memory, mapping.start, length)
                .map_err(errno::from_cpu)?;
        } else if wipe {
            cpu.zero(memory, mapping.start, length)
                .map_err(errno::from_cpu)?;
            map.forget_watches(cpu, memory, mapping.start, mapping.end);
            if unguarded {
                map.refresh(cpu, memory, mapping.start, mapping.end)?;
            }
        }
    }
    map.set_lock_all(Lock::None);
    map.future_lock = Lock::None;
    map.start_child(cpu, memory);
    Ok(map)
}

/// `copy_files` and `copy_fs`: a new thread or process shares its maker's descriptor table and file
/// system context when the flags say so, and otherwise has copies of its own.
fn share_or_copy(thread: &mut crate::process::Thread, flags: u32, child: Tid) {
    if flags & CLONE_FILES == 0 {
        let copy = thread.files.borrow().duplicate(child as u64);
        thread.files = alloc::sync::Arc::new(atomic_refcell::AtomicRefCell::new(copy));
    }
    if flags & CLONE_FS == 0 {
        let copy = thread.fs.borrow().clone();
        thread.fs = alloc::sync::Arc::new(atomic_refcell::AtomicRefCell::new(copy));
    }
}

/// `copy_process` for a thread of the caller's process: it shares everything the process holds and
/// starts on the same memory from the caller's registers, with its own stack and thread pointer if
/// given.
fn make_thread<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    flags: u32,
    stack: u64,
    tls: u64,
    child_tid: u64,
) -> Result<Pid, Errno> {
    let pid = kernel.threads[&tid].process;
    let memory = kernel.processes[&pid].memory;
    let thread_cpu = cx
        .cpu
        .make_thread(memory)
        .map_err(|error| cx.cpu_error(error))?;
    let child = match kernel.allocate_id(cx) {
        Ok(child) => child,
        Err(error) => {
            cx.cpu.drop_thread(thread_cpu);
            return Err(error);
        }
    };
    let mut registers = RegisterBlock::ZERO;
    cx.cpu
        .read_registers(kernel.threads[&tid].cpu, &mut registers);
    registers.x[0] = 0;
    if stack != 0 {
        registers.sp = stack;
    }
    if flags & CLONE_SETTLS != 0 {
        registers.tpidr_el0 = tls;
    }
    cx.cpu.set_registers(thread_cpu, &registers);
    let mut thread = kernel.threads[&tid].clone();
    share_or_copy(&mut thread, flags, child);
    thread.vfork_parent = (flags & CLONE_VFORK != 0).then_some(tid);
    thread.cpu = thread_cpu;
    thread.resume = Resume::FromRegisters;
    thread.pending = Default::default();
    thread.saved_mask = None;
    thread.call = Default::default();
    thread.restart = None;
    thread.runtime = 0;
    thread.system = 0;
    thread.forced = 0;
    thread.parent_death_signal = 0;
    thread.default_timer_slack = thread.timer_slack;
    thread.robust_list = (0, 0);
    thread.clear_child_tid = if flags & CLONE_CHILD_CLEARTID != 0 {
        child_tid
    } else {
        0
    };
    // A thread on the same memory has no registered area of its own yet, nor, unless the caller
    // waits, its alternate stack.
    if flags & CLONE_VFORK == 0 {
        thread.alternate_stack = (0, 0, 0);
    }
    thread.rseq = None;
    thread.pidfd = None;
    // A child is not traced unless its tracer asked for that (`ptrace_init_task`).
    thread.trace = None;
    thread.opening = None;
    reset_on_fork(&mut thread.schedule);
    kernel.threads.insert(child, thread);
    super::sem::copy_undo(kernel, tid, child, flags & CLONE_SYSVSEM != 0);
    kernel
        .processes
        .get_mut(&pid)
        .expect("process exists")
        .threads
        .push(child);
    if flags & CLONE_CHILD_SETTID != 0 {
        let _ = guest(kernel, cx, child).put(child_tid, &child.to_le_bytes());
    }
    kernel.start_new(child);
    Ok(child)
}

/// Which children a wait is for.
#[derive(Clone, Copy)]
enum Which {
    Any,
    Process(Pid),
    Group(Pid),
}

/// What a wait found.
enum Found {
    /// A child and the wait status, `siginfo` code and value to report.
    Child {
        pid: Pid,
        status: i32,
        code: u32,
        value: i32,
    },
    /// Children that may yet change, none of which has.
    None,
}

/// Linux's `do_wait`: the first eligible child with something to report, which an exited child's
/// report reaps unless `WNOWAIT`; ECHILD when none is eligible.
fn find_child(kernel: &mut Kernel, tid: Tid, which: Which, options: u32) -> Result<Found, Errno> {
    let pid = kernel.threads[&tid].process;
    let own_group = kernel.processes[&pid].group;
    let eligible = |child: Pid, parent: Pid, parent_thread: Tid, group: Pid, exit_signal: u32| {
        parent == pid
            && (options & __WNOTHREAD == 0 || parent_thread == tid)
            && match which {
                Which::Any => true,
                Which::Process(target) => child == target,
                Which::Group(0) => group == own_group,
                Which::Group(group_wanted) => group == group_wanted,
            }
            // `eligible_child`: a child whose end does not raise SIGCHLD is waited for only with
            // __WCLONE, and only it, unless __WALL.
            && (options & __WALL != 0 || ((exit_signal != SIGCHLD) == (options & __WCLONE != 0)))
    };
    let mut children: alloc::vec::Vec<Pid> = kernel
        .processes
        .values()
        .filter(|process| {
            eligible(
                process.pid,
                process.parent,
                process.parent_thread,
                process.group,
                process.exit_signal,
            )
        })
        .map(|process| process.pid)
        .collect();
    // A child whose context is taken off the kernel lives, and has nothing to report here.
    let away = kernel.processes_away.iter().any(|(&child, away)| {
        eligible(
            child,
            away.parent,
            away.parent_thread,
            away.group,
            away.exit_signal,
        )
    });
    // The threads the caller's process traces, children or not, whatever their exit signal
    // (`ptrace_do_wait`, `eligible_child`), each with its stop not yet reported.
    let traced =
        kernel.traced_for_wait(
            tid,
            options & __WNOTHREAD != 0,
            |traced, process| match which {
                Which::Any => true,
                Which::Process(target) => traced == target,
                Which::Group(0) => kernel.processes[&process].group == own_group,
                Which::Group(group_wanted) => kernel.processes[&process].group == group_wanted,
            },
        );
    // A stop for the caller's process is reported whatever the options ask for, as a trap
    // (`wait_task_stopped` for ptrace).
    if let Some(&(stopped, Some(signal))) = traced.iter().find(|(_, stop)| stop.is_some()) {
        if options & WNOWAIT == 0 {
            kernel.trace_stop_reported(stopped);
        }
        return Ok(Found::Child {
            pid: stopped,
            status: crate::wait::stopped(signal as i32),
            code: CLD_TRAPPED,
            value: signal as i32,
        });
    }
    // `do_wait` looks at the caller's own children first, then each other thread's in turn.
    let threads = &kernel.processes[&pid].threads;
    let own = threads
        .iter()
        .position(|thread| *thread == tid)
        .unwrap_or(0);
    children.sort_by_key(|child| {
        let process = &kernel.processes[child];
        let turn = threads
            .iter()
            .position(|thread| *thread == process.parent_thread)
            .map_or(usize::MAX, |at| (at + threads.len() - own) % threads.len());
        (turn, process.started)
    });
    let keep = options & WNOWAIT != 0;
    // `notask_error`: a child that has ended counts only for a wait that takes ends.
    let mut waitable = away || !traced.is_empty();
    for child in children {
        let process = &kernel.processes[&child];
        if let Some(status) = process.ended {
            if options & WEXITED == 0 {
                continue;
            }
            let (code, value) = match crate::wait::decode(status) {
                crate::wait::Ended::Exited(code) => (CLD_EXITED, code),
                crate::wait::Ended::Killed {
                    signal,
                    core_dumped: true,
                } => (CLD_DUMPED, signal),
                crate::wait::Ended::Killed { signal, .. } => (CLD_KILLED, signal),
            };
            if !keep {
                kernel.reap(child);
            }
            return Ok(Found::Child {
                pid: child,
                status,
                code,
                value,
            });
        }
        waitable = true;
        match process.report {
            Some(Report::Stopped(signal)) if options & WSTOPPED != 0 => {
                if !keep {
                    kernel.processes.get_mut(&child).expect("child").report = None;
                }
                return Ok(Found::Child {
                    pid: child,
                    status: crate::wait::stopped(signal as i32),
                    code: CLD_STOPPED,
                    value: signal as i32,
                });
            }
            Some(Report::Continued) if options & WCONTINUED != 0 => {
                if !keep {
                    kernel.processes.get_mut(&child).expect("child").report = None;
                }
                return Ok(Found::Child {
                    pid: child,
                    status: crate::wait::WAIT_CONTINUED,
                    code: CLD_CONTINUED,
                    value: SIGCONT as i32,
                });
            }
            _ => {}
        }
    }
    if waitable {
        Ok(Found::None)
    } else {
        Err(Errno::ECHILD)
    }
}

/// Parks a wait that found nothing yet, unless a signal ends it first.
fn wait_more(kernel: &mut Kernel, tid: Tid) -> Outcome {
    if kernel.signal_pending(tid) {
        return Err(Errno::ERESTARTSYS).into();
    }
    kernel.park(tid, Channel::Children);
    Outcome::Parked
}

/// The resource use a wait reports for a child: its and its reaped children's CPU time.
fn child_usage(kernel: &Kernel, child: Pid) -> rusage {
    let mut usage = rusage::new_zeroed();
    if let Some(process) = kernel.processes.get(&child) {
        let used = process.ended_time + process.children_time;
        let system = process.ended_system + process.children_system;
        usage.ru_utime = super::time::timeval(used - system);
        usage.ru_stime = super::time::timeval(system);
        usage.ru_maxrss = (process.peak_mapped.max(process.children_maxrss) / 1024) as i64;
    }
    usage
}

pub(super) fn wait4<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    [pid, status_out, options, usage_out, ..]: Args,
) -> Outcome {
    let options = options as u32;
    if options & !(WNOHANG | WUNTRACED | WCONTINUED | __WNOTHREAD | __WCLONE | __WALL) != 0 {
        return Err(Errno::EINVAL).into();
    }
    let pid = pid as i32;
    let which = match pid {
        i32::MIN => return Err(Errno::ESRCH).into(),
        -1 => Which::Any,
        0 => Which::Group(0),
        _ if pid < 0 => Which::Group(-pid),
        _ => Which::Process(pid),
    };
    // Usage is read before the child goes.
    let child_ready = find_child(kernel, tid, which, (options | WEXITED) | WNOWAIT);
    match child_ready {
        Err(error) => Err(error).into(),
        Ok(Found::None) if options & WNOHANG != 0 => Outcome::Return(0),
        Ok(Found::None) => wait_more(kernel, tid),
        Ok(Found::Child { pid: child, .. }) => {
            let usage = child_usage(kernel, child);
            let Ok(Found::Child {
                pid: child, status, ..
            }) = find_child(kernel, tid, Which::Process(child), options | WEXITED)
            else {
                unreachable!("the child was found a moment ago");
            };
            if status_out != 0
                && let Err(error) = guest(kernel, cx, tid).put(status_out, &status.to_le_bytes())
            {
                return Err(error).into();
            }
            if usage_out != 0
                && let Err(error) = guest(kernel, cx, tid).write_value(usage_out, &usage)
            {
                return Err(error).into();
            }
            Outcome::Return(child as u64)
        }
    }
}

pub(super) fn waitid<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    [kind, id, info_out, options, usage_out, _]: Args,
) -> Outcome {
    // `kernel_waitid`: what it found, or its error.
    let found = match find_for_waitid(kernel, tid, kind, id, options as u32) {
        Err(Errno::PARKED) => return Outcome::Parked,
        found => found,
    };
    // The child's use is written only when one was found, then the information always, zeros when
    // nothing was found or it failed (`SYSCALL_DEFINE5(waitid)`).
    let (result, fields) = match found {
        Ok(Some((child, code, value, uid, usage))) => {
            if usage_out != 0
                && let Err(error) = guest(kernel, cx, tid).write_value(usage_out, &usage)
            {
                return Err(error).into();
            }
            (
                Ok(0),
                [SIGCHLD as i32, 0, code as i32, child, uid as i32, value],
            )
        }
        Ok(None) => (Ok(0), [0; 6]),
        Err(error) => (Err(error), [0; 6]),
    };
    if info_out == 0 {
        return result.into();
    }
    if !crate::user::access_ok(info_out, 128) {
        return Err(Errno::EFAULT).into();
    }
    let mut guest = guest(kernel, cx, tid);
    for (offset, value) in [0u64, 4, 8, 16, 20, 24].into_iter().zip(fields) {
        if guest
            .write(info_out + offset, &value.to_le_bytes())
            .is_err()
        {
            return Err(Errno::EFAULT).into();
        }
    }
    result.into()
}

/// What `waitid` reports of a child: its number, the `siginfo` code and status, its user and use.
type Reported = (Pid, u32, i32, u32, rusage);

/// `kernel_waitid`: the checks in order, then the first child with something to report (its number,
/// the `siginfo` code and status, its user and use), none with WNOHANG, or a wait.
fn find_for_waitid(
    kernel: &mut Kernel,
    tid: Tid,
    kind: u64,
    id: u64,
    options: u32,
) -> Result<Option<Reported>, Errno> {
    if options
        & !(WNOHANG | WNOWAIT | WEXITED | WSTOPPED | WCONTINUED | __WNOTHREAD | __WCLONE | __WALL)
        != 0
        || options & (WEXITED | WSTOPPED | WCONTINUED) == 0
    {
        return Err(Errno::EINVAL);
    }
    let id = id as i32;
    let mut nonblocking = false;
    let which = match kind as u32 {
        P_ALL => Which::Any,
        P_PID if id > 0 => Which::Process(id),
        P_PGID if id >= 0 => Which::Group(id),
        // A process descriptor's process while it exists; one reaped is no one's child.
        P_PIDFD if id >= 0 => {
            let file = super::io::open_file(kernel, tid, id as u64)?;
            let file = file.borrow();
            let named = super::pidfd::named(kernel, &file)?;
            nonblocking = file.flags & O_NONBLOCK != 0;
            Which::Process(if named.exists { named.pid } else { 0 })
        }
        _ => return Err(Errno::EINVAL),
    };
    match find_child(kernel, tid, which, options | WNOWAIT)? {
        Found::None if options & WNOHANG != 0 => Ok(None),
        // A nonblocking descriptor waits as with WNOHANG, then says so.
        Found::None if nonblocking => Err(Errno::EAGAIN),
        Found::None => {
            if kernel.signal_pending(tid) {
                return Err(Errno::ERESTARTSYS);
            }
            kernel.park(tid, Channel::Children);
            Err(Errno::PARKED)
        }
        Found::Child { pid: child, .. } => {
            let usage = child_usage(kernel, child);
            let uid = kernel.task_credentials(child).uid;
            let Found::Child { code, value, .. } =
                find_child(kernel, tid, Which::Process(child), options)?
            else {
                unreachable!("the child was found a moment ago");
            };
            Ok(Some((child, code, value, uid, usage)))
        }
    }
}
