//! inotify (`fs/notify/inotify/inotify_user.c` and the `fsnotify` hooks of the file layer): an
//! instance is a descriptor whose watches sit on inodes; the file calls tell the watches of the
//! inode they touch and, with the name, of the directory it is in.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use atomic_refcell::AtomicRefCell;

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
pub(crate) const MAX_QUEUED: usize = 16384;

/// `fs.inotify.max_user_instances` and `max_user_watches`: the reference's values, which bound
/// each user's instances and their watches (`inc_ucount`).
pub(crate) const MAX_USER_INSTANCES: usize = 8192;
pub(crate) const MAX_USER_WATCHES: usize = 1_048_576;

/// The live instances of user `uid`, forgetting those gone.
fn instances_of(kernel: &mut Kernel, uid: u32) -> Vec<Arc<AtomicRefCell<Instance>>> {
    kernel
        .inotify_instances
        .retain(|(_, instance)| instance.strong_count() > 0);
    kernel
        .inotify_instances
        .iter()
        .filter(|(owner, _)| *owner == uid)
        .filter_map(|(_, instance)| instance.upgrade())
        .collect()
}

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

/// An inotify instance: its watches by number and the events not yet read. A watch holds its
/// inode, as Linux's mark does (`fsnotify_get_inode_ref`), so a backend inode stays the one its
/// node number finds while watched.
#[derive(Default)]
pub(crate) struct Instance {
    /// Its number from `Kernel::next_object`.
    object: u64,
    watches: BTreeMap<i32, Arc<AtomicRefCell<Inode>>>,
    last: i32,
    queue: VecDeque<Event>,
}

/// A watch as its inode holds it.
pub(crate) struct Mark {
    instance: Weak<AtomicRefCell<Instance>>,
    wd: i32,
    mask: u32,
}

/// An instance's watches for fdinfo, newest first as Linux lists its marks: number, inode and
/// the mask a user sees (`inotify_mark_user_mask`).
pub(super) fn marks(instance: &Arc<AtomicRefCell<Instance>>) -> Vec<(i32, crate::fs::Node, u32)> {
    instance
        .borrow()
        .watches
        .iter()
        .rev()
        .filter_map(|(wd, inode)| {
            let node = inode.clone();
            let mask = node
                .borrow()
                .watches
                .iter()
                .find(|mark| {
                    mark.wd == *wd && core::ptr::eq(mark.instance.as_ptr(), Arc::as_ptr(instance))
                })?
                .mask;
            Some((
                *wd,
                node,
                mask & (IN_ALL_EVENTS | IN_ONESHOT | IN_EXCL_UNLINK),
            ))
        })
        .collect()
}

/// The instance behind an open file, if it is one.
pub(crate) fn of_file(file: &FileRef) -> Option<Arc<AtomicRefCell<Instance>>> {
    of(&file.borrow())
}

pub(super) fn of(file: &OpenFile) -> Option<Arc<AtomicRefCell<Instance>>> {
    match &file.anon {
        Some(Anon::Inotify(instance)) => Some(instance.clone()),
        _ => None,
    }
}

pub(super) fn channel(instance: &Arc<AtomicRefCell<Instance>>) -> Channel {
    Channel::Object(crate::process::ObjectId::Numbered(instance.borrow().object))
}

pub(super) fn inotify_init1<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    [flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(IN_CLOEXEC | IN_NONBLOCK) != 0 {
        return Err(Errno::EINVAL);
    }
    // An instance is charged to its maker's effective user (`inotify_new_group`).
    let uid = kernel.credentials(tid).euid;
    if instances_of(kernel, uid).len() >= MAX_USER_INSTANCES {
        return Err(Errno::EMFILE);
    }
    let node = super::anon_node(kernel, cx);
    let opener = kernel.credentials(tid).serial;
    let file = OpenFile::new(
        kernel.next_object(),
        node,
        O_RDONLY | (flags & O_NONBLOCK),
        LINK.to_vec(),
        opener,
    );
    let instance = Arc::new(AtomicRefCell::new(Instance {
        object: kernel.next_object(),
        ..Instance::default()
    }));
    kernel
        .inotify_instances
        .push((uid, Arc::downgrade(&instance)));
    file.borrow_mut().anon = Some(Anon::Inotify(instance));
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let fd = kernel
        .files_mut(tid)
        .add(file, flags & IN_CLOEXEC != 0, 0, limit)?;
    Ok(fd as u64)
}

/// The instance a descriptor names: EBADF without one (`fdget` passes over O_PATH files), EINVAL
/// if it is not an instance.
fn instance(kernel: &Kernel, tid: Tid, fd: u64) -> Result<Arc<AtomicRefCell<Instance>>, Errno> {
    let file = super::io::file(kernel, tid, fd)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    of(&file.borrow()).ok_or(Errno::EINVAL)
}

pub(super) fn inotify_add_watch<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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
    let instance = instance(kernel, tid, fd)?;
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
        .position(|mark| core::ptr::eq(mark.instance.as_ptr(), Arc::as_ptr(&instance)));
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
    // A new watch is charged to the instance's user (`inotify_new_watch`).
    let owner = kernel
        .inotify_instances
        .iter()
        .find(|(_, other)| core::ptr::eq(other.as_ptr(), Arc::as_ptr(&instance)))
        .map(|(uid, _)| *uid);
    if let Some(uid) = owner {
        let watches: usize = instances_of(kernel, uid)
            .iter()
            .map(|other| other.borrow().watches.len())
            .sum();
        if watches >= MAX_USER_WATCHES {
            return Err(Errno::ENOSPC);
        }
    }
    let wd = {
        let mut own = instance.borrow_mut();
        own.last += 1;
        let wd = own.last;
        own.watches.insert(wd, node.clone());
        wd
    };
    node.borrow_mut().watches.push(Mark {
        instance: Arc::downgrade(&instance),
        wd,
        mask: wanted,
    });
    Ok(wd as u64)
}

pub(super) fn inotify_rm_watch<S: crate::System>(
    kernel: &mut Kernel,
    _: &mut Context<'_, S>,
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
    node.borrow_mut().watches.retain(|mark| {
        !(mark.wd == wd && core::ptr::eq(mark.instance.as_ptr(), Arc::as_ptr(&instance)))
    });
    queue(kernel, &instance, wd, IN_IGNORED, 0, &[]);
    Ok(0)
}

/// Queues an event, merged with the last one if it is the same (`inotify_merge`), or the queue's
/// overflow once it is full.
fn queue(
    kernel: &mut Kernel,
    instance: &Arc<AtomicRefCell<Instance>>,
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
    // F_NOTIFY requests hear of a directory's children and of the directory itself.
    if !name.is_empty() || directory {
        super::dnotify::tell(kernel, node, mask, 0);
    }
    if node.borrow().watches.is_empty() {
        return;
    }
    let flag = if directory { IN_ISDIR } else { 0 };
    let marks: Vec<(Weak<AtomicRefCell<Instance>>, i32, u32)> = node
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
fn forget(kernel: &mut Kernel, node: &Node, instance: &Arc<AtomicRefCell<Instance>>, wd: i32) {
    node.borrow_mut().watches.retain(|mark| {
        !(mark.wd == wd && core::ptr::eq(mark.instance.as_ptr(), Arc::as_ptr(instance)))
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

/// The directory and name an inode was made with or last moved to, if they still name it. On
/// the backend, the name its last lookup found, while the inode has a link.
fn place_of(node: &Node) -> Option<(Node, Vec<u8>)> {
    let (parent, name) = {
        let inode = node.borrow();
        (
            inode.place.parent.as_ref()?.upgrade()?,
            inode.place.name.clone(),
        )
    };
    if crate::fs::backend::remote(&parent).is_some() {
        return (node.borrow().nlink > 0).then_some((parent, name));
    }
    let current = parent.borrow().directory()?.get(&name)?;
    Arc::ptr_eq(&current, node).then_some((parent, name))
}

/// An event about an open file: on its inode and the directory it was opened in.
pub(crate) fn file_changed(kernel: &mut Kernel, file: &FileRef, mask: u32) {
    let (node, dentry, opened) = {
        let file = file.borrow();
        if file.anon.is_some() {
            return;
        }
        (file.node.clone(), file.dentry.clone(), file.id)
    };
    let parent = dentry.and_then(|(parent, name)| parent.upgrade().map(|parent| (parent, name)));
    // The name it was opened by is gone, and it was not moved to another (`d_unlinked`). A
    // backend directory's names are the backend's: there a name goes with an unlink or a
    // replacing rename through the kernel, which notes it on the inode.
    let removed_from_backend = parent.as_ref().is_some_and(|(parent, name)| {
        crate::fs::backend::remote(parent).is_some()
            && node
                .borrow()
                .name_removed_since(&Arc::downgrade(parent), name, opened)
    });
    let names_it = parent.as_ref().is_some_and(|(parent, name)| {
        if crate::fs::backend::remote(parent).is_some() {
            return node.borrow().nlink > 0 && !removed_from_backend;
        }
        parent
            .borrow()
            .directory()
            .and_then(|entries| entries.get(name))
            .is_some_and(|current| Arc::ptr_eq(&current, &node))
    });
    let unlinked = node.borrow().nlink == 0
        || removed_from_backend
        || (!names_it && place_of(&node).is_none());
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
            !except.is_some_and(|file| Arc::ptr_eq(file, &descriptor.file))
                && Arc::ptr_eq(&descriptor.file.borrow().node, node)
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
    // FS_RENAME, which only F_NOTIFY requests take, comes first, for a rename within one
    // directory (`fsnotify_handle_event`).
    if Arc::ptr_eq(from.0, to.0) {
        super::dnotify::tell(kernel, from.0, 0, DN_RENAME);
    }
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
pub(super) fn read<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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
pub(super) fn ioctl<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    instance: &Arc<AtomicRefCell<Instance>>,
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
pub(crate) fn released(instance: &Arc<AtomicRefCell<Instance>>) {
    let watches = core::mem::take(&mut instance.borrow_mut().watches);
    for (wd, node) in watches {
        node.borrow_mut().watches.retain(|mark| {
            !(mark.wd == wd && core::ptr::eq(mark.instance.as_ptr(), Arc::as_ptr(instance)))
        });
    }
}
