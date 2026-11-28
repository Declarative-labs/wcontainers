//! Bytes of a mapping that another party may change at any moment, kept mapped while any view lives and reached only
//! through `PointerBytes`.

use std::io;
#[cfg(target_os = "linux")]
use std::os::fd::BorrowedFd;
use std::sync::Arc;

use memmap2::{MmapOptions, MmapRaw};
use thiserror::Error;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};

/// Bytes of a mapping that another party, such as a guest or another process, may read and write at any moment.
///
/// A view of the whole mapping, which keeps the mapping alive. Copies behave as `PointerBytes` says.
pub struct SharedMapping {
    /// The view's bytes, made only by `new` from the map below.
    bytes: PointerBytes,
    /// Kept only so that the bytes stay mapped.
    _map: Arc<MmapRaw>,
}

/// A mapping made and checked, from which `StandaloneVolatileBytes::new` makes a `SharedMapping`.
pub struct Mapped(MmapRaw);

#[derive(Debug, Error)]
pub enum MapError {
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

impl Mapped {
    /// Maps fresh anonymous memory of the length, zero when made.
    pub fn anonymous(length: usize) -> Result<Mapped, MapError> {
        if length == 0 {
            return Err(MapError::Length);
        }
        Ok(Mapped(MmapRaw::from(MmapOptions::new().len(length).map_anon()?)))
    }

    /// Maps the memory file's bytes from the offset for the length, shared with whatever else maps the file.
    ///
    /// The order is the point: a seal, once seen, holds for good, so a size read after it is a size the file never goes
    /// below; a size read before it may be shrunk away, and the file sealed, before the seals are read.
    #[cfg(target_os = "linux")]
    pub fn file(file: BorrowedFd<'_>, offset: u64, length: usize) -> Result<Mapped, MapError> {
        use rustix::fs::{SealFlags, fcntl_get_seals, fstat, fstatfs};

        if length == 0 || !offset.is_multiple_of(size_of::<u64>() as u64) {
            return Err(MapError::Length);
        }
        if !fcntl_get_seals(file).is_ok_and(|seals| seals.contains(SealFlags::SHRINK)) {
            return Err(MapError::Unsealed);
        }
        if fstatfs(file).map_err(io::Error::from)?.f_type as u32 == libc::HUGETLBFS_MAGIC as u32 {
            return Err(MapError::HugePages);
        }
        // A 64-bit Linux keeps every file's size at most `LLONG_MAX` (include/linux/fs.h), so this never falls back to 0,
        // which would refuse everything.
        let size = u64::try_from(fstat(file).map_err(io::Error::from)?.st_size).unwrap_or(0);
        if offset.checked_add(length as u64).is_none_or(|end| end > size) {
            return Err(MapError::Short);
        }
        Ok(Mapped(MmapOptions::new().offset(offset).len(length).map_raw(&file)?))
    }
}

impl StandaloneVolatileBytes for SharedMapping {
    type Config = Mapped;

    fn new(config: Mapped) -> SharedMapping {
        let map = config.0;
        #[expect(unsafe_code, reason = "memory the host mapped, which only the base type reaches")]
        // SAFETY: the bytes stay mapped while any view lives, since each holds the `Arc` of the map. A `Mapped` is made
        // only by `anonymous` and `file`, both readable and writable, and its field is private, so nothing else forms a
        // reference into it. Anonymous memory never traps. A file was seen sealed against shrinking, which no descriptor
        // can undo, before its size was read and found to cover the bytes, and it is not on the huge-page file system, so
        // it never traps either; this holds only while no code in this process breaks I/O safety (an `unsafe` `dup2` over
        // the borrowed descriptor between the checks and the mapping). NOT KEPT: under strict overcommit
        // (`vm.overcommit_memory=2`) a file's page can fail to be charged when first touched, and the access ends the
        // process with SIGBUS; a crash, not a write anywhere else.
        let pointer = unsafe { PointerAndLength::new(map.as_mut_ptr(), map.len()) };
        SharedMapping { bytes: PointerBytes::new(pointer), _map: Arc::new(map) }
    }
}

/// Forwards to the base type.
impl VolatileBytes for SharedMapping {
    fn len(&self) -> u64 {
        self.bytes.len()
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        self.bytes.read_at(offset, into)
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        self.bytes.write_at(offset, bytes)
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        self.bytes.read_to(sink)
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        self.bytes.write_from(source)
    }
}
