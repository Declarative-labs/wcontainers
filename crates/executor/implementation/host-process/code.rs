//! A stub's code area and its slot table's pages: one memfd the embedder writes by position and the stub runs,
//! execute-only, after [`crate::pages::Command::MapCode`] and [`crate::pages::Command::MapSlots`].

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;

use rustix::fs::MemfdFlags;
use rustix::io::Errno;
use zerocopy::IntoBytes;

use crate::layout::{CODE_AREA, CODE_AREA_SIZE, CODE_FILE_SIZE};
use crate::probe::HostProbe;

/// The code area of one guest memory.
pub struct CodeArea {
    file: Arc<OwnedFd>,
    /// The embedder's mapping, whose addresses name the lines the caches are maintained on.
    #[cfg(target_arch = "aarch64")]
    mapping: memmap2::MmapRaw,
}

impl CodeArea {
    /// An empty code area.
    pub fn new() -> Result<Self, Errno> {
        let file = rustix::fs::memfd_create("wcontainers code area", MemfdFlags::CLOEXEC)?;
        rustix::fs::ftruncate(&file, CODE_FILE_SIZE as u64)?;
        Ok(Self {
            #[cfg(target_arch = "aarch64")]
            mapping: memmap2::MmapOptions::new()
                .len(CODE_FILE_SIZE)
                .map_raw(&file)
                .map_err(|error| Errno::from_io_error(&error).unwrap_or(Errno::NOMEM))?,
            file: Arc::new(file),
        })
    }

    /// The memfd, for the stub's socket before its `MapCode`.
    pub fn file(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }

    /// The memfd, held for a command that sends it.
    #[cfg(target_arch = "aarch64")]
    pub(crate) fn shared_file(&self) -> Arc<OwnedFd> {
        Arc::clone(&self.file)
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
        self.write_file(probe, at - CODE_AREA, words);
    }

    /// Writes `words` at byte `offset` of the memfd, past the code area's own bytes where slot pages are, as
    /// [`Self::write`] does.
    pub fn write_slots(&mut self, probe: &HostProbe, offset: usize, words: &[u32]) {
        assert!(
            offset >= CODE_AREA_SIZE && offset + size_of_val(words) <= CODE_FILE_SIZE,
            "slots outside the slot pages"
        );
        self.write_file(probe, offset, words);
    }

    /// Words are little-endian, as both hosts are.
    fn write_file(&mut self, probe: &HostProbe, offset: usize, words: &[u32]) {
        crate::files::write(self.file.as_fd(), offset as u64, words.as_bytes())
            .expect("the host refused a write inside the code area's memfd");
        #[cfg(target_arch = "aarch64")]
        publish_code(
            probe,
            self.mapping.as_ptr() as usize + offset,
            size_of_val(words),
        );
        #[cfg(not(target_arch = "aarch64"))]
        let _ = probe;
    }
}

/// The architecture's maintenance after writing code at `[start, start + bytes)` of this process's mapping: clean the data
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
