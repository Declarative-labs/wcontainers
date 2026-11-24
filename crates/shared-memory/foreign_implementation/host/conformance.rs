#![cfg(unix)]

use wcontainers_shared_memory::{HostMapping, OutsideError, Words};

/// The bytes of each mapping: one page of the smallest size hosts use.
const LENGTH: usize = 4096;

/// The bytes of one word.
const WORD: usize = size_of::<u64>();

/// Checks that the words count exactly the bytes mapped, reach the last of them, and refuse every byte after.
fn counts_exactly(words: &Words, length: usize) {
    assert_eq!(words.count(), (length / WORD) as u64);
    let last = length as u64 - 1;
    assert_eq!(words.write(last, &[0xa5]), Ok(()));
    let mut byte = [0];
    assert_eq!(words.read(last, &mut byte), Ok(()));
    assert_eq!(byte, [0xa5]);
    for at in [length as u64, LENGTH as u64 - 1, LENGTH as u64] {
        if at >= length as u64 {
            assert_eq!(words.read(at, &mut [0]), Err(OutsideError), "{at}");
            assert_eq!(words.write(at, &[0]), Err(OutsideError), "{at}");
        }
    }
    assert_eq!(words.load((length / WORD) as u64), Err(OutsideError));
    assert!(words.part(0..length as u64 + 1).is_err());
}

mod anonymous {
    use super::*;

    wcontainers_shared_memory_conformance::tests!(Words::new(
        HostMapping::anonymous(LENGTH).expect("the host maps a page")
    ));

    mod volatile_bytes {
        use super::*;

        wcontainers_volatile_bytes_conformance::tests!(
            Words::new(HostMapping::anonymous(LENGTH).expect("the host maps a page"))
                .part(3..LENGTH as u64 - 1)
                .expect("the offsets lie inside the mapping")
        );
    }

    #[test]
    fn refuses_a_length_not_a_multiple_of_a_word() {
        for length in [0, 1, 7, LENGTH + 1] {
            assert!(HostMapping::anonymous(length).is_err(), "{length}");
        }
    }

    #[test]
    fn counts_only_the_bytes_asked_when_they_are_not_whole_pages() {
        for length in [
            WORD,
            2 * WORD,
            LENGTH - WORD,
            LENGTH + WORD,
            3 * LENGTH - 5 * WORD,
        ] {
            let words =
                Words::new(HostMapping::anonymous(length).expect("the host maps the bytes"));
            counts_exactly(&words, length);
        }
    }
}

#[cfg(all(target_os = "linux", not(miri)))]
mod file {
    use std::fs::OpenOptions;
    use std::os::fd::{AsFd, OwnedFd};

    use rustix::fs::{
        FallocateFlags, MemfdFlags, SealFlags, fallocate, fcntl_add_seals, ftruncate, memfd_create,
    };
    use rustix::io::Errno;
    use wcontainers_shared_memory::{FileError, Mapped};

    use super::*;

    /// Returns a memory file of the length with the seals.
    fn memory_file(length: u64, seals: SealFlags) -> OwnedFd {
        let file = memfd_create("words", MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
            .expect("a memory file");
        ftruncate(&file, length).expect("the file grows");
        if !seals.is_empty() {
            fcntl_add_seals(&file, seals).expect("the file takes the seals");
        }
        file
    }

    /// Returns words over a sealed memory file of the test's length; the mapping outlives the descriptor.
    fn words() -> Words {
        let file = memory_file(LENGTH as u64, SealFlags::SHRINK);
        Words::new(HostMapping::file(file.as_fd(), 0, LENGTH).expect("the host maps the file"))
    }

    /// Opens the file again through the process's descriptor table, as a second, separate opening of the same file.
    fn reopen(file: &OwnedFd, write: bool) -> std::fs::File {
        use std::os::fd::AsRawFd;

        OpenOptions::new()
            .read(true)
            .write(write)
            .open(format!("/proc/self/fd/{}", file.as_raw_fd()))
            .expect("the file opens again")
    }

    wcontainers_shared_memory_conformance::tests!(words());

    mod volatile_bytes {
        use super::*;

        wcontainers_volatile_bytes_conformance::tests!(
            words()
                .part(3..LENGTH as u64 - 1)
                .expect("the offsets lie inside the mapping")
        );
    }

    #[test]
    fn maps_a_file_sealed_against_shrinking_and_refuses_every_other_set_of_seals() {
        let every = [
            SealFlags::SEAL,
            SealFlags::SHRINK,
            SealFlags::GROW,
            SealFlags::WRITE,
            SealFlags::FUTURE_WRITE,
        ];
        for chosen in 0..1_u32 << every.len() {
            let seals = every
                .iter()
                .enumerate()
                .filter(|(bit, _)| chosen & 1 << bit != 0)
                .fold(SealFlags::empty(), |seals, (_, &seal)| seals | seal);
            let file = memory_file(LENGTH as u64, seals);
            let mapped = HostMapping::file(file.as_fd(), 0, LENGTH);
            if !seals.contains(SealFlags::SHRINK) {
                assert!(matches!(mapped, Err(FileError::Unsealed)), "{seals:?}");
            } else if seals.intersects(SealFlags::WRITE | SealFlags::FUTURE_WRITE) {
                assert!(matches!(mapped, Err(FileError::Refused(_))), "{seals:?}");
            } else {
                let words = Words::new(mapped.expect("a file sealed against shrinking maps"));
                assert_eq!(ftruncate(&file, 0), Err(Errno::PERM), "{seals:?}");
                counts_exactly(&words, LENGTH);
            }
        }
    }

    #[test]
    fn refuses_a_file_sealed_only_against_growing_before_a_safe_shrink_could_fault() {
        let file = memory_file(2 * LENGTH as u64, SealFlags::GROW);
        let mapped = HostMapping::file(file.as_fd(), 0, 2 * LENGTH);
        if let Ok(mapping) = mapped {
            let words = Words::new(mapping);
            ftruncate(&file, 0).expect("the file shrinks");
            // With the mapping wrongly made, this load raises SIGBUS and ends the test binary.
            let _ = words.load(0);
            panic!("mapped a file that can shrink");
        }
        assert!(matches!(mapped, Err(FileError::Unsealed)));
    }

    #[test]
    fn refuses_a_file_that_cannot_be_sealed() {
        let file = memfd_create("words", MemfdFlags::CLOEXEC).expect("a memory file");
        ftruncate(&file, LENGTH as u64).expect("the file grows");
        assert!(matches!(
            HostMapping::file(file.as_fd(), 0, LENGTH),
            Err(FileError::Unsealed)
        ));
        let path = std::env::temp_dir().join(format!("words-{}", std::process::id()));
        let ordinary = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("an ordinary file");
        std::fs::remove_file(&path).expect("the file's name goes");
        ordinary.set_len(LENGTH as u64).expect("the file grows");
        assert!(matches!(
            HostMapping::file(ordinary.as_fd(), 0, LENGTH),
            Err(FileError::Unsealed)
        ));
    }

    #[test]
    fn refuses_a_descriptor_that_cannot_write() {
        let file = memory_file(LENGTH as u64, SealFlags::SHRINK);
        let reader = reopen(&file, false);
        assert!(matches!(
            HostMapping::file(reader.as_fd(), 0, LENGTH),
            Err(FileError::Refused(_))
        ));
    }

    #[test]
    fn refuses_huge_pages() {
        let Ok(file) = memfd_create(
            "words",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING | MemfdFlags::HUGETLB,
        ) else {
            eprintln!("this host makes no memory file of huge pages");
            return;
        };
        let huge = 2 << 20;
        if ftruncate(&file, huge).is_err() || fcntl_add_seals(&file, SealFlags::SHRINK).is_err() {
            eprintln!("this host cannot size or seal a memory file of huge pages");
            return;
        }
        assert!(matches!(
            HostMapping::file(file.as_fd(), 0, huge as usize),
            Err(FileError::HugePages)
        ));
    }

    #[test]
    fn counts_only_the_bytes_asked_when_they_are_not_whole_pages() {
        let file = memory_file(3 * LENGTH as u64, SealFlags::SHRINK);
        for (offset, length) in [
            (0, LENGTH - WORD),
            (0, WORD),
            (WORD as u64, LENGTH - 2 * WORD),
            (LENGTH as u64 - WORD as u64, LENGTH + 2 * WORD),
            (LENGTH as u64 + 3 * WORD as u64, LENGTH),
        ] {
            let words = Words::new(
                HostMapping::file(file.as_fd(), offset, length).expect("the host maps the file"),
            );
            counts_exactly(&words, length);
        }
    }

    #[test]
    fn refuses_offsets_past_the_end_of_the_file() {
        let file = memory_file(LENGTH as u64, SealFlags::SHRINK);
        assert!(matches!(
            HostMapping::file(file.as_fd(), 8, LENGTH),
            Err(FileError::Short)
        ));
        assert!(matches!(
            HostMapping::file(file.as_fd(), u64::MAX - 7, 8),
            Err(FileError::Short)
        ));
        assert!(matches!(
            HostMapping::file(file.as_fd(), 4, 8),
            Err(FileError::Length)
        ));
    }

    #[test]
    fn sees_what_another_mapping_of_the_file_writes() {
        let file = memory_file(LENGTH as u64, SealFlags::SHRINK);
        let first =
            Words::new(HostMapping::file(file.as_fd(), 0, LENGTH).expect("the host maps the file"));
        let second =
            Words::new(HostMapping::file(file.as_fd(), 0, LENGTH).expect("the host maps the file"));
        assert_eq!(first.store(3, u64::MAX), Ok(()));
        assert_eq!(second.load(3), Ok(u64::MAX));
    }

    #[test]
    fn maps_the_files_bytes_from_the_offset_asked_up_to_its_last_byte() {
        for (offset, length) in [
            (0, LENGTH),
            (WORD as u64, LENGTH - WORD),
            (LENGTH as u64 - WORD as u64, 2 * WORD),
            (LENGTH as u64 + 3 * WORD as u64, LENGTH - 3 * WORD),
        ] {
            let end = offset + length as u64;
            let file = memory_file(end, SealFlags::SHRINK);
            let every: Vec<u8> = (0..end / WORD as u64)
                .flat_map(|index| (index + 1).to_ne_bytes())
                .collect();
            assert_eq!(rustix::io::pwrite(&file, &every, 0), Ok(every.len()));
            let words = Words::new(
                HostMapping::file(file.as_fd(), offset, length).expect("the host maps the file"),
            );
            let first = offset / WORD as u64 + 1;
            assert_eq!(words.load(0), Ok(first), "{offset}");
            let last = words.count() - 1;
            assert_eq!(words.load(last), Ok(end / WORD as u64), "{offset}");
            let mut read = vec![0; length];
            assert_eq!(words.read(0, &mut read), Ok(()));
            assert_eq!(read, every[offset as usize..], "{offset}");
        }
    }

    /// Races `HostMapping::file` against another thread that shrinks the file to nothing and then seals it.
    ///
    /// If the size were read before the seals, a call could see the file whole, then see it sealed after the shrink,
    /// and map bytes past its end, which fault. Several pairs race at once, each try at a different spin of each side;
    /// the test never touches a mapping, it asks whether one was made over bytes the file no longer has.
    #[test]
    fn a_shrink_then_seal_racing_the_checks_never_maps_past_the_end() {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
        use std::time::{Duration, Instant};

        /// The longest the test runs when no wrong mapping turns up.
        const RUN: Duration = Duration::from_secs(4);
        /// The pairs of racing threads.
        const PAIRS: usize = 8;
        /// The most spins either side waits before it starts, so that tries cover every overlap of the two.
        const SPINS: u64 = 1 << 12;

        let deadline = Instant::now() + RUN;
        let (tries, found) = (AtomicU64::new(0), AtomicBool::new(false));
        std::thread::scope(|scope| {
            for pair in 0..PAIRS as u64 {
                let (tries, found) = (&tries, &found);
                scope.spawn(move || {
                    let mut seed = pair.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
                    let mut spins = move || {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        seed % SPINS
                    };
                    while Instant::now() < deadline && !found.load(SeqCst) {
                        tries.fetch_add(1, SeqCst);
                        let file = memory_file(2 * LENGTH as u64, SealFlags::empty());
                        let (mine, theirs, go) = (spins(), spins(), AtomicBool::new(false));
                        let mapped = std::thread::scope(|inner| {
                            inner.spawn(|| {
                                while !go.load(SeqCst) {
                                    std::hint::spin_loop();
                                }
                                (0..theirs).for_each(|_| std::hint::spin_loop());
                                ftruncate(&file, 0).expect("the file shrinks");
                                fcntl_add_seals(&file, SealFlags::SHRINK)
                                    .expect("the file takes the seal");
                            });
                            go.store(true, SeqCst);
                            (0..mine).for_each(|_| std::hint::spin_loop());
                            HostMapping::file(file.as_fd(), 0, 2 * LENGTH)
                        });
                        if let Ok(mapping) = mapped {
                            let size = rustix::fs::fstat(&file).expect("the file's size").st_size;
                            if size < mapping.len() as i64 {
                                found.store(true, SeqCst);
                            }
                        }
                    }
                });
            }
        });
        let tries = tries.load(SeqCst);
        eprintln!("{tries} tries in {RUN:?} or less");
        assert!(
            !found.load(SeqCst),
            "a file the other thread shrank was mapped, after {tries} tries"
        );
    }

    #[test]
    fn keeps_every_byte_while_another_descriptor_shrinks_truncates_grows_or_punches_the_file() {
        let file = memory_file(2 * LENGTH as u64, SealFlags::SHRINK);
        let words = Words::new(
            HostMapping::file(file.as_fd(), LENGTH as u64, LENGTH).expect("the host maps the file"),
        );
        assert_eq!(words.write(0, &[0x5a; LENGTH]), Ok(()));
        let other = reopen(&file, true);
        assert_eq!(ftruncate(&other, LENGTH as u64), Err(Errno::PERM));
        assert_eq!(ftruncate(&other, 0), Err(Errno::PERM));
        let truncating = OpenOptions::new().write(true).truncate(true).open(format!(
            "/proc/self/fd/{}",
            std::os::fd::AsRawFd::as_raw_fd(&file)
        ));
        assert!(truncating.is_err(), "opening with truncation is a shrink");
        let mut held = vec![0; LENGTH];
        assert_eq!(words.read(0, &mut held), Ok(()));
        assert_eq!(held, vec![0x5a; LENGTH]);

        ftruncate(&other, 64 * LENGTH as u64).expect("a file sealed only against shrinking grows");
        counts_exactly(&words, LENGTH);

        fallocate(
            &other,
            FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE,
            0,
            3 * LENGTH as u64,
        )
        .expect("the file's pages are given back");
        assert_eq!(words.read(0, &mut held), Ok(()));
        assert_eq!(held, vec![0; LENGTH], "a punched page reads as zeros");
        assert_eq!(words.store(1, 7), Ok(()));
        assert_eq!(words.load(1), Ok(7));
    }
}
