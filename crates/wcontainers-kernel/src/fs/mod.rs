//! The file layer: inodes, directories, mounts and walking paths, as Linux's VFS does them.
//!
//! Every file system here lives in the kernel's memory: the in-memory kind (tmpfs), devices, and the
//! process information tree. An inode is shared through `Rc`, so an open file keeps an unlinked inode
//! alive as on Linux.

pub(crate) mod dev;
pub(crate) mod proc;

use alloc::collections::BTreeMap;
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::RefCell;

use wcontainers_abi::Stream;

use crate::Errno;
use crate::credentials::Credentials;
use crate::linux::general::*;

pub(crate) type Node = Rc<RefCell<Inode>>;

/// Linux's limit on symbolic links followed in one walk (`MAXSYMLINKS`).
const MAX_SYMLINKS: u32 = 40;

/// What tmpfs reports as the size of each directory entry (`BOGO_DIRENT_SIZE`).
const DIRENT_SIZE: u64 = 20;

/// A point in time, as the guest sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Time {
    pub seconds: i64,
    pub nanoseconds: u32,
}

impl Time {
    pub fn from_nanoseconds(nanoseconds: u64) -> Self {
        Self {
            seconds: (nanoseconds / 1_000_000_000) as i64,
            nanoseconds: (nanoseconds % 1_000_000_000) as u32,
        }
    }
}

/// One file system instance: its device number and the numbers it gives its inodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileSystem {
    pub kind: Kind,
    /// The device number `stat` reports, as Linux's `st_dev` encodes it.
    pub device: u64,
    pub read_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Tmpfs,
    /// Pseudo-terminal devices, at `/dev/pts`.
    Devpts,
    Proc,
    /// The pipes the container's standard streams are.
    Pipe,
}

impl Kind {
    /// The magic number `statfs` reports.
    pub fn magic(self) -> u64 {
        match self {
            Kind::Tmpfs => TMPFS_MAGIC.into(),
            Kind::Devpts => DEVPTS_SUPER_MAGIC.into(),
            Kind::Proc => PROC_SUPER_MAGIC.into(),
            Kind::Pipe => PIPEFS_MAGIC.into(),
        }
    }
}

pub(crate) struct Inode {
    pub fs: FileSystem,
    pub ino: u64,
    /// File type and permission bits, as `st_mode`.
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub nlink: u32,
    /// For devices, the device number as `st_rdev` encodes it.
    pub rdev: u64,
    pub atime: Time,
    pub mtime: Time,
    pub ctime: Time,
    pub btime: Time,
    /// For directories only.
    pub place: Place,
    pub content: Content,
}

pub(crate) enum Content {
    Regular(Regular),
    Directory(Directory),
    Symlink(Vec<u8>),
    /// A device, a named pipe or a socket node; what opening it does depends on its type and number.
    Special,
    /// One of the container's standard output pipes, read by the host.
    HostStream(Stream),
    /// A file or directory of the process information tree, made when it is looked up.
    Proc(proc::Entry),
}

/// A regular file's bytes, and which pages have been written (holes take no space).
#[derive(Default)]
pub(crate) struct Regular {
    pub data: Vec<u8>,
    pub pages: alloc::collections::BTreeSet<u64>,
}

impl Regular {
    pub fn write(&mut self, offset: usize, bytes: &[u8]) {
        let end = offset + bytes.len();
        if self.data.len() < end {
            self.data.resize(end, 0);
        }
        self.data[offset..end].copy_from_slice(bytes);
        if !bytes.is_empty() {
            for page in offset as u64 / 4096..=(end as u64 - 1) / 4096 {
                self.pages.insert(page);
            }
        }
    }

    pub fn truncate(&mut self, length: usize) {
        self.data.resize(length, 0);
        let pages = (length as u64).div_ceil(4096);
        self.pages.retain(|page| *page < pages);
    }
}

/// A directory's entries, in the order tmpfs lists them: by the position each got when it was added.
pub(crate) struct Directory {
    entries: BTreeMap<u64, (Vec<u8>, Node)>,
    names: BTreeMap<Vec<u8>, u64>,
    next_position: u64,
}

/// Where a directory sits. Directories have exactly one name, so they know it.
#[derive(Default)]
pub(crate) struct Place {
    /// The directory above; for the root of a mounted file system, the one above the mount point. None
    /// for the root of everything.
    pub parent: Option<Weak<RefCell<Inode>>>,
    /// This directory's name in its parent.
    pub name: Vec<u8>,
    /// The root of a file system mounted here.
    pub mounted: Option<Node>,
}

/// The first position given to an entry (`DIR_OFFSET_MIN`); `.` and `..` come before it.
pub(crate) const FIRST_POSITION: u64 = 3;

impl Directory {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            names: BTreeMap::new(),
            next_position: FIRST_POSITION,
        }
    }

    pub fn get(&self, name: &[u8]) -> Option<Node> {
        let position = self.names.get(name)?;
        Some(self.entries[position].1.clone())
    }

    pub fn insert(&mut self, name: &[u8], node: Node) {
        let position = self.next_position;
        self.next_position += 1;
        self.names.insert(name.to_vec(), position);
        self.entries.insert(position, (name.to_vec(), node));
    }

    pub fn remove(&mut self, name: &[u8]) -> Option<Node> {
        let position = self.names.remove(name)?;
        Some(self.entries.remove(&position).expect("entry").1)
    }

    /// Puts another inode under an existing name, keeping its position.
    pub fn replace(&mut self, name: &[u8], node: Node) {
        let position = self.names[name];
        self.entries.insert(position, (name.to_vec(), node));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries at or below a position, newest first, as tmpfs lists them.
    pub fn at_or_below(&self, position: u64) -> impl Iterator<Item = (u64, &[u8], &Node)> {
        self.entries
            .range(..=position)
            .rev()
            .map(|(position, (name, node))| (*position, name.as_slice(), node))
    }

    /// Every entry, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], &Node)> {
        self.entries
            .values()
            .map(|(name, node)| (name.as_slice(), node))
    }
}

impl Inode {
    pub fn file_type(&self) -> u32 {
        self.mode & S_IFMT
    }

    pub fn is_directory(&self) -> bool {
        self.file_type() == S_IFDIR
    }

    pub fn directory(&self) -> Option<&Directory> {
        match &self.content {
            Content::Directory(directory) => Some(directory),
            _ => None,
        }
    }

    pub fn directory_mut(&mut self) -> Option<&mut Directory> {
        match &mut self.content {
            Content::Directory(directory) => Some(directory),
            _ => None,
        }
    }

    /// The size `stat` reports.
    pub fn size(&self) -> u64 {
        match &self.content {
            Content::Regular(file) => file.data.len() as u64,
            Content::Directory(_) if self.fs.kind == Kind::Devpts => 0,
            Content::Directory(directory) => DIRENT_SIZE * (directory.len() as u64 + 2),
            Content::Symlink(target) => target.len() as u64,
            Content::Proc(entry) => entry.size(),
            Content::Special | Content::HostStream(_) => 0,
        }
    }

    /// The 512 byte blocks `stat` reports.
    pub fn blocks(&self) -> u64 {
        match &self.content {
            Content::Regular(file) => file.pages.len() as u64 * 8,
            _ => 0,
        }
    }

    /// The `d_type` of a directory entry for this inode.
    pub fn dirent_type(&self) -> u8 {
        (self.file_type() >> 12) as u8
    }

    /// Linux's `generic_permission`: whether `credentials` may access this inode for `mask` (`R_OK`,
    /// `W_OK`, `X_OK` bits). With `CAP_DAC_OVERRIDE` reading and writing are always allowed, and running
    /// a file needs at least one execute bit.
    pub fn permits(&self, credentials: &Credentials, mask: u32, use_fs_ids: bool) -> bool {
        let uid = if use_fs_ids {
            credentials.fsuid
        } else {
            credentials.uid
        };
        let mode = self.mode;
        let bits = if uid == self.uid {
            (mode >> 6) & 7
        } else if credentials.in_group(self.gid, use_fs_ids) {
            (mode >> 3) & 7
        } else {
            mode & 7
        };
        if mask & !bits == 0 {
            return true;
        }
        if credentials.has(crate::credentials::CAP_DAC_OVERRIDE) {
            let needs_execute = mask & X_OK != 0;
            return !needs_execute || self.is_directory() || mode & 0o111 != 0;
        }
        if credentials.has(crate::credentials::CAP_DAC_READ_SEARCH)
            && mask & W_OK == 0
            && (self.is_directory() || mask & X_OK == 0)
        {
            return true;
        }
        false
    }
}

/// The set-user and set-group bits a change to a file drops: set-user always, set-group when the group
/// may run it (otherwise the bit means mandatory locking).
pub(crate) fn privileges_to_drop(mode: u32) -> u32 {
    let mut drop = 0;
    if mode & S_IFMT != S_IFDIR {
        drop |= mode & S_ISUID;
        if mode & (S_ISGID | S_IXGRP) == S_ISGID | S_IXGRP {
            drop |= S_ISGID;
        }
    }
    drop
}

/// Where the kernel's file systems get their device and inode numbers.
pub(crate) struct Numbers {
    next_device_minor: u32,
    next_ino: u64,
}

impl Default for Numbers {
    fn default() -> Self {
        Self {
            next_device_minor: 0x20,
            next_ino: 1,
        }
    }
}

impl Numbers {
    pub fn file_system(&mut self, kind: Kind, read_only: bool) -> FileSystem {
        let minor = self.next_device_minor;
        self.next_device_minor += 1;
        FileSystem {
            kind,
            device: encode_device(0, minor),
            read_only,
        }
    }

    pub fn ino(&mut self) -> u64 {
        let ino = self.next_ino;
        self.next_ino += 1;
        ino
    }
}

/// A device number as `st_dev` and `st_rdev` hold it (Linux's `new_encode_dev`).
pub(crate) fn encode_device(major: u32, minor: u32) -> u64 {
    u64::from((minor & 0xff) | (major << 8) | ((minor & !0xff) << 12))
}

pub(crate) fn decode_device(device: u64) -> (u32, u32) {
    let device = device as u32;
    (
        (device & 0xfff00) >> 8,
        (device & 0xff) | ((device >> 12) & 0xfff00),
    )
}

/// What a new inode starts as.
pub(crate) struct New {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub rdev: u64,
    pub now: Time,
    pub content: Content,
    /// For a directory, the directory it is made in and its name there.
    pub parent: Option<Node>,
    pub name: Vec<u8>,
}

pub(crate) fn new_inode(fs: FileSystem, ino: u64, new: New) -> Node {
    let nlink = if new.mode & S_IFMT == S_IFDIR { 2 } else { 1 };
    Rc::new(RefCell::new(Inode {
        fs,
        ino,
        mode: new.mode,
        uid: new.uid,
        gid: new.gid,
        nlink,
        rdev: new.rdev,
        atime: new.now,
        mtime: new.now,
        ctime: new.now,
        btime: new.now,
        place: Place {
            parent: new.parent.as_ref().map(Rc::downgrade),
            name: new.name,
            mounted: None,
        },
        content: new.content,
    }))
}

/// Where a walk starts and what it may not go above.
#[derive(Clone)]
pub(crate) struct Walk<'a> {
    pub root: &'a Node,
    pub credentials: &'a Credentials,
    pub proc: &'a proc::View<'a>,
}

/// The result of walking all but the last part of a path.
pub(crate) struct Parent {
    pub directory: Node,
    /// The last part; empty when the path names the start itself ("/" or ".").
    pub name: Vec<u8>,
    /// The path ended with a slash, so the last part must be a directory.
    pub trailing_slash: bool,
}

impl<'a> Walk<'a> {
    /// Finds the inode a path names, starting at `start` for a relative path. The last part is
    /// followed when it is a symbolic link only if `follow`.
    pub fn resolve(&self, start: &Node, path: &[u8], follow: bool) -> Result<Node, Errno> {
        let mut links = 0;
        self.resolve_counted(start, path, follow, &mut links)
    }

    fn resolve_counted(
        &self,
        start: &Node,
        path: &[u8],
        follow: bool,
        links: &mut u32,
    ) -> Result<Node, Errno> {
        let parent = self.parent_counted(start, path, links)?;
        if parent.name.is_empty() {
            return Ok(parent.directory);
        }
        let node = self.step(&parent.directory, &parent.name)?;
        let node = if follow || parent.trailing_slash {
            self.follow(&parent.directory, node, links)?
        } else {
            node
        };
        if parent.trailing_slash && !node.borrow().is_directory() {
            return Err(Errno::ENOTDIR);
        }
        Ok(node)
    }

    /// Finds the inode a path names, following every link, and its path from the root as Linux's
    /// `d_path` gives it.
    pub fn locate(&self, start: &Node, path: &[u8]) -> Result<(Node, Vec<u8>), Errno> {
        let mut links = 0;
        let mut at = start.clone();
        let mut path = path.to_vec();
        loop {
            let parent = self.parent_counted(&at, &path, &mut links)?;
            if parent.name.is_empty() {
                let full = self.path_of(&parent.directory).unwrap_or_default();
                return Ok((parent.directory, full));
            }
            let node = self.step(&parent.directory, &parent.name)?;
            let target = match &node.borrow().content {
                Content::Symlink(target) => Some(target.clone()),
                _ => None,
            };
            if let Some(target) = target {
                links += 1;
                if links > MAX_SYMLINKS {
                    return Err(Errno::ELOOP);
                }
                at = parent.directory;
                path = target;
                continue;
            }
            let node = self.follow(&parent.directory, node, &mut links)?;
            if parent.trailing_slash && !node.borrow().is_directory() {
                return Err(Errno::ENOTDIR);
            }
            if node.borrow().is_directory()
                && let Some(full) = self.path_of(&node)
            {
                return Ok((node, full));
            }
            let mut full = self.path_of(&parent.directory).unwrap_or_default();
            if !full.ends_with(b"/") {
                full.push(b'/');
            }
            full.extend_from_slice(&parent.name);
            return Ok((node, full));
        }
    }

    /// Resolves a symbolic link's target found in `at`, following the last part.
    pub fn resolve_from(&self, at: &Node, target: &[u8], links: &mut u32) -> Result<Node, Errno> {
        self.resolve_counted(at, target, true, links)
    }

    /// Walks every part of a path but the last, which is returned unresolved. "." and ".." as the last
    /// part are resolved, and the name is then empty.
    pub fn parent(&self, start: &Node, path: &[u8]) -> Result<Parent, Errno> {
        let mut links = 0;
        self.parent_counted(start, path, &mut links)
    }

    fn parent_counted(&self, start: &Node, path: &[u8], links: &mut u32) -> Result<Parent, Errno> {
        if path.is_empty() {
            return Err(Errno::ENOENT);
        }
        if path.len() >= PATH_MAX as usize {
            return Err(Errno::ENAMETOOLONG);
        }
        let mut at = if path[0] == b'/' {
            self.root.clone()
        } else {
            start.clone()
        };
        let parts: Vec<&[u8]> = path
            .split(|byte| *byte == b'/')
            .filter(|part| !part.is_empty())
            .collect();
        let trailing_slash = path.ends_with(b"/");
        let Some((last, middle)) = parts.split_last() else {
            return Ok(Parent {
                directory: at,
                name: Vec::new(),
                trailing_slash,
            });
        };
        for part in middle {
            let next = self.step(&at, part)?;
            let next = self.follow(&at, next, links)?;
            if !next.borrow().is_directory() {
                return Err(Errno::ENOTDIR);
            }
            at = next;
        }
        if last.len() > NAME_MAX as usize {
            return Err(Errno::ENAMETOOLONG);
        }
        if *last == b"." || *last == b".." {
            let directory = self.step(&at, last)?;
            return Ok(Parent {
                directory,
                name: Vec::new(),
                trailing_slash,
            });
        }
        self.check_search(&at)?;
        Ok(Parent {
            directory: at,
            name: last.to_vec(),
            trailing_slash,
        })
    }

    fn check_search(&self, directory: &Node) -> Result<(), Errno> {
        let directory = directory.borrow();
        if !directory.is_directory() {
            return Err(Errno::ENOTDIR);
        }
        if !directory.permits(self.credentials, X_OK, true) {
            return Err(Errno::EACCES);
        }
        Ok(())
    }

    /// One step from a directory to a name in it, crossing mounts.
    pub fn step(&self, directory: &Node, name: &[u8]) -> Result<Node, Errno> {
        if name.len() > NAME_MAX as usize {
            return Err(Errno::ENAMETOOLONG);
        }
        self.check_search(directory)?;
        let found = if name == b"." {
            directory.clone()
        } else if name == b".." {
            if Rc::ptr_eq(directory, self.root) {
                directory.clone()
            } else {
                let up = directory
                    .borrow()
                    .place
                    .parent
                    .as_ref()
                    .and_then(Weak::upgrade);
                match up {
                    Some(parent) => parent,
                    None => directory.clone(),
                }
            }
        } else {
            let inode = directory.borrow();
            match &inode.content {
                Content::Directory(entries) => entries.get(name).ok_or(Errno::ENOENT)?,
                Content::Proc(entry) => {
                    let entry = *entry;
                    drop(inode);
                    proc::lookup(self.proc, directory, &entry, name)?
                }
                _ => return Err(Errno::ENOTDIR),
            }
        };
        Ok(mounted_root(found))
    }

    /// Follows a node while it is a symbolic link, relative to the directory it was found in.
    pub fn follow(&self, directory: &Node, node: Node, links: &mut u32) -> Result<Node, Errno> {
        let target = {
            let inode = node.borrow();
            if inode.file_type() != S_IFLNK {
                drop(inode);
                return Ok(node);
            }
            match &inode.content {
                Content::Symlink(target) => target.clone(),
                Content::Proc(entry) => {
                    let entry = *entry;
                    drop(inode);
                    *links += 1;
                    if *links > MAX_SYMLINKS {
                        return Err(Errno::ELOOP);
                    }
                    return proc::follow(self.proc, self, directory, &entry, links);
                }
                _ => return Ok(node.clone()),
            }
        };
        *links += 1;
        if *links > MAX_SYMLINKS {
            return Err(Errno::ELOOP);
        }
        self.resolve_counted(directory, &target, true, links)
    }

    /// The path of a directory from the walk's root, as `getcwd` gives it. None when the directory is
    /// not below the root or has been removed.
    pub fn path_of(&self, node: &Node) -> Option<Vec<u8>> {
        let mut parts: Vec<Vec<u8>> = Vec::new();
        let mut at = node.clone();
        loop {
            if Rc::ptr_eq(&at, self.root) {
                break;
            }
            let up = {
                let inode = at.borrow();
                if !inode.is_directory() || inode.nlink == 0 {
                    return None;
                }
                parts.push(inode.place.name.clone());
                inode.place.parent.as_ref()?.upgrade()?
            };
            at = mounted_root(up);
        }
        let mut path = Vec::new();
        for part in parts.iter().rev() {
            path.push(b'/');
            path.extend_from_slice(part);
        }
        if path.is_empty() {
            path.push(b'/');
        }
        Some(path)
    }
}

/// The root of whatever is mounted on a directory, or the directory itself.
fn mounted_root(mut node: Node) -> Node {
    loop {
        let mounted = node.borrow().place.mounted.clone();
        match mounted {
            Some(root) => node = root,
            None => return node,
        }
    }
}

/// Mounts the root of a file system on a directory.
pub(crate) fn mount(on: &Node, root: &Node) {
    let (parent, name) = {
        let inode = on.borrow();
        (inode.place.parent.clone(), inode.place.name.clone())
    };
    {
        let mut inode = root.borrow_mut();
        inode.place.parent = parent;
        inode.place.name = name;
    }
    on.borrow_mut().place.mounted = Some(root.clone());
}
