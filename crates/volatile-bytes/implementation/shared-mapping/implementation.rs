//! Bytes of a mapping that another party may change at any moment, reached only by the C library's `memcpy`.

#[cfg(kani)]
mod proofs;

use core::ffi::c_void;
use core::ops::Range;
use std::io;
#[cfg(target_os = "linux")]
use std::os::fd::BorrowedFd;
use std::sync::Arc;

use memmap2::{MmapOptions, MmapRaw};
use thiserror::Error;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};

// TODO: the copies call the C library's memcpy because Rust has no copy that tolerates a source another party changes. Check on https://github.com/rust-lang/rust/issues/158947 (volatile atomics), https://github.com/rust-lang/rfcs/pull/3301 (a per-byte atomic copy) and https://github.com/rust-lang/rfcs/pull/2728 (volatile atomics); replace the call with Rust's own copy when one is stable.

/// The C library's `memcpy`, which `copy` reads back by a volatile load so that the compiler cannot know which function
/// it calls and so cannot replace the call with its own copy.
static MEMCPY: unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> *mut c_void = libc::memcpy;

/// Bytes of a mapping that another party, such as a guest or another process, may read and write at any moment.
///
/// A view of the whole mapping, or of a part of it. While the other party writes, each byte a read copies is some value
/// that byte held during the call; the bytes of one call need not come from one moment, and a byte may be read more than
/// once. A write stores only the bytes of the view, each its new value, possibly more than once. Nothing here orders this
/// program's other memory, atomics or locks.
pub struct SharedMapping {
    /// Reached only through `bytes`, and made only by `whole`, whose promise every copy relies on.
    map: Arc<MmapRaw>,
    start: usize,
    length: usize,
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

impl SharedMapping {
    /// Returns a view of the whole mapping.
    ///
    /// # Safety
    /// The map is readable and writable, reaching its bytes never traps while it lives, and no reference or slice over
    /// any of them is ever formed.
    #[expect(unsafe_code, reason = "the promise each mapping makes, on which every copy relies")]
    unsafe fn whole(map: MmapRaw) -> SharedMapping {
        let length = map.len();
        SharedMapping { map: Arc::new(map), start: 0, length }
    }

    /// Returns a view of the bytes at the offsets from this view's start, on the same mapping.
    ///
    /// PROVISIONAL: no interface declares how a view of a sub-range is made; README.md, "The constructor".
    pub fn part(&self, offsets: Range<u64>) -> Option<SharedMapping> {
        let (start, length) = part(self.start, self.length, offsets)?;
        Some(SharedMapping { map: Arc::clone(&self.map), start, length })
    }

    /// Returns the address of the view's first byte, if the `count` bytes from it all lie inside the mapping.
    fn bytes(&self, count: usize) -> Option<*mut u8> {
        let at = inside(self.map.len(), self.start, count)?;
        Some(self.map.as_mut_ptr().wrapping_add(at))
    }
}

impl StandaloneVolatileBytes for SharedMapping {
    type Config = Mapped;

    fn new(config: Mapped) -> SharedMapping {
        #[expect(unsafe_code, reason = "memory the host mapped, which only this type reaches")]
        // SAFETY: a `Mapped` is made only by `anonymous` and `file`, both readable and writable, and its field is private,
        // so nothing else forms a reference into it. Anonymous memory never traps. A file was seen sealed against
        // shrinking, which no descriptor can undo, before its size was read and found to cover the bytes, and it is not
        // on the huge-page file system, so it never traps either; this holds only while no code in this process breaks
        // I/O safety (an `unsafe` `dup2` over the borrowed descriptor between the checks and the mapping). NOT KEPT:
        // under strict overcommit (`vm.overcommit_memory=2`) a file's page can fail to be charged when first touched, and
        // the access ends the process with SIGBUS; a crash, not a write anywhere else.
        unsafe { SharedMapping::whole(config.0) }
    }
}

/// Reads and writes from the view's start, as many bytes as the shorter of the view and the buffer holds.
impl VolatileBytes for SharedMapping {
    fn len(&self) -> u64 {
        self.length as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = into.len().min(self.length);
        let Some(from) = self.bytes(count) else {
            return 0;
        };
        #[expect(unsafe_code, reason = "the copy out of the mapping")]
        // SAFETY: the `count` bytes from `from` lie inside the mapping (checked by `bytes`), which `whole`'s promise keeps
        // readable with no reference over them; `into` is this call's own buffer, so the two do not overlap.
        unsafe {
            copy(into.as_mut_ptr(), from, count);
        }
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let count = bytes.len().min(self.length);
        let Some(to) = self.bytes(count) else {
            return 0;
        };
        #[expect(unsafe_code, reason = "the copy into the mapping")]
        // SAFETY: the `count` bytes from `to` lie inside the mapping (checked by `bytes`), which `whole`'s promise keeps
        // writable with no reference over them; `bytes` is this call's own buffer, so the two do not overlap.
        unsafe {
            copy(to, bytes.as_ptr(), count);
        }
        count
    }

    // The sink and the source each take all the bytes as one slice, and no slice over the mapping may be formed, so
    // both copy through a buffer as long as the view.

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut bytes = vec![0; self.length];
        let count = self.read(&mut bytes);
        sink.write(&bytes[..count])
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut bytes = vec![0; self.length];
        let count = source.read(&mut bytes)?;
        Ok(self.write(&bytes[..count]))
    }
}

/// Returns `start` if the `count` bytes from it all lie inside `length` bytes.
fn inside(length: usize, start: usize, count: usize) -> Option<usize> {
    start.checked_add(count).filter(|&end| end <= length).map(|_| start)
}

/// Returns the start and length of the bytes at `offsets` in a view of `length` bytes from `start`, if they lie inside it.
fn part(start: usize, length: usize, offsets: Range<u64>) -> Option<(usize, usize)> {
    let begin = usize::try_from(offsets.start).ok()?;
    let end = usize::try_from(offsets.end).ok()?;
    if begin > end || end > length {
        return None;
    }
    Some((start.checked_add(begin)?, end - begin))
}

/// Copies `length` bytes from `from` to `to` with the C library's `memcpy`, called through a pointer the compiler cannot
/// see into; makes no call for no bytes, so that C's rules for the pointers of an empty copy never arise.
///
/// # Safety
/// `from` is readable and `to` writable for `length` bytes, and the two ranges do not overlap.
#[expect(unsafe_code, reason = "the one foreign call, to the C library's memcpy, that both directions use")]
unsafe fn copy(to: *mut u8, from: *const u8, length: usize) {
    if length == 0 {
        return;
    }
    // SAFETY: `MEMCPY` is a static that lives for the whole program and holds a valid function pointer, so the volatile
    // load reads a valid value; the call's ranges are the caller's promise. What `memcpy` does with a source another
    // party changes meanwhile is the C library's, and the stated assumption of README.md.
    unsafe {
        let memcpy = (&raw const MEMCPY).read_volatile();
        memcpy(to.cast(), from.cast(), length);
    }
}
