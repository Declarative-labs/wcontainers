//! Moving bytes through pipes without the caller's memory, as Linux's `fs/splice.c`: splice, tee and
//! vmsplice. A call that must wait parks before it has moved anything, so running it again is exact.

use wcontainers_abi::{PAGE_SIZE, Permissions};

use super::io::{
    MAX_RW_COUNT, file_bytes, open_file, positioned, refresh_proc, verify_area, write_bytes,
    write_limit,
};
use super::pipe::{channel, signal_ends, with_pipe};
use super::{Args, guest};
use crate::counters::reason;
use crate::file::FileRef;
use crate::fs::Content;
use crate::fs::dev::Device;
use crate::linux::general::*;
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

const SPLICE_F_ALL: u32 = SPLICE_F_MOVE | SPLICE_F_NONBLOCK | SPLICE_F_MORE | SPLICE_F_GIFT;

/// Whether the file is a pipe or a named pipe's end (`get_pipe_info`).
fn is_pipe(file: &FileRef) -> bool {
    matches!(file.borrow().node.borrow().content, Content::Pipe(_))
}

fn same_pipe(a: &FileRef, b: &FileRef) -> bool {
    alloc::rc::Rc::ptr_eq(&a.borrow().node, &b.borrow().node)
}

fn sigpipe(kernel: &mut Kernel, cx: &mut Context<'_>, tid: Tid) {
    let process = kernel.process_of(tid);
    let info = crate::signal::sender_info(
        SIGPIPE,
        SI_USER as i32,
        process.pid,
        kernel.credentials(tid).uid,
    );
    kernel.send_to_thread(cx, tid, SIGPIPE, info);
}

fn park(kernel: &mut Kernel, tid: Tid, pipe: &FileRef) -> Errno {
    let node = pipe.borrow().node.clone();
    kernel.park(tid, channel(&node));
    Errno::PARKED
}

/// After bytes went into a pipe: its readers wake and hear of them (`wakeup_pipe_readers`).
fn readers_woken(kernel: &mut Kernel, cx: &mut Context<'_>, pipe: &FileRef) {
    let node = pipe.borrow().node.clone();
    kernel.wake_poll(channel(&node), EPOLLIN | EPOLLRDNORM);
    signal_ends(kernel, cx, &node, true);
}

/// After a pipe's slots went: its writers wake and hear of the room (`wakeup_pipe_writers`).
fn writers_woken(kernel: &mut Kernel, cx: &mut Context<'_>, pipe: &FileRef) {
    let node = pipe.borrow().node.clone();
    kernel.wake_poll(channel(&node), EPOLLOUT | EPOLLWRNORM);
    signal_ends(kernel, cx, &node, false);
}

/// `ipipe_prep`: waits until the input has bytes or no writer.
fn input_ready(
    kernel: &mut Kernel,
    tid: Tid,
    input: &FileRef,
    nonblock: bool,
) -> Result<(), Errno> {
    if !with_pipe(input, |pipe| pipe.is_empty()) {
        return Ok(());
    }
    if kernel.signal_pending(tid) {
        return Err(Errno::ERESTARTSYS);
    }
    if with_pipe(input, |pipe| pipe.writers()) == 0 {
        return Ok(());
    }
    if nonblock {
        return Err(Errno::EAGAIN);
    }
    Err(park(kernel, tid, input))
}

/// `opipe_prep` and `wait_for_space`: waits until the output has room; with no reader, SIGPIPE.
fn output_ready(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    output: &FileRef,
    nonblock: bool,
    readers_first: bool,
) -> Result<(), Errno> {
    let full = with_pipe(output, |pipe| pipe.host.is_none() && pipe.is_full());
    let readers = with_pipe(output, |pipe| pipe.readers());
    if readers == 0 && (readers_first || full) {
        sigpipe(kernel, cx, tid);
        return Err(Errno::EPIPE);
    }
    if !full {
        return Ok(());
    }
    if nonblock {
        return Err(Errno::EAGAIN);
    }
    if kernel.signal_pending(tid) {
        return Err(Errno::ERESTARTSYS);
    }
    Err(park(kernel, tid, output))
}

/// Puts a slot's bytes into the host's pipe, or the slot into a pipe the kernel holds.
fn into_output(cx: &mut Context<'_>, output: &FileRef, moved: impl FnOnce(&mut crate::pipe::Pipe)) {
    let node = output.borrow().node.clone();
    let mut inode = node.borrow_mut();
    let Content::Pipe(pipe) = &mut inode.content else {
        unreachable!("the file is a pipe");
    };
    moved(pipe);
    if let Some(stream) = pipe.host {
        let bytes = pipe.take_into(usize::MAX, usize::MAX).0;
        cx.host.output(stream, &bytes);
    }
}

pub(super) fn splice(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd_in, off_in, fd_out, off_out, length, flags]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if length == 0 {
        return Ok(0);
    }
    if flags & !SPLICE_F_ALL != 0 {
        return Err(Errno::EINVAL);
    }
    let input = open_file(kernel, tid, fd_in)?;
    let output = open_file(kernel, tid, fd_out)?;
    // `__do_splice`: a pipe takes no offset; the offsets are read before and written after.
    let (in_pipe, out_pipe) = (is_pipe(&input), is_pipe(&output));
    if (in_pipe && off_in != 0) || (out_pipe && off_out != 0) {
        return Err(Errno::ESPIPE);
    }
    let out_offset = match off_out {
        0 => None,
        at => Some(guest(kernel, cx, tid).read_value::<i64>(at)?),
    };
    let in_offset = match off_in {
        0 => None,
        at => Some(guest(kernel, cx, tid).read_value::<i64>(at)?),
    };
    if !input.borrow().readable() || !output.borrow().writable() {
        return Err(Errno::EBADF);
    }
    let nonblock = flags & SPLICE_F_NONBLOCK != 0;
    let file_nonblock = |file: &FileRef| file.borrow().flags & O_NONBLOCK != 0;
    for (pipe, file) in [(in_pipe, &input), (out_pipe, &output)] {
        if pipe {
            super::pipe::lock(kernel, tid, file)?;
        }
    }
    let (moved, written_back) = match (in_pipe, out_pipe) {
        (true, true) => {
            if same_pipe(&input, &output) {
                return Err(Errno::EINVAL);
            }
            let nonblock = nonblock || file_nonblock(&input) || file_nonblock(&output);
            (
                pipe_to_pipe(kernel, cx, tid, &input, &output, length, nonblock)?,
                None,
            )
        }
        (true, false) => {
            let at = match out_offset {
                Some(at) => {
                    positioned(&output, true).map_err(|_| Errno::EINVAL)?;
                    at as u64
                }
                None => output.borrow().offset,
            };
            if output.borrow().flags & O_APPEND != 0 {
                return Err(Errno::EINVAL);
            }
            verify_area(at, length)?;
            let nonblock = nonblock || file_nonblock(&input);
            if super::socket::of(&output).is_some() {
                return pipe_to_socket(kernel, cx, tid, &input, &output, length, nonblock);
            }
            let mut at = at;
            let moved = pipe_to_file(kernel, cx, tid, &input, &output, &mut at, length, nonblock);
            match out_offset {
                Some(_) => (moved?, Some((off_out, at))),
                None => {
                    output.borrow_mut().offset = at;
                    (moved?, None)
                }
            }
        }
        (false, true) => {
            let at = match in_offset {
                Some(at) => {
                    positioned(&input, false).map_err(|_| Errno::EINVAL)?;
                    at as u64
                }
                None => input.borrow().offset,
            };
            verify_area(at, length)?;
            if super::socket::of(&input).is_some() {
                let pipe_nonblock = nonblock || file_nonblock(&output);
                return socket_to_pipe(
                    kernel,
                    cx,
                    tid,
                    &input,
                    &output,
                    length,
                    pipe_nonblock,
                    nonblock,
                );
            }
            let nonblock = nonblock || file_nonblock(&output);
            let mut at = at;
            let moved = file_to_pipe(kernel, cx, tid, &input, &output, &mut at, length, nonblock);
            match in_offset {
                Some(_) => (moved?, Some((off_in, at))),
                None => {
                    input.borrow_mut().offset = at;
                    (moved?, None)
                }
            }
        }
        (false, false) => return Err(Errno::EINVAL),
    };
    if let Some((address, at)) = written_back {
        guest(kernel, cx, tid).put(address, &at.to_le_bytes())?;
    }
    Ok(moved)
}

/// `splice_pipe_to_pipe`: whole slots move as they are, a last part is copied.
fn pipe_to_pipe(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    input: &FileRef,
    output: &FileRef,
    length: u64,
    nonblock: bool,
) -> Result<u64, Errno> {
    input_ready(kernel, tid, input, nonblock)?;
    output_ready(kernel, cx, tid, output, nonblock, false)?;
    let mut moved = 0u64;
    let mut emptied = false;
    let mut result = Ok(());
    while moved < length {
        if with_pipe(output, |pipe| pipe.readers()) == 0 {
            sigpipe(kernel, cx, tid);
            result = Err(Errno::EPIPE);
            break;
        }
        let Some(front) = with_pipe(input, |pipe| pipe.front_len()) else {
            break;
        };
        if with_pipe(output, |pipe| pipe.host.is_none() && pipe.is_full()) {
            break;
        }
        let wanted = (length - moved).min(usize::MAX as u64) as usize;
        emptied |= wanted >= front;
        let node = input.borrow().node.clone();
        let mut inode = node.borrow_mut();
        let Content::Pipe(source) = &mut inode.content else {
            unreachable!("the file is a pipe");
        };
        let mut count = 0;
        into_output(cx, output, |target| {
            count = source.move_front(target, wanted)
        });
        moved += count as u64;
    }
    if moved > 0 {
        readers_woken(kernel, cx, output);
    }
    if emptied {
        writers_woken(kernel, cx, input);
    }
    match result {
        Err(error) if moved == 0 => Err(error),
        _ => Ok(moved),
    }
}

/// `splice_file_to_pipe`: room first, then the file's bytes read into slots, one for each of the
/// file's pages, as many as fit.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
pub(super) fn file_to_pipe(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    input: &FileRef,
    output: &FileRef,
    at: &mut u64,
    length: u64,
    nonblock: bool,
) -> Result<u64, Errno> {
    output_ready(kernel, cx, tid, output, nonblock, true)?;
    // `do_splice_read`: nothing asked is nothing read; no more than the pipe has room for.
    if length == 0 {
        return Ok(0);
    }
    let free = with_pipe(output, |pipe| {
        if pipe.host.is_some() {
            pipe.capacity as u64
        } else {
            (pipe.capacity - pipe.occupancy()) as u64
        }
    });
    let length = length.min(free * PAGE_SIZE);
    verify_area(*at, length)?;
    let length = length.min(MAX_RW_COUNT);
    let node = input.borrow().node.clone();
    let (regular, device, unspliceable) = {
        let inode = node.borrow();
        let device = match inode.content {
            Content::Special => Device::from_number(inode.rdev),
            _ => None,
        };
        (
            matches!(inode.content, Content::Regular(_)),
            device,
            inode.is_directory()
                || inode.is_secret()
                || device == Some(Device::Null)
                || matches!(
                    inode.content,
                    Content::Proc(crate::fs::proc::Entry::ProcessFile(..))
                ),
        )
    };
    if unspliceable {
        return Err(Errno::EINVAL);
    }
    let bytes = if super::tty::end(input).is_some() {
        // The terminal's read waits holding the pipe's lock.
        let taken = super::tty::take(kernel, cx, tid, input, length);
        if taken == Err(Errno::PARKED) {
            super::pipe::hold(kernel, tid, output);
        }
        taken?
    } else {
        refresh_proc(kernel, cx, tid, input, *at)?;
        file_bytes(cx, input, length, *at)?
    };
    if regular {
        let flags = input.borrow().flags;
        super::io::accessed(kernel, cx, &node, flags);
    }
    let in_page = if regular { *at % PAGE_SIZE } else { 0 };
    let taken = super::pipe::put(cx, kernel, output, &bytes, in_page);
    // A device's read leaves the position alone.
    if device.is_none() {
        *at += taken;
    }
    Ok(taken)
}

/// `splice_to_socket`: the pipe's bytes sent on the socket in one message, waiting for bytes
/// first. A send that waits for room parks with nothing taken from the pipe; its rerun sends the
/// same bytes on from where the send got to.
fn pipe_to_socket(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    input: &FileRef,
    output: &FileRef,
    length: u64,
    nonblock: bool,
) -> Result<u64, Errno> {
    if kernel.signal_pending(tid) {
        return Err(Errno::ERESTARTSYS);
    }
    if with_pipe(input, |pipe| pipe.is_empty()) {
        if with_pipe(input, |pipe| pipe.writers()) == 0 {
            return Ok(0);
        }
        if nonblock {
            return Err(Errno::EAGAIN);
        }
        return Err(park(kernel, tid, input));
    }
    let wanted = length.min(usize::MAX as u64) as usize;
    let bytes = core::cell::RefCell::new(with_pipe(input, |pipe| pipe.unread_bytes(wanted)));
    let flags = if output.borrow().flags & O_NONBLOCK != 0 {
        crate::linux::net::MSG_DONTWAIT
    } else {
        0
    };
    let sent = super::socket::send_bytes(kernel, cx, tid, output, &bytes, flags)?;
    if sent > 0 && with_pipe(input, |pipe| pipe.consume_front(sent as usize)) {
        writers_woken(kernel, cx, input);
    }
    Ok(sent)
}

/// `sock_splice_read`: room in the pipe first, then what the socket has, as a read of it gives it,
/// up to the room. SPLICE_F_NONBLOCK keeps a local stream socket's read from waiting
/// (`unix_stream_splice_read`); TCP's and datagram sockets' reads wait unless the socket is
/// nonblocking (measured).
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn socket_to_pipe(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    input: &FileRef,
    output: &FileRef,
    length: u64,
    nonblock: bool,
    flag_nonblock: bool,
) -> Result<u64, Errno> {
    output_ready(kernel, cx, tid, output, nonblock, true)?;
    let local_stream = super::socket::of(input).is_some_and(|socket| {
        let socket = socket.borrow();
        socket.family == crate::linux::net::AF_UNIX && socket.kind == crate::linux::net::SOCK_STREAM
    });
    let flags = if flag_nonblock && local_stream {
        crate::linux::net::MSG_DONTWAIT
    } else {
        0
    };
    let free = with_pipe(output, |pipe| {
        if pipe.host.is_some() {
            pipe.capacity as u64
        } else {
            (pipe.capacity - pipe.occupancy()) as u64
        }
    });
    let length = length.min(free * PAGE_SIZE).min(MAX_RW_COUNT);
    let bytes = super::socket::receive_bytes(kernel, cx, tid, input, length, flags)?;
    if bytes.is_empty() {
        return Ok(0);
    }
    Ok(super::pipe::put(cx, kernel, output, &bytes, 0))
}

/// `iter_file_splice_write` (and `splice_write_null` for the devices that drop what they get): slot
/// after slot written to the file at `at`, waiting for bytes only before the first.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn pipe_to_file(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    input: &FileRef,
    output: &FileRef,
    at: &mut u64,
    length: u64,
    nonblock: bool,
) -> Result<u64, Errno> {
    let node = output.borrow().node.clone();
    let terminal = super::tty::end(output).is_some();
    // `/dev/null` and `/dev/zero` drop what they get without moving the position
    // (`splice_write_null`); the rest write it where the position is and move it.
    let (regular, writable, moves) = {
        let inode = node.borrow();
        let device = match inode.content {
            Content::Special => Device::from_number(inode.rdev),
            _ => None,
        };
        let backend_file =
            matches!(inode.content, Content::Backend(_)) && inode.file_type() == S_IFREG;
        // Only files with `splice_write` take a splice (`do_splice_from`): tmpfs files other than
        // secret memory, backend files, and the devices other than `/dev/full`; not `/proc` files,
        // process, event, timer and signal descriptors or message queues.
        let splice_write = match inode.content {
            Content::Regular(_) => !inode.is_secret(),
            Content::Backend(_) => backend_file,
            Content::Special => device != Some(Device::Full),
            _ => false,
        };
        (
            matches!(inode.content, Content::Regular(_)) || backend_file,
            splice_write,
            !matches!(device, Some(Device::Null | Device::Zero)),
        )
    };
    if !writable {
        return Err(Errno::EINVAL);
    }
    // A call that waited for a terminal's room goes on after what it wrote.
    let mut done = kernel.threads[&tid].call.progress;
    let mut emptied = false;
    let result = loop {
        if done == length {
            break Ok(());
        }
        // `splice_from_pipe_next`.
        if kernel.signal_pending(tid) {
            break Err(Errno::ERESTARTSYS);
        }
        if with_pipe(input, |pipe| pipe.is_empty()) {
            if with_pipe(input, |pipe| pipe.writers()) == 0 || done > 0 {
                break Ok(());
            }
            if nonblock {
                break Err(Errno::EAGAIN);
            }
            break Err(park(kernel, tid, input));
        }
        // A terminal takes a slot at a time; a file every slot's bytes in one write, which a
        // backend's file takes as one request and nothing leaves the pipe until it is answered.
        let wanted = (length - done).min(usize::MAX as u64) as usize;
        let bytes = with_pipe(input, |pipe| {
            if terminal {
                let front = pipe.front_bytes();
                front[..front.len().min(wanted)].to_vec()
            } else {
                pipe.unread_bytes(wanted)
            }
        });
        let written = if terminal {
            super::tty::put(kernel, cx, tid, output, &bytes)
        } else if regular {
            write_limit(kernel, cx, tid, output, *at, bytes.len() as u64).and_then(|allowed| {
                let bytes = &bytes[..allowed as usize];
                write_bytes(kernel, cx, tid, output, bytes, allowed, Some(*at), false)
            })
        } else {
            write_bytes(
                kernel,
                cx,
                tid,
                output,
                &bytes,
                bytes.len() as u64,
                Some(*at),
                false,
            )
        };
        let written = match written {
            Ok(written) => written,
            Err(error) => break Err(error),
        };
        if written > 0 {
            emptied |= with_pipe(input, |pipe| pipe.consume_front(written as usize));
        }
        done += written;
        if moves {
            *at += written;
        }
        if written < bytes.len() as u64 {
            // A full terminal: a blocking one is waited for.
            if terminal {
                match super::tty::wait_for_room(kernel, tid, output, done) {
                    Errno::PARKED => {
                        if emptied {
                            writers_woken(kernel, cx, input);
                        }
                        return Err(Errno::PARKED);
                    }
                    error => break Err(error),
                }
            }
            break Ok(());
        }
    };
    if emptied {
        writers_woken(kernel, cx, input);
    }
    match result {
        Err(Errno::PARKED) => Err(Errno::PARKED),
        Err(error) if done == 0 => Err(error),
        _ => Ok(done),
    }
}

pub(super) fn tee(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd_in, fd_out, length, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !SPLICE_F_ALL != 0 {
        return Err(Errno::EINVAL);
    }
    if length == 0 {
        return Ok(0);
    }
    let input = open_file(kernel, tid, fd_in)?;
    let output = open_file(kernel, tid, fd_out)?;
    if !input.borrow().readable() || !output.borrow().writable() {
        return Err(Errno::EBADF);
    }
    if !is_pipe(&input) || !is_pipe(&output) || same_pipe(&input, &output) {
        return Err(Errno::EINVAL);
    }
    let nonblock = flags & SPLICE_F_NONBLOCK != 0
        || (input.borrow().flags | output.borrow().flags) & O_NONBLOCK != 0;
    input_ready(kernel, tid, &input, nonblock)?;
    output_ready(kernel, cx, tid, &output, nonblock, false)?;
    // `link_pipe`: copies of the input's slots, which stay.
    let mut copied = 0u64;
    let mut result = Ok(());
    let mut index = 0;
    while copied < length {
        if with_pipe(&output, |pipe| pipe.readers()) == 0 {
            sigpipe(kernel, cx, tid);
            result = Err(Errno::EPIPE);
            break;
        }
        if with_pipe(&output, |pipe| pipe.host.is_none() && pipe.is_full()) {
            break;
        }
        let wanted = (length - copied).min(usize::MAX as u64) as usize;
        let node = input.borrow().node.clone();
        let inode = node.borrow();
        let Content::Pipe(source) = &inode.content else {
            unreachable!("the file is a pipe");
        };
        let mut count = None;
        into_output(cx, &output, |target| {
            count = source.copy_slot(index, target, wanted)
        });
        let Some(count) = count else {
            break;
        };
        copied += count as u64;
        index += 1;
    }
    if copied > 0 {
        readers_woken(kernel, cx, &output);
    }
    match result {
        Err(error) if copied == 0 => Err(error),
        _ => Ok(copied),
    }
}

pub(super) fn vmsplice(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, vector, count, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !SPLICE_F_ALL != 0 {
        return Err(Errno::EINVAL);
    }
    let pipe = open_file(kernel, tid, fd)?;
    let to_pipe = if pipe.borrow().writable() {
        true
    } else if pipe.borrow().readable() {
        false
    } else {
        return Err(Errno::EBADF);
    };
    let parts = super::io::segments(&mut guest(kernel, cx, tid), vector, count)?;
    if parts.iter().all(|(_, length)| *length == 0) {
        return Ok(0);
    }
    if !is_pipe(&pipe) {
        return Err(Errno::EBADF);
    }
    let nonblock = flags & SPLICE_F_NONBLOCK != 0;
    if to_pipe {
        vmsplice_to_pipe(kernel, cx, tid, &pipe, &parts, nonblock)
    } else {
        vmsplice_to_user(kernel, cx, tid, &pipe, &parts, nonblock)
    }
}

/// `vmsplice_to_pipe`: room first, then the caller's pages, a slot for each page's part, as many as
/// fit. Linux puts the pages themselves in the pipe, so a later change to them shows in what is read;
/// here they are copied when the call is made (counted).
fn vmsplice_to_pipe(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    pipe: &FileRef,
    parts: &[(u64, u64)],
    nonblock: bool,
) -> Result<u64, Errno> {
    output_ready(kernel, cx, tid, pipe, nonblock, true)?;
    kernel
        .counters
        .count(cx.host, reason::VMSPLICE_COPIED, __NR_vmsplice.into());
    let mut total = 0u64;
    let mut result = Ok(());
    'parts: for &(base, length) in parts {
        let mut at = base;
        let end = base + length;
        while at < end {
            // `iov_iter_get_pages2`: up to 16 pages of one part, as far as they can be read.
            let reach = (end - at).min(16 * PAGE_SIZE - at % PAGE_SIZE);
            let readable = guest(kernel, cx, tid).accessible(at, reach, Permissions::READ);
            if readable == 0 {
                result = Err(Errno::EFAULT);
                break 'parts;
            }
            let bytes = guest(kernel, cx, tid).read(at, readable)?;
            let mut done = 0usize;
            while done < bytes.len() {
                // `add_to_pipe`.
                if with_pipe(pipe, |pipe| pipe.readers()) == 0 {
                    sigpipe(kernel, cx, tid);
                    result = Err(Errno::EPIPE);
                    break 'parts;
                }
                if with_pipe(pipe, |pipe| pipe.host.is_none() && pipe.is_full()) {
                    result = Err(Errno::EAGAIN);
                    break 'parts;
                }
                let piece =
                    ((PAGE_SIZE - (at + done as u64) % PAGE_SIZE) as usize).min(bytes.len() - done);
                let slot = bytes[done..done + piece].to_vec();
                into_output(cx, pipe, |target| target.push_spliced(slot));
                done += piece;
                total += piece as u64;
            }
            at += readable;
        }
    }
    if total > 0 {
        readers_woken(kernel, cx, pipe);
        return Ok(total);
    }
    result.map(|()| 0)
}

/// `vmsplice_to_user`: `__splice_from_pipe` with `pipe_to_user`, slot by slot into the parts; a slot
/// whose copy faults stays, with what was stored before the fault.
fn vmsplice_to_user(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    pipe: &FileRef,
    parts: &[(u64, u64)],
    nonblock: bool,
) -> Result<u64, Errno> {
    let total: u64 = parts.iter().map(|(_, length)| length).sum();
    let mut done = 0u64;
    let mut emptied = false;
    let result = loop {
        if done == total {
            break Ok(());
        }
        if kernel.signal_pending(tid) {
            break Err(Errno::ERESTARTSYS);
        }
        if with_pipe(pipe, |pipe| pipe.is_empty()) {
            if with_pipe(pipe, |pipe| pipe.writers()) == 0 || done > 0 {
                break Ok(());
            }
            if nonblock {
                break Err(Errno::EAGAIN);
            }
            break Err(park(kernel, tid, pipe));
        }
        let bytes = with_pipe(pipe, |pipe| {
            let front = pipe.front_bytes();
            front[..front.len().min((total - done) as usize)].to_vec()
        });
        // Into the parts from `done` on, stopping at a fault.
        let mut stored = 0usize;
        let mut skipped = 0u64;
        for &(base, length) in parts {
            if stored == bytes.len() {
                break;
            }
            let from = (done + stored as u64).saturating_sub(skipped);
            skipped += length;
            if from >= length {
                continue;
            }
            let piece = ((length - from) as usize).min(bytes.len() - stored);
            let put =
                guest(kernel, cx, tid).write_partial(base + from, &bytes[stored..stored + piece]);
            stored += put;
            if put < piece {
                break;
            }
        }
        if stored < bytes.len() {
            break Err(Errno::EFAULT);
        }
        emptied |= with_pipe(pipe, |pipe| pipe.consume_front(bytes.len()));
        done += bytes.len() as u64;
    };
    if emptied {
        writers_woken(kernel, cx, pipe);
    }
    match result {
        Err(Errno::PARKED) => Err(Errno::PARKED),
        Err(error) if done == 0 => Err(error),
        _ => Ok(done),
    }
}
