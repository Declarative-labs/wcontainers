//! Who runs while another waits, compared with real ARM64 Linux on one CPU like the made-up
//! machine: sched_yield, a thread that keeps coming back by system calls after its share, and the
//! scheduling calls, turns and weights between threads.

use std::path::Path;

use wcontainers_kernel::reason;
use wcontainers_kernel_test::{assert_same_output_one_cpu_seeded, guest, reference, run_ours};

#[test]
fn turns_match_one_cpu_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("turns", &[], &out);
    let ours = run_ours(&out, "/wc/turns", &["/wc/turns"]);
    let theirs = reference::run_one_cpu(&program, &[]);
    assert_eq!(
        String::from_utf8_lossy(&ours.outcome.stdout),
        String::from_utf8_lossy(&theirs.stdout)
    );
    assert_eq!(ours.outcome, theirs);
    // Only `stat`'s counted approximation.
    assert!(
        ours.warnings
            .iter()
            .all(|warning| warning.reason == reason::PROC_FIELDS_NOT_TRACKED),
        "{:?}",
        ours.warnings
    );
}

#[test]
fn thread_scheduling_matches_one_cpu_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    for run in assert_same_output_one_cpu_seeded(&out, "threadsched", &[], 0) {
        assert!(
            run.warnings
                .iter()
                .all(|warning| warning.reason == reason::PROC_FIELDS_NOT_TRACKED),
            "{:?}",
            run.warnings
        );
    }
}
