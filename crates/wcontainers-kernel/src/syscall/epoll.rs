//! epoll, as Linux's `fs/eventpoll.c`: an instance on the shared anonymous inode holds items, each
//! a file and descriptor number with the events it watches. An item joins the instance's ready list
//! when the file's readiness changes with an event it watches (`ep_poll_callback`); a wait reports
//! the ready items whose file is ready now, keeps level-triggered ones listed, and drops
//! edge-triggered ones until their next event.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::RefCell;

use super::Args;
use crate::file::{Anon, FileRef, OpenFile};
use crate::fs::Content;
use crate::linux::general::*;
use crate::process::{Channel, ObjectId, Pid, Tid};
use crate::{Context, Errno, Kernel};

/// What `/proc/<pid>/fd` shows.
const LINK: &[u8] = b"anon_inode:[eventpoll]";

/// Bits that say how an item is reported rather than what it waits for.
const PRIVATE_BITS: u32 = EPOLLWAKEUP | EPOLLONESHOT | EPOLLET | EPOLLEXCLUSIVE;

/// The events an exclusive item may watch.
const EXCLUSIVE_OK_BITS: u32 =
    EPOLLIN | EPOLLOUT | EPOLLERR | EPOLLHUP | EPOLLWAKEUP | EPOLLET | EPOLLEXCLUSIVE;

/// How deep instances may hold instances.
const MAX_NESTS: usize = 4;

/// The size of an `epoll_event` on ARM64 (not packed there).
const EVENT_SIZE: u64 = size_of::<epoll_event>() as u64;

/// The most events one wait may ask for.
const MAX_EVENTS: u64 = i32::MAX as u64 / EVENT_SIZE;

pub(crate) type EpollRef = Rc<RefCell<Epoll>>;

/// What a file's readiness hangs on (Linux's wait queue): changes to a kernel object, or a
/// process's signals for its signal descriptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Queue {
    Object(ObjectId),
    Signals(Pid),
}

/// An item's key: its descriptor number and its open file (`epoll_filefd`), by the file's number.
type Key = (i32, u64);

/// One instance.
#[derive(Default)]
pub(crate) struct Epoll {
    /// Its number from `Kernel::next_object`.
    object: u64,
    items: BTreeMap<Key, Item>,
    /// Items that had an event, in order (`rdllist`).
    ready: VecDeque<Key>,
    /// Threads waiting in `epoll_wait`, in the order they began (an exclusive wait queue).
    waiters: VecDeque<Tid>,
    /// `struct epoll_params` as EPIOCSPARAMS set it: busy polling, which nothing here does (no
    /// device is polled), so they are only kept.
    busy_poll: [u8; 8],
}

struct Item {
    file: Weak<RefCell<OpenFile>>,
    events: u32,
    data: u64,
    queue: Option<Queue>,
    /// On the ready list.
    linked: bool,
}

/// The items watching each queue, in Linux's wait queue order: others first, newest first (each
/// added at the head), then exclusive ones, oldest first (each added at the tail).
#[derive(Default)]
pub(crate) struct Watches(BTreeMap<Queue, Vec<Watch>>);

#[derive(Clone)]
struct Watch {
    epoll: Weak<RefCell<Epoll>>,
    key: Key,
    exclusive: bool,
}

impl Watches {
    fn add(&mut self, queue: Queue, watch: Watch) {
        let list = self.0.entry(queue).or_default();
        list.retain(|other| other.epoll.strong_count() > 0);
        if watch.exclusive {
            list.push(watch);
        } else {
            list.insert(0, watch);
        }
    }

    fn remove(&mut self, queue: Queue, epoll: &EpollRef, key: Key) {
        if let Some(list) = self.0.get_mut(&queue) {
            list.retain(|watch| {
                watch.epoll.strong_count() > 0
                    && !(watch.key == key && core::ptr::eq(watch.epoll.as_ptr(), Rc::as_ptr(epoll)))
            });
            if list.is_empty() {
                self.0.remove(&queue);
            }
        }
    }

    /// The instances holding an item for `queue`.
    fn holders(&self, queue: Queue) -> Vec<EpollRef> {
        self.0
            .get(&queue)
            .into_iter()
            .flatten()
            .filter_map(|watch| watch.epoll.upgrade())
            .collect()
    }
}

/// An instance's items for fdinfo: descriptor number, file, events and data, those whose file is
/// still open.
pub(super) fn items(epoll: &Epoll) -> Vec<(i32, FileRef, u32, u64)> {
    epoll
        .items
        .iter()
        .filter_map(|((fd, _), item)| Some((*fd, item.file.upgrade()?, item.events, item.data)))
        .collect()
}

/// The instance behind an open file, if it is one.
pub(super) fn of(file: &OpenFile) -> Option<EpollRef> {
    match &file.anon {
        Some(Anon::Epoll(epoll)) => Some(epoll.clone()),
        _ => None,
    }
}

fn object(epoll: &EpollRef) -> ObjectId {
    ObjectId::Numbered(epoll.borrow().object)
}

/// What threads in `epoll_wait` on an instance park on.
fn waiting(epoll: &EpollRef) -> Channel {
    Channel::Epoll(epoll.borrow().object)
}

/// The queue a file's readiness hangs on, for a file `vfs_poll` can ask (`file_can_poll`); none
/// for a file that cannot be polled. A file whose readiness never changes has no queue.
pub(crate) fn queue_of(kernel: &Kernel, tid: Tid, file: &FileRef) -> Result<Option<Queue>, Errno> {
    let object = |channel: Channel| match channel {
        Channel::Object(object) => Ok(Some(Queue::Object(object))),
        _ => unreachable!("a file's channel is an object"),
    };
    if let Some((pty, ..)) = super::tty::end(file) {
        return object(super::tty::channel(&pty));
    }
    let open = file.borrow();
    if let Some(counter) = super::eventfd::of(&open) {
        return object(super::eventfd::channel(&counter));
    }
    if let Some(instance) = super::inotify::of(&open) {
        return object(super::inotify::channel(&instance));
    }
    if let Some(epoll) = of(&open) {
        return Ok(Some(Queue::Object(self::object(&epoll))));
    }
    if super::signalfd::mask_of(&open).is_some() {
        return Ok(Some(Queue::Signals(kernel.threads[&tid].process)));
    }
    if super::timer::of(&open).is_some() {
        return Ok(Some(Queue::Object(ObjectId::Numbered(open.id))));
    }
    let node = open.node.clone();
    let inode = node.borrow();
    match &inode.content {
        Content::Pipe(_) => object(super::pipe::channel(&node)),
        Content::Pidfd(..) => object(super::pidfd::channel(&node)),
        Content::Mqueue(_) => object(super::mqueue::channel(&node)),
        Content::Socket(socket) => Ok(Some(Queue::Object(super::socket::object(socket)))),
        // Backend files are the container's own files, which the reference keeps on overlayfs
        // over ext4, whose files have no poll: EPERM, as for the in-memory ones.
        Content::Special
            if crate::fs::dev::Device::from_number(inode.rdev)
                == Some(crate::fs::dev::Device::Random) =>
        {
            Ok(None)
        }
        Content::Proc(entry) if !inode.is_directory() && crate::fs::proc::polls(entry) => Ok(None),
        _ => Err(Errno::EPERM),
    }
}

/// An item's events now: what its file's poll gives of those it watches (`ep_item_poll`).
fn item_poll(kernel: &Kernel, tid: Tid, file: &FileRef, events: u32) -> u32 {
    u32::from(super::io::poll_mask(kernel, tid, file)) & events
}

/// Readiness changed on `queue` with the events in `key` (0 for any): each item watching it takes
/// the event (`ep_poll_callback`), in wait queue order, until an exclusive one wakes a waiter for
/// it (`__wake_up_common` with one exclusive wake).
pub(crate) fn notify(kernel: &mut Kernel, queue: Queue, key: u32) {
    let Some(list) = kernel.poll_watches.0.get(&queue) else {
        return;
    };
    let list = list.clone();
    let mut taken: Vec<EpollRef> = Vec::new();
    for watch in list {
        let Some(epoll) = watch.epoll.upgrade() else {
            continue;
        };
        let events = {
            let mut instance = epoll.borrow_mut();
            let Some(item) = instance.items.get_mut(&watch.key) else {
                continue;
            };
            let events = item.events;
            // A one-shot item that fired, or an event it does not watch.
            if events & !PRIVATE_BITS == 0 || (key != 0 && key & events == 0) {
                continue;
            }
            if !item.linked {
                item.linked = true;
                instance.ready.push_back(watch.key);
            }
            events
        };
        let woke = wake_one(kernel, &epoll);
        taken.push(epoll);
        if watch.exclusive && woke {
            let wanted = match key & (EPOLLIN | EPOLLOUT) {
                0 => true,
                EPOLLIN => events & EPOLLIN != 0,
                EPOLLOUT => events & EPOLLOUT != 0,
                _ => false,
            };
            if wanted {
                break;
            }
        }
    }
    // Then whatever polls the instances themselves (`ep_poll_safewake`).
    for epoll in taken {
        kernel.wake_poll(Channel::Object(object(&epoll)), EPOLLIN);
    }
}

/// Wakes the first thread still waiting in `epoll_wait` on the instance; whether there was one.
fn wake_one(kernel: &mut Kernel, epoll: &EpollRef) -> bool {
    let channel = waiting(epoll);
    loop {
        let Some(tid) = epoll.borrow_mut().waiters.pop_front() else {
            return false;
        };
        if kernel
            .threads
            .get(&tid)
            .is_some_and(|thread| thread.call.wait == Some(channel))
        {
            kernel.wake_thread(tid);
            return true;
        }
    }
}

/// `ep_eventpoll_poll`: readable while an item on the ready list is ready now; those that are not
/// leave the list.
pub(super) fn poll(kernel: &Kernel, tid: Tid, epoll: &EpollRef) -> u16 {
    let keys: Vec<Key> = epoll.borrow().ready.iter().copied().collect();
    let mut ready = false;
    for key in keys {
        let found = epoll
            .borrow()
            .items
            .get(&key)
            .map(|item| (item.file.upgrade(), item.events));
        let now = match found {
            Some((Some(file), events)) => item_poll(kernel, tid, &file, events) != 0,
            _ => false,
        };
        if now {
            ready = true;
            break;
        }
        unlink(&mut epoll.borrow_mut(), key);
    }
    if ready {
        (POLLIN | POLLRDNORM) as u16
    } else {
        0
    }
}

fn unlink(instance: &mut Epoll, key: Key) {
    instance.ready.retain(|other| *other != key);
    if let Some(item) = instance.items.get_mut(&key) {
        item.linked = false;
    }
}

pub(super) fn epoll_create1(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !EPOLL_CLOEXEC != 0 {
        return Err(Errno::EINVAL);
    }
    let node = super::anon_node(kernel, cx);
    let opener = kernel.credentials(tid).serial;
    let file = OpenFile::new(kernel.next_object(), node, O_RDWR, LINK.to_vec(), opener);
    let epoll = Epoll {
        object: kernel.next_object(),
        ..Epoll::default()
    };
    file.borrow_mut().anon = Some(Anon::Epoll(Rc::new(RefCell::new(epoll))));
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let fd = kernel
        .files_mut(tid)
        .add(file, flags & EPOLL_CLOEXEC != 0, 0, limit)?;
    Ok(fd as u64)
}

pub(super) fn epoll_ctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [epfd, op, fd, event, ..]: Args,
) -> Result<u64, Errno> {
    let op = op as u32;
    let has_event = op != EPOLL_CTL_DEL;
    let mut wanted = if has_event {
        let bytes = super::guest(kernel, cx, tid).read(event, EVENT_SIZE)?;
        epoll_event {
            events: u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")),
            data: u64::from_le_bytes(bytes[8..].try_into().expect("8 bytes")),
        }
    } else {
        epoll_event { events: 0, data: 0 }
    };
    let epoll_file = super::io::file(kernel, tid, epfd)?;
    let target = super::io::file(kernel, tid, fd)?;
    if epoll_file.borrow().is_path() || target.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let queue = queue_of(kernel, tid, &target)?;
    // EPOLLWAKEUP needs CAP_BLOCK_SUSPEND, which Docker's root lacks; without it the bit goes.
    if has_event && !kernel.credentials(tid).has(CAP_BLOCK_SUSPEND) {
        wanted.events &= !EPOLLWAKEUP;
    }
    let Some(epoll) = of(&epoll_file.borrow()).filter(|_| !Rc::ptr_eq(&epoll_file, &target)) else {
        return Err(Errno::EINVAL);
    };
    let nested = of(&target.borrow());
    if has_event && wanted.events & EPOLLEXCLUSIVE != 0 {
        if op == EPOLL_CTL_MOD {
            return Err(Errno::EINVAL);
        }
        if op == EPOLL_CTL_ADD && (nested.is_some() || wanted.events & !EXCLUSIVE_OK_BITS != 0) {
            return Err(Errno::EINVAL);
        }
    }
    if op == EPOLL_CTL_ADD
        && let Some(inner) = &nested
        && loops(kernel, &epoll, inner)
    {
        return Err(Errno::ELOOP);
    }
    let key = (fd as i32, target.borrow().id);
    let exists = epoll.borrow().items.contains_key(&key);
    match op {
        EPOLL_CTL_ADD => {
            if exists {
                return Err(Errno::EEXIST);
            }
            let events = wanted.events | EPOLLERR | EPOLLHUP;
            epoll.borrow_mut().items.insert(
                key,
                Item {
                    file: Rc::downgrade(&target),
                    events,
                    data: wanted.data,
                    queue,
                    linked: false,
                },
            );
            if let Some(queue) = queue {
                kernel.poll_watches.add(
                    queue,
                    Watch {
                        epoll: Rc::downgrade(&epoll),
                        key,
                        exclusive: events & EPOLLEXCLUSIVE != 0,
                    },
                );
            }
            check_ready(kernel, tid, &epoll, key, &target, events);
        }
        EPOLL_CTL_DEL => {
            if !exists {
                return Err(Errno::ENOENT);
            }
            remove(kernel, &epoll, key);
        }
        EPOLL_CTL_MOD => {
            if !exists {
                return Err(Errno::ENOENT);
            }
            let events = {
                let mut instance = epoll.borrow_mut();
                let item = instance.items.get_mut(&key).expect("exists");
                if item.events & EPOLLEXCLUSIVE != 0 {
                    return Err(Errno::EINVAL);
                }
                item.events = wanted.events | EPOLLERR | EPOLLHUP;
                item.data = wanted.data;
                item.events
            };
            check_ready(kernel, tid, &epoll, key, &target, events);
        }
        _ => return Err(Errno::EINVAL),
    }
    Ok(0)
}

/// After an item is added or changed, it joins the ready list if its file is ready now with
/// what it watches (`ep_insert`, `ep_modify`), and waiters wake.
fn check_ready(
    kernel: &mut Kernel,
    tid: Tid,
    epoll: &EpollRef,
    key: Key,
    file: &FileRef,
    events: u32,
) {
    if item_poll(kernel, tid, file, events) == 0 {
        return;
    }
    {
        let mut instance = epoll.borrow_mut();
        let item = instance.items.get_mut(&key).expect("the item");
        if item.linked {
            return;
        }
        item.linked = true;
        instance.ready.push_back(key);
    }
    wake_one(kernel, epoll);
    kernel.wake_poll(Channel::Object(object(epoll)), EPOLLIN);
}

fn remove(kernel: &mut Kernel, epoll: &EpollRef, key: Key) {
    let item = {
        let mut instance = epoll.borrow_mut();
        instance.ready.retain(|other| *other != key);
        instance.items.remove(&key)
    };
    if let Some(Item {
        queue: Some(queue), ..
    }) = item
    {
        kernel.poll_watches.remove(queue, epoll, key);
    }
}

/// `ep_loop_check`: whether putting `inner` into `outer` makes a loop or nests instances more than
/// [`MAX_NESTS`] deep, counting down from `inner` and up from `outer`.
fn loops(kernel: &Kernel, outer: &EpollRef, inner: &EpollRef) -> bool {
    fn down(epoll: &EpollRef, into: &EpollRef, depth: usize) -> usize {
        let mut result = 0;
        let nested: Vec<EpollRef> = epoll
            .borrow()
            .items
            .values()
            .filter_map(|item| item.file.upgrade())
            .filter_map(|file| of(&file.borrow()))
            .collect();
        for inner in nested {
            result = if Rc::ptr_eq(&inner, into) || depth > MAX_NESTS {
                MAX_NESTS + 1
            } else {
                result.max(down(&inner, into, depth + 1) + 1)
            };
            if result > MAX_NESTS {
                break;
            }
        }
        result
    }
    fn up(kernel: &Kernel, epoll: &EpollRef, depth: usize) -> usize {
        if depth > MAX_NESTS {
            return depth;
        }
        kernel
            .poll_watches
            .holders(Queue::Object(object(epoll)))
            .iter()
            .map(|holder| up(kernel, holder, depth + 1) + 1)
            .max()
            .unwrap_or(0)
    }
    let depth = down(inner, outer, 0);
    depth > MAX_NESTS || depth + 1 + up(kernel, outer, 0) > MAX_NESTS
}

pub(super) fn epoll_pwait(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [epfd, events, maxevents, timeout, mask, mask_size]: Args,
) -> super::Outcome {
    if !kernel.threads[&tid].call.started {
        // A negative timeout waits for ever, zero not at all.
        let duration = match timeout as i32 {
            ..0 => None,
            milliseconds => Some(milliseconds as u64 * 1_000_000),
        };
        if let Err(error) = super::io::start_wait(kernel, cx, tid, duration, mask, mask_size) {
            return Err(error).into();
        }
    }
    let result = wait(kernel, cx, tid, epfd, events, maxevents);
    finish(kernel, tid, result)
}

pub(super) fn epoll_pwait2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [epfd, events, maxevents, timeout, mask, mask_size]: Args,
) -> super::Outcome {
    if !kernel.threads[&tid].call.started {
        let duration = match super::io::read_timeout(kernel, cx, tid, timeout) {
            // No timeout waits for ever.
            Ok(duration) => duration,
            Err(error) => return Err(error).into(),
        };
        if let Err(error) = super::io::start_wait(kernel, cx, tid, duration, mask, mask_size) {
            return Err(error).into();
        }
    }
    let result = wait(kernel, cx, tid, epfd, events, maxevents);
    finish(kernel, tid, result)
}

/// The mask goes back unless a handler is to run first (`restore_saved_sigmask_unless`).
fn finish(kernel: &mut Kernel, tid: Tid, result: Option<Result<u64, Errno>>) -> super::Outcome {
    let Some(result) = result else {
        return super::Outcome::Parked;
    };
    if result != Err(Errno::EINTR)
        && let Some(saved) = kernel.thread_mut(tid).saved_mask.take()
    {
        kernel.thread_mut(tid).signal_mask = saved;
    }
    result.into()
}

/// One run of `do_epoll_wait`: the ready items reported, or none when the time is up, EINTR for a
/// signal, or parked until an item has an event.
fn wait(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    epfd: u64,
    events: u64,
    maxevents: u64,
) -> Option<Result<u64, Errno>> {
    // `do_epoll_wait` in 7.0 takes the descriptor first (measured), then the count and the buffer.
    let file = match super::io::file(kernel, tid, epfd) {
        Ok(file) if !file.borrow().is_path() => file,
        _ => return Some(Err(Errno::EBADF)),
    };
    let maxevents = maxevents as i32;
    if maxevents <= 0 || maxevents as u64 > MAX_EVENTS {
        return Some(Err(Errno::EINVAL));
    }
    if !crate::user::access_ok(events, maxevents as u64 * EVENT_SIZE) {
        return Some(Err(Errno::EFAULT));
    }
    let Some(epoll) = of(&file.borrow()) else {
        return Some(Err(Errno::EINVAL));
    };
    epoll.borrow_mut().waiters.retain(|other| *other != tid);
    if !epoll.borrow().ready.is_empty() {
        match send_events(kernel, cx, tid, &epoll, events, maxevents as usize) {
            Ok(0) => {}
            result => return Some(result),
        }
    }
    let call = kernel.threads[&tid].call;
    let over = match call.deadline {
        Some(_) => call.progress <= cx.monotonic(),
        None => false,
    };
    if over {
        return Some(Ok(0));
    }
    if kernel.signal_pending(tid) {
        return Some(Err(Errno::EINTR));
    }
    epoll.borrow_mut().waiters.push_back(tid);
    kernel.park(tid, waiting(&epoll));
    None
}

/// `ep_send_events`: each ready item, in order, polled now; one with events is reported, then
/// dropped if one-shot (it waits for nothing until changed), kept listed at the end if
/// level-triggered, and left off until its next event if edge-triggered. Items not reached stay
/// first in line.
fn send_events(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    epoll: &EpollRef,
    events: u64,
    maxevents: usize,
) -> Result<u64, Errno> {
    let mut list: VecDeque<Key> = core::mem::take(&mut epoll.borrow_mut().ready);
    for key in &list {
        if let Some(item) = epoll.borrow_mut().items.get_mut(key) {
            item.linked = false;
        }
    }
    let mut reported = 0u64;
    let mut again: Vec<Key> = Vec::new();
    let mut failed = None;
    while let Some(key) = list.front().copied() {
        if reported as usize >= maxevents {
            break;
        }
        list.pop_front();
        let found = epoll
            .borrow()
            .items
            .get(&key)
            .map(|item| (item.file.upgrade(), item.events, item.data));
        let (file, watched, data) = match found {
            Some((Some(file), watched, data)) => (file, watched, data),
            // Its file is gone (`eventpoll_release`).
            Some((None, ..)) => {
                remove(kernel, epoll, key);
                continue;
            }
            None => continue,
        };
        let revents = item_poll(kernel, tid, &file, watched);
        if revents == 0 {
            continue;
        }
        // `epoll_put_uevent`: the two fields are stored one after the other, the padding between
        // them left as it was.
        let at = events + reported * EVENT_SIZE;
        let (events_bytes, data_bytes) = (revents.to_le_bytes(), data.to_le_bytes());
        let stores: [(u64, &[u8]); 2] = [(0, &events_bytes), (8, &data_bytes)];
        if super::guest(kernel, cx, tid).put_each(at, &stores) < 2 {
            list.push_front(key);
            if reported == 0 {
                failed = Some(Errno::EFAULT);
            }
            break;
        }
        reported += 1;
        let mut instance = epoll.borrow_mut();
        let item = instance.items.get_mut(&key).expect("the item");
        if watched & EPOLLONESHOT != 0 {
            item.events &= PRIVATE_BITS;
        } else if watched & EPOLLET == 0 {
            again.push(key);
        }
    }
    // What was not reached goes back first, then what was there before, then the
    // level-triggered items reported.
    let mut instance = epoll.borrow_mut();
    let before = core::mem::take(&mut instance.ready);
    for key in list.into_iter().chain(before).chain(again) {
        if let Some(item) = instance.items.get_mut(&key)
            && !item.linked
        {
            item.linked = true;
            instance.ready.push_back(key);
        }
    }
    match failed {
        Some(error) => Err(error),
        None => Ok(reported),
    }
}

/// `ep_eventpoll_ioctl`: busy polling's parameters, checked, kept and given back; every other
/// request is EINVAL (measured).
pub(super) fn ioctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    epoll: &EpollRef,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    use crate::linux_extra::{EPIOCGPARAMS, EPIOCSPARAMS};
    match request {
        EPIOCSPARAMS => {
            let bytes: [u8; 8] = super::guest(kernel, cx, tid)
                .read(argument, 8)?
                .try_into()
                .expect("8 bytes");
            let usecs = u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes"));
            let budget = u16::from_le_bytes([bytes[4], bytes[5]]);
            if bytes[7] != 0 || usecs > i32::MAX as u32 || bytes[6] > 1 {
                return Err(Errno::EINVAL);
            }
            // NAPI_POLL_WEIGHT.
            if budget > 64 && !kernel.credentials(tid).has(CAP_NET_ADMIN) {
                return Err(Errno::EPERM);
            }
            epoll.borrow_mut().busy_poll = bytes;
            Ok(0)
        }
        EPIOCGPARAMS => {
            let bytes = epoll.borrow().busy_poll;
            super::guest(kernel, cx, tid).write(argument, &bytes)?;
            Ok(0)
        }
        _ => Err(Errno::EINVAL),
    }
}
