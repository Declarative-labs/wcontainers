//! XF3's x86 litmus shapes in the ordered form, with the default form as the control, on a weaker host (ARM64).
//!
//! Each test runs `WC_LITMUS_ROUNDS` rounds (default 10,000,000) in each form, prints every outcome's count, and
//! fails if the ordered form shows an outcome x86 forbids. The control's counts are printed, not checked: whether a
//! host shows a reordering is the host's.

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

/// Runs a shape in both forms; returns by form the rounds x86 forbids and, for two threads, those with both loads 0.
fn shape(litmus: &Litmus) -> Vec<(u64, u64)> {
    let rounds = rounds();
    FORMS
        .iter()
        .map(|&form| {
            let started = Instant::now();
            let counts = litmus::run_shape(litmus, form, rounds);
            let mut forbidden = 0;
            let mut zeros = 0;
            for (outcome, &count) in counts.iter().enumerate() {
                if count == 0 {
                    continue;
                }
                let values = litmus.values(outcome);
                let bad = (litmus.forbidden)(&values);
                if bad {
                    forbidden += count;
                }
                if litmus.threads() == 2 && litmus::both_saw_zero(&values) {
                    zeros += count;
                }
                eprintln!(
                    "{}: {form:?}: {}: {count}{}",
                    litmus.name,
                    litmus.describe(outcome),
                    if bad { " (x86 forbids it)" } else { "" }
                );
            }
            eprintln!(
                "{}: {form:?}: {forbidden} of {rounds} rounds x86 forbids, {:.1} s",
                litmus.name,
                started.elapsed().as_secs_f64()
            );
            (forbidden, zeros)
        })
        .collect()
}

#[test]
#[ignore]
fn store_buffering() {
    let counts = shape(&litmus::STORE_BUFFERING);
    // x86 allows both loads 0; the ordered form's accesses are sequentially consistent, so it never shows it.
    assert_eq!(counts[0], (0, 0));
}

#[test]
#[ignore]
fn store_buffering_with_mfence() {
    assert_eq!(shape(&litmus::STORE_BUFFERING_FENCED)[0].0, 0);
}

#[test]
#[ignore]
fn message_passing() {
    assert_eq!(shape(&litmus::MESSAGE_PASSING)[0].0, 0);
}

#[test]
#[ignore]
fn load_buffering() {
    assert_eq!(shape(&litmus::LOAD_BUFFERING)[0].0, 0);
}

#[test]
#[ignore]
fn sequence_lock() {
    assert_eq!(shape(&litmus::SEQUENCE_LOCK)[0].0, 0);
}

#[test]
#[ignore]
fn store_back() {
    assert_eq!(shape(&litmus::STORE_BACK)[0].0, 0);
}

#[test]
#[ignore]
fn independent_reads_of_independent_writes() {
    assert_eq!(shape(&litmus::INDEPENDENT_READS)[0].0, 0);
}

/// Checks that the ordered form loses no addition of the lock xadd counter on `threads` threads.
fn counter(threads: usize) {
    let rounds = rounds();
    let expected = threads as u64 * rounds;
    let mut ordered = None;
    for form in FORMS {
        let started = Instant::now();
        let count = litmus::run_counter(threads, form, rounds);
        eprintln!(
            "lock xadd counter, {threads} threads: {form:?}: {count} of {expected}, {} lost, {:.1} s",
            expected - count,
            started.elapsed().as_secs_f64()
        );
        ordered.get_or_insert(count);
    }
    assert_eq!(ordered, Some(expected));
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
        assert_eq!(
            litmus::run_counter(threads, ORDERED, FEW),
            threads as u64 * FEW
        );
    }
}
