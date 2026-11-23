//! A stub's code area: one memfd the embedder writes through its own mapping and the stub runs, execute-only, after
//! [`crate::pages::Command::MapCode`].

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rustix::fs::MemfdFlags;
use rustix::io::Errno;
use rustix::mm::{MapFlags, ProtFlags};

use crate::layout::{CODE_AREA, CODE_AREA_SIZE};
use crate::probe::HostProbe;

/// The code area of one guest memory.
pub struct CodeArea {
    file: OwnedFd,
    /// The embedder's mapping, kept as a number so the value is `Send`; `Drop` unmaps it.
    base: usize,
}

impl CodeArea {
    /// An empty code area.
    pub fn new() -> Result<Self, Errno> {
        let file = rustix::fs::memfd_create("wcontainers code area", MemfdFlags::CLOEXEC)?;
        rustix::fs::ftruncate(&file, CODE_AREA_SIZE as u64)?;
        #[expect(
            unsafe_code,
            reason = "maps the code area where the host chooses, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: a new mapping at an address the host picks overlaps nothing; `Drop` unmaps it.
        let base = unsafe {
            rustix::mm::mmap(
                core::ptr::null_mut(),
                CODE_AREA_SIZE,
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

    /// The memfd, for the stub's socket before its `MapCode`.
    pub fn file(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }

    /// Writes `words` at the stub's address `at` and makes them what instruction fetch sees there; a stub thread runs
    /// them once it has left the host kernel after this returns, which synchronises its context.
    pub fn write(&mut self, probe: &HostProbe, at: usize, words: &[u32]) {
        let bytes = size_of_val(words);
        assert!(
            at >= CODE_AREA
                && at.is_multiple_of(size_of::<u32>())
                && at - CODE_AREA + bytes <= CODE_AREA_SIZE,
            "code outside the code area"
        );
        let offset = at - CODE_AREA;
        #[expect(
            unsafe_code,
            reason = "writes the shared mapping, which only this value writes and a stub only runs"
        )]
        // SAFETY: the range is inside the mapping, which lives as long as `self`; `&mut self` makes this the only
        // reference into it.
        let into =
            unsafe { core::slice::from_raw_parts_mut((self.base + offset) as *mut u8, bytes) };
        for (word, into) in words
            .iter()
            .zip(into.as_chunks_mut::<{ size_of::<u32>() }>().0)
        {
            *into = word.to_le_bytes();
        }
        #[cfg(target_arch = "aarch64")]
        publish_code(probe, self.base + offset, bytes);
        #[cfg(not(target_arch = "aarch64"))]
        let _ = probe;
    }
}

impl Drop for CodeArea {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "unmaps the mapping made by `new`")]
        // SAFETY: the mapping is this value's alone, and every reference into it borrowed `self`.
        let _ = unsafe { rustix::mm::munmap(self.base as *mut core::ffi::c_void, CODE_AREA_SIZE) };
    }
}

/// The architecture's maintenance after writing code at `[start, start + bytes)` through this mapping: clean the data
/// lines to the point of unification and invalidate the instruction lines, each only where `CTR_EL0` asks, each
/// finished by a barrier (Arm ARM, "Concurrent modification and execution of instructions").
#[cfg(target_arch = "aarch64")]
#[expect(
    unsafe_code,
    reason = "cache maintenance by address and its barriers are instructions no crate wraps"
)]
fn publish_code(probe: &HostProbe, start: usize, bytes: usize) {
    let end = start + bytes;
    let lines = |line: usize| (start & !(line - 1)..end).step_by(line);
    let (data, instruction) = probe.code_lines();
    if let Some(line) = data {
        for at in lines(line) {
            // SAFETY: cleaning a line of a mapping this process holds changes no memory's value.
            unsafe {
                core::arch::asm!("dc cvau, {}", in(reg) at, options(nostack, preserves_flags))
            };
        }
    }
    // SAFETY: a barrier has no other effect.
    unsafe { core::arch::asm!("dsb ish", options(nostack, preserves_flags)) };
    if let Some(line) = instruction {
        for at in lines(line) {
            // SAFETY: invalidating instruction lines changes no memory's value.
            unsafe {
                core::arch::asm!("ic ivau, {}", in(reg) at, options(nostack, preserves_flags))
            };
        }
        // SAFETY: as above.
        unsafe { core::arch::asm!("dsb ish", options(nostack, preserves_flags)) };
    }
}
