//! Securebits and the powers they keep or withhold, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn securebits_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "securebits", &[]);
}
