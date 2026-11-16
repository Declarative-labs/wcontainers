//! Process descriptors, as Linux's pidfs gives them: files that name a process for as long as the
//! descriptor lives, `pidfd_open`, `pidfd_send_signal` and `process_mrelease`; `waitid` and `clone`
//! take and make them through `named` and `make`.

use alloc::rc::Rc;

use super::Args;
use crate::file::{FileRef, OpenFile};
use crate::fs::{Content, Kind, New, Node, new_inode};
use crate::linux::general::*;
use crate::linux_extra::{
    PIDFD_SIGNAL_PROCESS_GROUP, PIDFD_SIGNAL_THREAD, PIDFD_SIGNAL_THREAD_GROUP, PIDFD_THREAD,
};
use crate::process::{Channel, Pid, Tid};
use crate::{Context, Errno, Kernel};

/// What `/proc/<pid>/fd` shows for a descriptor (pidfs keeps the anonymous inode's name).
const LINK: &[u8] = b"anon_inode:[pidfd]";

/// The process a descriptor was made for, by number, and whether it still exists (running, or ended
/// and not yet reaped). A reaped process's number may name another one later, which `exists` tells
/// apart.
#[derive(Clone, Copy)]
pub(super) struct Named {
    pub pid: Pid,
    pub exists: bool,
}

/// What a descriptor names; EBADF for a file that is not one (`pidfd_pid`).
pub(super) fn named(kernel: &Kernel, file: &OpenFile) -> Result<Named, Errno> {
    let Content::Pidfd(pid, _) = file.node.borrow().content else {
        return Err(Errno::EBADF);
    };
    let exists = kernel
        .processes
        .get(&pid)
        .and_then(|process| process.pidfd.as_ref())
        .is_some_and(|node| Rc::ptr_eq(node, &file.node));
    Ok(Named { pid, exists })
}

/// What pollers of a process's descriptors wait on.
pub(crate) fn channel(node: &Node) -> Channel {
    Channel::Object(Rc::as_ptr(node) as *const () as usize)
}

/// `pidfd_poll`: readable once the process has ended, and hung up as well once it is reaped.
pub(super) fn poll(kernel: &Kernel, file: &OpenFile) -> u16 {
    match named(kernel, file) {
        Ok(Named { exists: false, .. }) => (POLLIN | POLLRDNORM | POLLHUP) as u16,
        Ok(Named { pid, .. }) if kernel.processes[&pid].ended.is_some() => {
            (POLLIN | POLLRDNORM) as u16
        }
        _ => 0,
    }
}

/// A new open file naming `pid`, which must exist: one inode per process, as pidfs keeps one per
/// `struct pid`, owned by root with no file type (measured).
pub(super) fn make(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    pid: Pid,
    flags: u32,
) -> Result<FileRef, Errno> {
    let node = match kernel.processes[&pid].pidfd.clone() {
        Some(node) => node,
        None => {
            let now = kernel.now(cx);
            let fs = kernel
                .files
                .pidfs
                .get_or_insert_with(|| kernel.files.numbers.file_system(Kind::Pidfs, 0))
                .clone();
            let ino = kernel.files.numbers.ino();
            let node = new_inode(
                fs,
                ino,
                New {
                    mode: S_IFREG | 0o700,
                    uid: 0,
                    gid: 0,
                    rdev: 0,
                    now,
                    content: Content::Pidfd(pid, None),
                    parent: None,
                    name: LINK.to_vec(),
                },
            )?;
            kernel
                .processes
                .get_mut(&pid)
                .expect("process exists")
                .pidfd = Some(node.clone());
            node
        }
    };
    let opener = kernel.process_of(tid).credentials.serial;
    Ok(OpenFile::new(node, O_RDWR | flags, LINK.to_vec(), opener))
}

pub(super) fn pidfd_open(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(O_NONBLOCK | PIDFD_THREAD) != 0 {
        return Err(Errno::EINVAL);
    }
    let pid = pid as i32;
    if pid <= 0 {
        return Err(Errno::EINVAL);
    }
    // `find_get_pid`, then `pidfd_prepare`: a number that names only a group or session is EINVAL, as
    // is a thread other than the first without PIDFD_THREAD.
    match kernel.threads.get(&pid) {
        Some(thread) if thread.process != pid => {
            if flags & PIDFD_THREAD == 0 {
                return Err(Errno::EINVAL);
            }
            // Descriptors for threads other than a process's first come with threads.
            return Err(kernel.counters.refuse_flag(
                cx.host,
                __NR_pidfd_open,
                flags.into(),
                Errno::EINVAL,
            ));
        }
        Some(_) => {}
        None if kernel.processes.contains_key(&pid) => {}
        None if kernel
            .processes
            .values()
            .any(|process| process.group == pid || process.session == pid) =>
        {
            return Err(Errno::EINVAL);
        }
        None => return Err(Errno::ESRCH),
    }
    let file = make(kernel, cx, tid, pid, flags)?;
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let fd = kernel.files_mut(tid).add(file, true, 0, limit)?;
    Ok(fd as u64)
}

/// Whom a descriptor's signal goes to (`do_pidfd_send_signal`'s `type`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Thread,
    Process,
    Group,
}

pub(super) fn pidfd_send_signal(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, signal, info_address, flags, ..]: Args,
) -> Result<u64, Errno> {
    let (signal, flags) = (signal as u32, flags as u32);
    let scopes = PIDFD_SIGNAL_THREAD | PIDFD_SIGNAL_THREAD_GROUP | PIDFD_SIGNAL_PROCESS_GROUP;
    if flags & !scopes != 0 || flags.count_ones() > 1 {
        return Err(Errno::EINVAL);
    }
    // The caller's own thread or process, or what a descriptor names, with the scope its kind gives.
    let caller = kernel.threads[&tid].process;
    let (target, mut scope) = match fd as i32 {
        PIDFD_SELF_THREAD => (
            Named {
                pid: tid,
                exists: true,
            },
            Scope::Thread,
        ),
        PIDFD_SELF_THREAD_GROUP => (
            Named {
                pid: caller,
                exists: true,
            },
            Scope::Process,
        ),
        _ => {
            let file = super::io::open_file(kernel, tid, fd)?;
            let file = file.borrow();
            let target = named(kernel, &file)?;
            let scope = if file.flags & PIDFD_THREAD != 0 {
                Scope::Thread
            } else {
                Scope::Process
            };
            (target, scope)
        }
    };
    scope = match flags {
        PIDFD_SIGNAL_THREAD => Scope::Thread,
        PIDFD_SIGNAL_THREAD_GROUP => Scope::Process,
        PIDFD_SIGNAL_PROCESS_GROUP => Scope::Group,
        _ => scope,
    };
    let info = if info_address == 0 {
        let sender = kernel.process_of(tid);
        let code = if scope == Scope::Thread {
            SI_TKILL
        } else {
            SI_USER as i32
        };
        crate::signal::sender_info(signal, code, sender.pid, sender.credentials.uid)
    } else {
        let info = super::signals::user_info(kernel, cx, tid, info_address)?;
        let signo = u32::from_le_bytes(info[0..4].try_into().expect("four bytes"));
        let code = i32::from_le_bytes(info[8..12].try_into().expect("four bytes"));
        if signo != signal {
            return Err(Errno::EINVAL);
        }
        // Only the caller itself may be sent what claims to come from the kernel or `tkill`.
        if (target.pid != tid || scope == Scope::Group) && (code >= 0 || code == SI_TKILL) {
            return Err(Errno::EPERM);
        }
        info
    };
    match scope {
        Scope::Group => {
            let reused = !target.exists && kernel.processes.contains_key(&target.pid);
            if reused {
                return Err(Errno::ESRCH);
            }
            super::signals::signal_group(kernel, cx, tid, target.pid, signal, info)
        }
        _ if !target.exists => Err(Errno::ESRCH),
        Scope::Thread if kernel.threads.contains_key(&target.pid) => {
            super::signals::signal_thread(kernel, cx, tid, None, target.pid, signal, Some(info))
        }
        // An ended process's first thread is still there to be sent to.
        Scope::Thread => {
            super::signals::signal_process(kernel, cx, tid, target.pid, signal, info).map(|()| 0)
        }
        Scope::Process => {
            super::signals::signal_process(kernel, cx, tid, target.pid, signal, info).map(|()| 0)
        }
    }
}

/// `process_mrelease`: frees a dying process's memory early. A fatal signal ends a process here at
/// once, so there is never a dying one whose memory is left: an ended process has none (ESRCH), and
/// a running one is not dying (EINVAL).
pub(super) fn process_mrelease(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, flags, ..]: Args,
) -> Result<u64, Errno> {
    if flags != 0 {
        return Err(Errno::EINVAL);
    }
    let file = super::io::open_file(kernel, tid, fd)?;
    let target = named(kernel, &file.borrow())?;
    if !target.exists || kernel.processes[&target.pid].ended.is_some() {
        return Err(Errno::ESRCH);
    }
    Err(Errno::EINVAL)
}

/// The descriptor number a thread's table would give the next file, for `clone`'s CLONE_PIDFD,
/// which tells the parent its number before the child exists.
pub(super) fn next_descriptor(kernel: &Kernel, tid: Tid) -> Result<i32, Errno> {
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    kernel.files(tid).lowest_free(0, limit)
}

/// Gives a parent its descriptor for a child it just made (CLONE_PIDFD), close on exec.
pub(super) fn give_child_descriptor(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    child: Pid,
) -> Result<i32, Errno> {
    let file = make(kernel, cx, tid, child, 0)?;
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    kernel.files_mut(tid).add(file, true, 0, limit)
}

/// The cgroup every process of the container is in, as `cgroup_id` reports it: the made-up machine
/// has one hierarchy whose root the container is.
const CGROUP_ID: u64 = 1;

/// `pidfd_ioctl`: the inode's generation, the process's information, and its namespaces, which have
/// no files here.
pub(super) fn ioctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    use crate::linux::ioctl::FS_IOC_GETVERSION;
    use crate::linux_extra::{PIDFD_GET_INFO, PIDFD_GET_NAMESPACES, PIDFD_INFO_SIZE_VER0};
    let info = request & !(0x3fff << 16) == PIDFD_GET_INFO & !(0x3fff << 16)
        && (request >> 16) & 0x3fff >= PIDFD_INFO_SIZE_VER0 as u32;
    // `pidfs_ioctl_valid`, then the requests in `pidfd_ioctl`'s order.
    if request == FS_IOC_GETVERSION {
        if argument == 0 {
            return Err(Errno::EINVAL);
        }
        // pidfs's generation is 0 on a 64 bit machine.
        return super::guest(kernel, cx, tid)
            .put(argument, &0u32.to_le_bytes())
            .map(|()| 0);
    }
    if info {
        return pidfd_info(kernel, cx, tid, file, request, argument);
    }
    if !PIDFD_GET_NAMESPACES.contains(&request) {
        return Err(Errno::ENOTTY);
    }
    let named = named(kernel, &file.borrow())?;
    if !named.exists || kernel.processes[&named.pid].ended.is_some() {
        return Err(Errno::ESRCH);
    }
    if argument != 0 {
        return Err(Errno::EINVAL);
    }
    let caller = kernel.threads[&tid].process;
    if !kernel.ptrace_may_read(caller, named.pid) {
        return Err(Errno::EACCES);
    }
    // Namespace files (nsfs) are not built: EOPNOTSUPP, as Linux answers for a namespace it lacks.
    Err(kernel.counters.refuse(
        cx.host,
        crate::counters::reason::IOCTL_NOT_BUILT,
        request.into(),
        Errno::EOPNOTSUPP,
    ))
}

/// `pidfd_info`: in `struct pidfd_info` (the mask at 0, the cgroup at 8, the numbers from 16, the
/// credentials from 28, the wait status at 60, the core dump bits at 64, the supported mask at 72),
/// the process's numbers, credentials and cgroup, its wait status once reaped and
/// whether it may dump core, in as much of `struct pidfd_info` as the request's size holds.
fn pidfd_info(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    use crate::linux_extra::*;
    let size = ((request >> 16) & 0x3fff) as usize;
    if argument == 0 {
        return Err(Errno::EINVAL);
    }
    let asked: u64 = super::guest(kernel, cx, tid).read_value(argument)?;
    let (pid, exit) = match file.borrow().node.borrow().content {
        Content::Pidfd(pid, exit) => (pid, exit),
        _ => return Err(Errno::EBADF),
    };
    let mut info = [0u8; PIDFD_INFO_SIZE_VER2];
    let mut mask = 0u64;
    let mut put = |at: usize, value: u32| info[at..at + 4].copy_from_slice(&value.to_le_bytes());
    if asked & PIDFD_INFO_EXIT != 0
        && let Some(status) = exit
    {
        mask |= PIDFD_INFO_EXIT | PIDFD_INFO_CGROUPID;
        put(60, status as u32);
    }
    let cgroup = mask & PIDFD_INFO_CGROUPID != 0;
    let named = named(kernel, &file.borrow())?;
    if named.exists {
        let process = &kernel.processes[&pid];
        // A process that has ended has no memory, so nothing about its core dump is known.
        if asked & PIDFD_INFO_COREDUMP != 0 && process.ended.is_none() {
            let dump = if process.dumpable {
                PIDFD_COREDUMP_USER
            } else {
                PIDFD_COREDUMP_SKIP
            };
            put(64, dump);
            mask |= PIDFD_INFO_COREDUMP;
        }
        let c = &process.credentials;
        for (at, id) in [
            (28, c.uid),
            (32, c.gid),
            (36, c.euid),
            (40, c.egid),
            (44, c.suid),
            (48, c.sgid),
            (52, c.fsuid),
            (56, c.fsgid),
        ] {
            put(at, id);
        }
        mask |= PIDFD_INFO_CREDS | PIDFD_INFO_CGROUPID | PIDFD_INFO_PID;
        // The parent, the process and its first thread.
        put(24, process.parent as u32);
        put(20, pid as u32);
        put(16, pid as u32);
    } else if asked & PIDFD_INFO_EXIT == 0 {
        return Err(Errno::ESRCH);
    }
    if cgroup || named.exists {
        info[8..16].copy_from_slice(&CGROUP_ID.to_le_bytes());
    }
    if asked & PIDFD_INFO_SUPPORTED_MASK != 0 {
        mask |= PIDFD_INFO_SUPPORTED_MASK;
        let supported = PIDFD_INFO_PID
            | PIDFD_INFO_CREDS
            | PIDFD_INFO_CGROUPID
            | PIDFD_INFO_EXIT
            | PIDFD_INFO_COREDUMP
            | PIDFD_INFO_SUPPORTED_MASK
            | PIDFD_INFO_COREDUMP_SIGNAL;
        info[72..80].copy_from_slice(&supported.to_le_bytes());
    }
    info[0..8].copy_from_slice(&mask.to_le_bytes());
    // `copy_struct_to_user`: a larger structure's tail is cleared first, then the common part.
    let common = size.min(PIDFD_INFO_SIZE_VER2);
    let mut guest = super::guest(kernel, cx, tid);
    if size > common {
        guest.write(argument + common as u64, &alloc::vec![0; size - common])?;
    }
    guest.write(argument, &info[..common])?;
    Ok(0)
}
