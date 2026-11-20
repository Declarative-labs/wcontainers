//! RLIMIT_CPU's signals, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn cpu_limit_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "cpulimit", &[]);
    // SIGXCPU's default action dumps core where the core size limit allows, which is not written.
    assert!(
        ours.warnings
            .iter()
            .all(|warned| warned.reason == "core dump not written"),
        "{:?}",
        ours.warnings
    );
}
