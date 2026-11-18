//! Programs on a container whose root is on the file backend, as the runtimes run, compared with a
//! default container whose root is overlayfs: the container's own files must behave as the
//! reference's do, not as Linux's FUSE files.

use std::path::{Path, PathBuf};

use wcontainers_kernel_test::reference::IMAGE;
use wcontainers_kernel_test::{guest, runtimes};

/// A directory holding the program, built once per test binary.
fn directory() -> PathBuf {
    static DIRECTORY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIRECTORY
        .get_or_init(|| {
            let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rootbackend");
            guest::build("rootfiles", &[], &out);
            guest::build("coredump", &[], &out);
            out
        })
        .clone()
}

/// epoll on the root's regular files, a pipe write running on into a mapped program's pages
/// not read yet, and dnotify, with a save in each run.
#[test]
fn root_files_match_reference() {
    let saves = runtimes::Saves { count: 1, gap: 40 };
    let runs = runtimes::assert_same(IMAGE, &directory(), &["/wc/rootfiles"], saves);
    for run in &runs {
        assert!(
            !run.warnings
                .iter()
                .any(|warning| warning.reason == wcontainers_kernel::reason::COPY_SHORT_OF_PAGES),
            "a copy came short: {:?}",
            run.warnings
        );
    }
}

/// Core dumps written into the root's /tmp, which waits for the backend: the wait status and the
/// core file's headers, notes and memory, under several limits, with threads and a fault.
#[test]
fn core_dumps_match_reference() {
    let saves = runtimes::Saves { count: 1, gap: 40 };
    runtimes::assert_same(IMAGE, &directory(), &["/wc/coredump"], saves);
}
