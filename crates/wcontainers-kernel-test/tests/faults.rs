//! A fault with no handler kills the program as on real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::{guest, run_both};

#[test]
fn faults_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("fault", &["-nostdlib"], &out);
    // 128 plus SIGSEGV, SIGTRAP or SIGILL, as Docker reports a death by signal.
    let expected = [
        ("read", 139),
        ("write", 139),
        ("execute", 139),
        ("breakpoint", 133),
        ("undefined", 132),
        ("none", 0),
    ];
    for (kind, exit_code) in expected {
        let (ours, theirs) = run_both(&program, &[kind]);
        assert_eq!(ours.outcome, theirs, "{kind}");
        assert_eq!(theirs.exit_code, exit_code, "{kind}");
        assert!(
            ours.warnings_besides_start().is_empty(),
            "{kind}: {:?}",
            ours.warnings
        );
    }
}
