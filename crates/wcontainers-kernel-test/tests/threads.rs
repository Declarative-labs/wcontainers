//! Threads through the C library under several seeded schedules, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output_seeded;

fn out() -> std::path::PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest")
}

#[test]
fn threads_match_reference() {
    for run in assert_same_output_seeded(&out(), "threads", &[], 0) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

/// Saved and brought back at moments drawn from each seed, with threads waiting and running.
#[test]
fn threads_survive_saving() {
    for run in assert_same_output_seeded(&out(), "threads", &[], 6) {
        assert!(run.saved > 0);
    }
}
