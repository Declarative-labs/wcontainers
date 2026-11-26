//! Each guest thread's two pages shared with the embedder, as the stub reads and writes them: its control page, which
//! the stub only reads and obeys, and its register page, which the stub writes.

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
    pub const WORDS32: [usize; 1] = [ANSWER];
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
    pub const WORDS32: [usize; 4] = [REQUEST, THREAD_ID, EXIT_CLEAR, TAKEN];

    const _: () = assert!(EXIT_SYNDROME < REGISTERS);
    const _: () = assert!(RECEIVE + crate::zygote::RECEIVE_SIZE <= crate::layout::THREAD_PAGE_SIZE);
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

/// The kinds of command, as the stub reads them.
#[repr(u32)]
pub enum CommandKind {
    Map = 1,
    Protect = 2,
    Unmap = 3,
    NewThread = 4,
    MapShared = 5,
    Place = 6,
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
    pub fn from_word(word: u64) -> Option<Self> {
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
