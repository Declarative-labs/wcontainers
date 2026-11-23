//! Where mappings are placed, compared with the reference where it is the same: relative to each
//! other, to the stack and the vDSO, in holes, at hints, and on a PMD's boundary for anonymous
//! memory of whole PMDs. The addresses themselves differ, as ours are placed low first.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output_with;
use wcontainers_executor_unicorn::reference::docker;
use wcontainers_kernel_linux::{Seccomp, StartOptions};

/// The guest turns layout randomization off, which Docker's seccomp profile refuses.
#[test]
fn relative_placement_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let options = StartOptions {
        seccomp: Seccomp::Unconfined,
        ..Default::default()
    };
    assert_same_output_with(
        &out,
        "placement",
        options,
        &[docker::SECURITY_OPTION, docker::SECCOMP_UNCONFINED],
        &[],
    );
}
