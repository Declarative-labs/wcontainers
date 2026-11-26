//! Every look-ahead gives the same result as none: bounded look-ahead of random N, translating from the
//! first instruction ahead of time and everything else as it is reached, against none in the same engine,
//! comparing output, registers and memory. `WC_LOOKAHEAD_CASES` bounds a program (default 4), seeded from
//! `WC_LOOKAHEAD_SEED` (default 1).

use proptest::prelude::RngExt;
use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::seeded::generator;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect(name))
}

#[test]
fn bounded_matches_none_arm64() {
    arm64::bounded_matches_none();
}

#[test]
#[ignore]
fn measure_translating_at_run_time_arm64() {
    arm64::measure_translating_at_run_time();
}

#[test]
#[ignore]
fn measure_bound_arm64() {
    arm64::measure_bound();
}

#[test]
fn bounded_matches_none_x86_64() {
    x86_64::bounded_matches_none();
}

#[test]
#[ignore]
fn measure_translating_at_run_time_x86_64() {
    x86_64::measure_translating_at_run_time();
}

#[test]
#[ignore]
fn measure_bound_x86_64() {
    x86_64::measure_bound();
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
mod arm64 {
    use super::*;
    use wcontainers_translator_test::arm64 as harness;
    pub(super) use wcontainers_translator_test::arm64::programs::{
        Library, Position, build_with, ran, reachable_memory, ready, translated,
    };

    include!("for_guest/lookaheads.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    use wcontainers_translator_test::x86_64 as harness;
    pub(super) use wcontainers_translator_test::x86_64::programs::{
        Library, Position, build_with, ran, reachable_memory, ready, translated,
    };

    include!("for_guest/lookaheads.rs");
}
