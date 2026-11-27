//! The calls and instructions themselves, on this host: each use of `unsafe` is one marked line.
#![cfg(target_os = "linux")]

#[cfg(target_arch = "aarch64")]
use core::ffi::c_int;
use core::ffi::c_void;
#[cfg(target_arch = "aarch64")]
use core::marker::PhantomData;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
#[cfg(target_arch = "aarch64")]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, AtomicU64};

use rustix::fs::{MemfdFlags, SealFlags};
use rustix::mm::{MapFlags, MprotectFlags};
#[cfg(target_arch = "aarch64")]
use wcontainers_linux_process::{
    Arm64Registers, EXIT_BYTES, EXIT_SCRATCH_REGISTER, FRAME_REGISTER, GENERAL_REGISTERS,
    LARGEST_PREDICATE_BYTES, PredicatesAfterSystemCall, RUN_STOP_SIGNAL, RunEnd, RunFrame,
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
/// The run's words: x23 and x30 as an exit found them, where exits go, the thread's stack, its signal stack's top
/// record, and how the run ended: the signal (none for an exit), the exit's or the fault's address, and what more.
#[cfg(target_arch = "aarch64")]
const EXIT_AT: usize = core::mem::offset_of!(RunFrame, exit);
#[cfg(target_arch = "aarch64")]
const EXITS_GO: usize = 2;
#[cfg(target_arch = "aarch64")]
const THREAD_STACK: usize = 3;
#[cfg(target_arch = "aarch64")]
const SIGNAL_STACK: usize = 4;
#[cfg(target_arch = "aarch64")]
const END: usize = 5;
#[cfg(target_arch = "aarch64")]
const END_ADDRESS: usize = 6;
#[cfg(target_arch = "aarch64")]
const END_DETAIL: usize = 7;
#[cfg(target_arch = "aarch64")]
const EXITS_GO_AT: usize = EXIT_AT + EXITS_GO * size_of::<u64>();
#[cfg(target_arch = "aarch64")]
const THREAD_STACK_AT: usize = EXIT_AT + THREAD_STACK * size_of::<u64>();
#[cfg(target_arch = "aarch64")]
const SIGNAL_STACK_AT: usize = EXIT_AT + SIGNAL_STACK * size_of::<u64>();
#[cfg(target_arch = "aarch64")]
const END_AT: usize = EXIT_AT + END * size_of::<u64>();
#[cfg(target_arch = "aarch64")]
const END_DETAIL_AT: usize = EXIT_AT + END_DETAIL * size_of::<u64>();
/// What more a run's end says: the code had not started; the access was a write; it faulted for its alignment.
#[cfg(target_arch = "aarch64")]
const NOT_ENTERED: u64 = 1;
#[cfg(target_arch = "aarch64")]
const WRITE: u64 = 1 << 1;
#[cfg(target_arch = "aarch64")]
const ALIGNMENT: u64 = 1 << 2;
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

/// Where the record at a signal stack's bottom keeps each word.
#[cfg(target_arch = "aarch64")]
const TOP_RUNNING_AT: usize = core::mem::offset_of!(SignalStackRecord, running);
#[cfg(target_arch = "aarch64")]
const TOP_PENDING_AT: usize = core::mem::offset_of!(SignalStackRecord, pending);
#[cfg(target_arch = "aarch64")]
const TOP_ENTRY_AT: usize = core::mem::offset_of!(SignalStackRecord, entry_first);
#[cfg(target_arch = "aarch64")]
const TOP_EXIT_AT: usize = core::mem::offset_of!(SignalStackRecord, exit_first);
#[cfg(target_arch = "aarch64")]
const TOP_LABELS_AT: usize = core::mem::offset_of!(SignalStackRecord, abort);
/// The signal stack's least size, before it grows with what Linux says a signal frame may take
/// ([AT_MINSIGSTKSZ](https://docs.kernel.org/arch/arm64/elf_hwcaps.html), a few times over for nested signals).
#[cfg(target_arch = "aarch64")]
const SIGNAL_STACK_BYTES: usize = 1 << 18;
#[cfg(target_arch = "aarch64")]
const SIGNAL_FRAMES: usize = 8;
/// The signals of a run, each a fault an instruction raises but the last, [`RUN_STOP_SIGNAL`].
#[cfg(target_arch = "aarch64")]
const RUN_SIGNALS: [c_int; 5] = [
    libc::SIGSEGV,
    libc::SIGBUS,
    libc::SIGILL,
    libc::SIGTRAP,
    RUN_STOP_SIGNAL.as_raw(),
];
/// The signals a run's thread leaves unblocked: a run's, and the one an arithmetic fault raises.
#[cfg(target_arch = "aarch64")]
const UNBLOCKED: [c_int; 6] = [
    libc::SIGSEGV,
    libc::SIGBUS,
    libc::SIGILL,
    libc::SIGTRAP,
    libc::SIGFPE,
    RUN_STOP_SIGNAL.as_raw(),
];
/// The [records](https://github.com/torvalds/linux/blob/master/arch/arm64/include/uapi/asm/sigcontext.h) after a
/// signal frame's registers: where they start, how many bytes they take, and the magic of the one holding ESR_EL1.
#[cfg(target_arch = "aarch64")]
const RECORDS_ALIGNMENT: usize = 16;
#[cfg(target_arch = "aarch64")]
const RECORDS_BYTES: usize = 4096;
#[cfg(target_arch = "aarch64")]
const ESR_MAGIC: u32 = 0x4553_5201;
/// [ESR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-)'s
/// exception class, its data aborts from a lower level, and the bit that says a data abort was a write (WnR).
#[cfg(target_arch = "aarch64")]
const EXCEPTION_CLASS_SHIFT: u32 = 26;
#[cfg(target_arch = "aarch64")]
const EXCEPTION_CLASS_MASK: u64 = 0x3f;
#[cfg(target_arch = "aarch64")]
const DATA_ABORT_LOWER: u64 = 0x24;
#[cfg(target_arch = "aarch64")]
const WRITE_NOT_READ: u64 = 1 << 6;

#[cfg(target_arch = "aarch64")]
const _: () = assert!(
    EXIT_AT.is_multiple_of(size_of::<u64>())
        && END_DETAIL < wcontainers_linux_process::RUN_EXIT_WORDS
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
    file: OwnedFd,
    _mapping: DirectFixedMapping,
}

/// A thread prepared to run code: its signal stack, and the stack and mask it had before.
#[cfg(target_arch = "aarch64")]
#[derive(Debug)]
pub struct DirectRunThread {
    record: usize,
    /// The earlier signal stack's bottom, flags and size.
    earlier_stack: (usize, c_int, usize),
    earlier_mask: u64,
    /// Below the stack, a page nothing may touch; then the record, then the stack.
    _stack: DirectFixedMapping,
    /// The signal stack and the mask are this thread's alone: the handle never moves to another.
    _one_thread: PhantomData<*const ()>,
}

/// What a run and its signals' handler share, at the bottom of the thread's signal stack, where the guest's code never
/// reaches: the frame while a run is on, a stop that came outside its code, and where each part of the run lies.
#[cfg(target_arch = "aarch64")]
#[repr(C)]
#[derive(Debug)]
struct SignalStackRecord {
    /// The record's own address, which tells it from a signal stack that is not this crate's.
    own: u64,
    running: u64,
    pending: u64,
    /// The run's first instruction and its branch to the code; its first instruction after the code and the end of
    /// its instructions; where a run stopped before its code goes, and where a handler that ends one sends it.
    entry_first: u64,
    entry_last: u64,
    exit_first: u64,
    exit_end: u64,
    abort: u64,
    saved: u64,
}

#[cfg(target_arch = "aarch64")]
std::thread_local! {
    /// Whether this thread is prepared to run code.
    static PREPARED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// The handlers this process had before a run's, one for each of [`RUN_SIGNALS`].
#[cfg(target_arch = "aarch64")]
static EARLIER_HANDLERS: OnceLock<Result<[libc::sigaction; RUN_SIGNALS.len()], Errno>> =
    OnceLock::new();

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
    #[cfg(target_arch = "aarch64")]
    type RunThread = DirectRunThread;

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
            Ok(DirectCodeMapping {
                file,
                _mapping: mapping,
            })
        } else {
            Err(Errno::NOSYS)
        }
    }

    fn code_file<'code>(&self, code: &'code DirectCodeMapping) -> BorrowedFd<'code> {
        code.file.as_fd()
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
    fn prepare_run_thread(&self) -> Result<DirectRunThread, Errno> {
        if PREPARED.get() {
            return Err(Errno::BUSY);
        }
        install_run_handlers()?;
        let page = rustix::param::page_size();
        let stack = SIGNAL_STACK_BYTES
            .max(rustix::param::linux_minsigstksz() * SIGNAL_FRAMES)
            .next_multiple_of(page);
        let length = page + stack;
        #[expect(
            unsafe_code,
            reason = "maps memory at an address the kernel picks, which rustix leaves unsafe"
        )]
        // SAFETY: a new private mapping, which replaces nothing and which the handle alone holds.
        let mapped = unsafe {
            rustix::mm::mmap_anonymous(
                core::ptr::null_mut(),
                length,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::PRIVATE | MapFlags::NORESERVE,
            )
        }?;
        let mapping = DirectFixedMapping {
            address: mapped.addr(),
            length,
        };
        #[expect(
            unsafe_code,
            reason = "changes the protection of pages, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: the mapping's first page, which the handle alone holds and nothing refers to, becomes inaccessible.
        unsafe { rustix::mm::mprotect(mapped, page, MprotectFlags::empty()) }?;
        let record = mapping.address + page;
        #[expect(
            unsafe_code,
            reason = "writes the record into memory only a handler and the run read, through its address"
        )]
        // SAFETY: the record's page is mapped readable and writable, aligned, and held by the handle alone.
        unsafe {
            core::ptr::write(
                record as *mut SignalStackRecord,
                SignalStackRecord {
                    own: record as u64,
                    running: 0,
                    pending: 0,
                    entry_first: 0,
                    entry_last: 0,
                    exit_first: 0,
                    exit_end: 0,
                    abort: 0,
                    saved: 0,
                },
            );
        }
        let wanted = libc::stack_t {
            ss_sp: record as *mut c_void,
            ss_flags: 0,
            ss_size: stack,
        };
        let mut earlier_stack = libc::stack_t {
            ss_sp: core::ptr::null_mut(),
            ss_flags: 0,
            ss_size: 0,
        };
        let blocked = !UNBLOCKED
            .iter()
            .fold(0_u64, |kept, signal| kept | signal_bit(*signal));
        let mut earlier_mask = 0_u64;
        #[expect(
            unsafe_code,
            reason = "sets the thread's signal stack and its raw mask, which neither rustix nor nix offers safely"
        )]
        // SAFETY: the stack is mapped and held by the returned handle, which puts back the earlier stack before it
        // unmaps it and never leaves this thread, as this does where the mask is refused; the masks are the kernel's
        // 64-bit sets.
        let set = unsafe {
            let stack_set = libc::sigaltstack(&wanted, &mut earlier_stack) == 0;
            let mask_set = stack_set
                && libc::syscall(
                    libc::SYS_rt_sigprocmask,
                    libc::SIG_SETMASK,
                    core::ptr::from_ref(&blocked),
                    core::ptr::from_mut(&mut earlier_mask),
                    size_of::<u64>(),
                ) == 0;
            if stack_set && !mask_set {
                libc::sigaltstack(&earlier_stack, core::ptr::null_mut());
            }
            mask_set
        };
        if !set {
            return Err(
                Errno::from_io_error(&std::io::Error::last_os_error()).unwrap_or(Errno::INVAL)
            );
        }
        PREPARED.set(true);
        Ok(DirectRunThread {
            record,
            earlier_stack: (
                earlier_stack.ss_sp as usize,
                earlier_stack.ss_flags,
                earlier_stack.ss_size,
            ),
            earlier_mask,
            _stack: mapping,
            _one_thread: PhantomData,
        })
    }

    #[cfg(target_arch = "aarch64")]
    fn run_code(&self, thread: &mut DirectRunThread, frame: &mut RunFrame) -> RunEnd {
        frame.exit[SIGNAL_STACK] = thread.record as u64;
        entering::run(core::ptr::from_mut(frame));
        let [.., end, address, detail] = frame.exit;
        match i32::try_from(end).unwrap_or_default() {
            0 => RunEnd::Exit(address),
            libc::SIGSEGV | libc::SIGBUS => RunEnd::Fault {
                address,
                write: detail & WRITE != 0,
                alignment: detail & ALIGNMENT != 0,
            },
            stop if stop == RUN_STOP_SIGNAL.as_raw() => {
                if detail & NOT_ENTERED != 0 {
                    RunEnd::NotEntered
                } else {
                    RunEnd::Stopped
                }
            }
            raised => RunEnd::Raised(Signal::from_named_raw(raised).unwrap_or(Signal::ILL)),
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
    fn read_fpcr(&self) -> u64 {
        let value: u64;
        #[expect(unsafe_code, reason = "reads FPCR, which no crate wraps")]
        // SAFETY: the read has no effect beyond its output register.
        unsafe {
            core::arch::asm!("mrs {}, fpcr", out(reg) value, options(nomem, nostack, preserves_flags));
        }
        value
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

/// The bit of a signal in the kernel's 64-bit set.
#[cfg(target_arch = "aarch64")]
fn signal_bit(signal: c_int) -> u64 {
    1 << (signal - 1)
}

/// Installs the handler of a run for each of [`RUN_SIGNALS`], once in this process, keeping the handlers before it.
#[cfg(target_arch = "aarch64")]
fn install_run_handlers() -> Result<(), Errno> {
    let installed = EARLIER_HANDLERS.get_or_init(|| {
        let handler = on_run_signal as extern "C" fn(c_int, *mut libc::siginfo_t, *mut c_void);
        let mut earlier = [const { None }; RUN_SIGNALS.len()];
        for (signal, into) in RUN_SIGNALS.into_iter().zip(&mut earlier) {
            let stop = signal == RUN_STOP_SIGNAL.as_raw();
            // A fault in a handler is delivered, never fatal for being blocked; the stop waits while one runs.
            let flags = if stop {
                libc::SA_SIGINFO | libc::SA_ONSTACK | libc::SA_RESTART
            } else {
                libc::SA_SIGINFO | libc::SA_ONSTACK | libc::SA_NODEFER
            };
            #[expect(
                unsafe_code,
                reason = "installs a signal handler, which neither rustix nor nix offers with a handler of three arguments safely"
            )]
            // SAFETY: the handler reads only what the kernel hands it and the run's own record, and writes only the
            // running frame and the interrupted context, as `on_run_signal` says; each action is a whole value.
            let action = unsafe {
                let mut action: libc::sigaction = core::mem::zeroed();
                action.sa_sigaction = handler as usize;
                action.sa_flags = flags;
                libc::sigemptyset(&mut action.sa_mask);
                libc::sigaddset(&mut action.sa_mask, RUN_STOP_SIGNAL.as_raw());
                let mut before: libc::sigaction = core::mem::zeroed();
                (libc::sigaction(signal, &action, &mut before) == 0).then_some(before)
            };
            *into = Some(action.ok_or_else(|| {
                Errno::from_io_error(&std::io::Error::last_os_error()).unwrap_or(Errno::INVAL)
            })?);
        }
        Ok(earlier.map(|action| action.unwrap_or_else(|| unreachable!("each signal's handler was kept"))))
    });
    installed.as_ref().map(|_| ()).map_err(|error| *error)
}

/// What a run's handler does with a signal.
#[cfg(target_arch = "aarch64")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handling {
    /// Nothing: a stop that reached a thread prepared for no run.
    Nothing,
    /// The handler this process had before takes it: a fault outside a run's code.
    Earlier,
    /// The run ends at its next start: a stop that came outside its code.
    Pending,
    /// The run ends before its code starts.
    Abort,
    /// The run ends here, with what more its end says.
    End { address: u64, detail: u64 },
}

/// What a handler of a run does with `signal`, which came at `pc` with x23 holding `scratch`, given the record at the
/// bottom of the thread's signal stack, if it has one of this crate's.
#[cfg(target_arch = "aarch64")]
fn handling(
    signal: c_int,
    pc: u64,
    scratch: u64,
    record: Option<&SignalStackRecord>,
    fault: (u64, u64, c_int),
) -> Handling {
    let stop = signal == RUN_STOP_SIGNAL.as_raw();
    let Some(record) = record else {
        return if stop {
            Handling::Nothing
        } else {
            Handling::Earlier
        };
    };
    if record.running == 0 {
        return if stop {
            Handling::Pending
        } else {
            Handling::Earlier
        };
    }
    let in_entry = (record.entry_first..=record.entry_last).contains(&pc);
    // From an exit's load of where exits go to the run's last instruction, the code's registers are being saved; a
    // fault can come there only from the run's own instructions.
    let in_exit = (record.exit_first..record.exit_end).contains(&pc)
        || (stop && scratch == record.exit_first);
    match (stop, in_entry, in_exit) {
        (true, true, _) => Handling::Abort,
        (true, _, true) => Handling::Pending,
        (false, true, _) | (false, _, true) => Handling::Earlier,
        (true, false, false) => Handling::End {
            address: 0,
            detail: 0,
        },
        (false, false, false) => {
            let (address, syndrome, code) = fault;
            let mut detail = 0;
            if (syndrome >> EXCEPTION_CLASS_SHIFT) & EXCEPTION_CLASS_MASK == DATA_ABORT_LOWER
                && syndrome & WRITE_NOT_READ != 0
            {
                detail |= WRITE;
            }
            if signal == libc::SIGBUS && code == libc::BUS_ADRALN {
                detail |= ALIGNMENT;
            }
            Handling::End { address, detail }
        }
    }
}

/// ESR_EL1 as the records after a signal frame's registers hold it, or zero.
#[cfg(target_arch = "aarch64")]
fn syndrome(records: &[u8; RECORDS_BYTES]) -> u64 {
    let word = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            records.get(at..at + size_of::<u32>())?.try_into().ok()?,
        ))
    };
    let mut at = 0;
    while let (Some(magic), Some(size)) = (word(at), word(at + size_of::<u32>())) {
        if magic == 0 || size == 0 {
            break;
        }
        if magic == ESR_MAGIC {
            let value = at + 2 * size_of::<u32>();
            return records
                .get(value..value + size_of::<u64>())
                .and_then(|bytes| bytes.try_into().ok())
                .map_or(0, u64::from_le_bytes);
        }
        at += size as usize;
    }
    0
}

/// The handler of every signal of a run: on the thread's signal stack, never the code's, whatever its stack pointer
/// holds. It touches no thread-local and calls nothing that sets errno, since the code's thread pointer may be the
/// code's own.
#[cfg(target_arch = "aarch64")]
extern "C" fn on_run_signal(signal: c_int, info: *mut libc::siginfo_t, context: *mut c_void) {
    #[expect(
        unsafe_code,
        reason = "a signal handler is handed its signal's information and context as pointers"
    )]
    // SAFETY: the kernel hands a handler installed with SA_SIGINFO both, valid and this handler's alone while it runs.
    let (info, context) = unsafe { (&*info, &mut *context.cast::<libc::ucontext_t>()) };
    #[expect(
        unsafe_code,
        reason = "the faulting address is a field of a union, which libc reads unsafely"
    )]
    // SAFETY: for every signal of a run the kernel fills the union's fault address, unused for the stop.
    let address = unsafe { info.si_addr() } as u64;
    let records_at = core::ptr::from_ref(&context.uc_mcontext) as usize
        + (core::mem::offset_of!(libc::mcontext_t, pstate) + size_of::<u64>())
            .next_multiple_of(RECORDS_ALIGNMENT);
    #[expect(
        unsafe_code,
        reason = "the records after the frame's registers are a field libc keeps private"
    )]
    // SAFETY: the kernel's frame holds those records' bytes right after the registers, aligned to 16, as its header
    // says, and they are this handler's to read.
    let records = unsafe { &*(records_at as *const [u8; RECORDS_BYTES]) };
    let stack = &context.uc_stack;
    let bottom = stack.ss_sp as usize;
    let record = if stack.ss_flags & libc::SS_DISABLE == 0 && bottom != 0 {
        #[expect(
            unsafe_code,
            reason = "the record is found by the signal stack's address, which the kernel hands the handler"
        )]
        // SAFETY: a signal stack's bottom is mapped readable and writable; on one of this crate's the record lies
        // there, which only this thread's run and handler use, never at once; on another, its first word is not its
        // own address.
        let record = unsafe { &mut *(bottom as *mut SignalStackRecord) };
        (record.own == bottom as u64).then_some(record)
    } else {
        None
    };
    let registers = &mut context.uc_mcontext;
    let fault = (address, syndrome(records), info.si_code);
    let scratch = registers.regs[usize::from(EXIT_SCRATCH_REGISTER)];
    match (
        handling(signal, registers.pc, scratch, record.as_deref(), fault),
        record,
    ) {
        (Handling::Nothing, _) => {}
        (Handling::Pending, Some(record)) => record.pending = 1,
        (Handling::Abort, Some(record)) => {
            registers.regs[0] = record.running;
            registers.regs[usize::from(FRAME_REGISTER)] = record.running;
            registers.pc = record.abort;
        }
        (Handling::End { address, detail }, Some(record)) => {
            #[expect(
                unsafe_code,
                reason = "the running frame is found by the address the run left in the record"
            )]
            // SAFETY: while a run is on, the record holds its frame's address, which the run borrows mutably and does
            // not touch until this handler returns.
            let frame = unsafe { &mut *(record.running as *mut RunFrame) };
            let into = &mut frame.registers;
            into.x.copy_from_slice(&registers.regs[..GENERAL_REGISTERS]);
            into.sp = registers.sp;
            into.pc = registers.pc;
            into.nzcv = registers.pstate & NZCV;
            frame.exit[END] = signal as u64;
            frame.exit[END_ADDRESS] = address;
            frame.exit[END_DETAIL] = detail;
            if signal == RUN_STOP_SIGNAL.as_raw() {
                record.pending = 0;
            }
            registers.regs[usize::from(FRAME_REGISTER)] = record.running;
            registers.pc = record.saved;
        }
        (Handling::Earlier, _) | (_, None) => on_signal_elsewhere(signal),
    }
}

/// Gives the signal back to the handler this process had before a run's: it is put back, and the instruction that
/// faulted faults again.
#[cfg(target_arch = "aarch64")]
fn on_signal_elsewhere(signal: c_int) {
    let Some(Ok(earlier)) = EARLIER_HANDLERS.get() else {
        return;
    };
    let Some(index) = RUN_SIGNALS.iter().position(|kept| *kept == signal) else {
        return;
    };
    if signal == RUN_STOP_SIGNAL.as_raw() {
        return;
    }
    #[expect(
        unsafe_code,
        reason = "puts back a signal handler, which neither rustix nor nix offers safely"
    )]
    // SAFETY: the action is the one the kernel gave back when this crate installed its own, whole and unchanged.
    unsafe {
        libc::sigaction(signal, &earlier[index], core::ptr::null_mut());
    }
}

#[cfg(target_arch = "aarch64")]
impl Drop for DirectRunThread {
    fn drop(&mut self) {
        let mut ignored = 0_u64;
        let (bottom, flags, size) = self.earlier_stack;
        let earlier = libc::stack_t {
            ss_sp: bottom as *mut c_void,
            ss_flags: flags,
            ss_size: size,
        };
        #[expect(
            unsafe_code,
            reason = "puts back the thread's signal stack and its raw mask, which neither rustix nor nix offers safely"
        )]
        // SAFETY: the stack and mask this thread had when the handle was made, on that same thread, before the
        // handle's stack is unmapped.
        unsafe {
            libc::sigaltstack(&earlier, core::ptr::null_mut());
            libc::syscall(
                libc::SYS_rt_sigprocmask,
                libc::SIG_SETMASK,
                core::ptr::from_ref(&self.earlier_mask),
                core::ptr::from_mut(&mut ignored),
                size_of::<u64>(),
            );
        }
        PREPARED.set(false);
    }
}

/// The run itself, a function of assembly alone.
#[cfg(target_arch = "aarch64")]
#[expect(
    unsafe_code,
    reason = "UNSOUND: a safe fn that runs whatever code the frame's pc names; only the caller's keeping that code to what LinuxProcess::run_code says (its mappings and the frame's words alone, its own stack, no system call, only the registers named there changed, x28 the frame) keeps this process's memory as Rust holds it; no safe form of entering other code exists"
)]
mod entering {
    use super::{
        END_AT, END_DETAIL_AT, EXIT_AT, EXIT_LINKED, EXITS_GO_AT, FPCR_AT, FPSR_AT, KEPT_BYTES,
        KEPT_D_AT, KEPT_SYSTEM_AT, NOT_ENTERED, NZCV, NZCV_AT, PC_AT, REGISTERS_AT,
        RUN_STOP_SIGNAL, RunFrame, SIGNAL_STACK_AT, SP_AT, THREAD_STACK_AT, TOP_ENTRY_AT,
        TOP_EXIT_AT, TOP_LABELS_AT, TOP_PENDING_AT, TOP_RUNNING_AT, TPIDR_AT, V_AT, X_AT,
    };

    /// Runs the frame's code until it branches with link to an exit, faults or is stopped, as
    /// [`LinuxProcess::run_code`] says; the frame's run words say how it ended.
    #[unsafe(naked)]
    pub(super) extern "C" fn run(frame: *mut RunFrame) {
        core::arch::naked_asm!(
            // The thread's own: the registers a call keeps, FPCR and TPIDR_EL0, on its stack, whose top the frame keeps.
            "0:",
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
            // Where a signal finds each part of the run, then the frame: from here a stop ends the run, and one that
            // came before ends it now.
            "ldr x11, [x0, #{signal_stack}]",
            "adr x9, 0b",
            "adr x10, 3f",
            "stp x9, x10, [x11, #{top_entry}]",
            "adr x9, 2f",
            "adr x10, 5f",
            "stp x9, x10, [x11, #{top_exit}]",
            "adr x9, 6f",
            "adr x10, 4f",
            "stp x9, x10, [x11, #{top_labels}]",
            "str x0, [x11, #{top_running}]",
            "ldr x9, [x11, #{top_pending}]",
            "cbnz x9, 6f",
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
            "3:",
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
            "sub x0, x30, #{exit_linked}",
            "stp xzr, x0, [x28, #{end}]",
            // A handler that ends a run comes here, the general registers saved, x28 the frame.
            "4:",
            "add x23, x28, #{registers}",
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
            // Back to the thread: the run is off, then its stack and its own registers.
            "7:",
            "ldr x11, [x28, #{signal_stack}]",
            "str xzr, [x11, #{top_running}]",
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
            // A stop before the code started, x0 the frame: its registers stay as they were.
            "6:",
            "mov x28, x0",
            "ldr x11, [x28, #{signal_stack}]",
            "str xzr, [x11, #{top_pending}]",
            "mov x9, #{stop}",
            "str x9, [x28, #{end}]",
            "mov x9, #{not_entered}",
            "str x9, [x28, #{end_detail}]",
            "b 7b",
            "5:",
            kept = const KEPT_BYTES,
            kept_d = const KEPT_D_AT,
            kept_system = const KEPT_SYSTEM_AT,
            exits_go = const EXITS_GO_AT,
            exit = const EXIT_AT,
            thread_stack = const THREAD_STACK_AT,
            signal_stack = const SIGNAL_STACK_AT,
            end = const END_AT,
            end_detail = const END_DETAIL_AT,
            exit_linked = const EXIT_LINKED,
            top_entry = const TOP_ENTRY_AT,
            top_exit = const TOP_EXIT_AT,
            top_labels = const TOP_LABELS_AT,
            top_running = const TOP_RUNNING_AT,
            top_pending = const TOP_PENDING_AT,
            stop = const RUN_STOP_SIGNAL.as_raw(),
            not_entered = const NOT_ENTERED,
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
