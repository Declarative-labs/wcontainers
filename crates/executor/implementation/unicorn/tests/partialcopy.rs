//! What calls store in a buffer that runs into a page they cannot write, compared with real ARM64
//! Linux: arm64's copy routine, single stores, and copies of whole pages.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn partial_copies_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "partialcopy", &[]);
}
