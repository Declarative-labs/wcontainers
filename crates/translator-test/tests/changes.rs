//! Page permissions changed at random stops and system calls: full look-ahead ends exactly as look-ahead none
//! given the same changes at the same points (same fault, address and kind, same registers, same memory).
//! Changes often hit the page a register points at and the pages around it, so they land on what a loop's
//! proof or a kept run covers and on the page after the one being accessed.
//!
//! Knobs: `WC_CHANGES_CASES` (random programs, default 64), `WC_CHANGES_SEED` (default 1),
//! `WC_CHANGES_PROGRAM_SEEDS` (runs of each C program, default 3).

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use wcontainers_abi::{Exit, PAGE_SIZE, Permissions};
use wcontainers_translator::{LookAhead, Omit};
use wcontainers_translator_test::seeded::generator;

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

#[test]
fn random_programs_with_changes_match_none_arm64() {
    arm64::random_programs_with_changes_match_none();
}

#[test]
fn programs_with_changes_match_none_arm64() {
    arm64::programs_with_changes_match_none();
}

#[test]
fn hand_made_changes_match_none_arm64() {
    arm64::hand_made_changes_match_none();
}

#[test]
fn random_programs_with_changes_match_none_x86_64() {
    x86_64::random_programs_with_changes_match_none();
}

#[test]
fn programs_with_changes_match_none_x86_64() {
    x86_64::programs_with_changes_match_none();
}

#[test]
fn hand_made_changes_match_none_x86_64() {
    x86_64::hand_made_changes_match_none();
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
mod arm64 {
    use super::*;
    pub(super) use wcontainers_translator_test::arm64::changes::{Compared, Plan, compare};
    pub(super) use wcontainers_translator_test::arm64::guest::Change;
    pub(super) use wcontainers_translator_test::arm64::layer2::program;
    use wcontainers_translator_test::arm64::part;
    pub(super) use wcontainers_translator_test::arm64::programs::{Library, build, ready};
    pub(super) use wcontainers_translator_test::arm64::runner::{Pause, Stopped, fields};

    include!("for_guest/changes.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    pub(super) use wcontainers_translator_test::x86_64::changes::{Compared, Plan, compare};
    pub(super) use wcontainers_translator_test::x86_64::guest::Change;
    pub(super) use wcontainers_translator_test::x86_64::layer2::program;
    use wcontainers_translator_test::x86_64::part;
    pub(super) use wcontainers_translator_test::x86_64::programs::{Library, build, ready};
    pub(super) use wcontainers_translator_test::x86_64::runner::{Pause, Stopped, fields};

    include!("for_guest/changes.rs");
}
