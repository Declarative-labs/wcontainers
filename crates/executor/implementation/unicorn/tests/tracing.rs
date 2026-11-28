//! The tracing call as the LTP tests use it, compared with real ARM64 Linux.
//!
//! TRACEME, ATTACH, CONT, KILL, DETACH, PEEK, POKE, GETSIGINFO and SETSIGINFO, with the stops,
//! waits and SIGCHLD they bring.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;
use wcontainers_kernel_linux::reason;

#[test]
fn tracing_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "tracing", &[]);
    // Only the counted approximations of `status`, which it reads.
    assert!(
        ours.warnings
            .iter()
            .all(|warning| warning.reason == reason::PROC_FIELDS_NOT_TRACKED),
        "{:?}",
        ours.warnings
    );
}
