//! The tests every implementation of wcontainers-linux-process runs, written against its trait alone.
#![cfg(target_os = "linux")]

use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;

use nix::sys::signal::Signal as NixSignal;
use wcontainers_linux_process::{Errno, LinuxProcess, Pid, Signal};

/// Where the fixed page search starts: far above any heap or library a test process holds.
const SEARCH_START: usize = 1 << 40;
/// How many pages the search tries before it gives up.
const SEARCH_PAGES: usize = 1 << 12;
/// The bytes of the shared memory the tests make.
const SHARED_BYTES: usize = 1 << 16;
/// A word the tests write and read back.
const WORD: u64 = 0x0123_4567_89ab_cdef;
/// CTR_EL0's bit 31, which the architecture reserves as one.
#[cfg(target_arch = "aarch64")]
const CTR_EL0_RES1: u64 = 1 << 31;
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
    let (address, mapping) = fixed_page(&linux);
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
        fn shares_words_through_a_file_that_cannot_shrink() {
            $crate::shares_words_through_a_file_that_cannot_shrink($value);
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
