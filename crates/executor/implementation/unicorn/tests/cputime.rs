//! CPU time left out of sleeps, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::{guest, run_both};

#[test]
fn cpu_time_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("cputime", &[], &out);
    let (ours, theirs) = run_both(&program, &[]);
    let ours_text = String::from_utf8_lossy(&ours.outcome.stdout).into_owned();
    let theirs_text = String::from_utf8_lossy(&theirs.stdout).into_owned();
    let differences: Vec<String> = ours_text
        .lines()
        .zip(theirs_text.lines())
        .filter(|(our, their)| our != their)
        .map(|(our, their)| format!("ours {our:?} theirs {their:?}"))
        .collect();
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    assert_eq!(ours.outcome, theirs);
}
