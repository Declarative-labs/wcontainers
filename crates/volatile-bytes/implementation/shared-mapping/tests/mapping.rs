//! The tests of what the unsafe code relies on: ranges refused, copies at page edges, copies while another thread or
//! process writes, and every check of a file mapping.
#![cfg(target_os = "linux")]

use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_shared_mapping::{Mapped, SharedMapping};

/// The bytes the other party writes, from the start: an odd count, so that its copies end mid-word.
const OTHER: usize = 1001;

/// The bytes this thread writes, right after the other party's.
const MINE: usize = 1003;

/// The byte this thread sets to stop the other party.
const STOP: u64 = 4000;

/// The rounds of this thread's copies while the other party writes.
const ROUNDS: usize = 20_000;

/// The two values the other party writes in turn, over bytes that start as zero.
const WRITTEN: [u8; 2] = [0x5a, 0xa5];

fn anonymous(length: usize) -> SharedMapping {
    SharedMapping::new(Mapped::anonymous(length).expect("the host maps memory"))
}

fn page() -> usize {
    rustix::param::page_size()
}

#[test]
fn refuses_parts_outside_the_view() {
    let view = anonymous(64).part(8..40).expect("inside");
    #[expect(clippy::reversed_empty_ranges, reason = "a start after the end is one of the cases refused")]
    for offsets in [0..33, 33..33, 9..8, 0..u64::MAX, u64::MAX..u64::MAX, 1 << 63..1 << 63] {
        assert!(view.part(offsets.clone()).is_none(), "{offsets:?}");
    }
    assert!(view.part(32..32).is_some_and(|part| part.is_empty()));
}

#[test]
fn reads_and_writes_only_the_bytes_of_a_part() {
    let whole = anonymous(64);
    let part = whole.part(8..40).and_then(|view| view.part(8..16)).expect("inside");
    assert_eq!(part.write(&[u8::MAX; 64]), 8);
    let mut all = [0; 64];
    assert_eq!(whole.read(&mut all), 64);
    for (index, byte) in all.iter().enumerate() {
        assert_eq!(*byte, if (16..24).contains(&index) { u8::MAX } else { 0 }, "{index}");
    }
    let empty = whole.part(64..64).expect("inside");
    assert_eq!((empty.write(&[1]), empty.read(&mut all)), (0, 0));
}

#[test]
fn copies_across_page_edges_exactly() {
    let page = page();
    let whole = anonymous(4 * page);
    let pattern: Vec<u8> = (0..2 * page).map(|index| (index * 7) as u8 | 1).collect();
    for edge in [page, 2 * page] {
        for count in (1..=65).chain([page - 1, page, page + 1]) {
            for before in [0, 1, 7, 8, count / 2, count].into_iter().filter(|&before| before + 16 <= edge) {
                let start = edge - before;
                // The bytes copied and 16 on either side, which start as zero and must stay so.
                let window = whole.part((start - 16) as u64..(start + count + 16) as u64).expect("inside");
                window.write(&vec![0; count + 32]);
                let part = whole.part(start as u64..(start + count) as u64).expect("inside");
                assert_eq!(part.write(&pattern[..count]), count);
                let mut all = vec![0; count + 32];
                window.read(&mut all);
                assert_eq!(all[16..16 + count], pattern[..count], "{count} at {start}");
                assert!(all[..16].iter().chain(&all[16 + count..]).all(|&byte| byte == 0), "{count} at {start}");
            }
        }
    }
}

/// Writes the other party's bytes, each of the two values in turn, until the stop byte is set.
fn write_until_stopped(memory: &SharedMapping) {
    let (other, stop) = (memory.part(0..OTHER as u64).expect("inside"), memory.part(STOP..STOP + 1).expect("inside"));
    let mut byte = [0];
    for round in 0.. {
        other.write(&[WRITTEN[round % 2]; OTHER]);
        stop.read(&mut byte);
        if byte[0] != 0 {
            return;
        }
    }
}

/// Copies this thread's bytes in and back while the other party writes the bytes before them, checks that each byte of
/// the other party's it reads holds a value written, and then stops the other party.
fn copy_while_another_writes(memory: &SharedMapping) {
    let other = memory.part(0..OTHER as u64).expect("inside");
    let mine = memory.part(OTHER as u64..(OTHER + MINE) as u64).expect("inside");
    let (mut read, mut seen) = (vec![0; OTHER.max(MINE)], [false; 2]);
    for round in 0..ROUNDS {
        let value = 0x10 + (round % 16) as u8;
        mine.write(&[value; MINE]);
        assert_eq!(mine.read(&mut read), MINE);
        assert!(read[..MINE].iter().all(|&byte| byte == value), "the other party's copies changed this thread's bytes");
        assert_eq!(other.read(&mut read), OTHER);
        for &byte in &read[..OTHER] {
            assert!(byte == 0 || WRITTEN.contains(&byte), "{byte:#x} was never written");
            seen.iter_mut().zip(WRITTEN).for_each(|(seen, written)| *seen |= byte == written);
        }
    }
    memory.part(STOP..STOP + 1).expect("inside").write(&[1]);
    assert_eq!(seen, [true; 2], "the other party's writes were never seen");
}

#[cfg(not(miri))]
#[test]
fn copies_while_another_thread_writes() {
    let memory = anonymous(2 * STOP as usize);
    std::thread::scope(|scope| {
        scope.spawn(|| write_until_stopped(&memory));
        copy_while_another_writes(&memory);
    });
}

#[cfg(not(miri))]
mod file {
    use std::os::fd::{AsFd, AsRawFd, OwnedFd};
    use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
    use std::time::{Duration, Instant};

    use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, fstat, ftruncate, memfd_create};
    use rustix::io::Errno;
    use rustix::process::{Signal, set_parent_process_death_signal};
    use wcontainers_volatile_bytes_shared_mapping::MapError;

    use super::*;

    /// The variable that names the memory file to the other process.
    const PATH: &str = "SHARED_MAPPING_PATH";

    fn memory_file(length: u64, seals: SealFlags) -> OwnedFd {
        let file = memfd_create("mapping", MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING).expect("a memory file");
        ftruncate(&file, length).expect("the file grows");
        if !seals.is_empty() {
            fcntl_add_seals(&file, seals).expect("the file takes the seals");
        }
        file
    }

    fn map(file: impl AsFd, offset: u64, length: usize) -> Result<SharedMapping, MapError> {
        Mapped::file(file.as_fd(), offset, length).map(SharedMapping::new)
    }

    /// In the other process only: maps the file and writes until stopped, or until the process that started it ends.
    #[test]
    fn the_other_process() {
        let Ok(path) = std::env::var(PATH) else {
            return;
        };
        set_parent_process_death_signal(Some(Signal::KILL)).expect("the parent-death signal is set");
        let file = std::fs::OpenOptions::new().read(true).write(true).open(path).expect("the file opens");
        write_until_stopped(&map(&file, 0, 2 * STOP as usize).expect("the host maps the file"));
    }

    #[test]
    fn copies_while_another_process_writes() {
        let file = memory_file(2 * STOP, SealFlags::SHRINK);
        let memory = map(&file, 0, 2 * STOP as usize).expect("the host maps the file");
        let mut child = std::process::Command::new(std::env::current_exe().expect("this test binary"))
            .args(["--exact", "file::the_other_process", "--test-threads=1"])
            .env(PATH, format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd()))
            .spawn()
            .expect("the other process starts");
        let (deadline, mut first) = (Instant::now() + Duration::from_secs(30), [0]);
        while memory.read(&mut first) == 1 && first[0] == 0 {
            assert!(Instant::now() < deadline, "the other process never started");
        }
        copy_while_another_writes(&memory);
        assert!(child.wait().expect("the other process ends").success());
    }

    #[test]
    fn refuses_a_file_not_sealed_against_shrinking() {
        for seals in [SealFlags::empty(), SealFlags::GROW, SealFlags::SEAL, SealFlags::GROW | SealFlags::SEAL] {
            assert!(matches!(map(memory_file(64, seals), 0, 64), Err(MapError::Unsealed)), "{seals:?}");
        }
        let unsealable = memfd_create("mapping", MemfdFlags::CLOEXEC).expect("a memory file");
        ftruncate(&unsealable, 64).expect("the file grows");
        assert!(matches!(map(unsealable, 0, 64), Err(MapError::Unsealed)));
        let (pipe, _writer) = std::io::pipe().expect("a pipe");
        assert!(matches!(map(pipe, 0, 64), Err(MapError::Unsealed)));
        let sealed = memory_file(64, SealFlags::SHRINK);
        assert!(map(&sealed, 0, 64).is_ok());
        assert_eq!(ftruncate(&sealed, 0), Err(Errno::PERM));
    }

    #[test]
    fn refuses_no_length_and_offsets_not_a_multiple_of_8() {
        let file = memory_file(4 * page() as u64, SealFlags::SHRINK);
        assert!(matches!(map(&file, 0, 0), Err(MapError::Length)));
        for start in [0, 8, page() as u64, 1 << 32, u64::MAX - 7] {
            for offset in (1..8).map(|rest| start + rest) {
                assert!(matches!(map(&file, offset, 1), Err(MapError::Length)), "{offset}");
            }
        }
    }

    #[test]
    fn refuses_bytes_past_the_end_of_the_file() {
        let size = page() as u64 + 3;
        let file = memory_file(size, SealFlags::SHRINK);
        for (offset, length) in [(0, size), (8, size - 8), (size - 3, 3)] {
            assert_eq!(map(&file, offset, length as usize).expect("inside the file").len(), length);
        }
        for (offset, length) in [(0, size + 1), (8, size - 7), (size + 5, 1), (u64::MAX - 7, 8), (8, u64::MAX)] {
            assert!(matches!(map(&file, offset, length as usize), Err(MapError::Short)), "{offset} {length}");
        }
    }

    #[test]
    fn maps_the_files_bytes_from_the_offset() {
        let file = memory_file(2 * page() as u64, SealFlags::SHRINK);
        let every: Vec<u8> = (0..2 * page()).map(|index| (index * 7) as u8).collect();
        assert_eq!(rustix::io::pwrite(&file, &every, 0), Ok(every.len()));
        let offset = page() - 8;
        let mut read = vec![0; page()];
        assert_eq!(map(&file, offset as u64, page()).expect("inside").read(&mut read), page());
        assert_eq!(read, every[offset..offset + page()]);
    }

    #[test]
    fn refuses_huge_pages_and_a_descriptor_that_cannot_write() {
        let file = memory_file(64, SealFlags::SHRINK);
        let reader = std::fs::File::open(format!("/proc/self/fd/{}", file.as_raw_fd())).expect("the file opens again");
        assert!(matches!(map(reader, 0, 64), Err(MapError::Refused(_))));
        let flags = MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING | MemfdFlags::HUGETLB;
        let huge = memfd_create("mapping", flags)
            .ok()
            .filter(|file| ftruncate(file, 2 << 20).is_ok() && fcntl_add_seals(file, SealFlags::SHRINK).is_ok());
        match huge {
            Some(file) => assert!(matches!(map(file, 0, 2 << 20), Err(MapError::HugePages))),
            None => eprintln!("this host makes no sealed memory file of huge pages"),
        }
    }

    /// Races the checks against another thread that shrinks the file to nothing and then seals it: were the size read
    /// before the seals, a mapping would be made over bytes the file no longer has.
    #[test]
    fn a_shrink_then_seal_racing_the_checks_never_maps_past_the_end() {
        let (deadline, found) = (Instant::now() + Duration::from_secs(4), AtomicBool::new(false));
        std::thread::scope(|scope| {
            for pair in 0..8_u64 {
                let found = &found;
                scope.spawn(move || {
                    let mut seed = pair.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
                    let mut spins = move || {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        seed % (1 << 12)
                    };
                    while Instant::now() < deadline {
                        let file = memory_file(2 * page() as u64, SealFlags::empty());
                        let (mine, theirs, go) = (spins(), spins(), AtomicBool::new(false));
                        let mapped = std::thread::scope(|inner| {
                            inner.spawn(|| {
                                while !go.load(SeqCst) {
                                    std::hint::spin_loop();
                                }
                                (0..theirs).for_each(|_| std::hint::spin_loop());
                                ftruncate(&file, 0).expect("the file shrinks");
                                fcntl_add_seals(&file, SealFlags::SHRINK).expect("the file takes the seal");
                            });
                            go.store(true, SeqCst);
                            (0..mine).for_each(|_| std::hint::spin_loop());
                            map(&file, 0, 2 * page())
                        });
                        if mapped.is_ok_and(|memory| fstat(&file).expect("the size").st_size < memory.len() as i64) {
                            found.store(true, SeqCst);
                        }
                    }
                });
            }
        });
        assert!(!found.load(SeqCst), "a file the other thread shrank was mapped");
    }
}
