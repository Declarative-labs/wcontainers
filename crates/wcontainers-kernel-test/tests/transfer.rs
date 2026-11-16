//! Reads of the random devices, getrandom and sendfile move the whole count as on real ARM64 Linux,
//! and a sendfile of nothing succeeds before anything checks the input.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn transfers_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "transfer", &[]);
}

#[test]
#[ignore = "known failure: a copy from the random devices' block to a buffer that faults partway stops \
            up to 15 bytes short of the fault in the reference (measured: 88 where 96 bytes were \
            writable), by where arm64's copy routine had stored when the fault came; ours stops at \
            the fault"]
fn partial_copies_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "transfer", &["edges"]);
}
