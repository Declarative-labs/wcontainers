//! A program that only writes and exits behaves on our kernel as on real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::{guest, run_both};

#[test]
fn hello_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("hello", &["-nostdlib"], &out);
    let (ours, theirs) = run_both(&program, &[]);
    assert_eq!(ours.outcome, theirs);
    assert_eq!(ours.outcome.stdout, b"hello, world\n");
    assert_eq!(ours.outcome.exit_code, 3);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}
