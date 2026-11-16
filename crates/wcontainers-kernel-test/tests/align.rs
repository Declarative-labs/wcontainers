//! Misaligned accesses, compared with real ARM64 Linux. Both are known failures of the stand-in CPU, not
//! of the kernel: they show where unicorn differs from the reference's hardware.

use std::path::Path;

use wcontainers_kernel_test::{guest, run_both};

fn program() -> std::path::PathBuf {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    guest::build("align", &["-nostdlib"], &out)
}

#[test]
#[ignore = "known failure: unicorn faults an exclusive load from an unaligned address, which the reference's hardware runs"]
fn unaligned_exclusive_matches_reference() {
    let (ours, theirs) = run_both(&program(), &["exclusive"]);
    assert_eq!(ours.outcome, theirs);
}

#[test]
#[ignore = "known failure: unicorn (QEMU) never checks stack pointer alignment, so the stand-in never reports the alignment fault the reference's SIGBUS comes from"]
fn misaligned_stack_pointer_matches_reference() {
    let (ours, theirs) = run_both(&program(), &["stack"]);
    assert_eq!(ours.outcome, theirs);
}
