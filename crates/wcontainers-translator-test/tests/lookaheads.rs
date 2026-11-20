//! Every look-ahead gives the same result as none: bounded look-ahead of random N, translating from the
//! first instruction ahead of time and everything else as it is reached, against none in the same engine,
//! comparing output, registers and memory. `WC_LOOKAHEAD_CASES` bounds a program (default 4), seeded from
//! `WC_LOOKAHEAD_SEED` (default 1).

use proptest::prelude::RngExt;
use proptest::test_runner::{RngAlgorithm, TestRng};
use wcontainers_arm64::Arm64;
use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::{
    Library, Position, build_with, ran, reachable_memory, ready, translated,
};
use wcontainers_x86::X86_64;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect(name))
}

fn bounded_matches_none<G: GuestTest>() {
    let cases = setting("WC_LOOKAHEAD_CASES", 4);
    let seed = setting("WC_LOOKAHEAD_SEED", 1);
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    let mut rng = TestRng::from_seed(RngAlgorithm::ChaCha, &key);
    for (name, optimisation, defines) in [
        ("bench", "-O0", &["SCALE=0"][..]),
        ("bench", "-O2", &["SCALE=0"][..]),
        ("integers", "-O2", &[][..]),
        ("tables", "-O0", &[][..]),
        ("walks", "-O2", &[][..]),
    ] {
        let blob = build_with::<G>(name, optimisation, Library::None, defines, Position::Fixed);
        let (none, mut none_machine, none_registers) = translated::<G>(&blob, LookAhead::None);
        for case in 0..cases {
            let bound = match case {
                0 => wcontainers_translator::BOUND,
                _ => rng.random_range(1..=64),
            };
            let what =
                format!("{name} {optimisation} at bounded {bound} (seed {seed}, case {case})");
            let look_ahead = LookAhead::Bounded(bound);
            let mut machine = ready::<G>(&blob, look_ahead, Default::default());
            machine.translate_at_run_time(move || look_ahead);
            let (stopped, registers) = machine.run();
            let bounded = ran::<G>(&blob, &machine, stopped, &registers);
            assert_eq!(bounded, none, "{what}");
            assert_eq!(registers, none_registers, "registers of {what}");
            assert!(
                reachable_memory::<G>(&mut machine, &blob)
                    == reachable_memory::<G>(&mut none_machine, &blob),
                "memory of {what}"
            );
            eprintln!(
                "{what}: {} translated at run time",
                machine.count(Reason::TranslatedAtRunTime)
            );
        }
    }
}

/// Not a check: how long translating and loading one small piece takes while a program runs, in wasmtime and
/// in Chrome. The program is translated from its first instruction at bounded look-ahead, so every other
/// piece is made at run time.
fn measure_translating_at_run_time<G: GuestTest>() {
    use wcontainers_translator_test::browser;
    for (name, optimisation, library, defines) in [
        ("bench", "-O2", Library::None, &["SCALE=0"][..]),
        ("hello", "-O2", Library::C, &[][..]),
    ] {
        let blob = build_with::<G>(name, optimisation, library, defines, Position::Fixed);
        for bound in [16, wcontainers_translator::BOUND] {
            let look_ahead = LookAhead::Bounded(bound);
            let mut machine = ready::<G>(&blob, look_ahead, Default::default());
            machine.translate_at_run_time(move || look_ahead);
            let (stopped, registers) = machine.run();
            ran::<G>(&blob, &machine, stopped, &registers);
            let reading = machine
                .counters()
                .into_iter()
                .find(|c| c.reason == Reason::TranslatedAtRunTime.name())
                .expect("translated at run time");
            println!(
                "{name} {optimisation} at bounded {bound} in wasmtime: {} pieces, {:.0} us each",
                reading.count,
                reading.nanoseconds as f64 / reading.count as f64 / 1e3,
            );
            // The Chrome runner makes pieces at run time at the default N only. V8 compiles a function at its
            // first call unless told otherwise, so the piece's compile is in the time only with eager
            // compiling: by its baseline compiler, and by its optimising compiler alone.
            if bound == wcontainers_translator::BOUND {
                for (what, flags) in [
                    ("compiled at first call, not timed", ""),
                    ("baseline compile", "--no-wasm-lazy-compilation"),
                    (
                        "optimising compile",
                        "--no-wasm-lazy-compilation --no-liftoff",
                    ),
                ] {
                    let report =
                        browser::translated_with_engine_flags::<G>(&blob, look_ahead, flags);
                    let chrome = report.count("translated at run time");
                    println!(
                        "{name} {optimisation} at bounded {bound} in Chrome, {what}: {chrome} pieces, {:.0} us each",
                        report.time("translated at run time") as f64 / chrome as f64 / 1e3,
                    );
                }
            }
        }
    }
}

/// Not a check: the default N against others. The benchmark set (`bench.c`, SCALE=1, -O2) translated from
/// its first instruction at bounded look-ahead of each N, every other piece made at run time, against the same
/// at full: the whole run's time in wasmtime and the part spent making pieces, best of `WC_BENCH_RUNS`
/// (default 3). One command:
/// `nix develop -c cargo test --release -p wcontainers-translator-test --test lookaheads measure_bound -- --ignored --nocapture`.
fn measure_bound<G: GuestTest>() {
    use std::time::Instant;
    let runs = setting("WC_BENCH_RUNS", 3);
    let blob = build_with::<G>("bench", "-O2", Library::None, &["SCALE=1"], Position::Fixed);
    let (full, _, _) = translated::<G>(&blob, LookAhead::Full);
    for look_ahead in [16, 64, 256, 1024]
        .map(LookAhead::Bounded)
        .into_iter()
        .chain([LookAhead::Full])
    {
        let mut best = (f64::MAX, 0.0, 0);
        for _ in 0..runs {
            let mut machine = ready::<G>(&blob, look_ahead, Default::default());
            machine.translate_at_run_time(move || look_ahead);
            let start = Instant::now();
            let (stopped, registers) = machine.run();
            let seconds = start.elapsed().as_secs_f64();
            assert_eq!(ran::<G>(&blob, &machine, stopped, &registers), full);
            let making = machine
                .counters()
                .into_iter()
                .find(|c| c.reason == Reason::TranslatedAtRunTime.name())
                .map_or((0.0, 0), |c| (c.nanoseconds as f64 / 1e9, c.count));
            if seconds < best.0 {
                best = (seconds, making.0, making.1);
            }
        }
        println!(
            "bench at {look_ahead:?}: {:.3} s, of which {:.3} s making {} pieces at run time (load {})",
            best.0,
            best.1,
            best.2,
            wcontainers_translator_test::programs::load()
        );
    }
}

#[test]
fn bounded_matches_none_arm64() {
    bounded_matches_none::<Arm64>();
}

#[test]
#[ignore]
fn measure_translating_at_run_time_arm64() {
    measure_translating_at_run_time::<Arm64>();
}

#[test]
#[ignore]
fn measure_bound_arm64() {
    measure_bound::<Arm64>();
}

#[test]
fn bounded_matches_none_x86_64() {
    bounded_matches_none::<X86_64>();
}

#[test]
#[ignore]
fn measure_translating_at_run_time_x86_64() {
    measure_translating_at_run_time::<X86_64>();
}

#[test]
#[ignore]
fn measure_bound_x86_64() {
    measure_bound::<X86_64>();
}
