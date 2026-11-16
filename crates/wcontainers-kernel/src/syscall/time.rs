//! Clocks and sleeping.

use super::{Args, Outcome, guest};
use crate::linux::general::*;
use crate::linux_extra::{
    CLOCKFD, CLOCKFD_MASK, CPUCLOCK_CLOCK_MASK, CPUCLOCK_MAX, CPUCLOCK_PERTHREAD_MASK,
    CPUCLOCK_SCHED,
};
use crate::process::{Parked, Tid};
use crate::{Context, Errno, Kernel};

/// The CPU time the caller's process has used: the time since the container started, which is when
/// its one process started, less the time its threads have spent parked.
pub(super) fn cpu_time(kernel: &Kernel, cx: &mut Context<'_>, tid: Tid) -> u64 {
    let now = cx.host.monotonic_nanoseconds();
    let parked: u64 = kernel
        .process_of(tid)
        .threads
        .iter()
        .map(|thread| {
            let thread = &kernel.threads[thread];
            thread.slept
                + if thread.parked.is_some() {
                    now.saturating_sub(thread.parked_at)
                } else {
                    0
                }
        })
        .sum();
    now.saturating_sub(kernel.boot_monotonic)
        .saturating_sub(parked)
}

/// The thread a CPU clock id below zero names, as Linux's `pid_for_clock`: a process's or a thread's
/// clock, 0 meaning the caller's own; `gettime` also takes a thread of the process for the process.
/// None when the id names no clock.
fn cpu_clock_target(kernel: &Kernel, tid: Tid, id: i32, gettime: bool) -> Option<Tid> {
    if id & CPUCLOCK_CLOCK_MASK >= CPUCLOCK_MAX {
        return None;
    }
    let thread = id & CPUCLOCK_PERTHREAD_MASK != 0;
    let pid = !(id >> 3);
    if pid == 0 {
        return Some(tid);
    }
    let caller = kernel.threads[&tid].process;
    if thread {
        return kernel
            .threads
            .get(&pid)
            .filter(|target| target.process == caller)
            .map(|_| pid);
    }
    if gettime && pid == tid {
        return Some(tid);
    }
    kernel.processes.get(&pid).map(|process| process.threads[0])
}

/// Whether a clock id below zero names a clock device by descriptor (none exist here).
fn is_clock_device(id: i32) -> bool {
    id & CLOCKFD_MASK == CLOCKFD
}

/// The value of a clock in nanoseconds.
fn clock(kernel: &Kernel, cx: &mut Context<'_>, tid: Tid, clock: u64) -> Result<u64, Errno> {
    let id = clock as i32;
    if id < 0 {
        if is_clock_device(id) {
            return Err(Errno::EINVAL);
        }
        let target = cpu_clock_target(kernel, tid, id, true).ok_or(Errno::EINVAL)?;
        return Ok(cpu_time(kernel, cx, target));
    }
    Ok(match clock as u32 {
        CLOCK_REALTIME | CLOCK_REALTIME_COARSE | CLOCK_REALTIME_ALARM | CLOCK_TAI => {
            cx.host.realtime_nanoseconds()
        }
        CLOCK_MONOTONIC
        | CLOCK_MONOTONIC_RAW
        | CLOCK_MONOTONIC_COARSE
        | CLOCK_BOOTTIME
        | CLOCK_BOOTTIME_ALARM => cx.host.monotonic_nanoseconds(),
        CLOCK_PROCESS_CPUTIME_ID | CLOCK_THREAD_CPUTIME_ID => cpu_time(kernel, cx, tid),
        _ => return Err(Errno::EINVAL),
    })
}

fn timespec_bytes(nanoseconds: u64) -> __kernel_timespec {
    __kernel_timespec {
        tv_sec: (nanoseconds / 1_000_000_000) as i64,
        tv_nsec: (nanoseconds % 1_000_000_000) as i64,
    }
}

pub(super) fn clock_gettime(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, out, ..]: Args,
) -> Result<u64, Errno> {
    let now = clock(kernel, cx, tid, id)?;
    guest(kernel, cx, tid).write_value(out, &timespec_bytes(now))?;
    Ok(0)
}

pub(super) fn clock_getres(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, out, ..]: Args,
) -> Result<u64, Errno> {
    // The coarse clocks and the CPU clocks of profiling and user time tick with the scheduler (1000
    // times a second in the reference, measured); the others are as fine as the host's.
    let tick = 1_000_000;
    let signed = id as i32;
    let resolution = if signed < 0 {
        if is_clock_device(signed) || cpu_clock_target(kernel, tid, signed, false).is_none() {
            return Err(Errno::EINVAL);
        }
        if signed & CPUCLOCK_CLOCK_MASK == CPUCLOCK_SCHED {
            1
        } else {
            tick
        }
    } else {
        clock(kernel, cx, tid, id)?;
        match id as u32 {
            CLOCK_REALTIME_COARSE | CLOCK_MONOTONIC_COARSE => tick,
            _ => 1,
        }
    };
    if out != 0 {
        guest(kernel, cx, tid).write_value(out, &timespec_bytes(resolution))?;
    }
    Ok(0)
}

pub(super) fn gettimeofday(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [out, zone, ..]: Args,
) -> Result<u64, Errno> {
    let now = cx.host.realtime_nanoseconds();
    if out != 0 {
        let value = __kernel_old_timeval {
            tv_sec: (now / 1_000_000_000) as i64,
            tv_usec: (now % 1_000_000_000 / 1000) as i64,
        };
        guest(kernel, cx, tid).write_value(out, &value)?;
    }
    if zone != 0 {
        guest(kernel, cx, tid).write(zone, &[0; 8])?;
    }
    Ok(0)
}

/// Reads a sleep's duration, checked as Linux's `timespec64_valid` does.
fn read_duration(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
) -> Result<__kernel_timespec, Errno> {
    let value: __kernel_timespec = guest(kernel, cx, tid).read_value(address)?;
    if value.tv_sec < 0 || !(0..1_000_000_000).contains(&value.tv_nsec) {
        return Err(Errno::EINVAL);
    }
    Ok(value)
}

fn nanoseconds(value: &__kernel_timespec) -> u64 {
    (value.tv_sec as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(value.tv_nsec as u64)
}

/// Parks the thread until a time on the monotonic clock.
fn sleep_until(kernel: &mut Kernel, cx: &mut Context<'_>, tid: Tid, until: u64) -> Outcome {
    kernel.park(cx, tid, Parked::Until { nanoseconds: until });
    Outcome::Parked
}

pub(super) fn nanosleep(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [request, ..]: Args,
) -> Outcome {
    match read_duration(kernel, cx, tid, request) {
        Ok(duration) => {
            let until = cx
                .host
                .monotonic_nanoseconds()
                .saturating_add(nanoseconds(&duration));
            sleep_until(kernel, cx, tid, until)
        }
        Err(error) => Err(error).into(),
    }
}

pub(super) fn clock_nanosleep(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, flags, request, ..]: Args,
) -> Outcome {
    let signed = id as i32;
    let id = id as u32;
    let absolute = flags as u32 & TIMER_ABSTIME != 0;
    // In the order of Linux's `clock_nanosleep`: the clock, whether it can be slept on, the duration,
    // then the clock's own checks.
    let realtime = match id {
        _ if signed < 0 && is_clock_device(signed) => return Err(Errno::EOPNOTSUPP).into(),
        _ if signed < 0 => false,
        CLOCK_REALTIME | CLOCK_TAI | CLOCK_REALTIME_ALARM => true,
        CLOCK_MONOTONIC | CLOCK_BOOTTIME | CLOCK_BOOTTIME_ALARM => false,
        CLOCK_PROCESS_CPUTIME_ID => false,
        // Clocks that exist but cannot be slept on.
        CLOCK_THREAD_CPUTIME_ID
        | CLOCK_MONOTONIC_RAW
        | CLOCK_REALTIME_COARSE
        | CLOCK_MONOTONIC_COARSE => return Err(Errno::EOPNOTSUPP).into(),
        _ => return Err(Errno::EINVAL).into(),
    };
    let duration = match read_duration(kernel, cx, tid, request) {
        Ok(duration) => nanoseconds(&duration),
        Err(error) => return Err(error).into(),
    };
    // Linux's `alarm_timer_nsleep`: only the time flag, and waking the machine needs a power.
    if matches!(id, CLOCK_REALTIME_ALARM | CLOCK_BOOTTIME_ALARM) {
        if flags as u32 & !TIMER_ABSTIME != 0 {
            return Err(Errno::EINVAL).into();
        }
        if !kernel.process_of(tid).credentials.has(CAP_WAKE_ALARM) {
            return Err(Errno::EPERM).into();
        }
    }
    // Linux's `posix_cpu_nsleep`: the caller's own thread clock cannot be slept on, and the id must
    // name a clock.
    if signed < 0 {
        let own_thread =
            signed & CPUCLOCK_PERTHREAD_MASK != 0 && (!(signed >> 3) == 0 || !(signed >> 3) == tid);
        if own_thread || cpu_clock_target(kernel, tid, signed, false).is_none() {
            return Err(Errno::EINVAL).into();
        }
    }
    // The process's CPU clock does not move while its only thread sleeps, so a sleep on it that is not
    // already over never ends.
    if id == CLOCK_PROCESS_CPUTIME_ID || signed < 0 {
        let over = if absolute {
            duration <= cpu_time(kernel, cx, tid)
        } else {
            duration == 0
        };
        if over {
            return Outcome::Return(0);
        }
        kernel.park(cx, tid, Parked::Forever);
        return Outcome::Parked;
    }
    let now = cx.host.monotonic_nanoseconds();
    let until = if absolute {
        let clock_now = if realtime {
            cx.host.realtime_nanoseconds()
        } else {
            now
        };
        now.saturating_add(duration.saturating_sub(clock_now))
    } else {
        now.saturating_add(duration)
    };
    sleep_until(kernel, cx, tid, until)
}

pub(super) fn times(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [out, ..]: Args,
) -> Result<u64, Errno> {
    let ticks = |nanoseconds: u64| nanoseconds / 10_000_000;
    let used = ticks(cpu_time(kernel, cx, tid));
    if out != 0 {
        let value = crate::linux_extra::tms {
            tms_utime: used as i64,
            tms_stime: 0,
            tms_cutime: 0,
            tms_cstime: 0,
        };
        guest(kernel, cx, tid).write_value(out, &value)?;
    }
    Ok(ticks(cx.host.monotonic_nanoseconds()))
}
