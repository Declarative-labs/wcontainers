use std::io;
use std::ptr::NonNull;
use std::sync::Arc;

use crate::Mapped;
use crate::arithmetic::WORD;
use crate::seal::Sealed;
use memmap2::{MmapOptions, MmapRaw};
#[cfg(target_os = "linux")]
use rustix::fd::BorrowedFd;
use thiserror::Error;

/// Memory the host mapped for this program, readable and writable, and unmapped when the value drops.
pub struct HostMapping {
    map: MmapRaw,
}

#[derive(Debug, Error)]
pub enum AnonymousError {
    #[error("the length is zero or not a multiple of 8")]
    Length,
    #[error("the host refused the mapping: {0}")]
    Refused(#[from] io::Error),
}

#[cfg(target_os = "linux")]
#[derive(Debug, Error)]
pub enum FileError {
    #[error("the length is zero, or the length or the offset is not a multiple of 8")]
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
    pub fn anonymous(length: usize) -> Result<Arc<HostMapping>, AnonymousError> {
        if length == 0 || !length.is_multiple_of(WORD) {
            return Err(AnonymousError::Length);
        }
        let map = MmapRaw::from(MmapOptions::new().len(length).map_anon()?);
        Ok(Arc::new(HostMapping::aligned(map)))
    }

    /// Maps the memory file's bytes from the offset for the length, shared with whatever else maps the file.
    #[cfg(target_os = "linux")]
    pub fn file(
        file: BorrowedFd<'_>,
        offset: u64,
        length: usize,
    ) -> Result<Arc<HostMapping>, FileError> {
        if length == 0 || !length.is_multiple_of(WORD) || !offset.is_multiple_of(WORD as u64) {
            return Err(FileError::Length);
        }
        let map = shrink_sealed::ShrinkSealedFile::new(file)?.map(offset, length)?;
        Ok(Arc::new(HostMapping::aligned(map)))
    }

    /// Wraps a mapping whose start is a page plus a multiple of 8, so aligned to 8: memmap2 maps from the page that
    /// holds the offset and returns the address of the offset itself (memmap2-0.9.12 src/unix.rs:99-100, :246).
    fn aligned(map: MmapRaw) -> HostMapping {
        assert!(
            map.as_ptr().cast::<u64>().is_aligned() && !map.as_ptr().is_null(),
            "a mapping starts aligned to 8"
        );
        HostMapping { map }
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

impl Sealed for HostMapping {}

#[expect(
    unsafe_code,
    reason = "memory the host mapped, which only `Words` reaches"
)]
// SAFETY: `MmapRaw` keeps the range mapped readable and writable until it drops, and nothing here remaps or unmaps it.
// Anonymous memory never traps; a file is sealed against shrinking, which no descriptor of it can undo, and was at
// least as long as the range when mapped, so it stays so, and it is not on the huge-page file system. The order matters:
// the seal was seen BEFORE the size was read (`ShrinkSealedFile`), since a size read first can be shrunk away and the
// file sealed after. memmap2 returns the address of the offset asked, not of its page, so the bytes from `base` are
// the file's bytes from the offset. `aligned` checked the start, and both constructors the length, which is the
// mapping's own and never changes. Only `base` hands out the address. Whatever else reaches the bytes (another mapping
// of the file, another process, or the kernel through the file) is an outside agent and writes only the file's pages.
// NOT KEPT: under strict overcommit (`vm.overcommit_memory=2`) a page of a file faulted in can fail to be charged, and
// the access ends the process with SIGBUS (MEASURED, REVIEW.md); a crash, not a write anywhere else.
unsafe impl Mapped for HostMapping {
    fn base(&self) -> NonNull<u64> {
        NonNull::new(self.map.as_mut_ptr().cast()).expect("a mapping is never at address 0")
    }

    fn len(&self) -> usize {
        self.map.len()
    }
}
