//! Pipes and named pipes, as Linux's `fs/pipe.c` keeps them: a ring of page-sized slots, small writes
//! merged into the last slot, reads that take what is there, and waits that park until the other end
//! moves.
//! The container's standard output pipes are pipes whose reader is the host, which takes every byte
//! as it is written.

use alloc::collections::VecDeque;
use alloc::rc::Weak;
use alloc::vec::Vec;
use core::cell::RefCell;

use wcontainers_abi::{PAGE_SIZE, Stream};

use crate::file::{FileRef, OpenFile};
use crate::linux::general::*;

/// Slots a new pipe has (`PIPE_DEF_BUFFERS`) and the largest size without CAP_SYS_RESOURCE
/// (`pipe_max_size`).
pub(crate) const DEFAULT_SLOTS: usize = 16;
pub(crate) const MAX_SIZE: u64 = 1 << 20;

/// One page of a pipe: the bytes written into it, and how many of them have been read.
#[derive(Clone)]
struct Slot {
    page: Vec<u8>,
    read: usize,
    /// Written by a packet-mode writer (`O_DIRECT`): never merged, and a read takes it whole.
    packet: bool,
}

#[derive(Clone)]
pub(crate) struct Pipe {
    slots: VecDeque<Slot>,
    /// How many slots it may hold (`max_usage`).
    pub capacity: usize,
    /// The open files of its ends, for counting readers and writers, each with the count of writers
    /// it has seen opened (`f_pipe`), which a named pipe's `POLLHUP` needs.
    ends: Vec<(Weak<RefCell<OpenFile>>, u32)>,
    /// The host reads this pipe, at once: one of the container's standard streams.
    pub host: Option<Stream>,
    /// How many times a reader and a writer has opened it (`r_counter`, `w_counter`).
    pub opened_readers: u32,
    pub opened_writers: u32,
    /// Its readers and writers when last counted, to find the closes of ends since.
    pub counted: (usize, usize),
}

impl Pipe {
    /// An unnamed pipe, made with one reader and one writer.
    pub fn new(host: Option<Stream>) -> Self {
        Self {
            host,
            opened_readers: 1,
            opened_writers: 1,
            ..Self::named()
        }
    }

    /// A named pipe's buffer, made when it is opened with no end open.
    pub fn named() -> Self {
        Self {
            slots: VecDeque::new(),
            capacity: DEFAULT_SLOTS,
            ends: Vec::new(),
            host: None,
            opened_readers: 0,
            opened_writers: 0,
            counted: (0, 0),
        }
    }

    /// Notes an open file of one of its ends, which has seen `seen_writers` writers opened.
    pub fn opened(&mut self, file: &FileRef, seen_writers: u32) {
        self.ends.retain(|(end, _)| end.strong_count() > 0);
        self.ends
            .push((alloc::rc::Rc::downgrade(file), seen_writers));
        self.note_counts();
    }

    /// Counts its readers and writers now.
    pub fn note_counts(&mut self) {
        self.counted = (self.readers(), self.writers());
    }

    /// The open files of its reading (or writing) ends with `FASYNC` set, which I/O signals go to.
    pub fn async_ends(&self, readers: bool) -> Vec<FileRef> {
        self.ends
            .iter()
            .filter_map(|(end, _)| end.upgrade())
            .filter(|end| {
                let end = end.borrow();
                end.flags & FASYNC != 0
                    && if readers {
                        end.readable()
                    } else {
                        end.writable()
                    }
            })
            .collect()
    }

    /// Whether any end is open.
    pub fn is_open(&self) -> bool {
        self.ends.iter().any(|(end, _)| end.strong_count() > 0)
    }

    fn count(&self, wanted: fn(&OpenFile) -> bool) -> usize {
        self.ends
            .iter()
            .filter_map(|(end, _)| end.upgrade())
            .filter(|end| wanted(&end.borrow()))
            .count()
    }

    /// The writers an open file of it has seen opened.
    fn seen_writers(&self, file: &OpenFile) -> u32 {
        self.ends
            .iter()
            .find(|(end, _)| {
                end.upgrade()
                    .is_some_and(|end| core::ptr::eq(end.as_ptr(), file))
            })
            .map_or(0, |(_, seen)| *seen)
    }

    pub fn readers(&self) -> usize {
        if self.host.is_some() {
            return 1;
        }
        self.count(OpenFile::readable)
    }

    pub fn writers(&self) -> usize {
        self.count(OpenFile::writable)
    }

    /// Bytes waiting to be read (`FIONREAD`).
    pub fn unread(&self) -> u64 {
        self.slots
            .iter()
            .map(|slot| (slot.page.len() - slot.read) as u64)
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.slots.len() >= self.capacity
    }

    /// Takes up to `count` bytes, slot by slot; a packet slot is taken whole even if not all of it
    /// fits, and ends the read.
    pub fn take(&mut self, count: usize) -> Vec<u8> {
        let mut out = Vec::new();
        while out.len() < count {
            let Some(slot) = self.slots.front_mut() else {
                break;
            };
            let available = slot.page.len() - slot.read;
            let chunk = available.min(count - out.len());
            out.extend_from_slice(&slot.page[slot.read..slot.read + chunk]);
            slot.read += chunk;
            let packet = slot.packet;
            if slot.read == slot.page.len() || packet {
                self.slots.pop_front();
            }
            if packet {
                break;
            }
        }
        out
    }

    /// How many bytes of a `length`-byte write the merge step puts in the last slot: the part that
    /// is not whole pages, if it fits after what is there (`pipe_write`'s first step).
    pub fn mergeable(&self, length: usize, packet: bool) -> usize {
        let chars = length & (PAGE_SIZE as usize - 1);
        match self.slots.back() {
            Some(last)
                if chars > 0
                    && !packet
                    && !last.packet
                    && last.page.len() + chars <= PAGE_SIZE as usize =>
            {
                chars
            }
            _ => 0,
        }
    }

    /// Adds bytes to the last slot (the merge step).
    pub fn merge(&mut self, bytes: &[u8]) {
        let last = self.slots.back_mut().expect("a slot to merge into");
        last.page.extend_from_slice(bytes);
    }

    /// Fills one new slot with at most a page.
    pub fn push(&mut self, bytes: Vec<u8>, packet: bool) {
        debug_assert!(bytes.len() <= PAGE_SIZE as usize && !self.is_full());
        self.slots.push_back(Slot {
            page: bytes,
            read: 0,
            packet,
        });
    }

    /// Passes over empty pages at the front, as a read does.
    pub fn drop_empty(&mut self) {
        while self
            .slots
            .front()
            .is_some_and(|slot| slot.read == slot.page.len())
        {
            self.slots.pop_front();
        }
    }

    /// Slots in use.
    pub fn occupancy(&self) -> usize {
        self.slots.len()
    }
}

/// The size `F_SETPIPE_SZ` makes of a request: at least a page, a power of two of pages
/// (`round_pipe_size`); none for a request too large to round.
pub(crate) fn round_size(size: u64) -> Option<u64> {
    if size > 1 << 31 {
        return None;
    }
    Some(size.max(PAGE_SIZE).next_power_of_two())
}

/// What `poll` reports for one end of a pipe (`pipe_poll`).
pub(crate) fn poll(pipe: &Pipe, file: &OpenFile) -> u16 {
    let mut mask = 0;
    if file.readable() {
        if !pipe.is_empty() {
            mask |= POLLIN | POLLRDNORM;
        }
        if pipe.writers() == 0 && pipe.seen_writers(file) != pipe.opened_writers {
            mask |= POLLHUP;
        }
    }
    if file.writable() {
        if !pipe.is_full() {
            mask |= POLLOUT | POLLWRNORM;
        }
        if pipe.readers() == 0 {
            mask |= POLLERR;
        }
    }
    mask as u16
}
