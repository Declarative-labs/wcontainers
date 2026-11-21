//! Resource limits and signals a container's first process sends itself, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::{guest, run_both};

#[test]
fn limits_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("limits", &[], &out);
    let mut failed = Vec::new();
    for kind in ["fsize", "kill", "memory", "break", "stack"] {
        let (ours, theirs) = run_both(&program, &[kind]);
        if ours.outcome != theirs {
            failed.push(format!(
                "{kind}: ours {:?}, theirs {:?}",
                ours.outcome, theirs
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
