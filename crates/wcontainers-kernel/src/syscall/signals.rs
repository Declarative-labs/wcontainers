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
fn signal_process(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    target: Pid,
    signal: u32,
    info: [u8; 128],
) -> Result<(), Errno> {
    may_signal(kernel, tid, target, signal)?;
    kernel.send_to_process(cx, target, signal, info);
    Ok(())
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
    let mut result = Err(Errno::ESRCH);
    let mut any = false;
    for target in targets {
        match signal_process(kernel, cx, tid, target, signal, info) {
            Ok(()) => any = true,
            Err(error) => result = Err(error),
        }
    }
    if any { Ok(0) } else { result }
}

/// A signal to one thread, from `tkill`, `tgkill` and `rt_tgsigqueueinfo`.
fn signal_thread(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    group: Option<Pid>,
    target: Tid,
    signal: u32,
    info: Option<[u8; 128]>,
) -> Result<u64, Errno> {
    let Some(thread) = kernel.threads.get(&target) else {
        return Err(Errno::ESRCH);
    };
    let process = thread.process;
    if group.is_some_and(|group| group != process) {
        return Err(Errno::ESRCH);
    }
    may_signal(kernel, tid, process, signal)?;
    let info = info.unwrap_or_else(|| {
        let sender = kernel.process_of(tid);
        sender_info(signal, SI_TKILL, sender.pid, sender.credentials.uid)
    });
    kernel.send_to_thread(cx, target, signal, info);
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
    let bytes = guest(kernel, cx, tid).read(address, 128)?;
    let mut info = [0u8; 128];
    info.copy_from_slice(&bytes);
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
