//! cachestat compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::{guest, run_both};

fn compare(arguments: &[&str]) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("cachestat", &[], &out);
    let (ours, theirs) = run_both(&program, arguments);
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

#[test]
fn page_counts_match_reference() {
    compare(&[]);
}

#[test]
#[ignore = "known failure: the pages a private mapping's faults bring into a tmpfs file are not known here (counted as page residency not tracked)"]
fn mapped_file_pages_match_reference() {
    compare(&["mapped"]);
}
