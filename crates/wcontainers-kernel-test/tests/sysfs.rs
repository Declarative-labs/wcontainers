//! The read-only sysfs at `/sys`, compared with real ARM64 Linux, and the count of lookups of the
//! entries Linux has there.

use std::path::Path;

use wcontainers_kernel::reason;
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
    // devices, fs and kernel; a name Linux lacks there is not counted.
    assert_eq!(counted, [4, 6, 7]);
}
