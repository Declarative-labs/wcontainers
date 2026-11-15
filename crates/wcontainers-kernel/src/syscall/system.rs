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

pub(super) fn sched_getscheduler(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    _: Tid,
    [pid, ..]: Args,
) -> Result<u64, Errno> {
    if (pid as i32) < 0 {
        return Err(Errno::EINVAL);
    }
    if pid != 0 && !kernel.threads.contains_key(&(pid as i32)) {
        return Err(Errno::ESRCH);
    }
    Ok(u64::from(SCHED_NORMAL))
}

pub(super) fn sched_getparam(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, out, ..]: Args,
) -> Result<u64, Errno> {
    if (pid as i32) < 0 || out == 0 {
        return Err(Errno::EINVAL);
    }
    if pid != 0 && !kernel.threads.contains_key(&(pid as i32)) {
        return Err(Errno::ESRCH);
    }
    guest(kernel, cx, tid).write(out, &0i32.to_le_bytes())?;
    Ok(0)
}

pub(super) fn getpriority(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [which, who, ..]: Args,
) -> Result<u64, Errno> {
    let who = who as u32;
    let process = kernel.process_of(tid);
    // Some process must match: the caller's own number, group, or effective user.
    let found = match which as u32 {
        PRIO_PROCESS => who == 0 || who as i32 == process.pid,
        PRIO_PGRP => who == 0 || who as i32 == process.group,
        PRIO_USER => {
            who == 0
                || kernel
                    .processes
                    .values()
                    .any(|other| other.credentials.uid == who)
        }
        _ => return Err(Errno::EINVAL),
    };
    if !found {
        return Err(Errno::ESRCH);
    }
    // Linux returns 20 minus the nice value, so that the result is never negative.
    Ok(20)
}
