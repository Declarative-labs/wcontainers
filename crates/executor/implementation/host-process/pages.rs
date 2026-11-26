//! Each guest thread's two pages shared with the embedder: its control page, which the stub only reads and obeys,
//! and its register page, which the stub writes and the embedder only trusts as far as it checks.

use core::mem::offset_of;
use std::os::fd::BorrowedFd;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use rustix::io::Errno;
use rustix::mm::ProtFlags;

use crate::layout::{
    CODE_AREA, CODE_AREA_SIZE, REGISTER_PAGES, REGISTER_PAGES_OFFSET, SIGNAL_STACK_SIZE,
    SIGNAL_STACKS, SLOT_TABLE, SLOT_TABLE_SIZE, THREAD_PAGE_SIZE, THREAD_PAGES_SIZE,
    THREADS_PER_PROCESS,
};
use crate::zygote::RECEIVE_BYTE;
use wcontainers_linux_process::LinuxProcess;
use wcontainers_linux_process_direct::{DirectLinuxProcess, DirectSharedWords};

/// Byte offsets in a control page.
pub mod control {
    use crate::layout::THREAD_PAGE_SIZE;

    /// The turn the embedder last gave the thread, the futex word the thread waits on.
    pub const ANSWER: usize = 0;
    /// What the thread does once its commands are applied: a [`super::Resume`].
    pub const RESUME: usize = 8;
    /// How many commands the turn carries.
    pub const COMMAND_COUNT: usize = 16;
    /// The address a [`super::Resume::Load`] or [`super::Resume::Store`] touches.
    pub const TOUCH_ADDRESS: usize = 24;
    /// The word a [`super::Resume::Store`] writes.
    pub const TOUCH_VALUE: usize = 32;
    /// The address in the code area a [`super::Resume::Enter`] branches to.
    pub const ENTER_AT: usize = 40;
    /// The `iovec` a shared block's receive hands the host kernel: one byte at the register page's
    /// [`super::register::RECEIVE`].
    pub const IOVEC: usize = 48;
    /// The first command; each is [`COMMAND_SIZE`] bytes.
    pub const COMMANDS: usize = 64;
    /// A command's bytes: its kind with its permissions above bit 32, an address, a length and a file offset.
    pub const COMMAND_SIZE: usize = 32;
    /// The `stack_t` naming the thread's own signal stack, for `sigaltstack`.
    pub const STACK_RECORD: usize = THREAD_PAGE_SIZE - size_of::<libc::stack_t>();
    /// The CPU set a [`super::Command::Place`] moves the thread to, for `sched_setaffinity`.
    pub const CPU_SET: usize = STACK_RECORD - size_of::<libc::cpu_set_t>();
    /// Where the commands, or the start's filter, must end.
    pub const COMMANDS_END: usize = CPU_SET;
    /// The commands one page holds.
    pub const MAX_COMMANDS: usize = (COMMANDS_END - COMMANDS) / COMMAND_SIZE;

    // The embedder writes these records as 64-bit words.
    const _: () = assert!(
        ENTER_AT + 8 <= IOVEC
            && IOVEC + size_of::<libc::iovec>() <= COMMANDS
            && size_of::<libc::stack_t>() == 3 * size_of::<u64>()
            && core::mem::offset_of!(libc::stack_t, ss_sp) == 0
            && core::mem::offset_of!(libc::stack_t, ss_flags) == 8
            && core::mem::offset_of!(libc::stack_t, ss_size) == 16
            && size_of::<libc::cpu_set_t>().is_multiple_of(size_of::<u64>())
            && COMMANDS + MAX_COMMANDS * COMMAND_SIZE <= CPU_SET
    );
    /// A stub's start is thread 0's turn [`START`]: its page then holds the filter's instruction count here and its
    /// instructions at `COMMANDS`, and the stub reports `Done` once the filter is installed.
    pub const FILTER_LENGTH: usize = COMMAND_COUNT;
    /// The turn that starts a stub.
    pub const START: u32 = 1;

    /// Every 32-bit word of a control page; every other byte is reached only as part of a 64-bit word.
    pub(crate) const WORDS32: [usize; 1] = [ANSWER];
}

/// Byte offsets in a register page.
pub mod register {
    /// Where translated code's own words start, which it finds through x28: the native output's register page.
    pub const TRANSLATED: usize = 0;
    /// The bytes kept for them.
    pub const TRANSLATED_SIZE: usize = 128;
    /// The turn the thread last ended, the futex word the embedder waits on.
    pub const REQUEST: usize = TRANSLATED + TRANSLATED_SIZE;
    /// The thread's host id, as the stub reports it.
    pub const THREAD_ID: usize = REQUEST + 4;
    /// The word the host kernel zeroes, and wakes, once the thread has ended.
    pub const EXIT_CLEAR: usize = REQUEST + 8;
    /// Why the turn ended: an [`super::ExitKind`].
    pub const EXIT: usize = REQUEST + 16;
    /// What the exit reports.
    pub const EXIT_VALUE: usize = REQUEST + 24;
    /// The stub's own count of the stops that landed in a turn after its commands (ending it), as four words: then
    /// those that landed while it published an exit, waited for a turn, or ended.
    pub const STOPS_IN_TURN: usize = REQUEST + 32;
    pub const STOPS_IN_PUBLISH: usize = REQUEST + 40;
    pub const STOPS_IN_WAIT: usize = REQUEST + 48;
    pub const STOPS_IN_END: usize = REQUEST + 56;
    /// More of what the exit reports, where its kind says so.
    pub const EXIT_DETAIL: usize = REQUEST + 64;
    /// The turn the thread last took, written as it takes it (untrusted).
    pub const TAKEN: usize = REQUEST + 72;
    /// The stub's own count of the stops that landed in a floor entry or its save, which then finished from the frame.
    pub const STOPS_IN_CALL: usize = REQUEST + 80;
    /// The stub's own count of the exits from translated code whose FP/SIMD registers it saved after returning through
    /// a signal frame that held none (gVisor), cleared by the embedder as it reads it.
    pub const VECTORS_BY_RETURN: usize = REQUEST + 88;
    /// x23 as a floor entry found it, kept there before the entry names itself in x23.
    pub const CALL_SCRATCH: usize = REQUEST + 96;
    /// The syndrome of a fault in translated code, from the signal frame's `esr_context`; zero where the frame has none.
    pub const EXIT_SYNDROME: usize = REQUEST + 104;
    /// The thread's registers in the hardware, as a [`wcontainers_abi::RegisterBlock`], little endian: what a
    /// [`super::Resume::Enter`] loads and what an exit from translated code saves.
    pub const REGISTERS: usize = 256;
    /// A shared block's receive, which only the stub uses: its `msghdr`, the one byte and the control message, as
    /// the zygote's `block_frame` lays them out.
    pub const RECEIVE: usize =
        (REGISTERS + size_of::<wcontainers_abi::RegisterBlock>()).next_multiple_of(16);

    /// Every 32-bit word of a register page; every other byte is reached only as part of a 64-bit word.
    pub(crate) const WORDS32: [usize; 4] = [REQUEST, THREAD_ID, EXIT_CLEAR, TAKEN];

    const _: () = assert!(EXIT_SYNDROME < REGISTERS);
    const _: () = assert!(RECEIVE + crate::zygote::RECEIVE_SIZE <= crate::layout::THREAD_PAGE_SIZE);
    #[cfg(target_arch = "aarch64")]
    const _: () =
        assert!(size_of::<wcontainers_arm64::thread_state::RegisterPage>() <= TRANSLATED_SIZE);
    const _: () = assert!(
        REGISTERS.is_multiple_of(size_of::<u64>())
            && size_of::<wcontainers_abi::RegisterBlock>().is_multiple_of(size_of::<u64>())
            && REGISTERS + size_of::<wcontainers_abi::RegisterBlock>()
                <= crate::layout::THREAD_PAGE_SIZE
    );
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
    /// Loads the word at the control page's touch address, standing in for a guest's read.
    Load = 4,
    /// Stores the control page's touch value at its touch address, standing in for a guest's write.
    Store = 5,
    /// Loads every register from the register block, x28 the register page's address and x23 the control page's
    /// address in the code area, and branches there.
    Enter = 6,
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
    /// Maps the shared block whose memfd the stub receives first on its socket, shared and fixed at `address`, from
    /// the block's `offset`, then closes it.
    MapShared {
        /// The first byte.
        address: usize,
        /// The bytes mapped.
        length: usize,
        /// Their permissions.
        permissions: ProtFlags,
        /// The block's offset.
        offset: usize,
    },
    /// Maps the code area's memfd, which the stub receives first on its socket, execute-only over the whole code
    /// area, then closes it.
    MapCode,
    /// Maps the code area's memfd past the code area's bytes, which the stub receives first on its socket,
    /// execute-only over the whole slot table, then closes it.
    MapSlots,
    /// Moves the thread applying it to one CPU, as its runner's; a turn carries at most one.
    Place {
        /// The CPU's number.
        cpu: usize,
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
    MapShared = 5,
    Place = 6,
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
            Self::MapShared {
                address,
                length,
                permissions,
                offset,
            } => [
                with(CommandKind::MapShared, permissions),
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
            Self::MapCode => [
                with(CommandKind::MapShared, ProtFlags::EXEC),
                CODE_AREA as u64,
                CODE_AREA_SIZE as u64,
                0,
            ],
            Self::MapSlots => [
                with(CommandKind::MapShared, ProtFlags::EXEC),
                SLOT_TABLE as u64,
                SLOT_TABLE_SIZE as u64,
                CODE_AREA_SIZE as u64,
            ],
            Self::Place { cpu } => [
                with(CommandKind::Place, ProtFlags::empty()),
                cpu as u64,
                0,
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
    /// A touch or translated code faulted; the value is the address the host reported, the detail its signal and,
    /// above bit 32, code. For translated code the register block is what the code held, its pc the faulting one.
    Fault = 5,
    /// A load was done; the value is the word loaded.
    Loaded = 6,
    /// Translated code called a floor entry; the value is the entry's number, the register block what the code held.
    Called = 7,
    /// The stop signal ended translated code; the value is the host pc it stopped at, the register block what the
    /// code held there.
    Interrupted = 8,
}

impl ExitKind {
    pub(crate) fn from_word(word: u64) -> Option<Self> {
        [
            Self::Done,
            Self::CommandRefused,
            Self::BadTurn,
            Self::Stopped,
            Self::Fault,
            Self::Loaded,
            Self::Called,
            Self::Interrupted,
        ]
        .into_iter()
        .find(|kind| *kind as u64 == word)
    }
}

/// The embedder's mapping of one stub's thread pages, written through atomics because the stub runs beside it.
pub(crate) struct ThreadPages {
    words: DirectSharedWords,
}

impl ThreadPages {
    pub(crate) fn new() -> Result<Self, Errno> {
        let words = DirectLinuxProcess
            .create_shared_words("wcontainers thread pages", THREAD_PAGES_SIZE)?;
        Ok(Self { words })
    }

    pub(crate) fn file(&self) -> BorrowedFd<'_> {
        DirectLinuxProcess.shared_words_file(&self.words)
    }

    /// Every 32-bit word of the mapping, each byte only ever reached at one size (`word32`, `word64`).
    fn words32(&self) -> &[AtomicU32] {
        DirectLinuxProcess.shared_words32(&self.words)
    }

    /// Every 64-bit word of the mapping.
    fn words64(&self) -> &[AtomicU64] {
        DirectLinuxProcess.shared_words64(&self.words)
    }

    fn control_offset(thread: usize, offset: usize) -> usize {
        assert!(thread < THREADS_PER_PROCESS && offset < THREAD_PAGE_SIZE);
        thread * THREAD_PAGE_SIZE + offset
    }

    fn register_offset(thread: usize, offset: usize) -> usize {
        REGISTER_PAGES_OFFSET + Self::control_offset(thread, offset)
    }

    pub(crate) fn control32(&self, thread: usize, offset: usize) -> &AtomicU32 {
        let at = Self::control_offset(thread, word32(&control::WORDS32, offset));
        &self.words32()[at / size_of::<u32>()]
    }

    pub(crate) fn control64(&self, thread: usize, offset: usize) -> &AtomicU64 {
        let at = Self::control_offset(thread, word64(&control::WORDS32, offset));
        &self.words64()[at / size_of::<u64>()]
    }

    /// Writes into a thread's control page the records the stub hands the host kernel to read and never builds itself:
    /// its receive's `iovec` and its signal stack's `stack_t`. Only the pages of threads made are written, so a stub's
    /// start touches one control page rather than every index's.
    pub(crate) fn write_records(&self, thread: usize) {
        let byte = REGISTER_PAGES + thread * THREAD_PAGE_SIZE + register::RECEIVE + RECEIVE_BYTE;
        let iovec = [
            (offset_of!(libc::iovec, iov_base), byte as u64),
            (offset_of!(libc::iovec, iov_len), 1),
        ];
        let stack = [
            (
                offset_of!(libc::stack_t, ss_sp),
                (SIGNAL_STACKS + thread * SIGNAL_STACK_SIZE) as u64,
            ),
            (offset_of!(libc::stack_t, ss_flags), 0),
            (offset_of!(libc::stack_t, ss_size), SIGNAL_STACK_SIZE as u64),
        ];
        for (at, word) in iovec
            .map(|(at, word)| (control::IOVEC + at, word))
            .into_iter()
            .chain(stack.map(|(at, word)| (control::STACK_RECORD + at, word)))
        {
            self.control64(thread, at).store(word, Ordering::Relaxed);
        }
    }

    /// Writes the set of `cpu` alone into a thread's control page, for its next [`Command::Place`]; `INVAL` if no set
    /// holds it.
    pub(crate) fn write_cpu_set(&self, thread: usize, cpu: usize) -> Result<(), Errno> {
        let bits = u64::BITS as usize;
        if cpu >= size_of::<libc::cpu_set_t>() * 8 {
            return Err(Errno::INVAL);
        }
        for word in 0..size_of::<libc::cpu_set_t>() / size_of::<u64>() {
            let value = if word == cpu / bits {
                1 << (cpu % bits)
            } else {
                0
            };
            self.control64(thread, control::CPU_SET + word * size_of::<u64>())
                .store(value, Ordering::Relaxed);
        }
        Ok(())
    }

    pub(crate) fn register32(&self, thread: usize, offset: usize) -> &AtomicU32 {
        let at = Self::register_offset(thread, word32(&register::WORDS32, offset));
        &self.words32()[at / size_of::<u32>()]
    }

    pub(crate) fn register64(&self, thread: usize, offset: usize) -> &AtomicU64 {
        let at = Self::register_offset(thread, word64(&register::WORDS32, offset));
        &self.words64()[at / size_of::<u64>()]
    }
}

/// `offset`, if it is one of the page's 32-bit words. Each byte of the thread pages is reached at one size only:
/// Rust makes racing atomic accesses of different sizes over the same bytes undefined behaviour.
fn word32(words32: &[usize], offset: usize) -> usize {
    assert!(
        words32.contains(&offset),
        "{offset} is not a 32-bit word of a thread page"
    );
    offset
}

/// `offset`, if it starts a 64-bit word that holds none of the page's 32-bit words.
fn word64(words32: &[usize], offset: usize) -> usize {
    assert!(
        offset.is_multiple_of(size_of::<u64>())
            && !words32
                .iter()
                .any(|word| word / size_of::<u64>() == offset / size_of::<u64>()),
        "{offset} is not a 64-bit word of a thread page"
    );
    offset
}

const _: () = {
    let mut index = 0;
    while index < control::WORDS32.len() {
        assert!(control::WORDS32[index].is_multiple_of(size_of::<u32>()));
        index += 1;
    }
    let mut index = 0;
    while index < register::WORDS32.len() {
        assert!(register::WORDS32[index].is_multiple_of(size_of::<u32>()));
        index += 1;
    }
    assert!(REGISTER_PAGES_OFFSET >= THREADS_PER_PROCESS * THREAD_PAGE_SIZE);
    assert!(THREAD_PAGE_SIZE.is_multiple_of(size_of::<u64>()));
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Before the check, `control64(0, ANSWER)` gave a 64-bit atomic over the 32-bit answer word, so safe code of
    /// this crate could race two sizes over the same bytes.
    #[test]
    #[should_panic(expected = "is not a 64-bit word")]
    fn a_32_bit_word_is_never_reached_at_64_bits() {
        let pages = ThreadPages::new().expect("thread pages");
        let _ = pages.control64(0, control::ANSWER);
    }

    #[test]
    #[should_panic(expected = "is not a 64-bit word")]
    fn the_64_bit_word_holding_a_32_bit_one_is_refused() {
        let pages = ThreadPages::new().expect("thread pages");
        let _ = pages.register64(0, register::REQUEST);
    }

    #[test]
    #[should_panic(expected = "is not a 32-bit word")]
    fn a_64_bit_word_is_never_reached_at_32_bits() {
        let pages = ThreadPages::new().expect("thread pages");
        let _ = pages.register32(0, register::EXIT);
    }

    #[test]
    #[should_panic(expected = "is not a 64-bit word")]
    fn a_misaligned_offset_is_refused_not_rounded() {
        let pages = ThreadPages::new().expect("thread pages");
        let _ = pages.control64(0, control::RESUME + 4);
    }

    #[test]
    fn every_named_word_is_reached_at_its_own_size() {
        let pages = ThreadPages::new().expect("thread pages");
        for offset in control::WORDS32 {
            pages.control32(0, offset);
        }
        for offset in register::WORDS32 {
            pages.register32(THREADS_PER_PROCESS - 1, offset);
        }
        for offset in [
            control::RESUME,
            control::COMMAND_COUNT,
            control::TOUCH_ADDRESS,
            control::TOUCH_VALUE,
            control::ENTER_AT,
            control::COMMANDS,
            THREAD_PAGE_SIZE - size_of::<u64>(),
        ] {
            pages.control64(0, offset);
        }
        for offset in [
            register::TRANSLATED,
            register::EXIT,
            register::EXIT_VALUE,
            register::STOPS_IN_TURN,
            register::STOPS_IN_PUBLISH,
            register::STOPS_IN_WAIT,
            register::STOPS_IN_END,
            register::EXIT_DETAIL,
            register::STOPS_IN_CALL,
            register::VECTORS_BY_RETURN,
            register::CALL_SCRATCH,
            register::EXIT_SYNDROME,
            register::REGISTERS,
        ] {
            pages.register64(0, offset);
        }
    }

    /// Records the stub hands the host kernel lie on its read-only control page, so a guest store cannot change
    /// them between the stub's check and its call.
    #[test]
    fn every_thread_s_records_name_its_own_receive_byte_and_signal_stack() {
        let pages = ThreadPages::new().expect("thread pages");
        for thread in [0, 1, THREADS_PER_PROCESS - 1] {
            pages.write_records(thread);
            let word = |at| pages.control64(thread, at).load(Ordering::Relaxed);
            let register_page = (REGISTER_PAGES + thread * THREAD_PAGE_SIZE) as u64;
            let byte = word(control::IOVEC);
            assert!(byte >= register_page + register::RECEIVE as u64);
            assert!(
                byte < register_page + (register::RECEIVE + crate::zygote::RECEIVE_SIZE) as u64
            );
            assert_eq!(word(control::IOVEC + 8), 1);
            assert_eq!(
                word(control::STACK_RECORD),
                (SIGNAL_STACKS + thread * SIGNAL_STACK_SIZE) as u64
            );
            assert_eq!(word(control::STACK_RECORD + 8), 0);
            assert_eq!(word(control::STACK_RECORD + 16), SIGNAL_STACK_SIZE as u64);
        }
    }

    #[test]
    fn a_cpu_set_holds_its_cpu_alone_and_one_past_the_set_is_refused() {
        let pages = ThreadPages::new().expect("thread pages");
        let bits = size_of::<libc::cpu_set_t>() * 8;
        let words = bits / 64;
        for cpu in [0, 63, 64, 130, bits - 1] {
            pages.write_cpu_set(2, cpu).expect("a cpu inside the set");
            for word in 0..words {
                let expected = if word == cpu / 64 { 1 << (cpu % 64) } else { 0 };
                let at = control::CPU_SET + word * 8;
                assert_eq!(pages.control64(2, at).load(Ordering::Relaxed), expected);
            }
        }
        assert_eq!(pages.write_cpu_set(2, bits), Err(Errno::INVAL));
    }
}
