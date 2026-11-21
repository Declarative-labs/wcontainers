//! Native asynchronous I/O compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

/// Contexts, requests on files and pipes, their events, and the checks on each call.
#[test]
fn asynchronous_io_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "aio", &[]);
}

/// A completed request counts an eventfd up to its overflow.
#[test]
fn eventfd_overflow_matches_reference() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_executor_unicorn::assert_same_output(&out, "eventfdoverflow", &[]);
}

/// Poll requests: ready at once or waiting for their file, their checks, cancels and event
/// descriptors.
#[test]
fn poll_requests_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "aiopoll", &[]);
}
