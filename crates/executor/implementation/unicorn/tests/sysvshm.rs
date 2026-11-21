//! System V shared memory compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn shared_memory_segments_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "sysvshm", &[]);
}

/// `/proc/sysvipc` with a segment, a queue and a set of semaphores in use.
#[test]
fn proc_listings_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "sysvipcproc", &[]);
}
