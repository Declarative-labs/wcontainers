//! What a Linux process does itself that no safe crate offers: the
//! [system calls](https://man7.org/linux/man-pages/man2/syscalls.2.html) rustix leaves unsafe or lacks, and the
//! [ARM64 instructions Linux lets user space run](https://docs.kernel.org/arch/arm64/cpu-feature-registers.html).
#![cfg(target_os = "linux")]

use std::ops::Range;
use std::os::fd::BorrowedFd;
use std::sync::atomic::{AtomicU32, AtomicU64};

pub use rustix::io::Errno;
pub use rustix::mm::ProtFlags;
pub use rustix::process::Pid;
pub use rustix::process::Signal;

/// The longest SVE predicate register in bytes: a vector of 2048 bits has predicates of 256 bits.
pub const LARGEST_PREDICATE_BYTES: usize = 2048 / 8 / 8;

/// SVE's p0 and FFR as a system call left them, each in its register's bytes, the rest zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PredicatesAfterSystemCall {
    pub p0: [u8; LARGEST_PREDICATE_BYTES],
    pub ffr: [u8; LARGEST_PREDICATE_BYTES],
}

/// The general register that holds a run's [`RunFrame`] while its instructions run: x28.
#[cfg(target_arch = "aarch64")]
pub const FRAME_REGISTER: u8 = 28;
/// The general register that holds the address of a run's first instruction as it starts, and that an exit keeps in
/// the frame before it uses it: x23.
#[cfg(target_arch = "aarch64")]
pub const EXIT_SCRATCH_REGISTER: u8 = 23;
/// The general register a run sets to the first address of its mapping's window: x24.
#[cfg(target_arch = "aarch64")]
pub const WINDOW_BASE_REGISTER: u8 = 24;
/// The bytes of one exit's instructions, [`LinuxProcess::exit_instructions`].
#[cfg(target_arch = "aarch64")]
pub const EXIT_BYTES: usize = 16;
/// The words of a [`RunFrame`] its instructions own.
#[cfg(target_arch = "aarch64")]
pub const RUN_FRAME_WORDS: usize = 32;
/// The words of a [`RunFrame`] a run owns.
#[cfg(target_arch = "aarch64")]
pub const RUN_EXIT_WORDS: usize = 8;
/// The general registers, x0 to x30.
#[cfg(target_arch = "aarch64")]
pub const GENERAL_REGISTERS: usize = 31;
/// The vector registers, v0 to v31.
#[cfg(target_arch = "aarch64")]
pub const VECTOR_REGISTERS: usize = 32;

/// An ARM64 thread's registers in user space.
#[cfg(target_arch = "aarch64")]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Arm64Registers {
    pub v: [u128; VECTOR_REGISTERS],
    pub x: [u64; GENERAL_REGISTERS],
    pub sp: u64,
    pub pc: u64,
    /// The condition flags, in PSTATE's bits 31 to 28.
    pub nzcv: u64,
    pub fpsr: u64,
    pub fpcr: u64,
    pub tpidr_el0: u64,
}

/// The signal that stops a run on the thread it is sent to with [`LinuxProcess::tgkill`], wherever its instructions are.
#[cfg(target_arch = "aarch64")]
pub const RUN_STOP_SIGNAL: Signal = Signal::USR1;

/// How a run of instructions ended, as [`LinuxProcess::run_instructions`] returns it.
#[cfg(target_arch = "aarch64")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunEnd {
    /// The instructions branched with link to the exit at this address.
    Exit(u64),
    /// Nothing ran: the frame's `pc` is not an instruction of the mapping, or its stack pointer is outside the
    /// mapping's window for instructions that are not displaced.
    Refused,
    /// [`RUN_STOP_SIGNAL`] came before the instructions started: the frame's registers are those they would have
    /// started with.
    NotEntered,
    /// [`RUN_STOP_SIGNAL`] came while the instructions ran: the frame holds their registers, `pc` the instruction not
    /// yet run.
    Stopped,
    /// An access by the instructions faulted: the frame holds their registers, `pc` the faulting instruction.
    Fault {
        /// The address the host reports, its tag cleared.
        address: u64,
        write: bool,
        /// Whether the access faulted for its alignment.
        alignment: bool,
    },
    /// An instruction raised the signal (an undefined instruction, a breakpoint), at the frame's `pc`.
    Raised(Signal),
}

/// What one run of instructions holds: words they read and write through [`FRAME_REGISTER`], the words of the run,
/// and the registers they start with and leave.
#[cfg(target_arch = "aarch64")]
#[repr(C, align(16))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunFrame {
    /// The instructions' own words: [`FRAME_REGISTER`] holds their address while they run.
    pub words: [u64; RUN_FRAME_WORDS],
    /// The run's own words: what an exit found in [`EXIT_SCRATCH_REGISTER`] and x30, where the exits go, the stack and
    /// the signal stack of the thread that runs, and how the run ended. Nothing they hold before a run means anything.
    pub exit: [u64; RUN_EXIT_WORDS],
    /// The registers the instructions start with, but [`EXIT_SCRATCH_REGISTER`] (the first one's address, `pc`),
    /// [`WINDOW_BASE_REGISTER`] (the window's first address) and [`FRAME_REGISTER`] (this frame's address); then the
    /// registers they left at their exit, `pc` and x30 the address after their branch to the exit, x28 this frame's
    /// address.
    pub registers: Arm64Registers,
}

/// One method for each call or instruction, named as Linux or the Arm architecture names it; the instructions only on
/// ARM64.
#[allow(
    reviewable::explicit_shared_error::Errno,
    reason = "every Linux system call fails the same way, with the kernel's error number"
)]
pub trait LinuxProcess {
    /// A page this process mapped at an address it chose, unmapped on drop.
    type FixedMapping;
    /// Memory shared with other processes through its file, seen here as atomic words, unmapped on drop.
    type SharedWords;
    /// Instructions this process runs, at an address it chose, which no one can write, with the window of memory
    /// they were made for; unmapped on drop.
    type InstructionMapping;
    /// The thread that made it, ready to run instructions: its own signal stack and every other signal blocked, both
    /// as they were again on drop.
    #[cfg(target_arch = "aarch64")]
    type RunThread;

    /// Sends [the signal to one thread](https://man7.org/linux/man-pages/man2/tgkill.2.html) of a process.
    fn tgkill(&self, process: Pid, thread: Pid, signal: Signal) -> Result<(), Errno>;

    /// Maps an inaccessible private page [at the address](https://man7.org/linux/man-pages/man2/mmap.2.html) with
    /// `MAP_FIXED_NOREPLACE`, so that it never replaces a mapping; `EEXIST` where one is, `ENOSYS` from a Linux older
    /// than 4.17, which maps elsewhere.
    fn mmap_fixed_noreplace(
        &self,
        address: usize,
        length: usize,
    ) -> Result<Self::FixedMapping, Errno>;

    /// Replaces the pages `offset..offset + length` of the fixed mapping with a
    /// [shared mapping](https://man7.org/linux/man-pages/man2/mmap.2.html) of the file's bytes from `file_offset`, with
    /// the protection; its other pages stay, and its drop unmaps these too. `EINVAL` where the pages are not whole
    /// pages inside it; `EPERM` for an executable protection, since bytes another party writes never run as this
    /// process's instructions.
    fn map_file_within(
        &self,
        mapping: &mut Self::FixedMapping,
        offset: usize,
        length: usize,
        file: BorrowedFd<'_>,
        file_offset: u64,
        protection: ProtFlags,
    ) -> Result<(), Errno>;

    /// [Changes the protection](https://man7.org/linux/man-pages/man2/mprotect.2.html) of the pages
    /// `offset..offset + length` of the fixed mapping; `EINVAL` and `EPERM` as [`Self::map_file_within`].
    fn protect_within(
        &self,
        mapping: &mut Self::FixedMapping,
        offset: usize,
        length: usize,
        protection: ProtFlags,
    ) -> Result<(), Errno>;

    /// Maps `instructions` [at the address](https://man7.org/linux/man-pages/man2/mmap.2.html), readable and
    /// executable, from a [memory file](https://man7.org/linux/man-pages/man2/memfd_create.2.html) of their bytes
    /// sealed against every write first, so that no one can change them; never replaces a mapping. They were made for
    /// the host addresses `window`, and run with their stack pointer inside it unless they are `displaced`. `EINVAL`
    /// where the address or the length is not whole pages or the length is 0; `EEXIST` where something is mapped
    /// there; `ENOSYS` from a Linux older than 4.17.
    fn map_instructions(
        &self,
        address: usize,
        instructions: &[u8],
        window: Range<u64>,
        displaced: bool,
    ) -> Result<Self::InstructionMapping, Errno>;

    /// Returns the sealed memory file the instructions are mapped from.
    fn instructions_file<'mapping>(
        &self,
        instructions: &'mapping Self::InstructionMapping,
    ) -> BorrowedFd<'mapping>;

    /// The instructions of an exit: instructions run by [`Self::run_instructions`] that branch with link (`blr`) to a
    /// copy of them, [`FRAME_REGISTER`] holding their frame, end the run there.
    #[cfg(target_arch = "aarch64")]
    fn exit_instructions(&self) -> [u8; EXIT_BYTES];

    /// Prepares this thread to run instructions: an [alternate signal stack](https://man7.org/linux/man-pages/man2/sigaltstack.2.html)
    /// of its own, which every signal of a run is taken on, this process's handlers of a run (installed once), and
    /// every signal blocked but a run's (an access fault, an undefined instruction, a breakpoint, [`RUN_STOP_SIGNAL`]),
    /// by the [raw mask](https://man7.org/linux/man-pages/man2/rt_sigprocmask.2.html), which reaches the two signals
    /// glibc keeps for itself too. `EBUSY` where the thread is already prepared.
    #[cfg(target_arch = "aarch64")]
    fn prepare_run_thread(&self) -> Result<Self::RunThread, Errno>;

    /// Runs the mapping's instructions from the frame's `registers.pc` on the prepared thread, with every register from
    /// the frame as [`RunFrame::registers`] says, [`WINDOW_BASE_REGISTER`] the window's first address, until they
    /// branch with link to an exit ([`Self::exit_instructions`]), fault, raise a signal or are stopped; then the frame
    /// holds the registers they left, and the thread's own are as before. Refuses (`RunEnd::Refused`, nothing run) a
    /// `pc` that is not an instruction of the mapping, and a stack pointer outside the window (its top byte ignored, as
    /// the chip ignores it) for instructions that are not displaced.
    ///
    /// The caller keeps the instructions to this, which no type here can check yet: they are the translator's for
    /// the mapping's window, entered at a piece's first instruction or where a run of it stopped; so they read and
    /// write no memory but the mappings made for them and the frame's [`RunFrame::words`]; their stack is their own,
    /// whatever their stack pointer holds; they make no system call; they may change the general and vector registers,
    /// NZCV, FPSR, FPCR and TPIDR_EL0, and no other register user space can write (TPIDR2_EL0, SVCR, DIT, SSBS, TCO,
    /// the SVE bits above each vector's 128); and [`FRAME_REGISTER`] holds the frame's address at each of them.
    #[cfg(target_arch = "aarch64")]
    fn run_instructions(
        &self,
        thread: &mut Self::RunThread,
        instructions: &Self::InstructionMapping,
        frame: &mut RunFrame,
    ) -> RunEnd;

    /// Makes a sealed [memory file](https://man7.org/linux/man-pages/man2/memfd_create.2.html) of the length, which
    /// cannot shrink, and maps it shared; `EINVAL` for an empty one, which mmap(2) refuses.
    fn create_shared_words(&self, name: &str, length: usize) -> Result<Self::SharedWords, Errno>;

    /// Returns the memory file, for another process to map.
    fn shared_words_file<'words>(&self, words: &'words Self::SharedWords) -> BorrowedFd<'words>;

    /// Returns the memory as 32-bit atomic words.
    fn shared_words32<'words>(&self, words: &'words Self::SharedWords) -> &'words [AtomicU32];

    /// Returns the memory as 64-bit atomic words.
    fn shared_words64<'words>(&self, words: &'words Self::SharedWords) -> &'words [AtomicU64];

    /// Reads [`FPCR`](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPCR--Floating-point-Control-Register),
    /// this thread's floating-point control.
    #[cfg(target_arch = "aarch64")]
    fn read_fpcr(&self) -> u64;

    /// Reads [`CTR_EL0`](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register),
    /// the cache type register.
    #[cfg(target_arch = "aarch64")]
    fn read_ctr_el0(&self) -> u64;

    /// Cleans [the data cache line](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/DC--Data-Cache-operation--an-alias-of-SYS-)
    /// at the address to the point of unification; an address this process has not mapped is a fault.
    #[cfg(target_arch = "aarch64")]
    fn dc_cvau(&self, address: usize);

    /// Invalidates [the instruction cache line](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/IC--Instruction-Cache-operation--an-alias-of-SYS-)
    /// at the address to the point of unification; an address this process has not mapped is a fault.
    #[cfg(target_arch = "aarch64")]
    fn ic_ivau(&self, address: usize);

    /// Sets every bit of SVE's p0 and FFR, makes [the system call `getppid`](https://man7.org/linux/man-pages/man2/getppid.2.html),
    /// and returns both as it left them, or nothing on a host without SVE.
    #[cfg(target_arch = "aarch64")]
    fn predicates_after_system_call(&self) -> Option<PredicatesAfterSystemCall>;
}
