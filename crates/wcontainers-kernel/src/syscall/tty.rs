//! Terminal files (`tty_io.c`, `tty_ioctl.c`, `tty_jobctrl.c` and the pseudo-terminal parts of
//! `pty.c` and devpts): opening `/dev/ptmx`, a slave and `/dev/tty`, reading and writing with the
//! line discipline's waits, job control checks, the requests, controlling terminals, and what a
//! side's last close does.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use wcontainers_abi::Permissions;

use super::guest;
use crate::credentials::CAP_SYS_ADMIN;
use crate::file::{FileRef, OpenFile, TerminalEnd};
use crate::fs::{self, Content, New, Node};
use crate::linux::general::*;
use crate::linux::ioctl::*;
use crate::linux_extra::{N_MOUSE, N_TTY, NR_LDISCS, PTY_LIMIT, SIG_IGN, UNIX98_PTY_SLAVE_MAJOR};
use crate::process::{Channel, Pid, Tid};
use crate::tty::{MASTER, Pty, SLAVE, load_speeds};
use crate::{Context, Errno, Kernel};

pub(crate) type PtyRef = Rc<RefCell<Pty>>;

/// Docker mounts devpts with `gid=5,mode=620`: slaves belong to the tty group.
const TTY_GROUP: u32 = 5;
const SLAVE_MODE: u32 = 0o620;
/// `iterate_tty_read` takes from the line discipline this much at a time.
const READ_CHUNK: u64 = 64;
/// `iterate_tty_write` hands the line discipline this much at a time.
const WRITE_CHUNK: u64 = 2048;

/// The channel a terminal's waiters park on.
pub(crate) fn channel(pty: &PtyRef) -> Channel {
    Channel::Object(Rc::as_ptr(pty) as *const () as usize)
}

/// The terminal side behind an open file, if it is one: the pair, the side and whether the file
/// was hung up.
pub(crate) fn end(file: &FileRef) -> Option<(PtyRef, usize, bool)> {
    let file = file.borrow();
    let end = file.terminal.as_ref()?;
    Some((end.pty.clone(), end.side, end.hung_up))
}

/// Opens a terminal device on an open file not yet in a table (`tty_open`): `/dev/ptmx` makes a
/// pair, a devpts node opens its slave, `/dev/tty` the caller's controlling terminal.
pub(super) fn open(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    device: crate::fs::dev::Device,
    flags: u32,
) -> Result<(), Errno> {
    use crate::fs::dev::Device;
    match device {
        Device::Ptmx => open_master(kernel, cx, tid, file),
        Device::Tty => {
            let pty = kernel
                .process_of(tid)
                .terminal
                .clone()
                .ok_or(Errno::ENXIO)?;
            open_slave(kernel, cx, tid, file, &pty, flags)
        }
        Device::Pts => {
            let node = file.borrow().node.clone();
            let pty = kernel
                .files
                .terminals
                .values()
                .find(|pty| {
                    let pty = pty.borrow();
                    !pty.closed[MASTER] && Rc::ptr_eq(&pty.slave_node, &node)
                })
                .cloned()
                .ok_or(Errno::EIO)?;
            open_slave(kernel, cx, tid, file, &pty, flags)
        }
        _ => unreachable!("only terminals open here"),
    }
}

/// `ptmx_open`: a new pair with the lowest free number, its slave node in devpts, locked.
fn open_master(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
) -> Result<(), Errno> {
    if kernel.files.terminals.len() as u32 + 1 >= PTY_LIMIT {
        return Err(Errno::ENOSPC);
    }
    let index = (0..)
        .find(|index| !kernel.files.terminals.contains_key(index))
        .expect("a free number");
    let devpts = file
        .borrow()
        .node
        .borrow()
        .place
        .parent
        .as_ref()
        .and_then(|parent| parent.upgrade())
        .expect("ptmx lives in devpts");
    let now = kernel.now(cx);
    let fsuid = kernel.process_of(tid).credentials.fsuid;
    let name = alloc::format!("{index}").into_bytes();
    let fs = devpts.borrow().fs.clone();
    let slave_node = fs::new_inode(
        fs,
        u64::from(index) + 3,
        New {
            mode: S_IFCHR | SLAVE_MODE,
            uid: fsuid,
            gid: TTY_GROUP,
            rdev: fs::encode_device(UNIX98_PTY_SLAVE_MAJOR, index),
            now,
            content: Content::Special,
            parent: Some(devpts.clone()),
            name: name.clone(),
        },
    )?;
    devpts
        .borrow_mut()
        .directory_mut()
        .expect("devpts is a directory")
        .insert(&name, slave_node.clone());
    let pty = Rc::new(RefCell::new(Pty::new(index, slave_node)));
    pty.borrow_mut().files[MASTER].push(Rc::downgrade(file));
    file.borrow_mut().terminal = Some(TerminalEnd {
        pty: pty.clone(),
        side: MASTER,
        hung_up: false,
    });
    kernel.files.terminals.insert(index, pty);
    Ok(())
}

/// `tty_reopen` and `pty_open` for the slave, then the controlling terminal as `tty_open` sets it.
fn open_slave(
    kernel: &mut Kernel,
    _cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    pty: &PtyRef,
    flags: u32,
) -> Result<(), Errno> {
    {
        let mut pair = pty.borrow_mut();
        let admin = kernel.process_of(tid).credentials.has(CAP_SYS_ADMIN);
        if pair.sides[SLAVE].exclusive && !admin {
            return Err(Errno::EBUSY);
        }
        if pair.sides[SLAVE].other_closed || pair.locked || pair.count(MASTER) != 1 {
            pair.sides[SLAVE].io_error = true;
            return Err(Errno::EIO);
        }
        pair.sides[SLAVE].io_error = false;
        pair.sides[MASTER].other_closed = false;
        pair.closed[SLAVE] = false;
        pair.files[SLAVE].retain(|end| end.strong_count() > 0);
        pair.files[SLAVE].push(Rc::downgrade(file));
    }
    file.borrow_mut().terminal = Some(TerminalEnd {
        pty: pty.clone(),
        side: SLAVE,
        hung_up: false,
    });
    // `tty_open_proc_set_tty`: a session leader with no terminal takes one no session has, if it
    // opened it for reading.
    let readable = file.borrow().readable();
    let process = kernel.process_of(tid);
    if flags & O_NOCTTY == 0
        && process.session == process.pid
        && process.terminal.is_none()
        && pty.borrow().sides[SLAVE].session.is_none()
        && readable
    {
        set_controlling(kernel, tid, pty);
    }
    Ok(())
}

/// `__proc_set_tty`: the slave becomes the caller's session's terminal, its group in front.
fn set_controlling(kernel: &mut Kernel, tid: Tid, pty: &PtyRef) {
    let process = kernel.process_of_mut(tid);
    let (group, session) = (process.group, process.session);
    process.terminal = Some(pty.clone());
    process.terminal_old_group = None;
    let mut pair = pty.borrow_mut();
    pair.sides[SLAVE].pgrp = Some(group);
    pair.sides[SLAVE].session = Some(session);
}

/// Whether the caller's controlling terminal is this side.
fn is_controlling(kernel: &Kernel, tid: Tid, pty: &PtyRef, side: usize) -> bool {
    side == SLAVE
        && kernel
            .process_of(tid)
            .terminal
            .as_ref()
            .is_some_and(|terminal| Rc::ptr_eq(terminal, pty))
}

/// `__tty_check_change`: a background process using its controlling terminal is sent `signal`
/// (its group is), unless it blocks or ignores it, or its group is orphaned.
fn check_change(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    pty: &PtyRef,
    side: usize,
    signal: u32,
) -> Result<(), Errno> {
    if !is_controlling(kernel, tid, pty, side) {
        return Ok(());
    }
    let group = kernel.process_of(tid).group;
    let Some(foreground) = pty.borrow().sides[SLAVE].pgrp else {
        return Ok(());
    };
    if foreground == group {
        return Ok(());
    }
    let blocked = kernel.threads[&tid].signal_mask & crate::signal::bit(signal) != 0;
    let ignored = kernel.process_of(tid).actions[signal as usize - 1].sa_handler_kernel == SIG_IGN;
    if blocked || ignored {
        return if signal == SIGTTIN {
            Err(Errno::EIO)
        } else {
            Ok(())
        };
    }
    if kernel.group_orphaned(group) {
        return Err(Errno::EIO);
    }
    kernel.send_to_group(cx, group, signal, crate::signal::kernel_info(signal));
    Err(Errno::ERESTARTSYS)
}

/// Sends the signals keys and size changes raised, and wakes whoever waits on the pair.
fn after(kernel: &mut Kernel, cx: &mut Context<'_>, pty: &PtyRef) {
    let signals = core::mem::take(&mut pty.borrow_mut().signals);
    for (group, signal) in signals {
        kernel.send_to_group(cx, group, signal, crate::signal::kernel_info(signal));
    }
    // Input became readable: the I/O signal of each file of the side with `FASYNC` set.
    let sides = core::mem::take(&mut pty.borrow_mut().input_signals);
    for side in sides {
        let files: Vec<FileRef> = pty.borrow().files[side]
            .iter()
            .filter_map(|file| file.upgrade())
            .collect();
        for file in files {
            kernel.send_io_signal(cx, &file, POLL_IN);
        }
    }
    kernel.wake(channel(pty));
}

/// `tty_update_time`: a read or write moves the access or change time of every file's inode on
/// the side, in steps of 8 seconds.
fn update_time(kernel: &mut Kernel, cx: &mut Context<'_>, pty: &PtyRef, side: usize, change: bool) {
    let now = kernel.now(cx);
    let nodes: Vec<Node> = pty.borrow().files[side]
        .iter()
        .filter_map(|file| file.upgrade())
        .map(|file| file.borrow().node.clone())
        .collect();
    for node in nodes {
        let mut inode = node.borrow_mut();
        let time = if change {
            &mut inode.mtime
        } else {
            &mut inode.atime
        };
        if (now.seconds ^ time.seconds) & !7 != 0 {
            time.seconds = now.seconds;
            time.nanoseconds = 0;
        }
    }
}

/// `tty_read` and `n_tty_read` into the given segments.
pub(super) fn read(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    read_into(kernel, cx, tid, file, parts, None)
}

/// A read of up to `count` bytes into a kernel buffer, as sendfile's splicing reads
/// (`copy_splice_read`).
pub(super) fn take(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    count: u64,
) -> Result<Vec<u8>, Errno> {
    let mut out = Vec::new();
    read_into(kernel, cx, tid, file, &[(0, count)], Some(&mut out))?;
    Ok(out)
}

/// A read into the guest's segments, or into `kernel_buffer` (one segment, its address unused).
fn read_into(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
    mut kernel_buffer: Option<&mut Vec<u8>>,
) -> Result<u64, Errno> {
    let (pty, side, hung_up) = end(file).expect("a terminal");
    if pty.borrow().sides[side].io_error {
        return Err(Errno::EIO);
    }
    if hung_up {
        return Ok(0);
    }
    check_change(kernel, cx, tid, &pty, side, SIGTTIN)?;
    let total: u64 = parts.iter().map(|(_, length)| length).sum();
    if total == 0 {
        return Ok(0);
    }
    let nonblocking = file.borrow().flags & O_NONBLOCK != 0;
    let now = cx.host.monotonic_nanoseconds();
    let (packet, canonical, (vmin, vtime), available, status) = {
        let pair = pty.borrow();
        let tty = &pair.sides[side];
        (
            tty.packet,
            tty.canonical(),
            tty.read_limits(),
            tty.input_available(false),
            pair.sides[1 - side].pktstatus,
        )
    };
    // The first status change a packet-mode reader has not seen comes alone.
    if packet && status != 0 {
        pty.borrow_mut().sides[1 - side].pktstatus = 0;
        match kernel_buffer {
            Some(out) => out.push(status),
            None => guest(kernel, cx, tid).write(parts[0].0, &[status])?,
        }
        return Ok(1);
    }
    // Non-canonical reads wait for `VMIN` bytes, or with no minimum for `VTIME` tenths of a
    // second at most; with both, `VTIME` is the longest gap between bytes.
    let tenths = u64::from(vtime) * 100_000_000;
    let minimum = if canonical {
        1
    } else {
        usize::from(vmin).max(1)
    };
    let queued = pty.borrow().sides[side].queued() as u64;
    let enough = available && (canonical || queued as usize >= minimum.min(total as usize));
    if !enough {
        let thread = kernel.thread_mut(tid);
        let first = !thread.call.started;
        thread.call.started = true;
        let call = thread.call;
        let timed_out = call.deadline.is_some_and(|deadline| deadline <= now);
        let partial = available && !canonical;
        if partial && (timed_out || nonblocking || kernel.signal_pending(tid)) {
            // What has come is taken.
        } else {
            if pty.borrow().sides[side].other_closed {
                return Err(Errno::EIO);
            }
            if !canonical && vmin == 0 && (vtime == 0 || timed_out) {
                return Ok(0);
            }
            if nonblocking {
                return Err(Errno::EAGAIN);
            }
            if kernel.signal_pending(tid) {
                return Err(Errno::ERESTARTSYS);
            }
            let thread = kernel.thread_mut(tid);
            if !canonical && vtime != 0 {
                if vmin == 0 {
                    if first {
                        thread.call.deadline = Some(now + tenths);
                    }
                } else if partial && (first || thread.call.progress != queued) {
                    thread.call.deadline = Some(now + tenths);
                }
            }
            thread.call.progress = queued;
            kernel.park(tid, channel(&pty));
            return Err(Errno::PARKED);
        }
    }
    // Only as far as the buffer can take: the line discipline gives 64 bytes at a time, and the
    // piece a fault cuts short is lost.
    let mut guest = guest(kernel, cx, tid);
    let mut room = 0;
    for &(base, length) in parts {
        let writable = if kernel_buffer.is_some() {
            length
        } else {
            guest.accessible(base, length, Permissions::WRITE)
        };
        room += writable;
        if writable < length {
            break;
        }
    }
    let limit = if room < total {
        total.min((room / READ_CHUNK + 1) * READ_CHUNK)
    } else {
        total
    };
    let mut bytes = Vec::new();
    let room_made = {
        let mut pair = pty.borrow_mut();
        pair.sides[side].take(&mut bytes, limit as usize, packet);
        // `n_tty_check_unthrottle` for a pty: once little is left, the other side's writers hear
        // of room (`tty_wakeup`), before the buffer's work moves more in.
        let other = 1 - side;
        let wakeup = pair.sides[side].chars_in_buffer()
            <= crate::linux_extra::TTY_THRESHOLD_UNTHROTTLE
            && core::mem::take(&mut pair.sides[other].write_wakeup);
        pair.settle();
        wakeup.then_some(other)
    };

    // Each piece is copied from the kernel's buffer with arm64's copy routine (`copy_to_iter`),
    // segment by segment, until a copy falls short.
    let mut copied = 0u64;
    if let Some(out) = kernel_buffer.as_deref_mut() {
        out.extend_from_slice(&bytes);
        copied = bytes.len() as u64;
    }
    for piece in bytes.chunks(READ_CHUNK as usize) {
        if kernel_buffer.is_some() {
            break;
        }
        let stored = super::io::copy_to_segments(&mut guest, parts, copied, piece, 0);
        copied += stored;
        if stored < piece.len() as u64 {
            break;
        }
    }
    if let Some(other) = room_made {
        for file in async_files(&pty, other) {
            kernel.send_io_signal(cx, &file, POLL_OUT);
        }
    }
    after(kernel, cx, &pty);
    if copied == 0 && !bytes.is_empty() {
        return Err(Errno::EFAULT);
    }
    if copied > 0 {
        update_time(kernel, cx, &pty, side, false);
    }
    Ok(copied)
}

/// `tty_write` and `n_tty_write` from the given segments, 2048 bytes at a time, keeping what is
/// written in the call's slot while it waits for room.
pub(super) fn write(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    let (pty, side, hung_up) = end(file).expect("a terminal");
    if pty.borrow().sides[side].io_error || hung_up {
        return Err(Errno::EIO);
    }
    let total: u64 = parts.iter().map(|(_, length)| length).sum();
    if pty.borrow().sides[side].tostop() {
        check_change(kernel, cx, tid, &pty, side, SIGTTOU)?;
    }
    let nonblocking = file.borrow().flags & O_NONBLOCK != 0;
    let mut done = kernel.threads[&tid].call.progress;
    pty.borrow_mut().process_echoes(side);
    let result: Result<(), Errno> = loop {
        if done == total {
            break Ok(());
        }
        if kernel.signal_pending(tid) {
            break Err(Errno::ERESTARTSYS);
        }
        if side == SLAVE && pty.borrow().count(MASTER) == 0 {
            break Err(Errno::EIO);
        }
        // The next chunk is copied in whole first; a fault in it writes none of it.
        let size = (total - done).min(WRITE_CHUNK);
        let mut bytes = Vec::with_capacity(size as usize);
        let mut skipped = 0;
        let mut guest = guest(kernel, cx, tid);
        let mut faulted = false;
        for &(base, length) in parts {
            let end = skipped + length;
            let at = done + bytes.len() as u64;
            if at < end && (bytes.len() as u64) < size {
                let from = at - skipped;
                let wanted = (length - from).min(size - bytes.len() as u64);
                if guest.accessible(base + from, wanted, Permissions::READ) < wanted {
                    faulted = true;
                    break;
                }
                bytes.extend(guest.read(base + from, wanted)?);
            }
            skipped = end;
        }
        if faulted {
            break Err(Errno::EFAULT);
        }
        let written = pty.borrow_mut().write(side, &bytes) as u64;
        done += written;
        if written == size {
            continue;
        }
        if nonblocking {
            break Err(Errno::EAGAIN);
        }
        kernel.thread_mut(tid).call.progress = done;
        after(kernel, cx, &pty);
        kernel.park(tid, channel(&pty));
        return Err(Errno::PARKED);
    };
    // A write that ended with bytes left asks to hear of room (`n_tty_write`'s end).
    if done < total && !async_files(&pty, side).is_empty() {
        pty.borrow_mut().sides[side].write_wakeup = true;
    }
    after(kernel, cx, &pty);
    if done > 0 {
        update_time(kernel, cx, &pty, side, true);
        return Ok(done);
    }
    result.map(|()| 0)
}

/// Kernel bytes into a terminal, as sendfile's and splice's `iter_file_splice_write` hands them to
/// `tty_write`: what fits now. A caller left with bytes waits through [`wait_for_room`].
pub(super) fn put(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    bytes: &[u8],
) -> Result<u64, Errno> {
    let (pty, side, hung_up) = end(file).expect("a terminal");
    if pty.borrow().sides[side].io_error || hung_up {
        return Err(Errno::EIO);
    }
    if pty.borrow().sides[side].tostop() {
        check_change(kernel, cx, tid, &pty, side, SIGTTOU)?;
    }
    if kernel.signal_pending(tid) {
        return Err(Errno::ERESTARTSYS);
    }
    if side == SLAVE && pty.borrow().count(MASTER) == 0 {
        return Err(Errno::EIO);
    }
    pty.borrow_mut().process_echoes(side);
    let mut done = 0;
    for chunk in bytes.chunks(WRITE_CHUNK as usize) {
        let written = pty.borrow_mut().write(side, chunk);
        done += written;
        if written < chunk.len() {
            break;
        }
    }
    if done < bytes.len() && !async_files(&pty, side).is_empty() {
        pty.borrow_mut().sides[side].write_wakeup = true;
    }
    after(kernel, cx, &pty);
    if done > 0 {
        update_time(kernel, cx, &pty, side, true);
    }
    Ok(done as u64)
}

/// `n_tty_write` with no room left: EAGAIN for a nonblocking file, else the thread parks until the
/// terminal has room, with `done` bytes of its call written.
pub(super) fn wait_for_room(kernel: &mut Kernel, tid: Tid, file: &FileRef, done: u64) -> Errno {
    if file.borrow().flags & O_NONBLOCK != 0 {
        return Errno::EAGAIN;
    }
    let (pty, ..) = end(file).expect("a terminal");
    kernel.thread_mut(tid).call.progress = done;
    kernel.park(tid, channel(&pty));
    Errno::PARKED
}

/// The open files of a side with `FASYNC` set (`tty->fasync`).
fn async_files(pty: &PtyRef, side: usize) -> Vec<FileRef> {
    pty.borrow().files[side]
        .iter()
        .filter_map(|file| file.upgrade())
        .filter(|file| file.borrow().flags & FASYNC != 0)
        .collect()
}

/// What `poll` reports for a terminal file (`n_tty_poll`, or a hung-up file's).
pub(super) fn poll(file: &FileRef) -> u16 {
    let (pty, side, hung_up) = end(file).expect("a terminal");
    if hung_up {
        return (POLLIN | POLLOUT | POLLERR | POLLHUP | POLLRDNORM | POLLWRNORM) as u16;
    }
    let pair = pty.borrow();
    let tty = &pair.sides[side];
    let mut mask = 0;
    if tty.input_available(true) {
        mask |= POLLIN | POLLRDNORM;
    }
    if tty.packet && pair.sides[1 - side].pktstatus != 0 {
        mask |= POLLPRI | POLLIN | POLLRDNORM;
    }
    if tty.other_closed {
        mask |= POLLHUP;
    }
    // A pseudo-terminal keeps nothing queued for output, which is below `WAKEUP_CHARS`.
    if pair.write_room(side) > 0 {
        mask |= POLLOUT | POLLWRNORM;
    }
    mask as u16
}

/// The foreground group of a terminal file's side.
pub(super) fn foreground_group(file: &FileRef) -> Option<crate::process::Pid> {
    let (pty, side, _) = end(file)?;
    pty.borrow().sides[side].pgrp
}

/// `tty_fasync`: whether `FIOASYNC` and `F_SETFL`'s `O_ASYNC` may be set.
pub(super) fn can_async(file: &FileRef) -> bool {
    end(file).is_some_and(|(_, _, hung_up)| !hung_up)
}

/// `tty_ioctl`: the requests of every terminal, then the pseudo-terminal master's own
/// (`pty_unix98_ioctl`), then the line discipline's (`n_tty_ioctl`) and the settings
/// (`tty_mode_ioctl`).
pub(super) fn ioctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    let (pty, side, hung_up) = end(file).expect("a terminal");
    if hung_up {
        return Err(if request == TIOCSPGRP {
            Errno::ENOTTY
        } else {
            Errno::EIO
        });
    }
    let answer = requests(kernel, cx, tid, file, &pty, side, request, argument);
    pty.borrow_mut().settle();
    after(kernel, cx, &pty);
    answer
}

#[allow(
    clippy::too_many_arguments,
    reason = "every request names its file and side"
)]
fn requests(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    pty: &PtyRef,
    side: usize,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    // A master's settings, size and job control are its slave's (`tty_pair_get_tty`).
    let real = SLAVE;
    let admin = kernel.process_of(tid).credentials.has(CAP_SYS_ADMIN);
    if matches!(request, TIOCSETD | TIOCSBRK | TIOCCBRK | TCSBRK | TCSBRKP) {
        check_change(kernel, cx, tid, pty, side, SIGTTOU)?;
    }
    let put_int = |kernel: &mut Kernel, cx: &mut Context<'_>, value: i32| {
        guest(kernel, cx, tid)
            .put(argument, &value.to_le_bytes())
            .map(|()| 0)
    };
    let get_int = |kernel: &mut Kernel, cx: &mut Context<'_>| {
        guest(kernel, cx, tid).read_value::<i32>(argument)
    };
    match request {
        TIOCSTI => {
            if !is_controlling(kernel, tid, pty, side) && !admin {
                return Err(Errno::EPERM);
            }
            let byte: u8 = guest(kernel, cx, tid).read_value(argument)?;
            pty.borrow_mut().receive(side, &[byte], false);
            Ok(0)
        }
        TIOCGWINSZ => {
            let size = pty.borrow().sides[real].winsize;
            guest(kernel, cx, tid)
                .write_value(argument, &size)
                .map(|()| 0)
        }
        TIOCSWINSZ => {
            let size: winsize = guest(kernel, cx, tid).read_value(argument)?;
            pty.borrow_mut().resize(size);
            Ok(0)
        }
        TIOCCONS if side == MASTER => Err(Errno::EINVAL),
        // `tioccons` redirects the console, which needs CAP_SYS_ADMIN.
        TIOCCONS if !admin => Err(Errno::EPERM),
        TIOCCONS => Err(kernel.counters.refuse(
            cx.host,
            crate::counters::reason::IOCTL_NOT_BUILT,
            request.into(),
            Errno::ENOTTY,
        )),
        TIOCEXCL | TIOCNXCL => {
            pty.borrow_mut().sides[side].exclusive = request == TIOCEXCL;
            Ok(0)
        }
        TIOCGEXCL => {
            let exclusive = pty.borrow().sides[side].exclusive;
            put_int(kernel, cx, i32::from(exclusive))
        }
        TIOCGETD => put_int(kernel, cx, N_TTY as i32),
        // `tty_set_ldisc`: the discipline is looked up, then the current one is kept as it is, then
        // the new one's open runs.
        TIOCSETD => match get_int(kernel, cx)? {
            discipline if !(N_TTY as i32..NR_LDISCS).contains(&discipline) => Err(Errno::EINVAL),
            discipline if discipline == N_TTY as i32 => Ok(0),
            N_MOUSE if !admin => Err(Errno::EPERM),
            discipline => Err(kernel.counters.refuse(
                cx.host,
                crate::counters::reason::LDISC_NOT_BUILT,
                discipline as u64,
                Errno::EINVAL,
            )),
        },
        TIOCVHANGUP if !admin => Err(Errno::EPERM),
        TIOCVHANGUP => {
            hang_up(kernel, cx, pty, side);
            Ok(0)
        }
        TIOCGDEV => {
            let index = pty.borrow().index;
            let number = (index & 0xff) | (UNIX98_PTY_SLAVE_MAJOR << 8) | ((index & !0xff) << 12);
            put_int(kernel, cx, number as i32)
        }
        // A pseudo-terminal has no break, modem lines, counters or serial settings.
        TIOCSBRK | TIOCCBRK | TCSBRK | TCSBRKP => Ok(0),
        TIOCMGET | TIOCMSET | TIOCMBIC | TIOCMBIS | TIOCGICOUNT | TIOCSSERIAL | TIOCGSERIAL => {
            Err(Errno::ENOTTY)
        }
        TIOCGPTPEER if side == MASTER => open_peer(kernel, cx, tid, pty, argument as u32),
        TIOCGPTPEER => Err(Errno::EIO),
        TIOCNOTTY => {
            if !is_controlling(kernel, tid, pty, side) {
                return Err(Errno::ENOTTY);
            }
            let pid = kernel.process_of(tid).pid;
            disassociate(kernel, cx, pid, false);
            kernel.process_of_mut(tid).terminal = None;
            Ok(0)
        }
        TIOCSCTTY => set_controlling_request(kernel, tid, file, pty, argument, admin),
        TIOCGPGRP | TIOCGSID => {
            if side == SLAVE && !is_controlling(kernel, tid, pty, side) {
                return Err(Errno::ENOTTY);
            }
            let tty = &pty.borrow().sides[real];
            let value = if request == TIOCGPGRP {
                tty.pgrp.unwrap_or(0)
            } else {
                tty.session.ok_or(Errno::ENOTTY)?
            };
            put_int(kernel, cx, value)
        }
        TIOCSPGRP => {
            match check_change(kernel, cx, tid, pty, real, SIGTTOU) {
                Err(Errno::EIO) => return Err(Errno::ENOTTY),
                Err(error) => return Err(error),
                Ok(()) => {}
            }
            let group = get_int(kernel, cx)?;
            if group < 0 {
                return Err(Errno::EINVAL);
            }
            let process = kernel.process_of(tid);
            if !is_controlling(kernel, tid, pty, real)
                || pty.borrow().sides[real].session != Some(process.session)
            {
                return Err(Errno::ENOTTY);
            }
            // `session_of_pgrp`: the group's session, or the session of a process of that
            // number; a number only a session uses has neither.
            let session = kernel
                .processes
                .values()
                .find(|other| other.group == group)
                .or_else(|| kernel.processes.get(&group))
                .map(|other| other.session);
            if session.is_none()
                && !kernel
                    .processes
                    .values()
                    .any(|other| other.session == group)
            {
                return Err(Errno::ESRCH);
            }
            if session != Some(process.session) {
                return Err(Errno::EPERM);
            }
            pty.borrow_mut().sides[real].pgrp = Some(group);
            Ok(0)
        }
        // The master's own requests.
        TIOCSPTLCK if side == MASTER => {
            let locked = get_int(kernel, cx)?;
            pty.borrow_mut().locked = locked != 0;
            Ok(0)
        }
        TIOCGPTLCK if side == MASTER => {
            let locked = pty.borrow().locked;
            put_int(kernel, cx, i32::from(locked))
        }
        TIOCPKT if side == MASTER => {
            let on = get_int(kernel, cx)?;
            let mut pair = pty.borrow_mut();
            if on == 0 {
                pair.sides[MASTER].packet = false;
            } else if !pair.sides[MASTER].packet {
                pair.sides[SLAVE].pktstatus = 0;
                pair.sides[MASTER].packet = true;
            }
            Ok(0)
        }
        TIOCGPKT if side == MASTER => {
            let packet = pty.borrow().sides[MASTER].packet;
            put_int(kernel, cx, i32::from(packet))
        }
        TIOCGPTN if side == MASTER => {
            let index = pty.borrow().index;
            put_int(kernel, cx, index as i32)
        }
        TIOCSIG if side == MASTER => {
            let signal = argument as u32;
            if !matches!(signal, SIGINT | SIGQUIT | SIGTSTP) {
                return Err(Errno::EINVAL);
            }
            let group = pty.borrow().sides[SLAVE].pgrp;
            if let Some(group) = group {
                kernel.send_to_group(cx, group, signal, crate::signal::kernel_info(signal));
            }
            Ok(0)
        }
        // The line discipline's requests.
        TIOCOUTQ => put_int(kernel, cx, 0),
        TIOCINQ => {
            let queued = pty.borrow().sides[side].queued();
            put_int(kernel, cx, queued as i32)
        }
        TCXONC => {
            check_change(kernel, cx, tid, pty, side, SIGTTOU)?;
            let mut pair = pty.borrow_mut();
            let control = |pair: &Pty, index: u32| pair.sides[side].termios.c_cc[index as usize];
            match argument as u32 {
                TCOOFF => pair.output_off(side, true),
                TCOON => pair.output_off(side, false),
                TCIOFF => {
                    let stop = control(&pair, VSTOP);
                    if stop != 0 {
                        pair.send_xchar(side, stop);
                    }
                }
                TCION => {
                    let start = control(&pair, VSTART);
                    if start != 0 {
                        pair.send_xchar(side, start);
                    }
                }
                _ => return Err(Errno::EINVAL),
            }
            Ok(0)
        }
        TCFLSH => {
            check_change(kernel, cx, tid, pty, side, SIGTTOU)?;
            let mut pair = pty.borrow_mut();
            match argument as u32 {
                TCIFLUSH => pair.flush_input(side),
                TCIOFLUSH => {
                    pair.flush_input(side);
                    pair.flush_output(side);
                }
                TCOFLUSH => pair.flush_output(side),
                _ => return Err(Errno::EINVAL),
            }
            Ok(0)
        }
        _ => settings(kernel, cx, tid, pty, request, argument),
    }
}

/// `tty_mode_ioctl`: getting and setting the slave's settings.
fn settings(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    pty: &PtyRef,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    use zerocopy::{FromBytes, IntoBytes};
    let current = pty.borrow().sides[SLAVE].termios;
    let old_size = core::mem::size_of::<termios>();
    match request {
        TCGETS => guest(kernel, cx, tid)
            .write(argument, &current.as_bytes()[..old_size])
            .map(|()| 0),
        TCGETS2 => guest(kernel, cx, tid)
            .write(argument, current.as_bytes())
            .map(|()| 0),
        TCGETA => {
            // `struct termio`: 16 bit flags, the line and 8 control characters, padded to 18.
            let mut bytes = [0u8; 18];
            for (index, flag) in [
                current.c_iflag,
                current.c_oflag,
                current.c_cflag,
                current.c_lflag,
            ]
            .into_iter()
            .enumerate()
            {
                bytes[index * 2..index * 2 + 2].copy_from_slice(&(flag as u16).to_le_bytes());
            }
            bytes[8] = current.c_line;
            bytes[9..17].copy_from_slice(&current.c_cc[..8]);
            guest(kernel, cx, tid).write(argument, &bytes).map(|()| 0)
        }
        TCSETS | TCSETSW | TCSETSF | TCSETS2 | TCSETSW2 | TCSETSF2 | TCSETA | TCSETAW | TCSETAF => {
            check_change(kernel, cx, tid, pty, SLAVE, SIGTTOU)?;
            let mut new = current;
            if matches!(request, TCSETA | TCSETAW | TCSETAF) {
                let bytes = guest(kernel, cx, tid).read(argument, 18)?;
                let flag = |index: usize| {
                    u32::from(u16::from_le_bytes([bytes[index * 2], bytes[index * 2 + 1]]))
                };
                new.c_iflag = (new.c_iflag & 0xffff_0000) | flag(0);
                new.c_oflag = (new.c_oflag & 0xffff_0000) | flag(1);
                new.c_cflag = (new.c_cflag & 0xffff_0000) | flag(2);
                new.c_lflag = (new.c_lflag & 0xffff_0000) | flag(3);
                new.c_line = bytes[8];
                new.c_cc[..8].copy_from_slice(&bytes[9..17]);
            } else {
                let size = if matches!(request, TCSETS | TCSETSW | TCSETSF) {
                    old_size
                } else {
                    core::mem::size_of::<termios2>()
                };
                let bytes = guest(kernel, cx, tid).read(argument, size as u64)?;
                let mut whole = current.as_bytes().to_vec();
                whole[..size].copy_from_slice(&bytes);
                new = termios2::read_from_bytes(&whole).expect("termios2's size");
            }
            load_speeds(&mut new);
            // A change to the ADDRB bit is kept out (`tty_set_termios`).
            new.c_cflag ^= (new.c_cflag ^ current.c_cflag) & ADDRB;
            let mut pair = pty.borrow_mut();
            if matches!(request, TCSETSF | TCSETSF2 | TCSETAF) {
                pair.flush_input(SLAVE);
            }
            pair.set_termios(SLAVE, new);
            Ok(0)
        }
        TIOCGLCKTRMIOS => guest(kernel, cx, tid)
            .write(argument, &[0u8; 36][..old_size])
            .map(|()| 0),
        // `checkpoint_restore_ns_capable`: CAP_SYS_ADMIN or CAP_CHECKPOINT_RESTORE, neither of
        // which Docker gives.
        TIOCSLCKTRMIOS => {
            let credentials = &kernel.process_of(tid).credentials;
            if credentials.has(CAP_SYS_ADMIN) || credentials.has(CAP_CHECKPOINT_RESTORE) {
                return Err(kernel.counters.refuse(
                    cx.host,
                    crate::counters::reason::IOCTL_NOT_BUILT,
                    request.into(),
                    Errno::ENOTTY,
                ));
            }
            Err(Errno::EPERM)
        }
        TIOCGSOFTCAR => {
            let local = current.c_cflag & CLOCAL != 0;
            guest(kernel, cx, tid)
                .put(argument, &i32::from(local).to_le_bytes())
                .map(|()| 0)
        }
        TIOCSSOFTCAR => {
            let on: u32 = guest(kernel, cx, tid).read_value(argument)?;
            let mut new = current;
            new.c_cflag &= !CLOCAL;
            if on != 0 {
                new.c_cflag |= CLOCAL;
            }
            pty.borrow_mut().set_termios(SLAVE, new);
            Ok(0)
        }
        _ => Err(Errno::ENOTTY),
    }
}

/// `TIOCSCTTY` (`tiocsctty`): a session leader with no terminal takes the slave.
fn set_controlling_request(
    kernel: &mut Kernel,
    tid: Tid,
    file: &FileRef,
    pty: &PtyRef,
    argument: u64,
    admin: bool,
) -> Result<u64, Errno> {
    let process = kernel.process_of(tid);
    let leader = process.session == process.pid;
    let session = pty.borrow().sides[SLAVE].session;
    if leader && session == Some(process.session) {
        return Ok(0);
    }
    if !leader || process.terminal.is_some() {
        return Err(Errno::EPERM);
    }
    if let Some(session) = session {
        if argument == 1 && admin {
            clear_session(kernel, session);
        } else {
            return Err(Errno::EPERM);
        }
    }
    if !file.borrow().readable() && !admin {
        return Err(Errno::EPERM);
    }
    set_controlling(kernel, tid, pty);
    Ok(0)
}

/// `session_clear_tty`: no process of the session keeps a controlling terminal.
fn clear_session(kernel: &mut Kernel, session: Pid) {
    for process in kernel.processes.values_mut() {
        if process.session == session {
            process.terminal = None;
        }
    }
}

/// `disassociate_ctty`: a session leader lets its terminal go. Its foreground group is sent
/// SIGHUP, and SIGCONT unless the leader is ending; with no terminal, an ending leader's group
/// from before a hangup is sent both.
pub(crate) fn disassociate(kernel: &mut Kernel, cx: &mut Context<'_>, pid: Pid, on_exit: bool) {
    let process = &kernel.processes[&pid];
    if process.session != process.pid {
        return;
    }
    let session = process.session;
    let Some(pty) = process.terminal.clone() else {
        let old = kernel
            .processes
            .get_mut(&pid)
            .and_then(|process| process.terminal_old_group.take());
        if on_exit && let Some(group) = old {
            kernel.send_to_group(cx, group, SIGHUP, crate::signal::kernel_info(SIGHUP));
            kernel.send_to_group(cx, group, SIGCONT, crate::signal::kernel_info(SIGCONT));
        }
        return;
    };
    let group = pty.borrow().sides[SLAVE].pgrp;
    if let Some(group) = group {
        kernel.send_to_group(cx, group, SIGHUP, crate::signal::kernel_info(SIGHUP));
        if !on_exit {
            kernel.send_to_group(cx, group, SIGCONT, crate::signal::kernel_info(SIGCONT));
        }
    }
    {
        let mut pair = pty.borrow_mut();
        pair.sides[SLAVE].session = None;
        pair.sides[SLAVE].pgrp = None;
    }
    kernel
        .processes
        .get_mut(&pid)
        .expect("process")
        .terminal_old_group = None;
    clear_session(kernel, session);
}

/// `TIOCGPTPEER` (`ptm_open_peer`): the slave opened with `flags`, without a path lookup.
fn open_peer(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    pty: &PtyRef,
    flags: u32,
) -> Result<u64, Errno> {
    let node = pty.borrow().slave_node.clone();
    let index = pty.borrow().index;
    let path = alloc::format!("/dev/pts/{index}").into_bytes();
    let serial = kernel.process_of(tid).credentials.serial;
    let file = OpenFile::new(
        node,
        (flags | O_LARGEFILE) & !(O_CREAT | O_EXCL | O_NOCTTY | O_TRUNC | O_CLOEXEC),
        path,
        serial,
    );
    // An O_PATH file is made without the terminal's open, so even a locked slave is given
    // (`do_dentry_open`).
    if flags & O_PATH == 0 {
        open_slave(kernel, cx, tid, &file, pty, flags)?;
    }
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let fd = kernel
        .process_of_mut(tid)
        .files
        .add(file, flags & O_CLOEXEC != 0, 0, limit)?;
    Ok(fd as u64)
}

/// `__tty_hangup` of a side: its files are hung up, the session that has it loses it (its leader
/// sent SIGHUP and SIGCONT), and its job control is cleared.
fn hang_up(kernel: &mut Kernel, cx: &mut Context<'_>, pty: &PtyRef, side: usize) {
    let files: Vec<FileRef> = pty.borrow().files[side]
        .iter()
        .filter_map(|file| file.upgrade())
        .collect();
    for file in files {
        if let Some(end) = &mut file.borrow_mut().terminal {
            end.hung_up = true;
        }
    }
    let (session, group) = {
        let tty = &pty.borrow().sides[side];
        (tty.session, tty.pgrp)
    };
    if let Some(session) = session {
        let members: Vec<Pid> = kernel
            .processes
            .values()
            .filter(|process| process.session == session && process.ended.is_none())
            .map(|process| process.pid)
            .collect();
        for pid in members {
            let process = kernel.processes.get_mut(&pid).expect("member");
            if process
                .terminal
                .as_ref()
                .is_some_and(|terminal| Rc::ptr_eq(terminal, pty))
            {
                process.terminal = None;
            }
            if process.session != pid {
                continue;
            }
            process.terminal_old_group = group;
            kernel.send_to_process(cx, pid, SIGHUP, crate::signal::kernel_info(SIGHUP));
            kernel.send_to_process(cx, pid, SIGCONT, crate::signal::kernel_info(SIGCONT));
        }
    }
    let mut pair = pty.borrow_mut();
    pair.flush_input(side);
    let tty = &mut pair.sides[side];
    tty.session = None;
    tty.pgrp = None;
    tty.pktstatus = 0;
}

/// Deals with sides whose last file has closed (`tty_release` and `pty_close`): a slave's last
/// close tells the master; the master's hangs up the slave and takes its node away; once both
/// sides are closed, the number is free again.
pub(crate) fn released(kernel: &mut Kernel, cx: &mut Context<'_>) {
    let ptys: Vec<PtyRef> = kernel.files.terminals.values().cloned().collect();
    for pty in ptys {
        let (slave_open, master_open) = {
            let pair = pty.borrow();
            (pair.count(SLAVE), pair.count(MASTER))
        };
        let mut changed = false;
        if slave_open == 0 && !pty.borrow().closed[SLAVE] {
            changed = true;
            let mut pair = pty.borrow_mut();
            pair.closed[SLAVE] = true;
            if !pair.sides[SLAVE].io_error {
                let slave = &mut pair.sides[SLAVE];
                slave.io_error = true;
                slave.packet = false;
                pair.sides[MASTER].other_closed = true;
            }
        }
        if master_open == 0 && !pty.borrow().closed[MASTER] {
            changed = true;
            {
                let mut pair = pty.borrow_mut();
                pair.closed[MASTER] = true;
                let master = &mut pair.sides[MASTER];
                master.io_error = true;
                master.packet = false;
                master.other_closed = true;
                pair.sides[SLAVE].other_closed = true;
                let node = pair.slave_node.clone();
                drop(pair);
                remove_node(&node);
            }
            hang_up(kernel, cx, &pty, SLAVE);
        }
        let pair = pty.borrow();
        if pair.closed[MASTER] && pair.count(SLAVE) == 0 {
            let index = pair.index;
            drop(pair);
            kernel.files.terminals.remove(&index);
        }
        if changed {
            kernel.wake(channel(&pty));
        }
    }
}

/// `devpts_pty_kill`: the slave's name goes and its link count drops.
fn remove_node(node: &Node) {
    let (parent, name) = {
        let inode = node.borrow();
        (
            inode
                .place
                .parent
                .as_ref()
                .and_then(|parent| parent.upgrade()),
            inode.place.name.clone(),
        )
    };
    if let Some(parent) = parent
        && let Some(directory) = parent.borrow_mut().directory_mut()
    {
        directory.remove(&name);
    }
    let mut inode = node.borrow_mut();
    inode.nlink = inode.nlink.saturating_sub(1);
}
