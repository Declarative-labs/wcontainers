//! `/proc/<pid>/fdinfo/<fd>`: what Linux's `seq_show` prints about an open descriptor (its
//! position, flags, mount and inode, and the locks taken through it), then what the file's kind
//! adds (`show_fdinfo`).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use crate::file::Anon;
use crate::fs::proc::Task;
use crate::fs::{Content, Inode};
use crate::linux::general::*;
use crate::locks::{LockOwner, OFFSET_MAX};
use crate::{Errno, Kernel};

/// The bytes of a descriptor's fdinfo file; ENOENT once the descriptor is closed.
pub(super) fn contents(kernel: &Kernel, task: Task, fd: i32) -> Result<Vec<u8>, Errno> {
    let thread = kernel.threads.get(&task.tid).ok_or(Errno::ENOENT)?;
    let (file, close_on_exec, lock_owner) = {
        let files = thread.files.borrow();
        let descriptor = files.get(fd).map_err(|_| Errno::ENOENT)?;
        (
            descriptor.file.clone(),
            descriptor.close_on_exec,
            files.lock_owner(),
        )
    };
    let open = file.borrow();
    let node = open.node.clone();
    let inode = node.borrow();
    let mut out = String::new();
    let flags = open.flags | if close_on_exec { O_CLOEXEC } else { 0 };
    let _ = write!(
        out,
        "pos:\t{}\nflags:\t0{:o}\nmnt_id:\t{}\nino:\t{}\n",
        open.offset as i64,
        flags,
        super::handle::mount_id(kernel, &inode),
        inode.ino
    );
    locks(&mut out, &inode, &file, lock_owner);
    match (&open.anon, &inode.content) {
        (Some(Anon::Event(counter)), _) => {
            let counter = counter.borrow();
            let _ = write!(
                out,
                "eventfd-count: {:16x}\neventfd-id: {}\neventfd-semaphore: {}\n",
                counter.count,
                counter.id,
                u8::from(counter.semaphore)
            );
        }
        (Some(Anon::Epoll(epoll)), _) => {
            for (target, item_file, events, data) in super::epoll::items(&epoll.borrow()) {
                let item = item_file.borrow();
                let target_inode = item.node.borrow();
                let _ = writeln!(
                    out,
                    "tfd: {:8} events: {:8x} data: {:16x}  pos:{} ino:{:x} sdev:{:x}",
                    target,
                    events,
                    data,
                    item.offset as i64,
                    target_inode.ino,
                    kernel_device(target_inode.fs.device)
                );
            }
        }
        (Some(Anon::Inotify(instance)), _) => {
            for (wd, watched, mask) in super::inotify::marks(instance) {
                let watched = watched.borrow();
                let _ = write!(
                    out,
                    "inotify wd:{:x} ino:{:x} sdev:{:x} mask:{:x} ignored_mask:0 ",
                    wd,
                    watched.ino,
                    kernel_device(watched.fs.device),
                    mask
                );
                // `show_mark_fhandle`: the file's identifier, when its file system makes one.
                if let Ok((kind, words)) = super::handle::encode(kernel, &watched, true) {
                    let _ = write!(
                        out,
                        "fhandle-bytes:{:x} fhandle-type:{:x} f_handle:",
                        words.len() * 4,
                        kind
                    );
                    for byte in words.iter().flat_map(|word| word.to_le_bytes()) {
                        let _ = write!(out, "{byte:02x}");
                    }
                }
                out.push('\n');
            }
        }
        (Some(Anon::Signals(mask)), _) => {
            let _ = writeln!(out, "sigmask:\t{mask:016x}");
        }
        (Some(Anon::Timer(id)), _) => out.push_str(&super::timer::fdinfo(kernel, *id)),
        // `pidfd_show_fdinfo`: the process's number while it has a task, -1 once reaped.
        (None, Content::Pidfd(pid, exit)) => {
            let alive = exit.is_none()
                && (kernel.processes.contains_key(pid) || kernel.threads.contains_key(pid));
            let shown = if alive { *pid } else { -1 };
            let _ = write!(out, "Pid:\t{shown}\nNSpid:\t{shown}\n");
        }
        (None, Content::Socket(socket)) => {
            if let Some(count) = super::socket::fdinfo(socket) {
                let _ = writeln!(out, "scm_fds: {count}");
            }
        }
        // `pty_show_fdinfo`, of the pseudo-terminal master only.
        _ => {
            if let Some(terminal) = &open.terminal
                && terminal.side == crate::tty::MASTER
            {
                let _ = writeln!(out, "tty-index:\t{}", terminal.pty.borrow().index);
            }
        }
    }
    Ok(out.into_bytes())
}

/// `show_fd_locks`: the file's flock, then the record locks taken through this file by this
/// descriptor table or by the file itself, numbered from 1, each as `/proc/locks` shows it.
fn locks(out: &mut String, inode: &Inode, file: &crate::file::FileRef, lock_owner: u64) {
    let own = alloc::rc::Rc::downgrade(file);
    let (major, minor) = crate::fs::decode_device(inode.fs.device);
    let mut id = 0;
    let mut line = |out: &mut String, kind: &str, write: bool, pid: i32, range: String| {
        id += 1;
        let _ = writeln!(
            out,
            "lock:\t{id}: {kind} ADVISORY  {} {pid} {major:02x}:{minor:02x}:{} {range}",
            if write { "WRITE" } else { "READ" },
            inode.ino
        );
    };
    for (holder, exclusive, pid) in &inode.flocks {
        if holder.ptr_eq(&own) {
            line(out, "FLOCK ", *exclusive, *pid, String::from("0 EOF"));
        }
    }
    for lock in &inode.record_locks {
        let (kind, pid) = match &lock.owner {
            LockOwner::Table(table) if *table == lock_owner => ("POSIX ", lock.pid),
            LockOwner::File(holder) if holder.ptr_eq(&own) => ("OFDLCK", -1),
            _ => continue,
        };
        if !lock.file.ptr_eq(&own) {
            continue;
        }
        let range = if lock.end == OFFSET_MAX {
            alloc::format!("{} EOF", lock.start)
        } else {
            alloc::format!("{} {}", lock.start, lock.end)
        };
        line(out, kind, lock.write, pid, range);
    }
}

/// A device number as the kernel holds it (`s_dev`: major above bit 20), from the number `stat`
/// gives.
fn kernel_device(device: u64) -> u64 {
    let (major, minor) = crate::fs::decode_device(device);
    (u64::from(major) << 20) | u64::from(minor)
}
