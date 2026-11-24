#![cfg(unix)]

use std::io;
use std::ptr::NonNull;
use std::sync::Arc;

use memmap2::{MmapOptions, MmapRaw};
#[cfg(target_os = "linux")]
use rustix::fd::BorrowedFd;
use thiserror::Error;
use wcontainers_shared_memory::{Mapped, Sealed};

/// The bytes of one word.
const WORD: usize = size_of::<u64>();

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
    pub fn file(file: BorrowedFd<'_>, offset: u64, length: usize) -> Result<Arc<HostMapping>, FileError> {
        use rustix::fs::{SealFlags, fcntl_get_seals, fstat, fstatfs};

        if length == 0 || !length.is_multiple_of(WORD) || !offset.is_multiple_of(WORD as u64) {
            return Err(FileError::Length);
        }
        if !fcntl_get_seals(file).is_ok_and(|seals| seals.contains(SealFlags::SHRINK)) {
            return Err(FileError::Unsealed);
        }
        if fstatfs(file).map_err(io::Error::from)?.f_type as u32 == libc::HUGETLBFS_MAGIC as u32 {
            return Err(FileError::HugePages);
        }
        let size = u64::try_from(fstat(file).map_err(io::Error::from)?.st_size).unwrap_or(u64::MIN);
        if offset.checked_add(length as u64).is_none_or(|end| end > size) {
            return Err(FileError::Short);
        }
        let map = MmapOptions::new().offset(offset).len(length).map_raw(&file)?;
        Ok(Arc::new(HostMapping::aligned(map)))
    }

    /// Wraps a mapping whose start is a page plus a multiple of 8, so aligned to 8.
    fn aligned(map: MmapRaw) -> HostMapping {
        assert!(map.as_ptr().cast::<u64>().is_aligned() && !map.as_ptr().is_null(), "a mapping starts aligned to 8");
        HostMapping { map }
    }
}

impl Sealed for HostMapping {}

#[expect(unsafe_code, reason = "memory the host mapped, which only `Words` reaches")]
// SAFETY: `MmapRaw` keeps the range mapped readable and writable until it drops, and nothing here remaps or unmaps it.
// Anonymous memory never traps; a file is sealed against shrinking and was at least as long as the range when mapped,
// so it stays so, and it is not on the huge-page file system. `aligned` checked the start, and both constructors the
// length, which never changes. Only `base` hands out the address. Whatever else writes the bytes (another process, or
// the kernel through the file) writes only the file's pages.
unsafe impl Mapped for HostMapping {
    fn base(&self) -> NonNull<u64> {
        NonNull::new(self.map.as_mut_ptr().cast()).expect("a mapping is never at address 0")
    }

    fn len(&self) -> usize {
        self.map.len()
    }
}
