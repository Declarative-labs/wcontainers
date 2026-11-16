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

/// Words in no form of Arm's list that `yaxpeax-arm` accepts as base integer instructions: MOVZ with hw
/// above 1 in 32 bit, conditional select and compare with wrong fixed bits, and others. The CPU raises
/// illegal instruction for each.
#[test]
fn words_in_no_form_are_undefined() {
    for word in [
        0x52d7_c74f,
        0x12ca_4fde,
        0x72d3_c194,
        0xdaac_9658,
        0x9aae_83e2,
        0x3a65_c0e8,
        0xba7c_3a01,
        0x3a28_0002,
        0x1afe_25d5,
        0x9aea_0d40,
        0xdae0_0189,
        0x5ae0_099c,
    ] {
        assert!(
            !forms::all().iter().any(|f| word & f.mask == f.value),
            "{word:#010x}"
        );
        assert_eq!(decode(word), Instruction::Undefined, "{word:#010x}");
    }
}

/// Every word we translate is a word of a form layer 1 compares with the CPU, so no meaning is given to a
/// word whose form is unproven. `WC_DECODE_WORDS` random words (default 200,000), seeded from
/// `WC_DECODE_SEED`.
#[test]
fn translated_words_are_of_forms_layer_1_covers() {
    use wcontainers_translator_test::layer1::covered;
    let count = setting("WC_DECODE_WORDS", 200_000) as u32;
    let seed = setting("WC_DECODE_SEED", 1);
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(b"any word");
    let mut config = Config::with_cases(count);
    config.failure_persistence = None;
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &key));
    let result = runner.run(&any::<u32>(), |word| match decode(word) {
        Instruction::Undefined | Instruction::Unsupported => Ok(()),
        // A word of a form taken from a hint space runs as that hint, which does nothing.
        Instruction::Nothing
            if forms::all()
                .iter()
                .any(|f| f.hint_without_feature() && word & f.mask == f.value) =>
        {
            Ok(())
        }
        _ if forms::all().iter().any(|f| f.presented(word) && covered(f)) => Ok(()),
        other => Err(TestCaseError::fail(format!(
            "{word:#010x} decodes as {other:?}, but no form layer 1 covers holds it: {:?}",
            forms::all()
                .iter()
                .filter(|f| f.presented(word))
                .map(|f| &f.name)
                .collect::<Vec<_>>()
        ))),
    });
    eprintln!("decoding: {count} random words, seed {seed}");
    result.unwrap();
}

/// No form of the CPU we present is left untranslated: words of every one decode to an instruction or,
/// where Arm's decode makes that word undefined, to undefined; never to "not translated".
/// `WC_DECODE_CASES` words per form, seeded from `WC_DECODE_SEED`.
#[test]
fn every_offered_form_is_translated() {
    use wcontainers_translator_test::layer1::covered;
    let count = setting("WC_DECODE_CASES", 64) as u32;
    let seed = setting("WC_DECODE_SEED", 1);
    let mut untranslated = Vec::new();
    let mut offered = 0;
    for form in forms::all().iter().filter(|f| covered(f)) {
        offered += 1;
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&seed.to_le_bytes());
        key[8..16].copy_from_slice(b"all form");
        for (i, byte) in form.name.bytes().enumerate() {
            key[16 + i % 16] ^= byte;
        }
        let mut rng = TestRng::from_seed(RngAlgorithm::ChaCha, &key);
        for _ in 0..count {
            let word = form.word(rng.random());
            if form.presented(word) && decode(word) == Instruction::Unsupported {
                untranslated.push(format!("{} {word:#010x}", form.name));
                break;
            }
        }
    }
    eprintln!("decoding: {offered} offered forms, {count} words each, seed {seed}");
    assert!(untranslated.is_empty(), "not translated: {untranslated:?}");
}
