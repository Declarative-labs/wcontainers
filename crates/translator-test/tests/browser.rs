//! C programs give the same output and exit status in Chrome, translated at look-ahead none and full, and at
//! bounded look-ahead from the first instruction with every other piece added while the program runs, for
//! speed and for size, as on real Linux of the guest's CPU.

use wcontainers_translator::{LookAhead, Omit, Optimise};

const PROGRAMS: [(&str, arm64::Library); 8] = [
    ("hello", arm64::Library::C),
    ("differences", arm64::Library::C),
    ("bench", arm64::Library::None),
    ("integers", arm64::Library::None),
    ("tables", arm64::Library::None),
    ("buffers", arm64::Library::None),
    ("mappings", arm64::Library::None),
    ("walks", arm64::Library::None),
];
const LEVELS: [&str; 5] = ["-O0", "-O1", "-O2", "-O3", "-Os"];

const EVERY_WAY: [(LookAhead, Optimise); 5] = [
    (LookAhead::None, Optimise::Speed),
    (LookAhead::Full, Optimise::Speed),
    (LookAhead::Full, Optimise::Size),
    (LookAhead::AT_RUN_TIME, Optimise::Speed),
    (LookAhead::AT_RUN_TIME, Optimise::Size),
];

#[test]
fn programs_match_linux_in_chrome_arm64() {
    arm64::programs_match_linux_in_chrome(&PROGRAMS, &EVERY_WAY);
}

/// x86-64 runs at look-ahead none only until full look-ahead is built for it.
#[test]
fn hello_matches_linux_in_chrome_at_none_x86_64() {
    x86_64::programs_match_linux_in_chrome(
        &[("hello", x86_64::Library::C)],
        &[(LookAhead::None, Optimise::Speed)],
    );
}

/// C's long double through the x87 unit, every way.
#[test]
fn long_double_matches_linux_in_chrome_x86_64() {
    x86_64::programs_match_linux_in_chrome(&[("long_double", x86_64::Library::C)], &EVERY_WAY);
}

/// MMX and the x87 unit's state instructions, every way.
#[test]
fn mmx_and_x87_state_match_linux_in_chrome_x86_64() {
    x86_64::programs_match_linux_in_chrome(
        &[
            ("mmx", x86_64::Library::C),
            ("x87_state", x86_64::Library::C),
        ],
        &EVERY_WAY,
    );
}

/// `xsave` and `xrstor`, AVX2's integer work with BMI2, and FMA, F16C and AVX's floating point (also in MXCSR's
/// default mode alone), for programs told the x86-64-v3 list, every way.
#[test]
fn x86_64_v3_programs_match_linux_in_chrome_x86_64() {
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).expect("a built list");
    x86_64::programs_told_match_linux_in_chrome(
        &[
            ("xstate", x86_64::Library::C),
            ("avx2_integer", x86_64::Library::C),
            ("fma_f16c", x86_64::Library::C),
            ("fma_fast", x86_64::Library::C),
        ],
        &EVERY_WAY,
        Some(v3.key()),
    );
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
mod arm64 {
    use super::*;
    pub(super) use wcontainers_translator_test::arm64::browser;
    pub(super) use wcontainers_translator_test::arm64::programs::{Library, build, native};

    include!("for_guest/browser.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    pub(super) use wcontainers_translator_test::x86_64::browser;
    pub(super) use wcontainers_translator_test::x86_64::programs::{Library, build, native};

    include!("for_guest/browser.rs");
}
