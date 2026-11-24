#![cfg(unix)]

use wcontainers_shared_memory::{HostMapping, OutsideError, SharedMemory, WordError};

/// The bytes of one word.
const WORD: usize = size_of::<u64>();

/// Returns the host's page size; under Miri, which copies a byte at a time, two cache lines instead.
fn page() -> usize {
    if cfg!(miri) {
        2 * wcontainers_shared_memory_conformance::CACHE_LINE
    } else {
        rustix::param::page_size()
    }
}

/// Returns the bytes of each mapping of the suite: three pages, so that it holds two page boundaries.
fn length() -> usize {
    3 * page()
}

/// Checks that the memory counts exactly the bytes mapped, reaches the last of them, and refuses every byte after.
fn counts_exactly(memory: &HostMapping, length: usize) {
    assert_eq!(memory.len(), length as u64);
    let last = length as u64 - 1;
    assert_eq!(memory.write(last, &[0xa5]), Ok(()));
    let mut byte = [0];
    assert_eq!(memory.read(last, &mut byte), Ok(()));
    assert_eq!(byte, [0xa5]);
    for at in [length as u64, length as u64 + 1, page() as u64] {
        if at >= length as u64 {
            assert_eq!(memory.read(at, &mut [0]), Err(OutsideError), "{at}");
            assert_eq!(memory.write(at, &[0]), Err(OutsideError), "{at}");
        }
    }
    let past = (length as u64).next_multiple_of(WORD as u64);
    assert_eq!(
        memory.load::<u64>(past),
        Err(WordError::Outside(OutsideError))
    );
    assert!(memory.borrow(0..length as u64 + 1).is_err());
}

mod anonymous {
    use super::*;

    wcontainers_shared_memory_conformance::tests!(
        HostMapping::anonymous(length()).expect("the host maps the pages"),
        page()
    );

    mod volatile_bytes {
        use super::*;

        wcontainers_volatile_bytes_conformance::tests!(
            HostMapping::anonymous(length())
                .expect("the host maps the pages")
                .borrow(3..length() as u64 - 1)
                .expect("the offsets lie inside the mapping")
        );
    }

    /// Checks copies far longer than the suite's, past the lengths at which a C library switches to its copies for large
    /// sizes, from starts and buffers at several alignments: each copies exactly its bytes and changes none around them.
    #[cfg(not(miri))]
    #[test]
    fn copies_long_lengths_exactly() {
        /// The longest copy: past the size at which glibc's x86-64 copy uses stores that bypass the cache on most hosts.
        const LONGEST: usize = 9 << 20;
        /// The bytes either side of each copy that are checked unchanged.
        const AROUND: usize = 64;
        /// The byte that fills the memory and the buffers outside the bytes copied.
        const SENTINEL: u8 = 0xee;

        let length = LONGEST + 2 * page();
        let memory = HostMapping::anonymous(length).expect("the host maps the pages");
        let source: Vec<u8> = (0..LONGEST + 16)
            .map(|index| (index * 31 + index / 251) as u8)
            .collect();
        for count in [(2 << 20) - 1, 2 << 20, (2 << 20) + 1, LONGEST - 3] {
            for shift in [0, 1, 7, 15] {
                assert_eq!(memory.write(0, &vec![SENTINEL; length]), Ok(()));
                let at = page() - shift;
                let written = &source[shift..shift + count];
                assert_eq!(memory.write(at as u64, written), Ok(()));
                let mut into = vec![SENTINEL; count + 2 * AROUND];
                assert_eq!(memory.read(at as u64, &mut into[AROUND + shift % 8..][..count]), Ok(()));
                assert!(into[AROUND + shift % 8..][..count] == *written, "{count} bytes at {at}");
                let mut around = vec![0; AROUND];
                for edge in [at - AROUND, at + count] {
                    assert_eq!(memory.read(edge as u64, &mut around), Ok(()));
                    assert!(around.iter().all(|&byte| byte == SENTINEL), "{count} bytes at {at}");
                }
                assert!(
                    into[..AROUND + shift % 8]
                        .iter()
                        .chain(&into[AROUND + shift % 8 + count..])
                        .all(|&byte| byte == SENTINEL),
                    "a read of {count} bytes at {at} wrote past its buffer"
                );
            }
        }
    }

    #[test]
    fn refuses_no_length() {
        assert!(HostMapping::anonymous(0).is_err());
    }

    #[test]
    fn counts_only_the_bytes_asked_when_they_are_not_whole_pages() {
        for length in [1, WORD - 1, WORD, page() - 1, page() + 1, 3 * page() - 5] {
            let memory = HostMapping::anonymous(length).expect("the host maps the bytes");
            counts_exactly(&memory, length);
        }
    }
}

#[cfg(all(target_os = "linux", not(miri)))]
mod file {
    use std::fs::OpenOptions;
    use std::os::fd::{AsFd, OwnedFd};

    use rustix::fs::{
        FallocateFlags, MemfdFlags, SealFlags, fallocate, fcntl_add_seals, fcntl_get_seals, ftruncate,
        memfd_create,
    };
    use rustix::io::Errno;
    use wcontainers_shared_memory::FileError;

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

    /// Returns a mapping of a sealed memory file of the suite's length; the mapping outlives the descriptor.
    fn mapped() -> HostMapping {
        let file = memory_file(length() as u64, SealFlags::SHRINK);
        HostMapping::file(file.as_fd(), 0, length()).expect("the host maps the file")
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

    wcontainers_shared_memory_conformance::tests!(mapped(), page());

    mod volatile_bytes {
        use super::*;

        wcontainers_volatile_bytes_conformance::tests!(
            mapped()
                .borrow(3..length() as u64 - 1)
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
            let file = memory_file(page() as u64, seals);
            let mapped = HostMapping::file(file.as_fd(), 0, page());
            if !seals.contains(SealFlags::SHRINK) {
                assert!(matches!(mapped, Err(FileError::Unsealed)), "{seals:?}");
            } else if seals.intersects(SealFlags::WRITE | SealFlags::FUTURE_WRITE) {
                assert!(matches!(mapped, Err(FileError::Refused(_))), "{seals:?}");
            } else {
                let memory = mapped.expect("a file sealed against shrinking maps");
                assert_eq!(ftruncate(&file, 0), Err(Errno::PERM), "{seals:?}");
                counts_exactly(&memory, page());
            }
        }
    }

    #[test]
    fn refuses_a_file_sealed_only_against_growing_before_a_safe_shrink_could_fault() {
        let file = memory_file(2 * page() as u64, SealFlags::GROW);
        let mapped = HostMapping::file(file.as_fd(), 0, 2 * page());
        if let Ok(mapping) = mapped {
            let memory = mapping;
            ftruncate(&file, 0).expect("the file shrinks");
            // With the mapping wrongly made, this load raises SIGBUS and ends the test binary.
            let _ = memory.load::<u64>(0);
            panic!("mapped a file that can shrink");
        }
        assert!(matches!(mapped, Err(FileError::Unsealed)));
    }

    /// Refuses a memory file made without sealing, and a file whose seals cannot be read at all.
    ///
    /// The second is a pipe: the kernel keeps seals only for memory files, and a pipe is not one on any host, wherever
    /// the temporary folder lives (an ordinary file there has readable seals when the folder is on tmpfs, so it cannot
    /// stand in). Were unreadable seals taken as a seal, the pipe would pass the check and be refused later, as too
    /// short, and this test fails. If a host ever reads a pipe's seals, the test fails rather than pass untried.
    #[test]
    fn refuses_a_file_that_cannot_be_sealed() {
        let file = memfd_create("words", MemfdFlags::CLOEXEC).expect("a memory file");
        ftruncate(&file, page() as u64).expect("the file grows");
        assert!(matches!(
            HostMapping::file(file.as_fd(), 0, page()),
            Err(FileError::Unsealed)
        ));
        let (pipe, _writer) = std::io::pipe().expect("a pipe");
        assert!(
            fcntl_get_seals(&pipe).is_err(),
            "this host reads a pipe's seals, so the test has no file whose seals cannot be read"
        );
        assert!(matches!(
            HostMapping::file(pipe.as_fd(), 0, page()),
            Err(FileError::Unsealed)
        ));
    }

    /// Refuses a length of zero at every offset, aligned or not.
    #[test]
    fn refuses_no_length() {
        let file = memory_file(page() as u64, SealFlags::SHRINK);
        for offset in [0, WORD as u64, 4] {
            assert!(
                matches!(HostMapping::file(file.as_fd(), offset, 0), Err(FileError::Length)),
                "{offset}"
            );
        }
    }

    #[test]
    fn refuses_a_descriptor_that_cannot_write() {
        let file = memory_file(page() as u64, SealFlags::SHRINK);
        let reader = reopen(&file, false);
        assert!(matches!(
            HostMapping::file(reader.as_fd(), 0, page()),
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

    /// Maps any length from an offset that is a multiple of 8, whole words or not, up to the file's last byte.
    #[test]
    fn counts_only_the_bytes_asked_when_they_are_not_whole_pages() {
        let file = memory_file(3 * page() as u64, SealFlags::SHRINK);
        for (offset, length) in [
            (0, page() - WORD),
            (0, WORD),
            (WORD as u64, page() - 2 * WORD),
            (page() as u64 - WORD as u64, page() + 2 * WORD),
            (page() as u64 + 3 * WORD as u64, page()),
            (0, 1),
            (0, WORD - 1),
            (2 * WORD as u64, WORD + 3),
            (WORD as u64, page() + 1),
            (2 * page() as u64 + WORD as u64, page() - WORD),
            (2 * page() as u64 + 5 * WORD as u64, page() - 5 * WORD - 1),
        ] {
            let memory =
                HostMapping::file(file.as_fd(), offset, length).expect("the host maps the file");
            counts_exactly(&memory, length);
        }
    }

    #[test]
    fn refuses_offsets_past_the_end_of_the_file() {
        let file = memory_file(page() as u64, SealFlags::SHRINK);
        for (offset, length) in [
            (8, page()),
            (0, page() + 1),
            (8, page() - 7),
            (page() as u64, 1),
            (u64::MAX - 7, 8),
            (u64::MAX - 7, 7),
            (8, usize::MAX),
            (page() as u64, usize::MAX - page() + 1),
            (1 << 32, 8),
            (0, (1 << 32) + 1),
            (1 << 40, 8),
            (0, 1 << 40),
        ] {
            assert!(
                matches!(HostMapping::file(file.as_fd(), offset, length), Err(FileError::Short)),
                "{offset} {length}"
            );
        }
    }

    /// Refuses, as `Length`, every offset that is not a multiple of 8: each remainder from 1 to 7, after a page, two
    /// and three pages (inside the file), and after 2^32, 2^63 and the last word of the range (past it), at lengths
    /// that end on a word and lengths that do not. An offset check limited to small offsets, or to offsets below any
    /// other bound, a check of another modulus, or of the end's alignment in place of the offset's, fails here, and not
    /// only at the alignment assert in `HostMapping::new`.
    #[test]
    fn refuses_every_offset_that_is_not_a_multiple_of_8() {
        let file = memory_file(4 * page() as u64, SealFlags::SHRINK);
        let page = page() as u64;
        let word = WORD as u64;
        for start in [0, word, page, page + word, 2 * page, 3 * page, 1 << 32, 1 << 63, u64::MAX - 7] {
            for offset in (1..word).map(|rest| start + rest) {
                for length in [1, 4, WORD, (word - offset % word) as usize, page as usize] {
                    assert!(
                        matches!(
                            HostMapping::file(file.as_fd(), offset, length),
                            Err(FileError::Length)
                        ),
                        "{offset} {length}"
                    );
                }
            }
        }
    }

    /// Maps a file whose size is not a whole number of pages, or of words, to exactly its last byte, from several
    /// offsets, and refuses one byte more, one word more, the rest of its last page and one page more. A check that
    /// lets the end run to the end of the file's last page or word, or any number of bytes past the file, fails here.
    #[test]
    fn maps_a_file_that_is_not_whole_pages_to_its_last_byte_and_no_further() {
        let page = page() as u64;
        let word = WORD as u64;
        for size in [1, word - 1, word, word + 1, 100, page - 1, page + 1, page + 100, 2 * page + word + 3] {
            let file = memory_file(size, SealFlags::SHRINK);
            for offset in [0, word, page, page + word].into_iter().filter(|&offset| offset < size) {
                let length = (size - offset) as usize;
                let memory = HostMapping::file(file.as_fd(), offset, length)
                    .unwrap_or_else(|error| panic!("{size} {offset}: {error}"));
                counts_exactly(&memory, length);
                let to_the_page_end = (size.next_multiple_of(page) - offset) as usize;
                for past in [length + 1, length + WORD, to_the_page_end + 1, length + page as usize]
                    .into_iter()
                    .chain((to_the_page_end > length).then_some(to_the_page_end))
                {
                    assert!(
                        matches!(HostMapping::file(file.as_fd(), offset, past), Err(FileError::Short)),
                        "{size} {offset} {past}"
                    );
                }
            }
            let after = size.next_multiple_of(word);
            assert!(
                matches!(HostMapping::file(file.as_fd(), after, 1), Err(FileError::Short)),
                "{size}"
            );
        }
    }

    #[test]
    fn sees_what_another_mapping_of_the_file_writes() {
        let file = memory_file(page() as u64, SealFlags::SHRINK);
        let first = HostMapping::file(file.as_fd(), 0, page()).expect("the host maps the file");
        let second = HostMapping::file(file.as_fd(), 0, page()).expect("the host maps the file");
        assert_eq!(first.store(3 * WORD as u64, u64::MAX), Ok(()));
        assert_eq!(second.load::<u64>(3 * WORD as u64), Ok(u64::MAX));
        assert_eq!(second.write(5, &[7]), Ok(()));
        let mut byte = [0];
        assert_eq!(first.read(5, &mut byte), Ok(()));
        assert_eq!(byte, [7]);
    }

    #[test]
    fn maps_the_files_bytes_from_the_offset_asked_up_to_its_last_byte() {
        for (offset, length) in [
            (0, page()),
            (WORD as u64, page() - WORD),
            (page() as u64 - WORD as u64, 2 * WORD),
            (page() as u64 + 3 * WORD as u64, page() - 3 * WORD),
        ] {
            let end = offset + length as u64;
            let file = memory_file(end, SealFlags::SHRINK);
            let every: Vec<u8> = (0..end / WORD as u64)
                .flat_map(|index| (index + 1).to_ne_bytes())
                .collect();
            assert_eq!(rustix::io::pwrite(&file, &every, 0), Ok(every.len()));
            let memory =
                HostMapping::file(file.as_fd(), offset, length).expect("the host maps the file");
            let first = offset / WORD as u64 + 1;
            assert_eq!(memory.load::<u64>(0), Ok(first), "{offset}");
            let last = memory.len() - WORD as u64;
            assert_eq!(memory.load::<u64>(last), Ok(end / WORD as u64), "{offset}");
            let mut read = vec![0; length];
            assert_eq!(memory.read(0, &mut read), Ok(()));
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
                        let file = memory_file(2 * page() as u64, SealFlags::empty());
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
                            HostMapping::file(file.as_fd(), 0, 2 * page())
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
        let file = memory_file(2 * page() as u64, SealFlags::SHRINK);
        let memory =
            HostMapping::file(file.as_fd(), page() as u64, page()).expect("the host maps the file");
        assert_eq!(memory.write(0, &vec![0x5a; page()]), Ok(()));
        let other = reopen(&file, true);
        assert_eq!(ftruncate(&other, page() as u64), Err(Errno::PERM));
        assert_eq!(ftruncate(&other, 0), Err(Errno::PERM));
        let truncating = OpenOptions::new().write(true).truncate(true).open(format!(
            "/proc/self/fd/{}",
            std::os::fd::AsRawFd::as_raw_fd(&file)
        ));
        assert!(truncating.is_err(), "opening with truncation is a shrink");
        let mut held = vec![0; page()];
        assert_eq!(memory.read(0, &mut held), Ok(()));
        assert_eq!(held, vec![0x5a; page()]);

        ftruncate(&other, 64 * page() as u64).expect("a file sealed only against shrinking grows");
        counts_exactly(&memory, page());

        fallocate(
            &other,
            FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE,
            0,
            3 * page() as u64,
        )
        .expect("the file's pages are given back");
        assert_eq!(memory.read(0, &mut held), Ok(()));
        assert_eq!(held, vec![0; page()], "a punched page reads as zeros");
        assert_eq!(memory.store(WORD as u64, 7_u64), Ok(()));
        assert_eq!(memory.load::<u64>(WORD as u64), Ok(7));
    }
}
