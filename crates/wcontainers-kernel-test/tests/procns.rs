//! The namespace links of `/proc/<pid>/ns` and the nsfs files they open, compared with real ARM64
//! Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn namespace_links_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "procns", &[]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}
