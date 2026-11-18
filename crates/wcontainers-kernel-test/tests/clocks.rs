//! Every kind of clock id, the CPU clocks below zero included, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn clocks_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "clocks", &[]);
}

/// adjtimex and clock_adjtime: a clock never synchronised, which no container process may set.
#[test]
fn clock_adjustments_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "adjtime", &[]);
}
