//! The private memory the kernel is given in place of a caller's buffer.
//!
//! On Linux it is a sealed memfd, mapped with `GUARD` bytes past its end that no page backs. A region lent for a call
//! is the file's last bytes, so from the region's start onward the kernel reaches only this file, and then, for the
//! next `GUARD` bytes, nothing: its access there fails with `EFAULT` (MEASURED by the suite's attacks). The file
//! begins with `BEFORE` bytes that the primitive never writes, so a reach back from a region's start also lands in
//! the file. No Rust reference ever covers the mapping; bytes go in and out with `pwrite` and `pread`.
//!
//! Under Kani and Miri a region is a heap buffer of exactly its length, so that they report any access past it.

use std::ffi::c_void;

use rustix::io::Errno;

/// The bytes mapped past the file's end with no page behind them: any offset a 32-bit field can name.
///
/// Assumed, and not checked: no kernel handler reaches 4 GiB or more past the address it is given.
#[cfg(not(any(kani, miri)))]
const GUARD: usize = 1 << 32;

/// The file's bytes before any region, never written and costing no memory until touched, so that a handler reaching
/// back from the address it is given touches only this file unless it reaches 4 GiB or more.
#[cfg(not(any(kani, miri)))]
const BEFORE: usize = 1 << 32;

/// The smallest file kept, a page, so that most calls reuse one.
#[cfg(not(any(kani, miri)))]
const SMALLEST: usize = 1 << 12;

#[cfg(not(any(kani, miri)))]
mod memory {
    use std::cell::RefCell;
    use std::os::fd::OwnedFd;

    use memmap2::{MmapOptions, MmapRaw};
    use rustix::fs::{FallocateFlags, MemfdFlags, SealFlags};
    use rustix::io::Errno;

    use super::{BEFORE, GUARD, SMALLEST};

    /// A sealed memfd of `BEFORE` and then `length` bytes, mapped with `GUARD` unbacked bytes after them.
    pub(super) struct Scratch {
        file: OwnedFd,
        map: MmapRaw,
        length: usize,
    }

    impl Scratch {
        fn new(wanted: usize) -> Result<Self, Errno> {
            let length = wanted.max(SMALLEST).checked_next_power_of_two().ok_or(Errno::NOMEM)?;
            let file =
                rustix::fs::memfd_create("socket-option", MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)?;
            let whole = length.checked_add(BEFORE).ok_or(Errno::NOMEM)?;
            rustix::fs::ftruncate(&file, whole as u64)?;
            rustix::fs::fcntl_add_seals(&file, SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL)?;
            let mapped = whole.checked_add(GUARD).ok_or(Errno::NOMEM)?;
            let map = MmapOptions::new()
                .len(mapped)
                .map_raw(&file)
                .map_err(|error| Errno::from_io_error(&error).unwrap_or(Errno::NOMEM))?;
            Ok(Self { file, map, length })
        }

        pub(super) fn length(&self) -> usize {
            self.length
        }

        /// The address of the first byte after `BEFORE`, where offsets are counted from.
        pub(super) fn start(&self) -> *mut u8 {
            self.map.as_mut_ptr().wrapping_add(BEFORE)
        }

        pub(super) fn write(&self, at: usize, bytes: &[u8]) -> Result<(), Errno> {
            let written = rustix::io::pwrite(&self.file, bytes, (BEFORE + at) as u64)?;
            if written == bytes.len() { Ok(()) } else { Err(Errno::IO) }
        }

        pub(super) fn read(&self, at: usize, into: &mut [u8]) -> Result<(), Errno> {
            let read = rustix::io::pread(&self.file, &mut *into, (BEFORE + at) as u64)?;
            if read == into.len() { Ok(()) } else { Err(Errno::IO) }
        }

        /// Punches the bytes out of the file, so that they read as zeros and no memory of the process is copied.
        pub(super) fn clear(&self, at: usize, length: usize) -> Result<(), Errno> {
            if length == usize::MIN {
                return Ok(());
            }
            let mode = FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE;
            rustix::fs::fallocate(&self.file, mode, (BEFORE + at) as u64, length as u64)
        }
    }

    thread_local! {
        /// Each thread's scratch, kept between calls and replaced by a longer one when a call needs it.
        static SCRATCH: RefCell<Option<Scratch>> = const { RefCell::new(None) };
    }

    /// Runs `use_scratch` with this thread's scratch of at least `length` bytes.
    pub(super) fn with<R>(
        length: usize,
        use_scratch: impl FnOnce(&Scratch) -> Result<R, Errno>,
    ) -> Result<R, Errno> {
        SCRATCH.with(|cell| {
            let mut kept = cell.borrow_mut();
            if kept.as_ref().is_none_or(|scratch| scratch.length < length) {
                *kept = None;
                *kept = Some(Scratch::new(length)?);
            }
            let scratch = kept.as_ref().ok_or(Errno::NOMEM)?;
            use_scratch(scratch)
        })
    }
}

#[cfg(any(kani, miri))]
mod memory {
    use std::cell::RefCell;

    use rustix::io::Errno;

    /// A heap buffer of exactly the length lent.
    pub(super) struct Scratch {
        bytes: RefCell<Vec<u8>>,
    }

    impl Scratch {
        pub(super) fn length(&self) -> usize {
            self.bytes.borrow().len()
        }

        pub(super) fn start(&self) -> *mut u8 {
            self.bytes.borrow_mut().as_mut_ptr()
        }

        pub(super) fn write(&self, at: usize, bytes: &[u8]) -> Result<(), Errno> {
            let mut held = self.bytes.borrow_mut();
            held.get_mut(at..at + bytes.len()).ok_or(Errno::IO)?.copy_from_slice(bytes);
            Ok(())
        }

        pub(super) fn read(&self, at: usize, into: &mut [u8]) -> Result<(), Errno> {
            let held = self.bytes.borrow();
            into.copy_from_slice(held.get(at..at + into.len()).ok_or(Errno::IO)?);
            Ok(())
        }

        pub(super) fn clear(&self, at: usize, length: usize) -> Result<(), Errno> {
            let mut held = self.bytes.borrow_mut();
            held.get_mut(at..at + length).ok_or(Errno::IO)?.fill(u8::MIN);
            Ok(())
        }
    }

    pub(super) fn with<R>(
        length: usize,
        use_scratch: impl FnOnce(&Scratch) -> Result<R, Errno>,
    ) -> Result<R, Errno> {
        let scratch = Scratch {
            bytes: RefCell::new(vec![u8::MIN; length]),
        };
        use_scratch(&scratch)
    }
}

/// `length` bytes of scratch that end where the guard begins, lent for one call.
pub(crate) struct Lent<'a> {
    scratch: &'a memory::Scratch,
    /// Where the region begins in the scratch.
    at: usize,
    length: usize,
}

impl<'a> Lent<'a> {
    /// The region's length.
    pub(crate) fn len(&self) -> usize {
        self.length
    }

    /// The address of the byte `offset` into the region, the only kind of address the kernel is given.
    pub(crate) fn address(&self, offset: usize) -> *mut c_void {
        self.scratch.start().wrapping_add(self.at + offset).cast()
    }

    /// Writes the bytes into the region at the offset.
    pub(crate) fn write(&self, offset: usize, bytes: &[u8]) -> Result<(), Errno> {
        self.check(offset, bytes.len())?;
        self.scratch.write(self.at + offset, bytes)
    }

    /// Reads the region's bytes at the offset into the buffer.
    pub(crate) fn read(&self, offset: usize, into: &mut [u8]) -> Result<(), Errno> {
        self.check(offset, into.len())?;
        self.scratch.read(self.at + offset, into)
    }

    /// Makes every byte of the region zero.
    pub(crate) fn clear(&self) -> Result<(), Errno> {
        self.scratch.clear(self.at, self.length)
    }

    /// The region from the offset on, which ends where this one ends.
    pub(crate) fn from(&self, offset: usize) -> Result<Lent<'a>, Errno> {
        self.check(offset, usize::MIN)?;
        Ok(Lent {
            scratch: self.scratch,
            at: self.at + offset,
            length: self.length - offset,
        })
    }

    fn check(&self, offset: usize, length: usize) -> Result<(), Errno> {
        match offset.checked_add(length) {
            Some(end) if end <= self.length => Ok(()),
            _ => Err(Errno::INVAL),
        }
    }
}

/// Lends `length` bytes of zeros that end where the guard begins, for `call`.
pub(crate) fn lend<R>(length: usize, call: impl FnOnce(&Lent<'_>) -> Result<R, Errno>) -> Result<R, Errno> {
    memory::with(length, |scratch| {
        let lent = Lent {
            scratch,
            at: scratch.length() - length,
            length,
        };
        lent.clear()?;
        call(&lent)
    })
}
