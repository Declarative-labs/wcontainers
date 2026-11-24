//! A stub's code area and its slot table's pages: one memfd the embedder writes by position and the stub runs,
//! execute-only, after [`crate::pages::Command::MapCode`] and [`crate::pages::Command::MapSlots`].

use std::collections::{BTreeSet, HashMap};
use std::io::IoSlice;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;

use rustix::fs::MemfdFlags;
use rustix::io::Errno;
use wcontainers_abi::PAGE_SIZE;
use zerocopy::IntoBytes;

use crate::layout::{CODE_AREA, CODE_AREA_SIZE, CODE_FILE_SIZE};
use crate::probe::HostProbe;

/// A page's words.
const PAGE_WORDS: usize = PAGE_SIZE as usize / size_of::<u32>();

/// The code area of one guest memory.
pub struct CodeArea {
    file: Arc<OwnedFd>,
    /// The embedder's mapping, whose addresses name the lines the caches are maintained on.
    #[cfg(target_arch = "aarch64")]
    mapping: memmap2::MmapRaw,
    /// Every page of the memfd ever written, by its index: the words staged writes change before they reach it.
    pages: HashMap<usize, Box<[u32; PAGE_WORDS]>>,
    /// The pages changed since the last flush.
    dirty: BTreeSet<usize>,
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
            pages: HashMap::new(),
            dirty: BTreeSet::new(),
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
        self.stage(at, words);
        self.flush(probe);
    }

    /// Writes `words` at byte `offset` of the memfd, past the code area's own bytes where slot pages are, as
    /// [`Self::write`] does.
    pub fn write_slots(&mut self, probe: &HostProbe, offset: usize, words: &[u32]) {
        self.stage_slots(offset, words);
        self.flush(probe);
    }

    /// As [`Self::write`], reaching the memfd at the next [`Self::flush`].
    pub(crate) fn stage(&mut self, at: usize, words: &[u32]) {
        assert!(
            at >= CODE_AREA
                && at.is_multiple_of(size_of::<u32>())
                && at - CODE_AREA + size_of_val(words) <= CODE_AREA_SIZE,
            "code outside the code area"
        );
        self.stage_file(at - CODE_AREA, words);
    }

    /// As [`Self::write_slots`], reaching the memfd at the next [`Self::flush`].
    pub(crate) fn stage_slots(&mut self, offset: usize, words: &[u32]) {
        assert!(
            offset >= CODE_AREA_SIZE
                && offset.is_multiple_of(size_of::<u32>())
                && offset + size_of_val(words) <= CODE_FILE_SIZE,
            "slots outside the slot pages"
        );
        self.stage_file(offset, words);
    }

    fn stage_file(&mut self, offset: usize, words: &[u32]) {
        let mut word = offset / size_of::<u32>();
        let mut rest = words;
        while !rest.is_empty() {
            let (page, within) = (word / PAGE_WORDS, word % PAGE_WORDS);
            let (head, tail) = rest.split_at((PAGE_WORDS - within).min(rest.len()));
            self.pages
                .entry(page)
                .or_insert_with(|| Box::new([0; PAGE_WORDS]))[within..within + head.len()]
                .copy_from_slice(head);
            self.dirty.insert(page);
            word += head.len();
            rest = tail;
        }
    }

    /// Writes every staged page to the memfd, a run of consecutive pages per host call, and makes them what
    /// instruction fetch sees.
    pub(crate) fn flush(&mut self, probe: &HostProbe) {
        let dirty: Vec<usize> = core::mem::take(&mut self.dirty).into_iter().collect();
        for run in dirty.chunk_by(|a, b| a + 1 == *b) {
            for run in run.chunks(UIO_MAXIOV) {
                let offset = run[0] * PAGE_SIZE as usize;
                let slices: Vec<IoSlice<'_>> = run
                    .iter()
                    .map(|page| IoSlice::new(self.pages[page].as_bytes()))
                    .collect();
                let bytes = run.len() * PAGE_SIZE as usize;
                let written = rustix::io::pwritev(self.file.as_fd(), &slices, offset as u64);
                if written != Ok(bytes) {
                    for page in run {
                        crate::files::write(
                            self.file.as_fd(),
                            (page * PAGE_SIZE as usize) as u64,
                            self.pages[page].as_bytes(),
                        )
                        .expect("the host refused a write inside the code area's memfd");
                    }
                }
                #[cfg(target_arch = "aarch64")]
                publish_code(probe, self.mapping.as_ptr() as usize + offset, bytes);
            }
        }
        #[cfg(not(target_arch = "aarch64"))]
        let _ = probe;
    }
}

/// The most slices one host call writes (Linux's `UIO_MAXIOV`).
const UIO_MAXIOV: usize = libc::UIO_MAXIOV as usize;

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
