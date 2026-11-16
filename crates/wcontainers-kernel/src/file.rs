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
}

pub(crate) type FileRef = Rc<RefCell<OpenFile>>;

impl OpenFile {
    pub fn new(node: Node, flags: u32, path: Vec<u8>, opener: u64) -> FileRef {
        Rc::new(RefCell::new(Self {
            node,
            flags,
            offset: 0,
            path,
            contents: None,
            read_position: 0,
            opener,
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
}

impl FdTable {
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

    /// Puts a file at a given descriptor, closing what was there.
    pub fn set(&mut self, fd: i32, descriptor: Descriptor) -> Option<Descriptor> {
        self.highest = self.highest.max(fd);
        self.slots.insert(fd, descriptor)
    }

    pub fn remove(&mut self, fd: i32) -> Result<Descriptor, Errno> {
        self.slots.remove(&fd).ok_or(Errno::EBADF)
    }

    pub fn iter(&self) -> impl Iterator<Item = (i32, &Descriptor)> {
        self.slots.iter().map(|(fd, descriptor)| (*fd, descriptor))
    }

    /// Closes every descriptor marked close-on-exec, as `execve` does.
    pub fn close_on_exec(&mut self) {
        self.slots.retain(|_, descriptor| !descriptor.close_on_exec);
    }
}
