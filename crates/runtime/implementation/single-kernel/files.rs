//! The local system's file backend: a directory of the host served to the kernel in Linux's FUSE
//! messages, with the rights of the user the runtime runs as. Every name is opened relative to its
//! directory's descriptor without following links, so nothing outside the directory is reached.
//!
//! What the host cannot do as Linux does is answered with the error Linux gives for it, counted and
//! warned (the reasons in [`reason`]).

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;

use fuse_abi::linux::{
    FATTR_ATIME, FATTR_ATIME_NOW, FATTR_FH, FATTR_GID, FATTR_MODE, FATTR_MTIME, FATTR_MTIME_NOW,
    FATTR_SIZE, FATTR_UID, FUSE_ASYNC_READ, FUSE_ATOMIC_O_TRUNC, FUSE_AUTO_INVAL_DATA,
    FUSE_BIG_WRITES, FUSE_DO_READDIRPLUS, FUSE_DONT_MASK, FUSE_GETATTR_FH,
    FUSE_KERNEL_MINOR_VERSION, FUSE_KERNEL_VERSION, FUSE_MAX_PAGES, FUSE_PARALLEL_DIROPS,
    FUSE_READDIRPLUS_AUTO, FUSE_ROOT_ID, fuse_attr, fuse_attr_out, fuse_batch_forget_in,
    fuse_create_in, fuse_dirent, fuse_entry_out, fuse_forget_in, fuse_forget_one, fuse_getattr_in,
    fuse_getxattr_in, fuse_getxattr_out, fuse_in_header, fuse_init_in, fuse_init_out, fuse_kstatfs,
    fuse_link_in, fuse_lseek_in, fuse_lseek_out, fuse_mkdir_in, fuse_mknod_in, fuse_opcode,
    fuse_open_in, fuse_open_out, fuse_out_header, fuse_read_in, fuse_release_in, fuse_rename_in,
    fuse_rename2_in, fuse_setattr_in, fuse_statfs_out, fuse_write_in, fuse_write_out,
};
use rustix::fs::{self as host, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, SeekFrom};
use rustix::io::Errno as HostErrno;
use wcontainers_abi::PAGE_SIZE;
use wcontainers_kernel::errno::Errno;
use wcontainers_kernel_linux::linux::general as linux;
use zerocopy::{FromBytes, Immutable, IntoBytes};

use crate::errno;

/// The file backend's slow paths.
pub mod reason {
    /// The host gave an error Linux has no number for, answered as EIO; the detail is the host's
    /// number.
    pub const HOST_ERROR_WITHOUT_LINUX_NUMBER: &str =
        "file backend: host error with no Linux number, answered as EIO";
    /// An open flag the host has no equivalent of was left out; the detail is the flag.
    pub const OPEN_FLAG_LEFT_OUT: &str = "file backend: open flag the host lacks left out";
    /// A request this backend does not serve was answered ENOSYS; the detail is its opcode.
    pub const REQUEST_NOT_SERVED: &str = "file backend: request not served, answered ENOSYS";
    /// A device node was asked for or shown: the host user cannot make one, and its number is not
    /// given (Linux's encoding of device numbers is the kernel's own).
    pub const DEVICE_NODE: &str = "file backend: device node not made or its number not given";
    /// A host directory could not be opened to serve the names in it; the detail is the host's
    /// error number.
    pub const DIRECTORY_NOT_OPENED: &str = "file backend: directory not opened, its names refused";
}

/// How long the kernel may keep a name and its attributes before asking again, in seconds: the host
/// may change the directory underneath.
const VALID_SECONDS: u64 = 1;

/// The largest write the kernel is told it may send.
const MAX_WRITE: u32 = 1 << 20;

/// What INIT grants of what the kernel asks.
const GRANTED: u32 = FUSE_ASYNC_READ
    | FUSE_ATOMIC_O_TRUNC
    | FUSE_BIG_WRITES
    | FUSE_DONT_MASK
    | FUSE_AUTO_INVAL_DATA
    | FUSE_DO_READDIRPLUS
    | FUSE_READDIRPLUS_AUTO
    | FUSE_PARALLEL_DIROPS
    | FUSE_MAX_PAGES;

/// Entries of a directory listing are aligned to this many bytes.
const DIRENT_ALIGN: usize = size_of::<u64>();

/// The permission bits of a mode, set-id and sticky bits included.
const PERMISSIONS: u32 = !linux::S_IFMT;

/// A Linux error number.
type Fail = u32;
type Answer = Result<Vec<u8>, Fail>;

/// One name the kernel holds a node for.
struct Node {
    parent: u64,
    name: CString,
    identity: Identity,
    lookups: u64,
    /// A directory's own descriptor, through which the names in it are reached.
    directory: Option<OwnedFd>,
}

enum Handle {
    File(OwnedFd),
    Directory { fd: OwnedFd, entries: Vec<Listed> },
}

struct Listed {
    name: CString,
    ino: u64,
    kind: u32,
}

/// One host directory served.
pub struct Files {
    nodes: HashMap<u64, Node>,
    identities: HashMap<Identity, u64>,
    handles: HashMap<u64, Handle>,
    last: u64,
    counts: HashMap<&'static str, u64>,
    unsent: Vec<(&'static str, u64, u64)>,
}

fn host_error(error: HostErrno) -> Result<Fail, HostErrno> {
    errno::linux(error).ok_or(error)
}

/// A host file's device and inode.
type Identity = (host::Dev, u64);

fn identity(stat: &host::Stat) -> Identity {
    (stat.st_dev, stat.st_ino)
}

/// Linux's type bits and directory entry type for a host file type.
fn kind(file: FileType) -> (u32, u32) {
    match file {
        FileType::RegularFile => (linux::S_IFREG, linux::DT_REG),
        FileType::Directory => (linux::S_IFDIR, linux::DT_DIR),
        FileType::Symlink => (linux::S_IFLNK, linux::DT_LNK),
        FileType::Fifo => (linux::S_IFIFO, linux::DT_FIFO),
        FileType::Socket => (linux::S_IFSOCK, linux::DT_SOCK),
        FileType::CharacterDevice => (linux::S_IFCHR, linux::DT_CHR),
        FileType::BlockDevice => (linux::S_IFBLK, linux::DT_BLK),
        FileType::Unknown => (0, linux::DT_UNKNOWN),
    }
}

/// The NUL-ended name at the start of `bytes`, and what follows it.
fn name(bytes: &[u8]) -> Result<(CString, &[u8]), Fail> {
    let name = CStr::from_bytes_until_nul(bytes).map_err(|_| Errno::EINVAL as u32)?;
    let rest = &bytes[name.to_bytes_with_nul().len()..];
    let plain = name.to_bytes();
    if plain.is_empty() || plain.contains(&b'/') || plain == b"." || plain == b".." {
        return Err(Errno::EINVAL as u32);
    }
    Ok((name.to_owned(), rest))
}

/// A FIFO or socket node.
#[cfg(not(target_vendor = "apple"))]
fn make_node(
    dir: BorrowedFd<'_>,
    name: &CStr,
    file: FileType,
    permissions: u32,
) -> Result<(), HostErrno> {
    host::mknodat(dir, name, file, Mode::from_raw_mode(permissions as _), 0)
}

/// A FIFO or socket node, made at its path, as macOS has no `mknodat`: a socket's by binding a socket there.
#[cfg(target_vendor = "apple")]
fn make_node(
    dir: BorrowedFd<'_>,
    name: &CStr,
    file: FileType,
    permissions: u32,
) -> Result<(), HostErrno> {
    use std::os::unix::ffi::OsStrExt;
    let directory = host::getpath(dir)?;
    let path = Path::new(std::ffi::OsStr::from_bytes(directory.to_bytes()))
        .join(std::ffi::OsStr::from_bytes(name.to_bytes()));
    if file == FileType::Socket {
        std::os::unix::net::UnixListener::bind(&path)
            .map(drop)
            .map_err(|error| HostErrno::from_io_error(&error).unwrap_or(HostErrno::IO))
    } else {
        let mode = nix::sys::stat::Mode::from_bits_truncate(permissions as _);
        nix::unistd::mkfifo(&path, mode).map_err(|error| HostErrno::from_raw_os_error(error as i32))
    }
}

fn read<T: FromBytes>(body: &[u8]) -> Result<(T, &[u8]), Fail> {
    T::read_from_prefix(body).map_err(|_| Errno::EINVAL as u32)
}

fn bytes<T: IntoBytes + Immutable>(value: &T) -> Vec<u8> {
    value.as_bytes().to_vec()
}

impl Files {
    /// Serves `directory`.
    pub fn new(directory: &Path) -> std::io::Result<Self> {
        let root = host::open(
            directory,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let stat = host::fstat(&root)?;
        let mut files = Self {
            nodes: HashMap::new(),
            identities: HashMap::new(),
            handles: HashMap::new(),
            last: FUSE_ROOT_ID,
            counts: HashMap::new(),
            unsent: Vec::new(),
        };
        files.identities.insert(identity(&stat), FUSE_ROOT_ID);
        files.nodes.insert(
            FUSE_ROOT_ID,
            Node {
                parent: FUSE_ROOT_ID,
                name: c".".to_owned(),
                identity: identity(&stat),
                lookups: 1,
                directory: Some(root),
            },
        );
        Ok(files)
    }

    /// The slow paths taken since last asked: reason, detail and count so far.
    pub fn warnings(&mut self) -> Vec<(&'static str, u64, u64)> {
        std::mem::take(&mut self.unsent)
    }

    fn slow(&mut self, reason: &'static str, detail: u64) {
        let count = self.counts.entry(reason).or_default();
        *count += 1;
        self.unsent.push((reason, detail, *count));
    }

    /// Linux's number for a host error, counting one it has none for.
    fn fail(&mut self, error: HostErrno) -> Fail {
        host_error(error).unwrap_or_else(|error| {
            self.slow(
                reason::HOST_ERROR_WITHOUT_LINUX_NUMBER,
                error.raw_os_error() as u64,
            );
            Errno::EIO as u32
        })
    }

    fn fresh(&mut self) -> u64 {
        self.last += 1;
        self.last
    }

    /// The directory a node's name is in, and the name: the root is "." in itself.
    fn at(&self, node: u64) -> Result<(BorrowedFd<'_>, &CStr), Fail> {
        let record = self.nodes.get(&node).ok_or(Errno::ENOENT as u32)?;
        if node == FUSE_ROOT_ID {
            let fd = record.directory.as_ref().expect("the root is open");
            return Ok((fd.as_fd(), &record.name));
        }
        Ok((self.directory(record.parent)?, &record.name))
    }

    /// A directory node's descriptor.
    fn directory(&self, node: u64) -> Result<BorrowedFd<'_>, Fail> {
        let record = self.nodes.get(&node).ok_or(Errno::ENOENT as u32)?;
        record
            .directory
            .as_ref()
            .map(AsFd::as_fd)
            .ok_or(Errno::EACCES as u32)
    }

    fn stat(&mut self, node: u64) -> Result<host::Stat, Fail> {
        let result = match self.nodes.get(&node).and_then(|n| n.directory.as_ref()) {
            Some(fd) => host::fstat(fd),
            None => {
                let (dir, name) = self.at(node)?;
                host::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW)
            }
        };
        result.map_err(|e| self.fail(e))
    }

    fn attr(&mut self, stat: &host::Stat) -> fuse_attr {
        let file = FileType::from_raw_mode(stat.st_mode as _);
        if matches!(file, FileType::CharacterDevice | FileType::BlockDevice) {
            self.slow(reason::DEVICE_NODE, stat.st_ino);
        }
        let mode: u32 = Mode::from_raw_mode(stat.st_mode as _).bits() as _;
        let permissions = mode & PERMISSIONS;
        fuse_attr {
            ino: stat.st_ino,
            size: stat.st_size as u64,
            blocks: stat.st_blocks as u64,
            atime: stat.st_atime as u64,
            mtime: stat.st_mtime as u64,
            ctime: stat.st_ctime as u64,
            atimensec: stat.st_atime_nsec as u32,
            mtimensec: stat.st_mtime_nsec as u32,
            ctimensec: stat.st_ctime_nsec as u32,
            mode: kind(file).0 | permissions,
            nlink: stat.st_nlink as _,
            uid: stat.st_uid,
            gid: stat.st_gid,
            rdev: 0,
            blksize: stat.st_blksize as u32,
            flags: 0,
        }
    }

    fn entry(&mut self, node: u64, stat: &host::Stat) -> fuse_entry_out {
        fuse_entry_out {
            nodeid: node,
            generation: 0,
            entry_valid: VALID_SECONDS,
            attr_valid: VALID_SECONDS,
            entry_valid_nsec: 0,
            attr_valid_nsec: 0,
            attr: self.attr(stat),
        }
    }

    /// The node for `name` in `parent`, now found to be `stat`: one more lookup of it.
    fn remember(&mut self, parent: u64, name: &CStr, stat: &host::Stat) -> u64 {
        let id = identity(stat);
        if let Some(&node) = self.identities.get(&id) {
            let record = self.nodes.get_mut(&node).expect("a remembered node");
            record.lookups += 1;
            if node != FUSE_ROOT_ID {
                record.parent = parent;
                record.name = name.to_owned();
            }
            return node;
        }
        let directory = if FileType::from_raw_mode(stat.st_mode as _) == FileType::Directory {
            let opened = self.directory(parent).and_then(|dir| {
                host::openat(
                    dir,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|e| e.raw_os_error() as u32)
            });
            match opened {
                Ok(fd) => Some(fd),
                Err(raw) => {
                    self.slow(reason::DIRECTORY_NOT_OPENED, u64::from(raw));
                    None
                }
            }
        } else {
            None
        };
        let node = self.fresh();
        self.identities.insert(id, node);
        self.nodes.insert(
            node,
            Node {
                parent,
                name: name.to_owned(),
                identity: id,
                lookups: 1,
                directory,
            },
        );
        node
    }

    fn look_up(&mut self, parent: u64, name: &CStr) -> Answer {
        let dir = self.directory(parent)?;
        let stat = host::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|e| self.fail(e))?;
        let node = self.remember(parent, name, &stat);
        Ok(bytes(&self.entry(node, &stat)))
    }

    fn forget(&mut self, node: u64, count: u64) {
        if node == FUSE_ROOT_ID {
            return;
        }
        let Some(record) = self.nodes.get_mut(&node) else {
            return;
        };
        record.lookups = record.lookups.saturating_sub(count);
        if record.lookups == 0 {
            let id = record.identity;
            self.nodes.remove(&node);
            if self.identities.get(&id) == Some(&node) {
                self.identities.remove(&id);
            }
        }
    }

    fn handle_fd(&self, handle: u64) -> Result<BorrowedFd<'_>, Fail> {
        match self.handles.get(&handle) {
            Some(Handle::File(fd) | Handle::Directory { fd, .. }) => Ok(fd.as_fd()),
            None => Err(Errno::EBADF as u32),
        }
    }

    /// The host's open flags for Linux's, counting those the host lacks.
    fn open_flags(&mut self, flags: u32) -> OFlags {
        let mut host = match flags & linux::O_ACCMODE {
            linux::O_WRONLY => OFlags::WRONLY,
            linux::O_RDWR => OFlags::RDWR,
            _ => OFlags::RDONLY,
        };
        let pairs = [
            (linux::O_APPEND, OFlags::APPEND),
            (linux::O_TRUNC, OFlags::TRUNC),
            (linux::O_EXCL, OFlags::EXCL),
            (linux::O_NONBLOCK, OFlags::NONBLOCK),
            (linux::O_DSYNC, OFlags::DSYNC),
            (linux::O_SYNC, OFlags::SYNC),
            (linux::O_DIRECTORY, OFlags::DIRECTORY),
        ];
        for (flag, equivalent) in pairs {
            if flags & flag == flag {
                host |= equivalent;
            }
        }
        // The descriptor is the backend's own, never the guest's, and 64-bit offsets are the host's.
        let meaningless = linux::O_CLOEXEC | linux::O_LARGEFILE | linux::O_NOCTTY | linux::O_CREAT;
        let known = pairs.iter().fold(
            linux::O_ACCMODE | meaningless | linux::O_NOFOLLOW,
            |all, &(f, _)| all | f,
        );
        let lacking = flags & !known;
        if lacking != 0 {
            self.slow(reason::OPEN_FLAG_LEFT_OUT, u64::from(lacking));
        }
        host | OFlags::NOFOLLOW | OFlags::CLOEXEC
    }

    /// Sets the mode a created name was asked for, which the host's own umask may have narrowed.
    fn set_mode(&mut self, dir: u64, name: &CStr, mode: u32) -> Result<(), Fail> {
        let dir = self.directory(dir)?;
        host::chmodat(dir, name, Mode::from_raw_mode(mode as _), AtFlags::empty())
            .map_err(|e| self.fail(e))
    }

    fn created(&mut self, parent: u64, name: &CStr) -> Answer {
        self.look_up(parent, name)
    }

    /// Serves one request; the reply, or none for a request FUSE gives none.
    pub fn serve(&mut self, message: &[u8]) -> Option<Vec<u8>> {
        let (header, _) = fuse_in_header::read_from_prefix(message).ok()?;
        let end = (header.len as usize).min(message.len());
        let body = message
            .get(size_of::<fuse_in_header>()..end)
            .unwrap_or_default();
        let opcode = header.opcode;
        let answer = match opcode {
            o if o == fuse_opcode::FUSE_FORGET as u32 => {
                if let Ok((forget, _)) = read::<fuse_forget_in>(body) {
                    self.forget(header.nodeid, forget.nlookup);
                }
                return None;
            }
            o if o == fuse_opcode::FUSE_BATCH_FORGET as u32 => {
                if let Ok((batch, mut rest)) = read::<fuse_batch_forget_in>(body) {
                    for _ in 0..batch.count {
                        let Ok((one, more)) = read::<fuse_forget_one>(rest) else {
                            break;
                        };
                        self.forget(one.nodeid, one.nlookup);
                        rest = more;
                    }
                }
                return None;
            }
            o if o == fuse_opcode::FUSE_NOTIFY_REPLY as u32 => return None,
            _ => self.answer(header.nodeid, opcode, body),
        };
        let (error, body) = match answer {
            Ok(body) => (0, body),
            Err(number) => (-(number as i32), Vec::new()),
        };
        let out = fuse_out_header {
            len: (size_of::<fuse_out_header>() + body.len()) as u32,
            error,
            unique: header.unique,
        };
        let mut reply = bytes(&out);
        reply.extend_from_slice(&body);
        Some(reply)
    }

    fn answer(&mut self, node: u64, opcode: u32, body: &[u8]) -> Answer {
        use fuse_opcode::*;
        let is = |op: fuse_opcode| opcode == op as u32;
        if is(FUSE_INIT) {
            let (init, _) = read::<fuse_init_in>(body)?;
            let out = fuse_init_out {
                major: FUSE_KERNEL_VERSION,
                minor: FUSE_KERNEL_MINOR_VERSION,
                max_readahead: init.max_readahead,
                flags: init.flags & GRANTED,
                max_write: MAX_WRITE,
                max_pages: (u64::from(MAX_WRITE) / PAGE_SIZE) as u16,
                ..Default::default()
            };
            return Ok(bytes(&out));
        }
        if is(FUSE_DESTROY) {
            self.handles.clear();
            self.nodes.retain(|&id, _| id == FUSE_ROOT_ID);
            self.identities.retain(|_, &mut id| id == FUSE_ROOT_ID);
            return Ok(Vec::new());
        }
        if is(FUSE_LOOKUP) {
            let (name, _) = name(body)?;
            return self.look_up(node, &name);
        }
        if is(FUSE_GETATTR) {
            let (getattr, _) = read::<fuse_getattr_in>(body)?;
            let stat = if getattr.getattr_flags & FUSE_GETATTR_FH != 0 {
                let fd = self.handle_fd(getattr.fh)?;
                host::fstat(fd).map_err(|e| self.fail(e))?
            } else {
                self.stat(node)?
            };
            return Ok(self.attr_out(&stat));
        }
        if is(FUSE_SETATTR) {
            let (set, _) = read::<fuse_setattr_in>(body)?;
            return self.set_attributes(node, &set);
        }
        if is(FUSE_READLINK) {
            let (dir, name) = self.at(node)?;
            let target = host::readlinkat(dir, name, Vec::new()).map_err(|e| self.fail(e))?;
            return Ok(target.into_bytes());
        }
        if is(FUSE_SYMLINK) {
            let (name, rest) = name(body)?;
            let target = CStr::from_bytes_until_nul(rest).map_err(|_| Errno::EINVAL as u32)?;
            let dir = self.directory(node)?;
            host::symlinkat(target, dir, &name).map_err(|e| self.fail(e))?;
            return self.created(node, &name);
        }
        if is(FUSE_MKNOD) {
            let (mknod, rest) = read::<fuse_mknod_in>(body)?;
            let (name, _) = name(rest)?;
            return self.make_node(node, &name, mknod.mode, mknod.umask);
        }
        if is(FUSE_MKDIR) {
            let (mkdir, rest) = read::<fuse_mkdir_in>(body)?;
            let (name, _) = name(rest)?;
            let mode = mkdir.mode & !mkdir.umask & PERMISSIONS;
            let dir = self.directory(node)?;
            host::mkdirat(dir, &name, Mode::from_raw_mode(mode as _)).map_err(|e| self.fail(e))?;
            self.set_mode(node, &name, mode)?;
            return self.created(node, &name);
        }
        if is(FUSE_UNLINK) || is(FUSE_RMDIR) {
            let (name, _) = name(body)?;
            let flags = if is(FUSE_RMDIR) {
                AtFlags::REMOVEDIR
            } else {
                AtFlags::empty()
            };
            let dir = self.directory(node)?;
            host::unlinkat(dir, &name, flags).map_err(|e| self.fail(e))?;
            return Ok(Vec::new());
        }
        if is(FUSE_RENAME) {
            let (rename, rest) = read::<fuse_rename_in>(body)?;
            return self.rename(node, rename.newdir, 0, rest);
        }
        if is(FUSE_RENAME2) {
            let (rename, rest) = read::<fuse_rename2_in>(body)?;
            return self.rename(node, rename.newdir, rename.flags, rest);
        }
        if is(FUSE_LINK) {
            let (link, rest) = read::<fuse_link_in>(body)?;
            let (name, _) = name(rest)?;
            let (old_dir, old_name) = self.at(link.oldnodeid)?;
            let new_dir = self.directory(node)?;
            host::linkat(old_dir, old_name, new_dir, &name, AtFlags::empty())
                .map_err(|e| self.fail(e))?;
            return self.created(node, &name);
        }
        if is(FUSE_OPEN) {
            let (open, _) = read::<fuse_open_in>(body)?;
            let flags = self.open_flags(open.flags);
            let (dir, name) = self.at(node)?;
            let fd = host::openat(dir, name, flags, Mode::empty()).map_err(|e| self.fail(e))?;
            return Ok(self.opened(Handle::File(fd)));
        }
        if is(FUSE_CREATE) {
            let (create, rest) = read::<fuse_create_in>(body)?;
            let (name, _) = name(rest)?;
            return self.create(node, &name, &create);
        }
        if is(FUSE_READ) {
            let (read_in, _) = read::<fuse_read_in>(body)?;
            let fd = self.handle_fd(read_in.fh)?;
            let mut data = vec![0; read_in.size as usize];
            let count =
                rustix::io::pread(fd, &mut data, read_in.offset).map_err(|e| self.fail(e))?;
            data.truncate(count);
            return Ok(data);
        }
        if is(FUSE_WRITE) {
            let (write, data) = read::<fuse_write_in>(body)?;
            let data = data
                .get(..write.size as usize)
                .ok_or(Errno::EINVAL as u32)?;
            let fd = self.handle_fd(write.fh)?;
            let mut done = 0;
            while done < data.len() {
                let offset = write.offset + done as u64;
                match rustix::io::pwrite(fd, &data[done..], offset) {
                    Ok(0) => break,
                    Ok(count) => done += count,
                    Err(error) => return Err(self.fail(error)),
                }
            }
            let out = fuse_write_out {
                size: done as u32,
                padding: 0,
            };
            return Ok(bytes(&out));
        }
        if is(FUSE_STATFS) {
            let root = self.directory(FUSE_ROOT_ID)?;
            let vfs = host::fstatvfs(root).map_err(|e| self.fail(e))?;
            let out = fuse_statfs_out {
                st: fuse_kstatfs {
                    blocks: vfs.f_blocks,
                    bfree: vfs.f_bfree,
                    bavail: vfs.f_bavail,
                    files: vfs.f_files,
                    ffree: vfs.f_ffree,
                    bsize: vfs.f_bsize as u32,
                    namelen: vfs.f_namemax as u32,
                    frsize: vfs.f_frsize as u32,
                    ..Default::default()
                },
            };
            return Ok(bytes(&out));
        }
        if is(FUSE_RELEASE) || is(FUSE_RELEASEDIR) {
            let (release, _) = read::<fuse_release_in>(body)?;
            self.handles.remove(&release.fh);
            return Ok(Vec::new());
        }
        if is(FUSE_FLUSH) {
            return Ok(Vec::new());
        }
        if is(FUSE_FSYNC) || is(FUSE_FSYNCDIR) {
            let (handle, _) = read::<u64>(body)?;
            let fd = self.handle_fd(handle)?;
            host::fsync(fd).map_err(|e| self.fail(e))?;
            return Ok(Vec::new());
        }
        if is(FUSE_SETXATTR) || is(FUSE_GETXATTR) || is(FUSE_LISTXATTR) || is(FUSE_REMOVEXATTR) {
            return self.attribute(node, opcode, body);
        }
        if is(FUSE_OPENDIR) {
            let dir = self.directory(node)?;
            let fd = host::openat(
                dir,
                c".",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| self.fail(e))?;
            return Ok(self.opened(Handle::Directory {
                fd,
                entries: Vec::new(),
            }));
        }
        if is(FUSE_READDIR) || is(FUSE_READDIRPLUS) {
            let (read_in, _) = read::<fuse_read_in>(body)?;
            return self.list(node, &read_in, is(FUSE_READDIRPLUS));
        }
        if is(FUSE_LSEEK) {
            let (seek, _) = read::<fuse_lseek_in>(body)?;
            let to = match seek.whence {
                linux::SEEK_SET => SeekFrom::Start(seek.offset),
                linux::SEEK_CUR => SeekFrom::Current(seek.offset as i64),
                linux::SEEK_END => SeekFrom::End(seek.offset as i64),
                linux::SEEK_DATA => SeekFrom::Data(seek.offset),
                linux::SEEK_HOLE => SeekFrom::Hole(seek.offset),
                _ => return Err(Errno::EINVAL as u32),
            };
            let fd = self.handle_fd(seek.fh)?;
            let offset = host::seek(fd, to).map_err(|e| self.fail(e))?;
            return Ok(bytes(&fuse_lseek_out { offset }));
        }
        self.slow(reason::REQUEST_NOT_SERVED, u64::from(opcode));
        Err(Errno::ENOSYS as u32)
    }

    fn attr_out(&mut self, stat: &host::Stat) -> Vec<u8> {
        let out = fuse_attr_out {
            attr_valid: VALID_SECONDS,
            attr_valid_nsec: 0,
            dummy: 0,
            attr: self.attr(stat),
        };
        bytes(&out)
    }

    fn opened(&mut self, handle: Handle) -> Vec<u8> {
        let fh = self.fresh();
        self.handles.insert(fh, handle);
        bytes(&fuse_open_out {
            fh,
            open_flags: 0,
            backing_id: 0,
        })
    }

    fn make_node(&mut self, parent: u64, name: &CStr, mode: u32, umask: u32) -> Answer {
        let permissions = mode & !umask & PERMISSIONS;
        let file = match mode & linux::S_IFMT {
            linux::S_IFREG | 0 => FileType::RegularFile,
            linux::S_IFIFO => FileType::Fifo,
            linux::S_IFSOCK => FileType::Socket,
            _ => {
                self.slow(reason::DEVICE_NODE, u64::from(mode));
                return Err(Errno::EPERM as u32);
            }
        };
        let dir = self.directory(parent)?;
        let made = if file == FileType::RegularFile {
            host::openat(
                dir,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(permissions as _),
            )
            .map(drop)
        } else {
            make_node(dir, name, file, permissions)
        };
        made.map_err(|e| self.fail(e))?;
        self.set_mode(parent, name, permissions)?;
        self.created(parent, name)
    }

    fn create(&mut self, parent: u64, name: &CStr, create: &fuse_create_in) -> Answer {
        let permissions = create.mode & !create.umask & PERMISSIONS;
        let flags = self.open_flags(create.flags);
        let dir = self.directory(parent)?;
        let made = host::openat(
            dir,
            name,
            flags | OFlags::CREATE | OFlags::EXCL,
            Mode::from_raw_mode(permissions as _),
        );
        let fd = match made {
            Ok(fd) => {
                host::fchmod(&fd, Mode::from_raw_mode(permissions as _))
                    .map_err(|e| self.fail(e))?;
                fd
            }
            Err(HostErrno::EXIST) if create.flags & linux::O_EXCL == 0 => {
                host::openat(dir, name, flags, Mode::empty()).map_err(|e| self.fail(e))?
            }
            Err(error) => return Err(self.fail(error)),
        };
        let mut reply = self.created(parent, name)?;
        reply.extend_from_slice(&self.opened(Handle::File(fd)));
        Ok(reply)
    }

    fn rename(&mut self, from: u64, to: u64, flags: u32, names: &[u8]) -> Answer {
        let (old, rest) = name(names)?;
        let (new, _) = name(rest)?;
        let mut host_flags = RenameFlags::empty();
        for (flag, equivalent) in [
            (linux::RENAME_NOREPLACE, RenameFlags::NOREPLACE),
            (linux::RENAME_EXCHANGE, RenameFlags::EXCHANGE),
        ] {
            if flags & flag != 0 {
                host_flags |= equivalent;
            }
        }
        if flags & !(linux::RENAME_NOREPLACE | linux::RENAME_EXCHANGE) != 0 {
            return Err(Errno::EINVAL as u32);
        }
        let old_dir = self.directory(from)?;
        let new_dir = self.directory(to)?;
        host::renameat_with(old_dir, &old, new_dir, &new, host_flags).map_err(|e| self.fail(e))?;
        // The nodes of what moved follow it.
        for (dir, name) in [(to, new), (from, old)] {
            let Ok(directory) = self.directory(dir) else {
                continue;
            };
            if let Ok(stat) = host::statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW)
                && let Some(&node) = self.identities.get(&identity(&stat))
                && node != FUSE_ROOT_ID
            {
                let record = self.nodes.get_mut(&node).expect("a remembered node");
                record.parent = dir;
                record.name = name;
            }
        }
        Ok(Vec::new())
    }

    fn set_attributes(&mut self, node: u64, set: &fuse_setattr_in) -> Answer {
        let valid = set.valid;
        let handle = if valid & FATTR_FH != 0 {
            match self.handles.get(&set.fh) {
                Some(Handle::File(fd)) => Some(fd.try_clone().map_err(|_| Errno::EMFILE as u32)?),
                _ => None,
            }
        } else {
            None
        };
        let directory = self
            .nodes
            .get(&node)
            .and_then(|n| n.directory.as_ref())
            .map(|fd| fd.try_clone())
            .transpose()
            .map_err(|_| Errno::EMFILE as u32)?;
        let own = handle.or(directory);
        if valid & FATTR_MODE != 0 {
            let mode = Mode::from_raw_mode((set.mode & PERMISSIONS) as _);
            let done = match &own {
                Some(fd) => host::fchmod(fd, mode),
                None => {
                    let (dir, name) = self.at(node)?;
                    host::chmodat(dir, name, mode, AtFlags::empty())
                }
            };
            done.map_err(|e| self.fail(e))?;
        }
        if valid & (FATTR_UID | FATTR_GID) != 0 {
            let owner = (valid & FATTR_UID != 0).then(|| host::Uid::from_raw(set.uid));
            let group = (valid & FATTR_GID != 0).then(|| host::Gid::from_raw(set.gid));
            let done = match &own {
                Some(fd) => host::fchown(fd, owner, group),
                None => {
                    let (dir, name) = self.at(node)?;
                    host::chownat(dir, name, owner, group, AtFlags::SYMLINK_NOFOLLOW)
                }
            };
            done.map_err(|e| self.fail(e))?;
        }
        if valid & FATTR_SIZE != 0 {
            let done = match &own {
                Some(fd) => host::ftruncate(fd, set.size),
                None => {
                    let (dir, name) = self.at(node)?;
                    host::openat(
                        dir,
                        name,
                        OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .and_then(|fd| host::ftruncate(fd, set.size))
                }
            };
            done.map_err(|e| self.fail(e))?;
        }
        if valid & (FATTR_ATIME | FATTR_MTIME) != 0 {
            let time = |given: u32, now: u32, seconds: u64, nanoseconds: u32| {
                let tv_nsec = if valid & given == 0 {
                    host::UTIME_OMIT
                } else if valid & now != 0 {
                    host::UTIME_NOW
                } else {
                    nanoseconds.into()
                };
                host::Timespec {
                    tv_sec: seconds as i64,
                    tv_nsec,
                }
            };
            let times = host::Timestamps {
                last_access: time(FATTR_ATIME, FATTR_ATIME_NOW, set.atime, set.atimensec),
                last_modification: time(FATTR_MTIME, FATTR_MTIME_NOW, set.mtime, set.mtimensec),
            };
            let done = match &own {
                Some(fd) => host::futimens(fd, &times),
                None => {
                    let (dir, name) = self.at(node)?;
                    host::utimensat(dir, name, &times, AtFlags::SYMLINK_NOFOLLOW)
                }
            };
            done.map_err(|e| self.fail(e))?;
        }
        let stat = match &own {
            Some(fd) => host::fstat(fd).map_err(|e| self.fail(e))?,
            None => self.stat(node)?,
        };
        Ok(self.attr_out(&stat))
    }

    /// The extended attribute calls, through a descriptor of the node (a link has none: Linux
    /// allows no user attributes on one).
    fn attribute(&mut self, node: u64, opcode: u32, body: &[u8]) -> Answer {
        use fuse_opcode::*;
        let is = |op: fuse_opcode| opcode == op as u32;
        let fd = match self.nodes.get(&node).and_then(|n| n.directory.as_ref()) {
            Some(fd) => fd.try_clone().map_err(|_| Errno::EMFILE as u32)?,
            None => {
                let (dir, name) = self.at(node)?;
                match host::openat(
                    dir,
                    name,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                ) {
                    Ok(fd) => fd,
                    Err(HostErrno::LOOP) => return Err(Errno::EPERM as u32),
                    Err(error) => return Err(self.fail(error)),
                }
            }
        };
        if is(FUSE_SETXATTR) {
            // Without FUSE_SETXATTR_EXT the request carries only the size and the flags.
            let (size, rest) = read::<u32>(body)?;
            let (flags, rest) = read::<u32>(rest)?;
            let name = CStr::from_bytes_until_nul(rest).map_err(|_| Errno::EINVAL as u32)?;
            let value = rest[name.to_bytes_with_nul().len()..]
                .get(..size as usize)
                .ok_or(Errno::EINVAL as u32)?;
            let mut host_flags = host::XattrFlags::empty();
            if flags & linux::XATTR_CREATE != 0 {
                host_flags |= host::XattrFlags::CREATE;
            }
            if flags & linux::XATTR_REPLACE != 0 {
                host_flags |= host::XattrFlags::REPLACE;
            }
            host::fsetxattr(&fd, name, value, host_flags).map_err(|e| self.fail(e))?;
            return Ok(Vec::new());
        }
        if is(FUSE_REMOVEXATTR) {
            let name = CStr::from_bytes_until_nul(body).map_err(|_| Errno::EINVAL as u32)?;
            host::fremovexattr(&fd, name).map_err(|e| self.fail(e))?;
            return Ok(Vec::new());
        }
        let (get, rest) = read::<fuse_getxattr_in>(body)?;
        let mut value = vec![0; get.size as usize];
        let count = if is(FUSE_GETXATTR) {
            let name = CStr::from_bytes_until_nul(rest).map_err(|_| Errno::EINVAL as u32)?;
            host::fgetxattr(&fd, name, &mut value[..])
        } else {
            host::flistxattr(&fd, &mut value[..])
        }
        .map_err(|e| self.fail(e))?;
        if get.size == 0 {
            return Ok(bytes(&fuse_getxattr_out {
                size: count as u32,
                padding: 0,
            }));
        }
        value.truncate(count);
        Ok(value)
    }

    /// A directory's entries from `read_in.offset`, as many as fit; with each entry's node when `plus`.
    fn list(&mut self, node: u64, read_in: &fuse_read_in, plus: bool) -> Answer {
        let offset = read_in.offset as usize;
        let relist = offset == 0
            || matches!(self.handles.get(&read_in.fh), Some(Handle::Directory { entries, .. }) if entries.is_empty());
        if relist {
            let entries = {
                let fd = match self.handles.get(&read_in.fh) {
                    Some(Handle::Directory { fd, .. }) => fd,
                    _ => return Err(Errno::EBADF as u32),
                };
                let mut dir =
                    Dir::read_from(fd).map_err(|e| host_error(e).unwrap_or(Errno::EIO as u32))?;
                dir.rewind();
                let mut entries = Vec::new();
                for entry in dir {
                    let entry = entry.map_err(|e| host_error(e).unwrap_or(Errno::EIO as u32))?;
                    entries.push(Listed {
                        name: entry.file_name().to_owned(),
                        ino: entry.ino(),
                        kind: kind(entry.file_type()).1,
                    });
                }
                entries
            };
            if let Some(Handle::Directory { entries: kept, .. }) = self.handles.get_mut(&read_in.fh)
            {
                *kept = entries;
            }
        }
        let Some(Handle::Directory { entries, .. }) = self.handles.get(&read_in.fh) else {
            return Err(Errno::EBADF as u32);
        };
        let wanted: Vec<(usize, CString, u64, u32)> = entries
            .iter()
            .enumerate()
            .skip(offset)
            .map(|(index, e)| (index, e.name.clone(), e.ino, e.kind))
            .collect();
        let mut out = Vec::new();
        for (index, name, ino, kind) in wanted {
            let plain = name.to_bytes();
            let head = if plus {
                size_of::<fuse_entry_out>() + size_of::<fuse_dirent>()
            } else {
                size_of::<fuse_dirent>()
            };
            let size = (head + plain.len()).next_multiple_of(DIRENT_ALIGN);
            if out.len() + size > read_in.size as usize {
                break;
            }
            if plus {
                let entry = if plain == b"." || plain == b".." {
                    fuse_entry_out::default()
                } else {
                    let dir = self.directory(node)?;
                    match host::statat(dir, &name, AtFlags::SYMLINK_NOFOLLOW) {
                        Ok(stat) => {
                            let found = self.remember(node, &name, &stat);
                            self.entry(found, &stat)
                        }
                        // Gone since it was listed.
                        Err(_) => continue,
                    }
                };
                out.extend_from_slice(entry.as_bytes());
            }
            let dirent = fuse_dirent {
                ino,
                off: index as u64 + 1,
                namelen: plain.len() as u32,
                type_: kind,
                name: Default::default(),
            };
            out.extend_from_slice(&dirent.as_bytes()[..size_of::<fuse_dirent>()]);
            out.extend_from_slice(plain);
            out.resize(out.len().next_multiple_of(DIRENT_ALIGN), 0);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One request's bytes: the header, then the body's parts.
    fn request(opcode: fuse_opcode, node: u64, unique: u64, parts: &[&[u8]]) -> Vec<u8> {
        let length = size_of::<fuse_in_header>() + parts.iter().map(|p| p.len()).sum::<usize>();
        let header = fuse_in_header {
            len: length as u32,
            opcode: opcode as u32,
            unique,
            nodeid: node,
            ..Default::default()
        };
        let mut bytes = header.as_bytes().to_vec();
        for part in parts {
            bytes.extend_from_slice(part);
        }
        bytes
    }

    /// The reply's error and body.
    fn reply(files: &mut Files, message: &[u8]) -> (i32, Vec<u8>) {
        let reply = files.serve(message).expect("a reply");
        let (header, body) = fuse_out_header::read_from_prefix(&reply).unwrap();
        assert_eq!(header.len as usize, reply.len());
        (header.error, body.to_vec())
    }

    /// A file made, written, read back, looked up again, listed and removed through FUSE's messages,
    /// as the host directory shows it.
    #[test]
    fn a_file_is_made_written_read_listed_and_removed() {
        // Beside the test program, in the build directory.
        let program = std::env::current_exe().unwrap();
        let directory = program.with_file_name(format!("wc-files-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let mut files = Files::new(&directory).unwrap();
        let init = fuse_init_in {
            major: FUSE_KERNEL_VERSION,
            minor: FUSE_KERNEL_MINOR_VERSION,
            flags: GRANTED,
            ..Default::default()
        };
        let (error, _) = reply(
            &mut files,
            &request(fuse_opcode::FUSE_INIT, 0, 1, &[init.as_bytes()]),
        );
        assert_eq!(error, 0);
        let create = fuse_create_in {
            flags: linux::O_RDWR,
            mode: linux::S_IFREG | 0o640,
            umask: 0o022,
            open_flags: 0,
        };
        let message = request(
            fuse_opcode::FUSE_CREATE,
            FUSE_ROOT_ID,
            2,
            &[create.as_bytes(), b"made\0"],
        );
        let (error, body) = reply(&mut files, &message);
        assert_eq!(error, 0);
        let (entry, rest) = fuse_entry_out::read_from_prefix(&body).unwrap();
        let (open, _) = fuse_open_out::read_from_prefix(rest).unwrap();
        assert_eq!(entry.attr.mode, linux::S_IFREG | 0o640);
        let write = fuse_write_in {
            fh: open.fh,
            size: 5,
            ..Default::default()
        };
        let message = request(
            fuse_opcode::FUSE_WRITE,
            entry.nodeid,
            3,
            &[write.as_bytes(), b"hello"],
        );
        let (error, body) = reply(&mut files, &message);
        assert_eq!(
            (
                error,
                fuse_write_out::read_from_prefix(&body).unwrap().0.size
            ),
            (0, 5)
        );
        let read = fuse_read_in {
            fh: open.fh,
            size: 64,
            ..Default::default()
        };
        let message = request(fuse_opcode::FUSE_READ, entry.nodeid, 4, &[read.as_bytes()]);
        assert_eq!(reply(&mut files, &message), (0, b"hello".to_vec()));
        assert_eq!(std::fs::read(directory.join("made")).unwrap(), b"hello");
        let message = request(fuse_opcode::FUSE_LOOKUP, FUSE_ROOT_ID, 5, &[b"made\0"]);
        let (error, body) = reply(&mut files, &message);
        assert_eq!(error, 0);
        assert_eq!(
            fuse_entry_out::read_from_prefix(&body).unwrap().0.nodeid,
            entry.nodeid
        );
        let message = request(
            fuse_opcode::FUSE_OPENDIR,
            FUSE_ROOT_ID,
            6,
            &[&[0; size_of::<fuse_open_in>()]],
        );
        let (error, body) = reply(&mut files, &message);
        assert_eq!(error, 0);
        let listing = fuse_read_in {
            fh: fuse_open_out::read_from_prefix(&body).unwrap().0.fh,
            size: PAGE_SIZE as u32,
            ..Default::default()
        };
        let message = request(
            fuse_opcode::FUSE_READDIR,
            FUSE_ROOT_ID,
            7,
            &[listing.as_bytes()],
        );
        let (error, body) = reply(&mut files, &message);
        assert_eq!(error, 0);
        let mut names = Vec::new();
        let mut rest = &body[..];
        while let Ok((dirent, after)) = fuse_dirent::read_from_prefix(rest) {
            names.push(after[..dirent.namelen as usize].to_vec());
            let size =
                (size_of::<fuse_dirent>() + dirent.namelen as usize).next_multiple_of(DIRENT_ALIGN);
            rest = &rest[size..];
        }
        names.sort();
        assert_eq!(names, [b".".to_vec(), b"..".to_vec(), b"made".to_vec()]);
        let message = request(fuse_opcode::FUSE_UNLINK, FUSE_ROOT_ID, 8, &[b"made\0"]);
        assert_eq!(reply(&mut files, &message).0, 0);
        let message = request(fuse_opcode::FUSE_LOOKUP, FUSE_ROOT_ID, 9, &[b"made\0"]);
        assert_eq!(reply(&mut files, &message).0, -(Errno::ENOENT as i32));
        let message = request(
            fuse_opcode::FUSE_LOOKUP,
            FUSE_ROOT_ID,
            10,
            &[b"../escape\0"],
        );
        assert_eq!(reply(&mut files, &message).0, -(Errno::EINVAL as i32));
        let _ = std::fs::remove_dir_all(&directory);
    }
}
