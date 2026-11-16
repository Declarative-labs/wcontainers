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
use crate::signal::UNBLOCKABLE;
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
        kernel.end_thread(cx, tid);
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

/// Reads a null-terminated list of string pointers, as `execve`'s `count` does.
fn read_pointers(guest: &mut crate::user::Guest<'_>, mut address: u64) -> Result<Vec<u64>, Errno> {
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
        out.push(pointer);
        address += 8;
    }
}

/// Reads the strings a list points to, each taking its room, as `copy_strings` does.
fn read_strings(
    guest: &mut crate::user::Guest<'_>,
    pointers: &[u64],
    room: &mut crate::loader::ArgumentRoom,
) -> Result<Vec<Vec<u8>>, Errno> {
    let mut out = Vec::with_capacity(pointers.len());
    for pointer in pointers {
        let string = guest
            .read_string(*pointer, MAX_ARG_STRLEN)
            .map_err(|error| {
                if error == Errno::ENAMETOOLONG {
                    Errno::E2BIG
                } else {
                    error
                }
            })?;
        room.take(&string)?;
        out.push(string);
    }
    Ok(out)
}

pub(super) fn execve(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [path, argv, envp, ..]: Args,
) -> Outcome {
    match exec(kernel, cx, tid, AT_FDCWD as u64, path, argv, envp, 0) {
        Ok(Exec::Started) => Outcome::Restart,
        Ok(Exec::Died) => Outcome::Gone,
        Ok(Exec::Checked) => Outcome::Return(0),
        Err(error) => Err(error).into(),
    }
}

pub(super) fn execveat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, argv, envp, flags, _]: Args,
) -> Outcome {
    match exec(kernel, cx, tid, dirfd, path, argv, envp, flags as u32) {
        Ok(Exec::Started) => Outcome::Restart,
        Ok(Exec::Died) => Outcome::Gone,
        Ok(Exec::Checked) => Outcome::Return(0),
        Err(error) => Err(error).into(),
    }
}

enum Exec {
    /// The new program is in place.
    Started,
    /// `AT_EXECVE_CHECK`: the file could be run; nothing changed.
    Checked,
    /// It failed after the point of no return, and the process died.
    Died,
}

/// Linux's `do_execveat_common`.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn exec(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    dirfd: u64,
    path: u64,
    argv: u64,
    envp: u64,
    flags: u32,
) -> Result<Exec, Errno> {
    // In the reference's order: the flags, the path, the file opened, then the argument and
    // environment lists.
    if flags & !(AT_SYMLINK_NOFOLLOW | AT_EMPTY_PATH | AT_EXECVE_CHECK) != 0 {
        return Err(Errno::EINVAL);
    }
    let path = if flags & AT_EMPTY_PATH != 0 {
        guest(kernel, cx, tid).read_path_maybe_empty(path)?
    } else {
        guest(kernel, cx, tid).read_path(path)?
    };
    let pid = kernel.threads[&tid].process;
    let dirfd = dirfd as i32;
    // A path from a descriptor is named through `/dev/fd`, as Linux's `alloc_bprm` names it; a script
    // cannot be run through one closed at exec, which its interpreter could not open.
    let from_descriptor = dirfd != AT_FDCWD && !path.starts_with(b"/");
    let (file_name, reachable) = if from_descriptor {
        let mut name = alloc::format!("/dev/fd/{dirfd}").into_bytes();
        if !path.is_empty() {
            name.push(b'/');
            name.extend_from_slice(&path);
        }
        let descriptor = kernel.processes[&pid].files.get(dirfd)?;
        (name, !descriptor.close_on_exec)
    } else {
        (path.clone(), true)
    };
    let program = if path.is_empty() && dirfd == AT_FDCWD {
        let cwd = kernel.processes[&pid].cwd.clone();
        crate::loader::program_of(kernel, cx.cpu, pid, cwd, Vec::new())?
    } else if path.is_empty() {
        let file = kernel.processes[&pid].files.file(dirfd)?;
        let (node, full) = {
            let file = file.borrow();
            (file.node.clone(), file.path.clone())
        };
        crate::loader::program_of(kernel, cx.cpu, pid, node, full)?
    } else {
        let at = super::files::start(kernel, tid, dirfd as u64, &path)?;
        crate::loader::open_program_at(
            kernel,
            cx.cpu,
            pid,
            &at,
            &path,
            flags & AT_SYMLINK_NOFOLLOW == 0,
        )?
    };
    // In the order of Linux's `do_execveat_common`: both lists counted, the room they have, then the
    // file name, the environment and the arguments copied.
    let stack_limit = kernel.processes[&pid].limits[RLIMIT_STACK as usize].current;
    let mut guest = guest(kernel, cx, tid);
    let argument_pointers = read_pointers(&mut guest, argv)?;
    let environment_pointers = read_pointers(&mut guest, envp)?;
    let mut room = crate::loader::ArgumentRoom::new(
        stack_limit,
        argument_pointers.len(),
        environment_pointers.len(),
    )?;
    room.take(&file_name)?;
    let environment = read_strings(&mut guest, &environment_pointers, &mut room)?;
    let mut arguments = read_strings(&mut guest, &argument_pointers, &mut room)?;
    room.first_argument(&mut arguments)?;
    if flags & AT_EXECVE_CHECK != 0 {
        return Ok(Exec::Checked);
    }
    if kernel.processes[&pid].threads.len() > 1 {
        kernel
            .counters
            .count(cx.host, reason::NOT_IMPLEMENTED, u64::from(__NR_execve));
        return Err(Errno::ENOSYS);
    }
    let mut program = program;
    program.reopenable = reachable;
    let loaded = crate::loader::load(
        kernel,
        cx,
        pid,
        &file_name,
        program,
        arguments,
        environment,
        room,
    );
    let mut image = match loaded {
        Ok(image) => image,
        Err(crate::loader::Failed::Refused(error)) => return Err(error),
        Err(crate::loader::Failed::Fatal) => {
            kernel.die_of(cx, pid, SIGSEGV);
            return Ok(Exec::Died);
        }
    };
    // Run from a descriptor, the process is named after the file run (a script's interpreter), not
    // `/dev/fd/<n>`.
    if from_descriptor {
        let path = &image.executable.1;
        image.name = path
            .rsplit(|byte| *byte == b'/')
            .next()
            .unwrap_or_default()
            .to_vec();
    }
    kernel.install_image(cx, pid, tid, image);
    Ok(Exec::Started)
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
    // In Linux's order. There is one process, so any other is missing.
    let process = kernel.process_of_mut(tid);
    let pid = if pid as i32 == 0 {
        process.pid
    } else {
        pid as i32
    };
    let group = if group as i32 == 0 { pid } else { group as i32 };
    if group < 0 {
        return Err(Errno::EINVAL);
    }
    if pid != process.pid {
        return Err(Errno::ESRCH);
    }
    if process.session == process.pid {
        return Err(Errno::EPERM);
    }
    // Joining a group needs a group of that number in the same session: only the process's own.
    if group != pid && group != process.group {
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
    changing(kernel, tid, false, |credentials| {
        let wanted = [ruid as u32, euid as u32, suid as u32];
        let current = [credentials.uid, credentials.euid, credentials.suid];
        // Linux returns at once when nothing changes, leaving even the file system id alone.
        if unchanged(wanted, current, credentials.fsuid) {
            return Ok(0);
        }
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
    })
}

pub(super) fn setresgid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [rgid, egid, sgid, ..]: Args,
) -> Result<u64, Errno> {
    changing(kernel, tid, false, |credentials| {
        let wanted = [rgid as u32, egid as u32, sgid as u32];
        let current = [credentials.gid, credentials.egid, credentials.sgid];
        if unchanged(wanted, current, credentials.fsgid) {
            return Ok(0);
        }
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
    })
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
    changing(kernel, tid, true, |credentials| {
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
    })
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
    changing(kernel, tid, true, |credentials| {
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
    })
}

pub(super) fn setreuid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [ruid, euid, ..]: Args,
) -> Result<u64, Errno> {
    let (ruid, euid) = (ruid as u32, euid as u32);
    changing(kernel, tid, true, |credentials| {
        let privileged = credentials.has(CAP_SETUID);
        let (old_uid, old_euid, old_suid) = (credentials.uid, credentials.euid, credentials.suid);
        if ruid != u32::MAX && !privileged && ruid != old_uid && ruid != old_euid {
            return Err(Errno::EPERM);
        }
        if euid != u32::MAX
            && !privileged
            && euid != old_uid
            && euid != old_euid
            && euid != old_suid
        {
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
    })
}

pub(super) fn setregid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [rgid, egid, ..]: Args,
) -> Result<u64, Errno> {
    let (rgid, egid) = (rgid as u32, egid as u32);
    changing(kernel, tid, true, |credentials| {
        let privileged = credentials.has(CAP_SETGID);
        let (old_gid, old_egid, old_sgid) = (credentials.gid, credentials.egid, credentials.sgid);
        if rgid != u32::MAX && !privileged && rgid != old_gid && rgid != old_egid {
            return Err(Errno::EPERM);
        }
        if egid != u32::MAX
            && !privileged
            && egid != old_gid
            && egid != old_egid
            && egid != old_sgid
        {
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
    })
}

pub(super) fn setfsuid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [uid, ..]: Args,
) -> Result<u64, Errno> {
    let uid = uid as u32;
    changing(kernel, tid, false, |credentials| {
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
    })
}

pub(super) fn setfsgid(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [gid, ..]: Args,
) -> Result<u64, Errno> {
    let gid = gid as u32;
    changing(kernel, tid, false, |credentials| {
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
    })
}

/// Changes the caller's credentials; a change Linux's `commit_creds` counts makes it undumpable.
/// The no-op test of Linux's `__sys_setresuid` and `__sys_setresgid`: every id asked for is -1 or
/// already the current one, and a new effective id also equals the file system id.
fn unchanged(wanted: [u32; 3], current: [u32; 3], fs_id: u32) -> bool {
    wanted
        .iter()
        .zip(current)
        .all(|(wanted, current)| *wanted == u32::MAX || *wanted == current)
        && (wanted[1] == u32::MAX || wanted[1] == fs_id)
}

/// The change is committed (new credentials) whenever it succeeds if `always`, otherwise only when it
/// changed something.
fn changing(
    kernel: &mut Kernel,
    tid: Tid,
    always: bool,
    change: impl FnOnce(&mut crate::credentials::Credentials) -> Result<u64, Errno>,
) -> Result<u64, Errno> {
    let process = kernel.process_of_mut(tid);
    let old = process.credentials.clone();
    let result = change(&mut process.credentials);
    let loses = process.credentials.loses_dumpable(&old);
    if loses {
        process.dumpable = false;
    }
    let commit = result.is_ok() && (always || process.credentials != old);
    let pid = process.pid;
    if loses {
        kernel.thread_mut(tid).parent_death_signal = 0;
    }
    if commit {
        kernel.commit_credentials(pid);
    }
    result
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
    if !kernel.process_of(tid).credentials.has(CAP_SETGID) {
        return Err(Errno::EPERM);
    }
    if size as u32 as u64 > MAX_GROUPS {
        return Err(Errno::EINVAL);
    }
    let size = size as u32 as u64;
    let bytes = guest(kernel, cx, tid).read(list, size * 4)?;
    let mut groups: Vec<u32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .collect();
    // -1 is no group.
    if groups.contains(&u32::MAX) {
        return Err(Errno::EINVAL);
    }
    groups.sort_unstable();
    changing(kernel, tid, true, |credentials| {
        credentials.groups = groups;
        Ok(0)
    })
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
    let (list, _) = kernel.threads.get(&target).ok_or(Errno::ESRCH)?.robust_list;
    // The length is always the head's size, and is written first, as on Linux.
    let mut guest = guest(kernel, cx, tid);
    guest.write(length, &ROBUST_LIST_HEAD_SIZE.to_le_bytes())?;
    guest.write(head, &list.to_le_bytes())?;
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

pub(super) fn rt_sigaction(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [signal, action, old, size, ..]: Args,
) -> Result<u64, Errno> {
    // In Linux's order: the size, the new action read, `do_sigaction`'s checks and change, and only
    // then the old action written back, so a bad old pointer still leaves the new action set.
    signal_set_size(size)?;
    let new: Option<kernel_sigaction> = if action != 0 {
        Some(guest(kernel, cx, tid).read_value(action)?)
    } else {
        None
    };
    let signal = signal as i32;
    if !(1..=SIGNALS as i32).contains(&signal)
        || new.is_some() && (signal as u32 == SIGKILL || signal as u32 == SIGSTOP)
    {
        return Err(Errno::EINVAL);
    }
    let index = signal as usize - 1;
    let previous = kernel.process_of(tid).actions[index];
    if let Some(mut new) = new {
        new.sa_mask.sig[0] &= !UNBLOCKABLE;
        // Linux keeps only the flags it knows (`UAPI_SA_FLAGS`), so programs can tell which it has.
        new.sa_flags &= u64::from(
            SA_NOCLDSTOP
                | SA_NOCLDWAIT
                | SA_SIGINFO
                | SA_ONSTACK
                | SA_RESTART
                | SA_NODEFER
                | SA_RESETHAND
                | SA_EXPOSE_TAGBITS
                | SA_RESTORER,
        );
        kernel.process_of_mut(tid).actions[index] = new;
        if crate::signal::action_ignores(new.sa_handler_kernel, signal as u32) {
            let pid = kernel.threads[&tid].process;
            kernel.flush_pending(pid, signal as u32);
        }
    }
    if old != 0 {
        guest(kernel, cx, tid).write_value(old, &previous)?;
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
    // The new mask is set before the old one is written back, as on Linux.
    if let Some(new) = new {
        kernel.check_unblocking(cx, tid, new & !UNBLOCKABLE)?;
        kernel.thread_mut(tid).signal_mask = new & !UNBLOCKABLE;
    }
    if old != 0 {
        guest(kernel, cx, tid).write(old, &current.to_le_bytes())?;
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
    let thread = &kernel.threads[&tid];
    let pending = thread.pending & thread.signal_mask;
    guest(kernel, cx, tid).write(set, &pending.to_le_bytes()[..size as usize])?;
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
    // `stack_t` has padding, so its fields are read and placed by their offsets.
    use core::mem::offset_of;
    type Stack = crate::linux::general::sigaltstack;
    let current = kernel.threads[&tid].alternate_stack;
    // Linux's `on_sig_stack`: whether the caller runs on its alternate stack now, unless the stack is
    // given up on entry to a handler (`SS_AUTODISARM`).
    let mut registers = wcontainers_abi::RegisterBlock::ZERO;
    cx.cpu.registers(kernel.threads[&tid].cpu, &mut registers);
    let on_stack = current.2 & SS_AUTODISARM as i32 == 0
        && registers.sp > current.0
        && registers.sp - current.0 <= current.1;
    let replacement = if new != 0 {
        let bytes = guest(kernel, cx, tid).read(new, size_of::<Stack>() as u64)?;
        if on_stack {
            return Err(Errno::EPERM);
        }
        let field = |offset: usize, length: usize| &bytes[offset..offset + length];
        let base = u64::from_le_bytes(
            field(offset_of!(Stack, ss_sp), 8)
                .try_into()
                .expect("8 bytes"),
        );
        let flags = i32::from_le_bytes(
            field(offset_of!(Stack, ss_flags), 4)
                .try_into()
                .expect("4 bytes"),
        );
        let size = u64::from_le_bytes(
            field(offset_of!(Stack, ss_size), 8)
                .try_into()
                .expect("8 bytes"),
        );
        let mode = flags & !(SS_AUTODISARM as i32);
        if mode != SS_DISABLE as i32 && mode != SS_ONSTACK as i32 && mode != 0 {
            return Err(Errno::EINVAL);
        }
        if (base, size, flags) == current {
            // The same stack again changes nothing, whatever its size.
            None
        } else if mode == SS_DISABLE as i32 {
            Some((0, 0, flags))
        } else {
            if size < MIN_SIGNAL_STACK {
                return Err(Errno::ENOMEM);
            }
            Some((base, size, flags))
        }
    } else {
        None
    };
    // The new stack is set even when the old one cannot be written back, as on Linux.
    if let Some(replacement) = replacement {
        kernel.thread_mut(tid).alternate_stack = replacement;
    }
    if old != 0 {
        let mut bytes = alloc::vec![0u8; size_of::<Stack>()];
        let mut put = |offset: usize, value: &[u8]| {
            bytes[offset..offset + value.len()].copy_from_slice(value);
        };
        put(offset_of!(Stack, ss_sp), &current.0.to_le_bytes());
        let state = if current.1 == 0 {
            SS_DISABLE as i32
        } else if on_stack {
            SS_ONSTACK as i32
        } else {
            0
        };
        let flags = state | (current.2 & SS_AUTODISARM as i32);
        put(offset_of!(Stack, ss_flags), &flags.to_le_bytes());
        put(offset_of!(Stack, ss_size), &current.1.to_le_bytes());
        guest(kernel, cx, tid).write(old, &bytes)?;
    }
    Ok(0)
}

/// A signal sent to the caller's own process, once it is found: an unknown signal is refused only
/// then, as Linux's `check_kill_permission` does.
fn send_to_self(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    signal: u64,
) -> Result<u64, Errno> {
    let signal = signal as u32;
    if signal as usize > SIGNALS {
        return Err(Errno::EINVAL);
    }
    kernel.send_to_self(cx, tid, signal).map(|()| 0)
}

/// There is one process, so any other target is missing.
pub(super) fn kill(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, signal, ..]: Args,
) -> Result<u64, Errno> {
    let pid = pid as i32;
    let process = kernel.process_of(tid);
    // -1 is every process but the first and the caller: none here.
    if pid == process.pid || pid == 0 || (pid < -1 && pid.unsigned_abs() as i32 == process.group) {
        return send_to_self(kernel, cx, tid, signal);
    }
    Err(Errno::ESRCH)
}

pub(super) fn tgkill(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [group, target, signal, ..]: Args,
) -> Result<u64, Errno> {
    if (group as i32) <= 0 || (target as i32) <= 0 {
        return Err(Errno::EINVAL);
    }
    if target as i32 == tid && group as i32 == kernel.threads[&tid].process {
        return send_to_self(kernel, cx, tid, signal);
    }
    Err(Errno::ESRCH)
}

pub(super) fn tkill(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [target, signal, ..]: Args,
) -> Result<u64, Errno> {
    if (target as i32) <= 0 {
        return Err(Errno::EINVAL);
    }
    if target as i32 == tid {
        return send_to_self(kernel, cx, tid, signal);
    }
    Err(Errno::ESRCH)
}

pub(super) fn prlimit64(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, resource, new, old, ..]: Args,
) -> Result<u64, Errno> {
    // In Linux's order: read the new value, find the process, then `do_prlimit`'s checks.
    let wanted: Option<rlimit64> = if new != 0 {
        Some(guest(kernel, cx, tid).read_value(new)?)
    } else {
        None
    };
    own_pid(kernel, tid, pid).map_err(|_| Errno::ESRCH)?;
    if resource as usize >= LIMITS {
        return Err(Errno::EINVAL);
    }
    let resource = resource as usize;
    let current = kernel.process_of(tid).limits[resource];
    if let Some(wanted) = &wanted {
        if wanted.rlim_cur > wanted.rlim_max {
            return Err(Errno::EINVAL);
        }
        if resource == RLIMIT_NOFILE as usize && wanted.rlim_max > 1 << 20 {
            return Err(Errno::EPERM);
        }
        // Raising a hard limit needs `CAP_SYS_RESOURCE`, which a default container does not have.
        if wanted.rlim_max > current.maximum {
            return Err(Errno::EPERM);
        }
        kernel.process_of_mut(tid).limits[resource] = Limit {
            current: wanted.rlim_cur,
            maximum: wanted.rlim_max,
        };
    }
    if old != 0 {
        let value = rlimit64 {
            rlim_cur: current.current,
            rlim_max: current.maximum,
        };
        guest(kernel, cx, tid).write_value(old, &value)?;
    }
    Ok(0)
}

pub(super) fn getrlimit(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [resource, old, ..]: Args,
) -> Result<u64, Errno> {
    if resource as usize >= LIMITS {
        return Err(Errno::EINVAL);
    }
    let limit = kernel.process_of(tid).limits[resource as usize];
    let value = rlimit64 {
        rlim_cur: limit.current,
        rlim_max: limit.maximum,
    };
    guest(kernel, cx, tid).write_value(old, &value)?;
    Ok(0)
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
    [option, second, third, fourth, fifth, _]: Args,
) -> Result<u64, Errno> {
    let rest_zero = third == 0 && fourth == 0 && fifth == 0;
    match option as u32 {
        PR_SET_NAME => {
            // `strncpy_from_user` of at most 15 bytes: a fault before the end or a zero is EFAULT.
            let mut name = Vec::new();
            let mut guest = guest(kernel, cx, tid);
            for index in 0..15 {
                let byte = guest.read(second + index, 1)?[0];
                if byte == 0 {
                    break;
                }
                name.push(byte);
            }
            kernel.process_of_mut(tid).set_name_exact(&name);
            Ok(0)
        }
        PR_GET_NAME => {
            let mut name = kernel.process_of(tid).name.clone();
            name.resize(16, 0);
            guest(kernel, cx, tid).write(second, &name)?;
            Ok(0)
        }
        PR_GET_DUMPABLE => Ok(kernel.process_of(tid).dumpable.into()),
        PR_SET_DUMPABLE => {
            if second > 1 {
                return Err(Errno::EINVAL);
            }
            kernel.process_of_mut(tid).dumpable = second == 1;
            Ok(0)
        }
        PR_GET_KEEPCAPS => Ok(kernel.process_of(tid).credentials.keep_capabilities.into()),
        PR_SET_KEEPCAPS => {
            if second > 1 {
                return Err(Errno::EINVAL);
            }
            changing(kernel, tid, true, |credentials| {
                credentials.keep_capabilities = second == 1;
                Ok(0)
            })
        }
        PR_CAPBSET_READ => {
            if second > u64::from(CAP_LAST_CAP) {
                return Err(Errno::EINVAL);
            }
            Ok((kernel.process_of(tid).credentials.bounding >> second) & 1)
        }
        // `cap_task_prctl`: clearing commits new credentials whatever was there; raising needs the
        // power permitted and inheritable (no secure bit can forbid it here).
        PR_CAP_AMBIENT if second == u64::from(PR_CAP_AMBIENT_CLEAR_ALL) => {
            if !rest_zero {
                return Err(Errno::EINVAL);
            }
            changing(kernel, tid, true, |credentials| {
                credentials.ambient = 0;
                Ok(0)
            })
        }
        PR_CAP_AMBIENT => {
            if third > u64::from(CAP_LAST_CAP) || fourth != 0 || fifth != 0 {
                return Err(Errno::EINVAL);
            }
            let bit = 1u64 << third;
            match second as u32 {
                PR_CAP_AMBIENT_IS_SET => Ok(u64::from(
                    kernel.process_of(tid).credentials.ambient & bit != 0,
                )),
                PR_CAP_AMBIENT_RAISE | PR_CAP_AMBIENT_LOWER => {
                    let raise = second as u32 == PR_CAP_AMBIENT_RAISE;
                    let credentials = &kernel.process_of(tid).credentials;
                    if raise
                        && (credentials.permitted & bit == 0 || credentials.inheritable & bit == 0)
                    {
                        return Err(Errno::EPERM);
                    }
                    changing(kernel, tid, true, |credentials| {
                        if raise {
                            credentials.ambient |= bit;
                        } else {
                            credentials.ambient &= !bit;
                        }
                        Ok(0)
                    })
                }
                _ => Err(Errno::EINVAL),
            }
        }
        PR_CAPBSET_DROP => {
            let credentials = &mut kernel.process_of_mut(tid).credentials;
            if !credentials.has(CAP_SETPCAP) {
                return Err(Errno::EPERM);
            }
            if second > u64::from(CAP_LAST_CAP) {
                return Err(Errno::EINVAL);
            }
            credentials.bounding &= !(1 << second);
            let pid = kernel.threads[&tid].process;
            kernel.commit_credentials(pid);
            Ok(0)
        }
        PR_GET_NO_NEW_PRIVS => {
            if second != 0 || !rest_zero {
                return Err(Errno::EINVAL);
            }
            Ok(kernel.process_of(tid).credentials.no_new_privileges.into())
        }
        PR_SET_NO_NEW_PRIVS => {
            if second != 1 || !rest_zero {
                return Err(Errno::EINVAL);
            }
            kernel.process_of_mut(tid).credentials.no_new_privileges = true;
            Ok(0)
        }
        // Docker's profile puts every process in filter mode.
        PR_GET_SECCOMP => Ok(crate::linux::ptrace::SECCOMP_MODE_FILTER.into()),
        PR_SET_SECCOMP => super::seccomp::prctl_set(kernel, cx, tid, second, third),
        PR_SET_CHILD_SUBREAPER => {
            kernel.process_of_mut(tid).child_subreaper = second != 0;
            Ok(0)
        }
        PR_GET_CHILD_SUBREAPER => {
            let value = i32::from(kernel.process_of(tid).child_subreaper);
            guest(kernel, cx, tid).write(second, &value.to_le_bytes())?;
            Ok(0)
        }
        PR_SET_PDEATHSIG => {
            if second > SIGNALS as u64 {
                return Err(Errno::EINVAL);
            }
            kernel.thread_mut(tid).parent_death_signal = second as i32;
            Ok(0)
        }
        PR_GET_PDEATHSIG => {
            let signal = kernel.threads[&tid].parent_death_signal;
            guest(kernel, cx, tid).write(second, &signal.to_le_bytes())?;
            Ok(0)
        }
        PR_GET_TIMERSLACK => Ok(kernel.threads[&tid].timer_slack),
        PR_SET_TIMERSLACK => {
            // Real-time threads keep theirs; zero restores the one the thread was born with.
            let thread = kernel.thread_mut(tid);
            if !matches!(
                thread.schedule.policy,
                SCHED_FIFO | SCHED_RR | SCHED_DEADLINE
            ) {
                thread.timer_slack = if second == 0 {
                    crate::process::TIMER_SLACK
                } else {
                    second
                };
            }
            Ok(0)
        }
        PR_GET_THP_DISABLE => {
            if second != 0 || !rest_zero {
                return Err(Errno::EINVAL);
            }
            Ok(kernel.process_of(tid).thp_disable)
        }
        PR_SET_THP_DISABLE => {
            use crate::linux_extra::PR_THP_DISABLE_EXCEPT_ADVISED;
            if fourth != 0 || fifth != 0 {
                return Err(Errno::EINVAL);
            }
            if (second == 0 && third != 0) || third & !PR_THP_DISABLE_EXCEPT_ADVISED != 0 {
                return Err(Errno::EINVAL);
            }
            kernel.process_of_mut(tid).thp_disable = if second == 0 { 0 } else { 1 | third };
            Ok(0)
        }
        // No memory deny-write-execute policy can be set (below), so none is in force.
        PR_GET_MDWE => {
            if second != 0 || !rest_zero {
                return Err(Errno::EINVAL);
            }
            Ok(0)
        }
        PR_GET_AUXV => {
            if fourth != 0 || fifth != 0 {
                return Err(Errno::EINVAL);
            }
            // `saved_auxv`: the vector as the program started with it, then zeros.
            let mut saved = kernel.process_of(tid).starting_stack.auxiliary.clone();
            saved.resize(crate::linux_extra::AT_VECTOR_SIZE * 8, 0);
            let size = (saved.len() as u64).min(third);
            if size > 0 {
                guest(kernel, cx, tid).write(second, &saved[..size as usize])?;
            }
            Ok(saved.len() as u64)
        }
        PR_GET_SECUREBITS => Ok(if kernel.process_of(tid).credentials.keep_capabilities {
            crate::linux_extra::SECBIT_KEEP_CAPS
        } else {
            0
        }),
        PR_SET_MM => {
            // `prctl_set_mm`: the whole-map forms first, then every other form needs CAP_SYS_RESOURCE.
            let form = second as u32;
            if fifth != 0
                || (fourth != 0
                    && !matches!(form, PR_SET_MM_AUXV | PR_SET_MM_MAP | PR_SET_MM_MAP_SIZE))
            {
                return Err(Errno::EINVAL);
            }
            if form == PR_SET_MM_MAP_SIZE {
                let size = size_of::<prctl_mm_map>() as u32;
                guest(kernel, cx, tid).write(third, &size.to_le_bytes())?;
                return Ok(0);
            }
            if form != PR_SET_MM_MAP && !kernel.process_of(tid).credentials.has(CAP_SYS_RESOURCE) {
                return Err(Errno::EPERM);
            }
            kernel
                .counters
                .count(cx.host, reason::PRCTL_NOT_BUILT, option);
            Err(Errno::EINVAL)
        }
        PR_SET_IO_FLUSHER | PR_GET_IO_FLUSHER
            if !kernel.process_of(tid).credentials.has(CAP_SYS_RESOURCE) =>
        {
            Err(Errno::EPERM)
        }
        _ => {
            kernel
                .counters
                .count(cx.host, reason::PRCTL_NOT_BUILT, option);
            Err(Errno::EINVAL)
        }
    }
}

/// Linux's `cap_validate_magic`: the header, and how many 32 bit words of each set its version moves.
/// An unknown version is replaced by the one Linux prefers, and is EINVAL.
fn capability_version(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    header: u64,
) -> Result<(__user_cap_header_struct, usize), Errno> {
    let head: __user_cap_header_struct = guest(kernel, cx, tid).read_value(header)?;
    match head.version {
        _LINUX_CAPABILITY_VERSION_1 => Ok((head, 1)),
        _LINUX_CAPABILITY_VERSION_2 | _LINUX_CAPABILITY_VERSION_3 => Ok((head, 2)),
        _ => {
            let mut fixed = head;
            fixed.version = _LINUX_CAPABILITY_VERSION_3;
            guest(kernel, cx, tid).write_value(header, &fixed)?;
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
    let (head, words) = match capability_version(kernel, cx, tid, header) {
        Err(Errno::EINVAL) if data == 0 => return Ok(0),
        result => result?,
    };
    if data == 0 {
        return Ok(0);
    }
    if head.pid < 0 {
        return Err(Errno::EINVAL);
    }
    own_pid(kernel, tid, head.pid as u64)?;
    let credentials = kernel.process_of(tid).credentials.clone();
    let half = |set: u64, upper: bool| {
        if upper {
            (set >> 32) as u32
        } else {
            set as u32
        }
    };
    let mut out = Vec::new();
    for upper in [false, true].into_iter().take(words) {
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
    let (head, words) = capability_version(kernel, cx, tid, header)?;
    if head.pid != 0 && head.pid != kernel.threads[&tid].process {
        return Err(Errno::EPERM);
    }
    let bytes = guest(kernel, cx, tid).read(data, words as u64 * 12)?;
    // The upper halves are zero for the one-word version.
    let word = |index: usize| {
        bytes.get(index * 4..index * 4 + 4).map_or(0, |four| {
            u64::from(u32::from_le_bytes(four.try_into().expect("4 bytes")))
        })
    };
    let effective = word(0) | word(3) << 32;
    let permitted = word(1) | word(4) << 32;
    let inheritable = word(2) | word(5) << 32;
    changing(kernel, tid, true, |credentials| {
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
    })
}

pub(super) fn personality(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [persona, ..]: Args,
) -> Result<u64, Errno> {
    let persona = persona as u32;
    // 32 bit programs do not run here, as on an ARM64 CPU without that mode.
    if persona != 0xffff_ffff && persona & 0xff == 8 {
        return Err(Errno::EINVAL);
    }
    let process = kernel.process_of_mut(tid);
    let old = process.personality;
    if persona != 0xffff_ffff {
        process.personality = persona;
    }
    Ok(old.into())
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
        let used = super::time::cpu_time(kernel, cx, tid);
        out.ru_utime = __kernel_old_timeval {
            tv_sec: (used / 1_000_000_000) as i64,
            tv_usec: (used % 1_000_000_000 / 1000) as i64,
        };
        // Which pages a program has touched is not known, so every mapped page counts as resident,
        // as mincore reports them.
        let mapped: u64 = kernel
            .process_of(tid)
            .map
            .iter()
            .map(|mapping| mapping.end - mapping.start)
            .sum();
        out.ru_maxrss = (mapped / 1024) as i64;
        kernel
            .counters
            .count(cx.host, crate::counters::reason::RESIDENCY_NOT_TRACKED, 0);
    }
    guest(kernel, cx, tid).write_value(usage, &out)?;
    Ok(0)
}
