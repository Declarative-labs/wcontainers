//! The decoder against Arm's list of forms: anything the CPU we present does not have is undefined.
//!
//! `WC_DECODE_CASES` words per form (default 64), seeded from `WC_DECODE_SEED` (default 1).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};
use wcontainers_abi::{HWCAP, HWCAP2, hwcap};
use wcontainers_arm64::{Instruction, decode, forms};

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

#[test]
fn offered_features_are_the_ones_the_kernel_reports() {
    let bit = |feature: &str| match feature {
        "FEAT_FP" => hwcap::FP,
        "FEAT_AdvSIMD" => hwcap::ASIMD,
        "FEAT_CRC32" => hwcap::CRC32,
        "FEAT_LSE" => hwcap::ATOMICS,
        other => panic!("no hwcap bit known for {other}"),
    };
    let offered = forms::OFFERED_FEATURES
        .iter()
        .fold(0, |all, f| all | bit(f));
    assert_eq!(offered, HWCAP);
    assert_eq!(HWCAP2, 0);
}

#[test]
fn words_of_forms_not_offered_are_undefined() {
    let count = setting("WC_DECODE_CASES", 64) as u32;
    let seed = setting("WC_DECODE_SEED", 1);
    let mut failures = Vec::new();
    for form in forms::all() {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&seed.to_le_bytes());
        for (i, byte) in form.name.bytes().enumerate() {
            key[8 + i % 24] ^= byte;
        }
        let mut config = Config::with_cases(count);
        config.failure_persistence = None;
        let mut runner =
            TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &key));
        let result = runner.run(&any::<u32>(), |free| {
            let word = form.word(free);
            match decode(word) {
                Instruction::Undefined => Ok(()),
                _ if forms::presented(word) => Ok(()),
                other => Err(TestCaseError::fail(format!(
                    "{word:#010x} is not offered but decodes as {other:?}"
                ))),
            }
        });
        if let Err(error) = result {
            eprintln!("{} ({}): {error}", form.name, form.features);
            failures.push(form.name.clone());
        }
    }
    eprintln!(
        "decoding: {} forms, {count} words each, seed {seed}, {} failing",
        forms::all().len(),
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:?}");
}

/// Instructions of features we do not offer: pointer signing branches and loads, FlagM, SB, CSSC, MTE.
#[test]
fn features_not_offered_are_undefined() {
    for word in [
        0xd65f_0bff, // retaa
        0xd61f_081f, // braaz
        0xd63f_081f, // blraaz
        0xf820_0420, // ldraa
        0x3a00_080d, // setf8
        0xba00_0400, // rmif
        0xd500_401f, // cfinv
        0xd503_30ff, // sb
        0xdac0_2020, // abs
        0x9180_0000, // addg
    ] {
        assert_eq!(decode(word), Instruction::Undefined, "{word:#010x}");
    }
}
