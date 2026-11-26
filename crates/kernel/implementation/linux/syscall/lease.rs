//! File leases (`fs/locks.c`): F_SETLEASE gives an open regular file a read or write lease. An
//! open that conflicts with one waits while the holder is told with an I/O signal, until the
//! holder lets go or comes down to a read lease, or `lease-break-time` passes.

use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use atomic_refcell::AtomicRefCell;
use wcontainers_abi::NANOSECONDS_PER_SECOND;
use wcontainers_executor::Executor;

use crate::file::{FileRef, OpenFile};
use crate::fs::{Kind, Node};
use crate::linux::general::*;
use crate::process::{Channel, Tid};
use crate::{Context, Errno, Kernel};

/// `fs.lease-break-time`: the seconds a holder has to let go (Linux's default).
pub(crate) const BREAK_SECONDS: u64 = 45;

/// One lease (`struct file_lease`), held by an open file.
pub(crate) struct Lease {
    file: Weak<AtomicRefCell<OpenFile>>,
    /// The descriptor its I/O signal names (`fa_fd`).
    fd: i32,
    /// F_RDLCK or F_WRLCK.
    kind: u32,
    /// A conflicting open waits: what the holder must come down to (F_UNLCK or F_RDLCK, Linux's
    /// `FL_UNLOCK_PENDING` and `FL_DOWNGRADE_PENDING`), and by when on the monotonic clock.
    pending: Option<(u32, u64)>,
}

impl Lease {
    fn held_by(&self, file: &FileRef) -> bool {
        core::ptr::eq(self.file.as_ptr(), Arc::as_ptr(file))
    }

    /// What F_GETLEASE shows (`target_leasetype`).
    fn target(&self) -> u32 {
        self.pending.map_or(self.kind, |(target, _)| target)
    }
}

/// `time_out_leases`: leases whose holders let the break time pass are removed or brought down;
/// leases of files that are gone go too. Waiting openers look again.
fn time_out<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    node: &Node,
) {
    if node.borrow().leases.is_empty() {
        return;
    }
    let now = cx.monotonic();
    let mut changed = false;
    node.borrow_mut().leases.retain_mut(|lease| {
        if lease.file.strong_count() == 0 {
            changed = true;
            return false;
        }
        match lease.pending {
            Some((F_UNLCK, deadline)) if now >= deadline => {
                changed = true;
                if let Some(file) = lease.file.upgrade() {
                    disown(&file);
                }
                false
            }
            Some((target, deadline)) if now >= deadline => {
                lease.kind = target;
                lease.pending = None;
                changed = true;
                true
            }
            _ => true,
        }
    });
    if changed {
        kernel.wake(Channel::Locks);
    }
}

/// A lease let go of (`lease_modify` to F_UNLCK) takes its file's owner and signal with it
/// (`f_delown`).
fn disown(file: &FileRef) {
    let mut file = file.borrow_mut();
    let owner = file.owner.get_or_insert_default();
    *owner = crate::file::Owner {
        kind: crate::file::OwnerKind::Process,
        uid: owner.uid,
        euid: owner.euid,
        ..Default::default()
    };
}

/// Whether some open file has `node` open for writing (`i_writecount` above 0).
pub(crate) fn open_for_writing(kernel: &Kernel, node: &Node) -> bool {
    kernel.threads.values().any(|thread| {
        thread.files.borrow().iter().any(|(_, descriptor)| {
            let file = descriptor.file.borrow();
            Arc::ptr_eq(&file.node, node) && !file.is_path() && file.writable() && !file.pseudo
        })
    })
}

/// Whether a live process runs `node` as its program (`exe_file_deny_write_access`, so
/// `i_writecount` is below 0).
pub(crate) fn running(kernel: &Kernel, node: &Node) -> bool {
    kernel
        .processes
        .values()
        .any(|process| process.ended.is_none() && Arc::ptr_eq(&process.executable.0, node))
}

/// The open files of `node` for reading only and for writing (`i_readcount`, `i_writecount`),
/// an opener waiting to write counting as one.
fn open_counts(kernel: &Kernel, node: &Node) -> (usize, usize) {
    let mut seen: Vec<*const AtomicRefCell<OpenFile>> = Vec::new();
    let (mut readers, mut writers) = (0, 0);
    for thread in kernel.threads.values() {
        for (_, descriptor) in thread.files.borrow().iter() {
            let pointer = Arc::as_ptr(&descriptor.file);
            let file = descriptor.file.borrow();
            if !Arc::ptr_eq(&file.node, node) || file.is_path() || seen.contains(&pointer) {
                continue;
            }
            seen.push(pointer);
            if file.writable() {
                writers += usize::from(!file.pseudo);
            } else if file.readable() {
                readers += 1;
            }
        }
    }
    if node
        .borrow()
        .leases
        .iter()
        .any(|lease| matches!(lease.pending, Some((F_UNLCK, _))))
    {
        writers += 1;
    }
    (readers, writers)
}

/// `check_conflicting_open`: a read lease wants no writer; a write lease wants no other opener.
fn conflicting_open(kernel: &Kernel, node: &Node, file: &FileRef, kind: u32) -> Result<(), Errno> {
    let (readers, writers) = open_counts(kernel, node);
    let conflicts = if kind == F_RDLCK {
        writers > 0
    } else {
        let own = file.borrow();
        let own_writer = usize::from(own.writable());
        let own_reader = usize::from(!own.writable() && own.readable());
        writers != own_writer || readers != own_reader
    };
    if conflicts {
        Err(Errno::EAGAIN)
    } else {
        Ok(())
    }
}

/// F_SETLEASE (`fcntl_setlease`, `vfs_setlease`, `generic_setlease`).
pub(super) fn set<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    fd: i32,
    file: &FileRef,
    argument: u64,
) -> Result<u64, Errno> {
    let kind = argument as u32;
    if !matches!(kind, F_RDLCK | F_WRLCK | F_UNLCK) {
        return Err(Errno::EINVAL);
    }
    let node = file.borrow().node.clone();
    // `vfs_setlease`: only file systems with a `setlease` take leases.
    if !matches!(
        node.borrow().fs.kind,
        Kind::Tmpfs | Kind::Overlay | Kind::Fuse
    ) {
        return Err(Errno::EINVAL);
    }
    let credentials = kernel.credentials(tid);
    if credentials.fsuid != node.borrow().uid && !credentials.has(CAP_LEASE) {
        return Err(Errno::EACCES);
    }
    if node.borrow().mode & S_IFMT != S_IFREG {
        return Err(Errno::EINVAL);
    }
    time_out(kernel, cx, &node);
    if kind == F_UNLCK {
        // `generic_delete_lease`.
        let mut inode = node.borrow_mut();
        let before = inode.leases.len();
        inode.leases.retain(|lease| !lease.held_by(file));
        let removed = inode.leases.len() != before;
        drop(inode);
        if !removed {
            return Err(Errno::EAGAIN);
        }
        disown(file);
        kernel.wake(Channel::Locks);
        return Ok(0);
    }
    // `generic_add_lease`.
    conflicting_open(kernel, &node, file, kind)?;
    let mut inode = node.borrow_mut();
    let others_refuse = inode.leases.iter().any(|lease| {
        !lease.held_by(file) && (kind == F_WRLCK || matches!(lease.pending, Some((F_UNLCK, _))))
    });
    if others_refuse {
        return Err(Errno::EAGAIN);
    }
    match inode.leases.iter_mut().find(|lease| lease.held_by(file)) {
        // `lease_modify` and `lease_clear_pending`.
        Some(lease) => {
            lease.kind = kind;
            lease.fd = fd;
            if kind == F_RDLCK && matches!(lease.pending, Some((F_RDLCK, _))) {
                lease.pending = None;
            }
        }
        None => inode.leases.push(Lease {
            file: Arc::downgrade(file),
            fd,
            kind,
            pending: None,
        }),
    }
    drop(inode);
    kernel.wake(Channel::Locks);
    // `lease_setup`: the caller's process owns the file if nothing does.
    if file.borrow().owner.is_none_or(|owner| owner.pid == 0) {
        let pid = kernel.process_of(tid).pid;
        super::io::set_owner_of(kernel, tid, file, pid, crate::file::OwnerKind::Process);
    }
    Ok(0)
}

/// F_GETLEASE (`fcntl_getlease`).
pub(super) fn get<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    file: &FileRef,
) -> u64 {
    let node = file.borrow().node.clone();
    time_out(kernel, cx, &node);
    let inode = node.borrow();
    let lease = inode.leases.iter().find(|lease| lease.held_by(file));
    u64::from(lease.map_or(F_UNLCK, Lease::target))
}

/// `__break_lease`: an open of `node` with `flags` (or a truncation, as O_WRONLY) that conflicts
/// with leases tells their holders and waits for them, or is EWOULDBLOCK with O_NONBLOCK.
pub(crate) fn break_leases<S: crate::System, C: Executor + 'static>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S, C>,
    tid: Tid,
    node: &Node,
    flags: u32,
) -> Result<(), Errno> {
    time_out(kernel, cx, node);
    let want_write = flags & O_ACCMODE != O_RDONLY;
    let deadline = cx.monotonic() + BREAK_SECONDS * NANOSECONDS_PER_SECOND;
    let mut told = Vec::new();
    let mut waits_until = None::<u64>;
    for lease in node.borrow_mut().leases.iter_mut() {
        if !want_write && lease.kind != F_WRLCK {
            continue;
        }
        let target = if want_write { F_UNLCK } else { F_RDLCK };
        let marked = match lease.pending {
            Some((F_UNLCK, _)) => true,
            Some(_) if !want_write => true,
            _ => false,
        };
        if !marked {
            lease.pending = Some((target, deadline));
            told.push((lease.file.clone(), lease.fd));
        }
        let until = lease.pending.map_or(deadline, |(_, until)| until);
        waits_until = Some(waits_until.map_or(until, |earlier| earlier.min(until)));
    }
    // `lease_break_callback`: the holder's I/O signal.
    for (file, fd) in told {
        if let Some(file) = file.upgrade()
            && let Some(owner) = file.borrow().owner.filter(|owner| owner.pid != 0)
        {
            kernel.signal_owner(cx, &owner, fd, POLL_MSG);
        }
    }
    let Some(until) = waits_until else {
        return Ok(());
    };
    if flags & O_NONBLOCK != 0 {
        return Err(Errno::EAGAIN);
    }
    if kernel.signal_pending(tid) {
        return Err(Errno::ERESTARTSYS);
    }
    kernel.thread_mut(tid).call.deadline = Some(until);
    kernel.park(tid, Channel::Locks);
    Err(Errno::PARKED)
}

/// An open file's last close lets go of its leases (`locks_remove_lease`).
pub(crate) fn released(kernel: &mut Kernel, file: &FileRef) {
    let node = file.borrow().node.clone();
    let mut inode = node.borrow_mut();
    if inode.leases.is_empty() {
        return;
    }
    let before = inode.leases.len();
    inode
        .leases
        .retain(|lease| !lease.held_by(file) && lease.file.strong_count() > 0);
    let changed = inode.leases.len() != before;
    drop(inode);
    if changed {
        kernel.wake(Channel::Locks);
    }
}
