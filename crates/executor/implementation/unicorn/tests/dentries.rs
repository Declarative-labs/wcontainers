//! What a directory entry shows of itself, compared with real ARM64 Linux: listings' `.` and `..`,
//! and the path of a file whose opened name was taken away.

use std::path::Path;

use wcontainers_executor_unicorn::{assert_same_output, assert_same_output_on_backend_root_seeded};

fn out() -> std::path::PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest")
}

#[test]
fn listings_have_their_dots() {
    assert_same_output(&out(), "dots", &[]);
}

#[test]
fn listings_have_their_dots_on_the_backend_root() {
    assert_same_output_on_backend_root_seeded(&out(), "dots", &[], 0);
}

#[test]
fn opened_names_taken_away_match_reference() {
    assert_same_output(&out(), "namesgone", &[]);
}

#[test]
fn opened_names_taken_away_on_the_backend_root_match_reference() {
    assert_same_output_on_backend_root_seeded(&out(), "namesgone", &[], 0);
}
