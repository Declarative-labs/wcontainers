//! A script's first line is split as real ARM64 Linux splits it when the script cannot start.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn script_lines_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "scriptline", &[]);
}
