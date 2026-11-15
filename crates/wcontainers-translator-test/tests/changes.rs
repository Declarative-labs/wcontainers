//! Page permissions changed at random stops and system calls: full look-ahead ends exactly as look-ahead none
//! given the same changes at the same points (same fault, address and kind, same registers, same memory).
//! Changes often hit the page a register points at and the pages around it, so they land on what a loop's
//! proof or a kept run covers and on the page after the one being accessed.
//!
//! Knobs: `WC_CHANGES_CASES` (random programs, default 64), `WC_CHANGES_SEED` (default 1),
//! `WC_CHANGES_PROGRAM_SEEDS` (runs of each C program, default 3).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use wcontainers_abi::Exit;
use wcontainers_translator_test::changes::{Compared, Plan, compare};
use wcontainers_translator_test::layer2::{assemble, program};
use wcontainers_translator_test::programs::{Library, build};
use wcontainers_translator_test::runner::Stopped;

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

fn plan() -> impl Strategy<Value = Plan> {
    (
        any::<u64>(),
        prop::sample::select(vec![1u32, 4, 32, 256]),
        prop::bool::weighted(0.75),
    )
        .prop_map(|(seed, every, stop_everywhere)| Plan {
            seed,
            every,
            stop_everywhere,
        })
}

/// Counts of what the runs did, printed so a run says how strong it was.
#[derive(Default)]
struct Tally {
    ran: u64,
    changes: u64,
    handled: u64,
    faulted: u64,
}

impl Tally {
    fn add(&mut self, compared: Compared) {
        self.ran += 1;
        self.changes += compared.changes as u64;
        self.handled += compared.handled as u64;
        if matches!(compared.stopped, Stopped::Exit(Exit::MemoryFault { .. })) {
            self.faulted += 1;
        }
    }

    fn report(&self, what: &str) {
        eprintln!(
            "changes: {} {what}; {} changes made, {} faults handled, {} ended by a fault",
            self.ran, self.changes, self.handled, self.faulted
        );
    }
}

#[test]
fn random_programs_with_changes_match_none() {
    let cases = knob("WC_CHANGES_CASES", 64);
    let seed = knob("WC_CHANGES_SEED", 1);
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
    let tally = std::cell::RefCell::new(Tally::default());
    let result = runner.run(&(program(), plan()), |(program, plan)| {
        let blob = assemble(&program);
        tally.borrow_mut().add(compare(&blob, plan));
        Ok(())
    });
    tally.borrow().report(&format!(
        "random programs (shrinking included), seed {seed}"
    ));
    if let Err(error) = result {
        panic!("{error}");
    }
}

#[test]
fn programs_with_changes_match_none() {
    let seeds = knob("WC_CHANGES_PROGRAM_SEEDS", 3);
    let mut tally = Tally::default();
    for (name, optimisation) in [
        ("walks", "-O0"),
        ("walks", "-O2"),
        ("integers", "-O2"),
        ("tables", "-O2"),
        ("bench", "-O0"),
        ("bench", "-O2"),
    ] {
        let blob = build(name, optimisation, Library::None);
        for seed in 0..seeds {
            for every in [16, 1024] {
                let plan = Plan {
                    seed,
                    every,
                    stop_everywhere: true,
                };
                eprintln!("{name} {optimisation}: {plan:?}");
                tally.add(compare(&blob, plan));
            }
        }
    }
    tally.report("program runs");
}
