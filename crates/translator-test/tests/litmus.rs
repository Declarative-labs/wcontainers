//! XF3's x86 litmus shapes in the ordered form, with the default form as the control, on a weaker host (ARM64).
//!
//! Each test first runs its shape on the simulated weaker engine, exhaustively, and asserts that the control shows
//! every outcome no interleaving gives and the ordered form none.
//!
//! On the chip it then runs `WC_LITMUS_ROUNDS` rounds (default 10,000,000) in each form, prints every outcome's
//! count, and fails if the ordered form shows such an outcome; where the ARM64 chip shows the shape
//! ([`ChipControl::Asserted`]) it also fails if the control on an ARM64 host never does.

use std::time::Instant;
use wcontainers_translator::{AccessOrdering, OrderedAccess};
use wcontainers_translator_test::litmus::{self, Litmus};

/// The rounds of each form when `WC_LITMUS_ROUNDS` does not say.
const ROUNDS: u64 = 10_000_000;

fn rounds() -> u64 {
    std::env::var("WC_LITMUS_ROUNDS").map_or(ROUNDS, |v| v.parse().unwrap())
}

const ORDERED: AccessOrdering = AccessOrdering::Ordered(OrderedAccess::SequentiallyConsistent);
const FORMS: [AccessOrdering; 2] = [ORDERED, AccessOrdering::Unordered];

/// Whether the control on the chip is asserted, by what the rented ARM64 chip (Neoverse-V2) showed.
///
/// The default form ran 10,000,000 rounds in three runs (2026-11-25 and 2026-11-26: this track's two and the
/// independent check's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChipControl {
    /// Shown in every run, so asserted on an ARM64 host.
    ///
    /// Store buffering, with and without `mfence`, hundreds of thousands to millions of times; the sequence lock
    /// 81,385 to 210,800; message passing 23 to 11,942.
    Asserted,
    /// Not shown in every run, so the simulated control is the one asserted and the chip's count printed.
    ///
    /// Load buffering 0, 0 and 0; the store-back 2,237, 1 and 0; independent reads of independent writes 10, 69
    /// and 0.
    Simulated,
}

/// Asserts on the simulated weaker engine that the control shows every outcome no interleaving gives.
///
/// The ordered form must show none of them.
fn assert_simulated_control(litmus: &Litmus) {
    let started = Instant::now();
    let excluded: Vec<usize> = (0..litmus.outcomes())
        .filter(|&o| litmus.excluded_by_interleavings(&litmus.values(o)))
        .collect();
    assert!(
        !excluded.is_empty(),
        "{}: an outcome to look for",
        litmus.name
    );
    let ordered = litmus::simulate_shape(litmus, ORDERED);
    let control = litmus::simulate_shape(litmus, AccessOrdering::Unordered);
    for (form, simulated) in [(ORDERED, &ordered), (AccessOrdering::Unordered, &control)] {
        let shown: Vec<String> = simulated
            .outcomes
            .iter()
            .map(|&o| {
                let mark = if excluded.contains(&o) {
                    " (no interleaving)"
                } else {
                    ""
                };
                format!("{}{mark}", litmus.describe(o))
            })
            .collect();
        eprintln!(
            "{}: simulated {form:?}: values {:?}, runs by thread {:?}, {} executions, outcomes: {}",
            litmus.name,
            simulated.domain,
            simulated.traces,
            simulated.executions,
            shown.join(", ")
        );
    }
    for &outcome in &excluded {
        let described = litmus.describe(outcome);
        assert!(
            control.outcomes.contains(&outcome),
            "{}: the simulated control never shows {described}",
            litmus.name
        );
        assert!(
            !ordered.outcomes.contains(&outcome),
            "{}: the simulated ordered form shows {described}",
            litmus.name
        );
    }
    assert!(
        ordered.outcomes.is_subset(&control.outcomes),
        "{}: every interleaving keeps coherence",
        litmus.name
    );
    eprintln!(
        "{}: simulated: the control shows all {} outcomes no interleaving gives, the ordered form none, {:.1} s",
        litmus.name,
        excluded.len(),
        started.elapsed().as_secs_f64()
    );
}

/// Runs a shape in both forms on the chip; returns by form the rounds with an outcome no interleaving gives.
fn on_chip(litmus: &Litmus) -> Vec<u64> {
    let rounds = rounds();
    FORMS
        .iter()
        .map(|&form| {
            let started = Instant::now();
            let counts = litmus::run_shape(litmus, form, rounds);
            let mut excluded = 0;
            for (outcome, &count) in counts.iter().enumerate() {
                if count == 0 {
                    continue;
                }
                let values = litmus.values(outcome);
                let forbidden = (litmus.forbidden)(&values);
                if litmus.excluded_by_interleavings(&values) {
                    excluded += count;
                }
                eprintln!(
                    "{}: {form:?}: {}: {count}{}",
                    litmus.name,
                    litmus.describe(outcome),
                    if forbidden { " (x86 forbids it)" } else { "" }
                );
            }
            eprintln!(
                "{}: {form:?}: {excluded} of {rounds} rounds with an outcome no interleaving gives, {:.1} s",
                litmus.name,
                started.elapsed().as_secs_f64()
            );
            excluded
        })
        .collect()
}

/// Checks a shape: the simulated control, then the chip.
fn shape(litmus: &Litmus, chip: ChipControl) {
    assert_simulated_control(litmus);
    let counts = on_chip(litmus);
    assert_eq!(
        counts[0], 0,
        "{}: the ordered form on the chip",
        litmus.name
    );
    if chip == ChipControl::Asserted && cfg!(target_arch = "aarch64") {
        assert!(
            counts[1] > 0,
            "{}: the control on this ARM64 chip",
            litmus.name
        );
    }
}

#[test]
#[ignore]
fn store_buffering() {
    // x86 allows both loads 0; the ordered form's accesses are sequentially consistent, so it never shows it.
    shape(&litmus::STORE_BUFFERING, ChipControl::Asserted);
}

#[test]
#[ignore]
fn store_buffering_with_mfence() {
    shape(&litmus::STORE_BUFFERING_FENCED, ChipControl::Asserted);
}

#[test]
#[ignore]
fn message_passing() {
    shape(&litmus::MESSAGE_PASSING, ChipControl::Asserted);
}

#[test]
#[ignore]
fn load_buffering() {
    shape(&litmus::LOAD_BUFFERING, ChipControl::Simulated);
}

#[test]
#[ignore]
fn sequence_lock() {
    shape(&litmus::SEQUENCE_LOCK, ChipControl::Asserted);
}

#[test]
#[ignore]
fn store_back() {
    shape(&litmus::STORE_BACK, ChipControl::Simulated);
}

#[test]
#[ignore]
fn independent_reads_of_independent_writes() {
    shape(&litmus::INDEPENDENT_READS, ChipControl::Simulated);
}

/// Checks that the ordered form loses no addition of the lock xadd counter on `threads` threads.
///
/// The control, whose loop keeps the count in a register, must lose some.
fn counter(threads: usize) {
    let rounds = rounds();
    let expected = threads as u64 * rounds;
    let counts: Vec<u64> = FORMS
        .iter()
        .map(|&form| {
            let started = Instant::now();
            let count = litmus::run_counter(threads, form, rounds);
            eprintln!(
                "lock xadd counter, {threads} threads: {form:?}: {count} of {expected}, {} lost, {:.1} s",
                expected - count,
                started.elapsed().as_secs_f64()
            );
            count
        })
        .collect();
    assert_eq!(counts[0], expected, "the ordered form");
    assert!(counts[1] < expected, "the control loses additions");
}

#[test]
#[ignore]
fn lock_xadd_counter_two_threads() {
    counter(2);
}

#[test]
#[ignore]
fn lock_xadd_counter_four_threads() {
    counter(4);
}

/// The x86 host's form (XF5): ordered loads, plain stores, `mfence` a fence and locked instructions atomic.
const LOADS_ORDERED: AccessOrdering =
    AccessOrdering::LoadsOrdered(OrderedAccess::SequentiallyConsistent);

/// Checks on an x86-64 host that the x86 host's form shows no outcome x86 forbids and loses no addition.
///
/// The default form, whose `mfence` is nothing, is the control and shows store buffering through it.
#[cfg(target_arch = "x86_64")]
#[test]
#[ignore]
fn the_x86_host_form_keeps_x86_order() {
    let rounds = rounds();
    for litmus in litmus::SHAPES {
        let mut forbidden_by_form = Vec::new();
        for form in [LOADS_ORDERED, AccessOrdering::Unordered] {
            let started = Instant::now();
            let counts = litmus::run_shape(litmus, form, rounds);
            let forbidden: u64 = counts
                .iter()
                .enumerate()
                .filter(|&(outcome, _)| (litmus.forbidden)(&litmus.values(outcome)))
                .map(|(_, &count)| count)
                .sum();
            eprintln!(
                "{}: {form:?}: {forbidden} of {rounds} rounds x86 forbids, {:.1} s",
                litmus.name,
                started.elapsed().as_secs_f64()
            );
            forbidden_by_form.push(forbidden);
        }
        assert_eq!(
            forbidden_by_form[0], 0,
            "{}: the x86 host's form",
            litmus.name
        );
        if litmus.name == litmus::STORE_BUFFERING_FENCED.name {
            assert!(
                forbidden_by_form[1] > 0,
                "the control: store buffering through an empty mfence"
            );
        }
    }
    for threads in [2, 4] {
        let count = litmus::run_counter(threads, LOADS_ORDERED, rounds);
        eprintln!("lock xadd counter, {threads} threads: {LOADS_ORDERED:?}: {count}");
        assert_eq!(count, threads as u64 * rounds);
    }
}

/// Checks every shape on the simulated weaker engine: its control shows each outcome the ordered form never shows.
#[test]
fn the_simulated_control_shows_what_the_ordered_form_never_shows() {
    for litmus in litmus::SHAPES {
        assert_simulated_control(litmus);
    }
}

/// Runs a few rounds of every shape in both forms, so that a plain test run sees the programs translate and end.
#[test]
fn every_shape_runs_a_few_rounds() {
    const FEW: u64 = 100;
    for litmus in litmus::SHAPES {
        for form in FORMS {
            let counts = litmus::run_shape(litmus, form, FEW);
            assert_eq!(counts.iter().sum::<u64>(), FEW);
        }
    }
    for threads in [2, 4] {
        for form in [ORDERED, LOADS_ORDERED] {
            assert_eq!(
                litmus::run_counter(threads, form, FEW),
                threads as u64 * FEW
            );
        }
    }
}
