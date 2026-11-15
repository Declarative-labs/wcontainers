//! The `seccomp` call and the filters a thread installs with it, run before each of its calls together
//! with Docker's profile.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;

use wcontainers_abi::{RegisterBlock, Syscall};
use zerocopy::{FromBytes, FromZeros};

use super::{Args, Outcome, guest};
use crate::counters::reason;
use crate::credentials::CAP_SYS_ADMIN;
use crate::linux::general::SIGSYS;
use crate::linux::ptrace::*;
use crate::process::Tid;
use crate::seccomp::{DOCKER_PROFILE_LENGTH, Data, Filter, MAX_INSTRUCTIONS_PER_PATH, rank};
use crate::{Context, Errno, Kernel};

/// Every filter flag Linux knows (`SECCOMP_FILTER_FLAG_MASK`).
const FLAGS: u32 = SECCOMP_FILTER_FLAG_TSYNC
    | SECCOMP_FILTER_FLAG_LOG
    | SECCOMP_FILTER_FLAG_SPEC_ALLOW
    | SECCOMP_FILTER_FLAG_NEW_LISTENER
    | SECCOMP_FILTER_FLAG_TSYNC_ESRCH
    | SECCOMP_FILTER_FLAG_WAIT_KILLABLE_RECV;

/// Linux's `MAX_ERRNO`: the largest error a filter's ERRNO answer can give.
const MAX_ERRNO: u32 = 4095;

/// The size of Linux's `struct sock_fprog`: a 16 bit length, padding and a pointer.
const FPROG_SIZE: u64 = 16;

pub(super) fn seccomp(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [operation, flags, arguments, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    match operation as u32 {
        // The thread is already in filter mode (Docker's profile), which strict mode cannot replace.
        SECCOMP_SET_MODE_STRICT => Err(Errno::EINVAL),
        SECCOMP_SET_MODE_FILTER => set_filter(kernel, cx, tid, flags, arguments),
        SECCOMP_GET_ACTION_AVAIL => {
            if flags != 0 {
                return Err(Errno::EINVAL);
            }
            let action: u32 = guest(kernel, cx, tid).read_value(arguments)?;
            match action {
                SECCOMP_RET_KILL_PROCESS
                | SECCOMP_RET_KILL_THREAD
                | SECCOMP_RET_TRAP
                | SECCOMP_RET_ERRNO
                | SECCOMP_RET_USER_NOTIF
                | SECCOMP_RET_TRACE
                | SECCOMP_RET_LOG
                | SECCOMP_RET_ALLOW => Ok(0),
                _ => Err(Errno::EOPNOTSUPP),
            }
        }
        SECCOMP_GET_NOTIF_SIZES => {
            if flags != 0 {
                return Err(Errno::EINVAL);
            }
            let sizes = seccomp_notif_sizes {
                seccomp_notif: size_of::<seccomp_notif>() as u16,
                seccomp_notif_resp: size_of::<seccomp_notif_resp>() as u16,
                seccomp_data: size_of::<seccomp_data>() as u16,
            };
            guest(kernel, cx, tid).write_value(arguments, &sizes)?;
            Ok(0)
        }
        _ => Err(Errno::EINVAL),
    }
}

/// `prctl(PR_SET_SECCOMP, mode, filter)`: the call's operations without flags.
pub(super) fn prctl_set(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    mode: u64,
    filter: u64,
) -> Result<u64, Errno> {
    match mode {
        _ if mode == u64::from(SECCOMP_MODE_STRICT) => Err(Errno::EINVAL),
        _ if mode == u64::from(SECCOMP_MODE_FILTER) => set_filter(kernel, cx, tid, 0, filter),
        _ => Err(Errno::EINVAL),
    }
}

/// Linux's `seccomp_set_mode_filter`, in its order.
fn set_filter(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    flags: u32,
    fprog: u64,
) -> Result<u64, Errno> {
    if flags & !FLAGS != 0 {
        return Err(Errno::EINVAL);
    }
    if flags & SECCOMP_FILTER_FLAG_TSYNC != 0
        && flags & SECCOMP_FILTER_FLAG_NEW_LISTENER != 0
        && flags & SECCOMP_FILTER_FLAG_TSYNC_ESRCH == 0
    {
        return Err(Errno::EINVAL);
    }
    if flags & SECCOMP_FILTER_FLAG_WAIT_KILLABLE_RECV != 0
        && flags & SECCOMP_FILTER_FLAG_NEW_LISTENER == 0
    {
        return Err(Errno::EINVAL);
    }
    // A listener is how another process answers calls a filter sends it; there are no listeners, so a
    // filter's USER_NOTIF answer is ENOSYS, as Linux gives with none.
    if flags & SECCOMP_FILTER_FLAG_NEW_LISTENER != 0 {
        kernel.counters.count(
            cx.host,
            reason::SECCOMP_LISTENER_NOT_IMPLEMENTED,
            flags.into(),
        );
        return Err(Errno::EINVAL);
    }
    let header = guest(kernel, cx, tid).read(fprog, FPROG_SIZE)?;
    let length = usize::from(u16::from_le_bytes([header[0], header[1]]));
    let instructions = u64::from_le_bytes(header[8..16].try_into().expect("eight bytes"));
    if length == 0 || length > BPF_MAXINSNS as usize {
        return Err(Errno::EINVAL);
    }
    let credentials = &kernel.process_of(tid).credentials;
    if !credentials.no_new_privileges && !credentials.has(CAP_SYS_ADMIN) {
        return Err(Errno::EACCES);
    }
    if instructions == 0 {
        return Err(Errno::EINVAL);
    }
    let bytes = guest(kernel, cx, tid).read(instructions, (length * 8) as u64)?;
    let program: Box<[sock_filter]> = bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| sock_filter::read_from_bytes(chunk).expect("eight bytes"))
        .collect();
    let filter = Rc::new(Filter::new(program)?);
    let thread = &kernel.threads[&tid];
    let held: usize = thread
        .filters
        .iter()
        .map(|filter| filter.len() + 4)
        .sum::<usize>()
        + DOCKER_PROFILE_LENGTH
        + 4;
    if held + filter.len() > MAX_INSTRUCTIONS_PER_PATH {
        return Err(Errno::ENOMEM);
    }
    let pid = thread.process;
    let mut filters = thread.filters.clone();
    filters.push(filter);
    if flags & SECCOMP_FILTER_FLAG_TSYNC != 0 {
        // Linux's `seccomp_can_sync_threads`: every other thread's filters must be ones the caller
        // also has, in the same order.
        let others: Vec<Tid> = kernel.processes[&pid]
            .threads
            .iter()
            .copied()
            .filter(|other| *other != tid)
            .collect();
        for other in &others {
            let theirs = &kernel.threads[other].filters;
            let shared = theirs.len() < filters.len()
                && theirs
                    .iter()
                    .zip(&filters)
                    .all(|(their, our)| Rc::ptr_eq(their, our));
            if !shared {
                if flags & SECCOMP_FILTER_FLAG_TSYNC_ESRCH != 0 {
                    return Err(Errno::ESRCH);
                }
                return Ok(*other as u64);
            }
        }
        for other in others {
            kernel.thread_mut(other).filters = filters.clone();
        }
    }
    kernel.thread_mut(tid).filters = filters;
    Ok(0)
}

/// Runs the thread's filters on a call, with Docker's answer as the oldest filter's, and acts on what
/// wins as Linux's `__seccomp_filter` does. `None` lets the call through.
pub(super) fn filter_call(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    call: &Syscall,
    docker: Option<Errno>,
) -> Option<Outcome> {
    let thread = &kernel.threads[&tid];
    let mut data: Data = [0; 16];
    data[0] = call.number as u32;
    data[1] = AUDIT_ARCH_AARCH64;
    if thread.filters.iter().any(|filter| filter.reads_pointer) {
        let mut registers = RegisterBlock::new_zeroed();
        cx.cpu.registers(thread.cpu, &mut registers);
        (data[2], data[3]) = (registers.pc as u32, (registers.pc >> 32) as u32);
    }
    for (index, argument) in call.arguments.iter().enumerate() {
        (data[4 + 2 * index], data[5 + 2 * index]) = (*argument as u32, (*argument >> 32) as u32);
    }
    // Newest first; an equal action does not displace an earlier answer.
    let mut answer = SECCOMP_RET_ALLOW;
    for filter in thread.filters.iter().rev() {
        let this = filter.run(&data);
        if rank(this) < rank(answer) {
            answer = this;
        }
    }
    let profile = docker.map_or(SECCOMP_RET_ALLOW, |errno| SECCOMP_RET_ERRNO | errno.0);
    if rank(profile) < rank(answer) {
        answer = profile;
    }
    let value = answer & SECCOMP_RET_DATA;
    match answer & SECCOMP_RET_ACTION_FULL {
        SECCOMP_RET_ALLOW | SECCOMP_RET_LOG => None,
        SECCOMP_RET_ERRNO => Some(Outcome::Return((-i64::from(value.min(MAX_ERRNO))) as u64)),
        // No tracer and no listener can exist, so both are answered as Linux answers without one.
        SECCOMP_RET_TRACE | SECCOMP_RET_USER_NOTIF => {
            Some(Outcome::from(Err::<u64, _>(Errno::ENOSYS)))
        }
        SECCOMP_RET_TRAP => {
            kernel.kill_by_signal(cx, tid, SIGSYS);
            Some(Outcome::Gone)
        }
        // KILL_THREAD, KILL_PROCESS and anything unknown: the process dies of SIGSYS, whatever its
        // handler, unless only this thread is to go and others remain.
        action => {
            let pid = kernel.threads[&tid].process;
            if action == SECCOMP_RET_KILL_THREAD && kernel.processes[&pid].threads.len() > 1 {
                kernel.end_thread(cx, tid);
            } else {
                kernel.end_process(cx, pid, SIGSYS as i32);
            }
            Some(Outcome::Gone)
        }
    }
}
