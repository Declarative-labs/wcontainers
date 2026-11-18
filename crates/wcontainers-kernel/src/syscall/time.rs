//! Clocks and sleeping.

use super::{Args, Outcome, guest};
use crate::linux::general::*;
use crate::linux_extra::{
    ADJ_ADJTIME, ADJ_OFFSET_READONLY, ADJ_OFFSET_SINGLESHOT, ADJ_SETOFFSET, ADJ_TICK, CLOCKFD,
    CLOCKFD_MASK, CPUCLOCK_CLOCK_MASK, CPUCLOCK_MAX, CPUCLOCK_PERTHREAD_MASK, CPUCLOCK_SCHED,
    NTP_PHASE_LIMIT, NTP_TIME_CONSTANT, NTP_TOLERANCE, STA_UNSYNC, TIME_ERROR, USER_TICK_USEC,
    kernel_timex,
};
use crate::process::{Channel, Pid, Restart, Tid};
use crate::{Context, Errno, Kernel};

/// The CPU time the caller's process has used: the time its threads have run, as charged when each
/// came back, and that of its threads that have ended.
pub(super) fn cpu_time(kernel: &Kernel, _: &mut Context<'_>, tid: Tid) -> u64 {
    process_cpu_time(kernel, kernel.threads[&tid].process)
}

/// The CPU time a process has used, one that has ended included.
pub(super) fn process_cpu_time(kernel: &Kernel, pid: Pid) -> u64 {
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
        if id & CPUCLOCK_PERTHREAD_MASK != 0 {
            let thread = match !(id >> 3) {
                0 => tid,
                other => other,
            };
            return Ok(kernel.threads.get(&thread).map_or(0, |found| found.runtime));
        }
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
        CLOCK_PROCESS_CPUTIME_ID => cpu_time(kernel, cx, tid),
        CLOCK_THREAD_CPUTIME_ID => kernel.threads[&tid].runtime,
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
    wall: bool,
) -> Outcome {
    let thread = kernel.thread_mut(tid);
    if !thread.call.started {
        thread.call.started = true;
        thread.call.progress = until;
        thread.call.deadline = Some(until.saturating_add(thread.timer_slack));
        thread.call.wall = wall;
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

/// The CPU clock a sleep is on, as `posix_cpu_timer_create` finds it; none when the id names none.
fn cpu_sleep_clock(kernel: &Kernel, tid: Tid, id: i32) -> Option<super::timer::Clock> {
    use super::timer::Clock;
    if id == CLOCK_PROCESS_CPUTIME_ID as i32 {
        return Some(Clock::Process(kernel.threads[&tid].process));
    }
    let pid = cpu_clock_target(kernel, tid, id, false)?;
    Some(if id & CPUCLOCK_PERTHREAD_MASK != 0 {
        Clock::Thread(!(id >> 3))
    } else {
        Clock::Process(pid)
    })
}

/// A sleep until a CPU clock reaches `until`, as Linux's `do_cpu_nanosleep`: a timer on the clock
/// wakes the thread, so only time used by other threads ends it. A signal interrupts it: when `left`
/// is given (a relative sleep or its restart) what is left is written there (an address of 0 takes
/// nothing) and the sleep continues from `restart_syscall`; an absolute one simply runs again.
fn cpu_sleep(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    clock: i32,
    until: u64,
    asked: u64,
    left: Option<u64>,
) -> Outcome {
    let call = kernel.threads[&tid].call;
    let timer = if call.started {
        call.progress
    } else {
        let Some(on) = cpu_sleep_clock(kernel, tid, clock) else {
            return Err(Errno::EINVAL).into();
        };
        // A time of zero disarms the timer and a time already passed fires it at once.
        if until == 0 || super::timer::cpu_clock_now(kernel, on).is_some_and(|now| now >= until) {
            return Outcome::Return(0);
        }
        let id = super::timer::start_cpu_sleep(kernel, tid, on, until, asked);
        let call = &mut kernel.thread_mut(tid).call;
        call.started = true;
        call.progress = id;
        call.deadline = None;
        id
    };
    let remaining = super::timer::cpu_sleep_left(kernel, timer);
    if remaining == 0 {
        super::timer::end_cpu_sleep(kernel, timer);
        return Outcome::Return(0);
    }
    if kernel.signal_pending(tid) {
        let until = super::timer::cpu_sleep_until(kernel, timer).unwrap_or(until);
        super::timer::end_cpu_sleep(kernel, timer);
        let Some(left) = left else {
            return Err(Errno::ERESTARTNOHAND).into();
        };
        if left != 0 {
            let remaining = timespec_bytes(remaining);
            if let Err(error) = guest(kernel, cx, tid).write_value(left, &remaining) {
                return Err(error).into();
            }
        }
        kernel.thread_mut(tid).restart = Some(Restart::CpuSleep { clock, until, left });
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
            let outcome = sleep_until(kernel, cx, tid, until, Some(left), false);
            if matches!(outcome, Outcome::Return(_)) {
                kernel.thread_mut(tid).restart = None;
            }
            outcome
        }
        Some(Restart::CpuSleep { clock, until, left }) => {
            let outcome = cpu_sleep(kernel, cx, tid, clock, until, until, Some(left));
            if matches!(outcome, Outcome::Return(_)) {
                kernel.thread_mut(tid).restart = None;
            }
            outcome
        }
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
        return sleep_until(kernel, cx, tid, until, Some(left), false);
    }
    match read_duration(kernel, cx, tid, request) {
        Ok(duration) => {
            let until = cx.monotonic().saturating_add(nanoseconds(&duration));
            sleep_until(kernel, cx, tid, until, Some(left), false)
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
            // Its clock and times are in its timer.
            return cpu_sleep(
                kernel,
                cx,
                tid,
                id as i32,
                0,
                0,
                (!absolute).then_some(left),
            );
        }
        return sleep_until(
            kernel,
            cx,
            tid,
            call.progress,
            (!absolute).then_some(left),
            call.wall,
        );
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
        if !kernel.credentials(tid).has(CAP_WAKE_ALARM) {
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
    if id == CLOCK_PROCESS_CPUTIME_ID || signed < 0 {
        let until = if absolute {
            duration
        } else {
            match cpu_sleep_clock(kernel, tid, signed)
                .and_then(|on| super::timer::cpu_clock_now(kernel, on))
            {
                // A relative time of zero disarms the timer.
                Some(now) if duration != 0 => now.saturating_add(duration),
                _ => 0,
            }
        };
        return cpu_sleep(
            kernel,
            cx,
            tid,
            signed,
            until,
            duration,
            (!absolute).then_some(left),
        );
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
    sleep_until(
        kernel,
        cx,
        tid,
        until,
        (!absolute).then_some(left),
        absolute && realtime,
    )
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

/// `do_adjtimex` on this machine's clock, which nothing in the container can adjust: setting
/// anything needs CAP_SYS_TIME, which no container process can hold, and reading gives a clock
/// never synchronised (the reference's own state, measured). Answers the clock's state.
fn adjust(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    timex: &mut kernel_timex,
) -> Result<u64, Errno> {
    let modes = timex.modes;
    let may_set = kernel.credentials(tid).has(CAP_SYS_TIME);
    if modes & ADJ_ADJTIME != 0 {
        if modes & ADJ_OFFSET_SINGLESHOT == 0 {
            return Err(Errno::EINVAL);
        }
        if modes & ADJ_OFFSET_READONLY == 0 && !may_set {
            return Err(Errno::EPERM);
        }
    } else {
        if modes != 0 && !may_set {
            return Err(Errno::EPERM);
        }
        if modes & ADJ_TICK != 0 && !(9000..=11000).contains(&timex.tick) {
            return Err(Errno::EINVAL);
        }
    }
    if modes & ADJ_SETOFFSET != 0 && !may_set {
        return Err(Errno::EPERM);
    }
    let read_only = modes & ADJ_ADJTIME != 0 && modes & ADJ_OFFSET_READONLY != 0;
    if modes != 0 && !read_only {
        return Err(kernel.counters.refuse(
            cx.host,
            crate::reason::FLAG_NOT_BUILT,
            (u64::from(__NR_adjtimex) << 32) | u64::from(modes),
            Errno::EPERM,
        ));
    }
    let now = cx.host.realtime_nanoseconds();
    *timex = kernel_timex {
        modes,
        offset: 0,
        freq: 0,
        maxerror: NTP_PHASE_LIMIT,
        esterror: NTP_PHASE_LIMIT,
        status: STA_UNSYNC,
        constant: NTP_TIME_CONSTANT,
        precision: 1,
        tolerance: NTP_TOLERANCE,
        time_sec: (now / 1_000_000_000) as i64,
        time_usec: (now % 1_000_000_000 / 1000) as i64,
        tick: USER_TICK_USEC,
        ppsfreq: 0,
        jitter: 0,
        shift: 0,
        stabil: 0,
        jitcnt: 0,
        calcnt: 0,
        errcnt: 0,
        stbcnt: 0,
        tai: 0,
        ..*timex
    };
    Ok(TIME_ERROR)
}

pub(super) fn adjtimex(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, ..]: Args,
) -> Result<u64, Errno> {
    let mut timex: kernel_timex = guest(kernel, cx, tid).read_value(address)?;
    let result = adjust(kernel, cx, tid, &mut timex);
    // The structure goes back whatever the answer.
    guest(kernel, cx, tid).write_value(address, &timex)?;
    result
}

pub(super) fn clock_adjtime(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, address, ..]: Args,
) -> Result<u64, Errno> {
    let mut timex: kernel_timex = guest(kernel, cx, tid).read_value(address)?;
    // `do_clock_adjtime`: only the wall clock adjusts; no clock device is built, and the CPU clocks
    // and the others have no `clock_adj`.
    let id = id as i32;
    match id {
        _ if id < 0 && is_clock_device(id) => return Err(Errno::EINVAL),
        _ if id < 0 => return Err(Errno::EOPNOTSUPP),
        0 => {}
        1..=9 | 11 => return Err(Errno::EOPNOTSUPP),
        _ => return Err(Errno::EINVAL),
    }
    let result = adjust(kernel, cx, tid, &mut timex)?;
    guest(kernel, cx, tid).write_value(address, &timex)?;
    Ok(result)
}
