use std::io;
#[cfg(target_os = "linux")]
use std::time::Duration;

use memmap2::{MmapOptions, MmapRaw};
#[cfg(target_os = "linux")]
use rustix::fd::BorrowedFd;
use thiserror::Error;

#[cfg(target_os = "linux")]
use crate::WaitError;
use crate::foreign_implementation::region::Region;
use crate::{OutsideError, SharedMemory, Word, WordError};

/// Memory the host mapped for this program, readable and writable, and unmapped when the value drops.
pub struct HostMapping {
    region: Region,
    #[expect(
        dead_code,
        reason = "held so that the bytes stay mapped while the region lives; only the region reaches them"
    )]
    map: MmapRaw,
}

#[derive(Debug, Error)]
pub enum AnonymousError {
    #[error("the length is zero")]
    Length,
    #[error("the host refused the mapping: {0}")]
    Refused(#[from] io::Error),
}

#[cfg(target_os = "linux")]
#[derive(Debug, Error)]
pub enum FileError {
    #[error("the length is zero, or the offset is not a multiple of 8")]
    Length,
    #[error("the file is not sealed against shrinking")]
    Unsealed,
    #[error("the file's pages are huge pages, which fault when the host has none left")]
    HugePages,
    #[error("the file ends before the offset plus the length")]
    Short,
    #[error("the host refused: {0}")]
    Refused(#[from] io::Error),
}

impl HostMapping {
    /// Maps fresh anonymous memory of the length, zero when made.
    pub fn anonymous(length: usize) -> Result<HostMapping, AnonymousError> {
        if length == 0 {
            return Err(AnonymousError::Length);
        }
        let map = MmapRaw::from(MmapOptions::new().len(length).map_anon()?);
        Ok(HostMapping::new(map))
    }

    /// Maps the memory file's bytes from the offset for the length, shared with whatever else maps the file.
    #[cfg(target_os = "linux")]
    pub fn file(
        file: BorrowedFd<'_>,
        offset: u64,
        length: usize,
    ) -> Result<HostMapping, FileError> {
        if length == 0 || !offset.is_multiple_of(size_of::<u64>() as u64) {
            return Err(FileError::Length);
        }
        let map = shrink_sealed::ShrinkSealedFile::new(file)?.map(offset, length)?;
        Ok(HostMapping::new(map))
    }

    /// Wraps a mapping whose start is a page plus a multiple of 8, so aligned to 8: memmap2 maps from the page that
    /// holds the offset and returns the address of the offset itself (memmap2-0.9.12 src/unix.rs:99-100, :246).
    fn new(map: MmapRaw) -> HostMapping {
        let base = map.as_mut_ptr();
        assert!(
            base.cast::<u64>().is_aligned() && !base.is_null(),
            "a mapping starts aligned to 8"
        );
        #[expect(
            unsafe_code,
            reason = "memory the host mapped, which only the region reaches"
        )]
        // SAFETY: `MmapRaw` keeps the range mapped readable and writable until it drops, after the region, and nothing here
        // remaps or unmaps it; it hands out the address and never a reference, and nothing here forms one, so the bytes lie
        // outside every Rust allocation. Anonymous memory never traps; a file is sealed against shrinking, which no
        // descriptor of it can undo, and was at least as long as the range when mapped, so it stays so, and it is not on
        // the huge-page file system. The order matters: the seal was seen BEFORE the size was read (`ShrinkSealedFile`),
        // since a size read first can be shrunk away and the file sealed after. memmap2 returns the address of the offset
        // asked, not of its page, so the bytes from `base` are the file's bytes from the offset, and the length is the
        // mapping's own. The start is aligned (asserted above). Whatever else reaches the bytes (another mapping of the
        // file, another process, a guest, or the kernel through the file) does so on the hardware's terms, not Rust's.
        // NOT KEPT: under strict overcommit (`vm.overcommit_memory=2`) a page of a file faulted in can fail to be charged,
        // and the access ends the process with SIGBUS (MEASURED, REVIEW.md); a crash, not a write anywhere else.
        let region = unsafe { Region::new(base, map.len()) };
        HostMapping { region, map }
    }
}

/// The one place the host file's size is read: only from a file already seen sealed against shrinking.
#[cfg(target_os = "linux")]
mod shrink_sealed {
    use std::io;

    use memmap2::{MmapOptions, MmapRaw};
    use rustix::fd::BorrowedFd;
    use rustix::fs::{SealFlags, fcntl_get_seals, fstat, fstatfs};

    use super::FileError;

    // The order is the point. A seal, once seen, holds for good, so a size read after it is a size the file never goes
    // below. A size read before it may be gone by the time the seal is seen (another descriptor shrinks the file and
    // then seals it), and a mapping made on that size faults past the end. The field is private to this module, `new`
    // reads the seals before it makes the value, and only `map` reads the size, so every size was read after the seals.
    // The checks and the mapping all use the one borrowed descriptor, which I/O safety keeps naming the same file.

    /// A memory file this program saw sealed against shrinking, not on the huge-page file system.
    pub(super) struct ShrinkSealedFile<'fd> {
        file: BorrowedFd<'fd>,
    }

    impl<'fd> ShrinkSealedFile<'fd> {
        /// Reads the file's seals and file system, the first thing done with it.
        pub(super) fn new(file: BorrowedFd<'fd>) -> Result<Self, FileError> {
            if !fcntl_get_seals(file).is_ok_and(|seals| seals.contains(SealFlags::SHRINK)) {
                return Err(FileError::Unsealed);
            }
            if fstatfs(file).map_err(io::Error::from)?.f_type as u32 == libc::HUGETLBFS_MAGIC as u32
            {
                return Err(FileError::HugePages);
            }
            Ok(Self { file })
        }

        /// Reads the size now, and maps the bytes from the offset for the length if the file holds all of them.
        pub(super) fn map(&self, offset: u64, length: usize) -> Result<MmapRaw, FileError> {
            let size = u64::try_from(fstat(self.file).map_err(io::Error::from)?.st_size)
                .unwrap_or(u64::MIN);
            if offset
                .checked_add(length as u64)
                .is_none_or(|end| end > size)
            {
                return Err(FileError::Short);
            }
            Ok(MmapOptions::new()
                .offset(offset)
                .len(length)
                .map_raw(&self.file)?)
        }
    }
}

impl SharedMemory for HostMapping {
    fn len(&self) -> u64 {
        self.region.len()
    }

    fn read(&self, at: u64, into: &mut [u8]) -> Result<(), OutsideError> {
        self.region.read(at, into)
    }

    fn write(&self, at: u64, from: &[u8]) -> Result<(), OutsideError> {
        self.region.write(at, from)
    }

    fn load<W: Word>(&self, at: u64) -> Result<W, WordError> {
        self.region.load(at)
    }

    fn store<W: Word>(&self, at: u64, value: W) -> Result<(), WordError> {
        self.region.store(at, value)
    }

    fn swap<W: Word>(&self, at: u64, value: W) -> Result<W, WordError> {
        self.region.swap(at, value)
    }

    fn compare_exchange<W: Word>(&self, at: u64, current: W, new: W) -> Result<W, WordError> {
        self.region.compare_exchange(at, current, new)
    }

    #[cfg(target_os = "linux")]
    fn wait(&self, at: u64, expected: u32, timeout: Option<Duration>) -> Result<(), WaitError> {
        self.region.wait(at, expected, timeout)
    }

    #[cfg(target_os = "linux")]
    fn wake(&self, at: u64, limit: u32) -> Result<u32, WaitError> {
        self.region.wake(at, limit)
    }
}
