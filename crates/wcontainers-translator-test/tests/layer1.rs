//! Layer 1 against the real CPU in Docker.
//!
//! `WC_LAYER1_CASES` cases per form (default 64), seeded from `WC_LAYER1_SEED` (default 1) and the form's
//! name, so any failure replays with the same two values. `WC_LAYER1_FORMS` limits the run to a
//! comma-separated list of form names.

use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};
use std::cell::RefCell;
use wcontainers_arm64::forms;
use wcontainers_translator_test::layer1::{Case, Stop, cases, compare, covered, translated};
use wcontainers_translator_test::layout::{CODE, DATA};
use wcontainers_translator_test::reference::Cases;

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
    for form in forms::all().iter().filter(|f| covered(f)) {
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

/// Alignment faults, each checked against the CPU and for the expected outcome, so the cases reach them.
#[test]
fn alignment_faults_match_the_cpu() {
    let stack = DATA + 0x1008;
    let target = CODE + 0x102;
    let cases = [
        // ldr x0, [sp]
        (0xf940_03e0, stack, Stop::Alignment(stack)),
        // stp x29, x30, [sp, #-16]!
        (0xa9bf_7bfd, stack, Stop::Alignment(stack)),
        // ldr x0, [sp, x1]
        (0xf861_6be0, stack + 3, Stop::Alignment(stack + 3)),
        // ldr x0, [sp, #8], with the stack pointer aligned
        (0xf940_07e0, stack + 8, Stop::Landed(CODE + 4)),
        // prfm pldl1keep, [sp]: a prefetch is not checked
        (0xf980_03e0, stack, Stop::Landed(CODE + 4)),
        // add sp, sp, #1: only loads and stores are checked
        (0x9100_07ff, stack, Stop::Landed(CODE + 4)),
        // br x1, to an address that is not a multiple of 4
        (0xd61f_0020, stack + 8, Stop::Alignment(target)),
        // rprfm through a misaligned stack pointer: a hint, so nothing
        (0xf8a0_4bf8, stack, Stop::Landed(CODE + 4)),
    ];
    let mut reference = Cases::start();
    for (word, sp, expected) in cases {
        let mut registers = [0u64; 34];
        registers[1] = target;
        registers[31] = sp;
        let case = Case {
            word,
            registers,
            seed: 1,
        };
        compare(&mut reference, &case).unwrap_or_else(|report| panic!("{report}"));
        assert_eq!(translated(&case).stop, expected, "word {word:#010x}");
    }
}
