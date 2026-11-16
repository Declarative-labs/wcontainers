//! Calls that name files by path: opening, making, removing, renaming, and reading and changing what a
//! file's inode records.

use alloc::rc::Rc;
use alloc::vec::Vec;

use super::{Args, guest};
use crate::container::make;
use crate::counters::reason;
use crate::credentials::{
    CAP_CHOWN, CAP_FOWNER, CAP_FSETID, CAP_MKNOD, CAP_SYS_CHROOT, Credentials,
};
use crate::file::OpenFile;
use crate::fs::dev::Device;
use crate::fs::{Content, Directory, Node, Regular, Time, proc};
use crate::linux::general::*;
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

/// The start of a walk for a `*at` call: the directory `dirfd` names, or the working directory.
pub(super) fn start(kernel: &Kernel, tid: Tid, dirfd: u64, path: &[u8]) -> Result<Node, Errno> {
    let process = kernel.process_of(tid);
    let dirfd = dirfd as i32;
    if path.starts_with(b"/") {
        return Ok(process.root.clone());
    }
    if dirfd == AT_FDCWD {
        return Ok(process.cwd.clone());
    }
    let node = process.files.file(dirfd)?.borrow().node.clone();
    if !path.is_empty() && !node.borrow().is_directory() {
        return Err(Errno::ENOTDIR);
    }
    Ok(node)
}

/// The inode a `*at` call names. An empty path names `dirfd` itself when `AT_EMPTY_PATH` is given.
pub(super) fn lookup(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    dirfd: u64,
    address: u64,
    flags: u32,
    follow: bool,
) -> Result<Node, Errno> {
    let path = if flags & AT_EMPTY_PATH != 0 {
        guest(kernel, cx, tid).read_path_maybe_empty(address)?
    } else {
        guest(kernel, cx, tid).read_path(address)?
    };
    if path.is_empty() {
        let process = kernel.process_of(tid);
        if dirfd as i32 == AT_FDCWD {
            return Ok(process.cwd.clone());
        }
        return Ok(process.files.file(dirfd as i32)?.borrow().node.clone());
    }
    let at = start(kernel, tid, dirfd, &path)?;
    kernel.with_walk(tid, |walk, _| walk.resolve(&at, &path, follow))
}

fn parent(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    dirfd: u64,
    address: u64,
) -> Result<crate::fs::Parent, Errno> {
    let path = guest(kernel, cx, tid).read_path(address)?;
    let at = start(kernel, tid, dirfd, &path)?;
    kernel.with_walk(tid, |walk, _| walk.parent(&at, &path))
}

/// The absolute path of `name` in `directory`, for `/proc/<pid>/fd`.
fn path_in(kernel: &Kernel, tid: Tid, directory: &Node, name: &[u8]) -> Vec<u8> {
    let mut path = kernel
        .with_walk(tid, |walk, _| {
            Ok(walk.path_of(directory).unwrap_or_default())
        })
        .unwrap_or_default();
    if !name.is_empty() {
        if !path.ends_with(b"/") {
            path.push(b'/');
        }
        path.extend_from_slice(name);
    }
    path
}

fn credentials(kernel: &Kernel, tid: Tid) -> &Credentials {
    &kernel.process_of(tid).credentials
}

/// May the caller make or remove names in this directory.
pub(super) fn may_change(kernel: &Kernel, tid: Tid, directory: &Node) -> Result<(), Errno> {
    let inode = directory.borrow();
    if !inode.is_directory() {
        return Err(Errno::ENOTDIR);
    }
    let caller = kernel.threads[&tid].process;
    crate::fs::permission(&inode, credentials(kernel, tid), W_OK | X_OK, caller)?;
    if inode.fs.read_only() {
        return Err(Errno::EROFS);
    }
    Ok(())
}

/// Whether a directory has no operations that change names: the process information tree, and
/// devpts (its root has only `simple_dir_inode_operations`' lookup).
fn fixed_names(directory: &Node) -> bool {
    let inode = directory.borrow();
    matches!(inode.content, Content::Proc(_)) || inode.fs.kind == crate::fs::Kind::Devpts
}

/// The mqueue file system's root, whose only name operations make and remove queues.
fn only_queues(directory: &Node) -> bool {
    directory.borrow().fs.kind == crate::fs::Kind::Mqueue
}

/// Callers that get this far in a directory with fixed names, or one that only makes and removes
/// queues, are refused with EPERM, as Linux does for a directory with no such operation.
fn changes_names(directory: &Node) -> Result<(), Errno> {
    if fixed_names(directory) || only_queues(directory) {
        return Err(Errno::EPERM);
    }
    Ok(())
}

/// The sticky bit: only the owner of the file or the directory, or `CAP_FOWNER`, may remove or rename.
fn sticky_allows(directory: &Node, node: &Node, credentials: &Credentials) -> bool {
    let directory = directory.borrow();
    if directory.mode & S_ISVTX == 0 {
        return true;
    }
    let inode = node.borrow();
    credentials.fsuid == inode.uid
        || credentials.fsuid == directory.uid
        || credentials.has(CAP_FOWNER)
}

/// Linux's `may_linkat` with `fs.protected_hardlinks` on (as in the reference): only the owner, a
/// caller with `CAP_FOWNER`, or one who may read and write a plain file may link to it.
fn may_hard_link(node: &Node, credentials: &Credentials) -> bool {
    let inode = node.borrow();
    if credentials.owns_or_capable(inode.uid) {
        return true;
    }
    let plain = inode.file_type() == S_IFREG
        && inode.mode & S_ISUID == 0
        && inode.mode & (S_ISGID | S_IXGRP) != S_ISGID | S_IXGRP;
    plain && inode.permits(credentials, R_OK | W_OK, true)
}

/// The owner and group a new inode gets: the caller's file system ids, or the directory's group when it
/// has the set-group bit.
fn new_owner(directory: &Node, credentials: &Credentials) -> (u32, u32, bool) {
    let directory = directory.borrow();
    if directory.mode & S_ISGID != 0 {
        (credentials.fsuid, directory.gid, true)
    } else {
        (credentials.fsuid, credentials.fsgid, false)
    }
}

fn touch_directory(directory: &Node, now: Time) {
    let mut inode = directory.borrow_mut();
    inode.mtime = now;
    inode.ctime = now;
}

/// A directory that has been removed takes no new names (Linux's `IS_DEADDIR`, `ENOENT`).
fn not_removed(directory: &Node) -> Result<(), Errno> {
    let inode = directory.borrow();
    if matches!(inode.content, Content::Directory(_)) && inode.nlink == 0 {
        return Err(Errno::ENOENT);
    }
    Ok(())
}

/// The checks before making a name: it must not exist (`EEXIST`). In the process information tree a
/// missing name is an error of the lookup itself (`ENOENT`), as Linux's proc lookups return it.
fn check_absent(kernel: &Kernel, tid: Tid, parent: &crate::fs::Parent) -> Result<(), Errno> {
    not_removed(&parent.directory)?;
    match kernel.with_walk(tid, |walk, _| walk.step(&parent.directory, &parent.name)) {
        Ok(_) => Err(Errno::EEXIST),
        Err(Errno::ENOENT) if matches!(parent.directory.borrow().content, Content::Proc(_)) => {
            Err(Errno::ENOENT)
        }
        Err(Errno::ENOENT) => Ok(()),
        Err(error) => Err(error),
    }
}

/// Makes an inode in a directory as a call from `tid` would.
#[allow(
    clippy::too_many_arguments,
    reason = "each is one attribute of the new inode"
)]
pub(super) fn create(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    directory: &Node,
    name: &[u8],
    mode: u32,
    rdev: u64,
    content: Content,
) -> Result<Node, Errno> {
    // A plain file made in the mqueue file system is a queue with the default limits
    // (`mqueue_create`).
    let content = match content {
        Content::Regular(_) if only_queues(directory) => Content::Mqueue(alloc::boxed::Box::new(
            super::mqueue::new_queue(kernel, tid, None)?,
        )),
        other => other,
    };
    let now = kernel.now(cx);
    let (uid, gid, inherit) = new_owner(directory, credentials(kernel, tid));
    let mut mode = mode;
    if inherit && mode & S_IFMT == S_IFDIR {
        mode |= S_ISGID;
    } else if mode & S_ISGID != 0
        && mode & S_IFMT != S_IFDIR
        && !credentials(kernel, tid).in_group(gid, true)
        && !credentials(kernel, tid).has(CAP_FSETID)
    {
        mode &= !S_ISGID;
    }
    let node = make(
        &mut kernel.files.numbers,
        directory,
        name,
        mode,
        uid,
        gid,
        rdev,
        now,
        content,
    )?;
    touch_directory(directory, now);
    Ok(node)
}

pub(super) fn openat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, flags, mode, ..]: Args,
) -> Result<u64, Errno> {
    // The flags are checked first (`build_open_flags`), then a descriptor is taken, before the path
    // is read or walked (`do_sys_openat2`'s `FD_ADD`).
    open_flags(flags as u32)?;
    free_descriptor(kernel, tid)?;
    let path = guest(kernel, cx, tid).read_path(path)?;
    open(kernel, cx, tid, dirfd, &path, flags as u32, mode as u32)
}

/// The size of `open_how` as first defined, the least `openat2` takes.
const OPEN_HOW_SIZE: u64 = 24;

pub(super) fn openat2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, how, size, ..]: Args,
) -> Result<u64, Errno> {
    // In the order of Linux's `openat2`, `copy_struct_from_user` and `build_open_flags`, which here
    // checks every argument.
    if size < OPEN_HOW_SIZE {
        return Err(Errno::EINVAL);
    }
    if size > wcontainers_abi::PAGE_SIZE {
        return Err(Errno::E2BIG);
    }
    if size > OPEN_HOW_SIZE {
        let rest = guest(kernel, cx, tid).read(how + OPEN_HOW_SIZE, size - OPEN_HOW_SIZE)?;
        if rest.iter().any(|byte| *byte != 0) {
            return Err(Errno::E2BIG);
        }
    }
    let how: open_how = guest(kernel, cx, tid).read_value(how)?;
    let flags = how.flags & !u64::from(O_CLOEXEC);
    if flags & !u64::from(VALID_OPEN_FLAGS) != 0
        || how.resolve & !u64::from(VALID_RESOLVE_FLAGS) != 0
        || how.resolve & u64::from(RESOLVE_BENEATH | RESOLVE_IN_ROOT)
            == u64::from(RESOLVE_BENEATH | RESOLVE_IN_ROOT)
    {
        return Err(Errno::EINVAL);
    }
    let flags = flags as u32;
    let creates = flags & (O_CREAT | (O_TMPFILE & !O_DIRECTORY)) != 0;
    if (creates && how.mode & !0o7777 != 0) || (!creates && how.mode != 0) {
        return Err(Errno::EINVAL);
    }
    if flags & O_PATH != 0 && flags & !PATH_FLAGS != 0 {
        return Err(Errno::EINVAL);
    }
    open_flags(flags)?;
    if how.resolve & u64::from(RESOLVE_CACHED) != 0 && flags & (O_TRUNC | O_CREAT | O_TMPFILE) != 0
    {
        return Err(Errno::EAGAIN);
    }
    free_descriptor(kernel, tid)?;
    // Lookups restricted by `resolve` are not built. Linux has had them as long as it has had openat2,
    // so these requests are answered as a kernel before openat2 answers the whole call: ENOSYS.
    if how.resolve & !u64::from(RESOLVE_CACHED) != 0 {
        return Err(kernel
            .counters
            .refuse_flag(cx.host, __NR_openat2, how.resolve, Errno::ENOSYS));
    }
    let path = guest(kernel, cx, tid).read_path(path)?;
    let flags = flags | (how.flags as u32 & O_CLOEXEC);
    open(kernel, cx, tid, dirfd, &path, flags, how.mode as u32)
}

/// Every `resolve` flag Linux knows.
const VALID_RESOLVE_FLAGS: u32 = RESOLVE_NO_XDEV
    | RESOLVE_NO_MAGICLINKS
    | RESOLVE_NO_SYMLINKS
    | RESOLVE_BENEATH
    | RESOLVE_IN_ROOT
    | RESOLVE_CACHED;

/// Linux's `get_unused_fd_flags` before an open: EMFILE when no descriptor is free below the limit.
fn free_descriptor(kernel: &Kernel, tid: Tid) -> Result<(), Errno> {
    let process = kernel.process_of(tid);
    let limit = process.limits[RLIMIT_NOFILE as usize].current;
    process.files.lowest_free(0, limit).map(|_| ())
}

/// The flags an open keeps, after `build_open_flags`'s checks.
fn open_flags(flags: u32) -> Result<u32, Errno> {
    let mut flags = flags & VALID_OPEN_FLAGS;
    if flags & O_PATH != 0 {
        flags &= PATH_FLAGS;
    }
    if flags & (O_DIRECTORY | O_CREAT) == O_DIRECTORY | O_CREAT {
        return Err(Errno::EINVAL);
    }
    let temporary = O_TMPFILE & !O_DIRECTORY;
    if flags & temporary != 0
        && (flags & (O_TMPFILE | O_CREAT) != O_TMPFILE
            || !matches!(flags & O_ACCMODE, O_WRONLY | O_RDWR))
    {
        return Err(Errno::EINVAL);
    }
    Ok(flags)
}

/// The flags `open` keeps; unknown bits are ignored, as Linux does for `open` and `openat`.
const VALID_OPEN_FLAGS: u32 = O_ACCMODE
    | FASYNC
    | O_CREAT
    | O_EXCL
    | O_NOCTTY
    | O_TRUNC
    | O_APPEND
    | O_NONBLOCK
    | O_DSYNC
    | O_DIRECT
    | O_LARGEFILE
    | O_DIRECTORY
    | O_NOFOLLOW
    | O_NOATIME
    | O_CLOEXEC
    | O_PATH
    | O_SYNC
    | O_TMPFILE;

/// The flags an `O_PATH` open keeps.
const PATH_FLAGS: u32 = O_DIRECTORY | O_NOFOLLOW | O_PATH | O_CLOEXEC;

pub(super) fn open(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    dirfd: u64,
    path: &[u8],
    flags: u32,
    mode: u32,
) -> Result<u64, Errno> {
    let flags = open_flags(flags)?;
    if kernel.threads[&tid].opening.is_some() {
        let file = super::pipe::open_named_again(kernel, tid)?;
        return named_opened(kernel, cx, tid, file, flags);
    }
    if flags & (O_TMPFILE & !O_DIRECTORY) != 0 {
        return Err(kernel.counters.refuse_flag(
            cx.host,
            __NR_openat,
            O_TMPFILE.into(),
            Errno::EOPNOTSUPP,
        ));
    }
    // Access mode 3 asks for both permissions and gives neither, as on Linux.
    let accmode = flags & O_ACCMODE;
    let at = start(kernel, tid, dirfd, path)?;
    let pid = kernel.threads[&tid].process;
    let parent = kernel.with_walk(tid, |walk, _| walk.parent(&at, path))?;
    let creating = flags & O_CREAT != 0 && flags & O_PATH == 0;
    // Creating a name written with a trailing slash is refused before it is looked up.
    if creating && parent.trailing_slash && !parent.name.is_empty() {
        return Err(Errno::EISDIR);
    }
    let found = if parent.name.is_empty() {
        Some(parent.directory.clone())
    } else {
        match kernel.with_walk(tid, |walk, _| walk.step(&parent.directory, &parent.name)) {
            Ok(node) => Some(node),
            // A proc lookup that fails is an error, not a name that may be made.
            Err(Errno::ENOENT)
                if !matches!(parent.directory.borrow().content, Content::Proc(_)) =>
            {
                None
            }
            Err(error) => return Err(error),
        }
    };
    let (node, created) = match found {
        Some(node) => {
            if creating && flags & O_EXCL != 0 {
                return Err(Errno::EEXIST);
            }
            let is_link = node.borrow().file_type() == S_IFLNK;
            let node = if is_link && !(creating && flags & O_EXCL != 0) {
                // A trailing slash follows the link whatever the flags say.
                if flags & O_NOFOLLOW != 0 && !parent.trailing_slash {
                    // A link not followed is not a directory, which is checked before opening it.
                    if flags & O_PATH != 0 {
                        node
                    } else if flags & O_DIRECTORY != 0 {
                        return Err(Errno::ENOTDIR);
                    } else {
                        return Err(Errno::ELOOP);
                    }
                } else {
                    let mut links = 0;
                    let followed = kernel.with_walk(tid, |walk, _| {
                        walk.follow(&parent.directory, node.clone(), &mut links)
                    });
                    match followed {
                        Ok(node) => node,
                        // A link to a missing file creates that file.
                        Err(Errno::ENOENT) if creating => {
                            let target = match &node.borrow().content {
                                Content::Symlink(target) => target.clone(),
                                _ => return Err(Errno::ENOENT),
                            };
                            return open(
                                kernel,
                                cx,
                                tid,
                                dirfd,
                                &resolve_relative(path, &target),
                                flags,
                                mode,
                            );
                        }
                        Err(error) => return Err(error),
                    }
                }
            } else {
                node
            };
            if creating && node.borrow().is_directory() {
                return Err(Errno::EISDIR);
            }
            (node, false)
        }
        None => {
            if !creating {
                return Err(Errno::ENOENT);
            }
            if parent.trailing_slash {
                return Err(Errno::EISDIR);
            }
            not_removed(&parent.directory)?;
            may_change(kernel, tid, &parent.directory)?;
            // `lookup_open`: a directory with no `create` refuses an open that would make a file.
            if fixed_names(&parent.directory) {
                return Err(Errno::EACCES);
            }
            let umask = kernel.process_of(tid).umask;
            let node = create(
                kernel,
                cx,
                tid,
                &parent.directory,
                &parent.name,
                S_IFREG | (mode & 0o7777 & !umask),
                0,
                Content::Regular(Regular::default()),
            )?;
            (node, true)
        }
    };
    let file_type = node.borrow().file_type();
    // A trailing slash asks for a directory as `O_DIRECTORY` does (`LOOKUP_DIRECTORY` in `do_open`).
    if (flags & O_DIRECTORY != 0 || parent.trailing_slash) && file_type != S_IFDIR {
        return Err(Errno::ENOTDIR);
    }
    let mut terminal = None;
    if flags & O_PATH == 0 {
        if file_type == S_IFDIR && (accmode != O_RDONLY || flags & O_TRUNC != 0) {
            return Err(Errno::EISDIR);
        }
        // Devices on a `nodev` mount may not be opened.
        if matches!(file_type, S_IFCHR | S_IFBLK) && node.borrow().fs.no_devices() {
            return Err(Errno::EACCES);
        }
        // Truncating takes write access to the mount before `may_open`.
        if !created && flags & O_TRUNC != 0 && file_type == S_IFREG && node.borrow().fs.read_only()
        {
            return Err(Errno::EROFS);
        }
        if !created {
            // Linux's `ACC_MODE`: both access bits (an open for ioctl only) ask for reading and
            // writing.
            let mut mask = 0;
            if accmode != O_WRONLY {
                mask |= R_OK;
            }
            if accmode != O_RDONLY || flags & O_TRUNC != 0 {
                mask |= W_OK;
            }
            let inode = node.borrow();
            // Linux's `inode_permission`, with the process information tree's own rules.
            let credentials = credentials(kernel, tid);
            crate::fs::permission(&inode, credentials, mask, pid)?;
            if flags & O_NOATIME != 0 && !credentials.owns_or_capable(inode.uid) {
                return Err(Errno::EPERM);
            }
            if mask & W_OK != 0
                && inode.fs.read_only()
                && matches!(file_type, S_IFREG | S_IFDIR | S_IFLNK)
            {
                return Err(Errno::EROFS);
            }
        }
        match file_type {
            // Docker's device rules (`devcgroup_inode_permission`) allow no block device and only
            // its standard character devices; of those, a driver not built answers as Linux without
            // it (ENODEV, from the console's and the misc devices' lookups).
            S_IFBLK => return Err(Errno::EPERM),
            S_IFCHR => {
                let rdev = node.borrow().rdev;
                match Device::from_number(rdev) {
                    Some(device @ (Device::Tty | Device::Ptmx | Device::Pts)) => {
                        terminal = Some(device);
                    }
                    Some(_) => {}
                    None if crate::fs::dev::ALLOWED_NOT_BUILT
                        .contains(&crate::fs::decode_device(rdev)) =>
                    {
                        return Err(kernel.counters.refuse(
                            cx.host,
                            reason::DEVICE_NOT_BUILT,
                            rdev,
                            Errno::ENODEV,
                        ));
                    }
                    None => return Err(Errno::EPERM),
                }
            }
            S_IFIFO => super::pipe::named(&node),
            S_IFSOCK => return Err(Errno::ENXIO),
            _ => {}
        }
        // A secret memory file and the anonymous inode cannot be opened again, as through
        // `/proc/self/fd` (`no_open`); a process descriptor can, but not truncated.
        if node.borrow().is_secret() || matches!(node.borrow().content, Content::Anon) {
            return Err(Errno::ENXIO);
        }
        if flags & O_TRUNC != 0 && node.borrow().is_anonymous() {
            return Err(Errno::EOPNOTSUPP);
        }
        if let Content::Proc(entry) = node.borrow().content {
            proc::open_check(&kernel.processes, &entry)?;
        }
        // Linux's `do_dentry_open` refuses direct I/O after the open itself, before truncating.
        // A named pipe's and a terminal's are refused once they are open (below).
        if flags & O_DIRECT != 0
            && file_type != S_IFIFO
            && terminal.is_none()
            && !node.borrow().takes_direct_io()
        {
            return Err(Errno::EINVAL);
        }
        if flags & O_TRUNC != 0 && file_type == S_IFREG {
            let now = kernel.now(cx);
            let mut inode = node.borrow_mut();
            match &mut inode.content {
                Content::Regular(regular) => {
                    regular.may_resize(0)?;
                    regular.truncate(cx.cpu, 0);
                }
                Content::Mqueue(queue) => queue.size = 0,
                _ => {}
            }
            inode.mtime = now;
            inode.ctime = now;
            drop(inode);
            crate::shared::changed(kernel, cx, &node)?;
        }
    }
    let follows = flags & O_NOFOLLOW == 0;
    let full_path = if follows {
        kernel
            .with_walk(tid, |walk, _| walk.locate(&at, path))
            .map(|(_, full)| full)
            .unwrap_or_else(|_| path_in(kernel, tid, &parent.directory, &parent.name))
    } else if parent.name.is_empty() {
        path_in(kernel, tid, &node, b"")
    } else {
        path_in(kernel, tid, &parent.directory, &parent.name)
    };
    // A 64 bit kernel opens every file large, except one opened only for its path, whose flags
    // `build_open_how` has reduced to `O_PATH_FLAGS`.
    let large = if flags & O_PATH == 0 { O_LARGEFILE } else { 0 };
    let file = OpenFile::new(
        node.clone(),
        (flags | large) & !(O_CREAT | O_EXCL | O_NOCTTY | O_TRUNC | O_CLOEXEC),
        full_path,
        credentials(kernel, tid).serial,
    );
    file.borrow_mut().created = created;
    if let Some(device) = terminal {
        super::tty::open(kernel, cx, tid, &file, device, flags)?;
        return named_opened(kernel, cx, tid, file, flags);
    }
    if file_type == S_IFIFO && flags & O_PATH == 0 {
        super::pipe::open_named(kernel, tid, &file)?;
        return named_opened(kernel, cx, tid, file, flags);
    }
    install(kernel, tid, file, flags)
}

/// A named pipe or a terminal once open: direct I/O is refused only now (`do_dentry_open` checks it
/// after the open), closing the file again; otherwise it takes a descriptor.
fn named_opened(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: crate::file::FileRef,
    flags: u32,
) -> Result<u64, Errno> {
    if flags & O_DIRECT != 0 {
        kernel.process_of_mut(tid).files.release(file);
        kernel.files_released(cx);
        return Err(Errno::EINVAL);
    }
    install(kernel, tid, file, flags)
}

/// Puts an opened file in the lowest free descriptor.
fn install(
    kernel: &mut Kernel,
    tid: Tid,
    file: crate::file::FileRef,
    flags: u32,
) -> Result<u64, Errno> {
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let fd = kernel
        .process_of_mut(tid)
        .files
        .add(file, flags & O_CLOEXEC != 0, 0, limit)?;
    Ok(fd as u64)
}

/// A link target, as a path from where the link was found.
fn resolve_relative(link_path: &[u8], target: &[u8]) -> Vec<u8> {
    if target.starts_with(b"/") {
        return target.to_vec();
    }
    let directory = match link_path.iter().rposition(|byte| *byte == b'/') {
        Some(end) => &link_path[..=end],
        None => b"",
    };
    let mut out = directory.to_vec();
    out.extend_from_slice(target);
    out
}

/// Makes the bytes of a file of the process information tree.
pub(super) fn proc_contents(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    entry: &proc::Entry,
) -> Result<Vec<u8>, Errno> {
    use proc::ProcessFile;
    // Fields not tracked show 0, and the mount table leaves out Docker's masks of `/proc` and the
    // mounts not built.
    let untracked = match entry {
        proc::Entry::ProcessFile(_, ProcessFile::Stat) => Some(0),
        proc::Entry::ProcessFile(_, ProcessFile::Statm) => Some(1),
        proc::Entry::ProcessFile(_, ProcessFile::Status) => Some(2),
        proc::Entry::ProcessFile(_, ProcessFile::Mounts) => Some(3),
        proc::Entry::Global(index) if proc::GLOBAL[*index].0 == "meminfo" => Some(4),
        _ => None,
    };
    if let Some(detail) = untracked {
        kernel
            .counters
            .count(cx.host, reason::PROC_FIELDS_NOT_TRACKED, detail);
    }
    let uptime = cx.monotonic().saturating_sub(kernel.boot_monotonic);
    let mut fresh = [0u8; 16];
    cx.host.random(&mut fresh);
    let mounts = mount_table();
    let memory_in_use = kernel.memory_in_use();
    let cpu = &mut *cx.cpu;
    let mut read_memory = |process: &crate::process::Process, start: u64, length: u64| {
        let mut bytes = alloc::vec![0; length as usize];
        let readable = process
            .map
            .accessible(start, length, wcontainers_abi::Permissions::READ);
        bytes.truncate(readable as usize);
        if cpu.read(process.memory, start, &mut bytes).is_err() {
            bytes.clear();
        }
        bytes
    };
    let kernel_ref: &Kernel = kernel;
    kernel_ref.with_walk(tid, |walk, _| {
        let mut source = proc::Source {
            view: walk.proc,
            machine: &kernel_ref.machine,
            hostname: &kernel_ref.hostname,
            uptime_nanoseconds: uptime,
            last_pid: kernel_ref.last_id,
            boot_id: kernel_ref.boot_id,
            fresh_random: fresh,
            read_memory: &mut read_memory,
            mounts: &mounts,
            memory_in_use,
        };
        proc::contents(&mut source, entry)
    })
}

/// The mounts of a default Docker container, as `/proc/<pid>/mounts` lists them here.
fn mount_table() -> alloc::string::String {
    alloc::string::String::from(
        "tmpfs / tmpfs rw,relatime 0 0\n\
         proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\n\
         tmpfs /dev tmpfs rw,nosuid,size=65536k,mode=755 0 0\n\
         mqueue /dev/mqueue mqueue rw,nosuid,nodev,noexec,relatime 0 0\n\
         shm /dev/shm tmpfs rw,nosuid,nodev,noexec,relatime,size=65536k 0 0\n",
    )
}

pub(super) fn mkdirat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, mode, ..]: Args,
) -> Result<u64, Errno> {
    let parent = parent(kernel, cx, tid, dirfd, path)?;
    if parent.name.is_empty() {
        return Err(Errno::EEXIST);
    }
    check_absent(kernel, tid, &parent)?;
    may_change(kernel, tid, &parent.directory)?;
    changes_names(&parent.directory)?;
    let umask = kernel.process_of(tid).umask;
    let mode = S_IFDIR | (mode as u32 & 0o1777 & !umask);
    create(
        kernel,
        cx,
        tid,
        &parent.directory,
        &parent.name,
        mode,
        0,
        Content::Directory(Directory::new()),
    )?;
    Ok(0)
}

pub(super) fn mknodat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, mode, device, ..]: Args,
) -> Result<u64, Errno> {
    let mode = mode as u32;
    let file_type = mode & S_IFMT;
    let (file_type, content) = match file_type {
        0 | S_IFREG => (S_IFREG, Content::Regular(Regular::default())),
        S_IFCHR | S_IFBLK | S_IFIFO | S_IFSOCK => (file_type, Content::Special),
        S_IFDIR => return Err(Errno::EPERM),
        _ => return Err(Errno::EINVAL),
    };
    let parent = parent(kernel, cx, tid, dirfd, path)?;
    if parent.name.is_empty() {
        return Err(Errno::EEXIST);
    }
    check_absent(kernel, tid, &parent)?;
    if parent.trailing_slash {
        return Err(Errno::ENOENT);
    }
    may_change(kernel, tid, &parent.directory)?;
    // A plain file is made as `vfs_create` makes it: EACCES where the directory has no `create`.
    if file_type == S_IFREG && fixed_names(&parent.directory) {
        return Err(Errno::EACCES);
    }
    if !(file_type == S_IFREG && only_queues(&parent.directory)) {
        changes_names(&parent.directory)?;
    }
    // Devices need `CAP_MKNOD`, except a whiteout (a character device 0:0).
    let whiteout = file_type == S_IFCHR && device == 0;
    if matches!(file_type, S_IFCHR | S_IFBLK)
        && !whiteout
        && !credentials(kernel, tid).has(CAP_MKNOD)
    {
        return Err(Errno::EPERM);
    }
    let umask = kernel.process_of(tid).umask;
    let rdev = if matches!(file_type, S_IFCHR | S_IFBLK) {
        let device = device as u32;
        crate::fs::encode_device(
            device >> 8 & 0xfff,
            (device & 0xff) | (device >> 12 & 0xfff00),
        )
    } else {
        0
    };
    create(
        kernel,
        cx,
        tid,
        &parent.directory,
        &parent.name,
        file_type | (mode & 0o7777 & !umask),
        rdev,
        content,
    )?;
    Ok(0)
}

pub(super) fn symlinkat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [target, dirfd, path, ..]: Args,
) -> Result<u64, Errno> {
    let target = guest(kernel, cx, tid).read_path(target)?;
    let parent = parent(kernel, cx, tid, dirfd, path)?;
    if parent.name.is_empty() {
        return Err(Errno::EEXIST);
    }
    check_absent(kernel, tid, &parent)?;
    if parent.trailing_slash {
        return Err(Errno::ENOENT);
    }
    may_change(kernel, tid, &parent.directory)?;
    changes_names(&parent.directory)?;
    create(
        kernel,
        cx,
        tid,
        &parent.directory,
        &parent.name,
        S_IFLNK | 0o777,
        0,
        Content::Symlink(target),
    )?;
    Ok(0)
}

pub(super) fn linkat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [old_dirfd, old_path, new_dirfd, new_path, flags, _]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(AT_SYMLINK_FOLLOW | AT_EMPTY_PATH) != 0 {
        return Err(Errno::EINVAL);
    }
    // With `AT_EMPTY_PATH`, a walk from a descriptor needs the credentials it was opened with or
    // `CAP_DAC_READ_SEARCH`, else the name is not found (Linux's `LOOKUP_LINKAT_EMPTY` in `path_init`).
    if flags & AT_EMPTY_PATH != 0 && old_dirfd as i32 != AT_FDCWD {
        let path = guest(kernel, cx, tid).read_path_maybe_empty(old_path)?;
        if !path.starts_with(b"/") {
            let file = kernel.process_of(tid).files.file(old_dirfd as i32)?;
            let credentials = credentials(kernel, tid);
            if file.borrow().opener != credentials.serial
                && !credentials.has(crate::credentials::CAP_DAC_READ_SEARCH)
            {
                return Err(Errno::ENOENT);
            }
        }
    }
    let node = lookup(
        kernel,
        cx,
        tid,
        old_dirfd,
        old_path,
        flags,
        flags & AT_SYMLINK_FOLLOW != 0,
    )?;
    let parent = parent(kernel, cx, tid, new_dirfd, new_path)?;
    if parent.name.is_empty() {
        return Err(Errno::EEXIST);
    }
    // In the order of Linux's `do_linkat` and `vfs_link`.
    check_absent(kernel, tid, &parent)?;
    if parent.trailing_slash {
        return Err(Errno::ENOENT);
    }
    if node.borrow().fs != parent.directory.borrow().fs {
        return Err(Errno::EXDEV);
    }
    if !may_hard_link(&node, credentials(kernel, tid)) {
        return Err(Errno::EPERM);
    }
    may_change(kernel, tid, &parent.directory)?;
    changes_names(&parent.directory)?;
    if node.borrow().is_directory() {
        return Err(Errno::EPERM);
    }
    if node.borrow().nlink == 0 {
        return Err(Errno::ENOENT);
    }
    // tmpfs counts each link as an inode (`shmem_link`).
    if let Some(space) = &node.borrow().fs.space {
        space.take_ispace(crate::fs::INODE_SPACE)?;
    }
    let now = kernel.now(cx);
    {
        let mut inode = node.borrow_mut();
        inode.nlink += 1;
        inode.ctime = now;
    }
    parent
        .directory
        .borrow_mut()
        .directory_mut()
        .expect("directory")
        .insert(&parent.name, node);
    touch_directory(&parent.directory, now);
    Ok(0)
}

pub(super) fn unlinkat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !AT_REMOVEDIR != 0 {
        return Err(Errno::EINVAL);
    }
    let raw = guest(kernel, cx, tid).read_path(path)?;
    let parent = parent(kernel, cx, tid, dirfd, path)?;
    let removing_directory = flags & AT_REMOVEDIR != 0;
    if parent.name.is_empty() {
        let last = raw
            .split(|byte| *byte == b'/')
            .rfind(|part| !part.is_empty());
        return Err(match last {
            Some(b".") if removing_directory => Errno::EINVAL,
            Some(b"..") if removing_directory => Errno::ENOTEMPTY,
            _ if removing_directory => Errno::EBUSY,
            _ => Errno::EISDIR,
        });
    }
    // `mnt_want_write` comes before the name is looked up.
    if parent.directory.borrow().fs.read_only() {
        return Err(Errno::EROFS);
    }
    // A mount point is found as the root mounted on it.
    let node = kernel.with_walk(tid, |walk, _| walk.step(&parent.directory, &parent.name))?;
    // In Linux's order: a trailing slash on a file, then `may_delete`, then the directory's own checks.
    let is_directory = node.borrow().is_directory();
    if !removing_directory && parent.trailing_slash {
        return Err(if is_directory {
            Errno::EISDIR
        } else {
            Errno::ENOTDIR
        });
    }
    if !removing_directory {
        remove_from(kernel, cx, tid, &parent.directory, &parent.name, &node)?;
        return Ok(0);
    }
    may_change(kernel, tid, &parent.directory)?;
    if !sticky_allows(&parent.directory, &node, credentials(kernel, tid)) {
        return Err(Errno::EPERM);
    }
    {
        if !is_directory {
            return Err(Errno::ENOTDIR);
        }
        changes_names(&parent.directory)?;
        if Rc::ptr_eq(&node, &kernel.process_of(tid).root) || node.borrow().place.mount_root {
            return Err(Errno::EBUSY);
        }
        if node
            .borrow()
            .directory()
            .is_some_and(|directory| !directory.is_empty())
        {
            return Err(Errno::ENOTEMPTY);
        }
    }
    let now = kernel.now(cx);
    parent
        .directory
        .borrow_mut()
        .directory_mut()
        .expect("directory")
        .remove(&parent.name);
    touch_directory(&parent.directory, now);
    node.borrow_mut().nlink = 0;
    parent.directory.borrow_mut().nlink -= 1;
    node.borrow_mut().ctime = now;
    Ok(0)
}

/// Removes a name of a file that is not a directory, after `may_delete`'s checks (`vfs_unlink`).
pub(super) fn remove_from(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    directory: &Node,
    name: &[u8],
    node: &Node,
) -> Result<(), Errno> {
    may_change(kernel, tid, directory)?;
    if !sticky_allows(directory, node, credentials(kernel, tid)) {
        return Err(Errno::EPERM);
    }
    if node.borrow().is_directory() {
        return Err(Errno::EISDIR);
    }
    if fixed_names(directory) {
        return Err(Errno::EPERM);
    }
    let now = kernel.now(cx);
    directory
        .borrow_mut()
        .directory_mut()
        .expect("directory")
        .remove(name);
    touch_directory(directory, now);
    let mut inode = node.borrow_mut();
    give_back_link(&inode);
    inode.nlink -= 1;
    inode.ctime = now;
    Ok(())
}

/// A name of a file with others gone gives back the inode space the link took (`shmem_unlink`).
fn give_back_link(inode: &crate::fs::Inode) {
    if inode.nlink > 1
        && let Some(space) = &inode.fs.space
    {
        space.give_ispace(crate::fs::INODE_SPACE);
    }
}

pub(super) fn renameat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [old_dirfd, old_path, new_dirfd, new_path, ..]: Args,
) -> Result<u64, Errno> {
    rename(kernel, cx, tid, old_dirfd, old_path, new_dirfd, new_path, 0)
}

pub(super) fn renameat2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [old_dirfd, old_path, new_dirfd, new_path, flags, _]: Args,
) -> Result<u64, Errno> {
    rename(
        kernel,
        cx,
        tid,
        old_dirfd,
        old_path,
        new_dirfd,
        new_path,
        flags as u32,
    )
}

#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn rename(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    old_dirfd: u64,
    old_path: u64,
    new_dirfd: u64,
    new_path: u64,
    flags: u32,
) -> Result<u64, Errno> {
    if flags & !(RENAME_NOREPLACE | RENAME_EXCHANGE | RENAME_WHITEOUT) != 0 {
        return Err(Errno::EINVAL);
    }
    if flags & RENAME_EXCHANGE != 0 && flags & (RENAME_NOREPLACE | RENAME_WHITEOUT) != 0 {
        return Err(Errno::EINVAL);
    }
    // In the order of Linux's `do_renameat2` and `vfs_rename`.
    let from = parent(kernel, cx, tid, old_dirfd, old_path)?;
    let to = parent(kernel, cx, tid, new_dirfd, new_path)?;
    if from.base.borrow().fs != to.base.borrow().fs {
        return Err(Errno::EXDEV);
    }
    if from.name.is_empty() {
        return Err(Errno::EBUSY);
    }
    if to.name.is_empty() {
        return Err(if flags & RENAME_NOREPLACE != 0 {
            Errno::EEXIST
        } else {
            Errno::EBUSY
        });
    }
    // `mnt_want_write` comes before the names are looked up.
    if from.directory.borrow().fs.read_only() {
        return Err(Errno::EROFS);
    }
    let source = kernel.with_walk(tid, |walk, _| walk.step(&from.directory, &from.name))?;
    not_removed(&to.directory)?;
    let target = match kernel.with_walk(tid, |walk, _| walk.step(&to.directory, &to.name)) {
        Ok(node) => Some(node),
        Err(Errno::ENOENT) if !matches!(to.directory.borrow().content, Content::Proc(_)) => None,
        Err(error) => return Err(error),
    };
    if flags & RENAME_EXCHANGE != 0 && target.is_none() {
        return Err(Errno::ENOENT);
    }
    if flags & RENAME_NOREPLACE != 0 && target.is_some() {
        return Err(Errno::EEXIST);
    }
    let source_is_directory = source.borrow().is_directory();
    let target_is_directory = target
        .as_ref()
        .is_some_and(|target| target.borrow().is_directory());
    if !source_is_directory
        && (from.trailing_slash || (flags & RENAME_EXCHANGE == 0 && to.trailing_slash))
    {
        return Err(Errno::ENOTDIR);
    }
    if flags & RENAME_EXCHANGE != 0 && !target_is_directory && to.trailing_slash {
        return Err(Errno::ENOTDIR);
    }
    // A directory may not move below itself, nor replace one of its own ancestors. The process
    // information tree's inodes are made at each lookup, so they are compared by entry.
    let below = |moving: &Node, destination: &Node| {
        let mut at = Some(destination.clone());
        while let Some(node) = at {
            if crate::fs::same_inode(&node, moving) {
                return true;
            }
            at = node
                .borrow()
                .place
                .parent
                .as_ref()
                .and_then(|parent| parent.upgrade());
        }
        false
    };
    if below(&source, &to.directory) {
        return Err(Errno::EINVAL);
    }
    if let Some(target) = &target
        && below(target, &from.directory)
    {
        return Err(if flags & RENAME_EXCHANGE != 0 {
            Errno::EINVAL
        } else {
            Errno::ENOTEMPTY
        });
    }
    if let Some(target) = &target
        && crate::fs::same_inode(&source, target)
    {
        return Ok(0);
    }
    let credentials = credentials(kernel, tid).clone();
    // `may_delete` of the source, then of the target or `may_create` where there is none.
    may_change(kernel, tid, &from.directory)?;
    if !sticky_allows(&from.directory, &source, &credentials) {
        return Err(Errno::EPERM);
    }
    may_change(kernel, tid, &to.directory)?;
    if let Some(target) = &target {
        if !sticky_allows(&to.directory, target, &credentials) {
            return Err(Errno::EPERM);
        }
        if flags & RENAME_EXCHANGE == 0 {
            if source_is_directory && !target_is_directory {
                return Err(Errno::ENOTDIR);
            }
            if !source_is_directory && target_is_directory {
                return Err(Errno::EISDIR);
            }
        }
    }
    changes_names(&from.directory)?;
    if source.borrow().place.mount_root
        || target
            .as_ref()
            .is_some_and(|target| target.borrow().place.mount_root)
    {
        return Err(Errno::EBUSY);
    }
    // A directory changing parent has its `..` rewritten, which needs write permission on it.
    let moves_parent = !Rc::ptr_eq(&from.directory, &to.directory);
    if moves_parent && source_is_directory && !source.borrow().permits(&credentials, W_OK, true) {
        return Err(Errno::EACCES);
    }
    if moves_parent
        && flags & RENAME_EXCHANGE != 0
        && target_is_directory
        && !target
            .as_ref()
            .expect("checked")
            .borrow()
            .permits(&credentials, W_OK, true)
    {
        return Err(Errno::EACCES);
    }
    if flags & RENAME_EXCHANGE == 0
        && target_is_directory
        && target
            .as_ref()
            .expect("checked")
            .borrow()
            .directory()
            .is_some_and(|d| !d.is_empty())
    {
        return Err(Errno::ENOTEMPTY);
    }
    let now = kernel.now(cx);
    let set_place = |node: &Node, directory: &Node, name: &[u8]| {
        let mut inode = node.borrow_mut();
        if inode.is_directory() {
            inode.place.parent = Some(Rc::downgrade(directory));
            inode.place.name = name.to_vec();
        }
        inode.ctime = now;
    };
    let entries = |directory: &Node| {
        let directory = directory.clone();
        move |f: &mut dyn FnMut(&mut Directory)| {
            f(directory.borrow_mut().directory_mut().expect("directory"))
        }
    };
    if flags & RENAME_EXCHANGE != 0 {
        let target = target.expect("checked");
        entries(&from.directory)(&mut |d| d.replace(&from.name, target.clone()));
        entries(&to.directory)(&mut |d| d.replace(&to.name, source.clone()));
        set_place(&target, &from.directory, &from.name);
        set_place(&source, &to.directory, &to.name);
        let source_dir = source_is_directory;
        let target_dir = target.borrow().is_directory();
        if source_dir != target_dir && !Rc::ptr_eq(&from.directory, &to.directory) {
            let (gains, loses) = if source_dir {
                (&to.directory, &from.directory)
            } else {
                (&from.directory, &to.directory)
            };
            gains.borrow_mut().nlink += 1;
            loses.borrow_mut().nlink -= 1;
        }
    } else {
        // In tmpfs's order: the new name's position first (`simple_offset_add`), then the whiteout
        // left at the old name, then the move to the head of the new directory's list.
        let position = to
            .directory
            .borrow_mut()
            .directory_mut()
            .expect("directory")
            .position_for(&to.name);
        if flags & RENAME_WHITEOUT != 0 {
            // As overlay file systems use: a character device 0:0 in place of the old name.
            create(
                kernel,
                cx,
                tid,
                &from.directory,
                &from.name,
                S_IFCHR,
                0,
                Content::Special,
            )?;
        } else {
            entries(&from.directory)(&mut |d| {
                d.remove(&from.name);
            });
        }
        if let Some(target) = &target {
            let mut inode = target.borrow_mut();
            if inode.is_directory() {
                inode.nlink = 0;
                drop(inode);
                to.directory.borrow_mut().nlink -= 1;
            } else {
                give_back_link(&inode);
                inode.nlink -= 1;
                inode.ctime = now;
            }
        }
        entries(&to.directory)(&mut |d| {
            d.put(&to.name, source.clone(), position);
        });
        set_place(&source, &to.directory, &to.name);
        if source_is_directory && !Rc::ptr_eq(&from.directory, &to.directory) {
            from.directory.borrow_mut().nlink -= 1;
            to.directory.borrow_mut().nlink += 1;
        }
    }
    touch_directory(&from.directory, now);
    touch_directory(&to.directory, now);
    Ok(0)
}

pub(super) fn readlinkat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, buffer, size, ..]: Args,
) -> Result<u64, Errno> {
    let size = size as i32;
    if size <= 0 {
        return Err(Errno::EINVAL);
    }
    let empty = guest(kernel, cx, tid)
        .read_path_maybe_empty(path)?
        .is_empty();
    let node = lookup(kernel, cx, tid, dirfd, path, AT_EMPTY_PATH, false)?;
    if matches!(node.borrow().content, Content::Symlink(_)) {
        super::io::accessed(kernel, cx, &node, 0);
    }
    // Where the kernel's copy of the target lies decides what a buffer that faults part way gets:
    // a tmpfs link's own allocation, or the end of a page `d_path` filled before its last byte.
    let (target, source) = {
        let inode = node.borrow();
        match &inode.content {
            Content::Symlink(target) => (target.clone(), 0),
            Content::Proc(entry) if inode.file_type() == S_IFLNK => {
                let entry = *entry;
                drop(inode);
                let target =
                    kernel.with_walk(tid, |walk, _| proc::link_target(walk.proc, walk, &entry))?;
                let source = wcontainers_abi::PAGE_SIZE.wrapping_sub(1 + target.len() as u64);
                (target, source)
            }
            // An empty path names `dirfd` itself, and when that is no link Linux says it is not there.
            _ => return Err(if empty { Errno::ENOENT } else { Errno::EINVAL }),
        }
    };
    let length = target.len().min(size as usize);
    guest(kernel, cx, tid).write_from(buffer, &target[..length], source)?;
    Ok(length as u64)
}

/// A mapped tmpfs file's blocks include the pages its mappings' faults brought in, which are not known
/// here: counted when reported.
fn count_unknown_blocks(kernel: &mut Kernel, cx: &mut Context<'_>, node: &Node) {
    let inode = node.borrow();
    if !inode.is_secret()
        && matches!(&inode.content, Content::Regular(regular) if regular.is_mapped())
    {
        kernel
            .counters
            .count(cx.host, reason::RESIDENCY_NOT_TRACKED, 0);
    }
}

/// Fills Linux's `stat` for an inode.
pub(super) fn stat_of(kernel: &Kernel, node: &Node) -> stat {
    let inode = node.borrow();
    // A process's directory reports its owner as it is now (`pid_getattr`); other inodes as made.
    let (uid, gid) = match inode.content {
        Content::Proc(proc::Entry::Process(task)) => {
            proc::dump_owner(&kernel.processes, task.pid, inode.mode)
        }
        _ => (inode.uid, inode.gid),
    };
    stat {
        st_dev: inode.fs.device,
        st_ino: inode.ino,
        st_mode: inode.shown_mode(),
        st_nlink: match inode.content {
            Content::Proc(proc::Entry::Root) => proc::root_links(kernel.processes.len()),
            _ => inode.nlink,
        },
        st_uid: uid,
        st_gid: gid,
        st_rdev: inode.rdev,
        __pad1: 0,
        st_size: match inode.content {
            // A descriptor directory's size is how many descriptors are open, as on Linux.
            Content::Proc(proc::Entry::Fds(task)) => kernel
                .processes
                .get(&task.pid)
                .map_or(0, |process| process.files.iter().count())
                as i64,
            _ => inode.size() as i64,
        },
        st_blksize: inode.fs.kind.block_size() as i32,
        __pad2: 0,
        st_blocks: inode.blocks() as i64,
        st_atime: inode.atime.seconds,
        st_atime_nsec: inode.atime.nanoseconds.into(),
        st_mtime: inode.mtime.seconds,
        st_mtime_nsec: inode.mtime.nanoseconds.into(),
        st_ctime: inode.ctime.seconds,
        st_ctime_nsec: inode.ctime.nanoseconds.into(),
        __unused4: 0,
        __unused5: 0,
    }
}

/// `getname_maybe_null`: with AT_EMPTY_PATH a null or empty path names the descriptor (or the working
/// directory). A path whose first byte cannot be read is not empty, and fails at its lookup.
fn names_descriptor(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    flags: u32,
) -> bool {
    flags & AT_EMPTY_PATH != 0
        && (address == 0
            || guest(kernel, cx, tid)
                .read(address, 1)
                .is_ok_and(|byte| byte == [0]))
}

/// The file an empty path names: the descriptor's (an `O_PATH` one too), or the working directory.
fn empty_path_node(kernel: &Kernel, tid: Tid, dirfd: u64) -> Result<Node, Errno> {
    let process = kernel.process_of(tid);
    if dirfd as i32 == AT_FDCWD {
        return Ok(process.cwd.clone());
    }
    Ok(process.files.file(dirfd as i32)?.borrow().node.clone())
}

pub(super) fn newfstatat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, buffer, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    let empty = names_descriptor(kernel, cx, tid, path, flags);
    // `vfs_fstatat`: a descriptor named so is a plain `fstat`, whatever the other flags are.
    let node = if empty && dirfd as i32 >= 0 {
        empty_path_node(kernel, tid, dirfd)?
    } else {
        if flags & !(AT_SYMLINK_NOFOLLOW | AT_EMPTY_PATH | AT_NO_AUTOMOUNT) != 0 {
            return Err(Errno::EINVAL);
        }
        if empty {
            empty_path_node(kernel, tid, dirfd)?
        } else {
            lookup(
                kernel,
                cx,
                tid,
                dirfd,
                path,
                flags,
                flags & AT_SYMLINK_NOFOLLOW == 0,
            )?
        }
    };
    count_unknown_blocks(kernel, cx, &node);
    let value = stat_of(kernel, &node);
    guest(kernel, cx, tid).write_value(buffer, &value)?;
    Ok(0)
}

pub(super) fn fstat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, ..]: Args,
) -> Result<u64, Errno> {
    let node = kernel
        .process_of(tid)
        .files
        .file(fd as i32)?
        .borrow()
        .node
        .clone();
    count_unknown_blocks(kernel, cx, &node);
    let value = stat_of(kernel, &node);
    guest(kernel, cx, tid).write_value(buffer, &value)?;
    Ok(0)
}

pub(super) fn statx(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, flags, mask, buffer, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    let empty = names_descriptor(kernel, cx, tid, path, flags);
    // `do_statx_fd` for a descriptor named so: the mask and the sync type are checked, not the other
    // flags.
    let by_descriptor = empty && dirfd as i32 >= 0;
    if mask as u32 & STATX__RESERVED != 0
        || flags & AT_STATX_SYNC_TYPE == AT_STATX_SYNC_TYPE
        || !by_descriptor
            && flags & !(AT_SYMLINK_NOFOLLOW | AT_EMPTY_PATH | AT_NO_AUTOMOUNT | AT_STATX_SYNC_TYPE)
                != 0
    {
        return Err(Errno::EINVAL);
    }
    let node = if empty {
        empty_path_node(kernel, tid, dirfd)?
    } else {
        lookup(
            kernel,
            cx,
            tid,
            dirfd,
            path,
            flags,
            flags & AT_SYMLINK_NOFOLLOW == 0,
        )?
    };
    count_unknown_blocks(kernel, cx, &node);
    let plain = stat_of(kernel, &node);
    let inode = node.borrow();
    let time = |time: Time| statx_timestamp {
        tv_sec: time.seconds,
        tv_nsec: time.nanoseconds,
        __reserved: 0,
    };
    let (major, minor) = crate::fs::decode_device(inode.fs.device);
    let (rdev_major, rdev_minor) = crate::fs::decode_device(inode.rdev);
    // tmpfs keeps fine-grained times (`fill_mg_cmtime`), so it reports the change and modification
    // times only when asked for one of them, and its birth time only when asked (`shmem_getattr`);
    // every other file system here reports the basic set (`generic_fillattr`).
    let requested = mask as u32;
    let basic = inode.fs.kind != crate::fs::Kind::Tmpfs;
    let mut reported = if basic {
        STATX_BASIC_STATS
    } else {
        let mut reported = STATX_BASIC_STATS & !(STATX_MTIME | STATX_CTIME);
        if requested & (STATX_MTIME | STATX_CTIME) != 0 {
            reported |= STATX_MTIME | STATX_CTIME;
        }
        reported | (requested & STATX_BTIME)
    };
    // `vfs_statx_path`: the unique mount number if asked for, else the plain one.
    let unique = requested & STATX_MNT_ID_UNIQUE != 0;
    reported |= if unique {
        STATX_MNT_ID_UNIQUE
    } else {
        STATX_MNT_ID
    };
    let mut out: statx = zerocopy::FromZeros::new_zeroed();
    out.stx_mask = reported;
    out.stx_blksize = inode.fs.kind.block_size();
    out.stx_nlink = plain.st_nlink;
    out.stx_uid = plain.st_uid;
    out.stx_gid = plain.st_gid;
    out.stx_mode = inode.shown_mode() as u16;
    out.stx_ino = inode.ino;
    out.stx_size = plain.st_size as u64;
    out.stx_blocks = plain.st_blocks as u64;
    let common = STATX_ATTR_AUTOMOUNT | STATX_ATTR_MOUNT_ROOT | STATX_ATTR_DAX;
    out.stx_attributes_mask = u64::from(if basic {
        common
    } else {
        common | STATX_ATTR_IMMUTABLE | STATX_ATTR_APPEND | STATX_ATTR_NODUMP
    });
    if inode.place.mount_root {
        out.stx_attributes = u64::from(STATX_ATTR_MOUNT_ROOT);
    }
    // tmpfs shows its inode flags that have an attribute (`shmem_getattr`).
    for (flag, attribute) in [
        (FS_APPEND_FL, STATX_ATTR_APPEND),
        (FS_IMMUTABLE_FL, STATX_ATTR_IMMUTABLE),
        (FS_NODUMP_FL, STATX_ATTR_NODUMP),
    ] {
        if inode.fsflags & flag != 0 {
            out.stx_attributes |= u64::from(attribute);
        }
    }
    out.stx_atime = time(inode.atime);
    out.stx_btime = if reported & STATX_BTIME != 0 {
        time(inode.btime)
    } else {
        time(Time::default())
    };
    out.stx_ctime = time(inode.ctime);
    out.stx_mtime = time(inode.mtime);
    out.stx_rdev_major = rdev_major;
    out.stx_rdev_minor = rdev_minor;
    out.stx_dev_major = major;
    out.stx_dev_minor = minor;
    out.stx_mnt_id = if unique {
        super::handle::unique_mount_id(&inode)
    } else {
        super::handle::mount_id(&inode)
    };
    drop(inode);
    guest(kernel, cx, tid).write_value(buffer, &out)?;
    Ok(0)
}

pub(super) fn faccessat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, mode, ..]: Args,
) -> Result<u64, Errno> {
    access(kernel, cx, tid, dirfd, path, mode as u32, 0)
}

pub(super) fn faccessat2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, mode, flags, ..]: Args,
) -> Result<u64, Errno> {
    access(kernel, cx, tid, dirfd, path, mode as u32, flags as u32)
}

fn access(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    dirfd: u64,
    path: u64,
    mode: u32,
    flags: u32,
) -> Result<u64, Errno> {
    if mode & !(R_OK | W_OK | X_OK) != 0 {
        return Err(Errno::EINVAL);
    }
    if flags & !(AT_EACCESS | AT_SYMLINK_NOFOLLOW | AT_EMPTY_PATH) != 0 {
        return Err(Errno::EINVAL);
    }
    // Checked with the real ids unless `AT_EACCESS`, and the powers only root's real id has.
    let mut checking = credentials(kernel, tid).clone();
    if flags & AT_EACCESS == 0 {
        checking.fsuid = checking.uid;
        checking.fsgid = checking.gid;
        checking.effective = if checking.uid == 0 {
            checking.permitted
        } else {
            0
        };
    }
    // The whole walk runs with these credentials, as Linux's `access_override_creds` replaces the
    // caller's for the call.
    let own = core::mem::replace(
        &mut kernel.process_of_mut(tid).credentials,
        checking.clone(),
    );
    let found = lookup(
        kernel,
        cx,
        tid,
        dirfd,
        path,
        flags,
        flags & AT_SYMLINK_NOFOLLOW == 0,
    );
    kernel.process_of_mut(tid).credentials = own;
    let node = found?;
    if mode == F_OK {
        return Ok(0);
    }
    let inode = node.borrow();
    // A regular file on a `noexec` mount may not run, whatever its mode (`do_faccessat`).
    if mode & X_OK != 0 && inode.file_type() == S_IFREG && inode.fs.no_execute() {
        return Err(Errno::EACCES);
    }
    crate::fs::permission(&inode, &checking, mode, kernel.threads[&tid].process)?;
    if mode & W_OK != 0
        && inode.fs.read_only()
        && matches!(inode.file_type(), S_IFREG | S_IFDIR | S_IFLNK)
    {
        return Err(Errno::EROFS);
    }
    Ok(0)
}

/// Changes an inode's permission bits as `chmod` does.
fn change_mode(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    node: &Node,
    mode: u32,
) -> Result<u64, Errno> {
    let credentials = credentials(kernel, tid).clone();
    let now = kernel.now(cx);
    // A read-only mount refuses before anything else, as `mnt_want_write` comes first.
    if node.borrow().fs.read_only() {
        return Err(Errno::EROFS);
    }
    if node.borrow().is_anonymous() {
        return Err(Errno::EOPNOTSUPP);
    }
    // A process's entries refuse mode changes, as Linux's `proc_setattr` does. The others take them.
    if let Content::Proc(entry) = node.borrow().content
        && entry.of_process().is_some()
    {
        return Err(Errno::EPERM);
    }
    let mut inode = node.borrow_mut();
    if !credentials.owns_or_capable(inode.uid) {
        return Err(Errno::EPERM);
    }
    let mut mode = mode & 0o7777;
    if mode & S_ISGID != 0 && !credentials.in_group(inode.gid, true) && !credentials.has(CAP_FSETID)
    {
        mode &= !S_ISGID;
    }
    if let Content::Regular(regular) = &inode.content
        && regular.seals & F_SEAL_EXEC != 0
        && (inode.mode ^ mode) & 0o111 != 0
    {
        return Err(Errno::EPERM);
    }
    inode.mode = (inode.mode & S_IFMT) | mode;
    inode.ctime = now;
    drop(inode);
    keep_proc(kernel, node);
    Ok(0)
}

/// Keeps the attributes of a `/proc` entry Linux keeps them for, for its next lookup.
fn keep_proc(kernel: &mut Kernel, node: &Node) {
    let inode = node.borrow();
    if let Content::Proc(entry) = inode.content
        && entry.keeps_attributes()
    {
        kernel
            .files
            .proc_kept
            .insert(inode.ino, proc::Kept::of(&inode));
    }
}

pub(super) fn fchmod(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, mode, ..]: Args,
) -> Result<u64, Errno> {
    let file = kernel.process_of(tid).files.file(fd as i32)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let node = file.borrow().node.clone();
    change_mode(kernel, cx, tid, &node, mode as u32)
}

pub(super) fn fchmodat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, mode, ..]: Args,
) -> Result<u64, Errno> {
    let node = lookup(kernel, cx, tid, dirfd, path, 0, true)?;
    change_mode(kernel, cx, tid, &node, mode as u32)
}

pub(super) fn fchmodat2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, mode, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(AT_SYMLINK_NOFOLLOW | AT_EMPTY_PATH) != 0 {
        return Err(Errno::EINVAL);
    }
    let node = lookup(
        kernel,
        cx,
        tid,
        dirfd,
        path,
        flags,
        flags & AT_SYMLINK_NOFOLLOW == 0,
    )?;
    if node.borrow().file_type() == S_IFLNK {
        return Err(Errno::EOPNOTSUPP);
    }
    change_mode(kernel, cx, tid, &node, mode as u32)
}

/// Changes an inode's owner and group as `chown` does; `u32::MAX` leaves one as it is.
fn change_owner(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    node: &Node,
    uid: u32,
    gid: u32,
) -> Result<u64, Errno> {
    let credentials = credentials(kernel, tid).clone();
    let now = kernel.now(cx);
    let mut inode = node.borrow_mut();
    if inode.fs.read_only() {
        return Err(Errno::EROFS);
    }
    if inode.is_anonymous() {
        return Err(Errno::EOPNOTSUPP);
    }
    let changes_uid = uid != u32::MAX && uid != inode.uid;
    let changes_gid = gid != u32::MAX && gid != inode.gid;
    if changes_uid && !credentials.has(CAP_CHOWN) {
        return Err(Errno::EPERM);
    }
    if changes_gid
        && !credentials.has(CAP_CHOWN)
        && (credentials.fsuid != inode.uid || !credentials.in_group(gid, true))
    {
        return Err(Errno::EPERM);
    }
    if (uid != u32::MAX || gid != u32::MAX)
        && !changes_uid
        && !changes_gid
        && credentials.fsuid != inode.uid
        && !credentials.has(CAP_CHOWN)
    {
        return Err(Errno::EPERM);
    }
    if let Content::Proc(entry) = inode.content {
        // A process's directory is immutable: it refuses a change of owner or group (measured on the
        // reference), though not one that names neither. Its other entries take the change and forget
        // it at once: Linux sets their owners from the process again each time they are looked up.
        // Other entries keep it.
        if matches!(entry, crate::fs::proc::Entry::Process(_))
            && (uid != u32::MAX || gid != u32::MAX)
        {
            return Err(Errno::EPERM);
        }
        if entry.of_process().is_some() {
            return Ok(0);
        }
    }
    if uid != u32::MAX {
        inode.uid = uid;
    }
    if gid != u32::MAX {
        inode.gid = gid;
    }
    inode.drop_privileges(true);
    inode.ctime = now;
    drop(inode);
    keep_proc(kernel, node);
    Ok(0)
}

pub(super) fn fchown(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, uid, gid, ..]: Args,
) -> Result<u64, Errno> {
    let file = kernel.process_of(tid).files.file(fd as i32)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let node = file.borrow().node.clone();
    change_owner(kernel, cx, tid, &node, uid as u32, gid as u32)
}

pub(super) fn fchownat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, uid, gid, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(AT_SYMLINK_NOFOLLOW | AT_EMPTY_PATH) != 0 {
        return Err(Errno::EINVAL);
    }
    let node = lookup(
        kernel,
        cx,
        tid,
        dirfd,
        path,
        flags,
        flags & AT_SYMLINK_NOFOLLOW == 0,
    )?;
    change_owner(kernel, cx, tid, &node, uid as u32, gid as u32)
}

pub(super) fn utimensat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, times, flags, ..]: Args,
) -> Result<u64, Errno> {
    // In Linux's order: read the times, skip if both are left alone, find the file, check the times.
    let flags = flags as u32;
    let wanted: Option<[__kernel_timespec; 2]> = if times == 0 {
        None
    } else {
        let mut guest = guest(kernel, cx, tid);
        Some([guest.read_value(times)?, guest.read_value(times + 16)?])
    };
    let omitted = |time: &__kernel_timespec| time.tv_nsec == i64::from(UTIME_OMIT);
    if wanted.is_some_and(|wanted| wanted.iter().all(omitted)) {
        return Ok(0);
    }
    let node = if path == 0 && dirfd as i32 != AT_FDCWD {
        if flags != 0 {
            return Err(Errno::EINVAL);
        }
        // Linux's `do_utimes_fd` takes no `O_PATH` descriptor.
        let file = kernel.process_of(tid).files.file(dirfd as i32)?;
        let file = file.borrow();
        if file.is_path() {
            return Err(Errno::EBADF);
        }
        file.node.clone()
    } else {
        if flags & !(AT_SYMLINK_NOFOLLOW | AT_EMPTY_PATH) != 0 {
            return Err(Errno::EINVAL);
        }
        if path == 0 {
            return Err(Errno::EFAULT);
        }
        lookup(
            kernel,
            cx,
            tid,
            dirfd,
            path,
            flags,
            flags & AT_SYMLINK_NOFOLLOW == 0,
        )?
    };
    if let Some(wanted) = &wanted {
        for time in wanted {
            let nanoseconds = time.tv_nsec;
            if nanoseconds != i64::from(UTIME_NOW)
                && nanoseconds != i64::from(UTIME_OMIT)
                && !(0..1_000_000_000).contains(&nanoseconds)
            {
                return Err(Errno::EINVAL);
            }
        }
    }
    let now = kernel.now(cx);
    let credentials = credentials(kernel, tid).clone();
    let mut inode = node.borrow_mut();
    // Only "now" for both times is a touch, which write access allows; anything else (even "now"
    // with one left alone) sets times, which needs the owner (Linux's `vfs_utimes`).
    let only_now = wanted.is_none_or(|wanted| {
        wanted
            .iter()
            .all(|time| time.tv_nsec == i64::from(UTIME_NOW))
    });
    if inode.fs.read_only() {
        return Err(Errno::EROFS);
    }
    if inode.is_anonymous() {
        let omitted = wanted.as_ref().is_some_and(|wanted| {
            wanted
                .iter()
                .all(|time| time.tv_nsec == i64::from(UTIME_OMIT))
        });
        return if omitted {
            Ok(0)
        } else {
            Err(Errno::EOPNOTSUPP)
        };
    }
    // A process's directory refuses every change (measured on the reference).
    if matches!(
        inode.content,
        Content::Proc(crate::fs::proc::Entry::Process(_))
    ) {
        return Err(Errno::EPERM);
    }
    let owner = credentials.owns_or_capable(inode.uid);
    if !owner {
        if !only_now {
            return Err(Errno::EPERM);
        }
        crate::fs::permission(&inode, &credentials, W_OK, kernel.threads[&tid].process)?;
    }
    let pick = |time: Option<&__kernel_timespec>, old: Time| match time {
        None => Some(now),
        Some(time) if time.tv_nsec == i64::from(UTIME_NOW) => Some(now),
        Some(time) if time.tv_nsec == i64::from(UTIME_OMIT) => {
            let _ = old;
            None
        }
        Some(time) => Some(Time {
            seconds: time.tv_sec,
            nanoseconds: time.tv_nsec as u32,
        }),
    };
    if let Some(atime) = pick(wanted.as_ref().map(|wanted| &wanted[0]), inode.atime) {
        inode.atime = atime;
    }
    if let Some(mtime) = pick(wanted.as_ref().map(|wanted| &wanted[1]), inode.mtime) {
        inode.mtime = mtime;
    }
    inode.ctime = now;
    drop(inode);
    keep_proc(kernel, &node);
    Ok(0)
}

fn truncate_node(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    node: &Node,
    length: u64,
) -> Result<u64, Errno> {
    if (length as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    if node.borrow().is_anonymous() {
        return Err(Errno::EOPNOTSUPP);
    }
    let may_keep_privileges = credentials(kernel, tid).has(CAP_FSETID);
    let now = kernel.now(cx);
    // A file of `/proc` takes any size its file system allows and keeps none (`proc_setattr`,
    // `proc_notify_change`): proc's largest is `MAX_NON_LFS`.
    let proc_file = {
        let inode = node.borrow();
        matches!(inode.content, Content::Proc(_)) && inode.file_type() == S_IFREG
    };
    if proc_file {
        super::io::new_size_ok(kernel, cx, tid, 0, length)?;
        if length > i32::MAX as u64 {
            return Err(Errno::EFBIG);
        }
        return Ok(0);
    }
    // A queue's size is only what it shows (`simple_setattr`), up to its file system's largest.
    if matches!(node.borrow().content, Content::Mqueue(_)) {
        let size = node.borrow().size();
        super::io::new_size_ok(kernel, cx, tid, size, length)?;
        if length > i32::MAX as u64 {
            return Err(Errno::EFBIG);
        }
        let mut inode = node.borrow_mut();
        if let Content::Mqueue(queue) = &mut inode.content {
            queue.size = length;
        }
        inode.mtime = now;
        inode.ctime = now;
        inode.drop_privileges(!may_keep_privileges);
        return Ok(0);
    }
    {
        let inode = node.borrow();
        if !matches!(inode.content, Content::Regular(_)) {
            return Err(if inode.is_directory() {
                Errno::EISDIR
            } else {
                Errno::EINVAL
            });
        }
    }
    let size = node.borrow().size();
    // A secret memory file takes a size once (`secretmem_setattr`).
    if node.borrow().is_secret() && size != 0 {
        return Err(Errno::EINVAL);
    }
    super::io::new_size_ok(kernel, cx, tid, size, length)?;
    let mut inode = node.borrow_mut();
    let Content::Regular(regular) = &mut inode.content else {
        return Err(Errno::EINVAL);
    };
    regular.may_resize(length)?;
    regular.truncate(cx.cpu, length);
    inode.mtime = now;
    inode.ctime = now;
    inode.drop_privileges(!may_keep_privileges);
    drop(inode);
    crate::shared::changed(kernel, cx, node)?;
    Ok(0)
}

pub(super) fn truncate(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [path, length, ..]: Args,
) -> Result<u64, Errno> {
    if (length as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    // In the order of Linux's `vfs_truncate`.
    let node = lookup(kernel, cx, tid, AT_FDCWD as u64, path, 0, true)?;
    if node.borrow().is_directory() {
        return Err(Errno::EISDIR);
    }
    if node.borrow().file_type() != S_IFREG {
        return Err(Errno::EINVAL);
    }
    // The permission comes before the mount's write access (measured).
    let caller = kernel.threads[&tid].process;
    crate::fs::permission(&node.borrow(), credentials(kernel, tid), W_OK, caller)?;
    if node.borrow().fs.read_only() {
        return Err(Errno::EROFS);
    }
    truncate_node(kernel, cx, tid, &node, length)
}

pub(super) fn ftruncate(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, length, ..]: Args,
) -> Result<u64, Errno> {
    if (length as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    let file = kernel.process_of(tid).files.file(fd as i32)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    if (length as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    let node = file.borrow().node.clone();
    if !file.borrow().writable() || node.borrow().file_type() != S_IFREG {
        return Err(Errno::EINVAL);
    }
    truncate_node(kernel, cx, tid, &node, length)
}

pub(super) fn fallocate(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, mode, offset, length, ..]: Args,
) -> Result<u64, Errno> {
    // In the order of Linux's `vfs_fallocate`; tmpfs takes keeping the size and punching holes.
    let file = kernel.process_of(tid).files.file(fd as i32)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let mode = mode as u32;
    if (offset as i64) < 0 || (length as i64) <= 0 {
        return Err(Errno::EINVAL);
    }
    let modes = FALLOC_FL_PUNCH_HOLE
        | FALLOC_FL_COLLAPSE_RANGE
        | FALLOC_FL_ZERO_RANGE
        | FALLOC_FL_INSERT_RANGE
        | FALLOC_FL_UNSHARE_RANGE
        | FALLOC_FL_WRITE_ZEROES;
    if mode & !(modes | FALLOC_FL_KEEP_SIZE) != 0 {
        return Err(Errno::EOPNOTSUPP);
    }
    // The modes are exclusive; some need the size kept and some forbid it.
    let keep = mode & FALLOC_FL_KEEP_SIZE != 0;
    let punch = mode & FALLOC_FL_PUNCH_HOLE != 0;
    let allowed = match mode & modes {
        FALLOC_FL_ALLOCATE_RANGE | FALLOC_FL_UNSHARE_RANGE | FALLOC_FL_ZERO_RANGE => true,
        FALLOC_FL_PUNCH_HOLE => keep,
        FALLOC_FL_COLLAPSE_RANGE | FALLOC_FL_INSERT_RANGE | FALLOC_FL_WRITE_ZEROES => !keep,
        _ => false,
    };
    if !allowed {
        return Err(Errno::EOPNOTSUPP);
    }
    if !file.borrow().writable() {
        return Err(Errno::EBADF);
    }
    let node = file.borrow().node.clone();
    {
        let inode = node.borrow();
        match inode.file_type() {
            S_IFREG => {}
            S_IFIFO => return Err(Errno::ESPIPE),
            S_IFDIR => return Err(Errno::EISDIR),
            _ => return Err(Errno::ENODEV),
        }
    }
    let largest = node.borrow().fs.kind.max_bytes();
    let end = offset
        .checked_add(length)
        .filter(|end| *end <= largest)
        .ok_or(Errno::EFBIG)?;
    if node.borrow().is_secret()
        || node.borrow().is_anonymous()
        || matches!(node.borrow().content, Content::Mqueue(_))
    {
        return Err(Errno::EOPNOTSUPP);
    }
    // Only keeping the size and punching holes exist in tmpfs.
    if mode & !(FALLOC_FL_KEEP_SIZE | FALLOC_FL_PUNCH_HOLE) != 0 {
        return Err(Errno::EOPNOTSUPP);
    }
    // tmpfs checks the size limit even when the size is kept, then the seals.
    if !punch {
        let size = node.borrow().size();
        super::io::new_size_ok(kernel, cx, tid, size, end)?;
    }
    let now = kernel.now(cx);
    let mut inode = node.borrow_mut();
    // Files of `/proc` have no `fallocate`.
    let Content::Regular(regular) = &mut inode.content else {
        return Err(Errno::EOPNOTSUPP);
    };
    if punch && regular.seals & (F_SEAL_WRITE | F_SEAL_FUTURE_WRITE) != 0
        || !punch && regular.seals & F_SEAL_GROW != 0 && end > regular.size()
    {
        return Err(Errno::EPERM);
    }
    if punch {
        regular.punch(cx.cpu, offset, end);
        inode.mtime = now;
    } else {
        // tmpfs refuses a range larger than it could ever hold at once, then takes the pages one
        // by one, giving back those it took if it runs out, and only then sets the size.
        let (first, last) = (offset / 4096, end.div_ceil(4096));
        if let Some(space) = &inode.fs.space
            && space.max_blocks() != 0
            && last - first > space.max_blocks()
        {
            return Err(Errno::ENOSPC);
        }
        let Content::Regular(regular) = &mut inode.content else {
            return Err(Errno::ENODEV);
        };
        let mut taken = Vec::new();
        for page in first..last {
            let held = regular.missing_pages(page, page + 1) == 0;
            if !regular.reserve(page) {
                regular.unreserve(&taken);
                return Err(Errno::ENOSPC);
            }
            if !held {
                taken.push(page);
            }
        }
        if mode & FALLOC_FL_KEEP_SIZE == 0 && regular.size() < end {
            regular.truncate(cx.cpu, end);
        }
    }
    inode.ctime = now;
    drop(inode);
    crate::shared::changed(kernel, cx, &node)?;
    Ok(0)
}

pub(super) fn getcwd(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [buffer, size, ..]: Args,
) -> Result<u64, Errno> {
    let path = kernel.with_walk(tid, |walk, process| walk.cwd_path(&process.cwd))?;
    let length = path.len() as u64 + 1;
    if length > size {
        return Err(Errno::ERANGE);
    }
    let mut bytes = path;
    bytes.push(0);
    // The path is made at the end of a page, which is where the copy starts from.
    guest(kernel, cx, tid).write_from(
        buffer,
        &bytes,
        wcontainers_abi::PAGE_SIZE.wrapping_sub(length),
    )?;
    Ok(length)
}

fn change_directory(kernel: &mut Kernel, tid: Tid, node: Node) -> Result<u64, Errno> {
    {
        let inode = node.borrow();
        if !inode.is_directory() {
            return Err(Errno::ENOTDIR);
        }
        crate::fs::permission(
            &inode,
            credentials(kernel, tid),
            X_OK,
            kernel.threads[&tid].process,
        )?;
    }
    kernel.process_of_mut(tid).cwd = node;
    Ok(0)
}

pub(super) fn chdir(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [path, ..]: Args,
) -> Result<u64, Errno> {
    let node = lookup(kernel, cx, tid, AT_FDCWD as u64, path, 0, true)?;
    change_directory(kernel, tid, node)
}

pub(super) fn fchdir(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, ..]: Args,
) -> Result<u64, Errno> {
    let node = kernel
        .process_of(tid)
        .files
        .file(fd as i32)?
        .borrow()
        .node
        .clone();
    change_directory(kernel, tid, node)
}

pub(super) fn chroot(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [path, ..]: Args,
) -> Result<u64, Errno> {
    let node = lookup(kernel, cx, tid, AT_FDCWD as u64, path, 0, true)?;
    {
        let inode = node.borrow();
        if !inode.is_directory() {
            return Err(Errno::ENOTDIR);
        }
        crate::fs::permission(
            &inode,
            credentials(kernel, tid),
            X_OK,
            kernel.threads[&tid].process,
        )?;
    }
    if !credentials(kernel, tid).has(CAP_SYS_CHROOT) {
        return Err(Errno::EPERM);
    }
    kernel.process_of_mut(tid).root = node;
    Ok(0)
}

pub(super) fn umask(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [mask, ..]: Args,
) -> Result<u64, Errno> {
    let process = kernel.process_of_mut(tid);
    let old = process.umask;
    process.umask = mask as u32 & 0o777;
    Ok(old.into())
}

/// What `statfs` reports for the file system holding `node`: tmpfs's limits and what is in use
/// (`shmem_statfs`; a limit of zero, as on the internal mount, reports nothing), and nothing for
/// the others (`simple_statfs`).
fn statfs_of(kernel: &mut Kernel, cx: &mut Context<'_>, node: &Node) -> statfs {
    let inode = node.borrow();
    let mut out: statfs = zerocopy::FromZeros::new_zeroed();
    out.f_type = inode.fs.kind.magic() as i64;
    out.f_bsize = wcontainers_abi::PAGE_SIZE as i64;
    out.f_namelen = 255;
    out.f_frsize = wcontainers_abi::PAGE_SIZE as i64;
    if let Some(space) = &inode.fs.space {
        if space.max_blocks() != 0 {
            let free = space.free_blocks();
            out.f_blocks = space.max_blocks() as i64;
            out.f_bfree = free as i64;
            out.f_bavail = free as i64;
            // Pages a mapping's faults brought in are counted by tmpfs and not known here.
            if space.has_mapped_files() {
                kernel
                    .counters
                    .count(cx.host, reason::RESIDENCY_NOT_TRACKED, 0);
            }
        }
        if space.max_inodes() != 0 {
            out.f_files = space.max_inodes() as i64;
            out.f_ffree = space.free_inodes() as i64;
        }
    }
    let (major, minor) = crate::fs::decode_device(inode.fs.device);
    out.f_fsid = __kernel_fsid_t {
        val: [(major << 8 | minor) as i32, 0],
    };
    out.f_flags = i64::from(inode.fs.flags | crate::linux_extra::ST_VALID);
    out
}

pub(super) fn statfs(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [path, buffer, ..]: Args,
) -> Result<u64, Errno> {
    let node = lookup(kernel, cx, tid, AT_FDCWD as u64, path, 0, true)?;
    let out = statfs_of(kernel, cx, &node);
    guest(kernel, cx, tid).write_value(buffer, &out)?;
    Ok(0)
}

pub(super) fn fstatfs(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, ..]: Args,
) -> Result<u64, Errno> {
    let file = kernel.process_of(tid).files.file(fd as i32)?;
    let node = file.borrow().node.clone();
    let mut out = statfs_of(kernel, cx, &node);
    if file.borrow().internal_mount {
        out.f_flags = i64::from(crate::linux_extra::ST_VALID);
    }
    guest(kernel, cx, tid).write_value(buffer, &out)?;
    Ok(0)
}

pub(super) fn flock(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    arguments: Args,
) -> super::Outcome {
    match flock_typed(kernel, cx, tid, arguments) {
        Ok(Some(value)) => super::Outcome::Return(value),
        Ok(None) => {
            // Another open file holds a conflicting lock: the wait lasts until it is let go or a
            // signal comes.
            if kernel.signal_pending(tid) {
                return Err(Errno::ERESTARTSYS).into();
            }
            kernel.park(tid, crate::process::Channel::Locks);
            super::Outcome::Parked
        }
        Err(error) => Err(error).into(),
    }
}

/// `flock`: Some(value) when it is done, None when it must wait for another holder to let go.
fn flock_typed(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, operation, ..]: Args,
) -> Result<Option<u64>, Errno> {
    // In the order of Linux's `flock` and `flock_lock_inode`. Mandatory locks were removed from
    // Linux; asking for one does nothing, before the descriptor is even looked at.
    let operation = operation as u32;
    if operation & LOCK_MAND != 0 {
        return Ok(Some(0));
    }
    let wait = operation & LOCK_NB == 0;
    let exclusive = match operation & !LOCK_NB {
        LOCK_SH => Some(false),
        LOCK_EX => Some(true),
        LOCK_UN => None,
        _ => return Err(Errno::EINVAL),
    };
    let file = kernel.process_of(tid).files.file(fd as i32)?;
    let usable = {
        let file = file.borrow();
        file.readable() || file.writable()
    };
    if file.borrow().is_path() || (exclusive.is_some() && !usable) {
        return Err(Errno::EBADF);
    }
    let node = file.borrow().node.clone();
    let mut inode = node.borrow_mut();
    let own = alloc::rc::Rc::downgrade(&file);
    inode.flocks.retain(|(holder, _)| holder.strong_count() > 0);
    // The file's own lock is dropped first, so a refused change loses it, as on Linux.
    if let Some(index) = inode
        .flocks
        .iter()
        .position(|(holder, _)| holder.ptr_eq(&own))
    {
        if Some(inode.flocks[index].1) == exclusive {
            return Ok(Some(0));
        }
        inode.flocks.remove(index);
        // Waiters blocked on the lock let go try again (`locks_delete_lock_ctx`).
        drop(inode);
        kernel.wake(crate::process::Channel::Locks);
        inode = node.borrow_mut();
    }
    let Some(exclusive) = exclusive else {
        return Ok(Some(0));
    };
    let conflict = inode
        .flocks
        .iter()
        .any(|(_, held_exclusive)| exclusive || *held_exclusive);
    if conflict {
        return if wait { Ok(None) } else { Err(Errno::EAGAIN) };
    }
    inode.flocks.push((own, exclusive));
    Ok(Some(0))
}
