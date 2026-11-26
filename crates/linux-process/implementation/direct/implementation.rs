//! The calls and instructions themselves, on this host: each use of `unsafe` is one marked line.
#![cfg(target_os = "linux")]

use core::ffi::c_void;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicU32, AtomicU64};

use rustix::fs::{MemfdFlags, SealFlags};
use rustix::mm::{MapFlags, ProtFlags};
use wcontainers_linux_process::{Errno, LinuxProcess, Pid, Signal};
#[cfg(target_arch = "aarch64")]
use wcontainers_linux_process::{LARGEST_PREDICATE_BYTES, PredicatesAfterSystemCall};

// The probe's system call lets Linux zero the bits of z0 to z31 above their low 128, which its assembly does not name
// as changed. That is sound only while the compiler keeps nothing there: while this crate is built without SVE.
#[cfg(all(target_arch = "aarch64", target_feature = "sve"))]
compile_error!(
    "the SVE probe's system call may clear Z register bits that code built with SVE keeps values in"
);

/// This process's own calls.
#[derive(Clone, Copy, Debug, Default)]
pub struct DirectLinuxProcess;

/// A page mapped at an address this process chose.
#[derive(Debug)]
pub struct DirectFixedMapping {
    address: usize,
    length: usize,
}

/// A sealed memory file and this process's shared mapping of it.
#[derive(Debug)]
pub struct DirectSharedWords {
    file: OwnedFd,
    mapping: memmap2::MmapRaw,
}

impl LinuxProcess for DirectLinuxProcess {
    type FixedMapping = DirectFixedMapping;
    type SharedWords = DirectSharedWords;

    fn tgkill(&self, process: Pid, thread: Pid, signal: Signal) -> Result<(), Errno> {
        #[expect(
            unsafe_code,
            reason = "glibc's tgkill takes only numbers; neither rustix nor nix sends a signal to one thread of another process"
        )]
        // SAFETY: plain integers in, an integer out.
        let sent = unsafe {
            libc::tgkill(
                process.as_raw_nonzero().get(),
                thread.as_raw_nonzero().get(),
                signal.as_raw(),
            )
        };
        if sent == 0 {
            Ok(())
        } else {
            Err(Errno::from_io_error(&std::io::Error::last_os_error()).unwrap_or(Errno::INVAL))
        }
    }

    fn mmap_fixed_noreplace(
        &self,
        address: usize,
        length: usize,
    ) -> Result<DirectFixedMapping, Errno> {
        let flags = MapFlags::PRIVATE | MapFlags::FIXED_NOREPLACE | MapFlags::NORESERVE;
        #[expect(
            unsafe_code,
            reason = "maps a fixed address, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: FIXED_NOREPLACE never replaces a mapping, and the page made here is inaccessible and held only by the
        // handle, which unmaps it.
        let mapped = unsafe {
            rustix::mm::mmap_anonymous(address as *mut c_void, length, ProtFlags::empty(), flags)
        }?;
        let mapping = DirectFixedMapping {
            address: mapped.addr(),
            length,
        };
        // Linux before 4.17 reads FIXED_NOREPLACE as a hint and may map elsewhere: that page is unmapped again.
        if mapping.address == address {
            Ok(mapping)
        } else {
            Err(Errno::NOSYS)
        }
    }

    fn create_shared_words(&self, name: &str, length: usize) -> Result<DirectSharedWords, Errno> {
        // As mmap(2), which maps no empty range.
        if length == 0 {
            return Err(Errno::INVAL);
        }
        let file = rustix::fs::memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)?;
        rustix::fs::ftruncate(&file, length as u64)?;
        // A file that cannot shrink: no other holder makes a word of the mapping fault.
        rustix::fs::fcntl_add_seals(&file, SealFlags::SHRINK | SealFlags::SEAL)?;
        let mapping = memmap2::MmapOptions::new()
            .len(length)
            .map_raw(&file)
            .map_err(|error| Errno::from_io_error(&error).unwrap_or(Errno::NOMEM))?;
        Ok(DirectSharedWords { file, mapping })
    }

    fn shared_words_file<'words>(&self, words: &'words DirectSharedWords) -> BorrowedFd<'words> {
        words.file.as_fd()
    }

    fn shared_words32<'words>(&self, words: &'words DirectSharedWords) -> &'words [AtomicU32] {
        #[expect(
            unsafe_code,
            reason = "views memory another process writes as atomics, which no safe call can give"
        )]
        // SAFETY: the mapping is page-aligned, readable and writable, as long as its file, which cannot shrink, and
        // lives as long as `words`; this process reaches it only through atomics, and another process's accesses are
        // not this one's memory model's.
        unsafe {
            core::slice::from_raw_parts(
                words.mapping.as_ptr().cast::<AtomicU32>(),
                words.mapping.len() / size_of::<AtomicU32>(),
            )
        }
    }

    fn shared_words64<'words>(&self, words: &'words DirectSharedWords) -> &'words [AtomicU64] {
        #[expect(
            unsafe_code,
            reason = "views memory another process writes as atomics, which no safe call can give"
        )]
        // SAFETY: as `shared_words32`.
        unsafe {
            core::slice::from_raw_parts(
                words.mapping.as_ptr().cast::<AtomicU64>(),
                words.mapping.len() / size_of::<AtomicU64>(),
            )
        }
    }

    #[cfg(target_arch = "aarch64")]
    fn read_ctr_el0(&self) -> u64 {
        let value: u64;
        #[expect(
            unsafe_code,
            reason = "reads CTR_EL0, which Linux lets user space read; no crate wraps it"
        )]
        // SAFETY: the read has no effect beyond its output register.
        unsafe {
            core::arch::asm!("mrs {}, ctr_el0", out(reg) value, options(nomem, nostack, preserves_flags));
        }
        value
    }

    #[cfg(target_arch = "aarch64")]
    fn dc_cvau(&self, address: usize) {
        #[expect(
            unsafe_code,
            reason = "cache maintenance by address is an instruction no crate wraps"
        )]
        // SAFETY: cleaning a line changes no memory's value; at an address not mapped it faults as an access does.
        unsafe {
            core::arch::asm!("dc cvau, {}", in(reg) address, options(nostack, preserves_flags));
        }
    }

    #[cfg(target_arch = "aarch64")]
    fn ic_ivau(&self, address: usize) {
        #[expect(
            unsafe_code,
            reason = "cache maintenance by address is an instruction no crate wraps"
        )]
        // SAFETY: invalidating instruction lines changes no memory's value; at an address not mapped it faults.
        unsafe {
            core::arch::asm!("ic ivau, {}", in(reg) address, options(nostack, preserves_flags));
        }
    }

    #[cfg(target_arch = "aarch64")]
    fn predicates_after_system_call(&self) -> Option<PredicatesAfterSystemCall> {
        if !std::arch::is_aarch64_feature_detected!("sve") {
            return None;
        }
        // On a thread of its own, which ends with whatever SVE state the call left.
        let probe = std::thread::spawn(|| {
            let mut p0 = [0_u8; LARGEST_PREDICATE_BYTES];
            let mut ffr = [0_u8; LARGEST_PREDICATE_BYTES];
            #[expect(
                unsafe_code,
                reason = "SVE state across a system call can only be set and read by its own instructions"
            )]
            // SAFETY: writes at most one predicate register's bytes to each buffer, and touches only clobbered
            // registers and the Z bits above 128, which code built without SVE never keeps (the `compile_error!`
            // above).
            unsafe {
                core::arch::asm!(
                    ".arch_extension sve",
                    "ptrue p0.b",
                    "setffr",
                    "svc #0",
                    "str p0, [{p0}]",
                    "rdffr p1.b",
                    "str p1, [{ffr}]",
                    p0 = in(reg) p0.as_mut_ptr(),
                    ffr = in(reg) ffr.as_mut_ptr(),
                    inout("x8") u64::from(linux_raw_sys::general::__NR_getppid) => _,
                    out("x0") _,
                    out("p0") _,
                    out("p1") _,
                    out("ffr") _,
                    options(nostack),
                );
            }
            PredicatesAfterSystemCall { p0, ffr }
        });
        // A probe thread that could not finish leaves the state unknown: as if every bit were kept.
        Some(probe.join().unwrap_or(PredicatesAfterSystemCall {
            p0: [u8::MAX; LARGEST_PREDICATE_BYTES],
            ffr: [u8::MAX; LARGEST_PREDICATE_BYTES],
        }))
    }
}

impl Drop for DirectFixedMapping {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "unmaps the page this handle alone holds")]
        // SAFETY: the page is the handle's alone, inaccessible, and nothing refers to it.
        let unmapped = unsafe { rustix::mm::munmap(self.address as *mut c_void, self.length) };
        // An unmap of a whole mapping this process made fails only on bad arguments, which the map refused.
        debug_assert!(unmapped.is_ok(), "a fixed page is unmapped");
    }
}
