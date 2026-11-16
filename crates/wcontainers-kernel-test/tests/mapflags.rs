//! mmap's flags, mprotect's PROT_GROWSDOWN, grows-down and droppable memory, madvise's advice,
//! statx's mount numbers and file system rules, the stat calls' null paths and a sysctl's vectored
//! reads, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel::reason;
use wcontainers_kernel_test::assert_same_output;

#[test]
fn map_flags_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "mapflags", &[]);
}

#[test]
fn advice_not_built_is_counted() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_kernel_test::guest::build("mapflags", &[], &out);
    let (ours, _) = wcontainers_kernel_test::run_both(&program, &["unbuilt"]);
    let mut counted: Vec<u64> = ours
        .counters
        .iter()
        .filter(|counter| counter.reason == reason::ADVICE_NOT_BUILT)
        .map(|counter| counter.detail)
        .collect();
    counted.sort();
    assert_eq!(counted, [25, 102, 103]);
}
