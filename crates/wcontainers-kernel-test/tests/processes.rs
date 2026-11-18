//! Many processes: fork, vfork, exec, exit statuses and waits, orphans, process groups, pipes
//! between processes, and signals with handlers, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn processes_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "processes", &[]);
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}

/// A SIGKILL to the sender's own group reaches every member before the sender ends.
#[test]
fn killing_own_group_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "groupkill", &[]);
}

/// Writers that find a pipe full wait together until its reader comes.
#[test]
fn writers_waiting_on_a_full_pipe_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "pipewriters", &[]);
}

#[test]
fn a_child_started_as_go_starts_one_matches_reference() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_kernel_test::assert_same_output(&out, "spawnpipe", &[]);
}

/// get_robust_list's checks, another process's list among them.
#[test]
fn robust_lists_match_reference() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_kernel_test::assert_same_output(&out, "robustlist", &[]);
}

/// ETXTBSY: a running program is not written, and a file being written is not run.
#[test]
fn busy_programs_match_reference() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_kernel_test::assert_same_output(&out, "textbusy", &[]);
}
