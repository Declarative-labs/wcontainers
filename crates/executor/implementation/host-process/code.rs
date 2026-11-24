//! A stub's code area and its slot table: one memfd the embedder writes by position and the stub runs, execute-only,
//! after [`crate::pages::Command::MapCode`] and [`crate::pages::Command::MapSlots`].

use std::collections::{BTreeSet, HashMap};
use std::io::IoSlice;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;

use rustix::fs::{FallocateFlags, MemfdFlags};
use rustix::io::Errno;
use wcontainers_abi::PAGE_SIZE;
use zerocopy::IntoBytes;

use crate::layout::{CODE_AREA, CODE_AREA_SIZE, CODE_FILE_SIZE, SLOT_TABLE, SLOT_TABLE_SIZE};
use crate::probe::HostProbe;

/// A page's words.
const PAGE_WORDS: usize = PAGE_SIZE as usize / size_of::<u32>();

/// The bytes of the slot table the embedder maps at once, where the caches are maintained by address.
#[cfg(target_arch = "aarch64")]
const SLOT_WINDOW: usize = 1 << 21;

/// The code area of one guest memory.
pub struct CodeArea {
    file: Arc<OwnedFd>,
    /// The embedder's mapping of the code area, whose addresses name the lines the caches are maintained on.
    #[cfg(target_arch = "aarch64")]
    mapping: memmap2::MmapRaw,
    /// The embedder's mappings of the slot table's windows written, by index, made only where the caches need lines.
    #[cfg(target_arch = "aarch64")]
    windows: HashMap<usize, memmap2::MmapRaw>,
    /// Every page of the memfd ever written, by its index: the words staged writes change before they reach it.
    pages: HashMap<usize, Box<[u32; PAGE_WORDS]>>,
    /// The pages changed since the last flush.
    dirty: BTreeSet<usize>,
    /// The slot table's pages a jump reached since the last flush, by index: one never written holds only the page its
    /// fault made, given back at the flush.
    reached: Vec<usize>,
}

impl CodeArea {
    /// An empty code area.
    pub fn new() -> Result<Self, Errno> {
        let file = rustix::fs::memfd_create("wcontainers code area", MemfdFlags::CLOEXEC)?;
        rustix::fs::ftruncate(&file, CODE_FILE_SIZE as u64)?;
        Ok(Self {
            #[cfg(target_arch = "aarch64")]
            mapping: memmap2::MmapOptions::new()
                .len(CODE_AREA_SIZE)
                .map_raw(&file)
                .map_err(|error| Errno::from_io_error(&error).unwrap_or(Errno::NOMEM))?,
            #[cfg(target_arch = "aarch64")]
            windows: HashMap::new(),
            file: Arc::new(file),
            pages: HashMap::new(),
            dirty: BTreeSet::new(),
            reached: Vec::new(),
        })
    }

    /// The memfd, for the stub's socket before its `MapCode` or `MapSlots`.
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
        let _ = self.flush(probe);
    }

    /// Writes `words` at the stub's address `at` in the slot table, as [`Self::write`] does.
    pub fn write_slots(&mut self, probe: &HostProbe, at: usize, words: &[u32]) {
        self.stage_slots(at, words);
        let _ = self.flush(probe);
    }

    /// As [`Self::write`], reaching the memfd at the next [`Self::flush`].
    pub(crate) fn stage(&mut self, at: usize, words: &[u32]) {
        self.stage_code(at, words.as_bytes());
    }

    /// As [`Self::stage`], for code as the bytes of its words.
    pub(crate) fn stage_code(&mut self, at: usize, bytes: &[u8]) {
        assert!(
            at >= CODE_AREA
                && at.is_multiple_of(size_of::<u32>())
                && bytes.len().is_multiple_of(size_of::<u32>())
                && at - CODE_AREA + bytes.len() <= CODE_AREA_SIZE,
            "code outside the code area"
        );
        self.stage_file(at - CODE_AREA, bytes);
    }

    /// As [`Self::write_slots`], reaching the memfd at the next [`Self::flush`].
    pub(crate) fn stage_slots(&mut self, at: usize, words: &[u32]) {
        assert!(
            at >= SLOT_TABLE
                && at.is_multiple_of(size_of::<u32>())
                && at - SLOT_TABLE + size_of_val(words) <= SLOT_TABLE_SIZE,
            "slots outside the slot table"
        );
        self.stage_file(CODE_AREA_SIZE + at - SLOT_TABLE, words.as_bytes());
    }

    fn stage_file(&mut self, offset: usize, bytes: &[u8]) {
        let page_bytes = PAGE_SIZE as usize;
        let mut at = offset;
        let mut rest = bytes;
        while !rest.is_empty() {
            let (page, within) = (at / page_bytes, at % page_bytes);
            let (head, tail) = rest.split_at((page_bytes - within).min(rest.len()));
            self.pages
                .entry(page)
                .or_insert_with(|| Box::new([0; PAGE_WORDS]))
                .as_mut_bytes()[within..within + head.len()]
                .copy_from_slice(head);
            self.dirty.insert(page);
            at += head.len();
            rest = tail;
        }
    }

    /// Notes that a jump reached the slot at the stub's address `at`.
    #[cfg(target_arch = "aarch64")]
    pub(crate) fn reached_slot(&mut self, at: usize) {
        if (SLOT_TABLE..SLOT_TABLE + SLOT_TABLE_SIZE).contains(&at) {
            self.reached
                .push((CODE_AREA_SIZE + at - SLOT_TABLE) / PAGE_SIZE as usize);
        }
    }

    /// Writes every staged page to the memfd, a run of consecutive pages per host call, and makes them what
    /// instruction fetch sees; gives back each page reached and never written. How many were given back, and how many
    /// the host kept.
    pub(crate) fn flush(&mut self, probe: &HostProbe) -> (usize, usize) {
        let (mut given, mut kept) = (0, 0);
        for page in core::mem::take(&mut self.reached) {
            if !self.pages.contains_key(&page) {
                let given_back = rustix::fs::fallocate(
                    self.file.as_fd(),
                    FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE,
                    (page * PAGE_SIZE as usize) as u64,
                    PAGE_SIZE,
                );
                given += usize::from(given_back.is_ok());
                kept += usize::from(given_back.is_err());
            }
        }
        let dirty: Vec<usize> = core::mem::take(&mut self.dirty).into_iter().collect();
        if dirty.is_empty() {
            return (given, kept);
        }
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
            }
        }
        #[cfg(target_arch = "aarch64")]
        if probe.code_lines() == (None, None) {
            publish_code(probe, 0, 0);
        } else {
            for page in dirty {
                let at = self.line_address(page * PAGE_SIZE as usize);
                publish_code(probe, at, PAGE_SIZE as usize);
            }
        }
        #[cfg(not(target_arch = "aarch64"))]
        let _ = probe;
        (given, kept)
    }

    /// The embedder's address of byte `offset` of the memfd, mapping its slot table's window at first use.
    #[cfg(target_arch = "aarch64")]
    fn line_address(&mut self, offset: usize) -> usize {
        if offset < CODE_AREA_SIZE {
            return self.mapping.as_ptr() as usize + offset;
        }
        let within = offset - CODE_AREA_SIZE;
        let window = within / SLOT_WINDOW;
        let mapping = self.windows.entry(window).or_insert_with(|| {
            memmap2::MmapOptions::new()
                .offset((CODE_AREA_SIZE + window * SLOT_WINDOW) as u64)
                .len(SLOT_WINDOW)
                .map_raw(&*self.file)
                .expect("the host refused a window of the slot table")
        });
        mapping.as_ptr() as usize + within % SLOT_WINDOW
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
