//! Open files and file descriptor tables.

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::Errno;
use crate::fs::Node;
use crate::linux::general::*;

/// One open file description: what `open` makes, shared by `dup` and kept across `exec`.
pub(crate) struct OpenFile {
    pub node: Node,
    /// The access mode and status flags (`O_APPEND`, `O_NONBLOCK`, ...), as `F_GETFL` reports them.
    pub flags: u32,
    pub offset: u64,
    /// The path it was opened by, from the root, for `/proc/<pid>/fd`.
    pub path: Vec<u8>,
    /// For files of the process information tree, the bytes last made, and where the last read of
    /// them stopped (`seq_file`'s `read_pos`).
    pub contents: Option<Vec<u8>>,
    pub read_position: u64,
    /// The serial of the credentials it was opened with (Linux's `f_cred`).
    pub opener: u64,
    /// Who its I/O signals go to and which signal, once set (Linux's `f_owner`).
    pub owner: Option<Owner>,
    /// The open that made it also made the file (Linux's `FMODE_CREATED`).
    pub created: bool,
    /// For a terminal, the side it opened.
    pub terminal: Option<TerminalEnd>,
    /// While `FASYNC` is set through a file that sends I/O signals, the descriptor it was set
    /// through, which the signals carry (Linux's `fa_fd`).
    pub async_fd: Option<i32>,
    /// Whether `O_DIRECT` may be set on it (`FMODE_CAN_ODIRECT`): an open tmpfs file, but not the file
    /// `memfd_create` makes, which is not opened through tmpfs's `open`.
    pub direct_io: bool,
    /// What a file on the shared anonymous inode is (Linux's `private_data` of such files).
    pub anon: Option<Anon>,
}

/// The state of a file on the shared anonymous inode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Anon {
    /// A signal descriptor and the signals it reads.
    Signals(u64),
}

/// The terminal side an open file reads and writes, and whether a hangup cut the file off.
pub(crate) struct TerminalEnd {
    pub pty: Rc<RefCell<crate::tty::Pty>>,
    pub side: usize,
    pub hung_up: bool,
}

/// Linux's `fown_struct`: a process number of a kind (none is 0), the signal (0 is `SIGIO`), and the
/// user and effective user of whoever named the owner, which decide whom the signal may reach.
#[derive(Clone, Copy, Default)]
pub(crate) struct Owner {
    pub pid: i32,
    pub kind: OwnerKind,
    pub signal: i32,
    pub uid: u32,
    pub euid: u32,
}

/// What an owner's number names (Linux's `PIDTYPE_PID`, `PIDTYPE_TGID`, `PIDTYPE_PGID`).
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum OwnerKind {
    #[default]
    Thread,
    Process,
    Group,
}

pub(crate) type FileRef = Rc<RefCell<OpenFile>>;

impl OpenFile {
    pub fn new(node: Node, flags: u32, path: Vec<u8>, opener: u64) -> FileRef {
        let direct_io = node.borrow().takes_direct_io();
        Rc::new(RefCell::new(Self {
            node,
            flags,
            offset: 0,
            path,
            contents: None,
            read_position: 0,
            opener,
            owner: None,
            created: false,
            terminal: None,
            async_fd: None,
            direct_io,
            anon: None,
        }))
    }

    pub fn readable(&self) -> bool {
        self.flags & O_PATH == 0 && matches!(self.flags & O_ACCMODE, O_RDONLY | O_RDWR)
    }

    pub fn writable(&self) -> bool {
        self.flags & O_PATH == 0 && matches!(self.flags & O_ACCMODE, O_WRONLY | O_RDWR)
    }

    pub fn is_path(&self) -> bool {
        self.flags & O_PATH != 0
    }
}

#[derive(Clone)]
pub(crate) struct Descriptor {
    pub file: FileRef,
    pub close_on_exec: bool,
}

#[derive(Clone, Default)]
pub(crate) struct FdTable {
    slots: BTreeMap<i32, Descriptor>,
    /// The highest descriptor ever used, which sets how large Linux's table has grown.
    highest: i32,
    /// Who the record locks taken through this table belong to (Linux's `files_struct` as a lock's
    /// owner).
    lock_owner: u64,
    /// The files whose descriptors were closed, in order, for the kernel to finish releasing.
    released: Vec<FileRef>,
}

impl FdTable {
    /// An empty table whose record locks belong to `lock_owner`.
    pub fn new(lock_owner: u64) -> Self {
        Self {
            lock_owner,
            ..Self::default()
        }
    }

    pub fn get(&self, fd: i32) -> Result<&Descriptor, Errno> {
        self.slots.get(&fd).ok_or(Errno::EBADF)
    }

    pub fn get_mut(&mut self, fd: i32) -> Result<&mut Descriptor, Errno> {
        self.slots.get_mut(&fd).ok_or(Errno::EBADF)
    }

    /// The open file behind a descriptor.
    pub fn file(&self, fd: i32) -> Result<FileRef, Errno> {
        Ok(self.get(fd)?.file.clone())
    }

    /// How many descriptors the table has room for, as Linux's `max_fds`: 64 at first, then the
    /// smallest power of two above the highest descriptor used. It never shrinks.
    pub fn capacity(&self) -> u32 {
        (self.highest as u32 + 1).next_power_of_two().max(64)
    }

    /// The lowest free descriptor at or above `minimum`, below `limit`.
    pub fn lowest_free(&self, minimum: i32, limit: u64) -> Result<i32, Errno> {
        let mut fd = minimum;
        for (&used, _) in self.slots.range(minimum..) {
            if used != fd {
                break;
            }
            fd += 1;
        }
        if fd as u64 >= limit {
            return Err(Errno::EMFILE);
        }
        Ok(fd)
    }

    /// Puts a file at the lowest free descriptor at or above `minimum`.
    pub fn add(
        &mut self,
        file: FileRef,
        close_on_exec: bool,
        minimum: i32,
        limit: u64,
    ) -> Result<i32, Errno> {
        let fd = self.lowest_free(minimum, limit)?;
        self.highest = self.highest.max(fd);
        self.slots.insert(
            fd,
            Descriptor {
                file,
                close_on_exec,
            },
        );
        Ok(fd)
    }

    /// Puts a file at a given descriptor, closing what was there; whether something was.
    pub fn set(&mut self, fd: i32, descriptor: Descriptor) -> bool {
        self.highest = self.highest.max(fd);
        let old = self.slots.insert(fd, descriptor);
        old.map(|old| self.closed(old)).is_some()
    }

    pub fn remove(&mut self, fd: i32) -> Result<(), Errno> {
        let old = self.slots.remove(&fd).ok_or(Errno::EBADF)?;
        self.closed(old);
        Ok(())
    }

    /// Gives up a file this table's process held outside a descriptor.
    pub fn release(&mut self, file: FileRef) {
        self.released.push(file);
    }

    /// The files given up since the last call, in order.
    pub fn take_released(&mut self) -> Vec<FileRef> {
        core::mem::take(&mut self.released)
    }

    pub fn lock_owner(&self) -> u64 {
        self.lock_owner
    }

    /// Closes every descriptor, as a process's end does.
    /// A copy for a child process: the same open files, its own record locks (`dup_fd`).
    pub fn duplicate(&self, lock_owner: u64) -> Self {
        Self {
            lock_owner,
            released: Vec::new(),
            ..self.clone()
        }
    }

    pub fn close_all(&mut self) {
        for (_, old) in core::mem::take(&mut self.slots) {
            self.closed(old);
        }
    }

    /// A descriptor was closed: the table's record locks on the file go (`filp_flush`), unless it
    /// was opened only for its path.
    fn closed(&mut self, descriptor: Descriptor) {
        {
            let file = descriptor.file.borrow();
            if !file.is_path() {
                let mut node = file.node.borrow_mut();
                crate::locks::release_table(&mut node.record_locks, self.lock_owner);
                // A queue's request to be told goes with a close by its process, whose number the
                // table's lock owner is (`mqueue_flush_file`).
                if let crate::fs::Content::Mqueue(queue) = &mut node.content {
                    queue.flush(self.lock_owner as crate::process::Pid);
                }
            }
        }
        self.release(descriptor.file);
    }

    pub fn iter(&self) -> impl Iterator<Item = (i32, &Descriptor)> {
        self.slots.iter().map(|(fd, descriptor)| (*fd, descriptor))
    }

    /// Closes every descriptor marked close-on-exec, as `execve` does.
    pub fn close_on_exec(&mut self) {
        let closing: Vec<i32> = self
            .slots
            .iter()
            .filter(|(_, descriptor)| descriptor.close_on_exec)
            .map(|(fd, _)| *fd)
            .collect();
        for fd in closing {
            let _ = self.remove(fd);
        }
    }
}
