//! Programs started with as many page-long arguments as an unlimited stack takes, compared with real
//! ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn most_arguments_an_unlimited_stack_takes_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "bigargs", &[]);
}
