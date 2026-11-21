//! Directory notification (`fs/notify/dnotify/dnotify.c`): F_NOTIFY puts a mark on a directory
//! for an open file of it and a descriptor table, and each change it asks for sends the file's
//! owner an I/O signal naming the descriptor.

use alloc::rc::{Rc, Weak};
use core::cell::RefCell;

use crate::file::{FileRef, OpenFile};
use crate::fs::Node;
use crate::linux::general::*;
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

/// One F_NOTIFY request (`struct dnotify_struct`).
pub(crate) struct Mark {
    /// The descriptor table it was asked through (`dn_owner`).
    table: u64,
    file: Weak<RefCell<OpenFile>>,
    fd: i32,
    /// The `DN_*` changes it wants, with DN_MULTISHOT to stay after the first.
    mask: u32,
}

/// The `DN_*` changes an inotify event's mask stands for (`convert_arg` read backwards).
fn changes(mask: u32) -> u32 {
    let mut changes = 0;
    if mask & IN_ACCESS != 0 {
        changes |= DN_ACCESS;
    }
    if mask & IN_MODIFY != 0 {
        changes |= DN_MODIFY;
    }
    if mask & IN_ATTRIB != 0 {
        changes |= DN_ATTRIB;
    }
    if mask & (IN_CREATE | IN_MOVED_TO) != 0 {
        changes |= DN_CREATE;
    }
    if mask & (IN_DELETE | IN_MOVED_FROM) != 0 {
        changes |= DN_DELETE;
    }
    changes
}

/// `fcntl_dirnotify`: no change asked takes the table's request on the file away; else the
/// changes are added to its request, or a new one is made, and the file gets the caller's
/// process as its owner if it has none.
pub(super) fn fcntl_notify(
    kernel: &Kernel,
    tid: Tid,
    fd: i32,
    file: &FileRef,
    argument: u64,
) -> Result<u64, Errno> {
    let mask = argument as u32;
    let table = kernel.threads[&tid].files.borrow().lock_owner();
    let node = file.borrow().node.clone();
    if mask & !DN_MULTISHOT == 0 {
        flush(&node, table, file);
        return Ok(0);
    }
    if !node.borrow().is_directory() {
        return Err(Errno::ENOTDIR);
    }
    if file.borrow().owner.is_none_or(|owner| owner.pid == 0) {
        let pid = kernel.process_of(tid).pid;
        super::io::set_owner_of(kernel, tid, file, pid, crate::file::OwnerKind::Process);
    }
    let mut inode = node.borrow_mut();
    let held = inode
        .dnotify
        .iter_mut()
        .find(|mark| mark.table == table && core::ptr::eq(mark.file.as_ptr(), Rc::as_ptr(file)));
    match held {
        Some(mark) => {
            mark.fd = fd;
            mark.mask |= mask;
        }
        // Newest first, as `attach_dn` links them.
        None => inode.dnotify.insert(
            0,
            Mark {
                table,
                file: Rc::downgrade(file),
                fd,
                mask,
            },
        ),
    }
    Ok(0)
}

/// `dnotify_flush`: a close through `table` takes its requests on `file` away.
pub(crate) fn flush(node: &Node, table: u64, file: &FileRef) {
    let mut inode = node.borrow_mut();
    if !inode.dnotify.is_empty() {
        inode.dnotify.retain(|mark| {
            !(mark.table == table && core::ptr::eq(mark.file.as_ptr(), Rc::as_ptr(file)))
        });
    }
}

/// `dnotify_handle_event`: an event on a directory with `name` about a child, or about the
/// directory itself; inotify's `mask`, or DN_RENAME's own `changes`. Each request that wants it
/// owes its owner a signal; a one-shot request goes after it.
pub(crate) fn tell(kernel: &mut Kernel, node: &Node, mask: u32, extra: u32) {
    let wanted = changes(mask) | extra;
    if wanted == 0 || node.borrow().dnotify.is_empty() {
        return;
    }
    let mut inode = node.borrow_mut();
    inode.dnotify.retain(|mark| {
        if mark.mask & wanted == 0 {
            return true;
        }
        kernel.dnotified.push((mark.file.clone(), mark.fd));
        mark.mask & DN_MULTISHOT != 0
    });
}

/// Sends the signals events owed (`send_sigio` with POLL_MSG), in order.
pub(crate) fn send<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>) {
    for (file, fd) in core::mem::take(&mut kernel.dnotified) {
        if let Some(file) = file.upgrade() {
            let owner = file.borrow().owner;
            if let Some(owner) = owner.filter(|owner| owner.pid != 0) {
                kernel.signal_owner(cx, &owner, fd, POLL_MSG);
            }
        }
    }
}
