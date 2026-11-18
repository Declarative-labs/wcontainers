//! Page permissions changed at random stops and system calls: full look-ahead ends exactly as look-ahead none
//! given the same changes at the same points (same fault, address and kind, same registers, same memory).
//! Changes often hit the page a register points at and the pages around it, so they land on what a loop's
//! proof or a kept run covers and on the page after the one being accessed.
//!
//! Knobs: `WC_CHANGES_CASES` (random programs, default 64), `WC_CHANGES_SEED` (default 1),
//! `WC_CHANGES_PROGRAM_SEEDS` (runs of each C program, default 3).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use wcontainers_abi::{Exit, PAGE_SIZE, Permissions};
use wcontainers_arm64::Arm64;
use wcontainers_translator::{LookAhead, Omit};
use wcontainers_translator_test::changes::{Compared, Plan, compare};
use wcontainers_translator_test::guest::{Change, GuestTest};
use wcontainers_translator_test::layer2::program;
use wcontainers_translator_test::programs::{Library, build, ready};
use wcontainers_translator_test::runner::{Pause, Stopped, fields};

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

fn plan() -> impl Strategy<Value = Plan> {
    (
        any::<u64>(),
        prop::sample::select(vec![1u32, 4, 32, 256]),
        prop::sample::select(vec![1.0, 1.0, 1.0, 0.5, 0.1, 0.0]),
    )
        .prop_map(|(seed, every, stops)| Plan { seed, every, stops })
}

/// Counts of what the runs did, printed so a run says how strong it was.
#[derive(Default)]
struct Tally {
    ran: u64,
    changes: u64,
    handled: u64,
    faulted: u64,
    translated: u64,
    left: u64,
}

impl Tally {
    fn add(&mut self, compared: Compared) {
        self.ran += 1;
        self.changes += compared.changes as u64;
        self.handled += compared.handled as u64;
        self.translated += compared.translated;
        self.left += compared.left;
        if matches!(compared.stopped, Stopped::Exit(Exit::MemoryFault { .. })) {
            self.faulted += 1;
        }
    }

    fn report(&self, what: &str) {
        eprintln!(
            "changes: {} {what}; {} changes made, {} faults handled, {} ended by a fault; at full {} pieces \
             translated at run time, {} instructions run alone after a check failed",
            self.ran, self.changes, self.handled, self.faulted, self.translated, self.left
        );
    }
}

fn random_programs_with_changes_match_none<G: GuestTest>() {
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
    let result = runner.run(&(program::<G>(), plan()), |(program, plan)| {
        let blob = G::assemble(&program);
        tally.borrow_mut().add(compare::<G>(&blob, plan));
        Ok(())
    });
    tally.borrow().report(&format!(
        "random programs (shrinking included), seed {seed}"
    ));
    if let Err(error) = result {
        panic!("{error}");
    }
}

fn programs_with_changes_match_none<G: GuestTest>() {
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
        let blob = build::<G>(name, optimisation, Library::None);
        for seed in 0..seeds {
            for every in [16, 1024] {
                let plan = Plan {
                    seed,
                    every,
                    stops: 1.0,
                };
                eprintln!("{name} {optimisation}: {plan:?}");
                tally.add(compare::<G>(&blob, plan));
            }
        }
    }
    tally.report("program runs");
}

/// At the first system call, or at the first stop where the change's pc and field are reached, the change's
/// page is made to allow nothing; full must fault exactly as none does. With a stop, none stops at every
/// instruction and full at every stop check up to that point.
fn denied_at_the_first<G: GuestTest>(change: &Change) {
    let Change { blob, at, page } = change;
    let (at, page) = (*at, *page);
    let run = |look_ahead| {
        let mut machine = ready::<G>(blob, look_ahead, Omit::default());
        let mut first = true;
        machine.on_pause(move |pause, paused| {
            let reached = match at {
                None => pause == Pause::Syscall,
                Some((pc, field, value)) => {
                    let state = fields::<G>(&paused.registers()).0;
                    let reached = pause == Pause::Stop
                        && state[G::PC_FIELD] == u128::from(pc)
                        && state[field] == value;
                    // Full stops at every check until the point, then runs on without stopping.
                    if first && !reached {
                        paused.raise_stop_flag();
                    }
                    reached
                }
            };
            if reached && std::mem::take(&mut first) {
                paused.set_permissions(page..page + PAGE_SIZE, Permissions::empty());
            }
        });
        if at.is_some() {
            if look_ahead == LookAhead::None {
                machine.stop_at_every_check();
            } else {
                machine.raise_stop_flag();
            }
        }
        machine.run()
    };
    let none = run(LookAhead::None);
    assert!(
        matches!(none.0, Stopped::Exit(Exit::MemoryFault { address, .. }) if address / PAGE_SIZE == page / PAGE_SIZE),
        "{}: none {:?}",
        blob.name,
        none.0
    );
    let full = run(LookAhead::Full);
    assert_eq!(full.0, none.0, "{}: why it stopped", blob.name);
    assert_eq!(full.1, none.1, "{}: registers", blob.name);
}

/// The guest's hand-made programs (`GuestTest::changes`): groups and loops across system calls and calls,
/// accesses below sp once a frame is popped, the longest accesses onto a page changed at a stop.
fn hand_made_changes_match_none<G: GuestTest>() {
    let changes = G::changes();
    std::thread::scope(|scope| {
        for change in &changes {
            scope.spawn(move || denied_at_the_first::<G>(change));
        }
    });
    eprintln!("changes: {} hand-made programs", changes.len());
}

#[test]
fn random_programs_with_changes_match_none_arm64() {
    random_programs_with_changes_match_none::<Arm64>();
}

#[test]
fn programs_with_changes_match_none_arm64() {
    programs_with_changes_match_none::<Arm64>();
}

#[test]
fn hand_made_changes_match_none_arm64() {
    hand_made_changes_match_none::<Arm64>();
}
