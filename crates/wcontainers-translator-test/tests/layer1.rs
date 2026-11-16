//! Layer 1 against the real CPU in Docker.
//!
//! `WC_LAYER1_CASES` cases per form (default 64), seeded from `WC_LAYER1_SEED` (default 1) and the form's
//! name, so any failure replays with the same two values. `WC_LAYER1_FORMS` limits the run to a
//! comma-separated list of form names. `WC_LAYER1_CHROME` cases per form (default 1), from the same seed,
//! also run in Chrome, a thousand to a page, and are compared with the CPU.

use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};
use std::cell::RefCell;
use wcontainers_arm64::forms;
use wcontainers_translator_test::layer1::{
    Case, Stop, cases, compare, compare_outcome, covered, translated, translated_in_chrome,
};
use wcontainers_translator_test::layout::{CODE, DATA};
use wcontainers_translator_test::reference::Cases;

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

#[test]
fn sample_matches_the_cpu_in_chrome() {
    let count = setting("WC_LAYER1_CHROME", 1);
    let seed = setting("WC_LAYER1_SEED", 1);
    let mut sample: Vec<(String, Case)> = Vec::new();
    for form in forms::all().iter().filter(|f| covered(f)) {
        let mut runner = generator(seed, &form.name, Config::default());
        let strategy = cases(form.clone());
        for _ in 0..count {
            let tree = strategy.new_tree(&mut runner).expect("a case");
            sample.push((form.name.clone(), tree.current()));
        }
    }
    let all: Vec<Case> = sample.iter().map(|(_, case)| case.clone()).collect();
    let outcomes = translated_in_chrome(&all);
    let mut reference = Cases::start();
    let mut failures = Vec::new();
    for ((name, case), outcome) in sample.iter().zip(outcomes) {
        if let Err(error) = compare_outcome(&mut reference, case, outcome) {
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
        let mut runner = generator(seed, &form.name, config);
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
            vectors: [0; 32],
            float: [0; 2],
            tracking: false,
            seed: 1,
        };
        compare(&mut reference, &case).unwrap_or_else(|report| panic!("{report}"));
        assert_eq!(translated(&case).stop, expected, "word {word:#010x}");
    }
}

/// The vector registers reach the CPU and come back: a known sum, checked against the CPU and for the
/// expected value.
#[test]
fn vector_registers_reach_the_cpu() {
    // add v0.16b, v1.16b, v2.16b
    let word = 0x4e22_8420;
    let mut vectors = [0u128; 32];
    vectors[1] = 0x0102_0304_0506_0708_090a_0b0c_0d0e_0fff;
    vectors[2] = 0x1010_1010_1010_1010_1010_1010_1010_1002;
    let case = Case {
        word,
        registers: [0; 34],
        vectors,
        float: [0; 2],
        tracking: false,
        seed: 1,
    };
    let mut reference = Cases::start();
    compare(&mut reference, &case).unwrap_or_else(|report| panic!("{report}"));
    let outcome = translated(&case);
    assert_eq!(
        outcome.vectors[0],
        0x1112_1314_1516_1718_191a_1b1c_1d1e_1f01
    );
    assert_eq!(outcome.vectors[1..3], vectors[1..3]);
}

/// Words in no form of Arm's list, for any feature, are illegal on the CPU and in ours. Random words,
/// `WC_LAYER1_CASES` of them (default 64), seeded from `WC_LAYER1_SEED`.
#[test]
fn words_in_no_form_are_illegal_as_on_the_cpu() {
    let count = setting("WC_LAYER1_CASES", 64) as u32;
    let seed = setting("WC_LAYER1_SEED", 1);
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(b"no form ");
    let mut config = Config::with_cases(count);
    config.failure_persistence = None;
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &key));
    let reference = RefCell::new(Cases::start());
    let unallocated = any::<u32>().prop_filter("in a form", |&word| {
        !forms::all().iter().any(|f| word & f.mask == f.value)
    });
    let result = runner.run(&unallocated, |word| {
        let mut registers = [0u64; 34];
        registers[31] = DATA + 0x1000;
        let case = Case {
            word,
            registers,
            vectors: [0; 32],
            float: [0; 2],
            tracking: false,
            seed,
        };
        compare(&mut reference.borrow_mut(), &case).map_err(TestCaseError::fail)?;
        prop_assert_eq!(translated(&case).stop, Stop::Illegal, "word {:#010x}", word);
        Ok(())
    });
    eprintln!("layer 1: {count} words in no form, seed {seed}");
    result.unwrap();
}

/// What we answer by design rather than as the reference CPU does, and the faults of the CPU we present
/// where the reference CPU has more features: checked against their expected outcome.
#[test]
fn designed_answers_and_faults() {
    use wcontainers_arm64::{CACHE_TYPE, MAIN_ID, ZERO_BLOCK_ID};
    let run = |word: u32, x1: u64, x2: u64| {
        let mut registers = [0u64; 34];
        registers[1] = x1;
        registers[2] = x2;
        registers[31] = DATA + 0x1000;
        translated(&Case {
            word,
            registers,
            vectors: [0; 32],
            float: [0; 2],
            tracking: false,
            seed: 1,
        })
    };
    // mrs x0 of CNTFRQ_EL0, CTR_EL0, DCZID_EL0, MIDR_EL1, MPIDR_EL1, ID_AA64ISAR0_EL1, ID_AA64PFR0_EL1.
    for (word, value) in [
        (0xd53b_e000, 1_000_000_000),
        (0xd53b_0020, CACHE_TYPE),
        (0xd53b_00e0, ZERO_BLOCK_ID),
        (0xd538_0000, MAIN_ID),
        (0xd538_00a0, 0x8000_0000),
        (0xd538_0600, 0x0021_0000),
        (0xd538_0400, 0x11),
    ] {
        let outcome = run(word, 0, 0);
        assert_eq!(outcome.stop, Stop::Landed(CODE + 4), "{word:#010x}");
        assert_eq!(outcome.registers[0], value, "{word:#010x}");
    }
    // mrs x0, cntvct_el0 counts up.
    assert_eq!(run(0xd53b_e040, 0, 0).stop, Stop::Landed(CODE + 4));
    // msr fpcr, x1 keeps only the bits we present.
    assert_eq!(
        run(0xd51b_4401, u64::MAX, 0).float[1],
        wcontainers_arm64::FPCR_BITS
    );
    // Unaligned atomics, load-acquire and exclusives fault, as a CPU without FEAT_LSE2 does:
    // ldadd x2, x0, [x1]; cas x0, x2, [x1]; ldar x0, [x1]; stlr x0, [x1]; ldxr x0, [x1]; ldaxp x0, x3, [x1].
    let unaligned = DATA + 0x101;
    for word in [
        0xf822_0020u32,
        0xc8a0_7c22,
        0xc8df_fc20,
        0xc89f_fc20,
        0xc85f_7c20,
        0xc87f_8c20,
    ] {
        assert_eq!(
            run(word, unaligned, 1).stop,
            Stop::Alignment(unaligned),
            "{word:#010x}"
        );
    }
    // Aligned, they work: ldadd x2, x0, [x1] leaves the old value.
    assert_eq!(
        run(0xf822_0020, DATA + 0x100, 1).stop,
        Stop::Landed(CODE + 4)
    );
}
