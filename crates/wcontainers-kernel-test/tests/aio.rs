//! Native asynchronous I/O compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

/// Contexts, requests on files and pipes, their events, and the checks on each call.
#[test]
fn asynchronous_io_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "aio", &[]);
}
