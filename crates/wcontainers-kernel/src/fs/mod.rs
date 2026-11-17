//! The file layer: inodes, directories, mounts and walking paths, as Linux's VFS does them.
//!
//! Every file system here lives in the kernel's memory: the in-memory kind (tmpfs), devices, and the
//! process information tree. An inode is shared through `Rc`, so an open file keeps an unlinked inode
//! alive as on Linux.

pub(crate) mod backend;
pub(crate) mod dev;
pub(crate) mod proc;

use alloc::collections::BTreeMap;
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::RefCell;

use alloc::vec;
use core::cell::Cell;
use wcontainers_abi::{Cpu, SharedId};

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
#[derive(Clone)]
pub(crate) struct FileSystem {
    pub kind: Kind,
    /// The device number `stat` reports, as Linux's `st_dev` encodes it.
    pub device: u64,
    /// How it is mounted, as `statfs`'s `ST_*` bits.
    pub flags: u32,
    /// For tmpfs, what it holds against its limits.
    pub space: Option<Rc<Space>>,
}

/// The same file system mounted the same way: a read-only bind mount (`/proc/sys`) is another
/// mount of the same device.
impl PartialEq for FileSystem {
    fn eq(&self, other: &Self) -> bool {
        (self.kind, self.device, self.flags) == (other.kind, other.device, other.flags)
    }
}

/// The space each inode takes from tmpfs's inode limit (`BOGO_INODE_SIZE`).
pub(crate) const INODE_SPACE: u64 = 1024;

/// A symbolic link target tmpfs keeps in the inode, with its zero, at most this long; a longer one
/// takes a page (`SHORT_SYMLINK_LEN`).
const SHORT_SYMLINK: usize = 128;

/// What tmpfs counts against its limits (`shmem_sb_info`): pages, and the inode space inodes, extra
/// links and extended attributes take. A limit of zero is none, as on the kernel's internal mount,
/// which still counts its pages.
pub(crate) struct Space {
    max_blocks: Cell<u64>,
    max_inodes: Cell<u64>,
    used_blocks: Cell<u64>,
    used_ispace: Cell<u64>,
    /// Files whose pages a mapping's faults may have brought in, which are not known here.
    mapped: Cell<u64>,
}

impl Space {
    pub fn new(max_blocks: u64, max_inodes: u64) -> Rc<Self> {
        Rc::new(Self {
            max_blocks: Cell::new(max_blocks),
            max_inodes: Cell::new(max_inodes),
            used_blocks: Cell::new(0),
            used_ispace: Cell::new(0),
            mapped: Cell::new(0),
        })
    }

    /// Sets the limits, as mounting with `size=` and `nr_inodes=` does.
    pub fn set_limits(&self, max_blocks: u64, max_inodes: u64) {
        self.max_blocks.set(max_blocks);
        self.max_inodes.set(max_inodes);
    }

    pub fn max_blocks(&self) -> u64 {
        self.max_blocks.get()
    }

    pub fn max_inodes(&self) -> u64 {
        self.max_inodes.get()
    }

    /// Takes inode space (`shmem_reserve_inode` takes [`INODE_SPACE`]); `ENOSPC` when too little
    /// is left.
    pub fn take_ispace(&self, amount: u64) -> Result<(), Errno> {
        let used = self.used_ispace.get();
        let max = self.max_inodes.get();
        if max != 0 && used + amount > max * INODE_SPACE {
            return Err(Errno::ENOSPC);
        }
        self.used_ispace.set(used + amount);
        Ok(())
    }

    pub fn give_ispace(&self, amount: u64) {
        self.used_ispace.set(self.used_ispace.get() - amount);
    }

    /// The pages that may still be taken.
    pub fn free_blocks(&self) -> u64 {
        match self.max_blocks.get() {
            0 => u64::MAX,
            max => max.saturating_sub(self.used_blocks.get()),
        }
    }

    pub fn used_blocks(&self) -> u64 {
        self.used_blocks.get()
    }

    /// The inodes there is still space for, as `statfs` reports them.
    pub fn free_inodes(&self) -> u64 {
        (self.max_inodes.get() * INODE_SPACE).saturating_sub(self.used_ispace.get()) / INODE_SPACE
    }

    /// Whether a file on it has been mapped, so that pages faults bring in may be missing from the
    /// count.
    pub fn has_mapped_files(&self) -> bool {
        self.mapped.get() != 0
    }

    fn take_blocks(&self, count: u64) {
        self.used_blocks.set(self.used_blocks.get() + count);
    }

    fn give_blocks(&self, count: u64) {
        self.used_blocks.set(self.used_blocks.get() - count);
    }
}

/// What an extended attribute takes from tmpfs's inode space (`simple_xattr_space`).
pub(crate) fn xattr_space(name: &[u8], value: &[u8]) -> u64 {
    40 + (name.len() + value.len()) as u64
}

impl FileSystem {
    pub fn read_only(&self) -> bool {
        self.flags & crate::linux_extra::ST_RDONLY != 0
    }

    /// Devices opened from it are refused (`nodev`).
    pub fn no_devices(&self) -> bool {
        self.flags & crate::linux_extra::ST_NODEV != 0
    }

    /// Whether nothing on it may run: a `noexec` mount, or secret memory, pipes, process descriptors
    /// and the anonymous inode, whose file systems say so themselves (`SB_I_NOEXEC`; measured)
    /// without showing it in statfs's flags.
    pub fn no_execute(&self) -> bool {
        self.flags & crate::linux_extra::ST_NOEXEC != 0
            || matches!(
                self.kind,
                Kind::Secretmem
                    | Kind::Pipe
                    | Kind::Pidfs
                    | Kind::Anon
                    | Kind::Mqueue
                    | Kind::Sockfs
            )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Tmpfs,
    /// Pseudo-terminal devices, at `/dev/pts`.
    Devpts,
    Proc,
    /// The pipes the container's standard streams are.
    Pipe,
    /// Secret memory files, which only a shared mapping reaches (`memfd_secret`).
    Secretmem,
    /// Process descriptors.
    Pidfs,
    /// The one anonymous inode many kinds of descriptor share (`anon_inodefs`).
    Anon,
    /// POSIX message queues, at `/dev/mqueue`.
    Mqueue,
    /// A file system outside the kernel, reached with FUSE messages.
    Fuse,
    /// Sockets, an inode each.
    Sockfs,
}

impl Kind {
    /// The block size `stat` reports. `statfs` reports a page for every kind (measured on the
    /// reference, where proc's files say 1024 and its file system 4096).
    pub fn block_size(self) -> u32 {
        match self {
            Kind::Proc | Kind::Devpts => 1024,
            _ => 4096,
        }
    }

    /// The largest file the file system has (`s_maxbytes`): tmpfs's and the pseudo file systems'
    /// (`pseudo_fs_fill_super`) `MAX_LFS_FILESIZE` (measured for pidfs, pipes, timer descriptors and
    /// secret memory), and for the rest `MAX_NON_LFS`, which `alloc_super` sets and they keep.
    pub fn max_bytes(self) -> u64 {
        match self {
            Kind::Tmpfs
            | Kind::Pidfs
            | Kind::Pipe
            | Kind::Secretmem
            | Kind::Anon
            | Kind::Fuse
            | Kind::Sockfs => i64::MAX as u64,
            Kind::Devpts | Kind::Proc | Kind::Mqueue => i32::MAX as u64,
        }
    }

    /// The magic number `statfs` reports.
    pub fn magic(self) -> u64 {
        match self {
            Kind::Tmpfs => TMPFS_MAGIC.into(),
            Kind::Devpts => DEVPTS_SUPER_MAGIC.into(),
            Kind::Proc => PROC_SUPER_MAGIC.into(),
            Kind::Pipe => PIPEFS_MAGIC.into(),
            Kind::Secretmem => SECRETMEM_MAGIC.into(),
            Kind::Pidfs => PID_FS_MAGIC.into(),
            Kind::Anon => ANON_INODE_FS_MAGIC.into(),
            Kind::Mqueue => crate::linux_extra::MQUEUE_MAGIC.into(),
            Kind::Fuse => FUSE_SUPER_MAGIC.into(),
            Kind::Sockfs => SOCKFS_MAGIC.into(),
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
    /// `flock` locks: the open file holding each, and whether it is exclusive. An entry whose open file
    /// is gone was released when it closed.
    pub flocks: Vec<(alloc::rc::Weak<RefCell<crate::file::OpenFile>>, bool)>,
    /// Record locks (`fcntl`'s).
    pub record_locks: Vec<crate::locks::RecordLock>,
    /// Extended attributes by full name, for file systems that keep them.
    pub xattrs: BTreeMap<Vec<u8>, Vec<u8>>,
    /// tmpfs's inode flags (`FS_*_FL`, as `FS_IOC_GETFLAGS` shows them).
    pub fsflags: u32,
    /// How long data written is expected to live (`RWH_WRITE_LIFE_*`, Linux's `i_write_hint`).
    pub write_hint: u64,
    /// The inotify watches on it.
    pub watches: Vec<crate::syscall::inotify::Mark>,
}

impl Drop for Inode {
    /// An inode gone gives back its space, its extended attributes' and a long link target's page
    /// (`shmem_evict_inode`).
    fn drop(&mut self) {
        let Some(space) = &self.fs.space else {
            return;
        };
        let xattrs: u64 = self
            .xattrs
            .iter()
            .filter(|(name, _)| charged_xattr(name))
            .map(|(name, value)| xattr_space(name, value))
            .sum();
        space.give_ispace(INODE_SPACE + xattrs);
        if self.has_long_target() {
            space.give_blocks(1);
        }
    }
}

/// Whether an extended attribute counts against tmpfs's inode space: all but the access control
/// lists, which tmpfs keeps apart (`system.*`).
pub(crate) fn charged_xattr(name: &[u8]) -> bool {
    !name.starts_with(b"system.")
}

pub(crate) enum Content {
    Regular(Regular),
    Directory(Directory),
    Symlink(Vec<u8>),
    /// A device, a named pipe or a socket node; what opening it does depends on its type and number.
    Special,
    /// A pipe: made by `pipe2`, a named pipe's while open, or one of the container's standard
    /// output pipes, which the host reads.
    Pipe(crate::pipe::Pipe),
    /// A file or directory of the process information tree, made when it is looked up.
    Proc(proc::Entry),
    /// A process descriptor's file, naming a process by number while the process holds it, and
    /// its wait status once it is reaped (`pidfs_exit`).
    Pidfd(crate::process::Pid, Option<i32>),
    /// The anonymous inode; what each file on it is lies in the file (`OpenFile::anon`).
    Anon,
    /// A POSIX message queue.
    Mqueue(alloc::boxed::Box<crate::syscall::mqueue::MessageQueue>),
    /// An inode of a file system outside the kernel, whatever its type.
    Backend(backend::Remote),
    /// A socket's own inode.
    Socket(crate::syscall::socket::SocketRef),
}

/// A regular file's bytes, which pages have been written (holes take no space), and tmpfs's seals.
pub(crate) struct Regular {
    /// The file's first bytes, up to the last one written; the rest up to `size` read as zero. Those
    /// the block holds are stale here while there is one.
    data: Vec<u8>,
    size: u64,
    /// While a mapping shares the file: the block of shared pages holding its first bytes.
    pub block: Option<Block>,
    pub pages: alloc::collections::BTreeSet<u64>,
    /// Pages `fallocate` reserved that nothing has written yet: they take space but read as holes
    /// to SEEK_DATA and SEEK_HOLE, as tmpfs's pages that are not up to date.
    pub allocated: alloc::collections::BTreeSet<u64>,
    /// `F_SEAL_*` bits; a file not made by `memfd_create` to be sealed is sealed against sealing.
    pub seals: u32,
    /// Whether the file has been mapped: tmpfs then brings in the pages a program's faults touch,
    /// which are not known here.
    mapped: bool,
    /// The tmpfs its pages count against.
    space: Option<Rc<Space>>,
    /// Whether it is a System V shared memory segment, whose mappings are never merged (Linux's
    /// `shm_vm_ops` has a `close`).
    pub segment: bool,
}

impl Content {
    /// The block of shared pages a mapped file's mappings show, while it has one.
    pub fn block(&self) -> Option<Block> {
        match self {
            Content::Regular(regular) => regular.block,
            Content::Backend(remote) => remote.pages.as_ref().map(|pages| pages.block),
            _ => None,
        }
    }
}

impl Default for Regular {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            size: 0,
            block: None,
            pages: alloc::collections::BTreeSet::new(),
            allocated: alloc::collections::BTreeSet::new(),
            seals: F_SEAL_SEAL,
            mapped: false,
            space: None,
            segment: false,
        }
    }
}

impl Drop for Regular {
    fn drop(&mut self) {
        if let Some(space) = &self.space {
            space.give_blocks((self.pages.len() + self.allocated.len()) as u64);
            if self.mapped {
                space.mapped.set(space.mapped.get() - 1);
            }
        }
    }
}

impl Regular {
    /// Counts the file's pages against a tmpfs from now on.
    fn count_against(&mut self, space: &Option<Rc<Space>>) {
        if let Some(space) = space {
            space.take_blocks((self.pages.len() + self.allocated.len()) as u64);
        }
        self.space.clone_from(space);
    }

    pub fn is_mapped(&self) -> bool {
        self.mapped
    }

    pub fn set_mapped(&mut self) {
        if !self.mapped
            && let Some(space) = &self.space
        {
            space.mapped.set(space.mapped.get() + 1);
        }
        self.mapped = true;
    }

    fn holds(&self, page: u64) -> bool {
        self.pages.contains(&page) || self.allocated.contains(&page)
    }

    /// How many pages from `first` up to `end` (not included) are not held yet.
    pub fn missing_pages(&self, first: u64, end: u64) -> u64 {
        (first..end).filter(|page| !self.holds(*page)).count() as u64
    }

    /// Holds a page of zeros where it held none, as a write whose copy failed leaves it; its space
    /// is known to be free.
    pub fn hold_page(&mut self, page: u64) {
        if !self.holds(page)
            && let Some(space) = &self.space
        {
            space.take_blocks(1);
        }
        self.allocated.remove(&page);
        self.pages.insert(page);
    }

    /// Reserves a page for `fallocate` if it holds none; `false` when tmpfs is full.
    pub fn reserve(&mut self, page: u64) -> bool {
        if self.holds(page) {
            return true;
        }
        if let Some(space) = &self.space {
            if space.free_blocks() == 0 {
                return false;
            }
            space.take_blocks(1);
        }
        self.allocated.insert(page);
        true
    }

    /// Gives back pages `fallocate` reserved in this call, when it fails part way.
    pub fn unreserve(&mut self, pages: &[u64]) {
        for page in pages {
            if self.allocated.remove(page)
                && let Some(space) = &self.space
            {
                space.give_blocks(1);
            }
        }
    }

    /// How many of `length` bytes at `offset` a write may change, as tmpfs's `shmem_write_begin` allows
    /// them page by page under its seals; `EPERM` when none.
    pub fn writable(&self, offset: u64, length: u64) -> Result<u64, Errno> {
        if self.seals & (F_SEAL_WRITE | F_SEAL_FUTURE_WRITE) != 0 {
            return Err(Errno::EPERM);
        }
        // Page by page, as `shmem_write_begin`: the seals, then a page from tmpfs if the file holds
        // none there.
        let size = self.size;
        let end = offset + length;
        let mut free = self
            .space
            .as_ref()
            .map_or(u64::MAX, |space| space.free_blocks());
        let mut at = offset;
        while at < end {
            let page = at / 4096;
            let chunk_end = ((page + 1) * 4096).min(end);
            if self.seals & F_SEAL_GROW != 0 && chunk_end > size {
                if at == offset {
                    return Err(Errno::EPERM);
                }
                break;
            }
            if !self.holds(page) {
                if free == 0 {
                    if at == offset {
                        return Err(Errno::ENOSPC);
                    }
                    break;
                }
                free -= 1;
            }
            at = chunk_end;
        }
        Ok(at - offset)
    }

    /// Whether the seals allow a change of size (tmpfs's `shmem_setattr`).
    pub fn may_resize(&self, length: u64) -> Result<(), Errno> {
        let size = self.size;
        if (length < size && self.seals & F_SEAL_SHRINK != 0)
            || (length > size && self.seals & F_SEAL_GROW != 0)
        {
            return Err(Errno::EPERM);
        }
        Ok(())
    }

    /// A file holding `bytes`, as unpacking an image makes it.
    pub fn with_bytes(bytes: Vec<u8>) -> Self {
        let mut regular = Self::default();
        regular.mark_written(0, bytes.len() as u64);
        regular.size = bytes.len() as u64;
        regular.data = bytes;
        regular
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    /// Up to `count` bytes from `offset`, stopping at the end of the file.
    pub fn read(&self, cpu: &mut dyn Cpu, offset: u64, count: u64) -> Vec<u8> {
        let start = offset.min(self.size());
        let end = start.saturating_add(count).min(self.size());
        let mut bytes = vec![0; (end - start) as usize];
        let held = (self.data.len() as u64).clamp(start, end);
        if held > start {
            bytes[..(held - start) as usize]
                .copy_from_slice(&self.data[start as usize..held as usize]);
        }
        if let Some(block) = self.block
            && start < block.length
        {
            let shared_end = end.min(block.length);
            cpu.shared_read(block.id, start, &mut bytes[..(shared_end - start) as usize])
                .expect("the block holds the file's first pages");
        }
        bytes
    }

    /// Every byte of the file.
    pub fn bytes(&self, cpu: &mut dyn Cpu) -> Vec<u8> {
        self.read(cpu, 0, self.size())
    }

    pub fn write(&mut self, cpu: &mut dyn Cpu, offset: u64, bytes: &[u8]) {
        let end = offset + bytes.len() as u64;
        if (self.data.len() as u64) < end {
            self.data.resize(end as usize, 0);
        }
        self.size = self.size.max(end);
        self.data[offset as usize..end as usize].copy_from_slice(bytes);
        if let Some(block) = self.block
            && offset < block.length
        {
            let shared_end = end.min(block.length);
            cpu.shared_write(block.id, offset, &bytes[..(shared_end - offset) as usize])
                .expect("the block holds the file's first pages");
        }
        self.mark_written(offset, bytes.len() as u64);
    }

    fn mark_written(&mut self, offset: u64, length: u64) {
        if length > 0 {
            for page in offset / 4096..=(offset + length - 1) / 4096 {
                let was_reserved = self.allocated.remove(&page);
                if self.pages.insert(page)
                    && !was_reserved
                    && let Some(space) = &self.space
                {
                    space.take_blocks(1);
                }
            }
        }
    }

    /// Drops the pages `keep` refuses, giving them back to tmpfs.
    fn drop_pages(&mut self, keep: impl Fn(u64) -> bool) {
        let before = self.pages.len() + self.allocated.len();
        self.pages.retain(|page| keep(*page));
        self.allocated.retain(|page| keep(*page));
        if let Some(space) = &self.space {
            space.give_blocks((before - self.pages.len() - self.allocated.len()) as u64);
        }
    }

    /// Punches a hole in `[offset, end)`: its bytes read as zero and its whole pages are given back.
    pub fn punch(&mut self, cpu: &mut dyn Cpu, offset: u64, end: u64) {
        let (from, to) = (offset.min(self.size()), end.min(self.size()));
        let held = self.data.len() as u64;
        self.data[from.min(held) as usize..to.min(held) as usize].fill(0);
        self.zero_block(cpu, from, to);
        let first = offset.div_ceil(4096);
        let last = end / 4096;
        self.drop_pages(|page| page < first || page >= last);
    }

    fn zero_block(&mut self, cpu: &mut dyn Cpu, from: u64, to: u64) {
        if let Some(block) = self.block
            && from < block.length
        {
            let zeros = vec![0; (to.min(block.length) - from) as usize];
            cpu.shared_write(block.id, from, &zeros)
                .expect("the block holds the file's first pages");
        }
    }

    /// Sets the size. Pages dropped from the end are gone from shared mappings too, so they read as
    /// zero if the file grows again.
    pub fn truncate(&mut self, cpu: &mut dyn Cpu, length: u64) {
        if let Some(block) = self.block
            && length < self.size()
        {
            self.zero_block(cpu, length, block.length);
        }
        self.data.truncate(length as usize);
        self.size = length;
        let pages = length.div_ceil(4096);
        self.drop_pages(|page| page < pages);
    }

    /// Moves the bytes the block holds back into the file's own memory, ending the block.
    pub fn take_block(&mut self, cpu: &mut dyn Cpu) -> Option<Block> {
        let block = self.block.take()?;
        let end = block.length.min(self.size());
        if (self.data.len() as u64) < end {
            self.data.resize(end as usize, 0);
        }
        cpu.shared_read(block.id, 0, &mut self.data[..end as usize])
            .expect("the block holds the file's first pages");
        Some(block)
    }
}

/// A block of shared pages holding a file's first `length` bytes, which every shared mapping of the
/// file shows.
#[derive(Clone, Copy)]
pub(crate) struct Block {
    pub id: SharedId,
    pub length: u64,
}

/// A directory's entries as tmpfs keeps them: each has a position (the offset it got when its name
/// was added, which a rename onto the name keeps) and a place in the list of names tmpfs lists in
/// (newest first; a rename puts the moved entry at the head, as `d_move`).
pub(crate) struct Directory {
    /// By position: the name, the inode and the entry's rank in the list.
    entries: BTreeMap<u64, (Vec<u8>, Node, u64)>,
    names: BTreeMap<Vec<u8>, u64>,
    /// The list: rank to position, the highest rank at the head.
    list: BTreeMap<u64, u64>,
    next_position: u64,
    next_rank: u64,
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
    /// This is the root of a mounted file system.
    pub mount_root: bool,
    /// For a directory of the process information tree, whose inodes live only while something holds
    /// them, the directory above, held so that `..` still finds it.
    pub held_parent: Option<Node>,
}

/// The first position given to an entry (`DIR_OFFSET_MIN`); `.` and `..` come before it.
pub(crate) const FIRST_POSITION: u64 = 3;

impl Directory {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            names: BTreeMap::new(),
            list: BTreeMap::new(),
            next_position: FIRST_POSITION,
            next_rank: 0,
        }
    }

    pub fn get(&self, name: &[u8]) -> Option<Node> {
        let position = self.names.get(name)?;
        Some(self.entries[position].1.clone())
    }

    /// Adds a name with a new position, in place of what the name held.
    pub fn insert(&mut self, name: &[u8], node: Node) {
        let position = self.next_position;
        self.next_position += 1;
        self.put(name, node, position);
    }

    /// The position of a name: its own if it is here, otherwise a new one, taken now
    /// (`simple_offset_add`).
    pub fn position_for(&mut self, name: &[u8]) -> u64 {
        if let Some(position) = self.names.get(name) {
            return *position;
        }
        let position = self.next_position;
        self.next_position += 1;
        position
    }

    /// Puts an inode under a name at a position, at the head of the list, in place of what the name
    /// held.
    pub fn put(&mut self, name: &[u8], node: Node, position: u64) -> Option<Node> {
        let old = self.remove(name);
        let rank = self.next_rank;
        self.next_rank += 1;
        self.names.insert(name.to_vec(), position);
        self.entries.insert(position, (name.to_vec(), node, rank));
        self.list.insert(rank, position);
        old
    }

    pub fn remove(&mut self, name: &[u8]) -> Option<Node> {
        let position = self.names.remove(name)?;
        let (_, node, rank) = self.entries.remove(&position).expect("entry");
        self.list.remove(&rank);
        Some(node)
    }

    /// Puts another inode under an existing name, keeping its position, at the head of the list.
    pub fn replace(&mut self, name: &[u8], node: Node) {
        let position = self.names[name];
        self.put(name, node, position);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries tmpfs lists from a position, after `.` and `..` (`offset_iterate_dir`): down the
    /// list from its head at 2 or below, otherwise from the entry with the greatest position at or
    /// below it, or from the head when there is none.
    pub fn listed_from(&self, position: u64) -> impl Iterator<Item = (u64, &[u8], &Node)> {
        let start = (position > 2)
            .then(|| self.entries.range(..=position).next_back())
            .flatten()
            .map_or(u64::MAX, |(_, (_, _, rank))| *rank);
        self.list.range(..=start).rev().map(|(_, position)| {
            let (name, node, _) = &self.entries[position];
            (*position, name.as_slice(), node)
        })
    }

    /// Every entry, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], &Node)> {
        self.entries
            .values()
            .map(|(name, node, _)| (name.as_slice(), node))
    }
}

impl Inode {
    pub fn file_type(&self) -> u32 {
        self.mode & S_IFMT
    }

    /// The block size `stat` reports: the backend's for its inodes, else the file system kind's.
    pub fn block_size(&self) -> u32 {
        match &self.content {
            Content::Backend(remote) if remote.block_size.get() != 0 => remote.block_size.get(),
            _ => self.fs.kind.block_size(),
        }
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
            Content::Regular(file) => file.size(),
            Content::Directory(_) if self.fs.kind == Kind::Devpts => 0,
            Content::Directory(directory) => DIRENT_SIZE * (directory.len() as u64 + 2),
            Content::Symlink(target) => target.len() as u64,
            Content::Proc(entry) => entry.size(),
            Content::Mqueue(queue) => queue.size,
            Content::Backend(remote) => remote.size.get(),
            Content::Special
            | Content::Pipe(_)
            | Content::Pidfd(..)
            | Content::Anon
            | Content::Socket(_) => 0,
        }
    }

    /// The 512 byte blocks `stat` reports.
    pub fn blocks(&self) -> u64 {
        match &self.content {
            Content::Regular(file) => (file.pages.len() + file.allocated.len()) as u64 * 8,
            Content::Backend(remote) => remote.blocks.get(),
            _ if self.has_long_target() => 8,
            _ => 0,
        }
    }

    /// A tmpfs link whose target takes a page of its own.
    pub fn has_long_target(&self) -> bool {
        self.fs.kind == Kind::Tmpfs
            && matches!(&self.content, Content::Symlink(target) if target.len() + 1 > SHORT_SYMLINK)
    }

    /// Takes the inode space a new value of an extended attribute needs, before the checks of
    /// whether it may be set (`shmem_xattr_handler_set`).
    pub fn reserve_xattr(&self, name: &[u8], value: &[u8]) -> Result<(), Errno> {
        match &self.fs.space {
            Some(space) if charged_xattr(name) => space.take_ispace(xattr_space(name, value)),
            _ => Ok(()),
        }
    }

    /// Gives back space [`Self::reserve_xattr`] took, when the attribute was not set after all.
    pub fn unreserve_xattr(&self, name: &[u8], value: &[u8]) {
        if let Some(space) = &self.fs.space
            && charged_xattr(name)
        {
            space.give_ispace(xattr_space(name, value));
        }
    }

    /// Puts a value whose space is reserved in place of the old one, giving the old one's back.
    pub fn put_xattr(&mut self, name: Vec<u8>, value: Vec<u8>) {
        let old = self.xattrs.insert(name.clone(), value);
        if let Some(old) = old {
            self.unreserve_xattr(&name, &old);
        }
    }

    /// Removes an extended attribute, giving back its space.
    pub fn remove_xattr(&mut self, name: &[u8]) -> Option<Vec<u8>> {
        let old = self.xattrs.remove(name)?;
        self.unreserve_xattr(name, &old);
        Some(old)
    }

    /// What a write, truncation or change of owner takes away from a file that is not a directory: the
    /// set-user and set-group bits when `mode_bits`, and file capabilities that are not empty (Linux's
    /// `cap_inode_need_killpriv`).
    pub fn drop_privileges(&mut self, mode_bits: bool) {
        if mode_bits {
            self.mode &= !privileges_to_drop(self.mode);
        }
        if !self.is_directory()
            && self
                .xattrs
                .get(crate::syscall::CAPABILITY)
                .is_some_and(|value| !value.is_empty())
        {
            self.remove_xattr(crate::syscall::CAPABILITY);
        }
    }

    /// Whether an open file of this inode takes direct I/O (Linux's `FMODE_CAN_ODIRECT`): only tmpfs's
    /// regular files do.
    /// A secret memory file: no reads, writes or positions, one size change, only shared mappings.
    /// Whether it is a System V shared memory segment's file.
    pub fn is_segment(&self) -> bool {
        matches!(&self.content, Content::Regular(regular) if regular.segment)
    }

    /// An inode of process descriptors (regular inside, shown with no file type) or the anonymous
    /// inode (with none): no attributes change and there is no `fallocate` (`pidfs_setattr`,
    /// `anon_inode_setattr`; measured).
    pub fn is_anonymous(&self) -> bool {
        matches!(self.fs.kind, Kind::Pidfs | Kind::Anon)
    }

    /// The mode `stat` shows.
    pub fn shown_mode(&self) -> u32 {
        if self.is_anonymous() {
            self.mode & !S_IFMT
        } else {
            self.mode
        }
    }

    pub fn is_secret(&self) -> bool {
        self.fs.kind == Kind::Secretmem
    }

    /// Whether its name is shown as deleted: it was unlinked, or never had one in any directory.
    pub fn shown_deleted(&self) -> bool {
        self.nlink == 0 || self.is_secret()
    }

    /// Whether opening it with O_DIRECT is allowed (`FMODE_CAN_ODIRECT`): tmpfs's regular files
    /// and the backend's (`fuse_open`).
    pub fn takes_direct_io(&self) -> bool {
        match self.content {
            Content::Regular(_) => self.fs.kind == Kind::Tmpfs,
            Content::Backend(_) => self.file_type() == S_IFREG,
            _ => false,
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
        // `acl_permission_check`: another than the owner checks the access list when the group
        // has bits, which a backend without FUSE_POSIX_ACL gives as none and the inode holds.
        if uid != self.uid
            && mode & 0o070 != 0
            && let Some(remote) = backend::remote_of(self)
        {
            remote.acls.borrow_mut()[0].get_or_insert_with(Vec::new);
        }
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

/// Linux's `inode_permission` for a process `caller`, with the process information tree's own rules: a
/// process directory may not be written (EPERM, measured on the reference), a process may always use
/// its own descriptor directory, and `/proc/sys` may not be written even by root.
pub(crate) fn permission(
    inode: &Inode,
    credentials: &Credentials,
    mask: u32,
    caller: crate::process::Pid,
) -> Result<(), Errno> {
    if let Content::Proc(entry) = inode.content {
        match entry {
            proc::Entry::Process(_) if mask & W_OK != 0 => return Err(Errno::EPERM),
            proc::Entry::Fds(task) if task.pid == caller => return Ok(()),
            // `proc_sys_permission`: the root of `/proc/sys` may not be written and is otherwise open
            // to all; below it no file runs, and the table's bits decide with no power over them
            // (`test_perm`, by the effective ids).
            proc::Entry::Global(index) if proc::is_sysctl_root(index) => {
                return if mask & W_OK != 0 {
                    Err(Errno::EACCES)
                } else {
                    Ok(())
                };
            }
            proc::Entry::Global(index) if proc::GLOBAL[index].0.starts_with("sys/") => {
                if mask & X_OK != 0 && inode.file_type() == S_IFREG {
                    return Err(Errno::EACCES);
                }
                let bits = if credentials.euid == 0 {
                    inode.mode >> 6
                } else if credentials.in_group(0, false) {
                    inode.mode >> 3
                } else {
                    inode.mode
                };
                return if mask & !bits & 7 == 0 {
                    Ok(())
                } else {
                    Err(Errno::EACCES)
                };
            }
            _ => {}
        }
    }
    if inode.permits(credentials, mask, true) {
        Ok(())
    } else {
        Err(Errno::EACCES)
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
    pub fn file_system(&mut self, kind: Kind, flags: u32) -> FileSystem {
        let minor = self.next_device_minor;
        self.next_device_minor += 1;
        FileSystem {
            kind,
            device: encode_device(0, minor),
            flags,
            space: None,
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

/// Makes an inode, taking its space from tmpfs (`shmem_reserve_inode`): `ENOSPC` when tmpfs has
/// room for no more inodes, or a long link target finds no page.
pub(crate) fn new_inode(fs: FileSystem, ino: u64, new: New) -> Result<Node, Errno> {
    let nlink = if new.mode & S_IFMT == S_IFDIR { 2 } else { 1 };
    let mut content = new.content;
    if let Some(space) = &fs.space {
        space.take_ispace(INODE_SPACE)?;
        if let Content::Regular(regular) = &mut content {
            regular.count_against(&fs.space);
        }
    }
    let long_target =
        matches!(&content, Content::Symlink(target) if target.len() + 1 > SHORT_SYMLINK);
    if fs.kind == Kind::Tmpfs
        && long_target
        && let Some(space) = &fs.space
    {
        if space.free_blocks() == 0 {
            space.give_ispace(INODE_SPACE);
            return Err(Errno::ENOSPC);
        }
        space.take_blocks(1);
    }
    Ok(Rc::new(RefCell::new(Inode {
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
            mount_root: false,
            held_parent: None,
        },
        content,
        flocks: Vec::new(),
        record_locks: Vec::new(),
        xattrs: BTreeMap::new(),
        fsflags: 0,
        write_hint: 0,
        watches: Vec::new(),
    })))
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
    /// The directory the last part is looked up in: `directory` itself, except for a last part of
    /// `..`, where Linux's `filename_parentat` stops at the directory below it (and its mount).
    pub base: Node,
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
            let (target, remote_link) = match &node.borrow().content {
                Content::Symlink(target) => (Some(target.clone()), false),
                Content::Backend(_) => (None, node.borrow().file_type() == S_IFLNK),
                _ => (None, false),
            };
            let target = if remote_link {
                Some(backend::readlink(&node)?)
            } else {
                target
            };
            if let Some(target) = target {
                self.may_follow(&parent.directory, &node.borrow())?;
                links += 1;
                if links > MAX_SYMLINKS {
                    return Err(Errno::ELOOP);
                }
                at = parent.directory;
                path = target;
                continue;
            }
            let magic = match &node.borrow().content {
                Content::Proc(entry) => proc::magic_path(self.proc, self, entry),
                _ => None,
            };
            let node = self.follow(&parent.directory, node, &mut links)?;
            if parent.trailing_slash && !node.borrow().is_directory() {
                return Err(Errno::ENOTDIR);
            }
            if let Some(path) = magic {
                return Ok((node, path));
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
                base: at.clone(),
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
        if *last == b"." || *last == b".." {
            let directory = self.step(&at, last)?;
            return Ok(Parent {
                directory,
                base: at,
                name: Vec::new(),
                trailing_slash,
            });
        }
        self.check_search(&at)?;
        Ok(Parent {
            base: at.clone(),
            directory: at,
            name: last.to_vec(),
            trailing_slash,
        })
    }

    fn check_search(&self, directory: &Node) -> Result<(), Errno> {
        backend::fresh(directory)?;
        let directory = directory.borrow();
        if !directory.is_directory() {
            return Err(Errno::ENOTDIR);
        }
        permission(&directory, self.credentials, X_OK, self.proc.caller)
    }

    /// One step from a directory to a name in it, crossing mounts.
    pub fn step(&self, directory: &Node, name: &[u8]) -> Result<Node, Errno> {
        // A removed directory holds nothing, and Linux looks nothing up in it (`IS_DEADDIR`) before the
        // file system sees the name.
        {
            let inode = directory.borrow();
            if (matches!(inode.content, Content::Directory(_))
                || matches!(inode.content, Content::Backend(_)) && inode.is_directory())
                && inode.nlink == 0
                && name != b"."
                && name != b".."
            {
                return Err(Errno::ENOENT);
            }
        }
        // The length limit is each file system's own; the process information tree has none, so a
        // long name there is simply not found.
        if name.len() > NAME_MAX as usize && !matches!(directory.borrow().content, Content::Proc(_))
        {
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
                Content::Backend(_) if inode.is_directory() => {
                    drop(inode);
                    backend::lookup(directory, name)?
                }
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

    /// Linux's `may_follow_link` with `fs.protected_symlinks` on (as in the reference): in a sticky
    /// directory anyone may write to, only links owned by the follower or by the directory's owner are
    /// followed.
    fn may_follow(&self, directory: &Node, link: &Inode) -> Result<(), Errno> {
        if self.credentials.fsuid == link.uid {
            return Ok(());
        }
        let directory = directory.borrow();
        if directory.mode & (S_ISVTX | S_IWOTH) != S_ISVTX | S_IWOTH || directory.uid == link.uid {
            return Ok(());
        }
        Err(Errno::EACCES)
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
                Content::Symlink(target) => {
                    self.may_follow(directory, &inode)?;
                    target.clone()
                }
                Content::Backend(_) => {
                    self.may_follow(directory, &inode)?;
                    drop(inode);
                    backend::readlink(&node)?
                }
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

    /// A directory's path as Linux's `d_path` gives it: from the walk's root, or from the top when the
    /// root is not above it, with " (deleted)" once it has been removed. None for other files.
    pub fn path_of(&self, node: &Node) -> Option<Vec<u8>> {
        let (mut path, _) = self.ascend(node, true).ok()?;
        if node.borrow().nlink == 0 {
            path.extend_from_slice(b" (deleted)");
        }
        Some(path)
    }

    /// The path of a directory as `getcwd` gives it: from the walk's root, or, for one outside it,
    /// from the top marked "(unreachable)". ENOENT once it has been removed.
    pub fn cwd_path(&self, node: &Node) -> Result<Vec<u8>, Errno> {
        let (path, reachable) = self.ascend(node, false)?;
        if reachable {
            return Ok(path);
        }
        let mut marked = UNREACHABLE.to_vec();
        marked.extend_from_slice(&path);
        Ok(marked)
    }

    /// A directory's path from the walk's root, or from the top when the root is not above it (then
    /// false). Removed directories are ENOENT unless `removed` allows them.
    fn ascend(&self, node: &Node, removed: bool) -> Result<(Vec<u8>, bool), Errno> {
        let mut parts: Vec<Vec<u8>> = Vec::new();
        let mut at = node.clone();
        let mut reachable = true;
        loop {
            if Rc::ptr_eq(&at, self.root) {
                break;
            }
            let up = {
                let inode = at.borrow();
                if !inode.is_directory() || (inode.nlink == 0 && !removed) {
                    return Err(Errno::ENOENT);
                }
                // In the process information tree the inodes above may be gone: its directories are
                // found again from the tree's root, mounted at `/proc`.
                if let Content::Proc(entry) = inode.content
                    && !matches!(entry, proc::Entry::Root)
                    && let Some(below) = entry.directory_path()
                    && let Ok(proc_root) = self.step(self.root, b"proc")
                    && matches!(proc_root.borrow().content, Content::Proc(proc::Entry::Root))
                {
                    let mut path = b"/proc".to_vec();
                    path.extend_from_slice(below.as_bytes());
                    for part in parts.iter().rev() {
                        path.push(b'/');
                        path.extend_from_slice(part);
                    }
                    return Ok((path, true));
                }
                match inode
                    .place
                    .parent
                    .as_ref()
                    .and_then(|parent| parent.upgrade())
                {
                    Some(up) => {
                        parts.push(inode.place.name.clone());
                        up
                    }
                    None => {
                        reachable = false;
                        break;
                    }
                }
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
        Ok((path, reachable))
    }
}

/// What `getcwd` puts before a directory outside the caller's root.
const UNREACHABLE: &[u8] = b"(unreachable)";

/// Whether two nodes are the same inode. Inodes of the process information tree are made at each
/// lookup, so those are the same when they are the same entry.
pub(crate) fn same_inode(a: &Node, b: &Node) -> bool {
    if Rc::ptr_eq(a, b) {
        return true;
    }
    match (&a.borrow().content, &b.borrow().content) {
        (Content::Proc(first), Content::Proc(second)) => first == second,
        _ => false,
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
        inode.place.mount_root = true;
    }
    on.borrow_mut().place.mounted = Some(root.clone());
}
