//! `/proc/self/status`'s descriptor table size and groups, the maps lines of shared anonymous
//! memory and `/proc/<pid>/fdinfo` for every kind of descriptor, compared with real ARM64 Linux;
//! and the counted approximations of `/proc`.

use std::path::Path;

use wcontainers_kernel::reason;
use wcontainers_kernel_test::assert_same_output;

#[test]
fn status_and_maps_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "procstatus", &[]);
    assert!(
        ours.counters
            .iter()
            .any(|counter| counter.reason == reason::PROC_FIELDS_NOT_TRACKED && counter.detail == 2)
    );
}

#[test]
fn fdinfo_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "fdinfo", &[]);
}

#[test]
fn private_mapping_sees_later_writes() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "procstatus", &["private"]);
}

#[test]
fn entries_not_built_are_counted() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_kernel_test::guest::build("procstatus", &[], &out);
    let (ours, theirs) = wcontainers_kernel_test::run_both(&program, &["unbuilt"]);
    // The reference has the first two; a name Linux has not is not counted.
    assert_eq!(
        String::from_utf8_lossy(&theirs.stdout),
        "/proc/self/mountstats: 1\n/proc/sys/kernel/random/urandom_min_reseed_secs: 1\n/proc/nonexistent: 0\n"
    );
    let not_built: u64 = ours
        .counters
        .iter()
        .filter(|counter| counter.reason == reason::PROC_ENTRY_NOT_BUILT)
        .map(|counter| counter.count)
        .sum();
    assert_eq!(not_built, 2);
}
