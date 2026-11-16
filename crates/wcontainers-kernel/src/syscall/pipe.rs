//! `pipe2`, and reading and writing pipes (`pipe_read`, `pipe_write`).

use alloc::vec::Vec;

use wcontainers_abi::PAGE_SIZE;

use super::{Args, guest};
use crate::file::{FileRef, OpenFile};
use crate::fs::{self, Content, New, Node};
use crate::linux::general::*;
use crate::pipe::Pipe;
use crate::process::{Channel, Tid};
use crate::{Context, Errno, Kernel};

/// The channel a pipe's waiters park on.
pub(crate) fn channel(node: &Node) -> Channel {
    Channel::Object(alloc::rc::Rc::as_ptr(node) as *const () as usize)
}

/// Whether an open file is a pipe's end.
pub(crate) fn is_pipe(file: &FileRef) -> bool {
    matches!(file.borrow().node.borrow().content, Content::Pipe(_))
}

pub(super) fn pipe2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fds, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    // `O_NOTIFICATION_PIPE` is `O_EXCL` (`linux/watch_queue.h`).
    if flags & !(O_CLOEXEC | O_NONBLOCK | O_DIRECT | O_EXCL) != 0 {
        return Err(Errno::EINVAL);
    }
    // Notification pipes carry the kernel's watch queues, which are not built.
    if flags & O_EXCL != 0 {
        return Err(kernel
            .counters
            .refuse_flag(cx.host, __NR_pipe2, O_EXCL.into(), Errno::EINVAL));
    }
    let now = kernel.now(cx);
    let process = kernel.process_of(tid);
    let (uid, gid) = (process.credentials.fsuid, process.credentials.fsgid);
    let ino = kernel.files.numbers.ino();
    let node = fs::new_inode(
        kernel.files.pipe_fs.clone(),
        ino,
        New {
            mode: S_IFIFO | 0o600,
            uid,
            gid,
            rdev: 0,
            now,
            content: Content::Pipe(Pipe::new(None)),
            parent: None,
            name: Vec::new(),
        },
    )?;
    let kept = flags & (O_NONBLOCK | O_DIRECT);
    let opener = kernel.process_of(tid).credentials.serial;
    let reader = OpenFile::new(node.clone(), O_RDONLY | kept, Vec::new(), opener);
    let writer = OpenFile::new(node.clone(), O_WRONLY | kept, Vec::new(), opener);
    attach(&node, &reader);
    attach(&node, &writer);
    let close_on_exec = flags & O_CLOEXEC != 0;
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let files = &mut kernel.process_of_mut(tid).files;
    let read_fd = files.add(reader, close_on_exec, 0, limit)?;
    let write_fd = match files.add(writer, close_on_exec, 0, limit) {
        Ok(fd) => fd,
        Err(error) => {
            let _ = files.remove(read_fd);
            return Err(error);
        }
    };
    let mut bytes = [0u8; 8];
    bytes[..4].copy_from_slice(&read_fd.to_le_bytes());
    bytes[4..].copy_from_slice(&write_fd.to_le_bytes());
    if let Err(error) = guest(kernel, cx, tid).write(fds, &bytes) {
        let files = &mut kernel.process_of_mut(tid).files;
        let _ = files.remove(read_fd);
        let _ = files.remove(write_fd);
        return Err(error);
    }
    Ok(0)
}

/// Notes a new open file of a pipe's end.
pub(crate) fn attach(node: &Node, file: &FileRef) {
    if let Content::Pipe(pipe) = &mut node.borrow_mut().content {
        pipe.opened(file);
    }
}

fn with_pipe<R>(file: &FileRef, f: impl FnOnce(&mut Pipe) -> R) -> R {
    let node = file.borrow().node.clone();
    let mut inode = node.borrow_mut();
    let Content::Pipe(pipe) = &mut inode.content else {
        unreachable!("the file is a pipe");
    };
    f(pipe)
}

/// `pipe_read` into the given segments: what is there, or a wait for a writer.
pub(super) fn read(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    let total: u64 = parts.iter().map(|(_, length)| length).sum();
    if total == 0 {
        return Ok(0);
    }
    let node = file.borrow().node.clone();
    if with_pipe(file, |pipe| pipe.is_empty()) {
        let (writers, nonblocking) = (
            with_pipe(file, |pipe| pipe.writers()),
            file.borrow().flags & O_NONBLOCK != 0,
        );
        if writers == 0 {
            return Ok(0);
        }
        if nonblocking {
            return Err(Errno::EAGAIN);
        }
        if kernel.signal_pending(tid) {
            return Err(Errno::ERESTARTSYS);
        }
        kernel.park(tid, channel(&node));
        return Err(Errno::PARKED);
    }
    // Only as much as the buffer takes leaves the pipe: a copy stops at the first fault and the
    // rest stays to be read.
    let mut guest = guest(kernel, cx, tid);
    let mut room = 0;
    for &(base, length) in parts {
        let writable = guest.accessible(base, length, wcontainers_abi::Permissions::WRITE);
        room += writable;
        if writable < length {
            break;
        }
    }
    if room == 0 {
        return Err(Errno::EFAULT);
    }
    let bytes = with_pipe(file, |pipe| pipe.take(room as usize));
    let mut done = 0usize;
    for &(base, length) in parts {
        if done == bytes.len() {
            break;
        }
        let chunk = &bytes[done..(done + length as usize).min(bytes.len())];
        guest.write(base, chunk)?;
        done += chunk.len();
    }
    kernel.wake(channel(&node));
    Ok(done as u64)
}

/// `pipe_write` from the given segments: as much as fits, then a wait for the reader, keeping what is
/// written in the call's slot. With no reader, SIGPIPE and EPIPE.
pub(super) fn write(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    let total: u64 = parts.iter().map(|(_, length)| length).sum();
    if total == 0 {
        return Ok(0);
    }
    let node = file.borrow().node.clone();
    let host = with_pipe(file, |pipe| pipe.host);
    let first_run = !kernel.threads[&tid].call.started;
    let mut done = kernel.threads[&tid].call.progress;
    let (nonblocking, packet) = {
        let file = file.borrow();
        (file.flags & O_NONBLOCK != 0, file.flags & O_DIRECT != 0)
    };
    // The bytes from `done` on, a page at most, as far as the buffer can be read.
    let next = |kernel: &mut Kernel, cx: &mut Context<'_>, done: u64, most: u64| {
        let mut skipped = 0;
        for &(base, length) in parts {
            if done < skipped + length {
                let at = base + (done - skipped);
                let length = (skipped + length - done).min(most);
                return guest(kernel, cx, tid).read(at, length);
            }
            skipped += length;
        }
        Ok(Vec::new())
    };
    let result = loop {
        if with_pipe(file, |pipe| pipe.readers()) == 0 {
            let process = kernel.process_of(tid);
            let info = crate::signal::sender_info(
                SIGPIPE,
                SI_USER as i32,
                process.pid,
                process.credentials.uid,
            );
            kernel.send_to_thread(cx, tid, SIGPIPE, info);
            break Err(Errno::EPIPE);
        }
        if done == total {
            break Ok(());
        }
        if let Some(stream) = host {
            // The host takes whole pages as they come; a page that cannot be read ends the write.
            match next(kernel, cx, done, PAGE_SIZE) {
                Ok(bytes) => {
                    cx.host.output(stream, &bytes);
                    done += bytes.len() as u64;
                    continue;
                }
                Err(error) => break Err(error),
            }
        }
        if first_run && done == 0 {
            let merge = with_pipe(file, |pipe| pipe.mergeable(total as usize, packet)) as u64;
            if merge > 0 {
                match next(kernel, cx, 0, merge) {
                    Ok(bytes) if bytes.len() as u64 == merge => {
                        with_pipe(file, |pipe| pipe.merge(&bytes));
                        done = merge;
                        continue;
                    }
                    _ => break Err(Errno::EFAULT),
                }
            }
        }
        if !with_pipe(file, |pipe| pipe.is_full()) {
            match next(kernel, cx, done, PAGE_SIZE) {
                Ok(bytes) => {
                    done += bytes.len() as u64;
                    with_pipe(file, |pipe| pipe.push(bytes, packet));
                    continue;
                }
                Err(error) => break Err(error),
            }
        }
        if nonblocking {
            break Err(Errno::EAGAIN);
        }
        if kernel.signal_pending(tid) {
            break Err(Errno::ERESTARTSYS);
        }
        let thread = kernel.thread_mut(tid);
        thread.call.started = true;
        thread.call.progress = done;
        kernel.wake(channel(&node));
        kernel.park(tid, channel(&node));
        return Err(Errno::PARKED);
    };
    if done > 0 {
        kernel.wake(channel(&node));
    }
    match result {
        Err(_) if done > 0 => Ok(done),
        Err(error) => Err(error),
        Ok(()) => Ok(done),
    }
}

/// Puts kernel bytes into a pipe without waiting, as splicing into a pipe does: as many whole
/// pages as fit; the host's pipes take everything.
pub(super) fn put(cx: &mut Context<'_>, kernel: &mut Kernel, file: &FileRef, bytes: &[u8]) -> u64 {
    let node = file.borrow().node.clone();
    let mut done = 0;
    let taken = with_pipe(file, |pipe| {
        if let Some(stream) = pipe.host {
            cx.host.output(stream, bytes);
            return bytes.len();
        }
        for chunk in bytes.chunks(PAGE_SIZE as usize) {
            if pipe.is_full() {
                break;
            }
            pipe.push(chunk.to_vec(), false);
            done += chunk.len();
        }
        done
    });
    kernel.wake(channel(&node));
    taken as u64
}
