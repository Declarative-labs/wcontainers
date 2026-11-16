//! Private file mappings compared with real ARM64 Linux: copy on write against later changes to the
//! file.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn private_file_mappings_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "privatemap", &[]);
}
