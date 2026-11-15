//! Every ARM64 Linux system call has an entry; one that is not built answers "not implemented",
//! counts and warns; a number Linux does not have does the same and matches the reference.

use std::path::Path;

use wcontainers_kernel::reason;
use wcontainers_kernel_test::{guest, run_both};

fn program() -> std::path::PathBuf {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    guest::build("syscall", &["-nostdlib"], &out)
}

#[test]
fn every_linux_call_has_an_entry() {
    let calls: Vec<_> = wcontainers_kernel::system_calls().collect();
    for name in [
        "read",
        "write",
        "openat",
        "clone3",
        "io_uring_setup",
        "exit_group",
    ] {
        assert!(calls.iter().any(|call| call.name == name), "{name}");
    }
    let numbers: std::collections::BTreeSet<_> = calls.iter().map(|call| call.number).collect();
    assert_eq!(numbers.len(), calls.len());
    assert_eq!(
        wcontainers_kernel::system_call_name(linux_raw_sys::general::__NR_write.into()),
        Some("write")
    );
}

#[test]
fn not_implemented_counts_and_warns() {
    let missing = wcontainers_kernel::system_calls()
        .find(|call| !call.implemented)
        .expect("some call is not built yet");
    let program = program();
    let bytes = std::fs::read(&program).unwrap();
    let number = missing.number.to_string();
    let run = wcontainers_kernel_test::driver::run(
        &bytes,
        &["/wc/syscall", &number],
        wcontainers_kernel_test::reference::ENVIRONMENT,
        0,
    )
    .unwrap();
    let enosys = -(linux_raw_sys::errno::ENOSYS as i64);
    assert_eq!(run.outcome.stdout, format!("{enosys}\n").into_bytes());
    assert_eq!(run.outcome.exit_code, 0);
    assert_eq!(run.warnings.len(), 1);
    assert_eq!(run.warnings[0].reason, reason::NOT_IMPLEMENTED);
    assert_eq!(run.warnings[0].detail, missing.number);
    let counter = run
        .counters
        .iter()
        .find(|counter| counter.reason == reason::NOT_IMPLEMENTED)
        .expect("counted");
    assert_eq!((counter.detail, counter.count), (missing.number, 1));
}

#[test]
fn unknown_number_matches_reference() {
    let (ours, theirs) = run_both(&program(), &["1000"]);
    assert_eq!(ours.outcome, theirs);
    assert_eq!(ours.warnings.len(), 1);
    assert_eq!(ours.warnings[0].reason, reason::UNKNOWN_SYSCALL);
    assert_eq!(ours.warnings[0].detail, 1000);
}
