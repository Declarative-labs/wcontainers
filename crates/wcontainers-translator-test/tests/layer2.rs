//! Layer 2: random small programs give the same result at full look-ahead, at bounded look-ahead of a random N,
//! and with pieces of all three mixed and rare events injected (see `mixed`), as at none, register by register
//! and byte by byte in the same engine, also when the thread stops at every stop check and carries on, and
//! the same output as on the real CPU.
//!
//! Knobs: `WC_LAYER2_CASES` (default 64), `WC_LAYER2_SEED` (default 1), `WC_LAYER2_NATIVE` (how many of the
//! cases also run on the real CPU, default 8; each starts a container).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::layer2::{OUTPUT, area_offset, assemble, program};
use wcontainers_translator_test::mixed::mixed;
use wcontainers_translator_test::programs::{
    native, ran as outcome, reachable_memory, ready, translated, translated_stopping_everywhere,
};

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

#[test]
fn full_matches_none_and_the_cpu() {
    let cases = knob("WC_LAYER2_CASES", 64);
    let seed = knob("WC_LAYER2_SEED", 1);
    let natives = knob("WC_LAYER2_NATIVE", 8);
    let mut seed_bytes = [0u8; 32];
    seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: cases as u32,
            failure_persistence: None,
            ..Config::default()
        },
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed_bytes),
    );
    let ran = std::cell::Cell::new(0u64);
    let faulted = std::cell::Cell::new(0u64);
    let left = std::cell::Cell::new(0u64);
    let lost = std::cell::Cell::new(0u64);
    let forgotten = std::cell::Cell::new(0u64);
    let result = runner.run(
        &(program(), 1..=64u32, any::<u64>()),
        |(program, bound, mix)| {
            let blob = assemble(&program);
            let (none, mut none_machine, none_registers) = translated(&blob, LookAhead::None);
            let (full, mut full_machine, full_registers) = translated(&blob, LookAhead::Full);
            // A program ends by writing its output, or by a memory fault (SIGSEGV).
            match none.status {
                0 => assert_eq!(none.stdout.len(), OUTPUT as usize),
                139 => faulted.set(faulted.get() + 1),
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
            assert_eq!(full, none, "output at full against none");
            assert_eq!(
                full_registers, none_registers,
                "registers at full against none"
            );
            assert!(
                reachable_memory(&mut full_machine, &blob)
                    == reachable_memory(&mut none_machine, &blob),
                "memory at full against none"
            );
            assert_eq!(full_machine.count(Reason::UnsupportedInstructionReached), 0);
            let (stopping, mut stopping_machine, stopping_registers) =
                translated_stopping_everywhere(&blob, LookAhead::Full);
            assert_eq!(stopping, none, "output at full stopping everywhere");
            assert_eq!(
                stopping_registers, none_registers,
                "registers at full stopping everywhere"
            );
            assert!(
                reachable_memory(&mut stopping_machine, &blob)
                    == reachable_memory(&mut none_machine, &blob),
                "memory at full stopping everywhere"
            );
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
                reachable_memory(&mut machine, &blob) == reachable_memory(&mut none_machine, &blob),
                "memory {what}"
            );
            let (mixed_ran, mut mixed_machine, mixed_registers, events) = mixed(&blob, mix);
            let what = format!("mixed from {mix} ({events:?})");
            assert_eq!(mixed_ran, none, "output {what}");
            assert_eq!(mixed_registers, none_registers, "registers {what}");
            assert!(
                reachable_memory(&mut mixed_machine, &blob)
                    == reachable_memory(&mut none_machine, &blob),
                "memory {what}"
            );
            lost.set(lost.get() + events.stacks_lost);
            forgotten.set(forgotten.get() + events.forgotten);
            if full_machine.count(Reason::LeftToAddress) > 0 {
                left.set(left.get() + 1);
            }
            if ran.get() < natives {
                assert_eq!(native(&blob), none, "output against the CPU");
            }
            ran.set(ran.get() + 1);
            Ok(())
        },
    );
    eprintln!(
        "layer 2: {} cases run, seed {seed}; {} faulted, {} left to an address at full, {} stacks lost and {} pieces forgotten in mixed runs",
        ran.get(),
        faulted.get(),
        left.get(),
        lost.get(),
        forgotten.get()
    );
    if let Err(error) = result {
        panic!("{error}");
    }
}
