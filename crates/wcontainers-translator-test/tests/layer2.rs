//! Layer 2: random small programs give the same result at full look-ahead (for speed and for size, plain and
//! with the thread stopping at every stop check and carrying on), at bounded look-ahead of a random N, and
//! with pieces of all three mixed and rare events injected (see `mixed`), as at none, register by register
//! and byte by byte in the same engine, and the same output as on the real CPU.
//!
//! Knobs: `WC_LAYER2_CASES` (default 64), `WC_LAYER2_SEED` (default 1), `WC_LAYER2_FROM` (the first case,
//! default 0; case `i` is generated from the seed and `i` alone, so a failure replays with `WC_LAYER2_FROM=i`
//! and `WC_LAYER2_CASES=1`), `WC_LAYER2_JOBS` (cases run at once, default 1), `WC_LAYER2_NATIVE` (how many
//! of the cases, the first, also run on the real CPU, default 8; each starts a container) and
//! `WC_LAYER2_CHROME` (how many, the first, also run in Chrome at none and at full in both settings, default
//! 1; each starts Chrome three times).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{LookAhead, Optimise};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::layer2::{OUTPUT, area_offset, assemble, program};
use wcontainers_translator_test::mixed::mixed;
use wcontainers_translator_test::programs::{
    native, ran as outcome, reachable_memory, ready, translated, translated_for,
    translated_stopping_everywhere,
};

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// The random generator of case `case` of a run from `seed`, so that any case replays by itself.
fn generator(seed: u64, case: u64) -> TestRunner {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(&case.to_le_bytes());
    let config = Config {
        cases: 1,
        failure_persistence: None,
        ..Config::default()
    };
    TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &key))
}

#[test]
fn full_matches_none_and_the_cpu() {
    let cases = knob("WC_LAYER2_CASES", 64);
    let seed = knob("WC_LAYER2_SEED", 1);
    let from = knob("WC_LAYER2_FROM", 0);
    let natives = knob("WC_LAYER2_NATIVE", 8);
    let chrome = knob("WC_LAYER2_CHROME", 1);
    let jobs = knob("WC_LAYER2_JOBS", 1).max(1);
    let next = AtomicU64::new(0);
    let [ran, faulted, left, lost, forgotten] = std::array::from_fn(|_| AtomicU64::new(0));
    let failures = Mutex::new(Vec::new());
    let count = |counter: &AtomicU64, n: u64| counter.fetch_add(n, Ordering::Relaxed);
    let work = || loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= cases {
            return;
        }
        let case = from + i;
        let (on_cpu, in_chrome) = (i < natives, i < chrome);
        let result = generator(seed, case).run(
            &(program(), 1..=64u32, any::<u64>()),
            |(program, bound, mix)| {
                let blob = assemble(&program);
                let (none, mut none_machine, none_registers) = translated(&blob, LookAhead::None);
                // A program ends by writing its output, or by a memory fault (SIGSEGV).
                match none.status {
                    0 => assert_eq!(none.stdout.len(), OUTPUT as usize),
                    139 => _ = count(&faulted, 1),
                    other => panic!("status {other} at none"),
                }
                // Accesses stay above the area's start: the code and tables are as built.
                let below = area_offset(&blob);
                let changed = reachable_memory(&mut none_machine, &blob)[..below]
                    .iter()
                    .zip(&blob.bytes)
                    .rposition(|(now, built)| now != built);
                assert_eq!(
                    changed.map(|at| at as i64 - below as i64),
                    None,
                    "a store this many bytes from the area's start"
                );
                let none_memory = reachable_memory(&mut none_machine, &blob);
                // Full for speed and for size, plain and stopping at every stop check.
                for optimise in [Optimise::Speed, Optimise::Size] {
                    let how = (LookAhead::Full, optimise);
                    for stopping in [false, true] {
                        let (full, mut full_machine, full_registers) = if stopping {
                            translated_stopping_everywhere(&blob, how)
                        } else {
                            translated_for(&blob, how)
                        };
                        let what =
                            format!("at full for {optimise:?}, stopping everywhere {stopping}");
                        assert_eq!(full, none, "output {what}");
                        assert_eq!(full_registers, none_registers, "registers {what}");
                        assert!(
                            reachable_memory(&mut full_machine, &blob) == none_memory,
                            "memory {what}"
                        );
                        assert_eq!(full_machine.count(Reason::UnsupportedInstructionReached), 0);
                        if optimise == Optimise::Speed
                            && !stopping
                            && full_machine.count(Reason::LeftToAddress) > 0
                        {
                            count(&left, 1);
                        }
                    }
                }
                // Bounded from the first instruction ahead of time and from wherever the code goes at run time.
                let look_ahead = LookAhead::Bounded(bound);
                let mut machine = ready(&blob, look_ahead, Default::default());
                machine.translate_at_run_time(move || look_ahead);
                let (stopped, registers) = machine.run();
                let what = format!("at bounded {bound}");
                assert_eq!(
                    outcome(&blob, &machine, stopped, &registers),
                    none,
                    "output {what}"
                );
                assert_eq!(registers, none_registers, "registers {what}");
                assert!(
                    reachable_memory(&mut machine, &blob) == none_memory,
                    "memory {what}"
                );
                let (mixed_ran, mut mixed_machine, mixed_registers, events) = mixed(&blob, mix);
                let what = format!("mixed from {mix} ({events:?})");
                assert_eq!(mixed_ran, none, "output {what}");
                assert_eq!(mixed_registers, none_registers, "registers {what}");
                assert!(
                    reachable_memory(&mut mixed_machine, &blob) == none_memory,
                    "memory {what}"
                );
                count(&lost, events.stacks_lost);
                count(&forgotten, events.forgotten);
                if on_cpu {
                    assert_eq!(native(&blob), none, "output against the CPU");
                }
                if in_chrome {
                    for how in [
                        (LookAhead::None, Optimise::Speed),
                        (LookAhead::Full, Optimise::Speed),
                        (LookAhead::Full, Optimise::Size),
                    ] {
                        let ours = browser::translated_with(&blob, how, Default::default());
                        assert_eq!(ours.ran, none, "output in Chrome at {how:?}");
                    }
                }
                Ok(())
            },
        );
        count(&ran, 1);
        if let Err(error) = result {
            eprintln!("layer 2, seed {seed}, case {case}: {error}");
            failures.lock().unwrap().push(case);
        }
    };
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(work);
        }
    });
    let mut failures = failures.into_inner().unwrap();
    failures.sort();
    let [ran, faulted, left, lost, forgotten] =
        [ran, faulted, left, lost, forgotten].map(|c| c.into_inner());
    eprintln!(
        "layer 2: {ran} cases run from case {from}, seed {seed}, {jobs} at once, {} on the CPU, {} in Chrome; {faulted} faulted, {left} left to an address at full, {lost} stacks lost and {forgotten} pieces forgotten in mixed runs; {} failing: {failures:?}",
        natives.min(cases),
        chrome.min(cases),
        failures.len()
    );
    assert!(failures.is_empty());
}
