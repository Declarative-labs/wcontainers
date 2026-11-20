//! RLIMIT_CPU's signals, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel::reason::CORE_DUMP_NOT_WRITTEN;
use wcontainers_kernel_test::assert_same_output;

#[test]
fn cpu_limit_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "cpulimit", &[]);
    // SIGXCPU's default action dumps core where the core size limit allows, which is not written.
    assert!(
        ours.warnings
            .iter()
            .all(|warned| warned.reason == CORE_DUMP_NOT_WRITTEN),
        "{:?}",
        ours.warnings
    );
}

/// The same on the machine's clock, as the Linux Test Project's setrlimit06 runs: the limits are
/// looked at within each second of it however fast the CPU side runs, so SIGXCPU comes first.
#[test]
fn cpu_limit_on_the_wall_clock_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = wcontainers_kernel_test::assert_same_output_on_wall_clock(&out, "cpulimit");
    assert!(
        ours.warnings
            .iter()
            .all(|warned| warned.reason == CORE_DUMP_NOT_WRITTEN),
        "{:?}",
        ours.warnings
    );
}
