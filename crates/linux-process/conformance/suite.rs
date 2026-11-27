//! The tests every implementation of wcontainers-linux-process runs, written against its trait alone.
#![cfg(target_os = "linux")]

use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
#[cfg(target_arch = "aarch64")]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use nix::sys::signal::Signal as NixSignal;
use wcontainers_linux_process::{Errno, LinuxProcess, Pid, ProtFlags, Signal};

/// Where the fixed page search starts: far above any heap or library a test process holds.
const SEARCH_START: usize = 1 << 40;
/// Where each test that maps a file within a fixed mapping searches: apart from every other test's search, so no test
/// maps the pages another has just unmapped and checks are unmapped.
const MAPPED_FILE_SEARCH_START: usize = SEARCH_START << 1;
const REFUSED_FILE_SEARCH_START: usize = SEARCH_START << 2;
/// How many pages the search tries before it gives up.
const SEARCH_PAGES: usize = 1 << 12;
/// The bytes of the shared memory the tests make.
const SHARED_BYTES: usize = 1 << 16;
/// The pages of the fixed mapping a file is mapped within.
const MAPPING_PAGES: usize = 4;
/// The file's pages mapped there, after the mapping's first.
const FILE_PAGES: usize = 2;
/// A word the tests write and read back.
const WORD: u64 = 0x0123_4567_89ab_cdef;
/// CTR_EL0's bit 31, which the architecture reserves as one.
#[cfg(target_arch = "aarch64")]
const CTR_EL0_RES1: u64 = 1 << 31;
/// Where the test of bad addresses searches: apart from the test that checks its page is unmapped after a drop.
const BAD_ADDRESS_SEARCH_START: usize = SEARCH_START * 7;
/// Where the code tests search: apart from the others.
const CODE_SEARCH_START: usize = SEARCH_START * 3;
#[cfg(target_arch = "aarch64")]
const RUN_SEARCH_START: usize = SEARCH_START * 5;
/// The pages of code mapped.
const CODE_PAGES: usize = 2;
/// Code that changes the thread pointer, the rounding mode, the stack pointer, d8 and x19, then exits (Arm ARM, C6.2):
/// `add x23, x23, #0x100` (the exit, from its own first address), `movz x5, #0x1234`, `msr tpidr_el0, x5`,
/// `msr fpcr, x6`, `mov sp, x7`, `fmov d8, xzr`, `movz x19, #0`, `blr x23`.
#[cfg(target_arch = "aarch64")]
const CODE_TO_EXIT: [u32; 8] = [
    0x9104_02f7,
    0xd282_4685,
    0xd51b_d045,
    0xd51b_4406,
    0x9100_00ff,
    0x9e67_03e8,
    0xd280_0013,
    0xd63f_02e0,
];
/// Where that code's exit is, past its first address.
#[cfg(target_arch = "aarch64")]
const EXIT_AT: usize = 0x100;
/// The registers it moves `MOVED` into, takes the rounding mode and the stack pointer from, zeroes (one a call
/// keeps, d8 and x19), and the ones a run names: the exit's scratch x23, the frame's x28, the link x30.
#[cfg(target_arch = "aarch64")]
const MOVED_TO: usize = 5;
#[cfg(target_arch = "aarch64")]
const MOVED: u64 = 0x1234;
#[cfg(target_arch = "aarch64")]
const NEW_ROUNDING_FROM: usize = 6;
#[cfg(target_arch = "aarch64")]
const NEW_STACK_FROM: usize = 7;
#[cfg(target_arch = "aarch64")]
const ZEROED_VECTOR: usize = 8;
#[cfg(target_arch = "aarch64")]
const KEPT_BY_A_CALL: usize = 19;
#[cfg(target_arch = "aarch64")]
const EXIT_SCRATCH: usize = wcontainers_linux_process::EXIT_SCRATCH_REGISTER as usize;
#[cfg(target_arch = "aarch64")]
const FRAME: usize = wcontainers_linux_process::FRAME_REGISTER as usize;
#[cfg(target_arch = "aarch64")]
const LINK: usize = 30;
#[cfg(target_arch = "aarch64")]
const EXIT_BYTES: usize = wcontainers_linux_process::EXIT_BYTES;
/// The code's stack pointers, which it never uses as addresses.
#[cfg(target_arch = "aarch64")]
const CODE_STACK: u64 = 0x10_0000;
#[cfg(target_arch = "aarch64")]
const NEW_STACK: u64 = 0x20_0000;
/// A tenth, rounded to nearest: rounding toward zero makes it one unit in the last place smaller
/// ([binary64](https://en.wikipedia.org/wiki/Double-precision_floating-point_format)).
#[cfg(target_arch = "aarch64")]
const TENTH_ROUNDED_TO_NEAREST: u64 = 0x3fb9_9999_9999_999a;
#[cfg(target_arch = "aarch64")]
const TEN: f64 = 10.0;
/// Where the fault, stop and early-stop tests search: apart from the others.
#[cfg(target_arch = "aarch64")]
const FAULT_SEARCH_START: usize = SEARCH_START * 9;
#[cfg(target_arch = "aarch64")]
const STOP_SEARCH_START: usize = SEARCH_START * 11;
#[cfg(target_arch = "aarch64")]
const EARLY_STOP_SEARCH_START: usize = SEARCH_START * 13;
/// Instructions that fault or raise a signal, each followed by an exit that only a mutant reaches
/// ([Arm ARM, C6.2](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions)): `ldr x0, [x1]`,
/// `str x0, [x1]`, `ldr x0, [sp]`, `br x1`, `udf #0`, `brk #0`; then `add x23, x23, #0x100` and `blr x23`.
#[cfg(target_arch = "aarch64")]
const LOAD_THROUGH_X1: u32 = 0xf940_0020;
#[cfg(target_arch = "aarch64")]
const STORE_THROUGH_X1: u32 = 0xf900_0020;
#[cfg(target_arch = "aarch64")]
const LOAD_THROUGH_SP: u32 = 0xf940_03e0;
#[cfg(target_arch = "aarch64")]
const BRANCH_TO_X1: u32 = 0xd61f_0020;
#[cfg(target_arch = "aarch64")]
const UNDEFINED: u32 = 0;
#[cfg(target_arch = "aarch64")]
const BREAKPOINT: u32 = 0xd420_0000;
#[cfg(target_arch = "aarch64")]
const TO_EXIT: [u32; 2] = [0x9104_02f7, 0xd63f_02e0];
/// `b .`: code that runs until it is stopped.
#[cfg(target_arch = "aarch64")]
const BRANCH_TO_ITSELF: u32 = 0x1400_0000;
/// The register the faulting instructions take their address from.
#[cfg(target_arch = "aarch64")]
const ADDRESS_FROM: usize = 1;
/// The hostile stack's bytes, a pattern no signal frame leaves.
#[cfg(target_arch = "aarch64")]
const HOSTILE_STACK_WORDS: usize = 1 << 13;
/// How long the stopping thread waits between stops.
#[cfg(target_arch = "aarch64")]
const BETWEEN_STOPS: std::time::Duration = std::time::Duration::from_millis(1);
/// The signals a prepared thread leaves unblocked, and the two no mask can block.
#[cfg(target_arch = "aarch64")]
const UNBLOCKED: [Signal; 8] = [
    Signal::SEGV,
    Signal::BUS,
    Signal::ILL,
    Signal::TRAP,
    Signal::FPE,
    wcontainers_linux_process::RUN_STOP_SIGNAL,
    Signal::KILL,
    Signal::STOP,
];
/// The line of `/proc/thread-self/status` that holds the thread's blocked signals, in hexadecimal.
#[cfg(target_arch = "aarch64")]
const BLOCKED_LINE: &str = "SigBlk:";
#[cfg(target_arch = "aarch64")]
const HEXADECIMAL: u32 = 16;

/// FPCR's rounding mode toward zero (RMode, bits 23 and 22).
#[cfg(target_arch = "aarch64")]
const ROUND_TOWARD_ZERO: u64 = 0b11 << 22;
/// The flags Z and C, and FPSR's QC and IOC.
#[cfg(target_arch = "aarch64")]
const FLAGS: u64 = 0x6000_0000;
#[cfg(target_arch = "aarch64")]
const STATUS: u64 = 0x0800_0001;

std::thread_local! {
    /// A thread-local the run test reads after its code changed the thread pointer.
    static THREAD_OWN: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// A process that waits until it is signalled.
const WAITING_PROGRAM: &str = "sleep";
const WAITING_SECONDS: &str = "30";

fn own_process() -> Pid {
    rustix::process::getpid()
}

fn own_thread() -> Pid {
    rustix::thread::gettid()
}

fn waiting_child() -> std::process::Child {
    match Command::new(WAITING_PROGRAM)
        .arg(WAITING_SECONDS)
        .stdin(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => panic!("starting {WAITING_PROGRAM}: {error}"),
    }
}

fn pid_of(child: &std::process::Child) -> Pid {
    let raw = i32::try_from(child.id()).expect("a process id fits");
    Pid::from_raw(raw).expect("a child's id is not zero")
}

/// Returns the first page at or above [`SEARCH_START`] the implementation maps.
fn fixed_page<L: LinuxProcess>(linux: &L) -> (usize, L::FixedMapping) {
    let page = rustix::param::page_size();
    (0..SEARCH_PAGES)
        .map(|index| SEARCH_START + index * page)
        .find_map(|address| {
            linux
                .mmap_fixed_noreplace(address, page)
                .ok()
                .map(|mapping| (address, mapping))
        })
        .expect("a free page above the search start")
}

/// Whether this process maps the page at `address`, read from its own `/proc/self/maps`.
fn is_mapped(address: usize) -> bool {
    let maps = std::fs::read_to_string("/proc/self/maps").expect("the process's own maps");
    maps.lines().any(|line| {
        let range = line.split_whitespace().next().unwrap_or_default();
        let Some((start, end)) = range.split_once('-') else {
            return false;
        };
        let parse = |text: &str| usize::from_str_radix(text, 16).unwrap_or_default();
        (parse(start)..parse(end)).contains(&address)
    })
}

/// Returns the first pages at or above `from` the implementation maps, `count` of them.
fn fixed_pages<L: LinuxProcess>(linux: &L, from: usize, count: usize) -> (usize, L::FixedMapping) {
    let page = rustix::param::page_size();
    (0..SEARCH_PAGES)
        .map(|index| from + index * count * page)
        .find_map(|address| {
            linux
                .mmap_fixed_noreplace(address, count * page)
                .ok()
                .map(|mapping| (address, mapping))
        })
        .expect("free pages above the search start")
}

/// The protection and sharing this process maps the page at `address` with, as `/proc/self/maps` writes them
/// (`rw-s`), if it maps it.
fn protection_of(address: usize) -> Option<String> {
    let maps = std::fs::read_to_string("/proc/self/maps").expect("the process's own maps");
    maps.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let (start, end) = fields.next()?.split_once('-')?;
        let parse = |text: &str| usize::from_str_radix(text, 16).ok();
        (parse(start)?..parse(end)?)
            .contains(&address)
            .then(|| fields.next().unwrap_or_default().to_owned())
    })
}

/// The bytes this process holds at `address`, read through `/proc/self/mem`, which reads a page whatever its
/// protection.
fn bytes_at(address: usize, length: usize) -> Vec<u8> {
    use std::os::unix::fs::FileExt;
    let memory = std::fs::File::open("/proc/self/mem").expect("the process's own memory");
    let mut bytes = vec![0_u8; length];
    memory
        .read_exact_at(&mut bytes, address as u64)
        .expect("the bytes at a mapped address");
    bytes
}

/// A memory file of `pages` pages, each filled with its own index plus one.
fn numbered_file(pages: usize) -> std::os::fd::OwnedFd {
    let page = rustix::param::page_size();
    let file = rustix::fs::memfd_create("wcontainers conformance", rustix::fs::MemfdFlags::CLOEXEC)
        .expect("a memory file");
    for index in 0..pages {
        let fill = vec![u8::try_from(index + 1).expect("few pages"); page];
        let written =
            rustix::io::pwrite(&file, &fill, (index * page) as u64).expect("the file is written");
        assert_eq!(written, page);
    }
    file
}

/// Ends the main thread of another process with the signal.
pub fn signals_one_thread_of_another_process(linux: impl LinuxProcess) {
    let mut child = waiting_child();
    let process = pid_of(&child);
    linux
        .tgkill(process, process, Signal::TERM)
        .expect("the child's main thread is signalled");
    let status = child.wait().expect("the child is reaped");
    assert_eq!(status.signal(), Some(NixSignal::SIGTERM as i32));
}

/// Refuses a thread that is not one of the process's, so a forged thread id reaches no other process.
pub fn refuses_a_thread_of_another_process(linux: impl LinuxProcess) {
    let mut child = waiting_child();
    let process = pid_of(&child);
    let refused = linux.tgkill(process, own_thread(), Signal::TERM);
    let refused_here = linux.tgkill(own_process(), process, Signal::TERM);
    child.kill().expect("the child is killed");
    let status = child.wait().expect("the child is reaped");
    assert_eq!(refused, Err(Errno::SRCH));
    assert_eq!(refused_here, Err(Errno::SRCH));
    assert_eq!(status.signal(), Some(NixSignal::SIGKILL as i32));
}

/// Maps a page where asked, refuses that page while it is held, and unmaps it on drop.
pub fn maps_a_fixed_page_and_unmaps_it_on_drop(linux: impl LinuxProcess) {
    let page = rustix::param::page_size();
    let (address, mapping) = fixed_page(&linux);
    assert!(is_mapped(address));
    assert_eq!(
        linux.mmap_fixed_noreplace(address, page).err(),
        Some(Errno::EXIST)
    );
    drop(mapping);
    assert!(!is_mapped(address));
    let again = linux.mmap_fixed_noreplace(address, page);
    assert!(again.is_ok());
}

/// Refuses a page this process already maps, leaving its bytes as they were.
pub fn refuses_to_replace_a_mapping(linux: impl LinuxProcess) {
    let page = rustix::param::page_size();
    let held = vec![WORD; 2 * page / size_of::<u64>()];
    let inside = (held.as_ptr() as usize).next_multiple_of(page);
    assert_eq!(
        linux.mmap_fixed_noreplace(inside, page).err(),
        Some(Errno::EXIST)
    );
    assert!(held.iter().all(|word| *word == WORD));
}

/// Refuses lengths and addresses Linux refuses, with Linux's error.
pub fn refuses_a_page_at_a_bad_address_or_length(linux: impl LinuxProcess) {
    let page = rustix::param::page_size();
    let (address, mapping) = fixed_pages(&linux, BAD_ADDRESS_SEARCH_START, 1);
    drop(mapping);
    assert_eq!(
        linux.mmap_fixed_noreplace(address + 1, page).err(),
        Some(Errno::INVAL)
    );
    assert_eq!(
        linux.mmap_fixed_noreplace(address, 0).err(),
        Some(Errno::INVAL)
    );
    assert!(
        linux
            .mmap_fixed_noreplace(usize::MAX - page + 1, page)
            .is_err()
    );
}

/// Shows a file's pages inside a fixed mapping where asked, with the protection asked, its other pages untouched, and
/// changes their protection; the bytes seen there are the file's, and the drop unmaps them all.
pub fn maps_a_file_within_a_fixed_mapping(linux: impl LinuxProcess) {
    let page = rustix::param::page_size();
    let (address, mut mapping) = fixed_pages(&linux, MAPPED_FILE_SEARCH_START, MAPPING_PAGES);
    let file = numbered_file(FILE_PAGES);
    linux
        .map_file_within(
            &mut mapping,
            page,
            FILE_PAGES * page,
            std::os::fd::AsFd::as_fd(&file),
            0,
            ProtFlags::READ,
        )
        .expect("the file is mapped within");
    assert_eq!(protection_of(address).as_deref(), Some("---p"));
    for index in 0..FILE_PAGES {
        let at = address + (index + 1) * page;
        assert_eq!(protection_of(at).as_deref(), Some("r--s"));
        assert_eq!(
            bytes_at(at, page),
            vec![u8::try_from(index + 1).expect("few pages"); page]
        );
    }
    let last = address + (MAPPING_PAGES - 1) * page;
    assert_eq!(protection_of(last).as_deref(), Some("---p"));
    linux
        .protect_within(&mut mapping, page, page, ProtFlags::READ | ProtFlags::WRITE)
        .expect("the protection changes");
    assert_eq!(protection_of(address + page).as_deref(), Some("rw-s"));
    assert_eq!(protection_of(address + 2 * page).as_deref(), Some("r--s"));
    linux
        .protect_within(&mut mapping, page, page, ProtFlags::empty())
        .expect("the protection changes");
    assert_eq!(protection_of(address + page).as_deref(), Some("---s"));
    drop(mapping);
    assert!(!is_mapped(address + page));
    assert!(!is_mapped(last));
}

/// Refuses pages that are not whole pages inside the mapping, and an executable protection, changing nothing.
pub fn refuses_pages_outside_a_fixed_mapping_or_executable(linux: impl LinuxProcess) {
    let page = rustix::param::page_size();
    let (address, mut mapping) = fixed_pages(&linux, REFUSED_FILE_SEARCH_START, MAPPING_PAGES);
    let file = numbered_file(MAPPING_PAGES);
    let whole = MAPPING_PAGES * page;
    let refused = [
        (page + 1, page, ProtFlags::READ, Errno::INVAL),
        (page, page + 1, ProtFlags::READ, Errno::INVAL),
        (page, 0, ProtFlags::READ, Errno::INVAL),
        (page, whole, ProtFlags::READ, Errno::INVAL),
        (whole, page, ProtFlags::READ, Errno::INVAL),
        (usize::MAX - page + 1, page, ProtFlags::READ, Errno::INVAL),
        (page, page, ProtFlags::READ | ProtFlags::EXEC, Errno::PERM),
        (page, page, ProtFlags::EXEC, Errno::PERM),
    ];
    for (offset, length, protection, error) in refused {
        let fd = std::os::fd::AsFd::as_fd(&file);
        assert_eq!(
            linux.map_file_within(&mut mapping, offset, length, fd, 0, protection),
            Err(error),
            "a file mapped at {offset:#x}, {length:#x} bytes, {protection:?}"
        );
        assert_eq!(
            linux.protect_within(&mut mapping, offset, length, protection),
            Err(error),
            "a protection at {offset:#x}, {length:#x} bytes, {protection:?}"
        );
    }
    for index in 0..MAPPING_PAGES {
        assert_eq!(
            protection_of(address + index * page).as_deref(),
            Some("---p")
        );
    }
}

/// The words written here are the file's bytes, which another holder cannot shrink.
pub fn shares_words_through_a_file_that_cannot_shrink(linux: impl LinuxProcess) {
    let words = linux
        .create_shared_words("wcontainers conformance", SHARED_BYTES)
        .expect("shared words");
    assert_eq!(
        linux.shared_words32(&words).len(),
        SHARED_BYTES / size_of::<u32>()
    );
    assert_eq!(
        linux.shared_words64(&words).len(),
        SHARED_BYTES / size_of::<u64>()
    );
    let last = SHARED_BYTES / size_of::<u64>() - 1;
    linux.shared_words64(&words)[last].store(WORD, Ordering::Release);
    linux.shared_words32(&words)[0].store(u32::MAX, Ordering::Release);
    let file = linux.shared_words_file(&words);
    let mut bytes = [0_u8; size_of::<u64>()];
    let read = rustix::io::pread(file, &mut bytes, (last * size_of::<u64>()) as u64)
        .expect("the file reads");
    assert_eq!(read, bytes.len());
    assert_eq!(u64::from_le_bytes(bytes), WORD);
    let mut first = [0_u8; size_of::<u32>()];
    rustix::io::pread(file, &mut first, 0).expect("the file reads");
    assert_eq!(u32::from_le_bytes(first), u32::MAX);
    assert_eq!(rustix::fs::ftruncate(file, 0), Err(Errno::PERM));
    assert_eq!(
        rustix::fs::fstat(file).expect("the file's size").st_size,
        i64::try_from(SHARED_BYTES).expect("fits")
    );
}

/// Maps code where asked, readable and executable, which no one can write, even through `/proc/self/mem`; refuses
/// what is not whole pages and a place already mapped; unmaps it on drop.
pub fn maps_code_no_one_can_write(linux: impl LinuxProcess) {
    let page = rustix::param::page_size();
    let (address, held) = fixed_pages(&linux, CODE_SEARCH_START, CODE_PAGES);
    drop(held);
    let code: Vec<u8> = (0..CODE_PAGES * page).map(|k| k as u8).collect();
    for (at, bytes) in [
        (address + 1, &code[..]),
        (address, &code[..page - 1]),
        (address, &code[..0]),
    ] {
        assert_eq!(
            linux.map_code(at, bytes).err(),
            Some(Errno::INVAL),
            "code of {} bytes at {at:#x}",
            bytes.len()
        );
    }
    let mapping = linux.map_code(address, &code).expect("the code is mapped");
    assert_eq!(
        linux.map_code(address, &code[..page]).err(),
        Some(Errno::EXIST)
    );
    for index in 0..CODE_PAGES {
        assert_eq!(
            protection_of(address + index * page).as_deref(),
            Some("r-xs")
        );
    }
    assert_eq!(bytes_at(address, code.len()), code);
    let memory = std::fs::OpenOptions::new()
        .write(true)
        .open("/proc/self/mem")
        .expect("the process's own memory");
    let written = std::os::unix::fs::FileExt::write_at(&memory, &[0; 8], address as u64);
    assert!(
        written.is_err(),
        "a write through /proc/self/mem: {written:?}"
    );
    assert_eq!(bytes_at(address, code.len()), code);
    // The file it is mapped from is sealed: neither a write nor a second, writable mapping of it is allowed.
    let file = linux.code_file(&mapping);
    let seals = rustix::fs::fcntl_get_seals(file).expect("the file's seals");
    assert!(seals.contains(rustix::fs::SealFlags::WRITE), "{seals:?}");
    assert_eq!(rustix::io::pwrite(file, &[0; 8], 0), Err(Errno::PERM));
    let writable = memmap2::MmapOptions::new().len(code.len()).map_raw(&file);
    assert_eq!(
        writable.map_err(|error| error.raw_os_error()).err(),
        Some(Some(Errno::PERM.raw_os_error())),
        "a second, writable mapping of the code's file"
    );
    assert_eq!(bytes_at(address, code.len()), code);
    drop(mapping);
    assert!(!is_mapped(address));
}

/// Runs code until it branches to an exit: the code starts with every register the frame gives it, x23 its first
/// address and x28 the frame's; the frame then holds every register the code left; the thread's own registers, its
/// thread pointer, rounding mode and the registers a call keeps, which the code changed, are as before.
#[cfg(target_arch = "aarch64")]
pub fn runs_code_until_an_exit_and_keeps_the_thread_s_own(linux: impl LinuxProcess + Sync) {
    use wcontainers_linux_process::{Arm64Registers, RunEnd, RunFrame};
    let page = rustix::param::page_size();
    let (address, held) = fixed_pages(&linux, RUN_SEARCH_START, 1);
    drop(held);
    let mut code = vec![0_u8; page];
    for (at, word) in (0..).step_by(size_of::<u32>()).zip(CODE_TO_EXIT) {
        code[at..at + size_of::<u32>()].copy_from_slice(&word.to_le_bytes());
    }
    code[EXIT_AT..EXIT_AT + EXIT_BYTES].copy_from_slice(&linux.exit_code());
    let _mapping = linux.map_code(address, &code).expect("the code is mapped");
    let mut frame = RunFrame::default();
    let mut registers = Arm64Registers::default();
    for (r, value) in registers.x.iter_mut().enumerate() {
        *value = WORD.rotate_left(r as u32);
    }
    for (r, value) in registers.v.iter_mut().enumerate() {
        *value = u128::from(WORD.rotate_right(r as u32)) << 64 | r as u128;
    }
    registers.x[NEW_STACK_FROM] = NEW_STACK;
    registers.x[NEW_ROUNDING_FROM] = ROUND_TOWARD_ZERO;
    registers.sp = CODE_STACK;
    registers.pc = address as u64;
    registers.nzcv = FLAGS;
    registers.fpsr = STATUS;
    registers.fpcr = 0;
    registers.tpidr_el0 = WORD;
    frame.registers = registers;
    frame.words = [WORD; wcontainers_linux_process::RUN_FRAME_WORDS];
    THREAD_OWN.set(WORD);
    let mut thread = linux.prepare_run_thread().expect("the thread is prepared");
    let control = linux.read_fpcr();
    assert_eq!(tenth().to_bits(), TENTH_ROUNDED_TO_NEAREST);
    let exit = linux.run_code(&mut thread, &mut frame);
    assert_eq!(THREAD_OWN.get(), WORD, "the thread's own thread-local");
    assert_eq!(
        linux.read_fpcr(),
        control,
        "the thread's floating-point control"
    );
    assert_eq!(
        tenth().to_bits(),
        TENTH_ROUNDED_TO_NEAREST,
        "the thread's rounding"
    );
    let exit_at = address as u64 + EXIT_AT as u64;
    assert_eq!(exit, RunEnd::Exit(exit_at));
    let left = frame.registers;
    let mut wanted = registers;
    wanted.x[EXIT_SCRATCH] = exit_at;
    wanted.x[FRAME] = core::ptr::from_ref(&frame) as u64;
    wanted.x[MOVED_TO] = MOVED;
    wanted.x[KEPT_BY_A_CALL] = 0;
    wanted.x[LINK] = address as u64 + (CODE_TO_EXIT.len() * size_of::<u32>()) as u64;
    wanted.pc = wanted.x[LINK];
    wanted.sp = NEW_STACK;
    wanted.tpidr_el0 = MOVED;
    wanted.fpcr = ROUND_TOWARD_ZERO;
    wanted.v[ZEROED_VECTOR] = 0;
    assert_eq!(left, wanted);
    assert_eq!(
        frame.words,
        [WORD; wcontainers_linux_process::RUN_FRAME_WORDS]
    );
    // Again, on another thread, while this one runs it too.
    std::thread::scope(|scope| {
        let linux = &linux;
        let runs: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(move || {
                    let mut frame = RunFrame {
                        registers,
                        ..RunFrame::default()
                    };
                    let mut thread = linux.prepare_run_thread().expect("the thread is prepared");
                    (linux.run_code(&mut thread, &mut frame), frame.registers.sp)
                })
            })
            .collect();
        for run in runs {
            assert_eq!(
                run.join().expect("the thread ends"),
                (RunEnd::Exit(exit_at), NEW_STACK)
            );
        }
    });
}

/// A tenth, computed with this thread's rounding mode.
#[cfg(target_arch = "aarch64")]
fn tenth() -> f64 {
    std::hint::black_box(1.0_f64) / std::hint::black_box(TEN)
}

/// Code of `instructions` at the first of `CODE_PAGES` free pages at or above `from`, its exit at [`EXIT_AT`].
#[cfg(target_arch = "aarch64")]
fn code_of<L: LinuxProcess>(linux: &L, from: usize, instructions: &[u32]) -> (u64, L::CodeMapping) {
    let page = rustix::param::page_size();
    let (address, held) = fixed_pages(linux, from, CODE_PAGES);
    drop(held);
    let mut code = vec![0_u8; page];
    for (at, word) in (0..).step_by(size_of::<u32>()).zip(instructions) {
        code[at..at + size_of::<u32>()].copy_from_slice(&word.to_le_bytes());
    }
    code[EXIT_AT..EXIT_AT + EXIT_BYTES].copy_from_slice(&linux.exit_code());
    let mapping = linux.map_code(address, &code).expect("the code is mapped");
    (address as u64, mapping)
}

/// A frame whose code starts at `pc`, each general register its own value but `ADDRESS_FROM`, which holds `address`.
#[cfg(target_arch = "aarch64")]
fn frame_at(pc: u64, address: u64, sp: u64) -> wcontainers_linux_process::RunFrame {
    let mut frame = wcontainers_linux_process::RunFrame::default();
    for (r, value) in frame.registers.x.iter_mut().enumerate() {
        *value = WORD.rotate_left(r as u32);
    }
    frame.registers.x[ADDRESS_FROM] = address;
    frame.registers.sp = sp;
    frame.registers.pc = pc;
    frame
}

/// Ends a run at an access that faults, whether for its page or its alignment, and at an instruction that raises a
/// signal: the frame holds the registers at the instruction, and the thread goes on as before.
#[cfg(target_arch = "aarch64")]
pub fn ends_a_run_at_a_fault_of_its_code(linux: impl LinuxProcess) {
    use wcontainers_linux_process::RunEnd;
    let page = rustix::param::page_size();
    let (unmapped, held) = fixed_pages(&linux, FAULT_SEARCH_START, 1);
    drop(held);
    let unmapped = unmapped as u64 + (page / 2) as u64;
    let misaligned = CODE_STACK + 1;
    let fault = |address, write, alignment| RunEnd::Fault {
        address,
        write,
        alignment,
    };
    // Each instruction, the address in x1, the stack pointer, how the run ends, and whether it ends at the target of
    // a branch to x1 rather than at the instruction.
    let cases = [
        (
            LOAD_THROUGH_X1,
            unmapped,
            CODE_STACK,
            fault(unmapped, false, false),
            false,
        ),
        (
            STORE_THROUGH_X1,
            unmapped,
            CODE_STACK,
            fault(unmapped, true, false),
            false,
        ),
        (
            LOAD_THROUGH_SP,
            unmapped,
            misaligned,
            fault(misaligned, false, true),
            false,
        ),
        (
            UNDEFINED,
            unmapped,
            CODE_STACK,
            RunEnd::Raised(Signal::ILL),
            false,
        ),
        (
            BREAKPOINT,
            unmapped,
            CODE_STACK,
            RunEnd::Raised(Signal::TRAP),
            false,
        ),
        (
            BRANCH_TO_X1,
            0,
            CODE_STACK,
            RunEnd::Raised(Signal::ILL),
            true,
        ),
    ];
    let mut thread = linux.prepare_run_thread().expect("the thread is prepared");
    for (k, (instruction, address, stack, wanted, branches)) in cases.into_iter().enumerate() {
        let from = FAULT_SEARCH_START + (k + 1) * CODE_PAGES * page;
        let (pc, _code) = code_of(&linux, from, &[instruction, TO_EXIT[0], TO_EXIT[1]]);
        // A branch to a misaligned address in the code faults at that address.
        let (address, wanted, ends_at) = if branches {
            let target = pc + 1;
            (target, fault(target, false, true), target)
        } else {
            (address, wanted, pc)
        };
        let mut frame = frame_at(pc, address, stack);
        let before = frame.registers;
        THREAD_OWN.set(WORD);
        let end = linux.run_code(&mut thread, &mut frame);
        assert_eq!(end, wanted, "the instruction {instruction:#010x}");
        let mut at_fault = before;
        at_fault.x[EXIT_SCRATCH] = pc;
        at_fault.x[FRAME] = core::ptr::from_ref(&frame) as u64;
        assert_eq!(frame.registers.x, at_fault.x);
        assert_eq!(frame.registers.pc, ends_at);
        assert_eq!(frame.registers.sp, stack);
        assert_eq!(THREAD_OWN.get(), WORD, "the thread's own thread-local");
    }
}

/// Stops running code with the stop signal from another thread, and takes the signal on the thread's own signal stack,
/// never at the code's stack pointer, whether that points into this process's memory or at nothing.
#[cfg(target_arch = "aarch64")]
pub fn stops_a_run_on_its_signal_stack_whatever_the_code_s_stack_pointer(
    linux: impl LinuxProcess + Sync,
) {
    use wcontainers_linux_process::{RUN_STOP_SIGNAL, RunEnd};
    let page = rustix::param::page_size();
    let hostile = vec![WORD; HOSTILE_STACK_WORDS];
    let into_this_process = hostile.as_ptr() as u64 + (hostile.len() * size_of::<u64>()) as u64;
    let (nothing, held) = fixed_pages(&linux, STOP_SEARCH_START, 1);
    drop(held);
    let (pc, _code) = code_of(&linux, STOP_SEARCH_START + page, &[BRANCH_TO_ITSELF]);
    for stack in [into_this_process, nothing as u64 + page as u64, 0] {
        let ran = AtomicBool::new(false);
        let (tid_sender, tid) = std::sync::mpsc::channel();
        let end = std::thread::scope(|scope| {
            let linux = &linux;
            let ran = &ran;
            let runner = scope.spawn(move || {
                let mut thread = linux.prepare_run_thread().expect("the thread is prepared");
                tid_sender.send(own_thread()).expect("the test waits");
                THREAD_OWN.set(WORD);
                let mut frame = frame_at(pc, 0, stack);
                let end = loop {
                    match linux.run_code(&mut thread, &mut frame) {
                        RunEnd::NotEntered => {}
                        end => break end,
                    }
                };
                ran.store(true, Ordering::Release);
                (
                    end,
                    frame.registers.pc,
                    frame.registers.sp,
                    THREAD_OWN.get(),
                )
            });
            let thread = tid.recv().expect("the runner's thread");
            while !ran.load(Ordering::Acquire) {
                linux
                    .tgkill(own_process(), thread, RUN_STOP_SIGNAL)
                    .expect("the runner is signalled");
                std::thread::sleep(BETWEEN_STOPS);
            }
            runner.join().expect("the runner ends")
        });
        assert_eq!(
            end,
            (RunEnd::Stopped, pc, stack, WORD),
            "the stack {stack:#x}"
        );
        assert!(
            hostile.iter().all(|word| *word == WORD),
            "no signal frame written at the code's stack pointer"
        );
    }
}

/// A stop that reaches a prepared thread before its run ends the run before its code starts, the frame unchanged; the
/// next run goes on as usual.
#[cfg(target_arch = "aarch64")]
pub fn ends_a_run_before_its_code_at_a_stop_that_came_before(linux: impl LinuxProcess) {
    use wcontainers_linux_process::{RUN_STOP_SIGNAL, RunEnd};
    let (pc, _code) = code_of(&linux, EARLY_STOP_SEARCH_START, &TO_EXIT);
    let mut thread = linux.prepare_run_thread().expect("the thread is prepared");
    linux
        .tgkill(own_process(), own_thread(), RUN_STOP_SIGNAL)
        .expect("this thread is signalled");
    let mut frame = frame_at(pc, 0, CODE_STACK);
    let before = frame.clone();
    assert_eq!(linux.run_code(&mut thread, &mut frame), RunEnd::NotEntered);
    assert_eq!(frame.registers, before.registers);
    assert_eq!(
        linux.run_code(&mut thread, &mut frame),
        RunEnd::Exit(pc + EXIT_AT as u64)
    );
}

/// The signals this thread blocks, read from `/proc/thread-self/status`.
#[cfg(target_arch = "aarch64")]
fn blocked() -> u64 {
    let status = std::fs::read_to_string("/proc/thread-self/status").expect("the thread's status");
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix(BLOCKED_LINE))
        .expect("the blocked signals");
    u64::from_str_radix(line.trim(), HEXADECIMAL).expect("a hexadecimal mask")
}

/// Blocks every signal but a run's on the prepared thread alone, the two glibc keeps for itself among them; refuses a
/// second preparation; puts the mask back on drop.
#[cfg(target_arch = "aarch64")]
pub fn blocks_every_signal_but_a_run_s_on_the_prepared_thread(linux: impl LinuxProcess + Sync) {
    let before = blocked();
    let unblocked = UNBLOCKED
        .iter()
        .fold(0_u64, |kept, signal| kept | 1 << (signal.as_raw() - 1));
    // Another thread, started before the preparation, which a new thread's mask would be taken from.
    let (prepared_sender, prepared) = std::sync::mpsc::channel::<()>();
    let (other, thread) = std::thread::scope(|scope| {
        let other = scope.spawn(move || {
            prepared.recv().expect("the test prepares its thread");
            blocked()
        });
        let thread = linux.prepare_run_thread().expect("the thread is prepared");
        prepared_sender.send(()).expect("the other thread waits");
        (other.join().expect("the other thread ends"), thread)
    });
    assert_eq!(blocked(), !unblocked);
    assert_eq!(linux.prepare_run_thread().err(), Some(Errno::BUSY));
    assert_eq!(other, before, "another thread's mask");
    drop(thread);
    assert_eq!(blocked(), before);
    drop(
        linux
            .prepare_run_thread()
            .expect("prepared again after the drop"),
    );
}

/// Each new memory starts zeroed, and two are apart.
pub fn makes_each_shared_memory_apart_and_zeroed(linux: impl LinuxProcess) {
    let first = linux
        .create_shared_words("first", SHARED_BYTES)
        .expect("shared words");
    let second = linux
        .create_shared_words("second", SHARED_BYTES)
        .expect("shared words");
    linux.shared_words64(&first)[0].store(WORD, Ordering::Release);
    assert!(
        linux
            .shared_words64(&second)
            .iter()
            .all(|word| word.load(Ordering::Acquire) == 0)
    );
    assert!(linux.create_shared_words("empty", 0).is_err());
}

/// Reads the cache type, whose reserved bit is one.
#[cfg(target_arch = "aarch64")]
pub fn reads_the_cache_type(linux: impl LinuxProcess) {
    assert_eq!(linux.read_ctr_el0() & CTR_EL0_RES1, CTR_EL0_RES1);
}

/// Cleans and invalidates every line of a mapped buffer, leaving its bytes as they were.
#[cfg(target_arch = "aarch64")]
pub fn cleans_and_invalidates_the_lines_of_a_buffer(linux: impl LinuxProcess) {
    let held = vec![WORD; rustix::param::page_size() / size_of::<u64>()];
    let start = held.as_ptr() as usize;
    for address in (start..start + held.len() * size_of::<u64>()).step_by(size_of::<u64>()) {
        linux.dc_cvau(address);
        linux.ic_ivau(address);
    }
    assert!(held.iter().all(|word| *word == WORD));
}

/// On a host with SVE, a system call leaves p0 and FFR zero, as Linux's arm64 ABI says.
#[cfg(target_arch = "aarch64")]
pub fn finds_the_predicates_a_system_call_leaves(linux: impl LinuxProcess) {
    let found = linux.predicates_after_system_call();
    if std::arch::is_aarch64_feature_detected!("sve") {
        let found = found.expect("a host with SVE answers");
        assert!(found.p0.iter().chain(&found.ffr).all(|byte| *byte == 0));
    } else {
        assert_eq!(found, None);
    }
}

#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn signals_one_thread_of_another_process() {
            $crate::signals_one_thread_of_another_process($value);
        }

        #[test]
        fn refuses_a_thread_of_another_process() {
            $crate::refuses_a_thread_of_another_process($value);
        }

        #[test]
        fn maps_a_fixed_page_and_unmaps_it_on_drop() {
            $crate::maps_a_fixed_page_and_unmaps_it_on_drop($value);
        }

        #[test]
        fn refuses_to_replace_a_mapping() {
            $crate::refuses_to_replace_a_mapping($value);
        }

        #[test]
        fn refuses_a_page_at_a_bad_address_or_length() {
            $crate::refuses_a_page_at_a_bad_address_or_length($value);
        }

        #[test]
        fn maps_a_file_within_a_fixed_mapping() {
            $crate::maps_a_file_within_a_fixed_mapping($value);
        }

        #[test]
        fn refuses_pages_outside_a_fixed_mapping_or_executable() {
            $crate::refuses_pages_outside_a_fixed_mapping_or_executable($value);
        }

        #[test]
        fn shares_words_through_a_file_that_cannot_shrink() {
            $crate::shares_words_through_a_file_that_cannot_shrink($value);
        }

        #[test]
        fn maps_code_no_one_can_write() {
            $crate::maps_code_no_one_can_write($value);
        }

        #[cfg(target_arch = "aarch64")]
        #[test]
        fn runs_code_until_an_exit_and_keeps_the_thread_s_own() {
            $crate::runs_code_until_an_exit_and_keeps_the_thread_s_own($value);
        }

        #[cfg(target_arch = "aarch64")]
        #[test]
        fn ends_a_run_at_a_fault_of_its_code() {
            $crate::ends_a_run_at_a_fault_of_its_code($value);
        }

        #[cfg(target_arch = "aarch64")]
        #[test]
        fn stops_a_run_on_its_signal_stack_whatever_the_code_s_stack_pointer() {
            $crate::stops_a_run_on_its_signal_stack_whatever_the_code_s_stack_pointer($value);
        }

        #[cfg(target_arch = "aarch64")]
        #[test]
        fn ends_a_run_before_its_code_at_a_stop_that_came_before() {
            $crate::ends_a_run_before_its_code_at_a_stop_that_came_before($value);
        }

        #[cfg(target_arch = "aarch64")]
        #[test]
        fn blocks_every_signal_but_a_run_s_on_the_prepared_thread() {
            $crate::blocks_every_signal_but_a_run_s_on_the_prepared_thread($value);
        }

        #[test]
        fn makes_each_shared_memory_apart_and_zeroed() {
            $crate::makes_each_shared_memory_apart_and_zeroed($value);
        }

        #[cfg(target_arch = "aarch64")]
        #[test]
        fn reads_the_cache_type() {
            $crate::reads_the_cache_type($value);
        }

        #[cfg(target_arch = "aarch64")]
        #[test]
        fn cleans_and_invalidates_the_lines_of_a_buffer() {
            $crate::cleans_and_invalidates_the_lines_of_a_buffer($value);
        }

        #[cfg(target_arch = "aarch64")]
        #[test]
        fn finds_the_predicates_a_system_call_leaves() {
            $crate::finds_the_predicates_a_system_call_leaves($value);
        }
    };
}
