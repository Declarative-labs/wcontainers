//! Native asynchronous I/O (`fs/aio.c`): a context is a ring of completion events mapped into
//! the process. io_submit's reads, writes and syncs run within the call, as Linux's buffered I/O
//! does, and post their events there; a poll request whose file is not ready waits, and posts its
//! event when the file's readiness changes; io_getevents takes them.

use alloc::vec::Vec;
use wcontainers_abi::NANOSECONDS_PER_SECOND;
use wcontainers_executor::Executor;

use super::{Args, guest};
use crate::linux::general::*;
use crate::linux_extra::*;
use crate::process::{Channel, Pid, Tid};
use crate::{Context, Errno, Kernel};

/// `fs.aio-max-nr` (Linux's default).
pub(crate) const MAX_NR: u64 = 65536;

/// One context (`struct kioctx`), named by its ring's address.
pub(crate) struct Ring {
    /// Its number from `Kernel::next_object`.
    object: u64,
    pid: Pid,
    memory: wcontainers_abi::MemoryId,
    /// Its slot in the process's table, which the ring's `id` shows.
    id: u32,
    address: u64,
    length: u64,
    /// Events the ring holds (`nr_events`).
    nr: u32,
    /// The events asked for, counted against `aio-max-nr`.
    max_reqs: u32,
    tail: u32,
    /// Poll requests waiting for their files' events, in the order they came.
    polls: Vec<Poll>,
}

/// A poll request waiting for its file's events (`struct poll_iocb`).
struct Poll {
    file: crate::file::FileRef,
    /// The events it waits for, with EPOLLERR and EPOLLHUP.
    events: u32,
    data: u64,
    /// The request's address in the program, by which io_cancel names it.
    request: u64,
    resfd: Option<crate::file::FileRef>,
    /// What its file's readiness hangs on.
    queue: super::epoll::Queue,
    /// A wake reached it: with the events the wake named, which it posts, or with none named (0),
    /// when its file is polled again (`aio_poll_wake`).
    woken: Option<u32>,
}

impl Ring {
    fn channel(&self) -> Channel {
        Channel::Held(crate::process::ObjectId::Numbered(self.object))
    }
}

/// Contexts of processes that are gone or have started another program go (`exit_aio`).
fn prune(kernel: &mut Kernel) {
    let processes = &kernel.processes;
    kernel.aio.retain(|ring| {
        processes
            .get(&ring.pid)
            .is_some_and(|process| process.memory == ring.memory)
    });
}

/// `fs.aio-nr`: the events every live context asked for.
pub(crate) fn in_use(kernel: &mut Kernel) -> u64 {
    prune(kernel);
    kernel.aio.iter().map(|ring| u64::from(ring.max_reqs)).sum()
}

/// `lookup_ioctx`: the caller's context whose ring is at `address` and still shows its slot.
fn lookup<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    address: u64,
) -> Option<usize> {
    prune(kernel);
    let pid = kernel.process_of(tid).pid;
    let shown: u32 = guest(kernel, cx, tid).read_value(address).ok()?;
    kernel
        .aio
        .iter()
        .position(|ring| ring.pid == pid && ring.address == address && ring.id == shown)
}

fn put_u32<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    address: u64,
    value: u32,
) {
    let _ = guest(kernel, cx, tid).write(address, &value.to_le_bytes());
}

pub(super) fn io_setup<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    [nr_events, context, ..]: Args,
) -> Result<u64, Errno> {
    let asked = nr_events as u32;
    let current: u64 = guest(kernel, cx, tid).read_value(context)?;
    if current != 0 || asked == 0 {
        return Err(Errno::EINVAL);
    }
    // `ioctx_alloc`: room for twice what was asked, at least four per CPU (one here).
    let doubled = u64::from(asked.max(4)) * 2;
    if doubled > 0x1000_0000 / IO_EVENT_SIZE {
        return Err(Errno::EINVAL);
    }
    if u64::from(asked) > MAX_NR {
        return Err(Errno::EAGAIN);
    }
    // `aio_setup_ring`: whole pages, as many events as they hold, mapped shared and not copied
    // by fork.
    let length = crate::memory::page_align(AIO_RING_HEADER + IO_EVENT_SIZE * doubled);
    let nr = ((length - AIO_RING_HEADER) / IO_EVENT_SIZE) as u32;
    let address = super::memory::mmap(
        kernel,
        cx,
        tid,
        [
            0,
            length,
            u64::from(PROT_READ | PROT_WRITE),
            u64::from(MAP_SHARED | MAP_ANONYMOUS),
            u64::MAX,
            0,
        ],
    )?;
    super::memory::madvise(
        kernel,
        cx,
        tid,
        [address, length, u64::from(MADV_DONTFORK), 0, 0, 0],
    )?;
    if in_use(kernel) + u64::from(asked) > MAX_NR {
        super::memory::munmap(kernel, cx, tid, [address, length, 0, 0, 0, 0])?;
        return Err(Errno::EAGAIN);
    }
    // `ioctx_add_table`: the lowest free slot of the process's table.
    let pid = kernel.process_of(tid).pid;
    let memory = kernel.process_of(tid).memory;
    let id = (0..)
        .find(|slot| {
            !kernel
                .aio
                .iter()
                .any(|ring| ring.pid == pid && ring.id == *slot)
        })
        .expect("a free slot");
    let header = [
        id,
        nr,
        0,
        0,
        AIO_RING_MAGIC,
        AIO_RING_COMPAT_FEATURES,
        0,
        AIO_RING_HEADER as u32,
    ];
    let bytes: Vec<u8> = header.iter().flat_map(|word| word.to_le_bytes()).collect();
    guest(kernel, cx, tid).write(address, &bytes)?;
    let object = kernel.next_object();
    kernel.aio.push(Ring {
        object,
        pid,
        memory,
        id,
        address,
        length,
        nr,
        max_reqs: asked,
        tail: 0,
        polls: Vec::new(),
    });
    if let Err(error) = guest(kernel, cx, tid).write(context, &address.to_le_bytes()) {
        destroy(kernel, cx, tid, kernel.aio.len() - 1);
        return Err(error);
    }
    Ok(0)
}

/// `kill_ioctx`: the ring is unmapped and the context goes.
fn destroy<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    index: usize,
) {
    let ring = kernel.aio.remove(index);
    let _ = super::memory::munmap(kernel, cx, tid, [ring.address, ring.length, 0, 0, 0, 0]);
    kernel.wake(ring.channel());
}

pub(super) fn io_destroy<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    [context, ..]: Args,
) -> Result<u64, Errno> {
    let index = lookup(kernel, cx, tid, context).ok_or(Errno::EINVAL)?;
    destroy(kernel, cx, tid, index);
    Ok(0)
}

/// The ring's head as the program left it, and the events between it and the tail.
fn waiting<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    index: usize,
) -> (u32, u32) {
    let (address, nr, tail) = {
        let ring = &kernel.aio[index];
        (ring.address, ring.nr, ring.tail)
    };
    let head: u32 = guest(kernel, cx, tid)
        .read_value(address + 8)
        .unwrap_or(tail)
        % nr;
    (head, (tail + nr - head) % nr)
}

/// `aio_complete`: an event at the ring's tail, and the request's event descriptor counts one.
fn complete<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    index: usize,
    event: [u64; 4],
    resfd: Option<crate::file::FileRef>,
) {
    let (address, slot, nr) = {
        let ring = &kernel.aio[index];
        (ring.address, ring.tail, ring.nr)
    };
    let bytes: Vec<u8> = event.iter().flat_map(|word| word.to_le_bytes()).collect();
    let at = address + AIO_RING_HEADER + u64::from(slot) * IO_EVENT_SIZE;
    let _ = guest(kernel, cx, tid).write(at, &bytes);
    let tail = (slot + 1) % nr;
    kernel.aio[index].tail = tail;
    put_u32(kernel, cx, tid, address + 12, tail);
    if let Some(file) = resfd {
        super::eventfd::signal(kernel, &file);
    }
    let channel = kernel.aio[index].channel();
    kernel.wake(channel);
}

pub(super) fn io_submit<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    [context, count, requests, ..]: Args,
) -> Result<u64, Errno> {
    if (count as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    let index = lookup(kernel, cx, tid, context).ok_or(Errno::EINVAL)?;
    let count = count.min(u64::from(kernel.aio[index].nr));
    let mut done = kernel.threads[&tid].call.progress;
    let mut failure = None;
    while done < count {
        let request = match guest(kernel, cx, tid).read_value::<u64>(requests + done * 8) {
            Ok(request) => request,
            Err(error) => {
                failure = Some(error);
                break;
            }
        };
        match submit_one(kernel, cx, tid, index, request) {
            Ok(()) => done += 1,
            Err(Errno::PARKED) => {
                let call = &mut kernel.thread_mut(tid).call;
                call.started = true;
                call.progress = done;
                return Err(Errno::PARKED);
            }
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    match failure {
        Some(error) if done == 0 => Err(error),
        _ => Ok(done),
    }
}

/// `io_submit_one`: the checks that refuse a request, then the request run and its event posted.
fn submit_one<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    index: usize,
    request: u64,
) -> Result<(), Errno> {
    let iocb = guest(kernel, cx, tid).read(request, IOCB_SIZE)?;
    let u16_at = |at: usize| u16::from_le_bytes([iocb[at], iocb[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes(iocb[at..at + 4].try_into().expect("4 bytes"));
    let u64_at = |at: usize| u64::from_le_bytes(iocb[at..at + 8].try_into().expect("8 bytes"));
    let (data, rw_flags, opcode, fd) = (u64_at(0), u32_at(12), u16_at(16), u32_at(20));
    let (buffer, length, offset) = (u64_at(24), u64_at(32), u64_at(40));
    let (reserved, flags, resfd) = (u64_at(48), u32_at(56), u32_at(60));
    if reserved != 0 || (length as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    // `aio_get_req`: a slot in the ring for its event.
    let (_, unread) = waiting(kernel, cx, tid, index);
    let ring = &kernel.aio[index];
    if unread + ring.polls.len() as u32 + 1 >= ring.nr {
        return Err(Errno::EAGAIN);
    }
    let file = super::io::file(kernel, tid, u64::from(fd))?;
    let eventfd = if flags & IOCB_FLAG_RESFD != 0 {
        let counter = super::io::file(kernel, tid, u64::from(resfd))?;
        if super::eventfd::of(&counter.borrow()).is_none() {
            return Err(Errno::EINVAL);
        }
        Some(counter)
    } else {
        None
    };
    guest(kernel, cx, tid).write(request + 8, &KIOCB_KEY.to_le_bytes())?;
    let fd = u64::from(fd);
    let result = match opcode {
        IOCB_CMD_PREAD | IOCB_CMD_PWRITE | IOCB_CMD_PREADV | IOCB_CMD_PWRITEV => {
            let write = matches!(opcode, IOCB_CMD_PWRITE | IOCB_CMD_PWRITEV);
            let vectored = matches!(opcode, IOCB_CMD_PREADV | IOCB_CMD_PWRITEV);
            // `aio_prep_rw` takes the flags first.
            super::io::rw_flags_allowed(&file, rw_flags)?;
            {
                let open = file.borrow();
                if open.is_path()
                    || !(if write {
                        open.writable()
                    } else {
                        open.readable()
                    })
                {
                    return Err(Errno::EBADF);
                }
                // `aio_read` and `aio_write` want the kind's `read_iter` or `write_iter`:
                // directories, epoll, inotify, process descriptors and message queues have
                // neither, and event counters, timers and signal descriptors only read that way.
                use crate::file::Anon;
                use crate::fs::Content;
                let iterates = match &open.anon {
                    Some(Anon::Event(_) | Anon::Timer(_) | Anon::Signals(_)) => !write,
                    Some(Anon::Epoll(_) | Anon::Inotify(_)) => false,
                    None => !matches!(
                        open.node.borrow().content,
                        Content::Directory(_) | Content::Pidfd(..) | Content::Mqueue(_)
                    ),
                };
                if !iterates {
                    return Err(Errno::EINVAL);
                }
            }
            // `rw_verify_area`.
            if (offset as i64) < 0 {
                return Err(Errno::EINVAL);
            }
            if vectored && length > u64::from(UIO_MAXIOV) {
                return Err(Errno::EINVAL);
            }
            run_rw(
                kernel, cx, tid, fd, buffer, length, offset, rw_flags, write, vectored,
            )?
        }
        IOCB_CMD_FSYNC | IOCB_CMD_FDSYNC => {
            if buffer != 0 || offset != 0 || length != 0 || rw_flags != 0 {
                return Err(Errno::EINVAL);
            }
            let args = [fd, 0, 0, 0, 0, 0];
            let result = if opcode == IOCB_CMD_FSYNC {
                super::io::fsync(kernel, cx, tid, args)
            } else {
                super::io::fdatasync(kernel, cx, tid, args)
            };
            match result {
                // A file with no way to be synced is refused at submission (`aio_fsync`).
                Err(Errno::EINVAL) => return Err(Errno::EINVAL),
                Err(Errno::PARKED) => return Err(Errno::PARKED),
                other => other,
            }
        }
        // `aio_poll`: events within poll's 16 bits and no field poll has no use for; a file ready
        // for one of them answers at once, else the request waits.
        IOCB_CMD_POLL => {
            if buffer > u64::from(u16::MAX) || offset != 0 || length != 0 || rw_flags != 0 {
                return Err(Errno::EINVAL);
            }
            let events = buffer as u32 | EPOLLERR | EPOLLHUP;
            let mask = u32::from(super::io::poll_mask(kernel, tid, &file)) & events;
            if mask == 0 {
                // A file with no wait queue cannot be waited on (`apt.error`).
                let Ok(Some(queue)) = super::epoll::queue_of(kernel, tid, &file) else {
                    return Err(Errno::EINVAL);
                };
                kernel.aio[index].polls.push(Poll {
                    file,
                    events,
                    data,
                    request,
                    resfd: eventfd,
                    queue,
                    woken: None,
                });
                return Ok(());
            }
            Ok(u64::from(mask))
        }
        _ => return Err(Errno::EINVAL),
    };
    let res = match result {
        Ok(value) => value,
        Err(Errno::ERESTARTSYS) => (-(Errno::EINTR.0 as i64)) as u64,
        Err(error) => (-(error.0 as i64)) as u64,
    };
    complete(kernel, cx, tid, index, [data, request, res, 0], eventfd);
    Ok(())
}

/// Readiness changed on `queue` with the events in `key` (0 for any): a waiting poll request on
/// it takes the wake unless it names none of its events (`aio_poll_wake`); whether one did.
pub(crate) fn notify(kernel: &mut Kernel, queue: super::epoll::Queue, key: u32) -> bool {
    let mut took = false;
    for poll in kernel.aio.iter_mut().flat_map(|ring| ring.polls.iter_mut()) {
        if poll.queue == queue && (key == 0 || key & poll.events != 0) {
            // The first wake decides: one naming events completes the request with them.
            poll.woken.get_or_insert(key);
            took = true;
        }
    }
    took
}

/// Poll requests that took wakes post their events: those the wake named, or else what their
/// file's poll gives now, if any of theirs (`aio_poll_complete_work`); the others wait on.
pub(crate) fn polled<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
) {
    prune(kernel);
    for index in 0..kernel.aio.len() {
        let Some(&tid) = kernel.processes[&kernel.aio[index].pid].threads.first() else {
            continue;
        };
        let mut at = 0;
        while at < kernel.aio[index].polls.len() {
            let poll = &mut kernel.aio[index].polls[at];
            let mask = match poll.woken.take() {
                None => 0,
                Some(0) => {
                    let poll = &kernel.aio[index].polls[at];
                    u32::from(super::io::poll_mask(kernel, tid, &poll.file)) & poll.events
                }
                Some(named) => named,
            };
            if mask == 0 {
                at += 1;
                continue;
            }
            let poll = kernel.aio[index].polls.remove(at);
            let event = [poll.data, poll.request, u64::from(mask), 0];
            complete(kernel, cx, tid, index, event, poll.resfd);
        }
    }
}

/// A read or write request run as the matching call: at its offset, or where a file without
/// positions (a pipe, a socket) is, as `read_iter` ignores the position there.
#[allow(clippy::too_many_arguments, reason = "the fields of one request")]
fn run_rw<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    fd: u64,
    buffer: u64,
    length: u64,
    offset: u64,
    rw_flags: u32,
    write: bool,
    vectored: bool,
) -> Result<Result<u64, Errno>, Errno> {
    // A request's flags (`kiocb_set_rw_flags`) are taken as preadv2 and pwritev2 take them.
    let positioned =
        |kernel: &mut Kernel, cx: &mut Context<'_, S, C>, at: u64| match (write, vectored) {
            (_, false) if rw_flags != 0 => super::io::vectored2(
                kernel,
                cx,
                tid,
                [fd, at, u64::from(rw_flags)],
                super::io::Bytes::Buffer(buffer, length),
                write,
            ),
            (false, false) => super::io::pread64(kernel, cx, tid, [fd, buffer, length, at, 0, 0]),
            (true, false) => super::io::pwrite64(kernel, cx, tid, [fd, buffer, length, at, 0, 0]),
            (false, true) => super::io::preadv2(
                kernel,
                cx,
                tid,
                [fd, buffer, length, at, 0, u64::from(rw_flags)],
            ),
            (true, true) => super::io::pwritev2(
                kernel,
                cx,
                tid,
                [fd, buffer, length, at, 0, u64::from(rw_flags)],
            ),
        };
    let result = match positioned(kernel, cx, offset) {
        Err(Errno::ESPIPE) if vectored || rw_flags != 0 => positioned(kernel, cx, u64::MAX),
        Err(Errno::ESPIPE) if write => {
            super::io::write(kernel, cx, tid, [fd, buffer, length, 0, 0, 0])
        }
        Err(Errno::ESPIPE) => super::io::read(kernel, cx, tid, [fd, buffer, length, 0, 0, 0]),
        other => other,
    };
    match result {
        Err(Errno::PARKED) => Err(Errno::PARKED),
        other => Ok(other),
    }
}

/// io_getevents and io_pgetevents after their first run's checks: events are taken as they come
/// until `min_nr` are, the time is up or a signal comes.
fn get_events<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    [context, min_nr, nr, events, ..]: Args,
) -> Result<u64, Errno> {
    let index = lookup(kernel, cx, tid, context).ok_or(Errno::EINVAL)?;
    let (min_nr, nr) = (min_nr as i64, nr as i64);
    if min_nr > nr || min_nr < 0 {
        return Err(Errno::EINVAL);
    }
    let mut taken = kernel.threads[&tid].call.messages;
    // `aio_read_events_ring`: from the head as far as the tail, into the caller's buffer.
    let (mut head, unread) = waiting(kernel, cx, tid, index);
    let (address, ring_nr) = (kernel.aio[index].address, kernel.aio[index].nr);
    let wanted = (u64::from(unread)).min((nr as u64).saturating_sub(taken));
    for _ in 0..wanted {
        let from = address + AIO_RING_HEADER + u64::from(head) * IO_EVENT_SIZE;
        let event = guest(kernel, cx, tid).read(from, IO_EVENT_SIZE)?;
        guest(kernel, cx, tid).write(events + taken * IO_EVENT_SIZE, &event)?;
        head = (head + 1) % ring_nr;
        put_u32(kernel, cx, tid, address + 8, head);
        taken += 1;
        kernel.thread_mut(tid).call.messages = taken;
    }
    let call = kernel.threads[&tid].call;
    let timed_out = call.deadline.is_some() && call.progress <= cx.monotonic();
    if taken >= min_nr as u64 || timed_out || nr == 0 {
        return Ok(taken);
    }
    if kernel.signal_pending(tid) {
        return if taken > 0 {
            Ok(taken)
        } else {
            Err(Errno::EINTR)
        };
    }
    let channel = kernel.aio[index].channel();
    kernel.park(tid, channel);
    Err(Errno::PARKED)
}

/// The timeout of a first run, as `timespec64_to_ktime` takes it unchecked: none, or the time
/// to wait (nothing for zero or less nanoseconds).
fn timeout<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    at: u64,
) -> Result<Option<u64>, Errno> {
    if at == 0 {
        return Ok(None);
    }
    let value: __kernel_timespec = guest(kernel, cx, tid).read_value(at)?;
    // Seconds past `KTIME_SEC_MAX`, negative ones among them, wait for ever.
    if value.tv_sec as u64 >= i64::MAX as u64 / NANOSECONDS_PER_SECOND {
        return Ok(None);
    }
    let nanoseconds =
        i128::from(value.tv_sec) * i128::from(NANOSECONDS_PER_SECOND) + i128::from(value.tv_nsec);
    Ok(Some(nanoseconds.clamp(0, i128::from(i64::MAX)) as u64))
}

pub(super) fn io_getevents<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    if !kernel.threads[&tid].call.started {
        let duration = timeout(kernel, cx, tid, args[4])?;
        super::io::start_wait(kernel, cx, tid, duration, 0, 0)?;
    }
    get_events(kernel, cx, tid, args)
}

pub(super) fn io_pgetevents<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    if !kernel.threads[&tid].call.started {
        let duration = timeout(kernel, cx, tid, args[4])?;
        let (mask, size) = if args[5] != 0 {
            let words = guest(kernel, cx, tid).read(args[5], 16)?;
            (
                u64::from_le_bytes(words[..8].try_into().expect("8 bytes")),
                u64::from_le_bytes(words[8..].try_into().expect("8 bytes")),
            )
        } else {
            (0, 0)
        };
        super::io::start_wait(kernel, cx, tid, duration, mask, size)?;
    }
    let result = get_events(kernel, cx, tid, args);
    if result == Err(Errno::PARKED) {
        return result;
    }
    // `restore_saved_sigmask_unless`: a handler about to run sees the temporary mask.
    let interrupted = kernel.signal_pending(tid);
    if !interrupted && let Some(saved) = kernel.thread_mut(tid).saved_mask.take() {
        kernel.thread_mut(tid).signal_mask = saved;
    }
    match result {
        Ok(0) | Err(Errno::EINTR) if interrupted => Err(Errno::ERESTARTNOHAND),
        other => other,
    }
}

pub(super) fn io_cancel<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    [context, request, ..]: Args,
) -> Result<u64, Errno> {
    let key: u32 = guest(kernel, cx, tid).read_value(request + 8)?;
    if key != KIOCB_KEY {
        return Err(Errno::EINVAL);
    }
    let index = lookup(kernel, cx, tid, context).ok_or(Errno::EINVAL)?;
    // Only a waiting poll request is left to cancel (`aio_poll_cancel`): it posts its event with
    // no events, and the call says the cancel is under way.
    let Some(at) = kernel.aio[index]
        .polls
        .iter()
        .position(|poll| poll.request == request)
    else {
        return Err(Errno::EINVAL);
    };
    let poll = kernel.aio[index].polls.remove(at);
    complete(
        kernel,
        cx,
        tid,
        index,
        [poll.data, poll.request, 0, 0],
        poll.resfd,
    );
    Err(Errno::EINPROGRESS)
}
