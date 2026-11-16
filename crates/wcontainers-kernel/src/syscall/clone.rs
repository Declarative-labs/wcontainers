//! Making child processes and waiting for them: `clone` in its fork and vfork forms, `wait4` and
//! `waitid`.

use wcontainers_abi::{RegisterBlock, Resume};
use zerocopy::FromZeros;

use super::{Args, Outcome, guest};
use crate::linux::general::*;
use crate::memory::Lock;
use crate::process::{Channel, Pid, Report, Tid};
use crate::{Context, Errno, Kernel, errno};

/// Linux's `CSIGNAL`: the low byte of clone's flags is the signal sent to the parent at the end.
const CSIGNAL: u64 = 0xff;

/// What a thread shares here with the rest of its process: the process holds its descriptor
/// table, its file system context and its semaphore undo list, so a thread must share them all.
const THREAD_SHARES: u32 = CLONE_FILES | CLONE_FS | CLONE_SYSVSEM;

/// `clone(flags, stack, parent_tid, tls, child_tid)`, arm64's argument order.
pub(super) fn clone(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [flags, stack, parent_tid, tls, child_tid, _]: Args,
) -> Outcome {
    // A vfork parent runs its call again when the child lets go of the memory.
    if kernel.threads[&tid].call.started {
        let child = kernel.threads[&tid].call.progress as Pid;
        return vfork_wait(kernel, tid, child);
    }
    let exit_signal = flags & CSIGNAL;
    let flags = (flags as u32) & !(CSIGNAL as u32);
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
            // The parent's copy of the child's number, written after the child exists.
            if flags & CLONE_PARENT_SETTID != 0 {
                let _ = guest(kernel, cx, tid).put(parent_tid, &child.to_le_bytes());
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

/// A `vfork` parent waits, not to be interrupted, until its child starts a program or ends.
fn vfork_wait(kernel: &mut Kernel, tid: Tid, child: Pid) -> Outcome {
    let borrowing = kernel
        .processes
        .get(&child)
        .is_some_and(|process| process.vfork_parent == Some(tid));
    if borrowing {
        kernel.park(tid, Channel::Vfork(child));
        return Outcome::Parked;
    }
    Outcome::Return(child as u64)
}

/// Linux's `copy_process` for a child process: its checks in order, then a copy of the caller's
/// process and thread, which starts in line to run.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn make_child(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
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
    // A thread that keeps its own tables or waits like vfork, or a process that shares the
    // caller's tables or its memory while both run, is not built.
    let thread = flags & CLONE_THREAD != 0;
    let unbuilt = if thread {
        (THREAD_SHARES & !flags) | (flags & (CLONE_VFORK | CLONE_PIDFD))
    } else {
        let shares_memory = flags & CLONE_VM != 0 && flags & CLONE_VFORK == 0;
        (flags & (CLONE_FILES | CLONE_FS | CLONE_SIGHAND))
            | if shares_memory { CLONE_VM } else { 0 }
    };
    if unbuilt != 0 {
        return Err(kernel.counters.refuse_flag(
            cx.host,
            __NR_clone,
            unbuilt.into(),
            Errno::ENOSYS,
        ));
    }
    // RLIMIT_NPROC counts the child among its user's tasks; root and a caller with the powers to
    // pass it are not held (`copy_process`).
    let credentials = &kernel.processes[&pid].credentials;
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
    // The parent's descriptor for the child: its number is written before the child exists, and
    // nothing is made if that faults.
    if flags & CLONE_PIDFD != 0 {
        let fd = super::pidfd::next_descriptor(kernel, pid)?;
        guest(kernel, cx, tid).put(pidfd_out, &fd.to_le_bytes())?;
    }
    if thread {
        return make_thread(kernel, cx, tid, flags, stack, tls, child_tid);
    }
    let child = kernel.allocate_id();
    let parent = &kernel.processes[&pid];
    // The memory: a copy, or for vfork the parent's own until the child lets go of it.
    let borrow = flags & CLONE_VM != 0;
    let memory = if borrow {
        parent.memory
    } else {
        cx.cpu
            .memory_duplicate(parent.memory)
            .map_err(|error| cx.cpu_error(error))?
    };
    let mut map = parent.map.clone();
    if !borrow {
        // `dup_mmap`: what the child does not get goes, what it gets wiped is zeroed, and nothing
        // stays locked.
        let mappings: alloc::vec::Vec<_> = map.iter().cloned().collect();
        for mapping in mappings {
            let wipe = mapping.on_fork.wipe
                || matches!(mapping.backing, crate::memory::Backing::Droppable);
            let length = mapping.end - mapping.start;
            if mapping.on_fork.skip {
                map.remove(mapping.start, mapping.end);
                cx.cpu
                    .set_permissions(
                        memory,
                        mapping.start,
                        length,
                        wcontainers_abi::Permissions::empty(),
                    )
                    .map_err(errno::from_cpu)?;
                cx.cpu
                    .zero(memory, mapping.start, length)
                    .map_err(errno::from_cpu)?;
            } else if wipe {
                cx.cpu
                    .zero(memory, mapping.start, length)
                    .map_err(errno::from_cpu)?;
            }
        }
        map.set_lock_all(Lock::None);
        map.future_lock = Lock::None;
    }
    let mut process = parent.clone();
    process.pid = child;
    process.memory = memory;
    process.map = map;
    process.threads = alloc::vec![child];
    process.files = parent.files.duplicate(child as u64);
    process.pending = Default::default();
    process.ended = None;
    process.vfork_parent = (flags & CLONE_VFORK != 0).then_some(tid);
    process.stopped = false;
    process.report = None;
    process.continued_unsent = false;
    process.next_timer = 0;
    process.ended_time = 0;
    process.children_time = 0;
    process.children_maxrss = 0;
    process.peak_mapped = 0;
    process.child_subreaper = false;
    process.started = kernel.entry_now;
    process.execed = false;
    process.terminal_old_group = None;
    process.pidfd = None;
    if flags & CLONE_PARENT != 0 {
        process.parent = parent.parent;
        process.exit_signal = parent.exit_signal;
    } else {
        process.parent = pid;
        process.exit_signal = exit_signal;
    }
    let thread_cpu = cx
        .cpu
        .thread_new(memory)
        .map_err(|error| cx.cpu_error(error))?;
    let mut registers = RegisterBlock::ZERO;
    cx.cpu.registers(kernel.threads[&tid].cpu, &mut registers);
    registers.x[0] = 0;
    if stack != 0 {
        registers.sp = stack;
    }
    if flags & CLONE_SETTLS != 0 {
        registers.tpidr_el0 = tls;
    }
    cx.cpu.set_registers(thread_cpu, &registers);
    let mut thread = kernel.threads[&tid].clone();
    thread.process = child;
    thread.cpu = thread_cpu;
    thread.resume = Resume::FromRegisters;
    thread.pending = Default::default();
    thread.saved_mask = None;
    thread.call = Default::default();
    thread.restart = None;
    thread.runtime = 0;
    thread.forced = 0;
    thread.parent_death_signal = 0;
    thread.robust_list = (0, 0);
    thread.clear_child_tid = if flags & CLONE_CHILD_CLEARTID != 0 {
        child_tid
    } else {
        0
    };
    if borrow {
        thread.rseq = None;
    }
    if thread.schedule.reset_on_fork {
        thread.schedule.policy = SCHED_NORMAL;
        thread.schedule.priority = 0;
        thread.schedule.nice = thread.schedule.nice.max(0);
        thread.schedule.reset_on_fork = false;
    }
    kernel.processes.insert(child, process);
    kernel.threads.insert(child, thread);
    super::sem::copy_undo(kernel, pid, child, flags & CLONE_SYSVSEM != 0);
    if flags & CLONE_PIDFD != 0 {
        super::pidfd::give_child_descriptor(kernel, cx, tid, child)?;
    }
    // `schedule_tail`: the child writes its own number where it was asked to.
    if flags & CLONE_CHILD_SETTID != 0 {
        let _ = guest(kernel, cx, child).put(child_tid, &child.to_le_bytes());
    }
    kernel.start_new(child);
    Ok(child)
}

/// `copy_process` for a thread of the caller's process: it shares everything the process holds and
/// starts on the same memory from the caller's registers, with its own stack and thread pointer if
/// given.
fn make_thread(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
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
        .thread_new(memory)
        .map_err(|error| cx.cpu_error(error))?;
    let child = kernel.allocate_id();
    let mut registers = RegisterBlock::ZERO;
    cx.cpu.registers(kernel.threads[&tid].cpu, &mut registers);
    registers.x[0] = 0;
    if stack != 0 {
        registers.sp = stack;
    }
    if flags & CLONE_SETTLS != 0 {
        registers.tpidr_el0 = tls;
    }
    cx.cpu.set_registers(thread_cpu, &registers);
    let mut thread = kernel.threads[&tid].clone();
    thread.cpu = thread_cpu;
    thread.resume = Resume::FromRegisters;
    thread.pending = Default::default();
    thread.saved_mask = None;
    thread.call = Default::default();
    thread.restart = None;
    thread.runtime = 0;
    thread.forced = 0;
    thread.parent_death_signal = 0;
    thread.robust_list = (0, 0);
    thread.clear_child_tid = if flags & CLONE_CHILD_CLEARTID != 0 {
        child_tid
    } else {
        0
    };
    // A thread on the same memory has no alternate stack and no registered area of its own yet.
    thread.alternate_stack = (0, 0, 0);
    thread.rseq = None;
    thread.opening = None;
    if thread.schedule.reset_on_fork {
        thread.schedule.policy = SCHED_NORMAL;
        thread.schedule.priority = 0;
        thread.schedule.nice = thread.schedule.nice.max(0);
        thread.schedule.reset_on_fork = false;
    }
    kernel.threads.insert(child, thread);
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
    let mut children: alloc::vec::Vec<Pid> = kernel
        .processes
        .values()
        .filter(|process| process.parent == pid)
        .filter(|process| match which {
            Which::Any => true,
            Which::Process(target) => process.pid == target,
            Which::Group(0) => process.group == own_group,
            Which::Group(group) => process.group == group,
        })
        // `eligible_child`: a child whose end does not raise SIGCHLD is waited for only with
        // __WCLONE, and only it, unless __WALL.
        .filter(|process| {
            options & __WALL != 0 || ((process.exit_signal != SIGCHLD) == (options & __WCLONE != 0))
        })
        .map(|process| process.pid)
        .collect();
    if children.is_empty() {
        return Err(Errno::ECHILD);
    }
    children.sort_by_key(|child| kernel.processes[child].started);
    let keep = options & WNOWAIT != 0;
    // `notask_error`: a child that has ended counts only for a wait that takes ends.
    let mut waitable = false;
    for child in children {
        let process = &kernel.processes[&child];
        if let Some(status) = process.ended {
            if options & WEXITED == 0 {
                continue;
            }
            let (code, value) = if status & 0x7f == 0 {
                (CLD_EXITED, (status >> 8) & 0xff)
            } else if status & 0x80 != 0 {
                (CLD_DUMPED, status & 0x7f)
            } else {
                (CLD_KILLED, status & 0x7f)
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
                    status: ((signal as i32) << 8) | 0x7f,
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
                    status: 0xffff,
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
        usage.ru_utime = __kernel_old_timeval {
            tv_sec: (used / 1_000_000_000) as i64,
            tv_usec: (used % 1_000_000_000 / 1000) as i64,
        };
        usage.ru_maxrss = (process.peak_mapped.max(process.children_maxrss) / 1024) as i64;
    }
    usage
}

pub(super) fn wait4(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
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

pub(super) fn waitid(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
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
            let uid = kernel.processes[&child].credentials.uid;
            let Found::Child { code, value, .. } =
                find_child(kernel, tid, Which::Process(child), options)?
            else {
                unreachable!("the child was found a moment ago");
            };
            Ok(Some((child, code, value, uid, usage)))
        }
    }
}
