//! write with bad buffers, huge counts, the standard descriptors and raw register values, compared with
//! real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::{guest, run_both};

#[test]
fn write_edges_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("write", &["-nostdlib"], &out);
    let (ours, theirs) = run_both(&program, &[]);
    assert_eq!(
        String::from_utf8_lossy(&ours.outcome.stdout),
        String::from_utf8_lossy(&theirs.stdout)
    );
    assert_eq!(ours.outcome, theirs);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}

/// Pipe writes whose buffer faults (found by layer 1 seed 3021).
#[test]
fn faulted_pipe_writes_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let run = wcontainers_kernel_test::assert_same_output(&out, "pipefault", &[]);
    assert!(run.warnings.is_empty(), "{:?}", run.warnings);
}
