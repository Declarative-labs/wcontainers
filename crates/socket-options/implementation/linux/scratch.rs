//! The private memory the kernel is given in place of a caller's buffer.
//!
//! On Linux it is a sealed memfd, mapped with `GUARD` bytes past its end that no page backs. A region lent for a call
//! is the file's last bytes, so from the region's start onward the kernel reaches only this file, and then, for the
//! next `GUARD` bytes, nothing: its access there fails with `EFAULT` (MEASURED by the suite's attacks). No Rust
//! reference ever covers the mapping; bytes go in and out with `pwrite` and `pread`.
//!
//! Under Kani and Miri a region is a heap buffer of exactly its length, so that they report any access past it.

use std::ffi::c_void;

use rustix::io::Errno;

/// The bytes mapped past the file's end with no page behind them: any offset a 32-bit field can name.
#[cfg(not(any(kani, miri)))]
const GUARD: usize = 1 << 32;

/// The smallest file kept, a page, so that most calls reuse one.
#[cfg(not(any(kani, miri)))]
const SMALLEST: usize = 1 << 12;

#[cfg(not(any(kani, miri)))]
mod memory {
    use std::cell::RefCell;
    use std::os::fd::OwnedFd;

    use memmap2::{MmapOptions, MmapRaw};
    use rustix::fs::{MemfdFlags, SealFlags};
    use rustix::io::Errno;

    use super::{GUARD, SMALLEST};

    /// A sealed memfd of `length` bytes, mapped with `GUARD` unbacked bytes after them.
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
            rustix::fs::ftruncate(&file, length as u64)?;
            rustix::fs::fcntl_add_seals(&file, SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL)?;
            let mapped = length.checked_add(GUARD).ok_or(Errno::NOMEM)?;
            let map = MmapOptions::new()
                .len(mapped)
                .map_raw(&file)
                .map_err(|error| Errno::from_io_error(&error).unwrap_or(Errno::NOMEM))?;
            Ok(Self { file, map, length })
        }

        pub(super) fn length(&self) -> usize {
            self.length
        }

        pub(super) fn start(&self) -> *mut u8 {
            self.map.as_mut_ptr()
        }

        pub(super) fn write(&self, at: usize, bytes: &[u8]) -> Result<(), Errno> {
            let written = rustix::io::pwrite(&self.file, bytes, at as u64)?;
            if written == bytes.len() { Ok(()) } else { Err(Errno::IO) }
        }

        pub(super) fn read(&self, at: usize, into: &mut [u8]) -> Result<(), Errno> {
            let read = rustix::io::pread(&self.file, &mut *into, at as u64)?;
            if read == into.len() { Ok(()) } else { Err(Errno::IO) }
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
        lent.write(usize::MIN, &vec![u8::MIN; length])?;
        call(&lent)
    })
}
