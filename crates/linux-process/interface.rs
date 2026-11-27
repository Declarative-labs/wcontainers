//! What a Linux process does itself that no safe crate offers: the
//! [system calls](https://man7.org/linux/man-pages/man2/syscalls.2.html) rustix leaves unsafe or lacks, and the
//! [ARM64 instructions Linux lets user space run](https://docs.kernel.org/arch/arm64/cpu-feature-registers.html).
#![cfg(target_os = "linux")]

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
    /// process's code.
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

    /// Makes a sealed [memory file](https://man7.org/linux/man-pages/man2/memfd_create.2.html) of the length, which
    /// cannot shrink, and maps it shared; `EINVAL` for an empty one, which mmap(2) refuses.
    fn create_shared_words(&self, name: &str, length: usize) -> Result<Self::SharedWords, Errno>;

    /// Returns the memory file, for another process to map.
    fn shared_words_file<'words>(&self, words: &'words Self::SharedWords) -> BorrowedFd<'words>;

    /// Returns the memory as 32-bit atomic words.
    fn shared_words32<'words>(&self, words: &'words Self::SharedWords) -> &'words [AtomicU32];

    /// Returns the memory as 64-bit atomic words.
    fn shared_words64<'words>(&self, words: &'words Self::SharedWords) -> &'words [AtomicU64];

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
