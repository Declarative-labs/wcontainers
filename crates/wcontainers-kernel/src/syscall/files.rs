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
use crate::fs::{Content, Directory, Node, Regular, Time, privileges_to_drop, proc};
use crate::linux::general::*;
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

/// The start of a walk for a `*at` call: the directory `dirfd` names, or the working directory.
fn start(kernel: &Kernel, tid: Tid, dirfd: u64, path: &[u8]) -> Result<Node, Errno> {
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
fn lookup(
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
    let pid = kernel.threads[&tid].process;
    kernel.with_walk(pid, |walk, _| walk.resolve(&at, &path, follow))
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
    let pid = kernel.threads[&tid].process;
    kernel.with_walk(pid, |walk, _| walk.parent(&at, &path))
}

/// The absolute path of `name` in `directory`, for `/proc/<pid>/fd`.
fn path_in(kernel: &Kernel, tid: Tid, directory: &Node, name: &[u8]) -> Vec<u8> {
    let pid = kernel.threads[&tid].process;
    let mut path = kernel
        .with_walk(pid, |walk, _| {
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
fn may_change(directory: &Node, credentials: &Credentials) -> Result<(), Errno> {
    let inode = directory.borrow();
    if !inode.is_directory() {
        return Err(Errno::ENOTDIR);
    }
    if !inode.permits(credentials, W_OK | X_OK, true) {
        return Err(Errno::EACCES);
    }
    // The process information tree has no operations that change names: permitted callers get EPERM.
    if matches!(inode.content, Content::Proc(_)) {
        return Err(Errno::EPERM);
    }
    if inode.fs.read_only() {
        return Err(Errno::EROFS);
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
    if credentials.fsuid == inode.uid || credentials.has(CAP_FOWNER) {
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

/// The checks before making a name: it must not exist (`EEXIST`). In the process information tree a
/// missing name is an error of the lookup itself (`ENOENT`), as Linux's proc lookups return it.
fn check_absent(kernel: &Kernel, tid: Tid, parent: &crate::fs::Parent) -> Result<(), Errno> {
    let pid = kernel.threads[&tid].process;
    match kernel.with_walk(pid, |walk, _| walk.step(&parent.directory, &parent.name)) {
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
fn create(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    directory: &Node,
    name: &[u8],
    mode: u32,
    rdev: u64,
    content: Content,
) -> Node {
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
    );
    touch_directory(directory, now);
    node
}

pub(super) fn openat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, flags, mode, ..]: Args,
) -> Result<u64, Errno> {
    // The flags are checked before the path is read, as Linux's `build_open_flags` runs first.
    open_flags(kernel, cx, flags as u32)?;
    let path = guest(kernel, cx, tid).read_path(path)?;
    open(kernel, cx, tid, dirfd, &path, flags as u32, mode as u32)
}

/// The flags an open keeps, after `build_open_flags`'s checks.
fn open_flags(kernel: &mut Kernel, cx: &mut Context<'_>, flags: u32) -> Result<u32, Errno> {
    let mut flags = flags & VALID_OPEN_FLAGS;
    if flags & O_PATH != 0 {
        flags &= PATH_FLAGS;
    }
    if flags & (O_DIRECTORY | O_CREAT) == O_DIRECTORY | O_CREAT {
        return Err(Errno::EINVAL);
    }
    let temporary = O_TMPFILE & !O_DIRECTORY;
    if flags & temporary != 0 {
        if flags & (O_TMPFILE | O_CREAT) != O_TMPFILE
            || !matches!(flags & O_ACCMODE, O_WRONLY | O_RDWR)
        {
            return Err(Errno::EINVAL);
        }
        kernel
            .counters
            .count(cx.host, reason::NOT_IMPLEMENTED, u64::from(O_TMPFILE));
        return Err(Errno::EOPNOTSUPP);
    }
    Ok(flags)
}

/// The flags `open` keeps; unknown bits are ignored, as Linux does for `open` and `openat`.
const VALID_OPEN_FLAGS: u32 = O_ACCMODE
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
    let flags = open_flags(kernel, cx, flags)?;
    // Access mode 3 asks for both permissions and gives neither, as on Linux.
    let accmode = flags & O_ACCMODE;
    let at = start(kernel, tid, dirfd, path)?;
    let pid = kernel.threads[&tid].process;
    let parent = kernel.with_walk(pid, |walk, _| walk.parent(&at, path))?;
    let creating = flags & O_CREAT != 0 && flags & O_PATH == 0;
    let found = if parent.name.is_empty() {
        Some(parent.directory.clone())
    } else {
        match kernel.with_walk(pid, |walk, _| walk.step(&parent.directory, &parent.name)) {
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
                if flags & O_NOFOLLOW != 0 {
                    if flags & O_PATH != 0 {
                        node
                    } else {
                        return Err(Errno::ELOOP);
                    }
                } else {
                    let mut links = 0;
                    let followed = kernel.with_walk(pid, |walk, _| {
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
            may_change(&parent.directory, credentials(kernel, tid))?;
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
            );
            (node, true)
        }
    };
    let file_type = node.borrow().file_type();
    if flags & O_DIRECTORY != 0 && file_type != S_IFDIR {
        return Err(Errno::ENOTDIR);
    }
    if flags & O_PATH == 0 {
        if file_type == S_IFDIR && (accmode != O_RDONLY || flags & O_TRUNC != 0) {
            return Err(Errno::EISDIR);
        }
        // Devices on a `nodev` mount may not be opened.
        if matches!(file_type, S_IFCHR | S_IFBLK) && node.borrow().fs.no_devices() {
            return Err(Errno::EACCES);
        }
        if !created {
            let mut mask = 0;
            if matches!(accmode, O_RDONLY | O_RDWR) {
                mask |= R_OK;
            }
            if matches!(accmode, O_WRONLY | O_RDWR) || flags & O_TRUNC != 0 {
                mask |= W_OK;
            }
            let inode = node.borrow();
            if !inode.permits(credentials(kernel, tid), mask, true) {
                return Err(Errno::EACCES);
            }
            if mask & W_OK != 0
                && inode.fs.read_only()
                && matches!(file_type, S_IFREG | S_IFDIR | S_IFLNK)
            {
                return Err(Errno::EROFS);
            }
        }
        match file_type {
            S_IFCHR | S_IFBLK => {
                let rdev = node.borrow().rdev;
                match Device::from_number(rdev) {
                    Some(Device::Tty) => return Err(Errno::ENXIO),
                    Some(Device::Ptmx) => {
                        kernel
                            .counters
                            .count(cx.host, reason::DEVICE_NOT_IMPLEMENTED, rdev);
                        return Err(Errno::ENODEV);
                    }
                    Some(_) => {}
                    None => return Err(Errno::ENXIO),
                }
            }
            S_IFIFO if !matches!(node.borrow().content, Content::HostStream(_)) => {
                let ino = node.borrow().ino;
                kernel
                    .counters
                    .count(cx.host, reason::NAMED_PIPE_NOT_IMPLEMENTED, ino);
                return Err(Errno::ENXIO);
            }
            S_IFSOCK => return Err(Errno::ENXIO),
            _ => {}
        }
        if flags & O_TRUNC != 0 && file_type == S_IFREG {
            let now = kernel.now(cx);
            let mut inode = node.borrow_mut();
            if let Content::Regular(regular) = &mut inode.content {
                regular.truncate(0);
            }
            inode.mtime = now;
            inode.ctime = now;
        }
    }
    let follows = flags & O_NOFOLLOW == 0;
    let full_path = if follows {
        kernel
            .with_walk(pid, |walk, _| walk.locate(&at, path))
            .map(|(_, full)| full)
            .unwrap_or_else(|_| path_in(kernel, tid, &parent.directory, &parent.name))
    } else if parent.name.is_empty() {
        path_in(kernel, tid, &node, b"")
    } else {
        path_in(kernel, tid, &parent.directory, &parent.name)
    };
    let file = OpenFile::new(
        node.clone(),
        flags & !(O_CREAT | O_EXCL | O_NOCTTY | O_TRUNC | O_CLOEXEC),
        full_path,
    );
    let entry = match &node.borrow().content {
        Content::Proc(entry) if file_type == S_IFREG && flags & O_PATH == 0 => Some(*entry),
        _ => None,
    };
    if let Some(entry) = entry {
        let contents = proc_contents(kernel, cx, tid, &entry)?;
        file.borrow_mut().contents = Some(contents);
    }
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
fn proc_contents(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    entry: &proc::Entry,
) -> Result<Vec<u8>, Errno> {
    let pid = kernel.threads[&tid].process;
    let uptime = cx
        .host
        .monotonic_nanoseconds()
        .saturating_sub(kernel.boot_monotonic);
    let mut fresh = [0u8; 16];
    cx.host.random(&mut fresh);
    let mounts = mount_table();
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
    kernel_ref.with_walk(pid, |walk, _| {
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
    may_change(&parent.directory, credentials(kernel, tid))?;
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
    );
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
    may_change(&parent.directory, credentials(kernel, tid))?;
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
    );
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
    may_change(&parent.directory, credentials(kernel, tid))?;
    create(
        kernel,
        cx,
        tid,
        &parent.directory,
        &parent.name,
        S_IFLNK | 0o777,
        0,
        Content::Symlink(target),
    );
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
    may_change(&parent.directory, credentials(kernel, tid))?;
    if node.borrow().is_directory() {
        return Err(Errno::EPERM);
    }
    if node.borrow().nlink == 0 {
        return Err(Errno::ENOENT);
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
    let pid = kernel.threads[&tid].process;
    // A mount point is found as the root mounted on it.
    let node = kernel.with_walk(pid, |walk, _| walk.step(&parent.directory, &parent.name))?;
    // In Linux's order: a trailing slash on a file, then `may_delete`, then the directory's own checks.
    let is_directory = node.borrow().is_directory();
    if !removing_directory && parent.trailing_slash {
        return Err(if is_directory {
            Errno::EISDIR
        } else {
            Errno::ENOTDIR
        });
    }
    may_change(&parent.directory, credentials(kernel, tid))?;
    if !sticky_allows(&parent.directory, &node, credentials(kernel, tid)) {
        return Err(Errno::EPERM);
    }
    if removing_directory {
        if !is_directory {
            return Err(Errno::ENOTDIR);
        }
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
    } else if is_directory {
        return Err(Errno::EISDIR);
    }
    let now = kernel.now(cx);
    parent
        .directory
        .borrow_mut()
        .directory_mut()
        .expect("directory")
        .remove(&parent.name);
    touch_directory(&parent.directory, now);
    let mut inode = node.borrow_mut();
    if is_directory {
        inode.nlink = 0;
        parent.directory.borrow_mut().nlink -= 1;
    } else {
        inode.nlink -= 1;
    }
    inode.ctime = now;
    Ok(0)
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
    if from.directory.borrow().fs != to.directory.borrow().fs {
        return Err(Errno::EXDEV);
    }
    if from.name.is_empty() || to.name.is_empty() {
        return Err(Errno::EBUSY);
    }
    let pid = kernel.threads[&tid].process;
    let source = kernel.with_walk(pid, |walk, _| walk.step(&from.directory, &from.name))?;
    let target = match kernel.with_walk(pid, |walk, _| walk.step(&to.directory, &to.name)) {
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
    // A directory may not move below itself, nor replace one of its own ancestors.
    let below = |moving: &Node, destination: &Node| {
        let mut at = Some(destination.clone());
        while let Some(node) = at {
            if Rc::ptr_eq(&node, moving) {
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
    if flags & RENAME_WHITEOUT != 0 && !credentials.has(CAP_MKNOD) {
        return Err(Errno::EPERM);
    }
    // `may_delete` of the source, then of the target or `may_create` where there is none.
    may_change(&from.directory, &credentials)?;
    if !sticky_allows(&from.directory, &source, &credentials) {
        return Err(Errno::EPERM);
    }
    may_change(&to.directory, &credentials)?;
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
        entries(&from.directory)(&mut |d| {
            d.remove(&from.name);
        });
        if let Some(target) = &target {
            let mut inode = target.borrow_mut();
            if inode.is_directory() {
                inode.nlink = 0;
                drop(inode);
                to.directory.borrow_mut().nlink -= 1;
            } else {
                inode.nlink -= 1;
                inode.ctime = now;
            }
            entries(&to.directory)(&mut |d| {
                d.remove(&to.name);
            });
        }
        entries(&to.directory)(&mut |d| d.insert(&to.name, source.clone()));
        set_place(&source, &to.directory, &to.name);
        if source_is_directory && !Rc::ptr_eq(&from.directory, &to.directory) {
            from.directory.borrow_mut().nlink -= 1;
            to.directory.borrow_mut().nlink += 1;
        }
    }
    // A whiteout, as overlay file systems use: a character device 0:0 left at the old name.
    if flags & RENAME_WHITEOUT != 0 {
        create(
            kernel,
            cx,
            tid,
            &from.directory,
            &from.name,
            S_IFCHR,
            0,
            Content::Special,
        );
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
    let target = {
        let inode = node.borrow();
        match &inode.content {
            Content::Symlink(target) => target.clone(),
            Content::Proc(entry) if inode.file_type() == S_IFLNK => {
                let entry = *entry;
                drop(inode);
                let pid = kernel.threads[&tid].process;
                kernel.with_walk(pid, |walk, _| proc::link_target(walk.proc, walk, &entry))?
            }
            // An empty path names `dirfd` itself, and when that is no link Linux says it is not there.
            _ => return Err(if empty { Errno::ENOENT } else { Errno::EINVAL }),
        }
    };
    let length = target.len().min(size as usize);
    guest(kernel, cx, tid).write(buffer, &target[..length])?;
    Ok(length as u64)
}

/// Fills Linux's `stat` for an inode.
pub(super) fn stat_of(node: &Node) -> stat {
    let inode = node.borrow();
    stat {
        st_dev: inode.fs.device,
        st_ino: inode.ino,
        st_mode: inode.mode,
        st_nlink: inode.nlink,
        st_uid: inode.uid,
        st_gid: inode.gid,
        st_rdev: inode.rdev,
        __pad1: 0,
        st_size: inode.size() as i64,
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

pub(super) fn newfstatat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, buffer, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(AT_SYMLINK_NOFOLLOW | AT_EMPTY_PATH | AT_NO_AUTOMOUNT) != 0 {
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
    guest(kernel, cx, tid).write_value(buffer, &stat_of(&node))?;
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
    guest(kernel, cx, tid).write_value(buffer, &stat_of(&node))?;
    Ok(0)
}

pub(super) fn statx(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, flags, mask, buffer, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(AT_SYMLINK_NOFOLLOW | AT_EMPTY_PATH | AT_NO_AUTOMOUNT | AT_STATX_SYNC_TYPE) != 0
        || flags & AT_STATX_SYNC_TYPE == AT_STATX_SYNC_TYPE
    {
        return Err(Errno::EINVAL);
    }
    if mask as u32 & STATX__RESERVED != 0 {
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
    let plain = stat_of(&node);
    let inode = node.borrow();
    let time = |time: Time| statx_timestamp {
        tv_sec: time.seconds,
        tv_nsec: time.nanoseconds,
        __reserved: 0,
    };
    let (major, minor) = crate::fs::decode_device(inode.fs.device);
    let (rdev_major, rdev_minor) = crate::fs::decode_device(inode.rdev);
    // tmpfs keeps fine-grained times only on request, so it reports the change and modification times,
    // and the birth time, only when asked for them; proc reports the basic set.
    let requested = mask as u32;
    let reported = if inode.fs.kind == crate::fs::Kind::Proc {
        STATX_BASIC_STATS | STATX_MNT_ID
    } else {
        let mut reported = (STATX_BASIC_STATS & !(STATX_MTIME | STATX_CTIME)) | STATX_MNT_ID;
        if requested & (STATX_MTIME | STATX_CTIME) != 0 {
            reported |= STATX_MTIME | STATX_CTIME;
        }
        reported | (requested & STATX_BTIME)
    };
    let mut out: statx = zerocopy::FromZeros::new_zeroed();
    out.stx_mask = reported;
    out.stx_blksize = inode.fs.kind.block_size();
    out.stx_nlink = inode.nlink;
    out.stx_uid = inode.uid;
    out.stx_gid = inode.gid;
    out.stx_mode = inode.mode as u16;
    out.stx_ino = inode.ino;
    out.stx_size = plain.st_size as u64;
    out.stx_blocks = plain.st_blocks as u64;
    let common = STATX_ATTR_AUTOMOUNT | STATX_ATTR_MOUNT_ROOT | STATX_ATTR_DAX;
    out.stx_attributes_mask = u64::from(if inode.fs.kind == crate::fs::Kind::Proc {
        common
    } else {
        common | STATX_ATTR_IMMUTABLE | STATX_ATTR_APPEND | STATX_ATTR_NODUMP
    });
    if inode.place.mount_root {
        out.stx_attributes = u64::from(STATX_ATTR_MOUNT_ROOT);
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
    out.stx_mnt_id = (inode.fs.device & 0xff) + 1;
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
        let old_fsuid = checking.fsuid;
        checking.fsuid = checking.uid;
        checking.fsgid = checking.gid;
        checking.effective = if checking.uid == 0 {
            checking.permitted
        } else {
            0
        };
        let _ = old_fsuid;
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
    if mode == F_OK {
        return Ok(0);
    }
    let inode = node.borrow();
    if !inode.permits(&checking, mode, true) {
        return Err(Errno::EACCES);
    }
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
    let mut inode = node.borrow_mut();
    if credentials.fsuid != inode.uid && !credentials.has(CAP_FOWNER) {
        return Err(Errno::EPERM);
    }
    if inode.fs.read_only() {
        return Err(Errno::EROFS);
    }
    let mut mode = mode & 0o7777;
    if mode & S_ISGID != 0 && !credentials.in_group(inode.gid, true) && !credentials.has(CAP_FSETID)
    {
        mode &= !S_ISGID;
    }
    inode.mode = (inode.mode & S_IFMT) | mode;
    inode.ctime = now;
    Ok(0)
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
    if inode.fs.read_only() {
        return Err(Errno::EROFS);
    }
    if uid != u32::MAX {
        inode.uid = uid;
    }
    if gid != u32::MAX {
        inode.gid = gid;
    }
    inode.mode &= !privileges_to_drop(inode.mode);
    inode.ctime = now;
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
        kernel
            .process_of(tid)
            .files
            .file(dirfd as i32)?
            .borrow()
            .node
            .clone()
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
    let only_now = wanted.is_none_or(|wanted| wanted.iter().all(|time| matches!(time.tv_nsec, n if n == i64::from(UTIME_NOW) || n == i64::from(UTIME_OMIT))));
    let owner = credentials.fsuid == inode.uid || credentials.has(CAP_FOWNER);
    if !owner {
        if !only_now {
            return Err(Errno::EPERM);
        }
        if !inode.permits(&credentials, W_OK, true) {
            return Err(Errno::EACCES);
        }
    }
    if inode.fs.read_only() {
        return Err(Errno::EROFS);
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
    let may_keep_privileges = credentials(kernel, tid).has(CAP_FSETID);
    let now = kernel.now(cx);
    let mut inode = node.borrow_mut();
    let Content::Regular(regular) = &mut inode.content else {
        return Err(if inode.is_directory() {
            Errno::EISDIR
        } else {
            Errno::EINVAL
        });
    };
    regular.truncate(length as usize);
    inode.mtime = now;
    inode.ctime = now;
    if !may_keep_privileges {
        inode.mode &= !privileges_to_drop(inode.mode);
    }
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
    let node = lookup(kernel, cx, tid, AT_FDCWD as u64, path, 0, true)?;
    if node.borrow().is_directory() {
        return Err(Errno::EISDIR);
    }
    if !node.borrow().permits(credentials(kernel, tid), W_OK, true) {
        return Err(Errno::EACCES);
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
    let known = FALLOC_FL_KEEP_SIZE
        | FALLOC_FL_PUNCH_HOLE
        | FALLOC_FL_COLLAPSE_RANGE
        | FALLOC_FL_ZERO_RANGE
        | FALLOC_FL_INSERT_RANGE
        | FALLOC_FL_UNSHARE_RANGE;
    if mode & !known != 0 {
        return Err(Errno::EOPNOTSUPP);
    }
    let punch = mode & FALLOC_FL_PUNCH_HOLE != 0;
    if punch && mode != FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE {
        return Err(Errno::EOPNOTSUPP);
    }
    if mode & (FALLOC_FL_COLLAPSE_RANGE | FALLOC_FL_INSERT_RANGE) != 0
        && mode & !(FALLOC_FL_COLLAPSE_RANGE | FALLOC_FL_INSERT_RANGE) != 0
    {
        return Err(Errno::EINVAL);
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
    let end = offset
        .checked_add(length)
        .filter(|end| *end <= i64::MAX as u64)
        .ok_or(Errno::EFBIG)?;
    // Only keeping the size and punching holes exist in tmpfs.
    if mode & !(FALLOC_FL_KEEP_SIZE | FALLOC_FL_PUNCH_HOLE) != 0 {
        return Err(Errno::EOPNOTSUPP);
    }
    let now = kernel.now(cx);
    let mut inode = node.borrow_mut();
    let Content::Regular(regular) = &mut inode.content else {
        return Err(Errno::ENODEV);
    };
    if punch {
        let size = regular.data.len() as u64;
        let (from, to) = (offset.min(size) as usize, end.min(size) as usize);
        regular.data[from..to].fill(0);
        // Whole pages in the range are given back.
        let first = offset.div_ceil(4096);
        let last = end / 4096;
        regular.pages.retain(|page| *page < first || *page >= last);
        inode.mtime = now;
    } else {
        if mode & FALLOC_FL_KEEP_SIZE == 0 && regular.data.len() < end as usize {
            regular.data.resize(end as usize, 0);
        }
        for page in offset / 4096..end.div_ceil(4096) {
            regular.pages.insert(page);
        }
    }
    inode.ctime = now;
    Ok(0)
}

pub(super) fn getcwd(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [buffer, size, ..]: Args,
) -> Result<u64, Errno> {
    let pid = kernel.threads[&tid].process;
    let path = kernel.with_walk(pid, |walk, process| {
        walk.path_of(&process.cwd).ok_or(Errno::ENOENT)
    })?;
    let length = path.len() as u64 + 1;
    if length > size {
        return Err(Errno::ERANGE);
    }
    let mut bytes = path;
    bytes.push(0);
    guest(kernel, cx, tid).write(buffer, &bytes)?;
    Ok(length)
}

fn change_directory(kernel: &mut Kernel, tid: Tid, node: Node) -> Result<u64, Errno> {
    {
        let inode = node.borrow();
        if !inode.is_directory() {
            return Err(Errno::ENOTDIR);
        }
        if !inode.permits(credentials(kernel, tid), X_OK, true) {
            return Err(Errno::EACCES);
        }
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
        if !inode.permits(credentials(kernel, tid), X_OK, true) {
            return Err(Errno::EACCES);
        }
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

fn statfs_of(node: &Node) -> statfs {
    let inode = node.borrow();
    let mut out: statfs = zerocopy::FromZeros::new_zeroed();
    out.f_type = inode.fs.kind.magic() as i64;
    out.f_bsize = inode.fs.kind.block_size().into();
    out.f_namelen = 255;
    out.f_frsize = inode.fs.kind.block_size().into();
    if inode.fs.kind == crate::fs::Kind::Tmpfs {
        out.f_blocks = 16384;
        out.f_bfree = 16384;
        out.f_bavail = 16384;
        out.f_files = 1 << 20;
        out.f_ffree = 1 << 20;
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
    guest(kernel, cx, tid).write_value(buffer, &statfs_of(&node))?;
    Ok(0)
}

pub(super) fn fstatfs(
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
    guest(kernel, cx, tid).write_value(buffer, &statfs_of(&node))?;
    Ok(0)
}
