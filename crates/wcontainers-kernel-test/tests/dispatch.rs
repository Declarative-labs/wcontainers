//! Every ARM64 Linux system call has an entry; one that is not built answers "not implemented",
//! counts and warns; a number Linux does not have does the same; the number is decoded as Linux does.

use std::path::Path;

use wcontainers_kernel::reason;
use wcontainers_kernel_test::{guest, run_both};

/// Built once for all tests here: a program renamed over while a reference container starts it can be
/// missing in the container's view of the shared directory.
fn program() -> std::path::PathBuf {
    static PROGRAM: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    PROGRAM
        .get_or_init(|| {
            let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
            guest::build("syscall", &["-nostdlib"], &out)
        })
        .clone()
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
    // A call not built that Docker's profile passes to the kernel (it answers ptrace and
    // Landlock's itself).
    let missing = wcontainers_kernel::system_calls()
        .find(|call| !call.implemented && call.name == "fanotify_mark")
        .expect("fanotify_mark is not built yet");
    let program = program();
    let number = missing.number.to_string();
    let run = wcontainers_kernel_test::run_ours(
        program.parent().unwrap(),
        "/wc/syscall",
        &["/wc/syscall", &number],
    );
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

/// Numbers above every call Linux has, including x8 = -1, give "not implemented" in the reference too:
/// Docker's seccomp filter passes numbers above its highest known call through to the kernel.
#[test]
fn numbers_above_linux_calls_match_reference() {
    for (argument, detail) in [("1000", 1000), ("-1", u64::from(u32::MAX))] {
        let (ours, theirs) = run_both(&program(), &[argument]);
        assert_eq!(ours.outcome, theirs, "{argument}");
        assert_eq!(ours.warnings.len(), 1, "{argument}");
        assert_eq!(ours.warnings[0].reason, reason::UNKNOWN_SYSCALL);
        assert_eq!(ours.warnings[0].detail, detail);
    }
}

/// Linux uses only the low 32 bits of x8: 2^32 + 64 is write (here to fd 0 with no bytes).
#[test]
fn number_high_bits_are_ignored_as_in_reference() {
    let (ours, theirs) = run_both(&program(), &[&((1u64 << 32) | 64).to_string()]);
    assert_eq!(ours.outcome, theirs);
    assert_eq!(ours.outcome.stdout, b"0\n");
    assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
}

/// Numbers in the gaps of Linux's list are refused with EPERM by Docker's default seccomp profile.
#[test]
fn numbers_in_gaps_match_reference() {
    for argument in ["244", "400"] {
        let (ours, theirs) = run_both(&program(), &[argument]);
        assert_eq!(ours.outcome, theirs, "{argument}");
    }
}
