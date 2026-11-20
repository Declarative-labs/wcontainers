//! Timed waits whose deadline comes while other processes compute, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn deadlines_end_waits_while_others_compute() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "deadlines", &[]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}
