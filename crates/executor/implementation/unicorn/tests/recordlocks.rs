//! fcntl's record locks in one process, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn record_locks_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "recordlocks", &[]);
}

/// File leases: the checks on taking one, and openers waiting while the holder is told.
#[test]
fn leases_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "leases", &[]);
}
