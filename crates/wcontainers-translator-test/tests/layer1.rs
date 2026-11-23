//! Layer 1 against the real CPU, written once for every guest.
//!
//! `WC_LAYER1_CASES` cases per form (default 64), seeded from `WC_LAYER1_SEED` (default 1) and the form's
//! name, so any failure replays with the same two values. `WC_LAYER1_FORMS` limits the run to a
//! comma-separated list of form names. `WC_LAYER1_JOBS` forms run at once (default one), each with a
//! reference runner of its own while it runs. `WC_LAYER1_CHROME` cases per form (default 1), from the same
//! seed, also run in Chrome, a thousand to a page, and are compared with the CPU.

use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, TestCaseError, TestRunner};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Mutex;
use wcontainers_arm64::Arm64;
use wcontainers_translator_test::guest::{Case, GuestTest, Stop};
use wcontainers_translator_test::jobs::at_once;
use wcontainers_translator_test::layer1::{
    Agreement, compare, compare_outcome, on_reference, translated_in_chrome, translated_on,
    translated_with,
};
use wcontainers_translator_test::reference::Reference;
use wcontainers_translator_test::seeded;
use wcontainers_x86::X86_64;

/// How many draws a strategy's filters may throw away for each case before the run gives up.
const REJECTS_PER_CASE: u32 = 100;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// The random generator for a form's cases, from the seed and the form's name.
fn generator(seed: u64, name: &str, config: Config) -> TestRunner {
    TestRunner::new_with_rng(config, seeded::generator(seed, &[], name))
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
    let mut known = Known::default();
    for ((name, case), outcome) in sample.iter().zip(outcomes) {
        match compare_outcome::<G>(&mut reference, case, outcome) {
            Err(error) => {
                eprintln!("{name} in Chrome: {error}");
                failures.push(name.clone());
            }
            Ok(agreement) => known.add(name, agreement),
        }
    }
    eprintln!(
        "layer 1 in Chrome: {} cases ({count} a form), seed {seed}, {} failing: {failures:?}",
        sample.len(),
        failures.len()
    );
    known.report("layer 1 in Chrome");
    assert!(failures.is_empty());
}

/// The known failures of the reference met, counted by form and reason.
#[derive(Default)]
struct Known(BTreeMap<(String, &'static str), u64>);

impl Known {
    fn add(&mut self, form: &str, agreement: Agreement) {
        if let Agreement::KnownFailure(reason) = agreement {
            *self.0.entry((form.to_string(), reason)).or_default() += 1;
        }
    }

    /// One line for each form and reason, and the total.
    fn report(&self, what: &str) {
        for ((form, reason), n) in &self.0 {
            eprintln!("{what}: {form}: {n} known failures: {reason}");
        }
        eprintln!(
            "{what}: {} known failures in all",
            self.0.values().sum::<u64>()
        );
    }
}

/// Runs `count` cases of each form chosen by `WC_LAYER1_FORMS`, `WC_LAYER1_JOBS` forms at once, each case
/// through `check` with a reference runner of its own; prints the summary, its first words `layer 1` and
/// `list`, and the known failures met, and returns the failing forms' names, sorted.
fn run_forms<G: GuestTest>(
    forms: &[G::Form],
    count: u32,
    list: &str,
    check: impl Fn(&mut Reference, &G::Form, &Case) -> Result<Agreement, String> + Sync,
) -> Vec<String> {
    let seed = setting("WC_LAYER1_SEED", 1);
    let jobs = setting("WC_LAYER1_JOBS", 1).max(1);
    let only: Option<Vec<String>> = std::env::var("WC_LAYER1_FORMS")
        .ok()
        .map(|list| list.split(',').map(String::from).collect());
    let chosen: Vec<_> = forms
        .iter()
        .filter(|f| {
            only.as_ref()
                .is_none_or(|names| names.iter().any(|n| n == G::form_name(f)))
        })
        .collect();
    let failures = Mutex::new(Vec::new());
    let known = Mutex::new(Known::default());
    // Reference runners not in use; a form takes one, or starts one when none is free.
    let free = Mutex::new(Vec::new());
    at_once(jobs, chosen.len() as u64, |i| {
        let form = chosen[i as usize];
        let name = G::form_name(form);
        let taken = free.lock().unwrap().pop();
        let reference = RefCell::new(taken.unwrap_or_else(|| Reference::start(&G::REFERENCE)));
        let mut config = Config::with_cases(count);
        config.failure_persistence = None;
        config.max_global_rejects = REJECTS_PER_CASE * count;
        let mut runner = generator(seed, name, config);
        let result = runner.run(&G::cases(form), |case| {
            let agreement =
                check(&mut reference.borrow_mut(), form, &case).map_err(TestCaseError::fail)?;
            known.lock().unwrap().add(name, agreement);
            Ok(())
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
    let mut failures: Vec<String> = failing.iter().map(|(_, name)| name.clone()).collect();
    failures.sort();
    eprintln!(
        "layer 1{list}: {} forms, {count} cases each, seed {seed}, {jobs} at once, {} failing: {failures:?}",
        chosen.len(),
        failures.len()
    );
    if !groups.is_empty() {
        eprintln!("layer 1 failing by group: {groups:?}");
    }
    known
        .into_inner()
        .unwrap()
        .report(&format!("layer 1{list}"));
    failures
}

fn every_form_matches_the_cpu<G: GuestTest>() {
    let count = setting("WC_LAYER1_CASES", 64) as u32;
    let failures = run_forms::<G>(G::forms(), count, "", |reference, _, case| {
        compare::<G>(reference, case)
    });
    assert!(
        failures.is_empty(),
        "{} forms differ from the CPU",
        failures.len()
    );
}

/// The forms of features not offered stop ours as illegal instruction; where the reference lacks the
/// feature too it must stop the same, else what it did is recorded. `WC_LAYER1_UNOFFERED_CASES` cases a form
/// (default 64), with the seed and the choice of forms as for the default forms.
fn unoffered_forms_are_illegal<G: GuestTest>() {
    let count = setting("WC_LAYER1_UNOFFERED_CASES", 64) as u32;
    // What the reference did with the forms it has: each stop's kind, counted, and the forms it too refused.
    let recorded = Mutex::new(BTreeMap::<String, u64>::new());
    let refused_there = Mutex::new(BTreeMap::<String, u64>::new());
    let failures = run_forms::<G>(
        G::unoffered_forms(),
        count,
        " of the unoffered forms",
        |reference, form, case| {
            let actual = translated_with::<G>(case, |m| m.set_offer(Default::default()));
            if !G::reference_has(form) {
                return compare_outcome::<G>(reference, case, actual);
            }
            if actual.stop != Stop::Illegal {
                return Err(format!(
                    "code {:02x?} ({}): translated {:?}, not illegal instruction",
                    case.code.code(),
                    G::disassemble(case.code.code()),
                    actual.stop
                ));
            }
            let stop = on_reference::<G>(reference, case).stop;
            let kind = format!("{stop:?}");
            let kind = kind.split('(').next().unwrap_or_default().to_string();
            *recorded.lock().unwrap().entry(kind).or_default() += 1;
            if stop == Stop::Illegal {
                *refused_there
                    .lock()
                    .unwrap()
                    .entry(G::form_name(form).to_string())
                    .or_default() += 1;
            }
            Ok(Agreement::Same)
        },
    );
    eprintln!(
        "layer 1, the unoffered forms the reference has: its stops {:?}; forms it refused too, with cases: {:?}",
        recorded.into_inner().unwrap(),
        refused_there.into_inner().unwrap()
    );
    assert!(
        failures.is_empty(),
        "{} unoffered forms are not illegal as they must be",
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
            let agreement = compare::<G>(&mut reference, &designed.case)
                .unwrap_or_else(|report| panic!("{name}: {report}"));
            assert_eq!(agreement, Agreement::Same, "{name}");
        }
        let (outcome, machine) = translated_on::<G>(&designed.case, |_| {});
        if let Some(tracking) = designed.tracking {
            assert_eq!(
                G::tracked(machine.stopped_state()),
                tracking,
                "{name}: whether the float flags are kept"
            );
        }
        for &(reason, count) in &designed.counted {
            assert_eq!(machine.count_named(reason), count, "{name}: {reason}");
        }
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
    let mut config = Config::with_cases(count);
    config.failure_persistence = None;
    config.max_local_rejects = REJECTS_PER_CASE * count;
    let mut runner = TestRunner::new_with_rng(config, seeded::generator(seed, b"no form ", ""));
    let reference = RefCell::new(Reference::start(&G::REFERENCE));
    let known = RefCell::new(Known::default());
    let result = runner.run(&G::no_form(seed), |case| {
        let agreement =
            compare::<G>(&mut reference.borrow_mut(), &case).map_err(TestCaseError::fail)?;
        known
            .borrow_mut()
            .add(&G::disassemble(case.code.code()), agreement);
        Ok(())
    });
    eprintln!("layer 1: {count} cases in no form, seed {seed}");
    known.borrow().report("layer 1, no form");
    result.unwrap();
}

/// What the CPU offers is what a program is told it has.
fn offered_features_are_the_ones_programs_are_told<G: GuestTest>() {
    let (mut offered, mut told) = (G::offered(), G::told());
    offered.sort();
    told.sort();
    assert_eq!(offered, told);
}

/// A request the runner cannot read fails the run with the runner's own message, not as a lost connection.
#[test]
fn the_reference_reports_its_own_failure_arm64() {
    Reference::check_runner_failure(&Arm64::REFERENCE);
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

#[test]
fn offered_features_are_the_ones_programs_are_told_x86_64() {
    offered_features_are_the_ones_programs_are_told::<X86_64>();
}

/// `rdtsc` and `rdtscp` give the clock's nanoseconds as edx:eax with the upper halves cleared, and `rdtscp`
/// ecx 0, the processor (plan 5.10): answered by design, so checked here and not against the CPU.
#[test]
fn the_time_stamp_is_the_clock_x86_64() {
    use wcontainers_translator_test::guest::x86_64::plain;
    use wcontainers_translator_test::layout::CODE;
    use wcontainers_x86::{RAX, RCX, RDX};
    const RDTSC: &[u8] = &[0x0f, 0x31];
    const RDTSCP: &[u8] = &[0x0f, 0x01, 0xf9];
    // Past 2^32 nanoseconds, so that the high half is not zero.
    const CLOCK: u64 = 3 << u32::BITS;
    let [rax, rcx, rdx] = [RAX, RCX, RDX].map(usize::from);
    for (code, processor) in [(RDTSC, false), (RDTSCP, true)] {
        let mut case = plain(code);
        for field in [rax, rcx, rdx] {
            case.state.0[field] = u64::MAX.into();
        }
        let before = std::time::Instant::now();
        let outcome = translated_with::<X86_64>(&case, |machine| machine.set_clock(CLOCK));
        let ran = before.elapsed().as_nanos() as u64;
        assert_eq!(
            outcome.stop,
            Stop::Landed(CODE + code.len() as u64),
            "{code:02x?}"
        );
        let [low, high, processor_number] =
            [rax, rdx, rcx].map(|field| outcome.state.0[field] as u64);
        assert!(
            low <= u32::MAX.into() && high <= u32::MAX.into(),
            "{code:02x?}"
        );
        let ticks = high << u32::BITS | low;
        assert!(
            (CLOCK..=CLOCK + ran).contains(&ticks),
            "{code:02x?}: {ticks:#x}"
        );
        assert_eq!(
            processor_number,
            if processor { 0 } else { u64::MAX },
            "{code:02x?}"
        );
    }
}

/// Every default x86-64 code and every code of a told feature beyond the default list has cases whose meaning is
/// written (or is the fault Linux gives, for the kernel's instructions), but those left out by design and those
/// the translator decodes as another code.
#[test]
fn every_default_code_is_drawn_and_written_x86_64() {
    use wcontainers_translator_test::guest::x86_64::{
        NOT_DRAWN, default_left_out, unwritten_forms,
    };
    let left = default_left_out();
    for (code, why) in &left {
        eprintln!("{code}: {why}");
    }
    let undrawn: Vec<_> = left.iter().filter(|(_, why)| *why == NOT_DRAWN).collect();
    assert!(
        undrawn.is_empty(),
        "{} default codes have no case: {undrawn:?}",
        undrawn.len()
    );
    let unwritten = unwritten_forms();
    assert!(
        unwritten.is_empty(),
        "{} default forms have no meaning: {unwritten:?}",
        unwritten.len()
    );
}

#[test]
fn every_form_matches_the_cpu_x86_64() {
    every_form_matches_the_cpu::<X86_64>();
}

/// The native output for a host process (docs/x86-native-output.md section 8): every form's cases translated,
/// run on this machine and compared with its CPU, with the cases counted by how they ended. Linux x86-64 only,
/// where the reference is this machine.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn every_form_matches_the_cpu_natively_x86_64() {
    use wcontainers_translator_test::guest::x86_64::native::{
        ANSWERS_CHECKED, REFERENCE_HOST, compare,
    };
    let count = setting("WC_LAYER1_CASES", 64) as u32;
    let ended = Mutex::new(BTreeMap::<String, u64>::new());
    let failures =
        run_forms::<X86_64>(X86_64::forms(), count, " natively", |reference, _, case| {
            let (agreement, how) = compare(reference, case, REFERENCE_HOST)?;
            *ended.lock().unwrap().entry(format!("{how:?}")).or_default() += 1;
            Ok(agreement)
        });
    eprintln!(
        "layer 1 natively, the cases by how they ended: {:?}; the inside's answer equal to the meaning's in {}",
        ended.into_inner().unwrap(),
        ANSWERS_CHECKED.load(std::sync::atomic::Ordering::Relaxed)
    );
    assert!(
        failures.is_empty(),
        "{} forms differ from the CPU natively",
        failures.len()
    );
}

/// The inside's answer to `cpuid` and `xgetbv` (docs/x86-native-output.md 3.5) is the meaning's, for every leaf
/// with each sub-leaf of its table and past it, for leaves above the highest, and for each register `xgetbv` names;
/// the upper halves of the registers read are set, as the instructions read only the lower ones.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_inside_answers_cpuid_and_xgetbv_as_the_meaning_x86_64() {
    use wcontainers_translator_test::guest::x86_64::{native::answered, plain};
    use wcontainers_translator_test::layer1::translated;
    use wcontainers_x86::cpuid::{EXTENDED, MAX_BASIC, MAX_EXTENDED, leaves, sub_leaves};
    use wcontainers_x86::{RAX, RCX};
    const CPUID: &[u8] = &[0x0f, 0xa2];
    const XGETBV: &[u8] = &[0x0f, 0x01, 0xd0];
    const UPPER: u64 = u64::MAX << u32::BITS;
    let [rax, rcx] = [RAX, RCX].map(usize::from);
    let past = [MAX_BASIC + 1, EXTENDED - 1, MAX_EXTENDED + 1, u32::MAX];
    let mut checked = 0;
    let mut check = |code: &[u8], eax: u32, ecx: u32| {
        let mut case = plain(code);
        case.state.0[rax] = (UPPER | u64::from(eax)).into();
        case.state.0[rcx] = (UPPER | u64::from(ecx)).into();
        let answered = answered(&case).expect("an instruction the inside answers");
        let meaning = translated::<X86_64>(&case);
        assert!(
            answered == meaning,
            "{code:02x?} eax {eax:#x} ecx {ecx:#x}: the answer {:?} {:x?}, the meaning {:?} {:x?}",
            answered.stop,
            answered.state.0,
            meaning.stop,
            meaning.state.0
        );
        checked += 1;
    };
    for leaf in leaves().chain(past) {
        for sub_leaf in (0..=sub_leaves(leaf) + 1).chain([u32::from(u8::MAX) + 1, u32::MAX]) {
            check(CPUID, leaf, sub_leaf);
        }
    }
    for ecx in [0, 1, 2, u32::MAX] {
        check(XGETBV, 0, ecx);
    }
    eprintln!("the inside's cpuid and xgetbv equal to the meaning's in {checked} cases");
}

/// The inside's answers to the reciprocal estimates, `popf` and the descriptor-table instructions equal the
/// meaning's on the operands layer 1's random cases rarely reach: every exponent of both signs with the fractions
/// at the estimates' interval edges and not-a-numbers, every selector of Linux's table and its neighbours, every
/// rflags bit popped, and accesses that fault in the read-only and unmapped data or off the canonical range.
#[test]
fn the_inside_answers_reciprocals_flags_and_descriptors_as_the_meaning_x86_64() {
    use wcontainers_translator_test::guest::x86_64::{
        FS, GS, RFLAGS, RSP, XMM, YMMH, native::answered, plain,
    };
    use wcontainers_translator_test::layer1::translated;
    use wcontainers_translator_test::layout::{DATA, DATA_BYTES, DATA_WRITABLE};
    use wcontainers_x86::{DF, ID, NT, RCX, USER_DS, USER_END};
    let mut checked = 0;
    let mut check = |case: &Case| {
        let answered = answered(case).unwrap_or_else(|| {
            panic!(
                "{:02x?}: an instruction the inside answers",
                case.code.code()
            )
        });
        let meaning = translated::<X86_64>(case);
        assert!(
            answered == meaning,
            "{:02x?}: the answer {:?} {:x?} {:x?}, the meaning {:?} {:x?} {:x?}",
            case.code.code(),
            answered.stop,
            answered.state.0,
            answered.changed,
            meaning.stop,
            meaning.state.0,
            meaning.changed
        );
        checked += 1;
    };
    // rcpps, rsqrtps, rcpss, rsqrtss xmm0, xmm1; vrcpps, vrsqrtps ymm0, ymm1; vrcpss, vrsqrtss xmm0, xmm2, xmm1.
    let reciprocals: [&[u8]; 8] = [
        &[0x0f, 0x53, 0xc1],
        &[0x0f, 0x52, 0xc1],
        &[0xf3, 0x0f, 0x53, 0xc1],
        &[0xf3, 0x0f, 0x52, 0xc1],
        &[0xc5, 0xfc, 0x53, 0xc1],
        &[0xc5, 0xfc, 0x52, 0xc1],
        &[0xc5, 0xea, 0x53, 0xc1],
        &[0xc5, 0xea, 0x52, 0xc1],
    ];
    let fraction_bits = f32::MANTISSA_DIGITS - 1;
    let quiet = 1u32 << (fraction_bits - 1);
    let fractions = [
        0,
        1,
        (1 << 12) - 1,
        1 << 12,
        (1 << 13) - 1,
        quiet,
        quiet - 1,
        (1 << fraction_bits) - 1,
    ];
    let lanes: Vec<u32> = [0, 1u32 << (u32::BITS - 1)]
        .into_iter()
        .flat_map(|sign| {
            (0..=u32::from(u8::MAX)).flat_map(move |exponent| {
                fractions
                    .into_iter()
                    .map(move |f| sign | exponent << fraction_bits | f)
            })
        })
        .collect();
    let lane = |k: usize| u128::from(lanes[k % lanes.len()]);
    let four = |k: usize| (0..4).map(|n| lane(k + n) << (32 * n)).sum::<u128>();
    for code in reciprocals {
        for k in (0..lanes.len()).step_by(4) {
            let mut case = plain(code);
            for (n, field) in [XMM, XMM + 1, XMM + 2, YMMH, YMMH + 1, YMMH + 2]
                .into_iter()
                .enumerate()
            {
                case.state.0[field] = four(k + lanes.len() / 2 * (n % 2) + 3 * n);
            }
            check(&case);
        }
    }
    // popfq and popfw of every bit alone, none and all, from the writable stack, with DF, NT and ID clear and set
    // before; then from the read-only data, across the data's end, and with rsp off the canonical range.
    let stack = DATA + DATA_WRITABLE / 2;
    for code in [&[0x9d][..], &[0x66, 0x9d]] {
        let values = (0..u64::BITS).map(|b| 1u64 << b).chain([0, u64::MAX]);
        for (value, before) in values.flat_map(|v| [(v, 0), (v, DF | NT | ID)]) {
            let mut case = plain(code);
            case.state.0[RSP] = stack.into();
            case.state.0[RFLAGS] |= u128::from(before);
            case.placed = ((stack - DATA) as u32, value.to_le_bytes().to_vec());
            check(&case);
        }
        for rsp in [DATA + DATA_WRITABLE, DATA + DATA_BYTES - 1, USER_END, 0] {
            let mut case = plain(code);
            case.state.0[RSP] = rsp.into();
            check(&case);
        }
    }
    // lar, lsl (32, 64 and 16 bits), verr and verw of a selector in rcx, its upper bits set and clear.
    let selector_reads: [&[u8]; 8] = [
        &[0x0f, 0x02, 0xc1],
        &[0x48, 0x0f, 0x02, 0xc1],
        &[0x66, 0x0f, 0x02, 0xc1],
        &[0x0f, 0x03, 0xc1],
        &[0x48, 0x0f, 0x03, 0xc1],
        &[0x66, 0x0f, 0x03, 0xc1],
        &[0x0f, 0x00, 0xe1],
        &[0x0f, 0x00, 0xe9],
    ];
    for code in selector_reads {
        for selector in 0..=u64::from(u8::MAX) {
            for upper in [0, u64::MAX << u16::BITS] {
                let mut case = plain(code);
                case.state.0[usize::from(RCX)] = (upper | selector).into();
                case.state.0[0] = u128::from(u64::MAX);
                check(&case);
            }
        }
    }
    // The memory forms: sgdt, sidt, smsw, sldt and str to [rsp], lar eax from [rsp], and smsw, sldt and str to
    // eax, rax and ax.
    let memory_forms: [&[u8]; 6] = [
        &[0x0f, 0x01, 0x04, 0x24],
        &[0x0f, 0x01, 0x0c, 0x24],
        &[0x0f, 0x01, 0x24, 0x24],
        &[0x0f, 0x00, 0x04, 0x24],
        &[0x0f, 0x00, 0x0c, 0x24],
        &[0x0f, 0x02, 0x04, 0x24],
    ];
    for code in memory_forms {
        for rsp in [stack, DATA + DATA_WRITABLE, DATA + DATA_BYTES - 4, USER_END] {
            let mut case = plain(code);
            case.state.0[RSP] = rsp.into();
            case.placed = ((stack - DATA) as u32, USER_DS.to_le_bytes().to_vec());
            check(&case);
        }
    }
    for code in [[0x0f, 0x01, 0xe0], [0x0f, 0x00, 0xc0], [0x0f, 0x00, 0xc8]] {
        for prefix in [&[][..], &[0x48], &[0x66]] {
            let mut case = plain(&[prefix, &code].concat());
            case.state.0[0] = u128::from(u64::MAX);
            check(&case);
        }
    }
    // The segment registers: mov from each to eax, ax, rax and [rsp]; mov to each from cx and from [rsp], push and
    // pop of fs and gs (8 and 2 bytes), and lfs, lgs and lss (16, 32 and 64-bit offsets), each with every selector
    // of Linux's table and its neighbours, from the writable, read-only and unmapped data.
    let sreg = |n: u8| 0xc0 | n << 3;
    let mut segment_forms: Vec<Vec<u8>> = Vec::new();
    for n in 0..6 {
        for prefix in [&[][..], &[0x66], &[0x48]] {
            segment_forms.push([prefix, &[0x8c, sreg(n)]].concat());
        }
        segment_forms.push(vec![0x8c, 0x04 | n << 3, 0x24]);
        // A mov to cs is no instruction.
        if n != 1 {
            segment_forms.push(vec![0x8e, sreg(n) | 1]);
            segment_forms.push(vec![0x8e, 0x04 | n << 3, 0x24]);
        }
    }
    for code in [[0x0f, 0xa0], [0x0f, 0xa8], [0x0f, 0xa1], [0x0f, 0xa9]] {
        segment_forms.push(code.to_vec());
        segment_forms.push([&[0x66][..], &code].concat());
    }
    for op in [0xb4, 0xb5, 0xb2] {
        for prefix in [&[][..], &[0x66], &[0x48]] {
            segment_forms.push([prefix, &[0x0f, op, 0x04, 0x24]].concat());
        }
    }
    for code in &segment_forms {
        let selectors = (0..=0x80u16).chain([USER_DS | 0x100, u16::MAX]);
        for selector in selectors {
            let mut case = plain(code);
            case.state.0[RSP] = stack.into();
            case.state.0[usize::from(RCX)] = (u64::MAX << u16::BITS | u64::from(selector)).into();
            case.state.0[0] = u128::from(u64::MAX);
            // Bases a load clears.
            case.state.0[FS] = USER_END.into();
            case.state.0[GS] = (USER_END - 1).into();
            // The selector at every place a form reads one: [rsp], and after an offset of 2, 4 or 8 bytes.
            case.placed = ((stack - DATA) as u32, selector.to_le_bytes().repeat(5));
            check(&case);
        }
        for rsp in [DATA + DATA_WRITABLE, DATA + DATA_BYTES - 1, USER_END] {
            let mut case = plain(code);
            case.state.0[RSP] = rsp.into();
            check(&case);
        }
    }
    eprintln!(
        "the inside's reciprocals, popf, descriptor reads and segment registers equal to the meaning's in \
         {checked} cases"
    );
}

/// The inside's answer equals the meaning's for `case`.
fn answered_as_the_meaning(case: &Case) {
    use wcontainers_translator_test::guest::x86_64::native::answered;
    use wcontainers_translator_test::layer1::translated;
    let code = case.code.code();
    let answered =
        answered(case).unwrap_or_else(|| panic!("{code:02x?}: an instruction the inside answers"));
    let meaning = translated::<X86_64>(case);
    assert!(
        answered == meaning,
        "{code:02x?}: the answer {:?} {:x?} {:x?}, the meaning {:?} {:x?} {:x?}",
        answered.stop,
        answered.state.0,
        answered.changed,
        meaning.stop,
        meaning.state.0,
        meaning.changed
    );
}

/// The inside's answers to the far transfers (`retf`, `iret`, far `jmp` and `call`), `wrfsbase`, `wrgsbase` and
/// `lfs` through a wrapping 32-bit address equal the meaning's: every selector of Linux's code segments and its
/// neighbours, canonical and non-canonical targets and bases, every rflags bit `iret` loads, frames and pointers
/// that fault at each slot; and `rdtsc` and `rdtscp` give the clock as the meaning does.
#[test]
fn the_inside_answers_far_transfers_bases_and_the_clock_as_the_meaning_x86_64() {
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_translator_test::guest::x86_64::native::{ANSWER_CLOCK, answered};
    use wcontainers_translator_test::guest::x86_64::{FS, RFLAGS, RSP, plain};
    use wcontainers_translator_test::layer1::translated_with;
    use wcontainers_translator_test::layout::{CODE, DATA, DATA_BYTES, DATA_WRITABLE};
    use wcontainers_x86::{
        ALIGNMENT_CHECK, ALWAYS, NT, RAX, RCX, RDX, TF, USER_CS, USER_DS, USER_END, USER32_CS,
    };
    let mut checked = 0;
    let stack = DATA + DATA_WRITABLE / 2;
    let pointer = stack + 64;
    let selectors = [
        USER_CS,
        USER_CS & !3,
        USER_CS | 1 << u8::BITS,
        USER32_CS,
        USER32_CS & !3,
        USER_DS,
        0,
    ];
    let targets = [
        CODE + PAGE_SIZE,
        USER_END - 1,
        USER_END,
        !(USER_END - 1),
        !USER_END,
        u64::MAX,
    ];
    let slots = |values: &[u64], bytes: usize| -> Vec<u8> {
        values
            .iter()
            .flat_map(|v| v.to_le_bytes()[..bytes].to_vec())
            .collect()
    };
    let with_stack = |code: &[u8], rsp: u64, placed: Vec<u8>| {
        let mut case = plain(code);
        case.state.0[RSP] = rsp.into();
        case.state.0[usize::from(RCX)] = pointer.into();
        case.placed = ((rsp.min(pointer) - DATA) as u32, placed);
        case
    };
    // The stacks a frame or a call's slots fault at: the read-only data, its end, the data's start (a call's
    // return address below it), and off the canonical range.
    let faulting = |bytes: u64| {
        [
            DATA + DATA_WRITABLE,
            DATA + DATA_BYTES - 1,
            DATA + bytes,
            DATA + 2 * bytes - 1,
            USER_END,
        ]
    };
    // retf (32-bit, 64-bit and 16-bit slots), and retf 0x10.
    for (prefix, bytes) in [(&[][..], 4), (&[0x48][..], 8), (&[0x66][..], 2)] {
        for op in [&[0xcb][..], &[0xca, 0x10, 0x00]] {
            let code = [prefix, op].concat();
            for selector in selectors {
                for target in targets {
                    let frame = slots(&[target, selector.into()], bytes);
                    answered_as_the_meaning(&with_stack(&code, stack, frame));
                    checked += 1;
                }
            }
            for rsp in faulting(bytes as u64) {
                answered_as_the_meaning(&with_stack(&code, rsp, vec![]));
                checked += 1;
            }
        }
    }
    // iret with each rflags bit it loads, the ss of Linux's table and others, and NT set before.
    for (prefix, bytes) in [(&[][..], 4), (&[0x48][..], 8), (&[0x66][..], 2)] {
        let code = [prefix, &[0xcf]].concat();
        let flags = [ALWAYS, ALWAYS | TF, ALWAYS | ALIGNMENT_CHECK, u64::MAX, 0];
        for selector in selectors {
            for target in targets {
                for rflags in flags {
                    for ss in [USER_DS, USER_DS & !3, USER_CS, 0] {
                        let frame = slots(
                            &[target, selector.into(), rflags, pointer, ss.into()],
                            bytes,
                        );
                        let mut case = with_stack(&code, stack, frame);
                        if rflags == 0 {
                            case.state.0[RFLAGS] |= u128::from(NT);
                        }
                        answered_as_the_meaning(&case);
                        checked += 1;
                    }
                }
            }
        }
        for rsp in faulting(bytes as u64) {
            answered_as_the_meaning(&with_stack(&code, rsp, vec![]));
            checked += 1;
        }
    }
    // jmp far and call far [rcx] (32-bit, 64-bit and 16-bit offsets), from stacks that take the call's slots and
    // those that fault at either; with 0x67 and fs, a pointer whose selector's address wraps at 2^32.
    for (prefix, bytes) in [(&[][..], 4), (&[0x48][..], 8), (&[0x66][..], 2)] {
        for modrm in [0x29, 0x19] {
            let code = [prefix, &[0xff, modrm]].concat();
            for selector in selectors {
                for target in targets {
                    let mut placed = vec![0; (pointer - stack) as usize];
                    placed.extend(&target.to_le_bytes()[..bytes]);
                    placed.extend(selector.to_le_bytes());
                    answered_as_the_meaning(&with_stack(&code, stack, placed));
                    checked += 1;
                }
            }
            for rsp in faulting(bytes as u64) {
                let mut case = with_stack(&code, rsp, vec![]);
                let mut placed = (CODE + PAGE_SIZE).to_le_bytes()[..bytes].to_vec();
                placed.extend(USER_CS.to_le_bytes());
                case.placed = ((pointer - DATA) as u32, placed);
                answered_as_the_meaning(&case);
                checked += 1;
            }
            let wrapping = [&[0x64, 0x67][..], code.as_slice()].concat();
            let end = u64::from(u32::MAX) + 1 - bytes as u64;
            let mut case = with_stack(&wrapping, stack, vec![]);
            case.state.0[usize::from(RCX)] = end.into();
            case.state.0[FS] = pointer.wrapping_sub(end).into();
            let mut placed = (CODE + PAGE_SIZE).to_le_bytes()[..bytes].to_vec();
            placed.extend(USER_CS.to_le_bytes());
            case.placed = ((pointer - DATA) as u32, placed);
            answered_as_the_meaning(&case);
            checked += 1;
        }
    }
    // lfs eax, fs:[ecx] whose selector's address wraps at 2^32, then the same with the fs base so the selector
    // is read where the wrap puts it.
    for (prefix, bytes) in [(&[][..], 4u64), (&[0x48][..], 8), (&[0x66][..], 2)] {
        let code = [&[0x64, 0x67][..], prefix, &[0x0f, 0xb4, 0x01]].concat();
        let end = u64::from(u32::MAX) + 1 - bytes;
        for base in [pointer.wrapping_sub(end), pointer] {
            let mut case = with_stack(&code, stack, vec![]);
            case.state.0[usize::from(RCX)] = end.into();
            case.state.0[FS] = base.into();
            case.placed = ((pointer - DATA) as u32, USER_DS.to_le_bytes().repeat(8));
            answered_as_the_meaning(&case);
            checked += 1;
        }
    }
    // wrfsbase and wrgsbase of ecx and rcx, canonical and not, the upper half set for the 32-bit forms.
    for code in [
        &[0xf3, 0x0f, 0xae, 0xd1][..],
        &[0xf3, 0x48, 0x0f, 0xae, 0xd1],
        &[0xf3, 0x0f, 0xae, 0xd9],
        &[0xf3, 0x48, 0x0f, 0xae, 0xd9],
    ] {
        for value in targets.into_iter().chain([0, 0x1234_5678_9abc_def0]) {
            let mut case = plain(code);
            case.state.0[usize::from(RCX)] = value.into();
            answered_as_the_meaning(&case);
            checked += 1;
        }
    }
    // rdtsc and rdtscp: the clock's reading in edx:eax; the rest, rcx's 0 for rdtscp too, as the meaning's.
    let [rax, rcx, rdx] = [RAX, RCX, RDX].map(usize::from);
    for code in [&[0x0f, 0x31][..], &[0x0f, 0x01, 0xf9]] {
        let mut case = plain(code);
        for field in [rax, rcx, rdx] {
            case.state.0[field] = u64::MAX.into();
        }
        let mut answer = answered(&case).expect("an instruction the inside answers");
        let mut meaning =
            translated_with::<X86_64>(&case, |machine| machine.set_clock(ANSWER_CLOCK));
        let ticks = |state: &[u128]| (state[rdx] as u64) << u32::BITS | state[rax] as u64;
        assert_eq!(ticks(&answer.state.0), ANSWER_CLOCK, "{code:02x?}");
        assert!(ticks(&meaning.state.0) >= ANSWER_CLOCK, "{code:02x?}");
        for field in [rax, rdx] {
            answer.state.0[field] = 0;
            meaning.state.0[field] = 0;
        }
        assert!(
            answer == meaning,
            "{code:02x?}: the answer {:?} {:x?}, the meaning {:?} {:x?}",
            answer.stop,
            answer.state.0,
            meaning.stop,
            meaning.state.0
        );
        checked += 1;
    }
    eprintln!(
        "the inside's far transfers, bases, far pointer wraps and clock equal to the meaning's in {checked} cases"
    );
}

/// The inside's answers to `xsave` and `xrstor` (layer 1 leaves them out: XCR0 and XSTATE_BV are by design) equal
/// the meaning's: each RFBM, components in and out of their initial configuration, a TOP that rotates the
/// registers, areas whose XSTATE_BV, header and MXCSR `xrstor` takes or refuses, and areas misaligned, in the
/// read-only data, across the data's end, off the canonical range, and wrapping a 32-bit address under fs.
#[test]
fn the_inside_answers_xsave_and_xrstor_as_the_meaning_x86_64() {
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_translator_test::guest::x86_64::{FS, FSW, R0, XMM, YMMH, plain};
    use wcontainers_translator_test::layout::{DATA, DATA_BYTES, DATA_WRITABLE};
    use wcontainers_x86::cpuid::{XSAVE_AVX, XSAVE_SSE, XSAVE_X87, xsave_bytes};
    use wcontainers_x86::{Fxsave, MXCSR_AT_START, MXCSR_WRITABLE, RAX, RCX, USER_END};
    /// XSTATE_BV after the legacy area, the header's next words, MXCSR and the x87 header's reserved byte.
    const XSTATE_BV: usize = size_of::<Fxsave>();
    const XCOMP_BV: usize = XSTATE_BV + size_of::<u64>();
    const MXCSR: usize = std::mem::offset_of!(Fxsave, mxcsr);
    const RESERVED: usize = std::mem::offset_of!(Fxsave, reserved0);
    const AREA: usize = xsave_bytes(XSAVE_X87 | XSAVE_SSE | XSAVE_AVX) as usize;
    let mut checked = 0;
    // Half an area, keeping its alignment.
    let legacy = XSTATE_BV as u64;
    let area_at = DATA + PAGE_SIZE;
    let mut seed = 0x5eed_u64;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        seed
    };
    let forms: [&[u8]; 4] = [
        &[0x0f, 0xae, 0x21],
        &[0x48, 0x0f, 0xae, 0x21],
        &[0x0f, 0xae, 0x29],
        &[0x48, 0x0f, 0xae, 0x29],
    ];
    let rfbms = [0, 1, 2, 4, 3, 6, 7, u64::MAX];
    for code in forms {
        for rfbm in rfbms {
            // The x87 status word (TOP 5) and registers, the xmm registers, the upper halves.
            for (status, registers, vectors, highs) in [
                (false, false, false, false),
                (true, true, true, true),
                (true, true, false, true),
                (false, true, false, false),
                (false, false, true, false),
            ] {
                for (bv, header, mxcsr, reserved) in [
                    (7u64, 0u64, MXCSR_AT_START, 0u8),
                    (0, 0, MXCSR_AT_START, u8::MAX),
                    (1, 0, MXCSR_AT_START, 0),
                    (6, 0, MXCSR_WRITABLE, 0),
                    (7, 0, MXCSR_AT_START | !MXCSR_WRITABLE, 0),
                    (0xf, 0, MXCSR_AT_START, 0),
                    (7, 1, MXCSR_AT_START, 0),
                ] {
                    let mut case = plain(code);
                    case.state.0[usize::from(RCX)] = area_at.into();
                    case.state.0[usize::from(RAX)] = rfbm.into();
                    if status {
                        case.state.0[FSW] = (5 << 11) | 0x41;
                    }
                    if registers {
                        for r in 0..8 {
                            case.state.0[R0 + r] =
                                u128::from(next()) | u128::from(next() as u16) << 64;
                        }
                    }
                    for r in 0..16 {
                        if vectors {
                            case.state.0[XMM + r] = u128::from(next()) << 64 | u128::from(next());
                        }
                        if highs {
                            case.state.0[YMMH + r] = u128::from(next()) << 64 | u128::from(next());
                        }
                    }
                    let mut area: Vec<u8> =
                        (0..AREA / 8).flat_map(|_| next().to_le_bytes()).collect();
                    area[XSTATE_BV..][..8].copy_from_slice(&bv.to_le_bytes());
                    area[XCOMP_BV..][..24].fill(0);
                    area[XCOMP_BV..][..8].copy_from_slice(&header.to_le_bytes());
                    area[MXCSR..][..4].copy_from_slice(&mxcsr.to_le_bytes());
                    area[RESERVED] = reserved;
                    case.placed = ((area_at - DATA) as u32, area);
                    answered_as_the_meaning(&case);
                    checked += 1;
                }
            }
        }
        // Misaligned, across the writable data's end, the data's end, the canonical range's end, off it.
        for at in [
            area_at + 16,
            DATA + DATA_WRITABLE - legacy,
            DATA + DATA_BYTES - legacy,
            USER_END - legacy,
            USER_END,
        ] {
            let mut case = plain(code);
            case.state.0[usize::from(RCX)] = at.into();
            case.state.0[usize::from(RAX)] = u64::MAX.into();
            answered_as_the_meaning(&case);
            checked += 1;
        }
        // fs:[ecx] whose last byte wraps at 2^32 to below the fs base, unmapped, and one that does not wrap.
        let wrapping = [&[0x64, 0x67][..], code].concat();
        for (ecx, base) in [
            (0xffff_ff00u64, area_at.wrapping_sub(0xffff_ff00)),
            (0, area_at),
        ] {
            let mut case = plain(&wrapping);
            case.state.0[usize::from(RCX)] = ecx.into();
            case.state.0[FS] = base.into();
            case.state.0[usize::from(RAX)] = 7u64.into();
            answered_as_the_meaning(&case);
            checked += 1;
        }
    }
    eprintln!("the inside's xsave and xrstor equal to the meaning's in {checked} cases");
}

/// `Condition::holds`, by which the inside takes a `jcc` exit's `int3` hit (docs/x86-native-output.md 3.2), is the
/// meaning's: `setcc al` under WebAssembly for each of the sixteen conditions on every combination of the five
/// flags conditions read.
#[test]
fn a_condition_holds_as_the_meaning_says_x86_64() {
    use wcontainers_translator_test::guest::x86_64::{RFLAGS, plain};
    use wcontainers_translator_test::layer1::translated;
    use wcontainers_x86::{CF, Condition, OF, PF, RAX, SF, ZF};
    /// `setcc al`: the two-byte opcode 0x0f 0x90 plus the condition's code, and ModRM naming al.
    const SETCC: [u8; 2] = [0x0f, 0x90];
    const AL: u8 = 0xc0;
    let flags = [CF, PF, ZF, SF, OF];
    let rax = usize::from(RAX);
    let mut checked = 0;
    for combination in 0..1u32 << flags.len() {
        let rflags: u64 = flags
            .iter()
            .enumerate()
            .filter(|(k, _)| combination & 1 << k != 0)
            .map(|(_, f)| f)
            .sum();
        for (code, condition) in Condition::ALL.into_iter().enumerate() {
            let bytes = [SETCC[0], SETCC[1] + code as u8, AL];
            let mut case = plain(&bytes);
            case.state.0[RFLAGS] |= u128::from(rflags);
            let outcome = translated::<X86_64>(&case);
            let set = outcome.state.0[rax] & u128::from(u8::MAX);
            assert_eq!(
                set != 0,
                condition.holds(case.state.0[RFLAGS] as u64),
                "{condition:?} with {rflags:#x}: {:?}",
                outcome.stop
            );
            checked += 1;
        }
    }
    eprintln!("a condition holds as the meaning says in {checked} cases");
}

#[test]
fn unoffered_forms_are_illegal_x86_64() {
    unoffered_forms_are_illegal::<X86_64>();
}

#[test]
fn bytes_in_no_form_stop_as_on_the_cpu_x86_64() {
    bytes_in_no_form_stop_as_on_the_cpu::<X86_64>();
}

#[test]
fn sample_matches_the_cpu_in_chrome_x86_64() {
    sample_matches_the_cpu_in_chrome::<X86_64>();
}

#[test]
fn hand_cases_do_what_they_must_x86_64() {
    hand_cases_do_what_they_must::<X86_64>();
}

/// Every x86-64 code of a feature not offered has cases, but those left out by design and those the
/// translator decodes as another code, which are in that code's list.
#[test]
fn every_unoffered_code_is_drawn_x86_64() {
    use wcontainers_translator_test::guest::x86_64::{NOT_DRAWN, unoffered_left_out};
    let left = unoffered_left_out();
    for (code, why) in &left {
        eprintln!("{code}: {why}");
    }
    let undrawn: Vec<_> = left.iter().filter(|(_, why)| *why == NOT_DRAWN).collect();
    assert!(
        undrawn.is_empty(),
        "{} unoffered codes have no case: {undrawn:?}",
        undrawn.len()
    );
}
