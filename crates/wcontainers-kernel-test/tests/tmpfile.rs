//! Files made with O_TMPFILE on tmpfs, named later or never, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn unnamed_files_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "tmpfile", &[]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}
