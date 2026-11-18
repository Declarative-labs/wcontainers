//! fcntl commands and prctl options Linux does not know answer as Linux does without being counted as
//! not built; ones Linux has that are not built are counted.

use std::path::Path;

use wcontainers_kernel::reason;
use wcontainers_kernel_test::{assert_same_output, guest, run_ours};

#[test]
fn unknown_ones_match_reference_uncounted() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "unknown", &[]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}

#[test]
fn ones_not_built_are_counted() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    guest::build("unknown", &[], &out);
    let ours = run_ours(&out, "/wc/unknown", &["/wc/unknown", "unbuilt"]);
    let counted = |name: &str, detail: u64| {
        ours.counters
            .iter()
            .any(|counter| counter.reason == name && counter.detail == detail)
    };
    assert!(counted(reason::FCNTL_NOT_BUILT, 1039));
    assert!(counted(reason::PRCTL_NOT_BUILT, 13));
}
