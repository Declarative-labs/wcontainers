//! Reads of the random devices, getrandom and sendfile move the whole count as on real ARM64 Linux,
//! and a sendfile of nothing succeeds before anything checks the input.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn transfers_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "transfer", &[]);
}

#[test]
fn partial_copies_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "transfer", &["edges"]);
}
