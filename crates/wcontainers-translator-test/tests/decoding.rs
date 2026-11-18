//! ARM64's own: its decoder against Arm's list of forms. Anything the CPU we present does not have is
//! undefined, and unsupported (counted and warned) when a CPU with more features has it.
//!
//! `WC_DECODE_CASES` words per form (default 64), seeded from `WC_DECODE_SEED` (default 1).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};
use wcontainers_arm64::{Instruction, decode, forms};

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

#[test]
fn words_of_forms_not_offered_are_undefined_or_unsupported_arm64() {
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
            if forms::presented(word) {
                return Ok(());
            }
            let elsewhere = forms::all().iter().any(|f| f.exists(word));
            match decode(word) {
                Instruction::Unsupported if elsewhere => Ok(()),
                Instruction::Undefined if !elsewhere => Ok(()),
                other => Err(TestCaseError::fail(format!(
                    "{word:#010x} is not offered (another CPU has it: {elsewhere}) but decodes as {other:?}"
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

/// Instructions of features we do not offer: pointer signing branches and loads, FlagM, SB, CSSC, MTE, the
/// encryption extensions and scalable vectors.
#[test]
fn features_not_offered_are_unsupported_arm64() {
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
        0x4e28_4820, // aese v0.16b, v1.16b
        0x4e28_6820, // aesimc v0.16b, v1.16b
        0x5e02_4020, // sha256h q0, q1, v2.4s
        0x5e28_0820, // sha1h s0, s1
        0xcec0_8020, // sha512su0 v0.2d, v1.2d
        0x4ee2_e020, // pmull2 v0.1q, v1.2d, v2.2d
        0x0ee2_e020, // pmull v0.1q, v1.1d, v2.1d
        0x04a2_0020, // add z0.s, z1.s, z2.s
        0x2598_e3e0, // ptrue p0.s
        0xa540_a000, // ld1w {z0.s}, p0/z, [x0]
        0xd503_405f, // msr dit, #0
        0xd51b_42a0, // msr dit, x0
        0xd53b_2420, // mrs x0, rndrrs
        0xd50b_7c20, // dc cvap, x0
        0xd50b_7460, // dc gva, x0
        0x5f10_e400, // fcvtzs h0, h0, #16
        0x4f10_e400, // fcvtzs v0.8h, v0.8h, #16
    ] {
        assert_eq!(decode(word), Instruction::Unsupported, "{word:#010x}");
    }
}

/// Words in no form of Arm's list that `yaxpeax-arm` accepts as base integer instructions: MOVZ with hw
/// above 1 in 32 bit, conditional select and compare with wrong fixed bits, and others. The CPU raises
/// illegal instruction for each.
#[test]
fn words_in_no_form_are_undefined_arm64() {
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
fn translated_words_are_of_forms_layer_1_covers_arm64() {
    use wcontainers_translator_test::guest::arm64::covered;
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
/// where Arm's decode makes that word undefined, to undefined; to unsupported exactly where Arm's decode
/// needs a feature we do not offer.
/// `WC_DECODE_CASES` words per form, seeded from `WC_DECODE_SEED`.
#[test]
fn every_offered_form_is_translated_arm64() {
    use wcontainers_translator_test::guest::arm64::{covered, of_feature_not_offered};
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
            let feature = of_feature_not_offered(form, word).is_some();
            if form.presented(word) && (decode(word) == Instruction::Unsupported) != feature {
                untranslated.push(format!("{} {word:#010x}", form.name));
                break;
            }
        }
    }
    eprintln!("decoding: {offered} offered forms, {count} words each, seed {seed}");
    assert!(untranslated.is_empty(), "not translated: {untranslated:?}");
}
