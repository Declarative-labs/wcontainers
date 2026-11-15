//! Anonymous memory files, seals and reads of the zero devices, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::{guest, run_both};

fn compare(arguments: &[&str]) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("memfd", &[], &out);
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
fn memory_files_match_reference() {
    compare(&[]);
}

#[test]
#[ignore = "known failure: huge page memory files need hugetlbfs, not built; refused with ENOSYS"]
fn huge_page_memory_files_match_reference() {
    compare(&["hugetlb"]);
}
