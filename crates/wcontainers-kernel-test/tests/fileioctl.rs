//! The requests `ioctl` takes on every kind of file, and tmpfs's inode flags, compared with real
//! ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn file_requests_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "fileioctl", &[]);
}
