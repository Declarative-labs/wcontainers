//! Writes to `/proc` files, seeks on them and when their bytes are made, compared with real ARM64
//! Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn proc_writes_seeks_and_reads_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "procwrite", &[]);
}
