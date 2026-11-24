#![cfg(unix)]

use wcontainers_shared_memory::Words;
use wcontainers_shared_memory_host::HostMapping;

/// The bytes of each mapping: one page of the smallest size hosts use.
const LENGTH: usize = 4096;

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
}

#[cfg(target_os = "linux")]
mod file {
    use std::os::fd::{AsFd, OwnedFd};

    use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
    use wcontainers_shared_memory_host::FileError;

    use super::*;

    /// Returns a memory file of the length, sealed against shrinking when asked.
    fn memory_file(length: u64, sealed: bool) -> OwnedFd {
        let file = memfd_create("words", MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
            .expect("a memory file");
        ftruncate(&file, length).expect("the file grows");
        if sealed {
            fcntl_add_seals(&file, SealFlags::SHRINK).expect("the file takes the seal");
        }
        file
    }

    /// Returns words over a sealed memory file of the test's length; the mapping outlives the descriptor.
    fn words() -> Words {
        let file = memory_file(LENGTH as u64, true);
        Words::new(HostMapping::file(file.as_fd(), 0, LENGTH).expect("the host maps the file"))
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
    fn refuses_a_file_that_can_shrink() {
        let file = memory_file(LENGTH as u64, false);
        assert!(matches!(
            HostMapping::file(file.as_fd(), 0, LENGTH),
            Err(FileError::Unsealed)
        ));
    }

    #[test]
    fn refuses_offsets_past_the_end_of_the_file() {
        let file = memory_file(LENGTH as u64, true);
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
        let file = memory_file(LENGTH as u64, true);
        let first =
            Words::new(HostMapping::file(file.as_fd(), 0, LENGTH).expect("the host maps the file"));
        let second =
            Words::new(HostMapping::file(file.as_fd(), 0, LENGTH).expect("the host maps the file"));
        assert_eq!(first.store(3, u64::MAX), Ok(()));
        assert_eq!(second.load(3), Ok(u64::MAX));
    }
}
