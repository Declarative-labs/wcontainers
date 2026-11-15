//! Calls about the calling process and thread: ending them, running a new program, ids, users,
//! signal settings, limits and the process's name.

use alloc::vec::Vec;

use zerocopy::FromZeros;

use super::{Args, Outcome, guest};
use crate::counters::reason;
use crate::credentials::{CAP_SETGID, CAP_SETUID};
use crate::linux::general::*;
use crate::linux::prctl::*;
use crate::process::{LIMITS, Limit, SIGNALS, Tid};
use crate::{Context, Errno, Kernel};

/// `exit(status)`: ends the calling thread, and its process when it is the last thread.
pub(super) fn exit(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    arguments: Args,
) -> Outcome {
    let status = arguments[0] as i32;
    let pid = kernel.threads[&tid].process;
    if kernel.processes[&pid].threads.len() == 1 {
        kernel.end_process(cx, pid, exit_status(status));
    } else {
        let thread = kernel.threads.remove(&tid).expect("thread exists");
        cx.cpu.thread_drop(thread.cpu);
        let process = kernel.processes.get_mut(&pid).expect("process exists");
        process.threads.retain(|other| *other != tid);
    }
    Outcome::Gone
}

/// `exit_group(status)`: ends every thread of the calling process.
pub(super) fn exit_group(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    arguments: Args,
) -> Outcome {
    let pid = kernel.threads[&tid].process;
    kernel.end_process(cx, pid, exit_status(arguments[0] as i32));
    Outcome::Gone
}

/// The wait status of a process that exited with `status`: its low byte, shifted, as Linux reports it.
fn exit_status(status: i32) -> i32 {
    (status & 0xff) << 8
}

/// The most strings `execve` takes in each list (`MAX_ARG_STRINGS`).
const MAX_ARG_STRINGS: usize = 0x7fff_ffff;

/// The longest one argument or variable may be (`MAX_ARG_STRLEN`).
const MAX_ARG_STRLEN: u64 = 32 * 4096;

/// Reads a null-terminated list of string pointers, as `execve` does.
fn read_strings(
    guest: &mut crate::user::Guest<'_>,
    mut address: u64,
) -> Result<Vec<Vec<u8>>, Errno> {
    let mut out = Vec::new();
    if address == 0 {
        return Ok(out);
    }
    loop {
        let pointer: u64 = guest.read_value(address)?;
        if pointer == 0 {
            return Ok(out);
        }
        if out.len() >= MAX_ARG_STRINGS {
            return Err(Errno::E2BIG);
        }
        let string = guest
            .read_string(pointer, MAX_ARG_STRLEN)
            .map_err(|error| {
                if error == Errno::ENAMETOOLONG {
                    Errno::E2BIG
                } else {
                    error
                }
            })?;
        out.push(string);
        address += 8;
    }
}

pub(super) fn execve(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [path, argv, envp, ..]: Args,
) -> Outcome {
    match exec(kernel, cx, tid, path, argv, envp) {
        Ok(()) => Outcome::Restart,
        Err(error) => Err(error).into(),
    }
}

fn exec(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    path: u64,
    argv: u64,
    envp: u64,
) -> Result<(), Errno> {
    let mut guest = guest(kernel, cx, tid);
    let path = guest.read_path(path)?;
    let arguments = read_strings(&mut guest, argv)?;
    let environment = read_strings(&mut guest, envp)?;
    let pid = kernel.threads[&tid].process;
    if kernel.processes[&pid].threads.len() > 1 {
        kernel
            .counters
            .count(cx.host, reason::NOT_IMPLEMENTED, u64::from(__NR_execve));
        return Err(Errno::ENOSYS);
    }
    let image = crate::loader::load(kernel, cx, pid, &path, arguments, environment)?;
    kernel.install_image(cx, pid, tid, image);
    Ok(())
}

pub(super) fn getpid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Result<u64, Errno> {
    Ok(kernel.threads[&tid].process as u64)
}

pub(super) fn getppid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Result<u64, Errno> {
    Ok(kernel.process_of(tid).parent as u64)
}

pub(super) fn gettid(_: &mut Kernel, _: &mut Context<'_>, tid: Tid, _: Args) -> Result<u64, Errno> {
    Ok(tid as u64)
}

/// The process a `pid` argument names: 0 or the caller's own number. Other processes come with fork.
fn own_pid(kernel: &Kernel, tid: Tid, pid: u64) -> Result<(), Errno> {
    let pid = pid as i32;
    if pid < 0 {
        return Err(Errno::EINVAL);
    }
    if pid == 0 || pid == kernel.threads[&tid].process {
        Ok(())
    } else {
        Err(Errno::ESRCH)
    }
}

pub(super) fn getpgid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [pid, ..]: Args,
) -> Result<u64, Errno> {
    if pid as i32 != 0 && !kernel.processes.contains_key(&(pid as i32)) {
        return Err(Errno::ESRCH);
    }
    let pid = if pid == 0 {
        kernel.threads[&tid].process
    } else {
        pid as i32
    };
    Ok(kernel.processes[&pid].group as u64)
}

pub(super) fn getsid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [pid, ..]: Args,
) -> Result<u64, Errno> {
    if pid as i32 != 0 && !kernel.processes.contains_key(&(pid as i32)) {
        return Err(Errno::ESRCH);
    }
    let pid = if pid == 0 {
        kernel.threads[&tid].process
    } else {
        pid as i32
    };
    Ok(kernel.processes[&pid].session as u64)
}

pub(super) fn setpgid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [pid, group, ..]: Args,
) -> Result<u64, Errno> {
    if (group as i32) < 0 {
        return Err(Errno::EINVAL);
    }
    own_pid(kernel, tid, pid)?;
    let process = kernel.process_of_mut(tid);
    let group = if group == 0 {
        process.pid
    } else {
        group as i32
    };
    if process.session == process.pid {
        return Err(Errno::EPERM);
    }
    if group != process.pid {
        return Err(Errno::EPERM);
    }
    process.group = group;
    Ok(0)
}

pub(super) fn setsid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Result<u64, Errno> {
    let process = kernel.process_of_mut(tid);
    if process.group == process.pid {
        return Err(Errno::EPERM);
    }
    process.session = process.pid;
    process.group = process.pid;
    Ok(process.pid as u64)
}

pub(super) fn getuid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Result<u64, Errno> {
    Ok(kernel.process_of(tid).credentials.uid.into())
}

pub(super) fn geteuid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Result<u64, Errno> {
    Ok(kernel.process_of(tid).credentials.euid.into())
}

pub(super) fn getgid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Result<u64, Errno> {
    Ok(kernel.process_of(tid).credentials.gid.into())
}

pub(super) fn getegid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Result<u64, Errno> {
    Ok(kernel.process_of(tid).credentials.egid.into())
}

pub(super) fn getresuid(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [ruid, euid, suid, ..]: Args,
) -> Result<u64, Errno> {
    let credentials = kernel.process_of(tid).credentials.clone();
    let mut guest = guest(kernel, cx, tid);
    guest.write(ruid, &credentials.uid.to_le_bytes())?;
    guest.write(euid, &credentials.euid.to_le_bytes())?;
    guest.write(suid, &credentials.suid.to_le_bytes())?;
    Ok(0)
}

pub(super) fn getresgid(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [rgid, egid, sgid, ..]: Args,
) -> Result<u64, Errno> {
    let credentials = kernel.process_of(tid).credentials.clone();
    let mut guest = guest(kernel, cx, tid);
    guest.write(rgid, &credentials.gid.to_le_bytes())?;
    guest.write(egid, &credentials.egid.to_le_bytes())?;
    guest.write(sgid, &credentials.sgid.to_le_bytes())?;
    Ok(0)
}

/// `setresuid`, with Linux's rule: without `CAP_SETUID` each new id must be one of the three current
/// ones. `u32::MAX` leaves an id as it is.
pub(super) fn setresuid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [ruid, euid, suid, ..]: Args,
) -> Result<u64, Errno> {
    let credentials = &mut kernel.process_of_mut(tid).credentials;
    let wanted = [ruid as u32, euid as u32, suid as u32];
    let current = [credentials.uid, credentials.euid, credentials.suid];
    if !credentials.has(CAP_SETUID)
        && wanted
            .iter()
            .any(|id| *id != u32::MAX && !current.contains(id))
    {
        return Err(Errno::EPERM);
    }
    let (old_uid, old_euid, old_suid) = (credentials.uid, credentials.euid, credentials.suid);
    if wanted[0] != u32::MAX {
        credentials.uid = wanted[0];
    }
    if wanted[1] != u32::MAX {
        credentials.euid = wanted[1];
    }
    if wanted[2] != u32::MAX {
        credentials.suid = wanted[2];
    }
    credentials.after_uid_change(old_uid, old_euid, old_suid);
    Ok(0)
}

pub(super) fn setresgid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [rgid, egid, sgid, ..]: Args,
) -> Result<u64, Errno> {
    let credentials = &mut kernel.process_of_mut(tid).credentials;
    let wanted = [rgid as u32, egid as u32, sgid as u32];
    let current = [credentials.gid, credentials.egid, credentials.sgid];
    if !credentials.has(CAP_SETGID)
        && wanted
            .iter()
            .any(|id| *id != u32::MAX && !current.contains(id))
    {
        return Err(Errno::EPERM);
    }
    if wanted[0] != u32::MAX {
        credentials.gid = wanted[0];
    }
    if wanted[1] != u32::MAX {
        credentials.egid = wanted[1];
    }
    if wanted[2] != u32::MAX {
        credentials.sgid = wanted[2];
    }
    credentials.fsgid = credentials.egid;
    Ok(0)
}

pub(super) fn setuid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [uid, ..]: Args,
) -> Result<u64, Errno> {
    let uid = uid as u32;
    if uid == u32::MAX {
        return Err(Errno::EINVAL);
    }
    let credentials = &mut kernel.process_of_mut(tid).credentials;
    let (old_uid, old_euid, old_suid) = (credentials.uid, credentials.euid, credentials.suid);
    if credentials.has(CAP_SETUID) {
        credentials.uid = uid;
        credentials.euid = uid;
        credentials.suid = uid;
    } else if uid == credentials.uid || uid == credentials.suid {
        credentials.euid = uid;
    } else {
        return Err(Errno::EPERM);
    }
    credentials.after_uid_change(old_uid, old_euid, old_suid);
    Ok(0)
}

pub(super) fn setgid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [gid, ..]: Args,
) -> Result<u64, Errno> {
    let gid = gid as u32;
    if gid == u32::MAX {
        return Err(Errno::EINVAL);
    }
    let credentials = &mut kernel.process_of_mut(tid).credentials;
    if credentials.has(CAP_SETGID) {
        credentials.gid = gid;
        credentials.egid = gid;
        credentials.sgid = gid;
    } else if gid == credentials.gid || gid == credentials.sgid {
        credentials.egid = gid;
    } else {
        return Err(Errno::EPERM);
    }
    credentials.fsgid = credentials.egid;
    Ok(0)
}

pub(super) fn setreuid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [ruid, euid, ..]: Args,
) -> Result<u64, Errno> {
    let (ruid, euid) = (ruid as u32, euid as u32);
    let credentials = &mut kernel.process_of_mut(tid).credentials;
    let privileged = credentials.has(CAP_SETUID);
    let (old_uid, old_euid, old_suid) = (credentials.uid, credentials.euid, credentials.suid);
    if ruid != u32::MAX && !privileged && ruid != old_uid && ruid != old_euid {
        return Err(Errno::EPERM);
    }
    if euid != u32::MAX && !privileged && euid != old_uid && euid != old_euid && euid != old_suid {
        return Err(Errno::EPERM);
    }
    if ruid != u32::MAX {
        credentials.uid = ruid;
    }
    if euid != u32::MAX {
        credentials.euid = euid;
    }
    if ruid != u32::MAX || (euid != u32::MAX && euid != old_uid) {
        credentials.suid = credentials.euid;
    }
    credentials.after_uid_change(old_uid, old_euid, old_suid);
    Ok(0)
}

pub(super) fn setregid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [rgid, egid, ..]: Args,
) -> Result<u64, Errno> {
    let (rgid, egid) = (rgid as u32, egid as u32);
    let credentials = &mut kernel.process_of_mut(tid).credentials;
    let privileged = credentials.has(CAP_SETGID);
    let (old_gid, old_egid, old_sgid) = (credentials.gid, credentials.egid, credentials.sgid);
    if rgid != u32::MAX && !privileged && rgid != old_gid && rgid != old_egid {
        return Err(Errno::EPERM);
    }
    if egid != u32::MAX && !privileged && egid != old_gid && egid != old_egid && egid != old_sgid {
        return Err(Errno::EPERM);
    }
    if rgid != u32::MAX {
        credentials.gid = rgid;
    }
    if egid != u32::MAX {
        credentials.egid = egid;
    }
    if rgid != u32::MAX || (egid != u32::MAX && egid != old_gid) {
        credentials.sgid = credentials.egid;
    }
    credentials.fsgid = credentials.egid;
    Ok(0)
}

pub(super) fn setfsuid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [uid, ..]: Args,
) -> Result<u64, Errno> {
    let uid = uid as u32;
    let credentials = &mut kernel.process_of_mut(tid).credentials;
    let old = credentials.fsuid;
    let allowed = [
        credentials.uid,
        credentials.euid,
        credentials.suid,
        credentials.fsuid,
    ]
    .contains(&uid)
        || credentials.has(CAP_SETUID);
    if uid != u32::MAX && allowed {
        credentials.fsuid = uid;
        credentials.after_fsuid_change(old);
    }
    Ok(old.into())
}

pub(super) fn setfsgid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [gid, ..]: Args,
) -> Result<u64, Errno> {
    let gid = gid as u32;
    let credentials = &mut kernel.process_of_mut(tid).credentials;
    let old = credentials.fsgid;
    let allowed = [
        credentials.gid,
        credentials.egid,
        credentials.sgid,
        credentials.fsgid,
    ]
    .contains(&gid)
        || credentials.has(CAP_SETGID);
    if gid != u32::MAX && allowed {
        credentials.fsgid = gid;
    }
    Ok(old.into())
}

/// Linux's limit on supplementary groups (`NGROUPS_MAX`).
const MAX_GROUPS: u64 = 65536;

pub(super) fn getgroups(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [size, list, ..]: Args,
) -> Result<u64, Errno> {
    let size = size as i32;
    if size < 0 {
        return Err(Errno::EINVAL);
    }
    let groups = kernel.process_of(tid).credentials.groups.clone();
    if size == 0 {
        return Ok(groups.len() as u64);
    }
    if (size as usize) < groups.len() {
        return Err(Errno::EINVAL);
    }
    let bytes: Vec<u8> = groups
        .iter()
        .flat_map(|group| group.to_le_bytes())
        .collect();
    guest(kernel, cx, tid).write(list, &bytes)?;
    Ok(groups.len() as u64)
}

pub(super) fn setgroups(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [size, list, ..]: Args,
) -> Result<u64, Errno> {
    if size > MAX_GROUPS {
        return Err(Errno::EINVAL);
    }
    if !kernel.process_of(tid).credentials.has(CAP_SETGID) {
        return Err(Errno::EPERM);
    }
    let bytes = guest(kernel, cx, tid).read(list, size * 4)?;
    let mut groups: Vec<u32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .collect();
    groups.sort_unstable();
    kernel.process_of_mut(tid).credentials.groups = groups;
    Ok(0)
}

pub(super) fn set_tid_address(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [address, ..]: Args,
) -> Result<u64, Errno> {
    kernel.thread_mut(tid).clear_child_tid = address;
    Ok(tid as u64)
}

/// The size of Linux's `robust_list_head`.
const ROBUST_LIST_HEAD_SIZE: u64 = 24;

pub(super) fn set_robust_list(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [head, length, ..]: Args,
) -> Result<u64, Errno> {
    if length != ROBUST_LIST_HEAD_SIZE {
        return Err(Errno::EINVAL);
    }
    kernel.thread_mut(tid).robust_list = (head, length);
    Ok(0)
}

pub(super) fn get_robust_list(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, head, length, ..]: Args,
) -> Result<u64, Errno> {
    let target = if pid == 0 { tid } else { pid as i32 };
    let (list, size) = kernel.threads.get(&target).ok_or(Errno::ESRCH)?.robust_list;
    let mut guest = guest(kernel, cx, tid);
    guest.write(head, &list.to_le_bytes())?;
    guest.write(length, &size.to_le_bytes())?;
    Ok(0)
}

/// Checks a signal set size argument, which must be Linux's 8 bytes.
fn signal_set_size(size: u64) -> Result<(), Errno> {
    if size != 8 {
        Err(Errno::EINVAL)
    } else {
        Ok(())
    }
}

const UNBLOCKABLE: u64 = (1 << (SIGKILL - 1)) | (1 << (SIGSTOP - 1));

pub(super) fn rt_sigaction(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [signal, action, old, size, ..]: Args,
) -> Result<u64, Errno> {
    signal_set_size(size)?;
    let signal = signal as i32;
    if !(1..=SIGNALS as i32).contains(&signal) {
        return Err(Errno::EINVAL);
    }
    let index = signal as usize - 1;
    let new: Option<kernel_sigaction> = if action != 0 {
        if signal as u32 == SIGKILL || signal as u32 == SIGSTOP {
            return Err(Errno::EINVAL);
        }
        Some(guest(kernel, cx, tid).read_value(action)?)
    } else {
        None
    };
    let previous = kernel.process_of(tid).actions[index];
    if old != 0 {
        guest(kernel, cx, tid).write_value(old, &previous)?;
    }
    if let Some(mut new) = new {
        new.sa_mask.sig[0] &= !UNBLOCKABLE;
        kernel.process_of_mut(tid).actions[index] = new;
    }
    Ok(0)
}

pub(super) fn rt_sigprocmask(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [how, set, old, size, ..]: Args,
) -> Result<u64, Errno> {
    signal_set_size(size)?;
    let current = kernel.threads[&tid].signal_mask;
    let new = if set != 0 {
        let set: u64 = guest(kernel, cx, tid).read_value(set)?;
        Some(match how as u32 {
            SIG_BLOCK => current | set,
            SIG_UNBLOCK => current & !set,
            SIG_SETMASK => set,
            _ => return Err(Errno::EINVAL),
        })
    } else {
        None
    };
    if old != 0 {
        guest(kernel, cx, tid).write(old, &current.to_le_bytes())?;
    }
    if let Some(new) = new {
        kernel.thread_mut(tid).signal_mask = new & !UNBLOCKABLE;
    }
    Ok(0)
}

pub(super) fn rt_sigpending(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [set, size, ..]: Args,
) -> Result<u64, Errno> {
    if size > 8 {
        return Err(Errno::EINVAL);
    }
    guest(kernel, cx, tid).write(set, &0u64.to_le_bytes()[..size as usize])?;
    Ok(0)
}

/// The smallest alternate signal stack ARM64 Linux accepts (`MINSIGSTKSZ`).
const MIN_SIGNAL_STACK: u64 = 5120;

pub(super) fn sigaltstack(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [new, old, ..]: Args,
) -> Result<u64, Errno> {
    let current = kernel.threads[&tid].alternate_stack;
    let replacement = if new != 0 {
        let bytes = guest(kernel, cx, tid).read(new, 24)?;
        let base = u64::from_le_bytes(bytes[0..8].try_into().expect("8 bytes"));
        let flags = i32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes"));
        let size = u64::from_le_bytes(bytes[16..24].try_into().expect("8 bytes"));
        let mode = flags & !(SS_AUTODISARM as i32);
        if mode != SS_DISABLE as i32 && mode != SS_ONSTACK as i32 && mode != 0 {
            return Err(Errno::EINVAL);
        }
        if mode == SS_DISABLE as i32 {
            Some((0, 0, flags))
        } else {
            if size < MIN_SIGNAL_STACK {
                return Err(Errno::ENOMEM);
            }
            Some((base, size, flags & SS_AUTODISARM as i32))
        }
    } else {
        None
    };
    if old != 0 {
        let mut bytes = Vec::with_capacity(24);
        bytes.extend_from_slice(&current.0.to_le_bytes());
        let flags = if current.1 == 0 {
            SS_DISABLE as i32
        } else {
            current.2
        };
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&current.1.to_le_bytes());
        guest(kernel, cx, tid).write(old, &bytes)?;
    }
    if let Some(replacement) = replacement {
        kernel.thread_mut(tid).alternate_stack = replacement;
    }
    Ok(0)
}

/// Signals whose default action is to do nothing.
fn ignored_by_default(signal: u32) -> bool {
    matches!(signal, SIGCHLD | SIGURG | SIGWINCH | SIGCONT)
}

/// A signal sent to the caller itself. With fork and signal delivery still to come, only the outcomes
/// that need no handler are exact: ignoring it, or dying of it.
fn signal_self(kernel: &mut Kernel, cx: &mut Context<'_>, tid: Tid, signal: u32) -> Outcome {
    if signal == 0 {
        return Outcome::Return(0);
    }
    let action = kernel.process_of(tid).actions[signal as usize - 1];
    let handler = action.sa_handler_kernel;
    let blocked = kernel.threads[&tid].signal_mask & (1 << (signal - 1)) != 0;
    if handler == crate::linux_extra::SIG_IGN
        || (handler == crate::linux_extra::SIG_DFL && ignored_by_default(signal))
    {
        return Outcome::Return(0);
    }
    if handler != crate::linux_extra::SIG_DFL
        || blocked
        || matches!(signal, SIGSTOP | SIGTSTP | SIGTTIN | SIGTTOU)
    {
        kernel
            .counters
            .count(cx.host, reason::SIGNAL_HANDLER_NOT_RUN, signal.into());
        return Err(Errno::ENOSYS).into();
    }
    let pid = kernel.threads[&tid].process;
    kernel.end_process(cx, pid, signal as i32);
    Outcome::Gone
}

pub(super) fn kill(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, signal, ..]: Args,
) -> Outcome {
    let signal = signal as u32;
    if signal as usize > SIGNALS {
        return Err(Errno::EINVAL).into();
    }
    let pid = pid as i32;
    let own = kernel.threads[&tid].process;
    if pid == own || pid == 0 || pid == -1 || pid == -own {
        return signal_self(kernel, cx, tid, signal);
    }
    Err(Errno::ESRCH).into()
}

pub(super) fn tgkill(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [group, target, signal, ..]: Args,
) -> Outcome {
    let signal = signal as u32;
    if (group as i32) <= 0 || (target as i32) <= 0 || signal as usize > SIGNALS {
        return Err(Errno::EINVAL).into();
    }
    if target as i32 == tid && group as i32 == kernel.threads[&tid].process {
        return signal_self(kernel, cx, tid, signal);
    }
    Err(Errno::ESRCH).into()
}

pub(super) fn tkill(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [target, signal, ..]: Args,
) -> Outcome {
    let signal = signal as u32;
    if (target as i32) <= 0 || signal as usize > SIGNALS {
        return Err(Errno::EINVAL).into();
    }
    if target as i32 == tid {
        return signal_self(kernel, cx, tid, signal);
    }
    Err(Errno::ESRCH).into()
}

pub(super) fn prlimit64(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, resource, new, old, ..]: Args,
) -> Result<u64, Errno> {
    if resource as usize >= LIMITS {
        return Err(Errno::EINVAL);
    }
    own_pid(kernel, tid, pid).map_err(|error| {
        if error == Errno::EINVAL {
            Errno::ESRCH
        } else {
            error
        }
    })?;
    let resource = resource as usize;
    let wanted: Option<rlimit64> = if new != 0 {
        Some(guest(kernel, cx, tid).read_value(new)?)
    } else {
        None
    };
    let current = kernel.process_of(tid).limits[resource];
    if let Some(wanted) = &wanted {
        if wanted.rlim_cur > wanted.rlim_max {
            return Err(Errno::EINVAL);
        }
        // Raising a hard limit needs `CAP_SYS_RESOURCE`, which a default container does not have.
        if wanted.rlim_max > current.maximum {
            return Err(Errno::EPERM);
        }
        if resource == RLIMIT_NOFILE as usize && wanted.rlim_max > 1 << 20 {
            return Err(Errno::EPERM);
        }
    }
    if old != 0 {
        let value = rlimit64 {
            rlim_cur: current.current,
            rlim_max: current.maximum,
        };
        guest(kernel, cx, tid).write_value(old, &value)?;
    }
    if let Some(wanted) = wanted {
        kernel.process_of_mut(tid).limits[resource] = Limit {
            current: wanted.rlim_cur,
            maximum: wanted.rlim_max,
        };
    }
    Ok(0)
}

pub(super) fn getrlimit(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [resource, old, ..]: Args,
) -> Result<u64, Errno> {
    prlimit64(kernel, cx, tid, [0, resource, 0, old, 0, 0])
}

pub(super) fn setrlimit(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [resource, new, ..]: Args,
) -> Result<u64, Errno> {
    if new == 0 {
        return Err(Errno::EFAULT);
    }
    prlimit64(kernel, cx, tid, [0, resource, new, 0, 0, 0])
}

pub(super) fn prctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [option, second, ..]: Args,
) -> Result<u64, Errno> {
    match option as u32 {
        PR_SET_NAME => {
            let name = guest(kernel, cx, tid).read(second, 16).or_else(|_| {
                guest(kernel, cx, tid)
                    .read_string(second, 16)
                    .map(|mut name| {
                        name.push(0);
                        name
                    })
            })?;
            kernel.process_of_mut(tid).set_name_exact(&name);
            Ok(0)
        }
        PR_GET_NAME => {
            let mut name = kernel.process_of(tid).name.clone();
            name.resize(16, 0);
            guest(kernel, cx, tid).write(second, &name)?;
            Ok(0)
        }
        PR_GET_DUMPABLE => Ok(1),
        PR_SET_DUMPABLE => {
            if second > 1 {
                return Err(Errno::EINVAL);
            }
            Ok(0)
        }
        PR_GET_KEEPCAPS => Ok(kernel.process_of(tid).credentials.keep_capabilities.into()),
        PR_SET_KEEPCAPS => {
            if second > 1 {
                return Err(Errno::EINVAL);
            }
            kernel.process_of_mut(tid).credentials.keep_capabilities = second == 1;
            Ok(0)
        }
        PR_CAPBSET_READ => {
            if second > u64::from(CAP_LAST_CAP) {
                return Err(Errno::EINVAL);
            }
            Ok((kernel.process_of(tid).credentials.bounding >> second) & 1)
        }
        PR_GET_SECCOMP | PR_GET_NO_NEW_PRIVS | PR_GET_CHILD_SUBREAPER => {
            if option as u32 == PR_GET_CHILD_SUBREAPER {
                guest(kernel, cx, tid).write(second, &0i32.to_le_bytes())?;
            }
            Ok(0)
        }
        _ => {
            kernel
                .counters
                .count(cx.host, reason::PRCTL_UNKNOWN, option);
            Err(Errno::EINVAL)
        }
    }
}

pub(super) fn capget(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [header, data, ..]: Args,
) -> Result<u64, Errno> {
    let head: __user_cap_header_struct = guest(kernel, cx, tid).read_value(header)?;
    if head.version != _LINUX_CAPABILITY_VERSION_3 && head.version != _LINUX_CAPABILITY_VERSION_2 {
        let mut fixed = head;
        fixed.version = _LINUX_CAPABILITY_VERSION_3;
        guest(kernel, cx, tid).write_value(header, &fixed)?;
        return if data == 0 { Ok(0) } else { Err(Errno::EINVAL) };
    }
    if head.pid < 0 {
        return Err(Errno::EINVAL);
    }
    own_pid(kernel, tid, head.pid as u64)?;
    if data == 0 {
        return Ok(0);
    }
    let credentials = kernel.process_of(tid).credentials.clone();
    let half = |set: u64, upper: bool| {
        if upper {
            (set >> 32) as u32
        } else {
            set as u32
        }
    };
    let mut out = Vec::new();
    for upper in [false, true] {
        out.extend_from_slice(half(credentials.effective, upper).to_le_bytes().as_slice());
        out.extend_from_slice(half(credentials.permitted, upper).to_le_bytes().as_slice());
        out.extend_from_slice(
            half(credentials.inheritable, upper)
                .to_le_bytes()
                .as_slice(),
        );
    }
    guest(kernel, cx, tid).write(data, &out)?;
    Ok(0)
}

pub(super) fn capset(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [header, data, ..]: Args,
) -> Result<u64, Errno> {
    let head: __user_cap_header_struct = guest(kernel, cx, tid).read_value(header)?;
    if head.version != _LINUX_CAPABILITY_VERSION_3 && head.version != _LINUX_CAPABILITY_VERSION_2 {
        let mut fixed = head;
        fixed.version = _LINUX_CAPABILITY_VERSION_3;
        guest(kernel, cx, tid).write_value(header, &fixed)?;
        return Err(Errno::EINVAL);
    }
    if head.pid != 0 && head.pid != kernel.threads[&tid].process {
        return Err(Errno::EPERM);
    }
    let bytes = guest(kernel, cx, tid).read(data, 24)?;
    let word = |index: usize| {
        u64::from(u32::from_le_bytes(
            bytes[index * 4..index * 4 + 4].try_into().expect("4 bytes"),
        ))
    };
    let effective = word(0) | word(3) << 32;
    let permitted = word(1) | word(4) << 32;
    let inheritable = word(2) | word(5) << 32;
    let credentials = &mut kernel.process_of_mut(tid).credentials;
    let can_set_pcap = credentials.has(crate::linux::general::CAP_SETPCAP);
    // Linux's `cap_capset` rules.
    if !can_set_pcap && inheritable & !(credentials.inheritable | credentials.permitted) != 0 {
        return Err(Errno::EPERM);
    }
    if inheritable & !(credentials.inheritable | credentials.bounding) != 0 {
        return Err(Errno::EPERM);
    }
    if permitted & !credentials.permitted != 0 || effective & !permitted != 0 {
        return Err(Errno::EPERM);
    }
    credentials.effective = effective;
    credentials.permitted = permitted;
    credentials.inheritable = inheritable;
    credentials.ambient &= permitted & inheritable;
    Ok(0)
}

pub(super) fn personality(
    _: &mut Kernel,
    cx: &mut Context<'_>,
    _: Tid,
    [persona, ..]: Args,
) -> Result<u64, Errno> {
    // Only the plain Linux personality exists here.
    if persona as u32 != 0xffff_ffff && persona as u32 != 0 {
        let _ = cx;
        return Err(Errno::EINVAL);
    }
    Ok(0)
}

pub(super) fn getrusage(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [who, usage, ..]: Args,
) -> Result<u64, Errno> {
    let who = who as i32;
    if who != RUSAGE_SELF as i32 && who != RUSAGE_CHILDREN && who != RUSAGE_THREAD as i32 {
        return Err(Errno::EINVAL);
    }
    let mut out = rusage::new_zeroed();
    if who != RUSAGE_CHILDREN {
        let used = super::time::cpu_time(kernel, cx);
        out.ru_utime = __kernel_old_timeval {
            tv_sec: (used / 1_000_000_000) as i64,
            tv_usec: (used % 1_000_000_000 / 1000) as i64,
        };
        out.ru_maxrss = 1;
    }
    guest(kernel, cx, tid).write_value(usage, &out)?;
    Ok(0)
}
