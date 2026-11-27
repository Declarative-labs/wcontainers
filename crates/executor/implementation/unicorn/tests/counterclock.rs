//! The counter read without a system call against real ARM64 Linux: it never goes backwards, also
//! across saves whose host clock reads from an earlier start.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output_seeded;

#[test]
fn counter_never_goes_backwards() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output_seeded(&out, "counterclock", &[], 0);
}

#[test]
fn counter_never_goes_backwards_across_saves() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output_seeded(&out, "counterclock", &[], 3);
}
