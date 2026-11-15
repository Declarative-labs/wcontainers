//! Layer 1 against the real CPU in Docker.
//!
//! `WC_LAYER1_CASES` cases per form (default 64), seeded from `WC_LAYER1_SEED` (default 1) and the form's
//! name, so any failure replays with the same two values. `WC_LAYER1_FORMS` limits the run to a
//! comma-separated list of form names.

use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};
use std::cell::RefCell;
use wcontainers_translator_test::layer1::{cases, compare, covered};
use wcontainers_translator_test::{forms, reference::Cases};

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

#[test]
fn every_covered_form_matches_the_cpu() {
    let count = setting("WC_LAYER1_CASES", 64) as u32;
    let seed = setting("WC_LAYER1_SEED", 1);
    let only: Option<Vec<String>> = std::env::var("WC_LAYER1_FORMS")
        .ok()
        .map(|list| list.split(',').map(String::from).collect());
    let reference = RefCell::new(Cases::start());
    let mut failures = Vec::new();
    let mut forms_run = 0;
    for form in forms::all().into_iter().filter(covered) {
        if only
            .as_ref()
            .is_some_and(|names| !names.contains(&form.name))
        {
            continue;
        }
        forms_run += 1;
        let mut config = Config::with_cases(count);
        config.failure_persistence = None;
        config.max_global_rejects = 100 * count;
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&seed.to_le_bytes());
        for (i, byte) in form.name.bytes().enumerate() {
            key[8 + i % 24] ^= byte;
        }
        let mut runner =
            TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &key));
        let result = runner.run(&cases(form.clone()), |case| {
            compare(&mut reference.borrow_mut(), &case).map_err(TestCaseError::fail)
        });
        if let Err(error) = result {
            eprintln!("{}: {error}", form.name);
            failures.push(form.name.clone());
        }
    }
    eprintln!(
        "layer 1: {forms_run} forms, {count} cases each, seed {seed}, {} failing: {failures:?}",
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{} forms differ from the CPU",
        failures.len()
    );
}
