//! Threads through the C library under several seeded schedules, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output_seeded;

fn out() -> std::path::PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest")
}

#[test]
fn threads_match_reference() {
    for run in assert_same_output_seeded(&out(), "threads", &[], 0) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

/// Saved and brought back at moments drawn from each seed, with threads waiting and running.
#[test]
fn threads_survive_saving() {
    for run in assert_same_output_seeded(&out(), "threads", &[], 6) {
        assert!(run.saved > 0);
    }
}

#[test]
fn futexes_match_reference() {
    for run in assert_same_output_seeded(&out(), "futexes", &[], 0) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

/// The priority inheriting futex calls, with waits on them taken through saves.
#[test]
fn priority_inheriting_futexes_match_reference() {
    for run in assert_same_output_seeded(&out(), "futexpi", &[], 3) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

#[test]
fn thread_groups_match_reference() {
    for run in assert_same_output_seeded(&out(), "threadlife", &[], 0) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

/// Each thread's directory in `/proc`, names per thread, and a process whose first thread ended.
#[test]
fn proc_task_matches_reference() {
    for run in assert_same_output_seeded(&out(), "proctask", &[], 0) {
        // `stat` and `status` leave fields out, which every read of them counts.
        let others: Vec<_> = run
            .warnings
            .iter()
            .filter(|warned| warned.reason != "proc fields not tracked")
            .collect();
        assert!(others.is_empty(), "{others:?}");
    }
}

/// clone's sharing flags for threads and processes: descriptor tables, file system contexts and
/// semaphore undo lists of their own or shared.
#[test]
fn clone_tables_match_reference() {
    for run in assert_same_output_seeded(&out(), "clonetables", &[], 0) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

/// clone's remaining combinations: processes on the caller's memory while both run, processes
/// sharing its signal handlers, and threads with process descriptors.
#[test]
fn clone_sharing_matches_reference() {
    // A thread waits for its first thread's end by reading its `/proc` state.
    for run in assert_same_output_seeded(&out(), "clonevm", &[], 0) {
        let others: Vec<_> = run
            .warnings
            .iter()
            .filter(|warned| warned.reason != "proc fields not tracked")
            .collect();
        assert!(others.is_empty(), "{others:?}");
    }
}

/// Credentials and personality held per thread, and a queue's notification request taken away by
/// any thread's close.
#[test]
fn thread_credentials_match_reference() {
    for run in assert_same_output_seeded(&out(), "threadcreds", &[], 0) {
        let others: Vec<_> = run
            .warnings
            .iter()
            .filter(|warned| warned.reason != "proc fields not tracked")
            .collect();
        assert!(others.is_empty(), "{others:?}");
    }
}

/// A signal sent to the process is taken by the thread Linux's `complete_signal` chooses, with the
/// others in `pause`.
#[test]
fn process_signal_takers_match_reference() {
    for run in assert_same_output_seeded(&out(), "sigpick", &[], 0) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

/// A child belongs to the thread that made it: parent death signals at that thread's end, waits
/// with __WNOTHREAD and the order a wait looks at the threads' children.
#[test]
fn parent_threads_match_reference() {
    for run in assert_same_output_seeded(&out(), "threadparent", &[], 0) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

/// How shared and private futexes are keyed: by file, by memory, or refused, on every kind of page.
#[test]
fn futex_keys_match_reference() {
    for run in assert_same_output_seeded(&out(), "futexkeys", &[], 2) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}
