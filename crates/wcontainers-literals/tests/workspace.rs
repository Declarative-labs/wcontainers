//! The check on the workspace this crate lives in: nothing outside literals.baseline, no line in it
//! that is no longer needed, nothing hidden, and a baseline that has only shrunk (docs/literals.md).

#[test]
fn the_workspace_keeps_to_its_baseline() {
    let root = wcontainers_literals::workspace_root(env!("CARGO_MANIFEST_DIR").as_ref())
        .expect("find the workspace");
    let out = wcontainers_literals::check(&root).unwrap_or_else(|e| panic!("{e}"));
    assert!(out.clean, "{}", out.report());
}
