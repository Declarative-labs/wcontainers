//! Page protection, fresh zero memory, private file mappings, the break and the growing stack, and the
//! faults they raise, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::{guest, run_both};

#[test]
fn memory_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("memory", &[], &out);
    let cases = [
        "protect", "none", "unmap", "zero", "beyond", "private", "grow", "overflow", "brk",
        "execute", "wrap", "tagged",
    ];
    let mut failed = Vec::new();
    for kind in cases {
        let (ours, theirs) = run_both(&program, &[kind]);
        if ours.outcome != theirs {
            failed.push(format!(
                "{kind}: ours {:?}, theirs {:?}",
                ours.outcome, theirs
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
