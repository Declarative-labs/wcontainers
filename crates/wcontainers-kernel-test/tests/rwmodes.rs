//! Reads and writes that may not wait, and fallocate's modes, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn modes_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "rwmodes", &[]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}
