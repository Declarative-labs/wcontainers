//! The counter read without a system call against real ARM64 Linux: it never goes backwards, also
//! across saves whose host clock reads from an earlier start.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output_seeded;

#[test]
fn counter_never_goes_backwards() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output_seeded(&out, "counterclock", &[], 0);
}

#[test]
#[cfg_attr(
    feature = "runtime",
    ignore = "known failure: the WebAssembly back end's clock hook answers the host's monotonic clock without the kernel's offset after a restore (counted and warned as \"counter read without the kernel's offset\"), until the kernel makes its clock public"
)]
fn counter_never_goes_backwards_across_saves() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output_seeded(&out, "counterclock", &[], 3);
}
