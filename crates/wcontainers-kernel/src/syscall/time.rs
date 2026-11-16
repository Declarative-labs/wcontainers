//! Clocks and sleeping.

use super::{Args, Outcome, guest};
use crate::linux::general::*;
use crate::linux_extra::{
    CLOCKFD, CLOCKFD_MASK, CPUCLOCK_CLOCK_MASK, CPUCLOCK_MAX, CPUCLOCK_PERTHREAD_MASK,
    CPUCLOCK_SCHED,
};
use crate::process::{Channel, Pid, Restart, Tid};
use crate::{Context, Errno, Kernel};

/// The CPU time the caller's process has used: the time its threads have run, as charged when each
/// came back, and that of its threads that have ended.
pub(super) fn cpu_time(kernel: &Kernel, _: &mut Context<'_>, tid: Tid) -> u64 {
    process_cpu_time(kernel, kernel.threads[&tid].process)
}

/// The CPU time a process has used, one that has ended included.
fn process_cpu_time(kernel: &Kernel, pid: Pid) -> u64 {
    let process = &kernel.processes[&pid];
    process.ended_time
        + process
            .threads
            .iter()
            .map(|thread| kernel.threads[thread].runtime)
            .sum::<u64>()
}

/// The process whose time a CPU clock id below zero reads, as Linux's `pid_for_clock`: a process's or
/// a thread's clock, 0 meaning the caller's own; `gettime` also takes a thread of the process for the
/// process. A process that has ended and is not yet reaped keeps its clock. None when the id names no
/// clock.
fn cpu_clock_target(kernel: &Kernel, tid: Tid, id: i32, gettime: bool) -> Option<Pid> {
    if id & CPUCLOCK_CLOCK_MASK >= CPUCLOCK_MAX {
        return None;
    }
    let thread = id & CPUCLOCK_PERTHREAD_MASK != 0;
    let pid = !(id >> 3);
    let caller = kernel.threads[&tid].process;
    if pid == 0 {
        return Some(caller);
    }
    if thread {
        return kernel
            .threads
            .get(&pid)
            .filter(|target| target.process == caller)
            .map(|_| caller);
    }
    if gettime && pid == tid {
        return Some(caller);
    }
    kernel.processes.contains_key(&pid).then_some(pid)
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
        return Ok(process_cpu_time(kernel, target));
    }
    Ok(match clock as u32 {
        CLOCK_REALTIME | CLOCK_REALTIME_COARSE | CLOCK_REALTIME_ALARM | CLOCK_TAI => {
            cx.host.realtime_nanoseconds()
        }
        CLOCK_MONOTONIC
        | CLOCK_MONOTONIC_RAW
        | CLOCK_MONOTONIC_COARSE
        | CLOCK_BOOTTIME
        | CLOCK_BOOTTIME_ALARM => cx.monotonic(),
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
        // Each field is its own store (`put_user`).
        let mut guest = guest(kernel, cx, tid);
        guest.put(
            out + core::mem::offset_of!(__kernel_old_timeval, tv_sec) as u64,
            &(now / 1_000_000_000).to_le_bytes(),
        )?;
        guest.put(
            out + core::mem::offset_of!(__kernel_old_timeval, tv_usec) as u64,
            &(now % 1_000_000_000 / 1000).to_le_bytes(),
        )?;
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

/// A sleep until a time on the monotonic clock, as Linux's `hrtimer_nanosleep`: the timer may end as
/// late as the thread's slack after it, and with nothing else to wake the machine it does. A signal
/// interrupts it: a relative sleep writes what is left to `left` (when given) and continues from
/// `restart_syscall` if no handler runs; an absolute one simply runs again.
fn sleep_until(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    until: u64,
    left: Option<u64>,
) -> Outcome {
    let thread = kernel.thread_mut(tid);
    if !thread.call.started {
        thread.call.started = true;
        thread.call.progress = until;
        thread.call.deadline = Some(until.saturating_add(thread.timer_slack));
    }
    let call = thread.call;
    let now = cx.monotonic();
    if call.deadline.is_some_and(|deadline| deadline <= now) {
        return Outcome::Return(0);
    }
    if kernel.signal_pending(tid) {
        let Some(left) = left else {
            return Err(Errno::ERESTARTNOHAND).into();
        };
        if left != 0 {
            let remaining = timespec_bytes(call.progress.saturating_sub(now));
            if let Err(error) = guest(kernel, cx, tid).write_value(left, &remaining) {
                return Err(error).into();
            }
        }
        kernel.thread_mut(tid).restart = Some(Restart::Sleep {
            until: call.progress,
            left,
        });
        return Err(Errno::ERESTART_RESTARTBLOCK).into();
    }
    kernel.park(tid, Channel::Nothing);
    Outcome::Parked
}

/// A sleep on a clock that never reaches its time (a CPU clock that does not move while the caller
/// sleeps): only a signal ends it.
fn sleep_forever(kernel: &mut Kernel, tid: Tid, relative: bool) -> Outcome {
    kernel.thread_mut(tid).call.started = true;
    if kernel.signal_pending(tid) {
        if !relative {
            return Err(Errno::ERESTARTNOHAND).into();
        }
        kernel.thread_mut(tid).restart = Some(Restart::Forever);
        return Err(Errno::ERESTART_RESTARTBLOCK).into();
    }
    kernel.park(tid, Channel::Nothing);
    Outcome::Parked
}

/// `restart_syscall`: continues the sleep a handler-less signal interrupted.
pub(super) fn restart_syscall(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Outcome {
    match kernel.threads[&tid].restart {
        Some(Restart::Sleep { until, left }) => {
            let outcome = sleep_until(kernel, cx, tid, until, Some(left));
            if matches!(outcome, Outcome::Return(_)) {
                kernel.thread_mut(tid).restart = None;
            }
            outcome
        }
        Some(Restart::Forever) => sleep_forever(kernel, tid, true),
        Some(restart @ Restart::Futex { .. }) => super::futex::restart(kernel, cx, tid, restart),
        // `do_no_restart_syscall`.
        None => Err(Errno::EINTR).into(),
    }
}

pub(super) fn nanosleep(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [request, left, ..]: Args,
) -> Outcome {
    if kernel.threads[&tid].call.started {
        let until = kernel.threads[&tid].call.progress;
        return sleep_until(kernel, cx, tid, until, Some(left));
    }
    match read_duration(kernel, cx, tid, request) {
        Ok(duration) => {
            let until = cx.monotonic().saturating_add(nanoseconds(&duration));
            sleep_until(kernel, cx, tid, until, Some(left))
        }
        Err(error) => Err(error).into(),
    }
}

pub(super) fn clock_nanosleep(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, flags, request, left, ..]: Args,
) -> Outcome {
    let absolute = flags as u32 & TIMER_ABSTIME != 0;
    let call = kernel.threads[&tid].call;
    if call.started {
        if call.deadline.is_none() {
            return sleep_forever(kernel, tid, !absolute);
        }
        return sleep_until(kernel, cx, tid, call.progress, (!absolute).then_some(left));
    }
    let signed = id as i32;
    let id = id as u32;
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
        return sleep_forever(kernel, tid, !absolute);
    }
    let now = cx.monotonic();
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
    sleep_until(kernel, cx, tid, until, (!absolute).then_some(left))
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
    Ok(ticks(cx.monotonic()))
}
