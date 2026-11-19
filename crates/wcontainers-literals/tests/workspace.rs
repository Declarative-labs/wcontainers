//! The check on the workspace this crate lives in: nothing outside literals.baseline, no line in it
//! that is no longer needed, nothing hidden, and a baseline that has only shrunk (docs/literals.md).

use std::io::Write;

#[test]
fn the_workspace_keeps_to_its_baseline() {
    let root = wcontainers_literals::workspace_root(env!("CARGO_MANIFEST_DIR").as_ref())
        .expect("find the workspace");
    let out = wcontainers_literals::check(&root).unwrap_or_else(|e| panic!("{e}"));
    // Written past the test harness's capture, so that every test run shows every mark and every
    // change that could add lines to the baseline, also when the check passes.
    let mut shown = std::io::stderr().lock();
    for line in out.marks.iter().chain(&out.summary) {
        writeln!(shown, "{line}").expect("write to the terminal");
    }
    assert!(out.clean, "{}", out.report());
}
