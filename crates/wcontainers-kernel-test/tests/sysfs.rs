//! The read-only sysfs at `/sys`, compared with real ARM64 Linux, the count of lookups of the
//! entries Linux has there, and what the machine says of its huge pages.

use std::path::Path;

use wcontainers_kernel_linux::reason;
use wcontainers_kernel_test::assert_same_output;

#[test]
fn sys_root_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "sysfs", &[]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}

#[test]
fn sys_entries_not_built_are_counted() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "sysfs", &["lookups"]);
    let mut counted: Vec<u64> = ours
        .counters
        .iter()
        .filter(|counter| counter.reason == reason::SYS_ENTRY_NOT_BUILT)
        .map(|counter| counter.detail)
        .collect();
    counted.sort();
    // devices, and names below the root (one in the cgroup); a name Linux lacks at the root is not
    // counted.
    assert_eq!(counted, [4, 9]);
}

#[test]
fn cgroup_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "sysfs", &["cgroup"]);
    // The mount table leaves out Docker's masks, and the name mkdir looks up is not there.
    let expected = [
        (reason::PROC_FIELDS_NOT_TRACKED, 3),
        (reason::SYS_ENTRY_NOT_BUILT, 9),
    ];
    for warning in &ours.warnings {
        assert!(
            expected.contains(&(warning.reason.as_str(), warning.detail)),
            "{warning:?}"
        );
    }
}

#[test]
fn huge_pages_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "sysfs", &["hugepages"]);
    // The 64 KiB size's missing demote_size is counted, as every miss below the root is, and
    // /proc/meminfo holds a few of Linux's fields.
    let expected = [
        (reason::SYS_ENTRY_NOT_BUILT, 9),
        (reason::PROC_FIELDS_NOT_TRACKED, 4),
    ];
    for warning in &ours.warnings {
        assert!(
            expected.contains(&(warning.reason.as_str(), warning.detail)),
            "{warning:?}"
        );
    }
}

#[test]
fn transparent_huge_pages_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "sysfs", &["transparent"]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}
