//! Clocks and sleeping.

use super::{Args, Outcome, guest};
use crate::linux::general::*;
use crate::process::{Parked, Tid};
use crate::{Context, Errno, Kernel};

/// The CPU time a process has used. One guest thread runs at a time and the container's first process
/// is the only one, so this is the time since the container started.
pub(super) fn cpu_time(kernel: &Kernel, cx: &mut Context<'_>) -> u64 {
    cx.host
        .monotonic_nanoseconds()
        .saturating_sub(kernel.boot_monotonic)
}

/// The value of a clock in nanoseconds.
fn clock(kernel: &Kernel, cx: &mut Context<'_>, clock: u64) -> Result<u64, Errno> {
    Ok(match clock as u32 {
        CLOCK_REALTIME | CLOCK_REALTIME_COARSE | CLOCK_REALTIME_ALARM | CLOCK_TAI => {
            cx.host.realtime_nanoseconds()
        }
        CLOCK_MONOTONIC
        | CLOCK_MONOTONIC_RAW
        | CLOCK_MONOTONIC_COARSE
        | CLOCK_BOOTTIME
        | CLOCK_BOOTTIME_ALARM => cx.host.monotonic_nanoseconds(),
        CLOCK_PROCESS_CPUTIME_ID | CLOCK_THREAD_CPUTIME_ID => cpu_time(kernel, cx),
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
    let now = clock(kernel, cx, id)?;
    guest(kernel, cx, tid).write_value(out, &timespec_bytes(now))?;
    Ok(0)
}

pub(super) fn clock_getres(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, out, ..]: Args,
) -> Result<u64, Errno> {
    clock(kernel, cx, id)?;
    // The coarse clocks tick with the scheduler; the others are as fine as the host's.
    let resolution = match id as u32 {
        CLOCK_REALTIME_COARSE | CLOCK_MONOTONIC_COARSE => 4_000_000,
        _ => 1,
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
fn sleep_until(kernel: &mut Kernel, tid: Tid, until: u64) -> Outcome {
    kernel.thread_mut(tid).parked = Some(Parked::Until { nanoseconds: until });
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
            sleep_until(kernel, tid, until)
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
    let id = id as u32;
    let absolute = flags as u32 & TIMER_ABSTIME != 0;
    let realtime = match id {
        CLOCK_REALTIME | CLOCK_TAI => true,
        CLOCK_MONOTONIC | CLOCK_BOOTTIME => false,
        CLOCK_THREAD_CPUTIME_ID => return Err(Errno::EINVAL).into(),
        CLOCK_PROCESS_CPUTIME_ID => false,
        _ => return Err(Errno::EOPNOTSUPP).into(),
    };
    let duration = match read_duration(kernel, cx, tid, request) {
        Ok(duration) => nanoseconds(&duration),
        Err(error) => return Err(error).into(),
    };
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
    sleep_until(kernel, tid, until)
}

pub(super) fn times(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [out, ..]: Args,
) -> Result<u64, Errno> {
    let ticks = |nanoseconds: u64| nanoseconds / 10_000_000;
    let used = ticks(cpu_time(kernel, cx));
    if out != 0 {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&used.to_le_bytes());
        guest(kernel, cx, tid).write(out, &bytes)?;
    }
    Ok(ticks(cx.host.monotonic_nanoseconds()))
}
