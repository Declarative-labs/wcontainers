//! The behaviour probe (docs/native.md section 5.8), run as a test that records, for the owner's decision 14,
//! whether this host would pass: every load and store form the native output gives the Split route's test
//! (every one that can cross a page) is copied as it is, with nothing in front, at the guest's level inside
//! Hypervisor.framework, its access placed across each of the data area's boundaries where a page refuses at
//! every byte offset, and what the hardware did is compared with the meaning's answer (the WebAssembly output
//! under wasmtime): the access faults and nothing changes. The host passes if every form of 16 bytes or fewer
//! (those a passing host would copy, section 3.4) matches; the forms of more than 16 bytes are listed apart,
//! since they take the Split route on every host. The verdict and the probe's time are printed; the test
//! fails only if the probe cannot run. It runs itself as a copy signed with the hypervisor entitlement.
//!
//! Knobs: `WCONTAINERS_PROBE_WORDS` words drawn per form, `WCONTAINERS_PROBE_SEED`.

#[cfg(host_hypervisor)]
mod common;

#[cfg(host_hypervisor)]
#[path = "../../hardware-virtualization-inside/bin/native-cases/protocol.rs"]
mod protocol;

#[cfg(host_hypervisor)]
mod runner;

#[cfg(host_hypervisor)]
fn main() {
    hosted::main();
}

#[cfg(not(host_hypervisor))]
fn main() {
    println!(
        "the behaviour probe needs a hypervisor this crate drives; nothing to run on this host"
    );
}

#[cfg(host_hypervisor)]
mod hosted {
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};

    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
    use wcontainers_arm64::{Arm64, decode, forms};
    use wcontainers_guest_cpu::{Native, Route};
    use wcontainers_translator_test::guest::arm64::words::{
        Case, Outcome, cases, covered, translated,
    };

    use crate::common::{be_signed, load_average, setting};
    use crate::runner::{REFUSING, Runner, access_bytes, across};

    const WORDS_VARIABLE: &str = "WCONTAINERS_PROBE_WORDS";
    const SEED_VARIABLE: &str = "WCONTAINERS_PROBE_SEED";
    /// Words drawn per form unless told otherwise.
    const DEFAULT_WORDS: u64 = 4;
    /// The seed unless told otherwise.
    const DEFAULT_SEED: u64 = 0x27bb_2ee6_87b0_b0fd;
    /// The most bytes a form a passing host would copy moves (section 3.4): a quadword.
    const COPIED_BYTES: u64 = size_of::<u128>() as u64;

    /// The random generator for a form's words, from the seed and the form's name.
    fn generator(seed: u64, name: &str) -> TestRunner {
        let mut key = [0u8; size_of::<[u64; 4]>()];
        key[..size_of::<u64>()].copy_from_slice(&seed.to_le_bytes());
        let rest = key.len() - size_of::<u64>();
        for (i, byte) in name.bytes().enumerate() {
            key[size_of::<u64>() + i % rest] ^= byte;
        }
        TestRunner::new_with_rng(
            Config::default(),
            TestRng::from_seed(RngAlgorithm::ChaCha, &key),
        )
    }

    /// What differs between the hardware's outcome and the meaning's.
    fn differs(got: &Outcome, expected: &Outcome) -> Option<String> {
        let mut found = Vec::new();
        if got.stop != expected.stop {
            found.push(format!("stop {:?}, meaning {:?}", got.stop, expected.stop));
        }
        for (k, (g, e)) in got.registers.iter().zip(&expected.registers).enumerate() {
            if g != e {
                found.push(format!("register {k} {g:#x}, meaning {e:#x}"));
            }
        }
        for (k, (g, e)) in got.vectors.iter().zip(&expected.vectors).enumerate() {
            if g != e {
                found.push(format!("v{k} {g:#x}, meaning {e:#x}"));
            }
        }
        if got.changed != expected.changed {
            found.push(format!(
                "{} bytes written, meaning {}",
                got.changed.len(),
                expected.changed.len()
            ));
        }
        (!found.is_empty()).then(|| found.join("; "))
    }

    pub fn main() {
        be_signed();
        let words = setting(WORDS_VARIABLE, DEFAULT_WORDS);
        let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
        let started = Instant::now();
        let mut runner = Runner::start();
        let mut probing = Duration::ZERO;
        let mut probed = 0u64;
        let mut cases_run = 0u64;
        // Each differing form: its bytes, how many of its cases differed, and the first.
        let mut differing: BTreeMap<String, (u64, u64, String)> = BTreeMap::new();
        for form in forms::all().iter().filter(|f| covered(f)) {
            let mut generator = generator(seed, &form.name);
            let strategy = cases(form.clone());
            let mut form_probed = false;
            for _ in 0..words {
                let case = strategy.new_tree(&mut generator).expect("a case").current();
                if Arm64::route(&decode(case.word)) != Route::Split {
                    continue;
                }
                let Some(bytes) = access_bytes(&case) else {
                    continue;
                };
                form_probed = true;
                let case = &case;
                let placed: Vec<Case> = REFUSING
                    .iter()
                    .flat_map(|&boundary| {
                        (1..bytes).filter_map(move |within| across(case, boundary, within))
                    })
                    .collect();
                std::thread::scope(|scope| {
                    let meanings = scope.spawn(|| placed.iter().map(translated).collect());
                    let at = Instant::now();
                    let got: Vec<Outcome> = placed.iter().map(|c| runner.raw(c)).collect();
                    probing += at.elapsed();
                    let meanings: Vec<Outcome> = meanings.join().expect("the meanings");
                    for ((c, g), e) in placed.iter().zip(&got).zip(&meanings) {
                        cases_run += 1;
                        if let Some(d) = differs(g, e) {
                            let entry = differing.entry(form.name.clone()).or_insert_with(|| {
                                (
                                    bytes,
                                    0,
                                    format!("word {:#010x}: {d}\n  case {c:x?}", c.word),
                                )
                            });
                            entry.1 += 1;
                        }
                    }
                });
            }
            probed += u64::from(form_probed);
        }
        runner.finish();
        let copied: Vec<_> = differing
            .iter()
            .filter(|(_, (bytes, ..))| *bytes <= COPIED_BYTES)
            .collect();
        for (name, (bytes, count, first)) in &differing {
            println!("differs {name} ({bytes} bytes, {count} cases): {first}");
        }
        println!(
            "behaviour probe: {probed} forms, {cases_run} placements ({words} words drawn per form, seed {seed:#x}); {} forms of more than {COPIED_BYTES} bytes differ (the Split route's on every host); {} forms of {COPIED_BYTES} bytes or fewer differ: this host {}; the hardware's runs took {probing:?}, the whole test {:?}, load {}",
            differing.len() - copied.len(),
            copied.len(),
            if copied.is_empty() {
                "would pass"
            } else {
                "would not pass"
            },
            started.elapsed(),
            load_average()
        );
        assert!(cases_run > 0, "the probe ran no case");
    }
}
