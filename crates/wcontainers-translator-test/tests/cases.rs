//! Not a check by itself: writes the random cases layer 1 and layer 2 draw for a seed, so that two commits
//! can be shown to draw the same ones (`same-cases.sh` runs each commit's own copy of this file and compares).
//! Layer 1: one line a form, its name, how many cases and the BLAKE3 hash of their text (the instruction's
//! bytes, the data seed, the tracking bit and the state as the reference's wire fields in order). Layer 2:
//! one line a program, its number, its bytes' length and hash, its code bytes and the other values drawn
//! with it. One command:
//! `WC_CASES_DIR=<directory> WC_CASES_SEED=<seed> nix develop -c cargo test --release -p wcontainers-translator-test --test cases -- --ignored --nocapture`
//! Knobs: `WC_CASES_LAYER1` (cases a form, default 64), `WC_CASES_LAYER2` (programs, default 64).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use std::cell::{Cell, RefCell};
use std::fmt::Write;
use wcontainers_arm64::forms;
use wcontainers_translator_test::layer1::{Case, cases, covered};
use wcontainers_translator_test::layer2::{assemble, program};
use wcontainers_translator_test::layout::CODE;

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// Layer 1's generator for a form, as `tests/layer1.rs` makes it.
fn form_generator(seed: u64, name: &str, config: Config) -> TestRunner {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    for (i, byte) in name.bytes().enumerate() {
        key[8 + i % 24] ^= byte;
    }
    TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &key))
}

/// Layer 2's generator for a case, as `tests/layer2.rs` makes it.
fn program_generator(seed: u64, case: u64) -> TestRunner {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(&case.to_le_bytes());
    let config = Config {
        cases: 1,
        failure_persistence: None,
        ..Config::default()
    };
    TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &key))
}

/// A case as text: its instruction's bytes, data seed and tracking bit, then the reference's fields (x0 to
/// x30, sp, pc, the flags, the thread pointer, v0 to v31, FPSR, FPCR).
fn case_text(case: &Case) -> String {
    let mut fields: Vec<u128> = case.registers[..32].iter().map(|&r| r.into()).collect();
    fields.push(CODE.into());
    fields.extend(case.registers[32..].iter().map(|&r| u128::from(r)));
    fields.extend(case.vectors);
    fields.extend(case.float.map(u128::from));
    let mut text = String::new();
    for byte in case.word.to_le_bytes() {
        write!(text, "{byte:02x}").unwrap();
    }
    write!(text, " {:x} {}", case.seed, u8::from(case.tracking)).unwrap();
    for field in fields {
        write!(text, " {field:x}").unwrap();
    }
    text.push('\n');
    text
}

fn layer1(seed: u64, count: u32) -> String {
    let mut out = String::new();
    for form in forms::all().iter().filter(|f| covered(f)) {
        let mut config = Config::with_cases(count);
        config.failure_persistence = None;
        config.max_global_rejects = 100 * count;
        // The runner's closure is `Fn`.
        let hasher = RefCell::new(blake3::Hasher::new());
        let drawn = Cell::new(0);
        form_generator(seed, &form.name, config)
            .run(&cases(form.clone()), |case| {
                hasher.borrow_mut().update(case_text(&case).as_bytes());
                drawn.set(drawn.get() + 1);
                Ok(())
            })
            .unwrap();
        let (drawn, hash) = (drawn.get(), hasher.borrow().finalize());
        writeln!(out, "{} {drawn} {hash}", form.name).unwrap();
    }
    out
}

fn layer2(seed: u64, count: u64) -> String {
    let out = RefCell::new(String::new());
    for case in 0..count {
        program_generator(seed, case)
            .run(
                &(program(), 1..=64u32, any::<u64>()),
                |(program, bound, mix)| {
                    let blob = assemble(&program);
                    writeln!(
                        out.borrow_mut(),
                        "{case} {} {} {} {bound} {mix:x}",
                        blob.bytes.len(),
                        blake3::hash(&blob.bytes),
                        blob.code_bytes
                    )
                    .unwrap();
                    Ok(())
                },
            )
            .unwrap();
    }
    out.into_inner()
}

#[test]
#[ignore]
fn write_cases() {
    let directory = std::env::var("WC_CASES_DIR").expect("WC_CASES_DIR names where to write");
    let seed = knob("WC_CASES_SEED", 1);
    std::fs::create_dir_all(&directory).unwrap();
    let one = knob("WC_CASES_LAYER1", 64) as u32;
    std::fs::write(format!("{directory}/layer1-{seed}.txt"), layer1(seed, one)).unwrap();
    let two = knob("WC_CASES_LAYER2", 64);
    std::fs::write(format!("{directory}/layer2-{seed}.txt"), layer2(seed, two)).unwrap();
    eprintln!("wrote {directory}/layer1-{seed}.txt and layer2-{seed}.txt");
}
