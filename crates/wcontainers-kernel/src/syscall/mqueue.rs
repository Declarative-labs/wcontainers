//! POSIX message queues as Linux's `ipc/mqueue.c` builds them: each queue is a file of the IPC
//! namespace's mqueue file system (which Docker mounts at `/dev/mqueue`) holding messages by
//! priority. Senders and receivers that must wait are kept in Linux's order as data, and a send or
//! receive hands the first of them what it waited for.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use zerocopy::IntoBytes;

use super::Args;
use super::guest;
use crate::file::FileRef;
use crate::fs::{Content, Node};
use crate::linux::general::*;
use crate::linux_extra::*;
use crate::process::{Channel, Pid, Tid};
use crate::{Context, Errno, Kernel};

/// What the namespace's queues hold against its limits: how many there are, and the bytes each
/// user's take against `RLIMIT_MSGQUEUE` (`UCOUNT_RLIMIT_MSGQUEUE`).
#[derive(Default)]
pub(crate) struct Usage {
    queues: Cell<u64>,
    bytes: RefCell<BTreeMap<u32, u64>>,
}

/// A queue's limits (`struct mq_attr`'s first fields).
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub max_messages: i64,
    pub message_size: i64,
}

/// A process to tell when a message comes to the empty queue (`notify_owner` and `notify`).
#[derive(Clone, Copy)]
pub(crate) struct Notify {
    pub owner: Pid,
    pub kind: u32,
    pub signal: u32,
    pub value: u64,
    /// The owner's exec count when it asked; after an exec nothing is sent.
    pub exec_id: u64,
}

/// A sender waiting for room, with its message.
struct Sender {
    tid: Tid,
    priority: i32,
    message: (u32, Vec<u8>),
}

/// A queue (`struct mqueue_inode_info`).
pub(crate) struct MessageQueue {
    pub limits: Limits,
    /// Messages by priority, oldest first in each.
    messages: BTreeMap<u32, VecDeque<Vec<u8>>>,
    pub count: i64,
    /// Bytes of message text held (`qsize`).
    pub bytes: u64,
    /// The size `stat` shows, which only truncation changes.
    pub size: u64,
    /// Waiting senders and receivers; the last is served first (`wq_get_first_waiter`).
    senders: Vec<Sender>,
    receivers: Vec<(Tid, i32)>,
    pub notify: Option<Notify>,
    charge: Option<(Rc<Usage>, u32, u64)>,
}

impl Drop for MessageQueue {
    /// `mqueue_evict_inode`: the queue's bytes and its place in the namespace are given back.
    fn drop(&mut self) {
        if let Some((usage, uid, bytes)) = &self.charge {
            usage.queues.set(usage.queues.get() - 1);
            if let Some(held) = usage.bytes.borrow_mut().get_mut(uid) {
                *held -= bytes;
            }
        }
    }
}

/// What a queue's file reads as, `stat` shows and `poll` reports.
impl MessageQueue {
    pub fn text(&self) -> Vec<u8> {
        let notify = self.notify.as_ref();
        let kind = notify.map_or(0, |notify| notify.kind as i32);
        let signal = notify
            .filter(|notify| notify.kind == SIGEV_SIGNAL)
            .map_or(0, |notify| notify.signal as i32);
        let pid = notify.map_or(0, |notify| notify.owner);
        alloc::format!(
            "QSIZE:{:<10} NOTIFY:{:<5} SIGNO:{:<5} NOTIFY_PID:{:<6}\n",
            self.bytes,
            kind,
            signal,
            pid
        )
        .into_bytes()
    }

    pub fn poll(&self) -> u16 {
        let mut events = 0;
        if self.count > 0 {
            events |= POLLIN | POLLRDNORM;
        }
        if self.count < self.limits.max_messages {
            events |= POLLOUT | POLLWRNORM;
        }
        events as u16
    }

    /// `mqueue_flush_file`: a close by the process to be told takes its request away.
    pub fn flush(&mut self, pid: Pid) {
        if self.notify.is_some_and(|notify| notify.owner == pid) {
            self.notify = None;
        }
    }

    /// `msg_insert`.
    fn insert(&mut self, (priority, text): (u32, Vec<u8>)) {
        self.count += 1;
        self.bytes += text.len() as u64;
        self.messages.entry(priority).or_default().push_back(text);
    }

    /// `msg_get`: the oldest message of the highest priority.
    fn take(&mut self) -> Option<(u32, Vec<u8>)> {
        let mut entry = self.messages.last_entry()?;
        let priority = *entry.key();
        let text = entry
            .get_mut()
            .pop_front()
            .expect("no empty priority is kept");
        if entry.get().is_empty() {
            entry.remove();
        }
        self.count -= 1;
        self.bytes -= text.len() as u64;
        Some((priority, text))
    }
}

/// `wq_add`: a waiter goes before the first of no greater priority value, so the last of the list
/// is the most urgent and, among equals, the first to have come.
fn wait_position(priorities: impl Iterator<Item = i32>, priority: i32) -> Option<usize> {
    priorities
        .enumerate()
        .find(|(_, other)| *other <= priority)
        .map(|(index, _)| index)
}

/// A task's priority as Linux orders tasks (`prio`): lower is more urgent.
fn task_priority(kernel: &Kernel, tid: Tid) -> i32 {
    let schedule = &kernel.threads[&tid].schedule;
    match schedule.policy {
        SCHED_FIFO | SCHED_RR => 99 - schedule.priority as i32,
        _ => 120 + schedule.nice,
    }
}

/// `mqueue_get_inode` and `mqueue_create_attr` for a new queue: its limits checked and its bytes
/// charged to the caller's real user.
pub(crate) fn new_queue(
    kernel: &Kernel,
    tid: Tid,
    limits: Option<Limits>,
) -> Result<MessageQueue, Errno> {
    let process = kernel.process_of(tid);
    let credentials = kernel.credentials(tid);
    let usage = &kernel.ipc.mq_usage;
    let resource = credentials.has(CAP_SYS_RESOURCE);
    if usage.queues.get() >= MQ_QUEUES_MAX && !resource {
        return Err(Errno::ENOSPC);
    }
    let limits = limits.unwrap_or(Limits {
        max_messages: MQ_MSG_DEFAULT,
        message_size: MQ_MSGSIZE_DEFAULT,
    });
    if limits.max_messages <= 0 || limits.message_size <= 0 {
        return Err(Errno::EINVAL);
    }
    let (most_messages, largest) = if resource {
        (HARD_MSGMAX, HARD_MSGSIZEMAX)
    } else {
        (MQ_MSG_MAX, MQ_MSGSIZE_MAX)
    };
    if limits.max_messages > most_messages || limits.message_size > largest {
        return Err(Errno::EINVAL);
    }
    let (count, size) = (limits.max_messages as u64, limits.message_size as u64);
    // One `msg_msg` per message and a tree node per priority that may be used.
    let tree = count * MSG_MSG_SIZE + count.min(MQ_PRIO_MAX) * TREE_NODE_SIZE;
    let bytes = count * size + tree;
    let uid = credentials.uid;
    let held = usage.bytes.borrow().get(&uid).copied().unwrap_or(0) + bytes;
    if held > process.limits[RLIMIT_MSGQUEUE as usize].current {
        return Err(Errno::EMFILE);
    }
    usage.bytes.borrow_mut().insert(uid, held);
    usage.queues.set(usage.queues.get() + 1);
    Ok(MessageQueue {
        limits,
        messages: BTreeMap::new(),
        count: 0,
        bytes: 0,
        size: FILENT_SIZE,
        senders: Vec::new(),
        receivers: Vec::new(),
        notify: None,
        charge: Some((usage.clone(), uid, bytes)),
    })
}

/// The queue a descriptor names: EBADF for any other file, or one opened only for its path.
fn queue_file(kernel: &Kernel, tid: Tid, fd: u64) -> Result<FileRef, Errno> {
    let file = super::io::file(kernel, tid, fd)?;
    let is_queue = {
        let open = file.borrow();
        !open.is_path() && matches!(open.node.borrow().content, Content::Mqueue(_))
    };
    if !is_queue {
        return Err(Errno::EBADF);
    }
    Ok(file)
}

/// Reads an absolute timeout on the real time clock (`prepare_timeout`) and makes it a deadline on
/// the monotonic clock.
fn deadline(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
) -> Result<Option<u64>, Errno> {
    if address == 0 {
        return Ok(None);
    }
    let timeout: __kernel_timespec = guest(kernel, cx, tid).read_value(address)?;
    if timeout.tv_sec < 0 || !(0..1_000_000_000).contains(&timeout.tv_nsec) {
        return Err(Errno::EINVAL);
    }
    let at = (timeout.tv_sec as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(timeout.tv_nsec as u64);
    let left = at.saturating_sub(cx.host.realtime_nanoseconds());
    Ok(Some(cx.monotonic().saturating_add(left)))
}

fn with_queue<R>(node: &Node, act: impl FnOnce(&mut MessageQueue) -> R) -> R {
    let mut inode = node.borrow_mut();
    let Content::Mqueue(queue) = &mut inode.content else {
        unreachable!("checked to be a queue");
    };
    act(queue)
}

pub(super) fn mq_open(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [name, flags, mode, attributes, ..]: Args,
) -> Result<u64, Errno> {
    let (flags, mode) = (flags as u32, mode as u32 & 0xffff);
    let limits = if attributes != 0 {
        let attr: mq_attr = guest(kernel, cx, tid).read_value(attributes)?;
        Some(Limits {
            max_messages: attr.mq_maxmsg,
            message_size: attr.mq_msgsize,
        })
    } else {
        None
    };
    let name = super::path_argument(kernel, cx, tid, name, false)?;
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    kernel.files(tid).lowest_free(0, limit)?;
    // `lookup_noperm`: one name in the file system's root.
    if name == b"." || name == b".." || name.contains(&b'/') {
        return Err(Errno::EACCES);
    }
    let root = kernel.ipc.mqueue_root.clone().expect("mounted at start");
    let existing = root.borrow().directory().expect("a directory").get(&name);
    if existing.is_none() && name.len() > NAME_MAX as usize {
        return Err(Errno::ENAMETOOLONG);
    }
    let node = match existing {
        None => {
            if flags & O_CREAT == 0 {
                return Err(Errno::ENOENT);
            }
            let umask = kernel.fs(tid).umask;
            super::files::may_change(kernel, tid, &root)?;
            let queue = Content::Mqueue(alloc::boxed::Box::new(new_queue(kernel, tid, limits)?));
            super::files::create(
                kernel,
                cx,
                tid,
                &root,
                &name,
                S_IFREG | (mode & 0o7777 & !umask),
                0,
                queue,
            )?
        }
        Some(node) => {
            if flags & (O_CREAT | O_EXCL) == O_CREAT | O_EXCL {
                return Err(Errno::EEXIST);
            }
            let mask = match flags & O_ACCMODE {
                O_RDONLY => R_OK,
                O_WRONLY => W_OK,
                O_RDWR => R_OK | W_OK,
                _ => return Err(Errno::EINVAL),
            };
            let pid = kernel.threads[&tid].process;
            crate::fs::permission(&node.borrow(), kernel.credentials(tid), mask, pid)?;
            node
        }
    };
    // `dentry_open` (not `do_sys_open`, so never `O_LARGEFILE`): a queue takes no direct I/O.
    if flags & O_DIRECT != 0 {
        return Err(Errno::EINVAL);
    }
    let mut path = b"/".to_vec();
    path.extend(&name);
    let file = crate::file::OpenFile::new(
        node,
        flags & !(O_CREAT | O_EXCL | O_NOCTTY | O_TRUNC | O_CLOEXEC),
        path,
        kernel.credentials(tid).serial,
    );
    file.borrow_mut().internal_mount = true;
    let fd = kernel.files_mut(tid).add(file, true, 0, limit)?;
    Ok(fd as u64)
}

pub(super) fn mq_unlink(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [name, ..]: Args,
) -> Result<u64, Errno> {
    let name = super::path_argument(kernel, cx, tid, name, false)?;
    if name == b"." || name == b".." || name.contains(&b'/') {
        return Err(Errno::EACCES);
    }
    let root = kernel.ipc.mqueue_root.clone().expect("mounted at start");
    let existing = root.borrow().directory().expect("a directory").get(&name);
    if existing.is_none() && name.len() > NAME_MAX as usize {
        return Err(Errno::ENAMETOOLONG);
    }
    let node = existing.ok_or(Errno::ENOENT)?;
    super::files::remove_from(kernel, cx, tid, &root, &name, &node)?;
    Ok(0)
}

pub(super) fn mq_timedsend(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, length, priority, timeout, ..]: Args,
) -> Result<u64, Errno> {
    if kernel.threads[&tid].call.started {
        return waited(kernel, cx, tid, None);
    }
    let deadline = deadline(kernel, cx, tid, timeout)?;
    let priority = priority as u32;
    if u64::from(priority) >= MQ_PRIO_MAX {
        return Err(Errno::EINVAL);
    }
    let file = queue_file(kernel, tid, fd)?;
    if !file.borrow().writable() {
        return Err(Errno::EBADF);
    }
    let node = file.borrow().node.clone();
    let size = with_queue(&node, |queue| queue.limits.message_size);
    if length > size as u64 {
        return Err(Errno::EMSGSIZE);
    }
    let text = guest(kernel, cx, tid).read(buffer, length)?;
    let nonblocking = file.borrow().flags & O_NONBLOCK != 0;
    let full = with_queue(&node, |queue| queue.count == queue.limits.max_messages);
    if full {
        if nonblocking {
            return Err(Errno::EAGAIN);
        }
        let waiter = task_priority(kernel, tid);
        with_queue(&node, |queue| {
            let at = wait_position(queue.senders.iter().map(|sender| sender.priority), waiter);
            let sender = Sender {
                tid,
                priority: waiter,
                message: (priority, text),
            };
            queue
                .senders
                .insert(at.unwrap_or(queue.senders.len()), sender);
        });
        return park(kernel, tid, &node, deadline);
    }
    let now = kernel.now(cx);
    let receiver = with_queue(&node, |queue| queue.receivers.pop());
    match receiver {
        // `pipelined_send`: the first waiting receiver takes the message.
        Some((receiver, _)) => {
            kernel.ipc.mq_waiting.remove(&receiver);
            kernel.ipc.mq_ready.insert(receiver, Some((priority, text)));
            kernel.wake_thread(receiver);
        }
        None => {
            let sender_pid = kernel.threads[&tid].process;
            let sender_uid = kernel.credentials(tid).uid;
            let notify = with_queue(&node, |queue| {
                queue.insert((priority, text));
                // `__do_notify`: the first message of an empty queue tells the process that asked,
                // once.
                if queue.count == 1 {
                    queue.notify.take()
                } else {
                    None
                }
            });
            if let Some(notify) = notify {
                tell(kernel, cx, notify, sender_pid, sender_uid);
            }
            kernel.wake(channel(&node));
        }
    }
    touch(&node, now);
    Ok(0)
}

/// Sends the signal a request asked for, if it did and its process has not run another program.
fn tell(kernel: &mut Kernel, cx: &mut Context<'_>, notify: Notify, pid: Pid, uid: u32) {
    if notify.kind != SIGEV_SIGNAL || notify.signal == 0 {
        return;
    }
    let Some(owner) = kernel.processes.get(&notify.owner) else {
        return;
    };
    if owner.ended.is_some() || owner.self_exec_id != notify.exec_id {
        return;
    }
    let mut info = crate::signal::sender_info(notify.signal, SI_MESGQ, pid, uid);
    info[24..32].copy_from_slice(&notify.value.to_le_bytes());
    kernel.send_to_process(cx, notify.owner, notify.signal, info);
}

/// Pollers of a queue wait on its inode.
pub(super) fn channel(node: &Node) -> Channel {
    Channel::Object(Rc::as_ptr(node) as *const u8 as usize)
}

fn touch(node: &Node, now: crate::fs::Time) {
    let mut inode = node.borrow_mut();
    inode.atime = now;
    inode.mtime = now;
    inode.ctime = now;
}

/// Parks a sender or receiver just put in the queue's line until it is handed what it waits for,
/// its time is up or a signal comes.
fn park(kernel: &mut Kernel, tid: Tid, node: &Node, deadline: Option<u64>) -> Result<u64, Errno> {
    kernel.ipc.mq_waiting.insert(tid, Rc::downgrade(node));
    let thread = kernel.thread_mut(tid);
    thread.call.started = true;
    thread.call.deadline = deadline;
    // Its time is on the wall clock.
    thread.call.wall = deadline.is_some();
    kernel.park(tid, Channel::Nothing);
    Err(Errno::PARKED)
}

/// A waiting send or receive run again (`wq_sleep`): what it was handed, or a signal (restarted),
/// its time up, or it waits on.
fn waited(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    receive: Option<(u64, u64)>,
) -> Result<u64, Errno> {
    if let Some(handed) = kernel.ipc.mq_ready.remove(&tid) {
        let (Some((priority, text)), Some((buffer, priority_at))) = (handed, receive) else {
            return Ok(0);
        };
        return store(kernel, cx, tid, buffer, priority_at, priority, &text);
    }
    let error = if kernel.signal_pending(tid) {
        Errno::ERESTARTSYS
    } else if kernel.threads[&tid]
        .call
        .deadline
        .is_some_and(|deadline| deadline <= cx.monotonic())
    {
        Errno::ETIMEDOUT
    } else {
        kernel.park(tid, Channel::Nothing);
        return Err(Errno::PARKED);
    };
    forget(kernel, tid);
    Err(error)
}

/// A thread leaves the line it waits in, as when it ends.
pub(crate) fn forget(kernel: &mut Kernel, tid: Tid) {
    kernel.ipc.mq_ready.remove(&tid);
    if let Some(node) = kernel
        .ipc
        .mq_waiting
        .remove(&tid)
        .and_then(|node| node.upgrade())
    {
        with_queue(&node, |queue| {
            queue.senders.retain(|sender| sender.tid != tid);
            queue.receivers.retain(|(other, _)| *other != tid);
        });
    }
}

/// The message's priority and text, stored for the receiver; EFAULT loses the message.
fn store(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    buffer: u64,
    priority_at: u64,
    priority: u32,
    text: &[u8],
) -> Result<u64, Errno> {
    let mut guest = guest(kernel, cx, tid);
    if priority_at != 0 {
        guest
            .put(priority_at, &priority.to_le_bytes())
            .map_err(|_| Errno::EFAULT)?;
    }
    guest.write(buffer, text).map_err(|_| Errno::EFAULT)?;
    Ok(text.len() as u64)
}

pub(super) fn mq_timedreceive(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, length, priority_at, timeout, ..]: Args,
) -> Result<u64, Errno> {
    if kernel.threads[&tid].call.started {
        return waited(kernel, cx, tid, Some((buffer, priority_at)));
    }
    let deadline = deadline(kernel, cx, tid, timeout)?;
    let file = queue_file(kernel, tid, fd)?;
    if !file.borrow().readable() {
        return Err(Errno::EBADF);
    }
    let node = file.borrow().node.clone();
    let size = with_queue(&node, |queue| queue.limits.message_size);
    if length < size as u64 {
        return Err(Errno::EMSGSIZE);
    }
    let nonblocking = file.borrow().flags & O_NONBLOCK != 0;
    let taken = with_queue(&node, |queue| queue.take());
    let Some((priority, text)) = taken else {
        if nonblocking {
            return Err(Errno::EAGAIN);
        }
        let waiter = task_priority(kernel, tid);
        with_queue(&node, |queue| {
            let at = wait_position(
                queue.receivers.iter().map(|(_, priority)| *priority),
                waiter,
            );
            queue
                .receivers
                .insert(at.unwrap_or(queue.receivers.len()), (tid, waiter));
        });
        return park(kernel, tid, &node, deadline);
    };
    let now = kernel.now(cx);
    touch(&node, now);
    // `pipelined_receive`: the first waiting sender's message takes the room.
    let sender = with_queue(&node, |queue| {
        let sender = queue.senders.pop()?;
        queue.insert(sender.message);
        Some(sender.tid)
    });
    match sender {
        Some(sender) => {
            kernel.ipc.mq_waiting.remove(&sender);
            kernel.ipc.mq_ready.insert(sender, None);
            kernel.wake_thread(sender);
        }
        None => kernel.wake(channel(&node)),
    }
    store(kernel, cx, tid, buffer, priority_at, priority, &text)
}

pub(super) fn mq_notify(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, request, ..]: Args,
) -> Result<u64, Errno> {
    let event = if request != 0 {
        // `struct sigevent`: the value, the signal, then how to tell.
        let event = guest(kernel, cx, tid).read(request, SIGEVENT_SIZE)?;
        let word = |at: usize| u32::from_le_bytes(event[at..at + 4].try_into().expect("4"));
        let (signal, kind) = (word(8), word(12));
        let value = u64::from_le_bytes(event[..8].try_into().expect("8"));
        if !matches!(kind, SIGEV_NONE | SIGEV_SIGNAL | SIGEV_THREAD) {
            return Err(Errno::EINVAL);
        }
        if kind == SIGEV_SIGNAL && signal > SIGNALS {
            return Err(Errno::EINVAL);
        }
        if kind == SIGEV_THREAD {
            // The cookie is read, then the descriptor must be a netlink socket; no socket exists here
            // (`netlink_getsockbyfd`).
            guest(kernel, cx, tid).read(value, NOTIFY_COOKIE_LEN)?;
            super::io::open_file(kernel, tid, signal.into())?;
            return Err(Errno::ENOTSOCK);
        }
        Some((kind, signal, value))
    } else {
        None
    };
    let file = queue_file(kernel, tid, fd)?;
    let node = file.borrow().node.clone();
    let pid = kernel.threads[&tid].process;
    let exec_id = kernel.process_of(tid).self_exec_id;
    let now = kernel.now(cx);
    let changed = with_queue(&node, |queue| match event {
        None => {
            let owner = queue.notify.is_some_and(|notify| notify.owner == pid);
            if owner {
                queue.notify = None;
            }
            Ok(owner)
        }
        Some(_) if queue.notify.is_some() => Err(Errno::EBUSY),
        Some((kind, signal, value)) => {
            queue.notify = Some(Notify {
                owner: pid,
                kind,
                signal,
                value,
                exec_id,
            });
            Ok(true)
        }
    })?;
    if changed {
        let mut inode = node.borrow_mut();
        inode.atime = now;
        inode.ctime = now;
    }
    Ok(0)
}

pub(super) fn mq_getsetattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, new, old, ..]: Args,
) -> Result<u64, Errno> {
    let new = if new != 0 {
        Some(guest(kernel, cx, tid).read_value::<mq_attr>(new)?)
    } else {
        None
    };
    if new.is_some_and(|new| new.mq_flags & !i64::from(O_NONBLOCK) != 0) {
        return Err(Errno::EINVAL);
    }
    let file = queue_file(kernel, tid, fd)?;
    let node = file.borrow().node.clone();
    let shown = with_queue(&node, |queue| mq_attr {
        mq_flags: i64::from(file.borrow().flags & O_NONBLOCK),
        mq_maxmsg: queue.limits.max_messages,
        mq_msgsize: queue.limits.message_size,
        mq_curmsgs: queue.count,
        ..Default::default()
    });
    if let Some(new) = new {
        let mut open = file.borrow_mut();
        open.flags = (open.flags & !O_NONBLOCK) | (new.mq_flags as u32 & O_NONBLOCK);
        drop(open);
        let now = kernel.now(cx);
        let mut inode = node.borrow_mut();
        inode.atime = now;
        inode.ctime = now;
    }
    if old != 0 {
        guest(kernel, cx, tid).write(old, shown.as_bytes())?;
    }
    Ok(0)
}
