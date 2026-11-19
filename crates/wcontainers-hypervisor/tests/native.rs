//! N2's differential test: every presented ARM64 form's seeded cases (layer 1's) run as the native output's
//! piece at the guest's level inside Hypervisor.framework, each stop resolved through the position map, and
//! compared whole with the WebAssembly output under wasmtime; the count at each stop is compared with
//! definition E. Beside each drawn case of a load or store runs one of the bar's own made from it: across a
//! page that refuses, tagged, with bit 55 set, or misaligned where the form checks alignment. It runs itself as a copy signed with the hypervisor entitlement.
//!
//! Knobs: `WCONTAINERS_NATIVE_CASES` cases per form, `WCONTAINERS_NATIVE_SEED`, `WCONTAINERS_NATIVE_FORMS` (a
//! comma-separated list of form names to run alone), `WCONTAINERS_NATIVE_JOBS` threads for the WebAssembly
//! side.

#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
#[path = "../../wcontainers-inside/src/bin/native-cases/protocol.rs"]
mod protocol;

#[cfg(target_os = "macos")]
mod runner;

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the native test needs Hypervisor.framework; nothing to run on this host");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::time::Instant;

    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
    use wcontainers_arm64::forms;
    use wcontainers_translator_test::layer1::{Case, Outcome, cases, covered, translated};

    use crate::common::{be_signed, load_average, setting};
    use crate::runner::{Runner, edge};

    const CASES_VARIABLE: &str = "WCONTAINERS_NATIVE_CASES";
    const SEED_VARIABLE: &str = "WCONTAINERS_NATIVE_SEED";
    const FORMS_VARIABLE: &str = "WCONTAINERS_NATIVE_FORMS";
    const JOBS_VARIABLE: &str = "WCONTAINERS_NATIVE_JOBS";
    /// Cases per form unless told otherwise.
    const DEFAULT_CASES: u64 = 16;
    /// The seed unless told otherwise.
    const DEFAULT_SEED: u64 = 0x2545_f491_4f6c_dd1d;
    /// Threads the WebAssembly side runs on unless told otherwise: the Mac's other work has equal claim.
    const DEFAULT_JOBS: u64 = 3;
    /// What separates form names in the list of forms to run.
    const SEPARATOR: char = ',';

    /// The random generator for a form's cases, from the seed and the form's name.
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

    /// The WebAssembly output's outcomes of `cases`, on `jobs` threads.
    fn translated_all(cases: &[Case], jobs: usize) -> Vec<Outcome> {
        let chunk = cases.len().div_ceil(jobs).max(1);
        std::thread::scope(|scope| {
            let handles: Vec<_> = cases
                .chunks(chunk)
                .map(|part| scope.spawn(move || part.iter().map(translated).collect::<Vec<_>>()))
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("the WebAssembly side"))
                .collect()
        })
    }

    pub fn main() {
        be_signed();
        let count = setting(CASES_VARIABLE, DEFAULT_CASES);
        let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
        let jobs = setting(JOBS_VARIABLE, DEFAULT_JOBS).max(1) as usize;
        let only: Option<Vec<String>> = std::env::var(FORMS_VARIABLE)
            .ok()
            .map(|list| list.split(SEPARATOR).map(String::from).collect());
        let started = Instant::now();
        let mut runner = Runner::start();
        let chosen: Vec<_> = forms::all()
            .iter()
            .filter(|f| covered(f))
            .filter(|f| only.as_ref().is_none_or(|names| names.contains(&f.name)))
            .collect();
        let mut failing = Vec::new();
        let mut total = 0u64;
        let mut edge_cases = 0u64;
        for form in &chosen {
            let mut generator = generator(seed, &form.name);
            let strategy = cases((*form).clone());
            let mut drawn: Vec<Case> = (0..count)
                .map(|_| strategy.new_tree(&mut generator).expect("a case").current())
                .collect();
            let edges: Vec<Case> = drawn.iter().filter_map(edge).collect();
            edge_cases += edges.len() as u64;
            drawn.extend(edges);
            let cases = drawn.len();
            let (wasm, native) = std::thread::scope(|scope| {
                let wasm = scope.spawn(|| translated_all(&drawn, jobs));
                let native: Vec<_> = drawn.iter().map(|case| runner.case(case)).collect();
                (wasm.join().expect("the WebAssembly side"), native)
            });
            let mut first = None;
            let mut failed = 0;
            for ((case, expected), got) in drawn.iter().zip(&wasm).zip(&native) {
                total += 1;
                if let Some(difference) = got.differs(expected) {
                    failed += 1;
                    first.get_or_insert_with(|| {
                        format!("word {:#010x}: {difference}\n  case {case:?}", case.word)
                    });
                }
            }
            if let Some(first) = first {
                println!("FAIL {}: {failed} of {cases}: {first}", form.name);
                failing.push(form.name.clone());
            }
        }
        runner.finish();
        println!(
            "native against WebAssembly: {} forms, {count} drawn cases each, {edge_cases} of the bar's own beside them ({total} in all), seed {seed:#x}, {} failing, in {:?}, load {}",
            chosen.len(),
            failing.len(),
            started.elapsed(),
            load_average()
        );
        if !failing.is_empty() {
            println!("failing forms: {}", failing.join(" "));
        }
        assert!(failing.is_empty(), "forms differ");
    }
}
