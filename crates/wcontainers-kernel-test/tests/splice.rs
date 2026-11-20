//! splice, tee and vmsplice compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn splicing_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let run = assert_same_output(&out, "splice", &[]);
    assert!(
        run.counters
            .iter()
            .any(|counter| counter.reason == "vmsplice pages copied"),
        "vmsplice's copies are counted"
    );
}

/// sendfile with descriptors that have no file behind them (found by layer 1 seed 3015).
#[test]
fn sending_from_anonymous_files_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let run = assert_same_output(&out, "sendanon", &[]);
    assert!(run.warnings.is_empty(), "{:?}", run.warnings);
}
