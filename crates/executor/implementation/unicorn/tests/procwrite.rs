//! Writes to `/proc` files, seeks on them and when their bytes are made, compared with real ARM64
//! Linux.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn proc_writes_seeks_and_reads_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "procwrite", &[]);
}

#[test]
fn coredump_filter_matches_reference() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_executor_unicorn::assert_same_output(&out, "coredumpfilter", &[]);
}

#[test]
fn oom_adjustments_match_reference() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_executor_unicorn::assert_same_output(&out, "oomadj", &[]);
}

#[test]
fn timer_slack_matches_reference() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_executor_unicorn::assert_same_output(&out, "timerslack", &[]);
}

/// prctl(PR_SET_MM): its checks, and a whole map put back as cmdline, environ, auxv and brk show.
#[test]
fn memory_map_setting_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "setmm", &[]);
}
