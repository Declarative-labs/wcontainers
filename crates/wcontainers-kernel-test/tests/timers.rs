//! Interval timers, POSIX timers and timer descriptors, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output_seeded;

#[test]
fn timers_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    for run in assert_same_output_seeded(&out, "timers", &[], 0) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}
