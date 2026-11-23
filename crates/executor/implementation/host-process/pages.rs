//! Each guest thread's two pages shared with the embedder: its control page, which the stub only reads and obeys,
//! and its register page, which the stub writes and the embedder only trusts as far as it checks.

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicU32, AtomicU64};

use rustix::fs::MemfdFlags;
use rustix::io::Errno;
use rustix::mm::{MapFlags, ProtFlags};

use crate::layout::{
    REGISTER_PAGES_OFFSET, THREAD_PAGE_SIZE, THREAD_PAGES_SIZE, THREADS_PER_PROCESS,
};

/// Byte offsets in a control page.
pub mod control {
    use crate::layout::THREAD_PAGE_SIZE;

    /// The turn the embedder last gave the thread, the futex word the thread waits on.
    pub const ANSWER: usize = 0;
    /// What the thread does once its commands are applied: a [`super::Resume`].
    pub const RESUME: usize = 8;
    /// How many commands the turn carries.
    pub const COMMAND_COUNT: usize = 16;
    /// The first command; each is [`COMMAND_SIZE`] bytes.
    pub const COMMANDS: usize = 64;
    /// A command's bytes: its kind with its permissions above bit 32, an address, a length and a file offset.
    pub const COMMAND_SIZE: usize = 32;
    /// The commands one page holds.
    pub const MAX_COMMANDS: usize = (THREAD_PAGE_SIZE - COMMANDS) / COMMAND_SIZE;
    /// A stub's start is thread 0's turn [`START`]: its page then holds the filter's instruction count here and its
    /// instructions at `COMMANDS`, and the stub reports `Done` once the filter is installed.
    pub const FILTER_LENGTH: usize = COMMAND_COUNT;
    /// The turn that starts a stub.
    pub const START: u32 = 1;
}

/// Byte offsets in a register page.
pub mod register {
    /// The turn the thread last ended, the futex word the embedder waits on.
    pub const REQUEST: usize = 0;
    /// The thread's host id, as the stub reports it.
    pub const THREAD_ID: usize = 4;
    /// The word the host kernel zeroes, and wakes, once the thread has ended.
    pub const EXIT_CLEAR: usize = 8;
    /// Why the turn ended: an [`super::ExitKind`].
    pub const EXIT: usize = 16;
    /// What the exit reports.
    pub const EXIT_VALUE: usize = 24;
    /// The stub's own count of the stops that landed in a turn after its commands (ending it), as four words: then
    /// those that landed while it published an exit, waited for a turn, or ended.
    pub const STOPS_IN_TURN: usize = 32;
    pub const STOPS_IN_PUBLISH: usize = 40;
    pub const STOPS_IN_WAIT: usize = 48;
    pub const STOPS_IN_END: usize = 56;
}

/// What a turn does after its commands.
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resume {
    /// Reports back at once, with the count of commands applied.
    Ping = 1,
    /// Ends the thread (the process, for its last thread).
    ExitThread = 2,
    /// Runs our own code until a stop ends the turn, standing in for guest code.
    Spin = 3,
}

/// A change to a stub's view of its memory, applied by the thread taking the turn before anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Maps the memory's memfd shared and fixed at `address`, from `offset` (the address itself, but for tests).
    Map {
        /// The first byte.
        address: usize,
        /// The bytes mapped.
        length: usize,
        /// Their permissions.
        permissions: ProtFlags,
        /// The memfd's offset.
        offset: usize,
    },
    /// Changes the permissions of a range.
    Protect {
        /// The first byte.
        address: usize,
        /// The bytes changed.
        length: usize,
        /// Their new permissions.
        permissions: ProtFlags,
    },
    /// Unmaps a range.
    Unmap {
        /// The first byte.
        address: usize,
        /// The bytes unmapped.
        length: usize,
    },
    /// Makes a host thread for a free index, which then waits for turns after `last_turn` (from
    /// [`crate::stub::Stub::new_thread`]).
    NewThread {
        /// The new thread's index.
        thread: usize,
        /// The value its answer word holds now.
        last_turn: u32,
    },
}

/// The kinds of command, as the stub reads them.
#[repr(u32)]
pub(crate) enum CommandKind {
    Map = 1,
    Protect = 2,
    Unmap = 3,
    NewThread = 4,
}

impl Command {
    /// Its four words in a control page.
    pub(crate) fn words(self) -> [u64; 4] {
        let with = |kind: CommandKind, permissions: ProtFlags| {
            u64::from(kind as u32) | (u64::from(permissions.bits()) << 32)
        };
        match self {
            Self::Map {
                address,
                length,
                permissions,
                offset,
            } => [
                with(CommandKind::Map, permissions),
                address as u64,
                length as u64,
                offset as u64,
            ],
            Self::Protect {
                address,
                length,
                permissions,
            } => [
                with(CommandKind::Protect, permissions),
                address as u64,
                length as u64,
                0,
            ],
            Self::Unmap { address, length } => [
                with(CommandKind::Unmap, ProtFlags::empty()),
                address as u64,
                length as u64,
                0,
            ],
            Self::NewThread { thread, last_turn } => [
                with(CommandKind::NewThread, ProtFlags::empty()),
                thread as u64,
                u64::from(last_turn),
                0,
            ],
        }
    }
}

/// Why a turn ended, as the stub writes it.
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    /// The resume was done; the value is the count of commands applied.
    Done = 1,
    /// A command's host call failed; the value is the command's index above bit 32 and the error number below.
    CommandRefused = 2,
    /// The control page named no resume the stub knows, or more commands than a page holds.
    BadTurn = 3,
    /// The stop signal ended the turn after its commands; the value is the count of commands applied.
    Stopped = 4,
}

impl ExitKind {
    pub(crate) fn from_word(word: u64) -> Option<Self> {
        [
            Self::Done,
            Self::CommandRefused,
            Self::BadTurn,
            Self::Stopped,
        ]
        .into_iter()
        .find(|kind| *kind as u64 == word)
    }
}

/// The embedder's mapping of one stub's thread pages, written through atomics because the stub runs beside it.
pub(crate) struct ThreadPages {
    file: OwnedFd,
    /// The mapping's address, kept as a number so the value is `Send`; `Drop` unmaps it.
    base: usize,
}

impl ThreadPages {
    pub(crate) fn new() -> Result<Self, Errno> {
        let file = rustix::fs::memfd_create("wcontainers thread pages", MemfdFlags::CLOEXEC)?;
        rustix::fs::ftruncate(&file, THREAD_PAGES_SIZE as u64)?;
        #[expect(
            unsafe_code,
            reason = "maps the thread pages where the host chooses, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: a new mapping at an address the host picks overlaps nothing; `Drop` unmaps it.
        let base = unsafe {
            rustix::mm::mmap(
                core::ptr::null_mut(),
                THREAD_PAGES_SIZE,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED,
                &file,
                0,
            )
        }?;
        Ok(Self {
            file,
            base: base as usize,
        })
    }

    pub(crate) fn file(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }

    /// Every 32-bit word of the mapping.
    fn words32(&self) -> &[AtomicU32] {
        #[expect(
            unsafe_code,
            reason = "views the shared thread pages as atomics, which no safe call can give"
        )]
        // SAFETY: the mapping is page-aligned, readable and writable, lives as long as `self`, and is only ever
        // accessed through atomics here; the stub's own accesses are another process's.
        unsafe {
            core::slice::from_raw_parts(
                self.base as *const AtomicU32,
                THREAD_PAGES_SIZE / size_of::<AtomicU32>(),
            )
        }
    }

    /// Every 64-bit word of the mapping.
    fn words64(&self) -> &[AtomicU64] {
        #[expect(
            unsafe_code,
            reason = "views the shared thread pages as atomics, which no safe call can give"
        )]
        // SAFETY: as `words32`.
        unsafe {
            core::slice::from_raw_parts(
                self.base as *const AtomicU64,
                THREAD_PAGES_SIZE / size_of::<AtomicU64>(),
            )
        }
    }

    fn control_offset(thread: usize, offset: usize) -> usize {
        assert!(thread < THREADS_PER_PROCESS && offset < THREAD_PAGE_SIZE);
        thread * THREAD_PAGE_SIZE + offset
    }

    fn register_offset(thread: usize, offset: usize) -> usize {
        REGISTER_PAGES_OFFSET + Self::control_offset(thread, offset)
    }

    pub(crate) fn control32(&self, thread: usize, offset: usize) -> &AtomicU32 {
        &self.words32()[Self::control_offset(thread, offset) / size_of::<u32>()]
    }

    pub(crate) fn control64(&self, thread: usize, offset: usize) -> &AtomicU64 {
        &self.words64()[Self::control_offset(thread, offset) / size_of::<u64>()]
    }

    pub(crate) fn register32(&self, thread: usize, offset: usize) -> &AtomicU32 {
        &self.words32()[Self::register_offset(thread, offset) / size_of::<u32>()]
    }

    pub(crate) fn register64(&self, thread: usize, offset: usize) -> &AtomicU64 {
        &self.words64()[Self::register_offset(thread, offset) / size_of::<u64>()]
    }
}

impl Drop for ThreadPages {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "unmaps the mapping made by `new`")]
        // SAFETY: the mapping is this value's alone, and every reference into it borrowed `self`.
        let _ =
            unsafe { rustix::mm::munmap(self.base as *mut core::ffi::c_void, THREAD_PAGES_SIZE) };
    }
}
