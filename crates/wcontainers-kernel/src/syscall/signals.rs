//! Sending signals to other processes and threads, and waiting for them.

use super::{Args, Outcome, guest};
use crate::linux::general::*;
use crate::process::{Channel, Pid, Tid};
use crate::signal::{UNBLOCKABLE, sender_info};
use crate::{Context, Errno, Kernel};

/// The number of signals.
const SIGNALS: u32 = crate::process::SIGNALS as u32;

/// Linux's `check_kill_permission`: an unknown signal is refused only once a target is found; a
/// process of the same session may always be continued; otherwise the sender's ids must match the
/// target's, or it needs CAP_KILL.
fn may_signal(kernel: &Kernel, tid: Tid, target: Pid, signal: u32) -> Result<(), Errno> {
    if signal > SIGNALS {
        return Err(Errno::EINVAL);
    }
    let sender = kernel.process_of(tid);
    if sender.pid == target {
        return Ok(());
    }
    let process = &kernel.processes[&target];
    let (mine, theirs) = (&sender.credentials, &process.credentials);
    let same_ids = mine.euid == theirs.suid
        || mine.euid == theirs.uid
        || mine.uid == theirs.suid
        || mine.uid == theirs.uid;
    if same_ids || mine.has(CAP_KILL) || (signal == SIGCONT && sender.session == process.session) {
        return Ok(());
    }
    Err(Errno::EPERM)
}

/// Sends `signal` from the caller to a process, after the permission check.
pub(super) fn signal_process(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    target: Pid,
    signal: u32,
    info: [u8; 128],
) -> Result<(), Errno> {
    may_signal(kernel, tid, target, signal)?;
    kernel.queue_to_process(cx, target, signal, info)
}

/// What `kill` sends: SI_USER from the caller.
fn kill_info(kernel: &Kernel, tid: Tid, signal: u32) -> [u8; 128] {
    let process = kernel.process_of(tid);
    sender_info(signal, SI_USER as i32, process.pid, process.credentials.uid)
}

/// Linux's `kill_something_info`.
pub(super) fn kill(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, signal, ..]: Args,
) -> Result<u64, Errno> {
    let (pid, signal) = (pid as i32, signal as u32);
    let info = kill_info(kernel, tid, signal);
    if pid > 0 {
        if !kernel.processes.contains_key(&pid) {
            return Err(Errno::ESRCH);
        }
        return signal_process(kernel, cx, tid, pid, signal, info).map(|()| 0);
    }
    if pid == i32::MIN {
        return Err(Errno::ESRCH);
    }
    let caller = kernel.threads[&tid].process;
    let targets: alloc::vec::Vec<Pid> = if pid == -1 {
        // Every process but the container's first and the caller.
        kernel
            .processes
            .keys()
            .copied()
            .filter(|other| Some(*other) != kernel.init && *other != caller)
            .collect()
    } else {
        let group = if pid == 0 {
            kernel.processes[&caller].group
        } else {
            -pid
        };
        kernel
            .processes
            .values()
            .filter(|process| process.group == group)
            .map(|process| process.pid)
            .collect()
    };
    if pid == -1 {
        // `kill_something_info`: the last answer that is not a refusal for permission.
        if targets.is_empty() {
            return Err(Errno::ESRCH);
        }
        let mut result = Ok(0);
        for target in targets {
            match signal_process(kernel, cx, tid, target, signal, info) {
                Err(Errno::EPERM) => {}
                answer => result = answer.map(|()| 0),
            }
        }
        return result;
    }
    signal_members(kernel, cx, tid, &targets, signal, info)
}

/// `__kill_pgrp_info`: a signal to every process of a group, ended ones too.
pub(super) fn signal_group(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    group: Pid,
    signal: u32,
    info: [u8; 128],
) -> Result<u64, Errno> {
    let targets: alloc::vec::Vec<Pid> = kernel
        .processes
        .values()
        .filter(|process| process.group == group)
        .map(|process| process.pid)
        .collect();
    signal_members(kernel, cx, tid, &targets, signal, info)
}

/// A group's answer: any success, else the last answer.
fn signal_members(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    targets: &[Pid],
    signal: u32,
    info: [u8; 128],
) -> Result<u64, Errno> {
    let mut result = Err(Errno::ESRCH);
    let mut any = false;
    for target in targets {
        match signal_process(kernel, cx, tid, *target, signal, info) {
            Ok(()) => any = true,
            Err(error) => result = Err(error),
        }
    }
    if any { Ok(0) } else { result }
}

/// A signal to one thread, from `tkill`, `tgkill`, `rt_tgsigqueueinfo` and `pidfd_send_signal`.
pub(super) fn signal_thread(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    group: Option<Pid>,
    target: Tid,
    signal: u32,
    info: Option<[u8; 128]>,
) -> Result<u64, Errno> {
    // An ended process's first thread is found until it is reaped; what it is sent is dropped
    // (`prepare_signal` for a group that is exiting).
    let (process, ended) = match kernel.threads.get(&target) {
        Some(thread) => (thread.process, false),
        None if kernel
            .processes
            .get(&target)
            .is_some_and(|process| process.ended.is_some()) =>
        {
            (target, true)
        }
        None => return Err(Errno::ESRCH),
    };
    if group.is_some_and(|group| group != process) {
        return Err(Errno::ESRCH);
    }
    may_signal(kernel, tid, process, signal)?;
    if ended {
        return Ok(0);
    }
    let info = info.unwrap_or_else(|| {
        let sender = kernel.process_of(tid);
        sender_info(signal, SI_TKILL, sender.pid, sender.credentials.uid)
    });
    kernel.queue_to_thread(cx, target, signal, info)?;
    Ok(0)
}

pub(super) fn tkill(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [target, signal, ..]: Args,
) -> Result<u64, Errno> {
    if (target as i32) <= 0 {
        return Err(Errno::EINVAL);
    }
    signal_thread(kernel, cx, tid, None, target as i32, signal as u32, None)
}

pub(super) fn tgkill(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [group, target, signal, ..]: Args,
) -> Result<u64, Errno> {
    if (group as i32) <= 0 || (target as i32) <= 0 {
        return Err(Errno::EINVAL);
    }
    signal_thread(
        kernel,
        cx,
        tid,
        Some(group as i32),
        target as i32,
        signal as u32,
        None,
    )
}

/// The size of Linux's `struct kernel_siginfo`, the part of a `siginfo` the kernel keeps.
const KERNEL_SIGINFO: u64 = 48;

/// `known_siginfo_layout`: whether Linux knows the layout of a signal's information by its code.
fn known_layout(signal: u32, code: i32) -> bool {
    if code == SI_KERNEL as i32 {
        return true;
    }
    if code > SI_USER as i32 {
        // The signals with codes of their own, and how many each has (`sig_sicodes`).
        let limit = match signal {
            SIGILL => Some(NSIGILL),
            SIGFPE => Some(NSIGFPE),
            SIGSEGV => Some(NSIGSEGV),
            SIGBUS => Some(NSIGBUS),
            SIGTRAP => Some(NSIGTRAP),
            SIGCHLD => Some(NSIGCHLD),
            SIGPOLL => Some(NSIGPOLL),
            SIGSYS => Some(NSIGSYS),
            _ => None,
        };
        return code <= limit.unwrap_or(NSIGPOLL) as i32;
    }
    code >= SI_DETHREAD || code == SI_ASYNCNL
}

/// `copy_siginfo_from_user`: the kernel's part of a program's `siginfo`, and for a layout Linux does
/// not know, the rest, which must be zero.
pub(super) fn user_info(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
) -> Result<[u8; 128], Errno> {
    let bytes = guest(kernel, cx, tid).read(address, KERNEL_SIGINFO)?;
    let mut info = [0u8; 128];
    info[..KERNEL_SIGINFO as usize].copy_from_slice(&bytes);
    let signo = u32::from_le_bytes(info[0..4].try_into().expect("four bytes"));
    let code = i32::from_le_bytes(info[8..12].try_into().expect("four bytes"));
    if !known_layout(signo, code) {
        let rest = guest(kernel, cx, tid).read(address + KERNEL_SIGINFO, 128 - KERNEL_SIGINFO)?;
        if rest.iter().any(|byte| *byte != 0) {
            return Err(Errno::E2BIG);
        }
    }
    Ok(info)
}

/// The `siginfo` a program queues: refused if it claims to come from the kernel or `tkill` and the
/// target is not the caller itself (`do_rt_sigqueueinfo`, `do_rt_tgsigqueueinfo`).
fn queued_info(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    target: Pid,
    signal: u32,
    address: u64,
) -> Result<[u8; 128], Errno> {
    let mut info = user_info(kernel, cx, tid, address)?;
    let code = i32::from_le_bytes(info[8..12].try_into().expect("four bytes"));
    if (code >= 0 || code == SI_TKILL) && tid != target {
        return Err(Errno::EPERM);
    }
    info[0..4].copy_from_slice(&signal.to_le_bytes());
    Ok(info)
}

pub(super) fn rt_sigqueueinfo(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, signal, info, ..]: Args,
) -> Result<u64, Errno> {
    let (pid, signal) = (pid as i32, signal as u32);
    let info = queued_info(kernel, cx, tid, pid, signal, info)?;
    if pid <= 0 || !kernel.processes.contains_key(&pid) {
        return Err(Errno::ESRCH);
    }
    signal_process(kernel, cx, tid, pid, signal, info).map(|()| 0)
}

pub(super) fn rt_tgsigqueueinfo(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [group, target, signal, info, ..]: Args,
) -> Result<u64, Errno> {
    let (group, target, signal) = (group as i32, target as i32, signal as u32);
    if group <= 0 || target <= 0 {
        return Err(Errno::EINVAL);
    }
    let info = queued_info(kernel, cx, tid, target, signal, info)?;
    signal_thread(kernel, cx, tid, Some(group), target, signal, Some(info))
}

/// `rt_sigsuspend`: waits with a temporary mask until a signal is acted on; the old mask comes back
/// once its handler has run.
pub(super) fn rt_sigsuspend(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [mask, size, ..]: Args,
) -> Outcome {
    if !kernel.threads[&tid].call.started {
        if size != 8 {
            return Err(Errno::EINVAL).into();
        }
        let mask: u64 = match guest(kernel, cx, tid).read_value(mask) {
            Ok(mask) => mask,
            Err(error) => return Err(error).into(),
        };
        let thread = kernel.thread_mut(tid);
        thread.saved_mask = Some(thread.signal_mask);
        thread.signal_mask = mask & !UNBLOCKABLE;
        thread.call.started = true;
    }
    if kernel.signal_pending(tid) {
        return Err(Errno::ERESTARTNOHAND).into();
    }
    kernel.park(tid, Channel::Nothing);
    Outcome::Parked
}

/// `rt_sigtimedwait`: takes a pending signal of the set, waiting for one up to the timeout; a
/// signal outside the set ends the wait with EINTR.
pub(super) fn rt_sigtimedwait(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [set, info, timeout, size, ..]: Args,
) -> Outcome {
    if !kernel.threads[&tid].call.started {
        if size != 8 {
            return Err(Errno::EINVAL).into();
        }
        let wanted: u64 = match guest(kernel, cx, tid).read_value(set) {
            Ok(set) => set,
            Err(error) => return Err(error).into(),
        };
        let deadline = if timeout != 0 {
            let value: __kernel_timespec = match guest(kernel, cx, tid).read_value(timeout) {
                Ok(value) => value,
                Err(error) => return Err(error).into(),
            };
            if value.tv_sec < 0 || !(0..1_000_000_000).contains(&value.tv_nsec) {
                return Err(Errno::EINVAL).into();
            }
            let duration = (value.tv_sec as u64)
                .saturating_mul(1_000_000_000)
                .saturating_add(value.tv_nsec as u64);
            Some(cx.host.monotonic_nanoseconds().saturating_add(duration))
        } else {
            None
        };
        let thread = kernel.thread_mut(tid);
        thread.call.started = true;
        thread.call.progress = wanted & !UNBLOCKABLE;
        thread.call.deadline = deadline;
    }
    let call = kernel.threads[&tid].call;
    if let Some(queued) = kernel.dequeue_signal(tid, call.progress) {
        if info != 0
            && let Err(error) = guest(kernel, cx, tid).write(info, &queued.info)
        {
            return Err(error).into();
        }
        return Outcome::Return(queued.signal.into());
    }
    if call
        .deadline
        .is_some_and(|deadline| deadline <= cx.host.monotonic_nanoseconds())
    {
        return Err(Errno::EAGAIN).into();
    }
    if kernel.signal_pending(tid) {
        return Err(Errno::EINTR).into();
    }
    kernel.park(tid, Channel::Signals);
    Outcome::Parked
}

/// `rt_sigreturn`: back from a handler.
pub(super) fn rt_sigreturn(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Outcome {
    kernel.sigreturn(cx, tid)
}
