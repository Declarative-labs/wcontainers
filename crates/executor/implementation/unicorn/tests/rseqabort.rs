//! Restartable sequences aborted by handlers, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn aborted_sections_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "rseqabort", &[]);
}

/// A thread taken off the one CPU inside its critical section lands at the abort address.
#[test]
fn preempted_sections_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_executor_unicorn::guest::build("rseqpreempt", &[], &out);
    let theirs = wcontainers_executor_unicorn::reference::run_one_cpu(&program, &[]);
    let path = "/wc/rseqpreempt";
    for seed in wcontainers_executor_unicorn::schedule_seeds() {
        let ours = wcontainers_executor_unicorn::run_ours_saving(&out, path, &[path], seed, 0);
        assert_eq!(ours.outcome, theirs, "seed {seed}");
    }
}
