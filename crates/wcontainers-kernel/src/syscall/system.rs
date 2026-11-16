//! Calls about the machine: its name, its memory, randomness and the one CPU.

use alloc::vec;
use alloc::vec::Vec;

use super::{Args, guest};
use crate::counters::reason;
use crate::linux::general::*;
use crate::linux::system::{__NEW_UTS_LEN, new_utsname};
use crate::linux_extra::{SCHED_ATTR_SIZE_VER0, SCHED_ATTR_SIZE_VER1, sched_attr};
use crate::process::Tid;
use crate::{Context, Errno, Kernel};
use wcontainers_abi::PAGE_SIZE;
use zerocopy::IntoBytes;

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
    // The reference's copy is from a stack buffer one byte past 16-byte alignment (measured), which
    // decides what a buffer that faults part way receives.
    guest(kernel, cx, tid).write_from(out, &bytes, 1)?;
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
    let uptime = cx.monotonic().saturating_sub(kernel.boot_monotonic) / 1_000_000_000;
    let memory = kernel.machine.memory_bytes;
    let (in_use, shared) = kernel.memory_in_use();
    kernel
        .counters
        .count(cx.host, reason::RESIDENCY_NOT_TRACKED, 0);
    // `struct sysinfo` has padding, so its fields are placed by their offsets, with the padding zero.
    use crate::linux::system::sysinfo as Info;
    use core::mem::offset_of;
    let mut bytes = vec![0u8; size_of::<Info>()];
    let mut put = |offset: usize, value: &[u8]| {
        bytes[offset..offset + value.len()].copy_from_slice(value);
    };
    put(offset_of!(Info, uptime), &uptime.max(1).to_le_bytes());
    put(offset_of!(Info, totalram), &memory.to_le_bytes());
    put(offset_of!(Info, freeram), &(memory - in_use).to_le_bytes());
    put(offset_of!(Info, sharedram), &shared.to_le_bytes());
    put(
        offset_of!(Info, procs),
        &(kernel.processes.len() as u16).to_le_bytes(),
    );
    put(offset_of!(Info, mem_unit), &1u32.to_le_bytes());
    guest(kernel, cx, tid).write(out, &bytes)?;
    Ok(0)
}

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
    // `import_ubuf` bounds the request as any read's.
    let length = length.min(super::io::MAX_RW_COUNT);
    if !crate::user::access_ok(buffer, length) {
        return Err(Errno::EFAULT);
    }
    super::io::random_bytes(kernel, cx, tid, &[(buffer, length)])
}

pub(super) fn sched_yield(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Result<u64, Errno> {
    kernel.yield_turn(tid);
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
    // `find_process_by_pid`: a negative number names no one.
    scheduled(kernel, tid, pid).map_err(|_| Errno::ESRCH)?;
    guest(kernel, cx, tid).write(mask, &1u64.to_le_bytes())?;
    Ok(8)
}

pub(super) fn sched_setaffinity(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, size, mask, ..]: Args,
) -> Result<u64, Errno> {
    // `find_process_by_pid`: a negative number names no one.
    scheduled(kernel, tid, pid).map_err(|_| Errno::ESRCH)?;
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
        guest(kernel, cx, tid).put(cpu, &0u32.to_le_bytes())?;
    }
    if node != 0 {
        guest(kernel, cx, tid).put(node, &0u32.to_le_bytes())?;
    }
    Ok(0)
}

/// The task a scheduling call names: 0 for the caller, a thread, or a process that has ended and is
/// not yet reaped, which Linux still finds.
fn scheduled(kernel: &Kernel, tid: Tid, pid: u64) -> Result<Task, Errno> {
    let pid = pid as i32;
    if pid < 0 {
        return Err(Errno::EINVAL);
    }
    if pid == 0 || pid == tid {
        return Ok(Task::Thread(tid));
    }
    if kernel.threads.contains_key(&pid) {
        return Ok(Task::Thread(pid));
    }
    match kernel.processes.get(&pid) {
        Some(process) if process.ended.is_some() => Ok(Task::Ended(pid)),
        _ => Err(Errno::ESRCH),
    }
}

pub(super) fn sched_getscheduler(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [pid, ..]: Args,
) -> Result<u64, Errno> {
    let target = scheduled(kernel, tid, pid)?;
    let schedule = task_schedule(kernel, target);
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
    let priority = task_schedule(kernel, target).priority as i32;
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

/// Linux's `do_sched_setscheduler`: the parameter, the thread, then `__sched_setscheduler` with the
/// thread's own nice value and time slice.
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
    let current = task_schedule(kernel, target);
    let (policy, flags) = match policy {
        Some(policy) if policy & SCHED_RESET_ON_FORK != 0 => (
            Some(policy & !SCHED_RESET_ON_FORK),
            u64::from(SCHED_FLAG_RESET_ON_FORK),
        ),
        policy => (policy, 0),
    };
    let request = Request {
        policy,
        flags,
        nice: current.nice,
        priority: priority as u32,
        runtime: current.slice,
        deadline: 0,
        period: 0,
    };
    change_schedule(kernel, tid, target, &request)
}

/// The time slice Linux's fair scheduler gives a thread by default (`sysctl_sched_base_slice`): 0.7 ms
/// scaled by the logarithm of the CPU count, which is one here.
const BASE_SLICE: u64 = 700_000;
/// The length of a scheduler tick (`TICK_NSEC`); the reference's kernel ticks 1000 times a second.
const TICK: u64 = 1_000_000;
/// A round-robin thread's time slice (`RR_TIMESLICE`).
const ROUND_ROBIN_SLICE: u64 = 100_000_000;

/// What a call asks the scheduler for, as Linux's `sched_attr`; no policy keeps the current one.
struct Request {
    policy: Option<u32>,
    flags: u64,
    nice: i32,
    priority: u32,
    runtime: u64,
    deadline: u64,
    period: u64,
}

fn is_fair(policy: u32) -> bool {
    matches!(policy, SCHED_NORMAL | SCHED_BATCH)
}

fn is_real_time(policy: u32) -> bool {
    matches!(policy, SCHED_FIFO | SCHED_RR)
}

/// Linux's `__checkparam_dl`: deadline parameters that make sense.
fn deadline_valid(request: &Request) -> bool {
    /// `sysctl_sched_dl_period_min` and `_max`, in nanoseconds.
    const PERIOD_MIN: u64 = 100_000;
    const PERIOD_MAX: u64 = (1 << 22) * 1000;
    if request.flags & crate::linux_extra::SCHED_FLAG_SUGOV != 0 {
        return true;
    }
    if request.deadline == 0 || request.runtime < 1 << 10 {
        return false;
    }
    if request.deadline & (1 << 63) != 0 || request.period & (1 << 63) != 0 {
        return false;
    }
    let period = if request.period == 0 {
        request.deadline
    } else {
        request.period
    };
    period >= request.deadline
        && request.deadline >= request.runtime
        && (PERIOD_MIN..=PERIOD_MAX).contains(&period)
}

/// Linux's `__sched_setscheduler` with the checks `user_check_sched_setscheduler` makes of a caller
/// without `CAP_SYS_NICE` (a default container's), whose real-time and nice limits are zero.
fn change_schedule(
    kernel: &mut Kernel,
    tid: Tid,
    target: Task,
    request: &Request,
) -> Result<u64, Errno> {
    let current = task_schedule(kernel, target);
    let (policy, reset_on_fork) = match request.policy {
        None => (current.policy, current.reset_on_fork),
        Some(policy) => {
            // The reference's kernel has no extensible scheduler class (measured: refused), although
            // it names its priority range.
            if !matches!(
                policy,
                SCHED_NORMAL | SCHED_FIFO | SCHED_RR | SCHED_BATCH | SCHED_IDLE | SCHED_DEADLINE
            ) {
                return Err(Errno::EINVAL);
            }
            (
                policy,
                request.flags & u64::from(SCHED_FLAG_RESET_ON_FORK) != 0,
            )
        }
    };
    if request.flags & !(u64::from(SCHED_FLAG_ALL) | crate::linux_extra::SCHED_FLAG_SUGOV) != 0 {
        return Err(Errno::EINVAL);
    }
    let priority = request.priority;
    if priority > 99
        || (policy == SCHED_DEADLINE && !deadline_valid(request))
        || is_real_time(policy) != (priority != 0)
    {
        return Err(Errno::EINVAL);
    }
    let limits = match target {
        Task::Thread(thread) => &kernel.process_of(thread).limits,
        Task::Ended(pid) => &kernel.processes[&pid].limits,
    };
    let real_time_limit = limits[RLIMIT_RTPRIO as usize].current;
    let nice_limit = limits[RLIMIT_NICE as usize].current;
    // Linux's `is_nice_reduction`: the nice limit allows this value.
    let allowed_nice = |nice: i32| (20 - nice) as u64 <= nice_limit;
    let needs_power = (is_fair(policy) && request.nice < current.nice && !allowed_nice(request.nice))
        || (is_real_time(policy)
            && ((policy != current.policy && real_time_limit == 0)
                || (priority > current.priority && u64::from(priority) > real_time_limit)))
        || policy == SCHED_DEADLINE
        // Leaving the idle policy is raising the priority to the current nice value's.
        || (current.policy == SCHED_IDLE && policy != SCHED_IDLE && !allowed_nice(current.nice))
        || (current.reset_on_fork && !reset_on_fork);
    if needs_power && !kernel.process_of(tid).credentials.has(CAP_SYS_NICE) {
        return Err(Errno::EPERM);
    }
    if request.flags & crate::linux_extra::SCHED_FLAG_SUGOV != 0 {
        return Err(Errno::EINVAL);
    }
    // The reference's kernel has no utilisation clamping.
    if request.flags & u64::from(SCHED_FLAG_UTIL_CLAMP) != 0 {
        return Err(Errno::EOPNOTSUPP);
    }
    let mut schedule = current;
    schedule.policy = policy;
    schedule.reset_on_fork = reset_on_fork;
    schedule.priority = priority;
    if is_fair(policy) {
        schedule.nice = request.nice;
        schedule.slice = if request.runtime == 0 {
            0
        } else {
            request.runtime.clamp(100_000, 100_000_000)
        };
    }
    if policy == SCHED_DEADLINE {
        schedule.deadline = (request.runtime, request.deadline, request.period);
    }
    *task_schedule_mut(kernel, target) = schedule;
    let Task::Thread(thread) = target else {
        return Ok(0);
    };
    let thread = kernel.thread_mut(thread);
    // `__setscheduler_params`: real-time threads have no timer slack; leaving that brings it back.
    if is_real_time(policy) || policy == SCHED_DEADLINE {
        thread.timer_slack = 0;
    } else if thread.timer_slack == 0 {
        thread.timer_slack = crate::process::TIMER_SLACK;
    }
    Ok(0)
}

/// A thread's scheduling parameters as Linux's `get_params` reports them.
fn current_attributes(schedule: &crate::process::Schedule) -> sched_attr {
    let mut attributes = sched_attr {
        sched_policy: schedule.policy,
        ..Default::default()
    };
    if schedule.reset_on_fork {
        attributes.sched_flags |= u64::from(SCHED_FLAG_RESET_ON_FORK);
    }
    if schedule.policy == SCHED_DEADLINE {
        (
            attributes.sched_runtime,
            attributes.sched_deadline,
            attributes.sched_period,
        ) = schedule.deadline;
    } else if is_real_time(schedule.policy) {
        attributes.sched_priority = schedule.priority;
    } else {
        attributes.sched_nice = schedule.nice;
        attributes.sched_runtime = slice(schedule);
    }
    attributes
}

fn slice(schedule: &crate::process::Schedule) -> u64 {
    if schedule.slice == 0 {
        BASE_SLICE
    } else {
        schedule.slice
    }
}

pub(super) fn sched_setattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, address, flags, ..]: Args,
) -> Result<u64, Errno> {
    // In the order of Linux's `sched_setattr` call and `sched_copy_attr`.
    if address == 0 || (pid as i32) < 0 || flags as u32 != 0 {
        return Err(Errno::EINVAL);
    }
    let known = size_of::<sched_attr>() as u64;
    let mut guest = guest(kernel, cx, tid);
    let size: u32 = guest.read_value(address)?;
    let size = if size == 0 {
        SCHED_ATTR_SIZE_VER0
    } else {
        size
    };
    let too_big = |guest: &mut crate::user::Guest<'_>| {
        guest.write_value(address, &(known as u32))?;
        Err(Errno::E2BIG)
    };
    if size < SCHED_ATTR_SIZE_VER0 || u64::from(size) > PAGE_SIZE {
        return too_big(&mut guest);
    }
    // Linux's `copy_struct_from_user`: what lies past the structure it knows must be zero.
    let bytes = guest.read(address, u64::from(size))?;
    if bytes.len() as u64 > known && bytes[known as usize..].iter().any(|byte| *byte != 0) {
        return too_big(&mut guest);
    }
    let mut attributes = sched_attr::default();
    let length = bytes.len().min(known as usize);
    attributes.as_mut_bytes()[..length].copy_from_slice(&bytes[..length]);
    if attributes.sched_flags & u64::from(SCHED_FLAG_UTIL_CLAMP) != 0 && size < SCHED_ATTR_SIZE_VER1
    {
        return Err(Errno::EINVAL);
    }
    let nice = attributes.sched_nice.clamp(-20, 19);
    if (attributes.sched_policy as i32) < 0 {
        return Err(Errno::EINVAL);
    }
    let keep_policy = attributes.sched_flags & u64::from(SCHED_FLAG_KEEP_POLICY) != 0;
    let target = scheduled(kernel, tid, pid)?;
    let mut request = Request {
        policy: (!keep_policy).then_some(attributes.sched_policy),
        flags: attributes.sched_flags,
        nice,
        priority: attributes.sched_priority,
        runtime: attributes.sched_runtime,
        deadline: attributes.sched_deadline,
        period: attributes.sched_period,
    };
    if attributes.sched_flags & u64::from(SCHED_FLAG_KEEP_PARAMS) != 0 {
        let kept = current_attributes(&task_schedule(kernel, target));
        let schedule = task_schedule(kernel, target);
        if schedule.policy == SCHED_DEADLINE {
            (request.runtime, request.deadline, request.period) = schedule.deadline;
        } else if is_real_time(schedule.policy) {
            request.priority = kept.sched_priority;
        } else {
            request.nice = kept.sched_nice;
            request.runtime = kept.sched_runtime;
        }
    }
    change_schedule(kernel, tid, target, &request)
}

pub(super) fn sched_getattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, address, size, flags, ..]: Args,
) -> Result<u64, Errno> {
    let size = size as u32;
    if address == 0
        || (pid as i32) < 0
        || u64::from(size) > PAGE_SIZE
        || size < SCHED_ATTR_SIZE_VER0
        || flags as u32 != 0
    {
        return Err(Errno::EINVAL);
    }
    let target = scheduled(kernel, tid, pid)?;
    let mut attributes = current_attributes(&task_schedule(kernel, target));
    let length = (size as usize).min(size_of::<sched_attr>());
    attributes.size = length as u32;
    if !crate::user::access_ok(address, size.into()) {
        return Err(Errno::EFAULT);
    }
    // Linux's `copy_struct_to_user`: what the caller has room for past the structure is cleared first.
    let mut guest = guest(kernel, cx, tid);
    if size as usize > length {
        guest.write(address + length as u64, &vec![0; size as usize - length])?;
    }
    guest.write(address, &attributes.as_bytes()[..length])?;
    Ok(0)
}

pub(super) fn sched_rr_get_interval(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, out, ..]: Args,
) -> Result<u64, Errno> {
    let target = scheduled(kernel, tid, pid)?;
    let schedule = task_schedule(kernel, target);
    // Linux's `get_rr_interval` of each class, in whole ticks.
    let interval = match schedule.policy {
        SCHED_RR => ROUND_ROBIN_SLICE,
        SCHED_FIFO | SCHED_DEADLINE => 0,
        _ => slice(&schedule) / TICK * TICK,
    };
    let value = __kernel_timespec {
        tv_sec: (interval / 1_000_000_000) as i64,
        tv_nsec: (interval % 1_000_000_000) as i64,
    };
    guest(kernel, cx, tid).write_value(out, &value)?;
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

/// A task a priority call names: a thread, or an ended process's leader, which keeps its scheduling
/// values until it is reaped.
#[derive(Clone, Copy)]
enum Task {
    Thread(Tid),
    Ended(crate::process::Pid),
}

/// The tasks `getpriority`'s `which` and `who` name: one thread (0 the caller), every thread of a
/// process group (0 the caller's) or every thread whose real user is `who` (0 the caller's).
fn named_tasks(kernel: &Kernel, tid: Tid, which: u32, who: u64) -> Result<Vec<Task>, Errno> {
    let who = who as u32;
    let caller = kernel.process_of(tid);
    let matches: &dyn Fn(&crate::process::Process) -> bool = match which {
        PRIO_PROCESS => {
            if who == 0 {
                return Ok(vec![Task::Thread(tid)]);
            }
            let who = who as i32;
            if kernel.threads.contains_key(&who) {
                return Ok(vec![Task::Thread(who)]);
            }
            return Ok(match kernel.processes.get(&who) {
                Some(process) if process.ended.is_some() => vec![Task::Ended(who)],
                _ => Vec::new(),
            });
        }
        PRIO_PGRP => {
            let group = if who == 0 { caller.group } else { who as i32 };
            &move |process| process.group == group
        }
        PRIO_USER => {
            let uid = if who == 0 {
                caller.credentials.uid
            } else {
                who
            };
            &move |process| process.credentials.uid == uid
        }
        _ => return Err(Errno::EINVAL),
    };
    // A group's members newest first, as its list holds them; a user's oldest first, as the task
    // list does.
    let mut processes: Vec<_> = kernel
        .processes
        .iter()
        .filter(|(_, process)| matches(process))
        .collect();
    if which == PRIO_PGRP {
        processes.reverse();
    }
    let mut tasks = Vec::new();
    for (pid, process) in processes {
        if process.ended.is_some() {
            tasks.push(Task::Ended(*pid));
        } else {
            tasks.extend(process.threads.iter().map(|tid| Task::Thread(*tid)));
        }
    }
    Ok(tasks)
}

fn task_schedule(kernel: &Kernel, task: Task) -> crate::process::Schedule {
    match task {
        Task::Thread(tid) => kernel.threads[&tid].schedule,
        Task::Ended(pid) => kernel.processes[&pid].ended_leader.schedule,
    }
}

fn task_credentials(kernel: &Kernel, task: Task) -> &crate::credentials::Credentials {
    match task {
        Task::Thread(tid) => &kernel.process_of(tid).credentials,
        Task::Ended(pid) => &kernel.processes[&pid].credentials,
    }
}

fn task_schedule_mut(kernel: &mut Kernel, task: Task) -> &mut crate::process::Schedule {
    match task {
        Task::Thread(tid) => &mut kernel.thread_mut(tid).schedule,
        Task::Ended(pid) => {
            &mut kernel
                .processes
                .get_mut(&pid)
                .expect("process exists")
                .ended_leader
                .schedule
        }
    }
}

pub(super) fn getpriority(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [which, who, ..]: Args,
) -> Result<u64, Errno> {
    // The highest of 20 less each task's nice value, so that the result is never negative.
    named_tasks(kernel, tid, which as u32, who)?
        .into_iter()
        .map(|task| (20 - task_schedule(kernel, task).nice) as u64)
        .max()
        .ok_or(Errno::ESRCH)
}

pub(super) fn setpriority(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [which, who, nice, ..]: Args,
) -> Result<u64, Errno> {
    // Linux's `setpriority` and `set_one_prio`: each task that refuses leaves its error, the last one
    // stays; ESRCH only when nothing is named.
    let tasks = named_tasks(kernel, tid, which as u32, who)?;
    let nice = (nice as i32).clamp(-20, 19);
    let caller = kernel.process_of(tid).credentials.clone();
    let powerful = caller.has(CAP_SYS_NICE);
    let mut result = Err(Errno::ESRCH);
    for task in tasks {
        let target = task_credentials(kernel, task);
        if target.uid != caller.euid && target.euid != caller.euid && !powerful {
            result = Err(Errno::EPERM);
            continue;
        }
        // `can_nice` checks the target's limit.
        let limit = match task {
            Task::Thread(tid) => kernel.process_of(tid).limits[RLIMIT_NICE as usize].current,
            Task::Ended(pid) => kernel.processes[&pid].limits[RLIMIT_NICE as usize].current,
        };
        if nice < task_schedule(kernel, task).nice && (20 - nice) as u64 > limit && !powerful {
            result = Err(Errno::EACCES);
            continue;
        }
        if result == Err(Errno::ESRCH) {
            result = Ok(0);
        }
        task_schedule_mut(kernel, task).nice = nice;
    }
    result
}

/// The `ioprio_*` calls' `which`: `getpriority`'s plus one.
fn io_which(which: u64) -> Result<u32, Errno> {
    use crate::linux_extra::{IOPRIO_WHO_PROCESS, IOPRIO_WHO_USER};
    if !(IOPRIO_WHO_PROCESS..=IOPRIO_WHO_USER).contains(&which) {
        return Err(Errno::EINVAL);
    }
    Ok(which as u32 - 1)
}

pub(super) fn ioprio_get(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [which, who, ..]: Args,
) -> Result<u64, Errno> {
    use crate::linux_extra::*;
    let which = io_which(which)?;
    let tasks = named_tasks(kernel, tid, which, who)?;
    // One process gives the value as set (`get_task_raw_ioprio`); a group or a user gives the best
    // (lowest) of each task's effective one, where no class means the one its scheduling policy and
    // nice value imply (`__get_task_ioprio`).
    if which == PRIO_PROCESS {
        return match tasks.first() {
            Some(task) => Ok(task_schedule(kernel, *task).io_priority.into()),
            None => Err(Errno::ESRCH),
        };
    }
    let effective = |schedule: crate::process::Schedule| {
        if schedule.io_priority >> IOPRIO_CLASS_SHIFT != IOPRIO_CLASS_NONE {
            return schedule.io_priority;
        }
        let class = match schedule.policy {
            SCHED_IDLE => IOPRIO_CLASS_IDLE,
            SCHED_FIFO | SCHED_RR => IOPRIO_CLASS_RT,
            _ => IOPRIO_CLASS_BE,
        };
        class << IOPRIO_CLASS_SHIFT | ((schedule.nice + 20) / 5) as u32
    };
    tasks
        .into_iter()
        .map(|task| u64::from(effective(task_schedule(kernel, task))))
        .min()
        .ok_or(Errno::ESRCH)
}

pub(super) fn ioprio_set(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [which, who, priority, ..]: Args,
) -> Result<u64, Errno> {
    use crate::linux_extra::*;
    // Linux's `ioprio_check_cap`, then the processes named.
    let priority = priority as u32;
    let class = (priority >> IOPRIO_CLASS_SHIFT) & 7;
    let level = priority & 7;
    let caller = kernel.process_of(tid).credentials.clone();
    match class {
        IOPRIO_CLASS_RT => {
            if !caller.has(CAP_SYS_ADMIN) && !caller.has(CAP_SYS_NICE) {
                return Err(Errno::EPERM);
            }
        }
        IOPRIO_CLASS_BE | IOPRIO_CLASS_IDLE => {}
        IOPRIO_CLASS_NONE if level == 0 => {}
        _ => return Err(Errno::EINVAL),
    }
    let tasks = named_tasks(kernel, tid, io_which(which)?, who)?;
    if tasks.is_empty() {
        return Err(Errno::ESRCH);
    }
    // `set_task_ioprio`, task by task until one refuses; an ended task has no I/O context to keep it.
    for task in tasks {
        let target = task_credentials(kernel, task);
        if target.uid != caller.euid && target.uid != caller.uid && !caller.has(CAP_SYS_NICE) {
            return Err(Errno::EPERM);
        }
        if let Task::Thread(tid) = task {
            // Linux keeps the value in 16 bits.
            kernel.thread_mut(tid).schedule.io_priority = u32::from(priority as u16);
        }
    }
    Ok(0)
}
