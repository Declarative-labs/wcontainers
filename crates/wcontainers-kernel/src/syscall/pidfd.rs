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
    let Content::Pidfd(pid) = file.node.borrow().content else {
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
                    mode: 0o700,
                    uid: 0,
                    gid: 0,
                    rdev: 0,
                    now,
                    content: Content::Pidfd(pid),
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
    let process = kernel.process_of_mut(tid);
    let limit = process.limits[RLIMIT_NOFILE as usize].current;
    let fd = process.files.add(file, true, 0, limit)?;
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
            let file = super::io::file(kernel, tid, fd)?;
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
    let file = super::io::file(kernel, tid, fd)?;
    let target = named(kernel, &file.borrow())?;
    if !target.exists || kernel.processes[&target.pid].ended.is_some() {
        return Err(Errno::ESRCH);
    }
    Err(Errno::EINVAL)
}

/// The descriptor numbers a process's table would give the next files, for `clone`'s CLONE_PIDFD,
/// which tells the parent its number before the child exists.
pub(super) fn next_descriptor(kernel: &Kernel, pid: Pid) -> Result<i32, Errno> {
    let process = &kernel.processes[&pid];
    let limit = process.limits[RLIMIT_NOFILE as usize].current;
    process.files.lowest_free(0, limit)
}

/// Gives a parent its descriptor for a child it just made (CLONE_PIDFD), close on exec.
pub(super) fn give_child_descriptor(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    child: Pid,
) -> Result<i32, Errno> {
    let file = make(kernel, cx, tid, child, 0)?;
    let process = kernel.process_of_mut(tid);
    let limit = process.limits[RLIMIT_NOFILE as usize].current;
    process.files.add(file, true, 0, limit)
}
