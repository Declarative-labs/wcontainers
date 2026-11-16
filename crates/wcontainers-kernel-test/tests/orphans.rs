//! Orphaned process groups and orphans' exit signals, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn orphans_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "orphans", &[]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}
