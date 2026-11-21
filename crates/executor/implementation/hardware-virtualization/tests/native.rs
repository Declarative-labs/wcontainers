//! N2's differential test: every presented ARM64 form's seeded cases (layer 1's) run as the native output's
//! piece at the guest's level inside Hypervisor.framework, each stop resolved through the position map, and
//! compared whole with the WebAssembly output under wasmtime; the count at each stop is compared with
//! definition E. Beside each drawn case of a load or store runs one of the bar's own made from it: across a
//! page that refuses, tagged, with bit 55 set, or misaligned where the form checks alignment. Beside them run
//! the words layer 1 leaves out because they are compared with the real CPU and these are not: each drawn
//! word with two of its register fields made one where `excluded` then names it (the register collisions),
//! and, for forms swept whole, every value of their other fields that `excluded` names (registers answered by
//! design, hints of features not presented, features not offered), each on a drawn case's registers. Each drawn
//! load or store exclusive also runs with its partner (the store after the load, the load before the store, same
//! size and base), so the monitor one leaves is compared through the other's status and stores. It runs itself
//! as a copy signed with the hypervisor entitlement.
//!
//! Knobs: `WCONTAINERS_NATIVE_CASES` cases per form, `WCONTAINERS_NATIVE_SEED`, `WCONTAINERS_NATIVE_FORMS` (a
//! comma-separated list of form names to run alone), `WCONTAINERS_NATIVE_JOBS` threads for the WebAssembly
//! side; `WCONTAINERS_NATIVE_BOUND` the look-ahead the exclusive pairs are translated at, both of their words in one
//! piece from look-ahead 2 on.

#[cfg(host_hypervisor)]
mod common;

#[cfg(host_hypervisor)]
#[path = "../../hardware-virtualization-inside/bin/native-cases/protocol.rs"]
mod protocol;

#[cfg(host_hypervisor)]
mod runner;

#[cfg(host_hypervisor)]
#[allow(dead_code)]
#[path = "../../../../wcontainers-arm64/tests/common/mod.rs"]
mod words;

#[cfg(host_hypervisor)]
fn main() {
    hosted::main();
}

#[cfg(not(host_hypervisor))]
fn main() {
    println!("the native test needs a hypervisor this crate drives; nothing to run on this host");
}

#[cfg(host_hypervisor)]
mod hosted {
    use std::time::Instant;

    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
    use wcontainers_arm64::encode::{self, field, form};
    use wcontainers_arm64::forms::{self, Form};
    use wcontainers_arm64::{Arm64, Instruction as I, Reg, SystemRegister, decode};
    use wcontainers_guest_cpu::{Guest, Native, Route};
    use wcontainers_translator::LookAhead;
    use wcontainers_translator_test::guest::arm64::words::{
        Case, Stop, cases, covered, excluded, translated,
    };
    use wcontainers_translator_test::layout::CODE;

    use crate::common::{be_signed, load_average, look_ahead, setting};
    use crate::runner::function::{same_end, webassembly};
    use crate::runner::{Runner, edge};
    use crate::words::{SWEPT_BITS, words_of};

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

    /// Why a word is left out here too: a branch to itself never finishes on the WebAssembly side.
    const ENDLESS: &str = "branch to itself";

    /// Whether `word` is one of `form`'s that layer 1 leaves out and this test runs.
    fn left_out(form: &Form, word: u32) -> bool {
        forms::form(word).is_some_and(|f| std::ptr::eq(f, form))
            && form.presented(word)
            && excluded(form, word).is_some_and(|why| why != ENDLESS)
    }

    /// The register fields of a form.
    fn register_fields(form: &Form) -> Vec<&str> {
        form.fields
            .iter()
            .filter(|(name, _, width)| {
                name.starts_with(field::REGISTER) && *width == encode::REGISTER_BITS
            })
            .map(|(name, ..)| name.as_str())
            .collect()
    }

    /// The drawn case with each word it becomes when one register field takes another's register, where
    /// layer 1 leaves that word out.
    fn collisions(form: &Form, case: &Case) -> Vec<Case> {
        let fields = register_fields(form);
        let mut words: Vec<u32> = Vec::new();
        for &to in &fields {
            for &from in &fields {
                let Some(value) = form.field(case.word, from).filter(|_| to != from) else {
                    continue;
                };
                if let Ok(word) = encode::replace(case.word, to, value)
                    && left_out(form, word)
                    && !words.contains(&word)
                {
                    words.push(word);
                }
            }
        }
        words
            .into_iter()
            .map(|word| Case {
                word,
                ..case.clone()
            })
            .collect()
    }

    /// The bits of a form other than its register fields that its words may set.
    fn swept_bits(form: &Form) -> u32 {
        let registers = form
            .fields
            .iter()
            .filter(|(name, ..)| name.starts_with(field::REGISTER))
            .fold(0u32, |m, &(_, low, width)| {
                m | (((1u32 << width) - 1) << low)
            });
        (!form.mask & !form.should_mask & !registers).count_ones()
    }

    /// For a form swept whole, every word of it that `keep` takes, each on the registers of one of `drawn` in
    /// turn.
    fn swept(form: &Form, drawn: &[Case], seed: u64, keep: impl Fn(u32) -> bool) -> Vec<Case> {
        if !(1..=SWEPT_BITS).contains(&swept_bits(form)) {
            return Vec::new();
        }
        let mut rng = fastrand::Rng::with_seed(seed);
        let words: Vec<u32> = words_of(form, 1, &mut rng)
            .into_iter()
            .filter(|&w| keep(w))
            .collect();
        words
            .into_iter()
            .zip(drawn.iter().cycle())
            .map(|(word, case)| Case {
                word,
                ..case.clone()
            })
            .collect()
    }

    /// A drawn exclusive case's word with its partner, in the order they run: a load exclusive followed by the
    /// store exclusive of its size, acquire bit and base, or a store exclusive preceded by such a load. The
    /// partner's other registers are ones the word does not name, so the store writes the case's values. With
    /// them, the store's status register.
    fn paired(form: &Form, case: &Case) -> Option<([u32; 2], usize)> {
        let (load, pair) = match decode(case.word) {
            I::LoadExclusive { pair, .. } => (true, pair),
            I::StoreExclusive { pair, .. } => (false, pair),
            _ => return None,
        };
        let named: Vec<u32> = register_fields(form)
            .iter()
            .filter_map(|f| form.field(case.word, f))
            .collect();
        let mut free = (0..u32::from(encode::ZERO)).filter(|r| !named.contains(r));
        let mut next = || free.next().expect("a register the word does not name");
        let zero = u32::from(encode::ZERO);
        let status = if load { next() } else { zero };
        let first = next();
        let second = if pair { next() } else { zero };
        let partner = [
            (field::LOAD, u32::from(!load)),
            (field::RS, status),
            (field::RT, first),
            (field::RT2, second),
        ]
        .into_iter()
        .try_fold(case.word, |word, (name, value)| {
            encode::replace(word, name, value)
        })
        .ok()
        .filter(|&word| forms::presented(word))?;
        let store = if load { partner } else { case.word };
        let status = form.field(store, field::RS)? as usize;
        Some((
            if load {
                [case.word, partner]
            } else {
                [partner, case.word]
            },
            status,
        ))
    }

    /// A counter read's destination: its value is the clock's when read, on either side.
    fn clock_destination(word: u32) -> Option<usize> {
        match decode(word) {
            I::ReadSystem {
                rt: Reg::X(n),
                register: SystemRegister::Counter,
            } => Some(usize::from(n)),
            _ => None,
        }
    }

    /// A case loading the code page's second word, run after a function of two words: what it loads is a fresh
    /// machine's, whatever ran before. The difference from the WebAssembly output, if any.
    fn code_page_fresh(runner: &mut Runner, look_ahead: LookAhead) -> Option<String> {
        const DESTINATION: encode::Register = 0;
        const BASE: encode::Register = 1;
        let unit = size_of::<u32>() as u64;
        let load = encode::load_store(form::LDR_32, DESTINATION, BASE, 0, unit).expect("fits");
        runner.load_function(&[load, load], look_ahead);
        runner.unload();
        let case = Case {
            word: load,
            registers: std::array::from_fn(|n| {
                if n == usize::from(BASE) {
                    CODE + Arm64::MAX_LENGTH
                } else {
                    0
                }
            }),
            vectors: std::array::from_fn(|_| 0),
            float: Default::default(),
            tracking: false,
            seed: 0,
        };
        runner.case(&case).differs(&translated(&case))
    }

    /// `run` of each of `items`, on `jobs` threads.
    fn on_threads<T: Sync, R: Send>(
        items: &[T],
        jobs: usize,
        run: impl Fn(&T) -> R + Sync,
    ) -> Vec<R> {
        let chunk = items.len().div_ceil(jobs).max(1);
        let run = &run;
        std::thread::scope(|scope| {
            let handles: Vec<_> = items
                .chunks(chunk)
                .map(|part| scope.spawn(move || part.iter().map(run).collect::<Vec<_>>()))
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
        let look_ahead = look_ahead();
        let only: Option<Vec<String>> = std::env::var(FORMS_VARIABLE)
            .ok()
            .map(|list| list.split(SEPARATOR).map(String::from).collect());
        let started = Instant::now();
        let mut runner = Runner::start();
        if let Some(difference) = code_page_fresh(&mut runner, look_ahead) {
            panic!("a case sees the code page of the function before it: {difference}");
        }
        let chosen: Vec<_> = forms::all()
            .iter()
            .filter(|f| covered(f))
            .filter(|f| only.as_ref().is_none_or(|names| names.contains(&f.name)))
            .collect();
        let mut failing = Vec::new();
        let mut total = 0u64;
        let mut edge_cases = 0u64;
        let mut left_out_cases = 0u64;
        let mut float_status_cases = 0u64;
        let mut pair_cases = 0u64;
        let mut stored = 0u64;
        let mut reasons: std::collections::BTreeMap<&str, u64> = Default::default();
        for form in &chosen {
            let mut generator = generator(seed, &form.name);
            let strategy = cases((*form).clone());
            let mut drawn: Vec<Case> = (0..count)
                .map(|_| strategy.new_tree(&mut generator).expect("a case").current())
                .collect();
            let edges: Vec<Case> = drawn.iter().filter_map(edge).collect();
            edge_cases += edges.len() as u64;
            let mut collided: Vec<Case> = drawn.iter().flat_map(|c| collisions(form, c)).collect();
            collided.extend(swept(form, &drawn, seed, |w| left_out(form, w)));
            left_out_cases += collided.len() as u64;
            // Every word routed for float status, with every register in its fields (words_of).
            let float_status = swept(form, &drawn, seed, |w| {
                !left_out(form, w) && Arm64::route(&decode(w)) == Route::FloatStatus
            });
            float_status_cases += float_status.len() as u64;
            for c in &collided {
                *reasons
                    .entry(excluded(form, c.word).expect("a left out word"))
                    .or_default() += 1;
            }
            drawn.extend(edges);
            drawn.extend(collided);
            drawn.extend(float_status);
            let cases = drawn.len();
            let (wasm, native) = std::thread::scope(|scope| {
                let wasm = scope.spawn(|| on_threads(&drawn, jobs, translated));
                let native: Vec<_> = drawn.iter().map(|case| runner.case(case)).collect();
                (wasm.join().expect("the WebAssembly side"), native)
            });
            let mut first = None;
            let mut failed = 0;
            for ((case, expected), got) in drawn.iter().zip(&wasm).zip(&native) {
                total += 1;
                let mut got = got.clone();
                if let Some(rt) = clock_destination(case.word) {
                    got.outcome.registers[rt] = expected.registers[rt];
                }
                if let Some(difference) = got.differs(expected) {
                    failed += 1;
                    first.get_or_insert_with(|| {
                        format!("word {:#010x}: {difference}\n  case {case:?}", case.word)
                    });
                }
            }
            let pairs: Vec<(Case, ([u32; 2], usize))> = drawn
                .iter()
                .take(count as usize)
                .filter_map(|c| Some((c.clone(), paired(form, c)?)))
                .collect();
            pair_cases += pairs.len() as u64;
            let (wasm, native) = std::thread::scope(|scope| {
                let wasm = scope.spawn(|| {
                    on_threads(&pairs, jobs, |(case, (words, _))| {
                        webassembly(words, case, look_ahead)
                    })
                });
                let native: Vec<_> = pairs
                    .iter()
                    .map(|(case, (words, _))| {
                        let mut function = runner.load_function(words, look_ahead);
                        let end = CODE + words.len() as u64 * Arm64::MAX_LENGTH;
                        let run = runner.run_function(&mut function, case, end, None);
                        runner.unload();
                        run
                    })
                    .collect();
                (wasm.join().expect("the WebAssembly side"), native)
            });
            for (((case, (words, status)), (stop, after, data)), got) in
                pairs.iter().zip(&wasm).zip(&native)
            {
                total += 1;
                let last = got.trace.last().expect("a state");
                if got.stop == Stop::Landed(CODE + words.len() as u64 * Arm64::MAX_LENGTH)
                    && last.x.get(*status) == Some(&0)
                {
                    stored += 1;
                }
                if !same_end(*stop, after, got.stop, last) || *data != got.data {
                    failed += 1;
                    first.get_or_insert_with(|| {
                        format!(
                            "words {words:#010x?} run as a pair: the WebAssembly output ends {stop:?} {after:x?}, natively {:?} {last:x?}\n  case {case:?}",
                            got.stop
                        )
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
            "native against WebAssembly: {} forms, {count} drawn cases each, {edge_cases} of the bar's own, {left_out_cases} words layer 1 leaves out, {float_status_cases} float status words and {pair_cases} exclusive pairs ({stored} storing) beside them ({total} in all), the pairs at {look_ahead:?}, seed {seed:#x}, {} failing, in {:?}, load {}",
            chosen.len(),
            failing.len(),
            started.elapsed(),
            load_average()
        );
        println!("words layer 1 leaves out, by its reason: {reasons:?}");
        if !failing.is_empty() {
            println!("failing forms: {}", failing.join(" "));
        }
        assert!(failing.is_empty(), "forms differ");
    }
}
