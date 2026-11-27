//! `ptrace`: a thread traces another, which stops for it at each signal it is about to act on
//! (Linux's kernel/ptrace.c with arm64's `arch_ptrace`, which adds no request of its own that is
//! built here). Built: PTRACE_TRACEME, PTRACE_ATTACH, PTRACE_CONT, PTRACE_KILL, PTRACE_DETACH,
//! PTRACE_PEEKTEXT/DATA, PTRACE_POKETEXT/DATA, PTRACE_GETSIGINFO and PTRACE_SETSIGINFO; every
//! other request answers EIO, as Linux's `ptrace_request` answers one it does not know.

use wcontainers_executor::Executor;

use super::{Args, guest};
use crate::linux::general::*;
use crate::linux::ptrace::{
    PTRACE_ATTACH, PTRACE_CONT, PTRACE_DETACH, PTRACE_GETSIGINFO, PTRACE_KILL, PTRACE_PEEKDATA,
    PTRACE_PEEKTEXT, PTRACE_POKEDATA, PTRACE_POKETEXT, PTRACE_SETSIGINFO, PTRACE_TRACEME,
};
use crate::process::Tid;
use crate::trace::{Going, Stop, Trace};
use crate::{Context, Errno, Kernel};

/// The size of the word `PTRACE_PEEK*` and `PTRACE_POKE*` move (`unsigned long`).
const WORD: u64 = 8;

/// The size of a `siginfo` (`struct siginfo`).
const SIGINFO: u64 = 128;

pub(super) fn ptrace<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    [request, pid, address, data, ..]: Args,
) -> Result<u64, Errno> {
    if request == u64::from(PTRACE_TRACEME) {
        return trace_me(kernel, tid);
    }
    let target = pid as i32;
    let Some(thread) = kernel.threads.get(&target) else {
        // A zombie's first thread is still found, and cannot be attached to.
        let zombie = kernel
            .processes
            .get(&target)
            .is_some_and(|process| process.ended.is_some());
        return Err(if zombie && request == u64::from(PTRACE_ATTACH) {
            Errno::EPERM
        } else {
            Errno::ESRCH
        });
    };
    if request == u64::from(PTRACE_ATTACH) {
        return attach(kernel, cx, tid, target);
    }
    // `ptrace_check_attach`: only the tracer, and but for PTRACE_KILL only while the thread is
    // stopped for it and no fatal signal is ending it.
    let tracer = thread.trace.as_ref().map(|trace| trace.tracer);
    let ending = kernel.is_dying(thread.process);
    if tracer != Some(tid)
        || (request != u64::from(PTRACE_KILL) && (!kernel.trace_stopped(target) || ending))
    {
        return Err(Errno::ESRCH);
    }
    match u32::try_from(request) {
        Ok(PTRACE_PEEKTEXT | PTRACE_PEEKDATA) => peek(kernel, cx, tid, target, address, data),
        Ok(PTRACE_POKETEXT | PTRACE_POKEDATA) => poke(kernel, cx, tid, target, address, data),
        Ok(PTRACE_GETSIGINFO) => {
            let info = stop_of(kernel, target).info;
            guest(kernel, cx, tid).write(data, &info).map(|()| 0)
        }
        Ok(PTRACE_SETSIGINFO) => {
            let bytes = guest(kernel, cx, tid).read(data, SIGINFO)?;
            let info = bytes.try_into().expect("a siginfo's size");
            if let Some(stop) = kernel
                .thread_mut(target)
                .trace
                .as_mut()
                .and_then(|trace| trace.stop.as_mut())
            {
                stop.info = info;
            }
            Ok(0)
        }
        Ok(PTRACE_CONT) => resume(kernel, cx, tid, target, data, false),
        Ok(PTRACE_DETACH) => resume(kernel, cx, tid, target, data, true),
        Ok(PTRACE_KILL) => {
            // `send_sig_info(SIGKILL, SEND_SIG_NOINFO, child)`: from the tracer's process.
            let caller = kernel.threads[&tid].process;
            let uid = kernel.credentials(tid).uid;
            let info = crate::signal::sender_info(SIGKILL, SI_USER as i32, caller, uid);
            kernel.send_to_thread(cx, target, SIGKILL, info);
            Ok(0)
        }
        _ => Err(Errno::EIO),
    }
}

/// `ptrace_traceme`: the caller's process's parent thread traces it; EPERM if it is traced.
fn trace_me(kernel: &mut Kernel, tid: Tid) -> Result<u64, Errno> {
    if kernel.traced(tid) {
        return Err(Errno::EPERM);
    }
    let tracer = kernel.process_of(tid).parent_thread;
    kernel.thread_mut(tid).trace = Some(Trace { tracer, stop: None });
    kernel.inspections.get_or_insert_default();
    Ok(0)
}

/// `ptrace_attach`: EPERM for a thread of the caller's own process, one the caller may not trace
/// by its real ids (`__ptrace_may_access` with PTRACE_MODE_ATTACH_REALCREDS) or one already
/// traced; then the caller traces it and it is sent SIGSTOP from the kernel.
fn attach<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    target: Tid,
) -> Result<u64, Errno> {
    let process = kernel.threads[&target].process;
    let own = kernel.threads[&tid].process == process;
    let may =
        crate::fs::proc::ptrace_may_read_as(&kernel.processes, &kernel.threads, tid, process, true);
    if own || !may || kernel.traced(target) {
        return Err(Errno::EPERM);
    }
    kernel.thread_mut(target).trace = Some(Trace {
        tracer: tid,
        stop: None,
    });
    kernel.inspections.get_or_insert_default();
    kernel.send_to_thread(cx, target, SIGSTOP, crate::signal::kernel_info(SIGSTOP));
    Ok(0)
}

/// The stop of a thread [`ptrace`] found stopped.
fn stop_of(kernel: &Kernel, target: Tid) -> &Stop {
    kernel.threads[&target]
        .trace
        .as_ref()
        .and_then(|trace| trace.stop.as_ref())
        .expect("a stopped thread")
}

/// `generic_ptrace_peekdata`: the word at `address` of the traced thread's memory, read as a
/// forced access reaches it (EIO if it does not reach all of it), stored at `data` (EFAULT).
fn peek<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    target: Tid,
    address: u64,
    data: u64,
) -> Result<u64, Errno> {
    let process = kernel.threads[&target].process;
    let word = super::process_guest(kernel, cx, process, tid, false)
        .forced_read(crate::user::untagged(address), WORD);
    if word.len() as u64 != WORD {
        return Err(Errno::EIO);
    }
    guest(kernel, cx, tid).put(data, &word).map(|()| 0)
}

/// `generic_ptrace_pokedata`: `data` written as the word at `address` of the traced thread's
/// memory as a forced access reaches it; EIO if it does not reach all of it.
fn poke<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    target: Tid,
    address: u64,
    data: u64,
) -> Result<u64, Errno> {
    let process = kernel.threads[&target].process;
    let written = super::process_guest(kernel, cx, process, tid, false)
        .forced_write(crate::user::untagged(address), &data.to_le_bytes());
    if written != WORD {
        return Err(Errno::EIO);
    }
    Ok(0)
}

/// `ptrace_resume` (`detach` false) and `ptrace_detach`: EIO for a signal number past `_NSIG`;
/// otherwise the stopped thread goes on with signal `data` (0 for none), let go of with `detach`.
fn resume<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    target: Tid,
    data: u64,
    detach: bool,
) -> Result<u64, Errno> {
    let signal = u32::try_from(data)
        .ok()
        .filter(|signal| *signal <= _NSIG)
        .ok_or(Errno::EIO)?;
    let thread = kernel.thread_mut(target);
    let stop = if detach {
        thread.trace.take().and_then(|trace| trace.stop)
    } else {
        thread.trace.as_mut().and_then(|trace| trace.stop.take())
    }
    .expect("a stopped thread");
    let going = Going {
        signal,
        tracer: tid,
    };
    kernel.resume_traced(cx, target, stop, &going);
    Ok(0)
}
