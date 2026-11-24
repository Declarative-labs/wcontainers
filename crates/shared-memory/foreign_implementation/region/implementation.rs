//! Memory outside every Rust allocation, reached only by the assembly copies, volatile loads and stores and atomic
//! instructions here, and never through a reference or a slice.
//!
//! The design rests on one sentence of the standard library's documentation of `core::ptr::read_volatile` (1.100.0,
//! library/core/src/ptr/mod.rs:2078): "Volatile operations, however, may also be used to access memory that is
//! _outside_ of any Rust allocation." Whether a program may treat memory it mapped itself as outside every allocation is
//! not settled yet (https://github.com/rust-lang/unsafe-code-guidelines/issues/607,
//! https://github.com/rust-lang/unsafe-code-guidelines/issues/543); the assembly reads and writes it as a run of such
//! volatile accesses would.

use core::sync::atomic::{AtomicPtr, Ordering, fence};

use crate::{OutsideError, Word, WordError, arithmetic};

// TODO: remove the assembly once Rust has volatile atomic operations (https://github.com/rust-lang/rust/issues/158947) and a copy that tolerates races (https://github.com/rust-lang/rfcs/pull/3301, https://github.com/rust-lang/rfcs/pull/2728).
#[cfg(all(target_arch = "x86_64", not(any(miri, kani))))]
#[path = "x86_64.rs"]
mod machine;
#[cfg(all(target_arch = "aarch64", not(any(miri, kani))))]
#[path = "aarch64.rs"]
mod machine;
// On any other architecture, and under Miri and Kani, which run no assembly, a plain volatile fallback: there a swap or
// a compare-and-exchange is a load and a store, not one atomic access.
#[cfg(not(all(
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(any(miri, kani))
)))]
#[path = "fallback.rs"]
mod machine;

/// The bytes of a mapping; made only by `new`, whose callers promise what every method here relies on.
pub(crate) struct Region {
    /// The first byte; an `AtomicPtr` only because a raw pointer is neither `Send` nor `Sync`, and never stored after
    /// `new`.
    base: AtomicPtr<u8>,
    length: usize,
}

impl Region {
    /// # Safety
    /// While the value lives, the `length` bytes from `base` stay mapped, readable and writable, and reaching them never
    /// traps; `base` is aligned to 8; and no reference or slice over any of them is ever formed. They lie outside every
    /// Rust allocation, except under Miri and Kani, where they are a heap allocation that only this value reaches.
    #[expect(
        unsafe_code,
        reason = "the promise each mapping makes, on which every access here relies"
    )]
    pub(crate) unsafe fn new(base: *mut u8, length: usize) -> Region {
        Region {
            base: AtomicPtr::new(base),
            length,
        }
    }

    pub(crate) fn len(&self) -> u64 {
        self.length as u64
    }

    pub(crate) fn read(&self, at: u64, into: &mut [u8]) -> Result<(), OutsideError> {
        let from = self.byte(arithmetic::bytes(self.length, at, into.len())?);
        #[expect(unsafe_code, reason = "the copy out of the region")]
        // SAFETY: the bytes from `from` lie inside the region (checked above), which `new`'s promise keeps readable and
        // outside every allocation; the buffer is this call's own, and so cannot overlap them.
        unsafe {
            machine::copy(into.as_mut_ptr(), from, into.len());
        }
        Ok(())
    }

    pub(crate) fn write(&self, at: u64, from: &[u8]) -> Result<(), OutsideError> {
        let to = self.byte(arithmetic::bytes(self.length, at, from.len())?);
        #[expect(unsafe_code, reason = "the copy into the region")]
        // SAFETY: the bytes from `to` lie inside the region (checked above), which `new`'s promise keeps writable and
        // outside every allocation; the buffer is this call's own, and so cannot overlap them.
        unsafe {
            machine::copy(to, from.as_ptr(), from.len());
        }
        Ok(())
    }

    pub(crate) fn load<W: Word>(&self, at: u64) -> Result<W, WordError> {
        let word = self.word::<W>(at)?;
        #[expect(unsafe_code, reason = "a volatile load of a word of the region")]
        // SAFETY: the word lies inside the region and is aligned (checked by `word`); `W` is `u32` or `u64`, for which
        // any bytes are a value.
        let value = unsafe { word.read_volatile() };
        fence(Ordering::Acquire);
        Ok(value)
    }

    pub(crate) fn store<W: Word>(&self, at: u64, value: W) -> Result<(), WordError> {
        let word = self.word::<W>(at)?;
        fence(Ordering::Release);
        #[expect(unsafe_code, reason = "a volatile store to a word of the region")]
        // SAFETY: the word lies inside the region and is aligned (checked by `word`).
        unsafe {
            word.write_volatile(value);
        }
        Ok(())
    }

    pub(crate) fn swap<W: Word>(&self, at: u64, value: W) -> Result<W, WordError> {
        let word = self.word::<W>(at)?;
        let value: u64 = value.into();
        #[expect(unsafe_code, reason = "the processor's swap on a word of the region")]
        // SAFETY: the word lies inside the region and is aligned (checked by `word`), and `W` is the width chosen.
        let old = unsafe {
            if size_of::<W>() == size_of::<u32>() {
                u64::from(machine::swap_u32(word.cast(), value as u32))
            } else {
                machine::swap_u64(word.cast(), value)
            }
        };
        Ok(narrow(old))
    }

    pub(crate) fn compare_exchange<W: Word>(
        &self,
        at: u64,
        current: W,
        new: W,
    ) -> Result<W, WordError> {
        let word = self.word::<W>(at)?;
        let (current, new): (u64, u64) = (current.into(), new.into());
        #[expect(
            unsafe_code,
            reason = "the processor's compare-and-exchange on a word of the region"
        )]
        // SAFETY: the word lies inside the region and is aligned (checked by `word`), and `W` is the width chosen.
        let old = unsafe {
            if size_of::<W>() == size_of::<u32>() {
                u64::from(machine::compare_exchange_u32(
                    word.cast(),
                    current as u32,
                    new as u32,
                ))
            } else {
                machine::compare_exchange_u64(word.cast(), current, new)
            }
        };
        Ok(narrow(old))
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn wait(
        &self,
        at: u64,
        expected: u32,
        timeout: Option<core::time::Duration>,
    ) -> Result<(), crate::WaitError> {
        let word = self.word::<u32>(at)?;
        let timeout = timeout.and_then(|timeout| {
            Some(libc::timespec {
                tv_sec: timeout.as_secs().try_into().ok()?,
                // Below a second's nanoseconds, so it fits 32 bits.
                tv_nsec: libc::c_long::from(timeout.subsec_nanos() as i32),
            })
        });
        match futex(word, libc::FUTEX_WAIT, expected, timeout.as_ref()) {
            Ok(_)
            | Err(
                rustix::io::Errno::AGAIN | rustix::io::Errno::INTR | rustix::io::Errno::TIMEDOUT,
            ) => Ok(()),
            Err(errno) => Err(crate::WaitError::Refused(errno)),
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn wake(&self, at: u64, limit: u32) -> Result<u32, crate::WaitError> {
        let word = self.word::<u32>(at)?;
        let woken =
            futex(word, libc::FUTEX_WAKE, limit, None).map_err(crate::WaitError::Refused)?;
        Ok(u32::try_from(woken).unwrap_or(u32::MAX))
    }

    /// Returns the address of the byte at the index, which the caller checked lies inside.
    fn byte(&self, index: usize) -> *mut u8 {
        self.base.load(Ordering::Relaxed).wrapping_add(index)
    }

    /// Returns the address of the word at the offset, if it lies inside and is aligned to its size.
    fn word<W: Word>(&self, at: u64) -> Result<*mut W, WordError> {
        Ok(self
            .byte(arithmetic::word(self.length, at, size_of::<W>())?)
            .cast())
    }
}

/// Returns the word the value came from, which is no wider than it.
fn narrow<W: Word>(value: u64) -> W {
    W::try_from(value)
        .unwrap_or_else(|_| unreachable!("the value came from a word of the same size"))
}

/// The futex system call on the 4 bytes at the address, which rustix takes only as a reference; returns the number the
/// call returns, which for a wake is the number of threads woken.
#[cfg(target_os = "linux")]
pub(crate) fn futex(
    word: *mut u32,
    operation: libc::c_int,
    value: u32,
    timeout: Option<&libc::timespec>,
) -> Result<usize, rustix::io::Errno> {
    let timeout = timeout.map_or(core::ptr::null(), core::ptr::from_ref);
    #[expect(
        unsafe_code,
        reason = "the futex system call, on an address the kernel itself checks"
    )]
    // SAFETY: the kernel reads the 4 bytes at `word` and the timeout, which is null or lives for the call, and writes no
    // memory of this program; an address it cannot read gives EFAULT.
    let result = unsafe {
        libc::syscall(
            libc::SYS_futex,
            word,
            operation,
            value,
            timeout,
            core::ptr::null::<u32>(),
            0_u32,
        )
    };
    usize::try_from(result).map_err(|_| {
        rustix::io::Errno::from_raw_os_error(
            std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EINVAL),
        )
    })
}
