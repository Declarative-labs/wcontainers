//! Calls about the machine: its name, its memory, randomness and the one CPU.

use alloc::vec;

use super::{Args, guest};
use crate::linux::general::*;
use crate::linux::system::{__NEW_UTS_LEN, new_utsname};
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

fn field(text: &[u8]) -> [u8; 65] {
    let mut out = [0; 65];
    let length = text.len().min(64);
    out[..length].copy_from_slice(&text[..length]);
    out
}

pub(super) fn uname(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [out, ..]: Args,
) -> Result<u64, Errno> {
    let value = new_utsname {
        sysname: field(b"Linux"),
        nodename: field(&kernel.hostname),
        release: field(kernel.machine.release.as_bytes()),
        version: field(kernel.machine.version.as_bytes()),
        machine: field(b"aarch64"),
        domainname: field(&kernel.domainname),
    };
    let mut bytes = vec![0u8; 65 * 6];
    for (index, part) in [
        value.sysname,
        value.nodename,
        value.release,
        value.version,
        value.machine,
        value.domainname,
    ]
    .iter()
    .enumerate()
    {
        bytes[index * 65..(index + 1) * 65].copy_from_slice(part);
    }
    guest(kernel, cx, tid).write(out, &bytes)?;
    Ok(0)
}

pub(super) fn sethostname(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [name, length, ..]: Args,
) -> Result<u64, Errno> {
    kernel.hostname = new_system_name(kernel, cx, tid, name, length)?;
    Ok(0)
}

pub(super) fn setdomainname(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [name, length, ..]: Args,
) -> Result<u64, Errno> {
    kernel.domainname = new_system_name(kernel, cx, tid, name, length)?;
    Ok(0)
}

/// The new name for `sethostname` or `setdomainname`, checked in Linux's order: the power first (a
/// default container lacks it), then the length, then the bytes.
fn new_system_name(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    name: u64,
    length: u64,
) -> Result<alloc::vec::Vec<u8>, Errno> {
    if !kernel
        .process_of(tid)
        .credentials
        .has(crate::linux::general::CAP_SYS_ADMIN)
    {
        return Err(Errno::EPERM);
    }
    let length = length as i32;
    if !(0..=__NEW_UTS_LEN as i32).contains(&length) {
        return Err(Errno::EINVAL);
    }
    guest(kernel, cx, tid).read(name, length as u64)
}

pub(super) fn sysinfo(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [out, ..]: Args,
) -> Result<u64, Errno> {
    let uptime = cx
        .host
        .monotonic_nanoseconds()
        .saturating_sub(kernel.boot_monotonic)
        / 1_000_000_000;
    let memory = kernel.machine.memory_bytes;
    // Linux's `struct sysinfo` on ARM64: longs, then two shorts and padding, then the high memory and
    // the unit.
    let mut bytes = vec![0u8; 112];
    let mut put =
        |offset: usize, value: u64| bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    put(0, uptime.max(1));
    put(32, memory);
    put(40, memory);
    bytes[80..82].copy_from_slice(&(kernel.processes.len() as u16).to_le_bytes());
    bytes[104..108].copy_from_slice(&1u32.to_le_bytes());
    guest(kernel, cx, tid).write(out, &bytes)?;
    Ok(0)
}

/// The most one `getrandom` call gives.
const RANDOM_MAX: u64 = 0x01ff_ffff;

pub(super) fn getrandom(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [buffer, length, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(GRND_NONBLOCK | GRND_RANDOM | GRND_INSECURE) != 0
        || flags & (GRND_RANDOM | GRND_INSECURE) == GRND_RANDOM | GRND_INSECURE
    {
        return Err(Errno::EINVAL);
    }
    let length = length.min(RANDOM_MAX).min(super::io::MAX_RW_COUNT);
    if !crate::user::access_ok(buffer, length) {
        return Err(Errno::EFAULT);
    }
    let mut done = 0;
    let mut chunk = vec![0u8; 4096];
    while done < length {
        let part = (length - done).min(4096) as usize;
        cx.host.random(&mut chunk[..part]);
        let written = guest(kernel, cx, tid).write_partial(buffer + done, &chunk[..part]);
        done += written as u64;
        if written < part {
            break;
        }
    }
    if done == 0 && length > 0 {
        return Err(Errno::EFAULT);
    }
    Ok(done)
}

pub(super) fn sched_yield(
    _: &mut Kernel,
    _: &mut Context<'_>,
    _: Tid,
    _: Args,
) -> Result<u64, Errno> {
    Ok(0)
}

/// The guest is told it has one CPU.
pub(super) fn sched_getaffinity(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, size, mask, ..]: Args,
) -> Result<u64, Errno> {
    if size < 8 || size % 8 != 0 {
        return Err(Errno::EINVAL);
    }
    if pid != 0 && !kernel.threads.contains_key(&(pid as i32)) {
        return Err(Errno::ESRCH);
    }
    guest(kernel, cx, tid).write(mask, &1u64.to_le_bytes())?;
    Ok(8)
}

pub(super) fn sched_setaffinity(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, size, mask, ..]: Args,
) -> Result<u64, Errno> {
    if pid != 0 && !kernel.threads.contains_key(&(pid as i32)) {
        return Err(Errno::ESRCH);
    }
    let bytes = guest(kernel, cx, tid).read(mask, size.min(8))?;
    if bytes.first().is_none_or(|first| first & 1 == 0) {
        return Err(Errno::EINVAL);
    }
    Ok(0)
}

pub(super) fn getcpu(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [cpu, node, ..]: Args,
) -> Result<u64, Errno> {
    if cpu != 0 {
        guest(kernel, cx, tid).write(cpu, &0u32.to_le_bytes())?;
    }
    if node != 0 {
        guest(kernel, cx, tid).write(node, &0u32.to_le_bytes())?;
    }
    Ok(0)
}

/// The thread a scheduling call names: 0 or the caller's own number. Others come with fork.
fn scheduled(kernel: &Kernel, tid: Tid, pid: u64) -> Result<Tid, Errno> {
    let pid = pid as i32;
    if pid < 0 {
        return Err(Errno::EINVAL);
    }
    if pid == 0 || pid == tid {
        return Ok(tid);
    }
    if kernel.threads.contains_key(&pid) {
        Ok(pid)
    } else {
        Err(Errno::ESRCH)
    }
}

pub(super) fn sched_getscheduler(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [pid, ..]: Args,
) -> Result<u64, Errno> {
    let target = scheduled(kernel, tid, pid)?;
    let schedule = kernel.threads[&target].schedule;
    let reset = if schedule.reset_on_fork {
        SCHED_RESET_ON_FORK
    } else {
        0
    };
    Ok(u64::from(schedule.policy | reset))
}

pub(super) fn sched_getparam(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, out, ..]: Args,
) -> Result<u64, Errno> {
    if out == 0 {
        return Err(Errno::EINVAL);
    }
    let target = scheduled(kernel, tid, pid)?;
    let priority = kernel.threads[&target].schedule.priority as i32;
    guest(kernel, cx, tid).write(out, &priority.to_le_bytes())?;
    Ok(0)
}

pub(super) fn sched_setscheduler(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, policy, param, ..]: Args,
) -> Result<u64, Errno> {
    if (policy as i32) < 0 {
        return Err(Errno::EINVAL);
    }
    set_schedule(kernel, cx, tid, pid, Some(policy as u32), param)
}

pub(super) fn sched_setparam(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, param, ..]: Args,
) -> Result<u64, Errno> {
    set_schedule(kernel, cx, tid, pid, None, param)
}

/// Linux's `do_sched_setscheduler` and the checks of `__sched_setscheduler` for an unprivileged
/// caller: a default container has no `CAP_SYS_NICE` and its real-time and nice limits are zero.
fn set_schedule(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    pid: u64,
    policy: Option<u32>,
    param: u64,
) -> Result<u64, Errno> {
    if param == 0 || (pid as i32) < 0 {
        return Err(Errno::EINVAL);
    }
    let priority: i32 = guest(kernel, cx, tid).read_value(param)?;
    let target = scheduled(kernel, tid, pid)?;
    let current = kernel.threads[&target].schedule;
    let (policy, reset_on_fork) = match policy {
        None => (current.policy, current.reset_on_fork),
        Some(policy) => {
            let reset = policy & SCHED_RESET_ON_FORK != 0;
            let policy = policy & !SCHED_RESET_ON_FORK;
            // The reference refuses the extensible class with no scheduler loaded for it (measured),
            // although it names its priority range.
            if !matches!(
                policy,
                SCHED_NORMAL | SCHED_FIFO | SCHED_RR | SCHED_BATCH | SCHED_IDLE | SCHED_DEADLINE
            ) {
                return Err(Errno::EINVAL);
            }
            (policy, reset)
        }
    };
    let real_time = matches!(policy, SCHED_FIFO | SCHED_RR);
    let priority = priority as u32;
    if priority > 99 || real_time != (priority != 0) {
        return Err(Errno::EINVAL);
    }
    if policy == SCHED_DEADLINE {
        // Linux checks the deadline parameters, which `sched_setscheduler` cannot carry.
        return Err(Errno::EINVAL);
    }
    let limits = &kernel.process_of(target).limits;
    let real_time_limit = limits[RLIMIT_RTPRIO as usize].current;
    let nice_limit = limits[RLIMIT_NICE as usize].current;
    let mut needs_power = false;
    if real_time
        && ((policy != current.policy && real_time_limit == 0)
            || (priority > current.priority && u64::from(priority) > real_time_limit))
    {
        needs_power = true;
    }
    // Leaving the idle policy is raising the priority to nice 19's or better.
    if current.policy == SCHED_IDLE
        && policy != SCHED_IDLE
        && (20 - current.nice) as u64 > nice_limit
    {
        needs_power = true;
    }
    if current.reset_on_fork && !reset_on_fork {
        needs_power = true;
    }
    if needs_power
        && !kernel
            .process_of(tid)
            .credentials
            .has(crate::linux::general::CAP_SYS_NICE)
    {
        return Err(Errno::EPERM);
    }
    kernel.thread_mut(target).schedule = crate::process::Schedule {
        policy,
        priority,
        reset_on_fork,
        ..current
    };
    Ok(0)
}

pub(super) fn sched_get_priority_max(
    _: &mut Kernel,
    _: &mut Context<'_>,
    _: Tid,
    [policy, ..]: Args,
) -> Result<u64, Errno> {
    match policy as u32 {
        SCHED_FIFO | SCHED_RR => Ok(99),
        // The reference's kernel has the extensible scheduler class.
        SCHED_NORMAL | SCHED_BATCH | SCHED_IDLE | SCHED_DEADLINE | SCHED_EXT => Ok(0),
        _ => Err(Errno::EINVAL),
    }
}

pub(super) fn sched_get_priority_min(
    _: &mut Kernel,
    _: &mut Context<'_>,
    _: Tid,
    [policy, ..]: Args,
) -> Result<u64, Errno> {
    match policy as u32 {
        SCHED_FIFO | SCHED_RR => Ok(1),
        // The reference's kernel has the extensible scheduler class.
        SCHED_NORMAL | SCHED_BATCH | SCHED_IDLE | SCHED_DEADLINE | SCHED_EXT => Ok(0),
        _ => Err(Errno::EINVAL),
    }
}

/// Which processes a `getpriority` or `setpriority` names; there is one process, so only it can match.
fn prioritized(kernel: &Kernel, tid: Tid, which: u64, who: u64) -> Result<bool, Errno> {
    let who = who as u32;
    let process = kernel.process_of(tid);
    Ok(match which as u32 {
        PRIO_PROCESS => who == 0 || who as i32 == process.pid,
        PRIO_PGRP => who == 0 || who as i32 == process.group,
        // Matched by real user; 0 is the caller's own.
        PRIO_USER => who == 0 || process.credentials.uid == who,
        _ => return Err(Errno::EINVAL),
    })
}

pub(super) fn getpriority(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [which, who, ..]: Args,
) -> Result<u64, Errno> {
    if !prioritized(kernel, tid, which, who)? {
        return Err(Errno::ESRCH);
    }
    // Linux returns 20 minus the nice value, so that the result is never negative.
    Ok((20 - kernel.threads[&tid].schedule.nice) as u64)
}

pub(super) fn setpriority(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [which, who, nice, ..]: Args,
) -> Result<u64, Errno> {
    // In the order of Linux's `setpriority` and `set_one_prio`.
    if !prioritized(kernel, tid, which, who)? {
        return Err(Errno::ESRCH);
    }
    let nice = (nice as i32).clamp(-20, 19);
    let current = kernel.threads[&tid].schedule.nice;
    let process = kernel.process_of(tid);
    let powerful = process.credentials.has(crate::linux::general::CAP_SYS_NICE);
    if nice < current
        && (20 - nice) as u64 > process.limits[RLIMIT_NICE as usize].current
        && !powerful
    {
        return Err(Errno::EACCES);
    }
    kernel.thread_mut(tid).schedule.nice = nice;
    Ok(0)
}
