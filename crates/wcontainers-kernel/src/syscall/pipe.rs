//! `pipe2`, and reading and writing pipes (`pipe_read`, `pipe_write`).

use alloc::vec::Vec;

use wcontainers_abi::{PAGE_SIZE, Permissions};

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
    let (uid, gid) = (kernel.credentials(tid).fsuid, kernel.credentials(tid).fsgid);
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
    // Only the writing end takes packet mode (`create_pipe_files`).
    let opener = kernel.credentials(tid).serial;
    let reader = OpenFile::new(
        node.clone(),
        O_RDONLY | (flags & O_NONBLOCK),
        Vec::new(),
        opener,
    );
    let writer = OpenFile::new(
        node.clone(),
        O_WRONLY | (flags & (O_NONBLOCK | O_DIRECT)),
        Vec::new(),
        opener,
    );
    attach(&node, &reader);
    attach(&node, &writer);
    let close_on_exec = flags & O_CLOEXEC != 0;
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let pid = kernel.threads[&tid].process;
    let mut files = kernel.files_mut(tid);
    let read_fd = files.add(reader, close_on_exec, 0, limit)?;
    let write_fd = match files.add(writer, close_on_exec, 0, limit) {
        Ok(fd) => fd,
        Err(error) => {
            let _ = files.remove(read_fd, pid);
            drop(files);
            kernel.files_released(cx);
            return Err(error);
        }
    };
    drop(files);
    let mut bytes = [0u8; 8];
    bytes[..4].copy_from_slice(&read_fd.to_le_bytes());
    bytes[4..].copy_from_slice(&write_fd.to_le_bytes());
    if let Err(error) = guest(kernel, cx, tid).write(fds, &bytes) {
        let mut files = kernel.files_mut(tid);
        let _ = files.remove(read_fd, pid);
        let _ = files.remove(write_fd, pid);
        drop(files);
        kernel.files_released(cx);
        return Err(error);
    }
    Ok(0)
}

/// Notes a new open file of a pipe's end.
pub(crate) fn attach(node: &Node, file: &FileRef) {
    if let Content::Pipe(pipe) = &mut node.borrow_mut().content {
        pipe.opened(file, 0);
    }
}

/// Gives a named pipe a fresh buffer when no end of it is open, as Linux frees its buffer at the last
/// close and makes one at the next open (`fifo_open`).
pub(super) fn named(node: &Node) {
    let mut inode = node.borrow_mut();
    if !matches!(&inode.content, Content::Pipe(pipe) if pipe.is_open()) {
        inode.content = Content::Pipe(Pipe::named());
    }
}

/// `fifo_open` for a new open file of a named pipe: it counts as an end at once; a reader waits for
/// a writer to open, and a writer for a reader, unless nonblocking (a nonblocking writer with no
/// reader is ENXIO); reading and writing never waits.
pub(super) fn open_named(kernel: &mut Kernel, tid: Tid, file: &FileRef) -> Result<(), Errno> {
    let node = file.borrow().node.clone();
    let (readable, writable, nonblocking) = {
        let file = file.borrow();
        (
            file.readable(),
            file.writable(),
            file.flags & O_NONBLOCK != 0,
        )
    };
    let waits_for = {
        let mut inode = node.borrow_mut();
        let Content::Pipe(pipe) = &mut inode.content else {
            unreachable!("a named pipe has its buffer");
        };
        match (readable, writable) {
            (true, false) => {
                pipe.opened_readers = pipe.opened_readers.wrapping_add(1);
                // A nonblocking reader with no writer sees no hangup until a writer has opened.
                let seen = if pipe.writers() == 0 && nonblocking {
                    pipe.opened_writers
                } else {
                    0
                };
                pipe.opened(file, seen);
                (pipe.writers() == 0 && !nonblocking).then_some(pipe.opened_writers)
            }
            (false, true) => {
                if nonblocking && pipe.readers() == 0 {
                    return Err(Errno::ENXIO);
                }
                pipe.opened_writers = pipe.opened_writers.wrapping_add(1);
                pipe.opened(file, 0);
                (pipe.readers() == 0).then_some(pipe.opened_readers)
            }
            (true, true) => {
                pipe.opened_readers = pipe.opened_readers.wrapping_add(1);
                pipe.opened_writers = pipe.opened_writers.wrapping_add(1);
                pipe.opened(file, 0);
                None
            }
            (false, false) => return Err(Errno::EINVAL),
        }
    };
    kernel.wake(channel(&node));
    let Some(count) = waits_for else {
        return Ok(());
    };
    let thread = kernel.thread_mut(tid);
    thread.opening = Some(file.clone());
    thread.call.started = true;
    thread.call.progress = count.into();
    kernel.park(tid, channel(&node));
    Err(Errno::PARKED)
}

/// A named pipe's open run again after its wait: done once the other end has opened, ended by a
/// signal (giving up its end), or waiting on.
pub(super) fn open_named_again(kernel: &mut Kernel, tid: Tid) -> Result<FileRef, Errno> {
    let thread = kernel.thread_mut(tid);
    let file = thread.opening.take().expect("an open waiting");
    let count = thread.call.progress as u32;
    let node = file.borrow().node.clone();
    let readable = file.borrow().readable();
    let now = match &node.borrow().content {
        Content::Pipe(pipe) if readable => pipe.opened_writers,
        Content::Pipe(pipe) => pipe.opened_readers,
        _ => unreachable!("a named pipe has its buffer"),
    };
    if now != count {
        return Ok(file);
    }
    kernel.thread_mut(tid).opening = Some(file);
    // A signal ends the wait; the end is given up as the call finishes.
    if kernel.signal_pending(tid) {
        return Err(Errno::ERESTARTSYS);
    }
    kernel.park(tid, channel(&node));
    Err(Errno::PARKED)
}

/// `kill_fasync` on a pipe's readers (`POLL_IN`) or writers (`POLL_OUT`).
pub(super) fn signal_ends(kernel: &mut Kernel, cx: &mut Context<'_>, node: &Node, readers: bool) {
    let ends = match &node.borrow().content {
        Content::Pipe(pipe) => pipe.async_ends(readers),
        _ => return,
    };
    let reason = if readers { POLL_IN } else { POLL_OUT };
    for end in ends {
        kernel.send_io_signal(cx, &end, reason);
    }
}

/// `pipe_release` of each file given up that was its last holder: a pipe left with readers and no
/// writers or the other way round sends both kinds of end their I/O signals. Releases run last given
/// up first, as `fput`'s task works do on the way back to the program, and the file is gone from the
/// pipe's ends first, as `__fput` takes it off before the release.
pub(crate) fn released(kernel: &mut Kernel, cx: &mut Context<'_>, files: Vec<FileRef>) {
    for file in files.into_iter().rev() {
        let node = file.borrow().node.clone();
        let last = alloc::rc::Rc::strong_count(&file) == 1;
        drop(file);
        let lost_a_side = match &node.borrow().content {
            Content::Pipe(pipe) => last && (pipe.readers() == 0) != (pipe.writers() == 0),
            _ => false,
        };
        if lost_a_side {
            signal_ends(kernel, cx, &node, true);
            signal_ends(kernel, cx, &node, false);
        }
    }
}

/// `pipe_lock`: waits, without a signal ending the wait, while another thread's call holds the
/// pipe's lock, parked or woken and not yet run again.
pub(super) fn lock(kernel: &mut Kernel, tid: Tid, file: &FileRef) -> Result<(), Errno> {
    let node = file.borrow().node.clone();
    let Channel::Object(address) = channel(&node) else {
        unreachable!("a pipe's channel is its object");
    };
    let holder = with_pipe(file, |pipe| pipe.held_by);
    let held = holder.is_some_and(|holder| {
        holder != tid
            && kernel
                .threads
                .get(&holder)
                .is_some_and(|thread| thread.call.holds == Some(address))
    });
    if held {
        kernel.park(tid, Channel::Object(address));
        return Err(Errno::PARKED);
    }
    Ok(())
}

/// Holds the pipe's lock while the call waits (it parked).
pub(super) fn hold(kernel: &mut Kernel, tid: Tid, file: &FileRef) {
    let node = file.borrow().node.clone();
    let Channel::Object(address) = channel(&node) else {
        unreachable!("a pipe's channel is its object");
    };
    with_pipe(file, |pipe| pipe.held_by = Some(tid));
    kernel.thread_mut(tid).call.holds = Some(address);
}

pub(super) fn with_pipe<R>(file: &FileRef, f: impl FnOnce(&mut Pipe) -> R) -> R {
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
    lock(kernel, tid, file)?;
    let node = file.borrow().node.clone();
    // Empty pages a faulted write left are passed over.
    with_pipe(file, |pipe| pipe.drop_empty());
    if with_pipe(file, |pipe| pipe.is_empty()) {
        let (writers, nonblocking) = (
            with_pipe(file, |pipe| pipe.writers()),
            file.borrow().flags & O_NONBLOCK != 0,
        );
        if writers == 0 {
            return Ok(0);
        }
        // Writers hear of the room before a wait, once, and of a nonblocking read's end.
        if !kernel.threads[&tid].call.started {
            signal_ends(kernel, cx, &node, false);
        }
        if nonblocking {
            return Err(Errno::EAGAIN);
        }
        if kernel.signal_pending(tid) {
            return Err(Errno::ERESTARTSYS);
        }
        kernel.thread_mut(tid).call.started = true;
        kernel.park(tid, channel(&node));
        return Err(Errno::PARKED);
    }
    // A slot's piece leaves the pipe only when it is copied whole: the copy stores up to the first
    // fault (exactly, measured) and the piece stays to be read.
    let mut guest = guest(kernel, cx, tid);
    let mut room = 0;
    for &(base, length) in parts {
        let writable = guest.accessible(base, length, wcontainers_abi::Permissions::WRITE);
        room += writable;
        if writable < length {
            break;
        }
    }
    let (taken, left) = with_pipe(file, |pipe| pipe.take_into(total as usize, room as usize));
    let bytes = [taken.as_slice(), left.as_slice()].concat();
    let mut done = 0usize;
    for &(base, length) in parts {
        if done == bytes.len() {
            break;
        }
        let chunk = &bytes[done..(done + length as usize).min(bytes.len())];
        guest.put(base, chunk)?;
        done += chunk.len();
    }
    if taken.is_empty() {
        signal_ends(kernel, cx, &node, false);
        return Err(Errno::EFAULT);
    }
    let done = taken.len();
    kernel.wake(channel(&node));
    signal_ends(kernel, cx, &node, false);
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
    lock(kernel, tid, file)?;
    let node = file.borrow().node.clone();
    let host = with_pipe(file, |pipe| pipe.host);
    let first_run = !kernel.threads[&tid].call.started;
    let mut done = kernel.threads[&tid].call.progress;
    let (nonblocking, packet) = {
        let file = file.borrow();
        (file.flags & O_NONBLOCK != 0, file.flags & O_DIRECT != 0)
    };
    // Up to `most` bytes from `done` on, across segments, as far as the buffer can be read; and
    // whether a fault stopped it short.
    let next = |kernel: &mut Kernel, cx: &mut Context<'_>, done: u64, most: u64| {
        let mut out = Vec::new();
        let mut skipped = 0;
        let mut guest = guest(kernel, cx, tid);
        for &(base, length) in parts {
            let end = skipped + length;
            if done + (out.len() as u64) < end && (out.len() as u64) < most {
                let from = done + out.len() as u64 - skipped;
                let wanted = (length - from).min(most - out.len() as u64);
                let readable = guest.accessible(base + from, wanted, Permissions::READ);
                if readable > 0 {
                    out.extend(guest.read(base + from, readable).expect("readable bytes"));
                }
                if readable < wanted {
                    return (out, true);
                }
            }
            skipped = end;
        }
        (out, false)
    };
    let result = loop {
        if with_pipe(file, |pipe| pipe.readers()) == 0 {
            let process = kernel.process_of(tid);
            let info = crate::signal::sender_info(
                SIGPIPE,
                SI_USER as i32,
                process.pid,
                kernel.credentials(tid).uid,
            );
            kernel.send_to_thread(cx, tid, SIGPIPE, info);
            break Err(Errno::EPIPE);
        }
        if done == total {
            break Ok(());
        }
        if let Some(stream) = host {
            // The host takes whole pages as they come; a page that cannot be read in full ends the
            // write and is dropped.
            let (bytes, faulted) = next(kernel, cx, done, PAGE_SIZE);
            if faulted {
                break Err(Errno::EFAULT);
            }
            cx.host.output(stream, &bytes);
            done += bytes.len() as u64;
            continue;
        }
        // The part that is not whole pages joins the last page if it fits; a fault in it ends the
        // write with nothing merged.
        if first_run && done == 0 {
            let merge = with_pipe(file, |pipe| pipe.mergeable(total as usize, packet)) as u64;
            if merge > 0 {
                let (bytes, faulted) = next(kernel, cx, 0, merge);
                if faulted {
                    break Err(Errno::EFAULT);
                }
                with_pipe(file, |pipe| pipe.merge(&bytes));
                done = merge;
                continue;
            }
        }
        // A new page; one the buffer cannot fill takes its slot empty and ends the write.
        if !with_pipe(file, |pipe| pipe.is_full()) {
            let (bytes, faulted) = next(kernel, cx, done, PAGE_SIZE);
            if faulted {
                with_pipe(file, |pipe| pipe.push(Vec::new(), packet));
                break Err(Errno::EFAULT);
            }
            done += bytes.len() as u64;
            with_pipe(file, |pipe| pipe.push(bytes, packet));
            continue;
        }
        if nonblocking {
            break Err(Errno::EAGAIN);
        }
        if kernel.signal_pending(tid) {
            break Err(Errno::ERESTARTSYS);
        }
        // Readers hear of what came before each wait (not again after a wake that found no room).
        let call = kernel.threads[&tid].call;
        if !call.started || call.progress != done {
            signal_ends(kernel, cx, &node, true);
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
    signal_ends(kernel, cx, &node, true);
    match result {
        Err(_) if done > 0 => Ok(done),
        Err(error) => Err(error),
        Ok(()) => Ok(done),
    }
}

/// Puts kernel bytes into a pipe without waiting, as splicing into a pipe does: a slot for each
/// page the bytes came from (the first starting `in_page` bytes into its page), as many as fit, none
/// of which a write adds to; the host's pipes take everything. Readers with `FASYNC` hear of them.
pub(super) fn put(
    cx: &mut Context<'_>,
    kernel: &mut Kernel,
    file: &FileRef,
    bytes: &[u8],
    in_page: u64,
) -> u64 {
    let node = file.borrow().node.clone();
    let mut done = 0;
    let taken = with_pipe(file, |pipe| {
        if let Some(stream) = pipe.host {
            cx.host.output(stream, bytes);
            return bytes.len();
        }
        let mut piece = (PAGE_SIZE - in_page % PAGE_SIZE) as usize;
        while done < bytes.len() && !pipe.is_full() {
            let end = (done + piece).min(bytes.len());
            pipe.push_spliced(bytes[done..end].to_vec());
            done = end;
            piece = PAGE_SIZE as usize;
        }
        done
    });
    kernel.wake(channel(&node));
    // `splice_file_to_pipe`'s `wakeup_pipe_readers`.
    if taken > 0 {
        signal_ends(kernel, cx, &node, true);
    }
    taken as u64
}
