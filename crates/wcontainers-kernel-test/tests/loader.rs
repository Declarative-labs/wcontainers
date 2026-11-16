//! Programs are placed in memory as Linux places them.

use std::path::Path;

use wcontainers_kernel_test::{guest, run_both};

/// The rest of each segment's last file page holds what Linux leaves there: the file's following bytes,
/// except in a writable segment with zeroed data after its file part, where they are cleared.
#[test]
fn segment_tails_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("segments", &["-nostdlib"], &out);
    let (ours, theirs) = run_both(&program, &[]);
    assert_eq!(
        String::from_utf8_lossy(&ours.outcome.stdout),
        String::from_utf8_lossy(&theirs.stdout)
    );
    assert_eq!(ours.outcome, theirs);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}
