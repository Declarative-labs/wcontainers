//! inotify (`fs/notify/inotify/inotify_user.c` and the `fsnotify` hooks of the file layer): an
//! instance is a descriptor whose watches sit on inodes; the file calls tell the watches of the
//! inode they touch and, with the name, of the directory it is in.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::RefCell;

use super::{Args, guest};
use crate::file::{Anon, FileRef, OpenFile};
use crate::fs::{Inode, Node};
use crate::linux::general::*;
use crate::process::{Channel, Tid};
use crate::{Context, Errno, Kernel};

/// What `/proc/<pid>/fd` shows.
const LINK: &[u8] = b"anon_inode:inotify";
/// `sizeof(struct inotify_event)`, to whose size names are padded.
const EVENT: usize = 16;
/// `fs.inotify.max_queued_events` (Linux's default).
const MAX_QUEUED: usize = 16384;

/// The events a watch may ask for.
const ALL_EVENTS: u32 = IN_ALL_EVENTS;

/// One queued event.
#[derive(Clone, PartialEq, Eq)]
struct Event {
    wd: i32,
    mask: u32,
    cookie: u32,
    name: Vec<u8>,
}

impl Event {
    /// Its size as read: the header and the name with its zero, padded.
    fn size(&self) -> usize {
        EVENT + padded(&self.name)
    }
}

fn padded(name: &[u8]) -> usize {
    if name.is_empty() {
        0
    } else {
        (name.len() + 1).next_multiple_of(EVENT)
    }
}

/// An inotify instance: its watches by number and the events not yet read.
#[derive(Default)]
pub(crate) struct Instance {
    watches: BTreeMap<i32, Weak<RefCell<Inode>>>,
    last: i32,
    queue: VecDeque<Event>,
}

/// A watch as its inode holds it.
pub(crate) struct Mark {
    instance: Weak<RefCell<Instance>>,
    wd: i32,
    mask: u32,
}

/// The instance behind an open file, if it is one.
pub(crate) fn of_file(file: &FileRef) -> Option<Rc<RefCell<Instance>>> {
    of(&file.borrow())
}

pub(super) fn of(file: &OpenFile) -> Option<Rc<RefCell<Instance>>> {
    match &file.anon {
        Some(Anon::Inotify(instance)) => Some(instance.clone()),
        _ => None,
    }
}

pub(super) fn channel(instance: &Rc<RefCell<Instance>>) -> Channel {
    Channel::Object(Rc::as_ptr(instance) as *const () as usize)
}

pub(super) fn inotify_init1(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(IN_CLOEXEC | IN_NONBLOCK) != 0 {
        return Err(Errno::EINVAL);
    }
    let node = super::anon_node(kernel, cx);
    let opener = kernel.credentials(tid).serial;
    let file = OpenFile::new(node, O_RDONLY | (flags & O_NONBLOCK), LINK.to_vec(), opener);
    file.borrow_mut().anon = Some(Anon::Inotify(Rc::default()));
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let fd = kernel
        .files_mut(tid)
        .add(file, flags & IN_CLOEXEC != 0, 0, limit)?;
    Ok(fd as u64)
}

/// The instance a descriptor names: EBADF without one, EINVAL if it is not an instance.
fn instance(kernel: &Kernel, tid: Tid, fd: u64) -> Result<Rc<RefCell<Instance>>, Errno> {
    let file = super::io::file(kernel, tid, fd)?;
    of(&file.borrow()).ok_or(Errno::EINVAL)
}

pub(super) fn inotify_add_watch(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, path, mask, ..]: Args,
) -> Result<u64, Errno> {
    let mask = mask as u32;
    // `ALL_INOTIFY_BITS`: a mask of flags alone is taken, and watches nothing.
    let known = ALL_EVENTS
        | IN_UNMOUNT
        | IN_Q_OVERFLOW
        | IN_IGNORED
        | IN_ONLYDIR
        | IN_DONT_FOLLOW
        | IN_EXCL_UNLINK
        | IN_MASK_ADD
        | IN_MASK_CREATE
        | IN_ISDIR
        | IN_ONESHOT;
    if mask & known == 0 {
        return Err(Errno::EINVAL);
    }
    if mask & IN_MASK_ADD != 0 && mask & IN_MASK_CREATE != 0 {
        return Err(Errno::EINVAL);
    }
    let file = super::io::file(kernel, tid, fd)?;
    let instance = of(&file.borrow()).ok_or(Errno::EINVAL)?;
    let path = super::path_argument(kernel, cx, tid, path, false)?;
    let at = super::files::start(kernel, tid, AT_FDCWD as u64, &path)?;
    let follow = mask & IN_DONT_FOLLOW == 0;
    let node = kernel.with_walk(tid, |walk, _| walk.resolve(&at, &path, follow))?;
    if mask & IN_ONLYDIR != 0 && !node.borrow().is_directory() {
        return Err(Errno::ENOTDIR);
    }
    let caller = kernel.threads[&tid].process;
    let credentials = kernel.credentials(tid).clone();
    crate::fs::permission(&node.borrow(), &credentials, R_OK, caller)?;
    let wanted = mask & (ALL_EVENTS | IN_ONESHOT | IN_EXCL_UNLINK);
    let existing = node
        .borrow()
        .watches
        .iter()
        .position(|mark| core::ptr::eq(mark.instance.as_ptr(), Rc::as_ptr(&instance)));
    if let Some(index) = existing {
        if mask & IN_MASK_CREATE != 0 {
            return Err(Errno::EEXIST);
        }
        let mut inode = node.borrow_mut();
        let mark = &mut inode.watches[index];
        mark.mask = if mask & IN_MASK_ADD != 0 {
            mark.mask | wanted
        } else {
            wanted
        };
        return Ok(mark.wd as u64);
    }
    let wd = {
        let mut own = instance.borrow_mut();
        own.last += 1;
        let wd = own.last;
        own.watches.insert(wd, Rc::downgrade(&node));
        wd
    };
    node.borrow_mut().watches.push(Mark {
        instance: Rc::downgrade(&instance),
        wd,
        mask: wanted,
    });
    Ok(wd as u64)
}

pub(super) fn inotify_rm_watch(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, wd, ..]: Args,
) -> Result<u64, Errno> {
    let instance = instance(kernel, tid, fd)?;
    let wd = wd as i32;
    let node = instance
        .borrow_mut()
        .watches
        .remove(&wd)
        .ok_or(Errno::EINVAL)?;
    if let Some(node) = node.upgrade() {
        node.borrow_mut().watches.retain(|mark| {
            !(mark.wd == wd && core::ptr::eq(mark.instance.as_ptr(), Rc::as_ptr(&instance)))
        });
    }
    queue(kernel, &instance, wd, IN_IGNORED, 0, &[]);
    Ok(0)
}

/// Queues an event, merged with the last one if it is the same (`inotify_merge`), or the queue's
/// overflow once it is full.
fn queue(
    kernel: &mut Kernel,
    instance: &Rc<RefCell<Instance>>,
    wd: i32,
    mask: u32,
    cookie: u32,
    name: &[u8],
) {
    let event = Event {
        wd,
        mask,
        cookie,
        name: name.to_vec(),
    };
    {
        let mut own = instance.borrow_mut();
        if own.queue.back() == Some(&event) {
            return;
        }
        if own.queue.len() >= MAX_QUEUED {
            let overflow = Event {
                wd: -1,
                mask: IN_Q_OVERFLOW,
                cookie: 0,
                name: Vec::new(),
            };
            if own.queue.back() != Some(&overflow) {
                own.queue.push_back(overflow);
            }
        } else {
            own.queue.push_back(event);
        }
    }
    kernel.wake_poll(channel(instance), EPOLLIN | EPOLLRDNORM);
}

/// Tells the watches of `node` of an event about it, `name` empty, or about the child `name`
/// it holds. A one-shot watch goes after its event.
/// An event about an open file whose name was removed (`unlinked`) skips the watches with
/// IN_EXCL_UNLINK (`fsnotify_handle_inode_event`'s `d_unlinked` check).
fn tell(
    kernel: &mut Kernel,
    node: &Node,
    mask: u32,
    cookie: u32,
    name: &[u8],
    directory: bool,
    unlinked: bool,
) {
    if node.borrow().watches.is_empty() {
        return;
    }
    let flag = if directory { IN_ISDIR } else { 0 };
    let marks: Vec<(Weak<RefCell<Instance>>, i32, u32)> = node
        .borrow()
        .watches
        .iter()
        .filter(|mark| mark.mask & mask != 0)
        .filter(|mark| !(unlinked && mark.mask & IN_EXCL_UNLINK != 0))
        .map(|mark| (mark.instance.clone(), mark.wd, mark.mask))
        .collect();
    for (instance, wd, wanted) in marks {
        let Some(instance) = instance.upgrade() else {
            continue;
        };
        queue(kernel, &instance, wd, (mask & wanted) | flag, cookie, name);
        if wanted & IN_ONESHOT != 0 {
            forget(kernel, node, &instance, wd);
        }
    }
}

/// A watch goes: IN_IGNORED is queued for it.
fn forget(kernel: &mut Kernel, node: &Node, instance: &Rc<RefCell<Instance>>, wd: i32) {
    node.borrow_mut().watches.retain(|mark| {
        !(mark.wd == wd && core::ptr::eq(mark.instance.as_ptr(), Rc::as_ptr(instance)))
    });
    instance.borrow_mut().watches.remove(&wd);
    queue(kernel, instance, wd, IN_IGNORED, 0, &[]);
}

/// An event on an inode and on the directory of the name it was reached by: the one given, else
/// the name it was made with or moved to while that still names it (`fsnotify` with
/// `fsnotify_parent`).
pub(crate) fn changed(kernel: &mut Kernel, node: &Node, at: Option<(&Node, &[u8])>, mask: u32) {
    reported(kernel, node, at, mask, false);
}

/// As [`changed`], for an event through a path that was `unlinked`.
fn reported(
    kernel: &mut Kernel,
    node: &Node,
    at: Option<(&Node, &[u8])>,
    mask: u32,
    unlinked: bool,
) {
    let directory = node.borrow().is_directory();
    match at {
        Some((parent, name)) => tell(kernel, parent, mask, 0, name, directory, unlinked),
        None => {
            if let Some((parent, name)) = place_of(node) {
                tell(kernel, &parent, mask, 0, &name, directory, unlinked);
            }
        }
    }
    tell(kernel, node, mask, 0, &[], directory, unlinked);
}

/// The directory and name an inode was made with or last moved to, if they still name it.
fn place_of(node: &Node) -> Option<(Node, Vec<u8>)> {
    let (parent, name) = {
        let inode = node.borrow();
        (
            inode.place.parent.as_ref()?.upgrade()?,
            inode.place.name.clone(),
        )
    };
    let current = parent.borrow().directory()?.get(&name)?;
    Rc::ptr_eq(&current, node).then_some((parent, name))
}

/// An event about an open file: on its inode and the directory it was opened in.
pub(crate) fn file_changed(kernel: &mut Kernel, file: &FileRef, mask: u32) {
    let (node, dentry) = {
        let file = file.borrow();
        if file.anon.is_some() {
            return;
        }
        (file.node.clone(), file.dentry.clone())
    };
    let parent = dentry.and_then(|(parent, name)| parent.upgrade().map(|parent| (parent, name)));
    // The name it was opened by is gone, and it was not moved to another (`d_unlinked`).
    let names_it = parent.as_ref().is_some_and(|(parent, name)| {
        parent
            .borrow()
            .directory()
            .and_then(|entries| entries.get(name))
            .is_some_and(|current| Rc::ptr_eq(&current, &node))
    });
    let unlinked = node.borrow().nlink == 0 || (!names_it && place_of(&node).is_none());
    // A file moved since it was opened is told of under its new name, as its dentry moved.
    match &parent {
        Some((parent, name)) if names_it || unlinked => {
            reported(kernel, &node, Some((parent, name)), mask, unlinked)
        }
        _ => reported(kernel, &node, None, mask, unlinked),
    }
}

/// An event on the inode alone, not its directory (`fsnotify_inode`).
pub(crate) fn inode_changed(kernel: &mut Kernel, node: &Node, mask: u32) {
    let directory = node.borrow().is_directory();
    tell(kernel, node, mask, 0, &[], directory, false);
}

/// Whether an open file other than `except` is of `node`: an inode with no name left lives on
/// while one is, and ends at the last close (`dentry_unlink_inode` at the last `dput`).
fn open_anywhere(kernel: &Kernel, node: &Node, except: Option<&FileRef>) -> bool {
    kernel.threads.values().any(|thread| {
        thread.files.borrow().iter().any(|(_, descriptor)| {
            !except.is_some_and(|file| Rc::ptr_eq(file, &descriptor.file))
                && Rc::ptr_eq(&descriptor.file.borrow().node, node)
        })
    })
}

/// An open file's last close: if it held the last of an inode with no name, the inode ends.
pub(crate) fn closed(kernel: &mut Kernel, file: &FileRef) {
    let node = file.borrow().node.clone();
    let gone = {
        let inode = node.borrow();
        inode.nlink == 0 && !inode.watches.is_empty()
    };
    if gone && !open_anywhere(kernel, &node, Some(file)) {
        ended(kernel, &node);
    }
}

/// A name was made in a directory (`fsnotify_create`, `fsnotify_mkdir`, `fsnotify_link`).
pub(crate) fn created(kernel: &mut Kernel, directory: &Node, name: &[u8], node: &Node) {
    let is_directory = node.borrow().is_directory();
    tell(kernel, directory, IN_CREATE, 0, name, is_directory, false);
}

/// A name was removed from a directory (`vfs_unlink`, `vfs_rmdir`): the inode's change of links,
/// its end if no link is left, and the directory's event.
pub(crate) fn removed(kernel: &mut Kernel, directory: &Node, name: &[u8], node: &Node) {
    let (is_directory, gone) = {
        let inode = node.borrow();
        (
            inode.is_directory(),
            inode.nlink == 0 || inode.is_directory(),
        )
    };
    if !is_directory {
        tell(kernel, node, IN_ATTRIB, 0, &[], false, false);
    }
    if gone && !open_anywhere(kernel, node, None) {
        ended(kernel, node);
    }
    tell(kernel, directory, IN_DELETE, 0, name, is_directory, false);
}

/// An inode with no name left (`fsnotify_inoderemove`): IN_DELETE_SELF, and every watch on it
/// goes.
pub(crate) fn ended(kernel: &mut Kernel, node: &Node) {
    if node.borrow().watches.is_empty() {
        return;
    }
    tell(kernel, node, IN_DELETE_SELF, 0, &[], false, false);
    let marks = core::mem::take(&mut node.borrow_mut().watches);
    for mark in marks {
        if let Some(instance) = mark.instance.upgrade() {
            instance.borrow_mut().watches.remove(&mark.wd);
            queue(kernel, &instance, mark.wd, IN_IGNORED, 0, &[]);
        }
    }
}

/// A rename (`fsnotify_move`): the two directories' events with one cookie, the replaced
/// target's change of links, and the moved inode's.
fn moved(
    kernel: &mut Kernel,
    from: (&Node, &[u8]),
    to: (&Node, &[u8]),
    node: &Node,
    replaced: Option<&Node>,
) {
    let directory = node.borrow().is_directory();
    kernel.move_cookie = kernel.move_cookie.wrapping_add(1);
    let cookie = kernel.move_cookie;
    tell(
        kernel,
        from.0,
        IN_MOVED_FROM,
        cookie,
        from.1,
        directory,
        false,
    );
    tell(kernel, to.0, IN_MOVED_TO, cookie, to.1, directory, false);
    if let Some(target) = replaced {
        let target_directory = target.borrow().is_directory();
        tell(kernel, target, IN_ATTRIB, 0, &[], target_directory, false);
    }
    // The inode's own move and end carry no IN_ISDIR (measured).
    tell(kernel, node, IN_MOVE_SELF, 0, &[], false, false);
}

/// `vfs_rename`'s events: the source's move; for an exchange the target's move back, else a
/// replaced target's end if it has no name left.
pub(crate) fn renamed(
    kernel: &mut Kernel,
    from: (&Node, &[u8]),
    to: (&Node, &[u8]),
    source: &Node,
    target: Option<&Node>,
    exchange: bool,
) {
    match target {
        Some(target) if exchange => {
            moved(kernel, from, to, source, None);
            moved(kernel, to, from, target, None);
        }
        Some(target) => {
            moved(kernel, from, to, source, Some(target));
            if target.borrow().nlink == 0 && !open_anywhere(kernel, target, None) {
                ended(kernel, target);
            }
        }
        None => moved(kernel, from, to, source, None),
    }
}

/// `inotify_poll`.
pub(super) fn poll(instance: &Instance) -> u16 {
    if instance.queue.is_empty() {
        0
    } else {
        (POLLIN | POLLRDNORM) as u16
    }
}

/// The bytes of events waiting, as FIONREAD gives them.
pub(super) fn waiting(instance: &Instance) -> usize {
    instance.queue.iter().map(Event::size).sum()
}

/// `inotify_read`: as many whole events as fit, waiting for one unless nonblocking; EINVAL when
/// the first does not fit.
pub(super) fn read(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    let instance = of(&file.borrow()).expect("an inotify instance");
    let room: u64 = parts.iter().map(|(_, length)| length).sum();
    if instance.borrow().queue.is_empty() {
        if file.borrow().flags & O_NONBLOCK != 0 {
            return Err(Errno::EAGAIN);
        }
        if kernel.signal_pending(tid) {
            return Err(Errno::ERESTARTSYS);
        }
        kernel.park(tid, channel(&instance));
        return Err(Errno::PARKED);
    }
    let mut bytes = Vec::new();
    let mut taken = 0;
    {
        let own = instance.borrow();
        for event in &own.queue {
            if (bytes.len() + event.size()) as u64 > room {
                break;
            }
            bytes.extend_from_slice(&event.wd.to_le_bytes());
            bytes.extend_from_slice(&event.mask.to_le_bytes());
            bytes.extend_from_slice(&event.cookie.to_le_bytes());
            let length = padded(&event.name);
            bytes.extend_from_slice(&(length as u32).to_le_bytes());
            let start = bytes.len();
            bytes.extend_from_slice(&event.name);
            bytes.resize(start + length, 0);
            taken += 1;
        }
    }
    if taken == 0 {
        return Err(Errno::EINVAL);
    }
    super::signalfd::store(kernel, cx, tid, parts, 0, &bytes)?;
    instance.borrow_mut().queue.drain(..taken);
    Ok(bytes.len() as u64)
}

/// FIONREAD on an instance.
pub(super) fn ioctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    instance: &Rc<RefCell<Instance>>,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    if request != crate::linux::ioctl::FIONREAD {
        return Err(Errno::ENOTTY);
    }
    let bytes = waiting(&instance.borrow()) as i32;
    guest(kernel, cx, tid).write_value(argument, &bytes)?;
    Ok(0)
}

/// An instance whose last descriptor went lets go of its watches.
pub(crate) fn released(instance: &Rc<RefCell<Instance>>) {
    let watches = core::mem::take(&mut instance.borrow_mut().watches);
    for (wd, node) in watches {
        if let Some(node) = node.upgrade() {
            node.borrow_mut().watches.retain(|mark| {
                !(mark.wd == wd && core::ptr::eq(mark.instance.as_ptr(), Rc::as_ptr(instance)))
            });
        }
    }
}
