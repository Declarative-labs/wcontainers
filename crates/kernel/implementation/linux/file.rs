//! Open files and file descriptor tables.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use atomic_refcell::AtomicRefCell;

use crate::Errno;
use crate::fs::Node;
use crate::linux::general::*;
use crate::process::Pid;

/// One open file description: what `open` makes, shared by `dup` and kept across `exec`.
pub(crate) struct OpenFile {
    /// Its number from `Kernel::next_object`, which waits and epoll know it by.
    pub id: u64,
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
    /// Whether it was opened through the kernel's own mount of its file system (mq_open's
    /// `mq_mnt`), which has no mount flags, rather than through a mount in the file tree.
    pub internal_mount: bool,
    /// For a file on a backend, its handle there.
    pub backend: Option<Arc<crate::fs::backend::Handle>>,
    /// The thread whose call took its position last (`io::lock_position`).
    pub position_holder: Option<crate::process::Tid>,
    /// The directory and name it was opened by, for its directory's inotify watches.
    pub dentry: Option<(alloc::sync::Weak<AtomicRefCell<crate::fs::Inode>>, Vec<u8>)>,
    /// For `/proc/<pid>/mem`, the memory it was opened on (Linux's `private_data` mm).
    pub memory: Option<wcontainers_abi::MemoryId>,
    /// Made without an open (`alloc_file_pseudo`, memory files), so not counted as a writer of
    /// its inode (`i_writecount`).
    pub pseudo: bool,
}

/// The state of a file on the shared anonymous inode.
#[derive(Clone)]
pub(crate) enum Anon {
    /// A signal descriptor and the signals it reads.
    Signals(u64),
    /// A timer descriptor and its timer.
    Timer(u64),
    /// An event counter (`eventfd`).
    Event(Arc<AtomicRefCell<crate::syscall::eventfd::Counter>>),
    /// An epoll instance.
    Epoll(crate::syscall::epoll::EpollRef),
    /// An inotify instance.
    Inotify(Arc<AtomicRefCell<crate::syscall::inotify::Instance>>),
}

/// The terminal side an open file reads and writes, and whether a hangup cut the file off.
pub(crate) struct TerminalEnd {
    pub pty: Arc<AtomicRefCell<crate::tty::Pty>>,
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

pub(crate) type FileRef = Arc<AtomicRefCell<OpenFile>>;

impl OpenFile {
    pub(crate) fn new(id: u64, node: Node, flags: u32, path: Vec<u8>, opener: u64) -> FileRef {
        let direct_io = node.borrow().takes_direct_io();
        Arc::new(AtomicRefCell::new(Self {
            id,
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
            internal_mount: false,
            backend: None,
            position_holder: None,
            dentry: None,
            memory: None,
            pseudo: false,
        }))
    }

    /// Whether it was made by `O_TMPFILE`, whose dentry never takes a name (`d_tmpfile`): a link
    /// given to its inode later names another.
    pub(crate) fn unnamed(&self) -> bool {
        self.flags & (O_TMPFILE & !O_DIRECTORY) != 0
    }

    /// Whether its path is shown as deleted (`d_path` of an unlinked dentry): its inode has no
    /// name left, or the name it was opened by was taken from it after the open.
    pub(crate) fn shown_deleted(&self) -> bool {
        let inode = self.node.borrow();
        inode.shown_deleted()
            || self
                .dentry
                .as_ref()
                .is_some_and(|(directory, name)| inode.name_removed_since(directory, name, self.id))
    }

    pub(crate) fn readable(&self) -> bool {
        self.flags & O_PATH == 0 && matches!(self.flags & O_ACCMODE, O_RDONLY | O_RDWR)
    }

    pub(crate) fn writable(&self) -> bool {
        self.flags & O_PATH == 0 && matches!(self.flags & O_ACCMODE, O_WRONLY | O_RDWR)
    }

    pub(crate) fn is_path(&self) -> bool {
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
    /// Every descriptor below it is in use (Linux's `next_fd`), where a search for a free one starts.
    next_fd: i32,
    /// Who the record locks taken through this table belong to (Linux's `files_struct` as a lock's
    /// owner).
    lock_owner: u64,
    /// The files whose descriptors were closed, in order, for the kernel to finish releasing.
    released: Vec<FileRef>,
    /// Whether a close let go of record locks, whose waiters try again.
    let_go_locks: bool,
    /// Notices that closes took message queue requests away, for the kernel to send.
    pub notices: Vec<(crate::syscall::socket::SocketRef, Vec<u8>)>,
}

impl FdTable {
    /// An empty table whose record locks belong to `lock_owner`.
    pub(crate) fn new(lock_owner: u64) -> Self {
        Self {
            lock_owner,
            ..Self::default()
        }
    }

    pub(crate) fn get(&self, fd: i32) -> Result<&Descriptor, Errno> {
        self.slots.get(&fd).ok_or(Errno::EBADF)
    }

    pub(crate) fn get_mut(&mut self, fd: i32) -> Result<&mut Descriptor, Errno> {
        self.slots.get_mut(&fd).ok_or(Errno::EBADF)
    }

    /// The open file behind a descriptor.
    pub(crate) fn file(&self, fd: i32) -> Result<FileRef, Errno> {
        Ok(self.get(fd)?.file.clone())
    }

    /// How many descriptors the table has room for, as Linux's `max_fds`: 64 at first, then the
    /// smallest power of two above the highest descriptor used. It never shrinks.
    pub(crate) fn capacity(&self) -> u32 {
        (self.highest as u32 + 1).next_power_of_two().max(64)
    }

    /// The lowest free descriptor at or above `minimum`, below `limit`.
    pub(crate) fn lowest_free(&self, minimum: i32, limit: u64) -> Result<i32, Errno> {
        let mut fd = minimum.max(self.next_fd);
        for (&used, _) in self.slots.range(fd..) {
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
    pub(crate) fn add(
        &mut self,
        file: FileRef,
        close_on_exec: bool,
        minimum: i32,
        limit: u64,
    ) -> Result<i32, Errno> {
        let fd = self.lowest_free(minimum, limit)?;
        self.highest = self.highest.max(fd);
        if minimum <= self.next_fd {
            self.next_fd = fd + 1;
        }
        self.slots.insert(
            fd,
            Descriptor {
                file,
                close_on_exec,
            },
        );
        Ok(fd)
    }

    /// Puts a file at a given descriptor, closing what was there for process `closer`; whether
    /// something was.
    pub(crate) fn set(&mut self, fd: i32, descriptor: Descriptor, closer: Pid) -> bool {
        self.highest = self.highest.max(fd);
        let old = self.slots.insert(fd, descriptor);
        old.map(|old| self.closed(old, closer, false)).is_some()
    }

    /// Closes a descriptor for process `closer`; `flushed` when the caller has already flushed a
    /// backend file and had its answer.
    pub(crate) fn remove(&mut self, fd: i32, closer: Pid, flushed: bool) -> Result<(), Errno> {
        let old = self.slots.remove(&fd).ok_or(Errno::EBADF)?;
        self.next_fd = self.next_fd.min(fd);
        self.closed(old, closer, flushed);
        Ok(())
    }

    /// Gives up a file this table's process held outside a descriptor.
    pub(crate) fn release(&mut self, file: FileRef) {
        self.released.push(file);
    }

    /// The files given up since the last call, in order.
    pub(crate) fn take_released(&mut self) -> Vec<FileRef> {
        core::mem::take(&mut self.released)
    }

    /// Whether record locks were let go since the last call.
    pub(crate) fn take_let_go_locks(&mut self) -> bool {
        core::mem::take(&mut self.let_go_locks)
    }

    pub(crate) fn lock_owner(&self) -> u64 {
        self.lock_owner
    }

    /// A copy for a child process: the same open files, its own record locks (`dup_fd`).
    pub(crate) fn duplicate(&self, lock_owner: u64) -> Self {
        Self {
            lock_owner,
            released: Vec::new(),
            let_go_locks: false,
            notices: Vec::new(),
            ..self.clone()
        }
    }

    /// Closes every descriptor for process `closer`, as the end of a table's last holder does.
    pub(crate) fn close_all(&mut self, closer: Pid) {
        self.next_fd = 0;
        for (_, old) in core::mem::take(&mut self.slots) {
            self.closed(old, closer, false);
        }
    }

    /// A descriptor was closed by process `closer`: a backend file is flushed there unless that
    /// was done (`filp_flush`; nothing waits for the answer, whose error only `close` reports),
    /// and the table's record locks on the file go, unless it was opened only for its path.
    fn closed(&mut self, descriptor: Descriptor, closer: Pid, flushed: bool) {
        {
            let file = descriptor.file.borrow();
            if let Some(handle) = &file.backend
                && !flushed
            {
                crate::fs::backend::flush_unwaited(handle, self.lock_owner);
            }
            if !file.is_path() {
                crate::syscall::dnotify::flush(&file.node, self.lock_owner, &descriptor.file);
                let mut node = file.node.borrow_mut();
                self.let_go_locks |=
                    crate::locks::release_table(&mut node.record_locks, self.lock_owner);
                // A queue's request to be told goes with a close by its process
                // (`mqueue_flush_file`).
                if let crate::fs::Content::Mqueue(queue) = &mut node.content
                    && let Some(notice) = queue.flush(closer)
                {
                    self.notices.push(notice);
                }
            }
        }
        self.release(descriptor.file);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (i32, &Descriptor)> {
        self.slots.iter().map(|(fd, descriptor)| (*fd, descriptor))
    }

    /// Closes every descriptor marked close-on-exec, as `execve` does in process `closer`.
    pub(crate) fn close_on_exec(&mut self, closer: Pid) {
        let closing: Vec<i32> = self
            .slots
            .iter()
            .filter(|(_, descriptor)| descriptor.close_on_exec)
            .map(|(fd, _)| *fd)
            .collect();
        for fd in closing {
            let _ = self.remove(fd, closer, false);
        }
    }
}
