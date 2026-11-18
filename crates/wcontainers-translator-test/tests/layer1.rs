//! Layer 1 against the real CPU, written once for every guest.
//!
//! `WC_LAYER1_CASES` cases per form (default 64), seeded from `WC_LAYER1_SEED` (default 1) and the form's
//! name, so any failure replays with the same two values. `WC_LAYER1_FORMS` limits the run to a
//! comma-separated list of form names. `WC_LAYER1_JOBS` forms run at once (default one), each with a
//! reference runner of its own while it runs. `WC_LAYER1_CHROME` cases per form (default 1), from the same
//! seed, also run in Chrome, a thousand to a page, and are compared with the CPU.

use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Mutex;
use wcontainers_arm64::Arm64;
use wcontainers_translator_test::guest::{Case, GuestTest};
use wcontainers_translator_test::jobs::at_once;
use wcontainers_translator_test::layer1::{
    compare, compare_outcome, translated, translated_in_chrome,
};
use wcontainers_translator_test::reference::Reference;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// The random generator for a form's cases, from the seed and the form's name.
fn generator(seed: u64, name: &str, config: Config) -> TestRunner {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    for (i, byte) in name.bytes().enumerate() {
        key[8 + i % 24] ^= byte;
    }
    TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &key))
}

fn sample_matches_the_cpu_in_chrome<G: GuestTest>() {
    let count = setting("WC_LAYER1_CHROME", 1);
    let seed = setting("WC_LAYER1_SEED", 1);
    let mut sample: Vec<(String, Case)> = Vec::new();
    for form in G::forms() {
        let name = G::form_name(form);
        let mut runner = generator(seed, name, Config::default());
        let strategy = G::cases(form);
        for _ in 0..count {
            let tree = strategy.new_tree(&mut runner).expect("a case");
            sample.push((name.to_string(), tree.current()));
        }
    }
    let all: Vec<Case> = sample.iter().map(|(_, case)| case.clone()).collect();
    let outcomes = translated_in_chrome::<G>(&all);
    let mut reference = Reference::start(&G::REFERENCE);
    let mut failures = Vec::new();
    for ((name, case), outcome) in sample.iter().zip(outcomes) {
        if let Err(error) = compare_outcome::<G>(&mut reference, case, outcome) {
            eprintln!("{name} in Chrome: {error}");
            failures.push(name.clone());
        }
    }
    eprintln!(
        "layer 1 in Chrome: {} cases ({count} a form), seed {seed}, {} failing: {failures:?}",
        sample.len(),
        failures.len()
    );
    assert!(failures.is_empty());
}

fn every_form_matches_the_cpu<G: GuestTest>() {
    let count = setting("WC_LAYER1_CASES", 64) as u32;
    let seed = setting("WC_LAYER1_SEED", 1);
    let jobs = setting("WC_LAYER1_JOBS", 1).max(1);
    let only: Option<Vec<String>> = std::env::var("WC_LAYER1_FORMS")
        .ok()
        .map(|list| list.split(',').map(String::from).collect());
    let chosen: Vec<_> = G::forms()
        .iter()
        .filter(|f| {
            only.as_ref()
                .is_none_or(|names| names.iter().any(|n| n == G::form_name(f)))
        })
        .collect();
    let failures = Mutex::new(Vec::new());
    // Reference runners not in use; a form takes one, or starts one when none is free.
    let free = Mutex::new(Vec::new());
    at_once(jobs, chosen.len() as u64, |i| {
        let form = chosen[i as usize];
        let name = G::form_name(form);
        let taken = free.lock().unwrap().pop();
        let reference = RefCell::new(taken.unwrap_or_else(|| Reference::start(&G::REFERENCE)));
        let mut config = Config::with_cases(count);
        config.failure_persistence = None;
        config.max_global_rejects = 100 * count;
        let mut runner = generator(seed, name, config);
        let result = runner.run(&G::cases(form), |case| {
            compare::<G>(&mut reference.borrow_mut(), &case).map_err(TestCaseError::fail)
        });
        free.lock().unwrap().push(reference.into_inner());
        if let Err(error) = result {
            eprintln!("{name}: {error}");
            failures
                .lock()
                .unwrap()
                .push((G::form_group(form).to_string(), name.to_string()));
        }
    });
    let failing = failures.into_inner().unwrap();
    let mut groups = BTreeMap::<&str, usize>::new();
    for (group, _) in &failing {
        *groups.entry(group).or_default() += 1;
    }
    let mut failures: Vec<&String> = failing.iter().map(|(_, name)| name).collect();
    failures.sort();
    eprintln!(
        "layer 1: {} forms, {count} cases each, seed {seed}, {jobs} at once, {} failing: {failures:?}",
        chosen.len(),
        failures.len()
    );
    if !groups.is_empty() {
        eprintln!("layer 1 failing by group: {groups:?}");
    }
    assert!(
        failures.is_empty(),
        "{} forms differ from the CPU",
        failures.len()
    );
}

/// The guest's hand-made cases: each compared with the CPU unless answered by design, and checked for
/// what it must do.
fn hand_cases_do_what_they_must<G: GuestTest>() {
    let mut reference = Reference::start(&G::REFERENCE);
    let all = G::hand_cases();
    for designed in &all {
        let name = designed.name;
        if designed.compared {
            compare::<G>(&mut reference, &designed.case)
                .unwrap_or_else(|report| panic!("{name}: {report}"));
        }
        let outcome = translated::<G>(&designed.case);
        if let Some(stop) = designed.stop {
            assert_eq!(outcome.stop, stop, "{name}");
        }
        for &(field, value) in &designed.fields {
            let field_name = G::REFERENCE.fields[field];
            assert_eq!(
                outcome.state.0[field], value,
                "{name}: {field_name} is {:#x}, must be {value:#x}",
                outcome.state.0[field]
            );
        }
    }
    eprintln!("layer 1: {} hand-made cases", all.len());
}

/// Bytes in no form of the guest's, for any feature, stop ours as they stop the CPU. Random cases,
/// `WC_LAYER1_CASES` of them (default 64), seeded from `WC_LAYER1_SEED`.
fn bytes_in_no_form_stop_as_on_the_cpu<G: GuestTest>() {
    let count = setting("WC_LAYER1_CASES", 64) as u32;
    let seed = setting("WC_LAYER1_SEED", 1);
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(b"no form ");
    let mut config = Config::with_cases(count);
    config.failure_persistence = None;
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &key));
    let reference = RefCell::new(Reference::start(&G::REFERENCE));
    let result = runner.run(&G::no_form(seed), |case| {
        compare::<G>(&mut reference.borrow_mut(), &case).map_err(TestCaseError::fail)
    });
    eprintln!("layer 1: {count} cases in no form, seed {seed}");
    result.unwrap();
}

/// What the CPU offers is what a program is told it has.
fn offered_features_are_the_ones_programs_are_told<G: GuestTest>() {
    let (mut offered, mut told) = (G::offered(), G::told());
    offered.sort();
    told.sort();
    assert_eq!(offered, told);
}

#[test]
fn offered_features_are_the_ones_programs_are_told_arm64() {
    offered_features_are_the_ones_programs_are_told::<Arm64>();
}

#[test]
fn every_form_matches_the_cpu_arm64() {
    every_form_matches_the_cpu::<Arm64>();
}

#[test]
fn sample_matches_the_cpu_in_chrome_arm64() {
    sample_matches_the_cpu_in_chrome::<Arm64>();
}

#[test]
fn hand_cases_do_what_they_must_arm64() {
    hand_cases_do_what_they_must::<Arm64>();
}

#[test]
fn bytes_in_no_form_stop_as_on_the_cpu_arm64() {
    bytes_in_no_form_stop_as_on_the_cpu::<Arm64>();
}
