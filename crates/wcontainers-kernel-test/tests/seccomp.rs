//! Calls Docker's default seccomp profile stops, compared with real ARM64 Linux in a default container.

use std::path::Path;

use wcontainers_kernel_test::{guest, run_both};

#[test]
fn profile_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("seccomp", &[], &out);
    let (ours, theirs) = run_both(&program, &[]);
    assert_eq!(
        String::from_utf8_lossy(&ours.outcome.stdout),
        String::from_utf8_lossy(&theirs.stdout)
    );
    assert_eq!(ours.outcome, theirs);
}
