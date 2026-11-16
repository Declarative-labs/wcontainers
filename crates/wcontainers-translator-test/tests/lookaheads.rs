//! Every look-ahead gives the same result as none: bounded look-ahead of random N, translating from the
//! first instruction ahead of time and everything else as it is reached, against none in the same engine,
//! comparing output, registers and memory. `WC_LOOKAHEAD_CASES` bounds a program (default 4), seeded from
//! `WC_LOOKAHEAD_SEED` (default 1).

use proptest::prelude::RngExt;
use proptest::test_runner::{RngAlgorithm, TestRng};
use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::programs::{
    Library, build_with, ran, reachable_memory, ready, translated,
};

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect(name))
}

#[test]
fn bounded_matches_none() {
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
        let blob = build_with(name, optimisation, Library::None, defines);
        let (none, mut none_machine, none_registers) = translated(&blob, LookAhead::None);
        for case in 0..cases {
            let bound = match case {
                0 => wcontainers_translator::BOUND,
                _ => rng.random_range(1..=64),
            };
            let what =
                format!("{name} {optimisation} at bounded {bound} (seed {seed}, case {case})");
            let look_ahead = LookAhead::Bounded(bound);
            let mut machine = ready(&blob, look_ahead, Default::default());
            machine.translate_at_run_time(move || look_ahead);
            let (stopped, registers) = machine.run();
            let bounded = ran(&blob, &machine, stopped, &registers);
            assert_eq!(bounded, none, "{what}");
            assert_eq!(registers, none_registers, "registers of {what}");
            assert!(
                reachable_memory(&mut machine, &blob) == reachable_memory(&mut none_machine, &blob),
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
#[test]
#[ignore]
fn measure_translating_at_run_time() {
    use wcontainers_translator_test::browser;
    for (name, optimisation, library, defines) in [
        ("bench", "-O2", Library::None, &["SCALE=0"][..]),
        ("hello", "-O2", Library::C, &[][..]),
    ] {
        let blob = build_with(name, optimisation, library, defines);
        for bound in [16, wcontainers_translator::BOUND] {
            let look_ahead = LookAhead::Bounded(bound);
            let mut machine = ready(&blob, look_ahead, Default::default());
            machine.translate_at_run_time(move || look_ahead);
            let (stopped, registers) = machine.run();
            ran(&blob, &machine, stopped, &registers);
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
            // The Chrome runner makes pieces at run time at the default N only.
            if bound == wcontainers_translator::BOUND {
                let report = browser::translated_with(&blob, look_ahead, Default::default());
                let chrome = report.count("translated at run time");
                println!(
                    "{name} {optimisation} at bounded {bound} in Chrome: {chrome} pieces, {:.0} us each",
                    report.time("translated at run time") as f64 / chrome as f64 / 1e3,
                );
            }
        }
    }
}
