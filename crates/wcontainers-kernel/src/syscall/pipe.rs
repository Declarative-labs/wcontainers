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

/// The type and permission bits of a pipe (`create_pipe_files`).
pub(crate) const MODE: u32 = S_IFIFO | S_IRUSR | S_IWUSR;

/// The thread number of no thread, for the outside's reads, which no call makes.
const NO_THREAD: Tid = 0;

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
            mode: MODE,
            uid,
            gid,
            rdev: 0,
            now,
            content: Content::Pipe(Pipe::new()),
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
            let _ = files.remove(read_fd, pid, false);
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
        let _ = files.remove(read_fd, pid, false);
        let _ = files.remove(write_fd, pid, false);
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
    if matches!(&inode.content, Content::Pipe(pipe) if pipe.is_open()) {
        return;
    }
    let mut pipe = Pipe::named();
    pipe.backend = match core::mem::replace(&mut inode.content, Content::Special) {
        Content::Pipe(old) => old.backend,
        Content::Backend(remote) => Some(alloc::rc::Rc::new(remote)),
        _ => None,
    };
    inode.content = Content::Pipe(pipe);
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
/// writers or the other way round wakes every waiter and epoll item of either end
/// (`wake_up_interruptible_all`, which edge-triggered items take as an event) and sends both
/// kinds of end their I/O signals. Releases run last given
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
            kernel.wake_poll(channel(&node), 0);
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
    if kernel.held_by_other(tid, holder, Channel::Object(address)) {
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
    kernel.hold_lock(tid, Channel::Object(address));
}

pub(crate) fn with_pipe<R>(file: &FileRef, f: impl FnOnce(&mut Pipe) -> R) -> R {
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
    if with_pipe(file, |pipe| pipe.is_empty()) {
        let (writers, nonblocking) = (
            with_pipe(file, |pipe| pipe.writers()),
            super::io::nonblocking(kernel, tid, file),
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
    kernel.wake_poll(channel(&node), EPOLLOUT | EPOLLWRNORM);
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
    let outside = with_pipe(file, |pipe| pipe.outside_reader);
    let first_run = !kernel.threads[&tid].call.started;
    let mut done = kernel.threads[&tid].call.progress;
    let nonblocking = super::io::nonblocking(kernel, tid, file);
    let packet = file.borrow().flags & O_DIRECT != 0;
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
        // An end outside reads it: with the pipe empty, bytes go out at once as far as its credit
        // allows, a page at a time (a page that cannot be read in full ends the write, unsent).
        if let Some(end) = outside
            && with_pipe(file, |pipe| pipe.is_empty())
        {
            let credit = kernel.ends.credit(end);
            if credit > 0 {
                let (bytes, faulted) = next(kernel, cx, done, credit.min(PAGE_SIZE));
                if faulted {
                    break Err(Errno::EFAULT);
                }
                kernel.ends.send(cx, end, &bytes);
                done += bytes.len() as u64;
                continue;
            }
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
        // A new page; one the buffer cannot fill ends the write and gives its slot back (measured:
        // the reader sees nothing and the pipe keeps all its room).
        if !with_pipe(file, |pipe| pipe.is_full()) {
            let (bytes, faulted) = next(kernel, cx, done, PAGE_SIZE);
            if faulted {
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
        // Readers hear of what came before each wait, and are woken for it (not again after a
        // wake that found no room: writers wait on the same channel, and two that found the
        // pipe full would wake each other for ever).
        let call = kernel.threads[&tid].call;
        let new = !call.started || call.progress != done;
        if new {
            signal_ends(kernel, cx, &node, true);
        }
        let thread = kernel.thread_mut(tid);
        thread.call.started = true;
        thread.call.progress = done;
        if new {
            kernel.wake_poll(channel(&node), EPOLLIN | EPOLLRDNORM);
        }
        kernel.park(tid, channel(&node));
        return Err(Errno::PARKED);
    };
    if done > 0 {
        kernel.wake_poll(channel(&node), EPOLLIN | EPOLLRDNORM);
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
/// of which a write adds to. Readers with `FASYNC` hear of them.
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
        let mut piece = (PAGE_SIZE - in_page % PAGE_SIZE) as usize;
        while done < bytes.len() && !pipe.is_full() {
            let end = (done + piece).min(bytes.len());
            pipe.push_spliced(bytes[done..end].to_vec());
            done = end;
            piece = PAGE_SIZE as usize;
        }
        done
    });
    kernel.wake_poll(channel(&node), EPOLLIN | EPOLLRDNORM);
    // `splice_file_to_pipe`'s `wakeup_pipe_readers`.
    if taken > 0 {
        signal_ends(kernel, cx, &node, true);
    }
    taken as u64
}

/// The outside's bytes into a pipe whose writer its end holds, within its credit: put in as far as
/// there is room and the readers woken, the rest kept until there is; with no reader left they are
/// dropped, as a write with none fails. Returns how many were past the credit, which are dropped.
pub(crate) fn from_outside(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    file: &FileRef,
    bytes: &[u8],
) -> usize {
    let node = file.borrow().node.clone();
    let (taken, delivered) = with_pipe(file, |pipe| {
        let taken = bytes
            .len()
            .min(usize::try_from(pipe.promised).unwrap_or(usize::MAX));
        pipe.promised -= taken as u64;
        if pipe.readers() == 0 {
            return (taken, 0);
        }
        // Bytes kept from before go in first.
        let delivered = match pipe.inbox.is_empty() {
            true => pipe.deliver(&bytes[..taken]),
            false => 0,
        };
        pipe.inbox.extend_from_slice(&bytes[delivered..taken]);
        (taken, delivered)
    });
    if delivered > 0 {
        woken_by_outside(kernel, cx, &node);
    }
    bytes.len() - taken
}

/// Moves the outside's kept bytes into a pipe whose writer its end holds, as far as there is room
/// (all are dropped once no reader is left). Returns whether any went in or were dropped.
pub(crate) fn from_inbox(kernel: &mut Kernel, cx: &mut Context<'_>, file: &FileRef) -> bool {
    let node = file.borrow().node.clone();
    let (delivered, dropped) = with_pipe(file, |pipe| match pipe.readers() {
        0 => (0, core::mem::take(&mut pipe.inbox).len()),
        _ => (pipe.deliver_inbox(), 0),
    });
    if delivered > 0 {
        woken_by_outside(kernel, cx, &node);
    }
    delivered + dropped > 0
}

fn woken_by_outside(kernel: &mut Kernel, cx: &mut Context<'_>, node: &Node) {
    kernel.wake_poll(channel(node), EPOLLIN | EPOLLRDNORM);
    signal_ends(kernel, cx, node, true);
}

/// Credit to give the outside writing a pipe: the room its writes can take past what it may
/// already send or has sent and waits, once that is worth a credit (a page, or any room with
/// nothing outstanding).
pub(crate) fn promise_outside(file: &FileRef) -> u64 {
    with_pipe(file, |pipe| {
        let outstanding = pipe.promised + pipe.inbox.len() as u64;
        let room = pipe.room_outside().saturating_sub(outstanding);
        if room < PAGE_SIZE && (outstanding > 0 || room == 0) {
            return 0;
        }
        pipe.promised += room;
        room
    })
}

/// Whether a pipe the outside writes has no reader left.
pub(crate) fn no_reader(file: &FileRef) -> bool {
    with_pipe(file, |pipe| pipe.readers() == 0)
}

/// Whether the outside's bytes kept for a pipe are all in.
pub(crate) fn inbox_empty(file: &FileRef) -> bool {
    with_pipe(file, |pipe| pipe.inbox.is_empty())
}

/// Up to `most` bytes of a pipe whose reader an end holds, taken as a read takes them, writers
/// woken; nothing while another call holds the pipe's lock.
pub(crate) fn to_outside(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    file: &FileRef,
    most: u64,
) -> Vec<u8> {
    let node = file.borrow().node.clone();
    let holder = with_pipe(file, |pipe| pipe.held_by);
    if kernel.held_by_other(NO_THREAD, holder, channel(&node)) {
        return Vec::new();
    }
    let most = usize::try_from(most).unwrap_or(usize::MAX);
    let bytes = with_pipe(file, |pipe| pipe.take_into(most, most).0);
    if !bytes.is_empty() {
        kernel.wake_poll(channel(&node), EPOLLOUT | EPOLLWRNORM);
        signal_ends(kernel, cx, &node, false);
    }
    bytes
}

/// Whether a pipe whose reader an end holds is finished: empty, with no writer left.
pub(crate) fn drained(file: &FileRef) -> bool {
    with_pipe(file, |pipe| pipe.is_empty() && pipe.writers() == 0)
}
