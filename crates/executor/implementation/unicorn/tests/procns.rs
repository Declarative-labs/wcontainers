//! The namespace links of `/proc/<pid>/ns` and the nsfs files they open, and the mount tables,
//! compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn namespace_links_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "procns", &[]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}

#[test]
fn mount_tables_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "mountinfo", &[]);
}

#[test]
fn mount_tables_match_reference_on_backend_root() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_executor_unicorn::assert_same_output_on_backend_root_seeded(
        &out,
        "mountinfo",
        &[],
        0,
    );
}

/// Docker's masks over `/proc` and `/sys` entries.
#[test]
fn masked_entries_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "masked", &[]);
}
