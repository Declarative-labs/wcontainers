//! One shared file shown at two places of one process, written through one view and read or run
//! through the other with no system call between, compared with real ARM64 Linux.

use std::path::{Path, PathBuf};

use wcontainers_kernel_test::assert_same_output;

fn out() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest")
}

#[test]
fn data_written_through_one_view_is_read_through_the_other() {
    assert_same_output(&out(), "sharedviews", &["data"]);
}

#[test]
fn a_store_across_a_rings_seam_matches_reference() {
    assert_same_output(&out(), "sharedviews", &["ring"]);
}

#[test]
#[cfg_attr(
    feature = "runtime",
    ignore = "known failure: the WebAssembly back end refuses a block's page shown next to itself (counted and warned as \"a block's page shown at neighbouring pages refused\"): one access across the two would need both copies of the page at once"
)]
fn a_ring_of_one_page_matches_reference() {
    assert_same_output(&out(), "sharedviews", &["ring1"]);
}

#[test]
#[cfg_attr(
    not(feature = "runtime"),
    ignore = "known stand-in failure: Unicorn runs stale code after it is written through another view or by another process"
)]
fn code_written_through_one_view_runs_through_the_other() {
    assert_same_output(&out(), "sharedviews", &["code"]);
}
