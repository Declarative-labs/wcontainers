//! C programs give the same output and exit status in Chrome, translated at look-ahead none and full, and at
//! bounded look-ahead from the first instruction with every other piece added while the program runs, for
//! speed and for size, as on real Linux of the guest's CPU.

use wcontainers_arm64::Arm64;
use wcontainers_translator::{LookAhead, Omit, Optimise};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::{Library, build, native};
use wcontainers_x86::X86_64;

const PROGRAMS: [(&str, Library); 7] = [
    ("hello", Library::C),
    ("differences", Library::C),
    ("bench", Library::None),
    ("integers", Library::None),
    ("tables", Library::None),
    ("buffers", Library::None),
    ("walks", Library::None),
];
const LEVELS: [&str; 5] = ["-O0", "-O1", "-O2", "-O3", "-Os"];

const EVERY_WAY: [(LookAhead, Optimise); 5] = [
    (LookAhead::None, Optimise::Speed),
    (LookAhead::Full, Optimise::Speed),
    (LookAhead::Full, Optimise::Size),
    (LookAhead::AT_RUN_TIME, Optimise::Speed),
    (LookAhead::AT_RUN_TIME, Optimise::Size),
];

/// Each program, at every optimisation level, translated each way. Levels that build to the same bytes as an
/// earlier one are run once.
fn programs_match_linux_in_chrome<G: GuestTest>(
    programs: &[(&str, Library)],
    ways: &[(LookAhead, Optimise)],
) {
    programs_told_match_linux_in_chrome::<G>(programs, ways, None);
}

/// As `programs_match_linux_in_chrome`, for programs told the offer whose key is `offer`.
fn programs_told_match_linux_in_chrome<G: GuestTest>(
    programs: &[(&str, Library)],
    ways: &[(LookAhead, Optimise)],
    offer: Option<u64>,
) {
    for &(name, library) in programs {
        let mut built: Vec<Vec<u8>> = Vec::new();
        for optimisation in LEVELS {
            let mut blob = build::<G>(name, optimisation, library);
            blob.offer = offer;
            if built.contains(&blob.bytes) {
                continue;
            }
            built.push(blob.bytes.clone());
            let theirs = native::<G>(&blob);
            for &(look_ahead, optimise) in ways {
                let how = (look_ahead, optimise);
                let ours = browser::translated_with::<G>(&blob, how, Omit::default());
                let what =
                    format!("{name} at {optimisation}, look-ahead {look_ahead:?} for {optimise:?}");
                assert_eq!(
                    String::from_utf8_lossy(&ours.ran.stdout),
                    String::from_utf8_lossy(&theirs.stdout),
                    "output of {what}"
                );
                assert_eq!(ours.ran.stdout, theirs.stdout, "output bytes of {what}");
                assert_eq!(
                    String::from_utf8_lossy(&ours.ran.stderr),
                    String::from_utf8_lossy(&theirs.stderr),
                    "error output of {what}"
                );
                assert_eq!(ours.ran.status, theirs.status, "exit status of {what}");
                assert_eq!(ours.count("unsupported instruction reached"), 0);
                if look_ahead == LookAhead::AT_RUN_TIME {
                    assert!(ours.count("translated at run time") > 0, "{what}");
                }
                eprintln!(
                    "{what}: matches in {}, {:?}",
                    ours.user_agent, ours.counters
                );
            }
        }
    }
}

#[test]
fn programs_match_linux_in_chrome_arm64() {
    programs_match_linux_in_chrome::<Arm64>(&PROGRAMS, &EVERY_WAY);
}

/// x86-64 runs at look-ahead none only until full look-ahead is built for it.
#[test]
fn hello_matches_linux_in_chrome_at_none_x86_64() {
    programs_match_linux_in_chrome::<X86_64>(
        &[("hello", Library::C)],
        &[(LookAhead::None, Optimise::Speed)],
    );
}

/// C's long double through the x87 unit, every way.
#[test]
fn long_double_matches_linux_in_chrome_x86_64() {
    programs_match_linux_in_chrome::<X86_64>(&[("long_double", Library::C)], &EVERY_WAY);
}

/// MMX and the x87 unit's state instructions, every way.
#[test]
fn mmx_and_x87_state_match_linux_in_chrome_x86_64() {
    programs_match_linux_in_chrome::<X86_64>(
        &[("mmx", Library::C), ("x87_state", Library::C)],
        &EVERY_WAY,
    );
}

/// `xsave` and `xrstor`, AVX2's integer work with BMI2, and FMA, F16C and AVX's floating point (also in MXCSR's
/// default mode alone), for programs told the x86-64-v3 list, every way.
#[test]
fn x86_64_v3_programs_match_linux_in_chrome_x86_64() {
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).expect("a built list");
    programs_told_match_linux_in_chrome::<X86_64>(
        &[
            ("xstate", Library::C),
            ("avx2_integer", Library::C),
            ("fma_f16c", Library::C),
            ("fma_fast", Library::C),
        ],
        &EVERY_WAY,
        Some(v3.key()),
    );
}
