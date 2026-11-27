//! The calls and instructions themselves, on this host: each use of `unsafe` is one marked line.
#![cfg(target_os = "linux")]

use core::ffi::c_void;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicU32, AtomicU64};

use rustix::fs::{MemfdFlags, SealFlags};
use rustix::mm::{MapFlags, MprotectFlags};
#[cfg(target_arch = "aarch64")]
use wcontainers_linux_process::{
    Arm64Registers, EXIT_BYTES, EXIT_SCRATCH_REGISTER, FRAME_REGISTER, LARGEST_PREDICATE_BYTES,
    PredicatesAfterSystemCall, RunFrame,
};
use wcontainers_linux_process::{Errno, LinuxProcess, Pid, ProtFlags, Signal};

/// The link register, x30.
#[cfg(target_arch = "aarch64")]
const LINK: u32 = 30;
/// The encodings of the instructions an exit is made of (Arm ARM, C6.2): STP (64-bit, signed offset), LDR
/// (immediate, 64-bit, unsigned offset), BLR and UDF, each with its fields clear.
#[cfg(target_arch = "aarch64")]
const STORE_PAIR: u32 = 0xa900_0000;
#[cfg(target_arch = "aarch64")]
const LOAD: u32 = 0xf940_0000;
#[cfg(target_arch = "aarch64")]
const BRANCH_WITH_LINK: u32 = 0xd63f_0000;
#[cfg(target_arch = "aarch64")]
const UNDEFINED: u32 = 0;
/// Where those fields lie: the second register, the base, the scaled immediates.
#[cfg(target_arch = "aarch64")]
const SECOND_REGISTER_SHIFT: u32 = 10;
#[cfg(target_arch = "aarch64")]
const BASE_SHIFT: u32 = 5;
#[cfg(target_arch = "aarch64")]
const PAIR_OFFSET_SHIFT: u32 = 15;
#[cfg(target_arch = "aarch64")]
const LOAD_OFFSET_SHIFT: u32 = 10;
/// The run's words: x23 and x30 as an exit found them, where exits go, and the thread's stack.
#[cfg(target_arch = "aarch64")]
const EXIT_AT: usize = core::mem::offset_of!(RunFrame, exit);
#[cfg(target_arch = "aarch64")]
const EXITS_GO_AT: usize = EXIT_AT + 2 * size_of::<u64>();
#[cfg(target_arch = "aarch64")]
const THREAD_STACK_AT: usize = EXIT_AT + 3 * size_of::<u64>();
/// The bytes of an exit before x30 is the address after its branch.
#[cfg(target_arch = "aarch64")]
const EXIT_LINKED: usize = 3 * size_of::<u32>();
/// The registers in the frame, and each part of them.
#[cfg(target_arch = "aarch64")]
const REGISTERS_AT: usize = core::mem::offset_of!(RunFrame, registers);
#[cfg(target_arch = "aarch64")]
const V_AT: usize = core::mem::offset_of!(Arm64Registers, v);
#[cfg(target_arch = "aarch64")]
const X_AT: usize = core::mem::offset_of!(Arm64Registers, x);
#[cfg(target_arch = "aarch64")]
const SP_AT: usize = core::mem::offset_of!(Arm64Registers, sp);
#[cfg(target_arch = "aarch64")]
const PC_AT: usize = core::mem::offset_of!(Arm64Registers, pc);
#[cfg(target_arch = "aarch64")]
const NZCV_AT: usize = core::mem::offset_of!(Arm64Registers, nzcv);
#[cfg(target_arch = "aarch64")]
const FPSR_AT: usize = core::mem::offset_of!(Arm64Registers, fpsr);
#[cfg(target_arch = "aarch64")]
const FPCR_AT: usize = core::mem::offset_of!(Arm64Registers, fpcr);
#[cfg(target_arch = "aarch64")]
const TPIDR_AT: usize = core::mem::offset_of!(Arm64Registers, tpidr_el0);
/// PSTATE's condition flags, bits 31 to 28: all `msr nzcv` writes.
#[cfg(target_arch = "aarch64")]
const NZCV: u64 = 0xf000_0000;
/// What a run keeps of the thread on its stack: x19 to x30, d8 to d15 (the registers a call keeps, AAPCS64 6.1.1
/// and 6.1.2), FPCR and TPIDR_EL0.
#[cfg(target_arch = "aarch64")]
const KEPT_BYTES: usize = (12 + 8 + 2) * size_of::<u64>();
#[cfg(target_arch = "aarch64")]
const KEPT_D_AT: usize = 12 * size_of::<u64>();
#[cfg(target_arch = "aarch64")]
const KEPT_SYSTEM_AT: usize = 20 * size_of::<u64>();

#[cfg(target_arch = "aarch64")]
const _: () = assert!(
    EXIT_AT.is_multiple_of(size_of::<u64>())
        && EXIT_AT / size_of::<u64>() < 1 << 6
        && REGISTERS_AT.is_multiple_of(2 * size_of::<u64>())
        && KEPT_BYTES.is_multiple_of(2 * size_of::<u64>())
);

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

/// Code mapped from a memory file sealed against writes, at an address this process chose.
#[derive(Debug)]
pub struct DirectCodeMapping {
    _mapping: DirectFixedMapping,
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
    type CodeMapping = DirectCodeMapping;

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

    fn map_file_within(
        &self,
        mapping: &mut DirectFixedMapping,
        offset: usize,
        length: usize,
        file: BorrowedFd<'_>,
        file_offset: u64,
        protection: ProtFlags,
    ) -> Result<(), Errno> {
        let address = mapping.pages_within(offset, length, protection)?;
        #[expect(
            unsafe_code,
            reason = "replaces pages at a fixed address, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: the pages lie inside the fixed mapping, which the handle alone holds (`&mut`) and through which no
        // reference to this process's memory is made, so replacing them changes nothing any reference here points to;
        // they are not executable.
        let mapped = unsafe {
            rustix::mm::mmap(
                address as *mut c_void,
                length,
                protection,
                MapFlags::SHARED | MapFlags::FIXED,
                file,
                file_offset,
            )
        }?;
        debug_assert_eq!(mapped.addr(), address, "a fixed mapping lands where asked");
        Ok(())
    }

    fn protect_within(
        &self,
        mapping: &mut DirectFixedMapping,
        offset: usize,
        length: usize,
        protection: ProtFlags,
    ) -> Result<(), Errno> {
        let address = mapping.pages_within(offset, length, protection)?;
        #[expect(
            unsafe_code,
            reason = "changes the protection at an address, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: as `map_file_within`: pages of the handle's own mapping, which no reference points into, made no
        // more than readable and writable.
        unsafe {
            rustix::mm::mprotect(
                address as *mut c_void,
                length,
                MprotectFlags::from_bits_truncate(protection.bits()),
            )
        }
    }

    fn map_code(&self, address: usize, code: &[u8]) -> Result<DirectCodeMapping, Errno> {
        let page = rustix::param::page_size();
        if code.is_empty() || !code.len().is_multiple_of(page) || !address.is_multiple_of(page) {
            return Err(Errno::INVAL);
        }
        let file = rustix::fs::memfd_create(
            "wcontainers code",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )?;
        rustix::fs::ftruncate(&file, code.len() as u64)?;
        let mut written = 0;
        while written < code.len() {
            written += rustix::io::pwrite(&file, &code[written..], written as u64)?;
        }
        // No one, this process included, writes the bytes again: only then may they run.
        rustix::fs::fcntl_add_seals(
            &file,
            SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL,
        )?;
        let flags = MapFlags::SHARED | MapFlags::FIXED_NOREPLACE;
        let protection = ProtFlags::READ | ProtFlags::EXEC;
        #[expect(
            unsafe_code,
            reason = "maps a fixed address, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: FIXED_NOREPLACE never replaces a mapping, and the pages made here are held only by the handle, which
        // unmaps them; their file is sealed against writes, so no reference sees them change.
        let mapped = unsafe {
            rustix::mm::mmap(
                address as *mut c_void,
                code.len(),
                protection,
                flags,
                &file,
                0,
            )
        }?;
        let mapping = DirectFixedMapping {
            address: mapped.addr(),
            length: code.len(),
        };
        // As `mmap_fixed_noreplace`: an old Linux maps elsewhere, and that mapping is unmapped again.
        if mapping.address == address {
            Ok(DirectCodeMapping { _mapping: mapping })
        } else {
            Err(Errno::NOSYS)
        }
    }

    #[cfg(target_arch = "aarch64")]
    fn exit_code(&self) -> [u8; EXIT_BYTES] {
        let frame = u32::from(FRAME_REGISTER) << BASE_SHIFT;
        let scratch = u32::from(EXIT_SCRATCH_REGISTER);
        let words = [
            STORE_PAIR
                | ((EXIT_AT / size_of::<u64>()) as u32) << PAIR_OFFSET_SHIFT
                | LINK << SECOND_REGISTER_SHIFT
                | frame
                | scratch,
            LOAD | ((EXITS_GO_AT / size_of::<u64>()) as u32) << LOAD_OFFSET_SHIFT | frame | scratch,
            BRANCH_WITH_LINK | scratch << BASE_SHIFT,
            UNDEFINED,
        ];
        let mut bytes = [0; EXIT_BYTES];
        for (into, word) in bytes
            .as_chunks_mut::<{ size_of::<u32>() }>()
            .0
            .iter_mut()
            .zip(words)
        {
            *into = word.to_le_bytes();
        }
        bytes
    }

    #[cfg(target_arch = "aarch64")]
    fn run_code(&self, frame: &mut RunFrame) -> u64 {
        entering::run(core::ptr::from_mut(frame))
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

impl DirectFixedMapping {
    /// The address of the pages `offset..offset + length` of the mapping: `EINVAL` unless they are whole pages inside
    /// it, `EPERM` for an executable protection.
    fn pages_within(
        &self,
        offset: usize,
        length: usize,
        protection: ProtFlags,
    ) -> Result<usize, Errno> {
        if protection.contains(ProtFlags::EXEC) {
            return Err(Errno::PERM);
        }
        let page = rustix::param::page_size();
        let end = offset.checked_add(length).ok_or(Errno::INVAL)?;
        if length == 0
            || !offset.is_multiple_of(page)
            || !length.is_multiple_of(page)
            || end > self.length
        {
            return Err(Errno::INVAL);
        }
        Ok(self.address + offset)
    }
}

/// The run itself, a function of assembly alone.
#[cfg(target_arch = "aarch64")]
#[expect(
    unsafe_code,
    reason = "UNSOUND: a safe fn that runs whatever code the frame's pc names, with its own stack, so nothing but that code's promise (translated code reaches only its window and its frame) keeps this process's memory as Rust holds it; no safe form of entering other code exists"
)]
mod entering {
    use super::{
        EXIT_AT, EXIT_LINKED, EXITS_GO_AT, FPCR_AT, FPSR_AT, KEPT_BYTES, KEPT_D_AT, KEPT_SYSTEM_AT,
        NZCV, NZCV_AT, PC_AT, REGISTERS_AT, RunFrame, SP_AT, THREAD_STACK_AT, TPIDR_AT, V_AT, X_AT,
    };

    /// Runs the frame's code until it branches with link to an exit, as [`LinuxProcess::run_code`] says; returns the
    /// exit's address.
    #[unsafe(naked)]
    pub(super) extern "C" fn run(frame: *mut RunFrame) -> u64 {
        core::arch::naked_asm!(
            // The thread's own: the registers a call keeps, FPCR and TPIDR_EL0, on its stack, whose top the frame keeps.
            "sub sp, sp, #{kept}",
            "stp x19, x20, [sp, #0]",
            "stp x21, x22, [sp, #16]",
            "stp x23, x24, [sp, #32]",
            "stp x25, x26, [sp, #48]",
            "stp x27, x28, [sp, #64]",
            "stp x29, x30, [sp, #80]",
            "stp d8, d9, [sp, #{kept_d} + 0]",
            "stp d10, d11, [sp, #{kept_d} + 16]",
            "stp d12, d13, [sp, #{kept_d} + 32]",
            "stp d14, d15, [sp, #{kept_d} + 48]",
            "mrs x9, fpcr",
            "mrs x10, tpidr_el0",
            "stp x9, x10, [sp, #{kept_system}]",
            "adr x9, 2f",
            "mov x10, sp",
            "stp x9, x10, [x0, #{exits_go}]",
            // The code's: every register from the frame, x28 the frame, x23 the code's first address.
            "mov x28, x0",
            "add x9, x28, #{registers}",
            "ldp q0, q1, [x9, #{v} + 0]",
            "ldp q2, q3, [x9, #{v} + 32]",
            "ldp q4, q5, [x9, #{v} + 64]",
            "ldp q6, q7, [x9, #{v} + 96]",
            "ldp q8, q9, [x9, #{v} + 128]",
            "ldp q10, q11, [x9, #{v} + 160]",
            "ldp q12, q13, [x9, #{v} + 192]",
            "ldp q14, q15, [x9, #{v} + 224]",
            "ldp q16, q17, [x9, #{v} + 256]",
            "ldp q18, q19, [x9, #{v} + 288]",
            "ldp q20, q21, [x9, #{v} + 320]",
            "ldp q22, q23, [x9, #{v} + 352]",
            "ldp q24, q25, [x9, #{v} + 384]",
            "ldp q26, q27, [x9, #{v} + 416]",
            "ldp q28, q29, [x9, #{v} + 448]",
            "ldp q30, q31, [x9, #{v} + 480]",
            "ldr x10, [x9, #{fpsr}]",
            "msr fpsr, x10",
            "ldr x10, [x9, #{fpcr}]",
            "msr fpcr, x10",
            "ldr x10, [x9, #{tpidr}]",
            "msr tpidr_el0, x10",
            "ldr x10, [x9, #{nzcv}]",
            "and x10, x10, #{nzcv_bits}",
            "msr nzcv, x10",
            "ldr x10, [x9, #{sp}]",
            "mov sp, x10",
            "ldr x23, [x9, #{pc}]",
            "add x9, x9, #{x}",
            "ldp x0, x1, [x9, #0]",
            "ldp x2, x3, [x9, #16]",
            "ldp x4, x5, [x9, #32]",
            "ldp x6, x7, [x9, #48]",
            "ldp x10, x11, [x9, #80]",
            "ldp x12, x13, [x9, #96]",
            "ldp x14, x15, [x9, #112]",
            "ldp x16, x17, [x9, #128]",
            "ldp x18, x19, [x9, #144]",
            "ldp x20, x21, [x9, #160]",
            "ldr x22, [x9, #176]",
            "ldp x24, x25, [x9, #192]",
            "ldp x26, x27, [x9, #208]",
            "ldr x29, [x9, #232]",
            "ldr x30, [x9, #240]",
            "ldr x8, [x9, #64]",
            "ldr x9, [x9, #72]",
            "br x23",
            // An exit: the code's x23 and x30 in the frame's words, x30 the address after the exit's branch.
            "2:",
            "add x23, x28, #{registers} + {x}",
            "stp x0, x1, [x23, #0]",
            "stp x2, x3, [x23, #16]",
            "stp x4, x5, [x23, #32]",
            "stp x6, x7, [x23, #48]",
            "stp x8, x9, [x23, #64]",
            "stp x10, x11, [x23, #80]",
            "stp x12, x13, [x23, #96]",
            "stp x14, x15, [x23, #112]",
            "stp x16, x17, [x23, #128]",
            "stp x18, x19, [x23, #144]",
            "stp x20, x21, [x23, #160]",
            "str x22, [x23, #176]",
            "stp x24, x25, [x23, #192]",
            "stp x26, x27, [x23, #208]",
            "stp x28, x29, [x23, #224]",
            "ldp x0, x1, [x28, #{exit}]",
            "str x0, [x23, #184]",
            "str x1, [x23, #240]",
            "add x23, x28, #{registers}",
            "str x1, [x23, #{pc}]",
            "mov x0, sp",
            "str x0, [x23, #{sp}]",
            "mrs x0, nzcv",
            "str x0, [x23, #{nzcv}]",
            "mrs x0, fpsr",
            "str x0, [x23, #{fpsr}]",
            "mrs x0, fpcr",
            "str x0, [x23, #{fpcr}]",
            "mrs x0, tpidr_el0",
            "str x0, [x23, #{tpidr}]",
            "stp q0, q1, [x23, #{v} + 0]",
            "stp q2, q3, [x23, #{v} + 32]",
            "stp q4, q5, [x23, #{v} + 64]",
            "stp q6, q7, [x23, #{v} + 96]",
            "stp q8, q9, [x23, #{v} + 128]",
            "stp q10, q11, [x23, #{v} + 160]",
            "stp q12, q13, [x23, #{v} + 192]",
            "stp q14, q15, [x23, #{v} + 224]",
            "stp q16, q17, [x23, #{v} + 256]",
            "stp q18, q19, [x23, #{v} + 288]",
            "stp q20, q21, [x23, #{v} + 320]",
            "stp q22, q23, [x23, #{v} + 352]",
            "stp q24, q25, [x23, #{v} + 384]",
            "stp q26, q27, [x23, #{v} + 416]",
            "stp q28, q29, [x23, #{v} + 448]",
            "stp q30, q31, [x23, #{v} + 480]",
            // Back to the thread: its stack, its own registers, and the exit's address.
            "sub x0, x30, #{exit_linked}",
            "ldr x9, [x28, #{thread_stack}]",
            "mov sp, x9",
            "ldp x9, x10, [sp, #{kept_system}]",
            "msr fpcr, x9",
            "msr tpidr_el0, x10",
            "ldp d8, d9, [sp, #{kept_d} + 0]",
            "ldp d10, d11, [sp, #{kept_d} + 16]",
            "ldp d12, d13, [sp, #{kept_d} + 32]",
            "ldp d14, d15, [sp, #{kept_d} + 48]",
            "ldp x19, x20, [sp, #0]",
            "ldp x21, x22, [sp, #16]",
            "ldp x23, x24, [sp, #32]",
            "ldp x25, x26, [sp, #48]",
            "ldp x27, x28, [sp, #64]",
            "ldp x29, x30, [sp, #80]",
            "add sp, sp, #{kept}",
            "ret",
            kept = const KEPT_BYTES,
            kept_d = const KEPT_D_AT,
            kept_system = const KEPT_SYSTEM_AT,
            exits_go = const EXITS_GO_AT,
            exit = const EXIT_AT,
            thread_stack = const THREAD_STACK_AT,
            exit_linked = const EXIT_LINKED,
            registers = const REGISTERS_AT,
            v = const V_AT,
            x = const X_AT,
            sp = const SP_AT,
            pc = const PC_AT,
            nzcv = const NZCV_AT,
            nzcv_bits = const NZCV,
            fpsr = const FPSR_AT,
            fpcr = const FPCR_AT,
            tpidr = const TPIDR_AT,
        )
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
