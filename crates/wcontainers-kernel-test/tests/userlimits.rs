//! RLIMIT_SIGPENDING and RLIMIT_NPROC, which count a user's queued signals and tasks, compared with
//! real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn queued_signals_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "userlimits", &["signals"]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}

#[test]
fn tasks_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "userlimits", &["tasks"]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}

#[test]
fn pipe_pages_per_user_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "pipeusers", &[]);
}
