//! How execve fails on ELF programs and the loaders they name, the room for arguments and a start
//! with no arguments, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn load_errors_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "loaderrors", &[]);
}

#[test]
fn failure_past_the_point_of_no_return_kills() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let run = assert_same_output(&out, "loaderrors", &["fatal"]);
    assert_eq!(run.outcome.exit_code, 139);
}

#[test]
fn start_with_no_arguments_gets_an_empty_one() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "loaderrors", &["empty"]);
}
