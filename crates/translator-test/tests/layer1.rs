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
use wcontainers_translator_test::jobs::at_once;
use wcontainers_translator_test::reference::Reference;
use wcontainers_translator_test::seeded;

/// How many draws a strategy's filters may throw away for each case before the run gives up.
const REJECTS_PER_CASE: u32 = 100;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// The random generator for a form's cases, from the seed and the form's name.
fn generator(seed: u64, name: &str, config: Config) -> TestRunner {
    TestRunner::new_with_rng(config, seeded::generator(seed, &[], name))
}

/// A request the runner cannot read fails the run with the runner's own message, not as a lost connection.
#[test]
fn the_reference_reports_its_own_failure_arm64() {
    Reference::check_runner_failure(&wcontainers_translator_test::arm64::part::REFERENCE);
}

#[test]
fn offered_features_are_the_ones_programs_are_told_arm64() {
    arm64::offered_features_are_the_ones_programs_are_told();
}

#[test]
fn every_form_matches_the_cpu_arm64() {
    arm64::every_form_matches_the_cpu();
}

#[test]
fn sample_matches_the_cpu_in_chrome_arm64() {
    arm64::sample_matches_the_cpu_in_chrome();
}

#[test]
fn hand_cases_do_what_they_must_arm64() {
    arm64::hand_cases_do_what_they_must();
}

#[test]
fn bytes_in_no_form_stop_as_on_the_cpu_arm64() {
    arm64::bytes_in_no_form_stop_as_on_the_cpu();
}

#[test]
fn offered_features_are_the_ones_programs_are_told_x86_64() {
    x86_64::offered_features_are_the_ones_programs_are_told();
}

/// `rdtsc` and `rdtscp` give the clock's nanoseconds as edx:eax with the upper halves cleared, and `rdtscp`
/// ecx 0, the processor (plan 5.10): answered by design, so checked here and not against the CPU.
#[test]
fn the_time_stamp_is_the_clock_x86_64() {
    use wcontainers_translator_test::layout::CODE;
    use wcontainers_translator_test::x86_64::part::plain;
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
        let outcome = x86_64::translated_with(&case, |machine| machine.set_clock(CLOCK));
        let ran = before.elapsed().as_nanos() as u64;
        assert_eq!(
            outcome.stop,
            x86_64::Stop::Landed(CODE + code.len() as u64),
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
    use wcontainers_translator_test::x86_64::part::{NOT_DRAWN, default_left_out, unwritten_forms};
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
    x86_64::every_form_matches_the_cpu();
}

/// The native output for a host process (docs/x86-native-output.md section 8): every form's cases translated,
/// run on this machine and compared with its CPU, with the cases counted by how they ended. Linux x86-64 only,
/// where the reference is this machine.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn every_form_matches_the_cpu_natively_x86_64() {
    use wcontainers_translator_test::x86_64::part::native::{
        ANSWERS_CHECKED, FAR_UNLIKE_MEANING, REFERENCE_HOST, SECOND_CALLS, STOPS_CHECKED,
        STRINGS_RUN, TRAPS_CHECKED, compare,
    };
    let count = setting("WC_LAYER1_CASES", 64) as u32;
    let ended = Mutex::new(BTreeMap::<String, u64>::new());
    let failures = x86_64::run_forms(
        wcontainers_translator_test::x86_64::part::forms(),
        count,
        " natively",
        |reference, _, case| {
            let (agreement, how) = compare(reference, case, REFERENCE_HOST)?;
            *ended.lock().unwrap().entry(format!("{how:?}")).or_default() += 1;
            Ok(agreement)
        },
    );
    eprintln!(
        "layer 1 natively, the cases by how they ended: {:?}; the inside's answer equal to the meaning's in {}; \
         the float status route's second call made in {}; a repeated string under the count calling the inside \
         correctly (its outcome the CPU's own) in {}; the inside's trap the meaning's in {}; a far transfer answered unlike the meaning (and \
         judged against the CPU) in {}; a replay check stopping with the guest as it started in {}",
        ended.into_inner().unwrap(),
        ANSWERS_CHECKED.load(std::sync::atomic::Ordering::Relaxed),
        SECOND_CALLS.load(std::sync::atomic::Ordering::Relaxed),
        STRINGS_RUN.load(std::sync::atomic::Ordering::Relaxed),
        TRAPS_CHECKED.load(std::sync::atomic::Ordering::Relaxed),
        FAR_UNLIKE_MEANING.load(std::sync::atomic::Ordering::Relaxed),
        STOPS_CHECKED.load(std::sync::atomic::Ordering::Relaxed)
    );
    assert!(
        failures.is_empty(),
        "{} forms differ from the CPU natively",
        failures.len()
    );
}

/// The in-process way's two checked forms (docs/x86-native-output.md sections 4 and 8): every form's cases
/// translated in each, run on this machine with the case's memory in a window with its guard pages and the probe's
/// page, and compared with its CPU, uncounted and counted with a stop after every host instruction. `WC_LAYER1_CHECK`
/// takes one form (`wrapping` or `exact`), both otherwise.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn every_form_matches_the_cpu_in_process_x86_64() {
    use std::sync::atomic::Ordering::Relaxed;
    use wcontainers_translator_test::x86_64::part::native::{
        REFERENCE_HOST, REFUSED, RSP_INTO_WINDOW, compare_in_process,
    };
    use wcontainers_x86::native::Check;
    let count = setting("WC_LAYER1_CASES", 64) as u32;
    let only = std::env::var("WC_LAYER1_CHECK").ok();
    let mut differing = 0;
    for (name, check) in [("wrapping", Check::Wrapping), ("exact", Check::Exact)] {
        if only.as_deref().is_some_and(|only| only != name) {
            continue;
        }
        let (refused, into_window) = (REFUSED.load(Relaxed), RSP_INTO_WINDOW.load(Relaxed));
        let ended = Mutex::new(BTreeMap::<String, u64>::new());
        let failures = x86_64::run_forms(
            wcontainers_translator_test::x86_64::part::forms(),
            count,
            " in process",
            |reference, _, case| {
                let (agreement, how) = compare_in_process(reference, case, REFERENCE_HOST, check)?;
                *ended.lock().unwrap().entry(format!("{how:?}")).or_default() += 1;
                Ok(agreement)
            },
        );
        eprintln!(
            "layer 1 in process, the {name} form, the cases by how they ended: {:?}; the probe refusing an \
             address and the meaning answering in {} runs; a case's rsp past the window taken modulo 2^32 in {} \
             runs; {} forms differ",
            ended.into_inner().unwrap(),
            REFUSED.load(Relaxed) - refused,
            RSP_INTO_WINDOW.load(Relaxed) - into_window,
            failures.len()
        );
        differing += failures.len();
    }
    assert_eq!(differing, 0, "forms differ from the CPU in process");
}

/// The in-process way's cases of docs/x86-native-output.md section 7 in both checked forms against the CPU: an
/// access one byte past the data and across it, at 2^32 - 1 with 8 bytes and at 2^32, a far-reaching SIB form, an
/// `fs:` operand with a negative offset, strings with a large count upward and downward and with rsi or rdi at
/// 2^32, 2^32 + 2^35, 2^40 and -1, bit tests whose offset reaches below the data or past 2^32, a push at the
/// stack's bottom and at rsp 0, `call r`, `call m` and `pushf` with rsp at 2^32 and 0, `enter` with a deep level,
/// the guest's 0x67 forms (an operand, every string plain and repeated, a bit test, `xlat`, `fs:` and `gs:` with
/// a base past 2^32), every bit test with the plan's offsets, `fxsave64` across 2^32 with and without 0x67, and the
/// masked stores. Every case ends as the CPU's, or as a named known difference (the wrapping form's landing modulo
/// 2^32 among them).
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_window_cases_of_the_plan_match_the_cpu_in_process_x86_64() {
    use wcontainers_translator_test::layout::{CODE, DATA, DATA_BYTES, DATA_WRITABLE};
    use wcontainers_translator_test::x86_64::part::native::{REFERENCE_HOST, compare_in_process};
    use wcontainers_translator_test::x86_64::part::{FS, GS, RFLAGS, RSP, plain};
    use wcontainers_x86::native::Check;
    use wcontainers_x86::{DF, RAX, RBX, RCX, RDI, RSI};
    const WINDOW: u64 = 1 << u32::BITS;
    let r = |n: u8| usize::from(n);
    let with = |code: &[u8], registers: &[(usize, u64)]| {
        let mut case = plain(code);
        for &(field, value) in registers {
            case.state.0[field] = value.into();
        }
        case
    };
    let end = DATA + DATA_BYTES;
    let starts = [WINDOW, WINDOW + (1 << 35), 1 << 40, u64::MAX];
    let mut cases = Vec::new();
    // mov rax, [rbx]: past the data, across its end, across 2^32, at 2^32.
    for at in [end, end - 1, WINDOW - 1, WINDOW, DATA] {
        cases.push(with(&[0x48, 0x8b, 0x03], &[(r(RBX), at)]));
    }
    // mov rax, [rbx + rcx*8 + 10h]: an index that wraps the sum back into the data, and one that reaches 2^43 past it.
    for index in [1 << 61, 1 << 40, (1 << 61) - 1] {
        cases.push(with(
            &[0x48, 0x8b, 0x44, 0xcb, 0x10],
            &[(r(RBX), DATA), (r(RCX), index)],
        ));
    }
    // mov rax, fs:[rbx] with a negative offset from the base.
    for (base, offset) in [
        (DATA + 0x100, -0x10i64),
        (DATA, -0x10),
        (WINDOW + 0x100, -0x200),
    ] {
        cases.push(with(
            &[0x64, 0x48, 0x8b, 0x03],
            &[(FS, base), (r(RBX), offset as u64)],
        ));
    }
    // rep movsb and rep stosb with a large count, upward and (DF set) downward, from inside the data.
    let down = |mut case: x86_64::Case| {
        case.state.0[RFLAGS] |= u128::from(DF);
        case
    };
    for count in [1 << 20, 0x1800] {
        cases.push(with(
            &[0xf3, 0xa4],
            &[(r(RSI), DATA), (r(RDI), DATA + 0x800), (r(RCX), count)],
        ));
        cases.push(down(with(
            &[0xf3, 0xa4],
            &[
                (r(RSI), DATA + 0x800),
                (r(RDI), DATA + 0x1000),
                (r(RCX), count),
            ],
        )));
        cases.push(with(
            &[0xf3, 0xaa],
            &[(r(RDI), DATA + 0x10), (r(RCX), count)],
        ));
        cases.push(down(with(
            &[0xf3, 0xaa],
            &[(r(RDI), DATA + 0x10), (r(RCX), count)],
        )));
        cases.push(down(with(
            &[0xf3, 0xaa],
            &[(r(RDI), 0x10), (r(RCX), count)],
        )));
    }
    // movsb, lodsb and stosb with a start at or past 2^32.
    for &at in &starts {
        cases.push(with(&[0xa4], &[(r(RSI), at), (r(RDI), DATA)]));
        cases.push(with(&[0xa4], &[(r(RSI), DATA), (r(RDI), at)]));
        cases.push(with(&[0xac], &[(r(RSI), at)]));
        cases.push(with(&[0xf3, 0xaa], &[(r(RDI), at), (r(RCX), 4)]));
    }
    // bt and bts [rax], rcx: below the data, past 2^32, the independent check's 0x200000005.
    for offset in [(-64i64) as u64, 8 << u32::BITS, 0x2_0000_0005, 5] {
        for code in [&[0x48, 0x0f, 0xa3, 0x08][..], &[0x48, 0x0f, 0xab, 0x08]] {
            cases.push(with(code, &[(r(RAX), DATA), (r(RCX), offset)]));
        }
    }
    // push rax at the data's bottom and at rsp 0; call rax with rsp at 2^32 and 0.
    for rsp in [DATA, 0, 8, WINDOW] {
        cases.push(with(&[0x50], &[(RSP, rsp)]));
        cases.push(with(&[0xff, 0xd0], &[(RSP, rsp), (r(RAX), CODE)]));
    }
    // rsp held as B plus the guest's: an operand one page below rsp 0 and 2^31 - 1 past its top, an index reaching
    // past the window, a write of rsp past it by `mov`, `sub`, `pop rsp` and `leave`, and `ret 0xffff` at the top.
    let rbp = usize::from(wcontainers_x86::RBP);
    cases.push(with(
        &[0x48, 0x8b, 0x84, 0x24, 0x00, 0xf0, 0xff, 0xff],
        &[(RSP, 0)],
    ));
    cases.push(with(
        &[0x48, 0x8b, 0x84, 0x24, 0xff, 0xff, 0xff, 0x7f],
        &[(RSP, WINDOW - 8)],
    ));
    cases.push(with(
        &[0x48, 0x8b, 0x04, 0xc4],
        &[(RSP, DATA), (r(RAX), 1 << 33)],
    ));
    cases.push(with(&[0x48, 0x89, 0xc4], &[(r(RAX), WINDOW + 16)]));
    cases.push(with(&[0x48, 0x89, 0xc4], &[(r(RAX), DATA + 8)]));
    cases.push(with(&[0x48, 0x83, 0xec, 0x10], &[(RSP, 8)]));
    cases.push(with(&[0x48, 0x83, 0xec, 0x10], &[(RSP, DATA + 64)]));
    cases.push(with(&[0x5c], &[(RSP, DATA)]));
    for frame in [DATA, WINDOW - 8, 1 << 40] {
        cases.push(with(&[0xc9], &[(rbp, frame)]));
    }
    cases.push(with(&[0xc2, 0xff, 0xff], &[(RSP, WINDOW - 8)]));
    // The guest's 0x67: mov rax, [ebx]; stosb; bt dword [eax], ecx; xlat.
    cases.push(with(
        &[0x67, 0x48, 0x8b, 0x03],
        &[(r(RBX), 0xffff_ffff_0000_0010)],
    ));
    cases.push(with(&[0x67, 0x48, 0x8b, 0x03], &[(r(RBX), WINDOW + DATA)]));
    cases.push(with(&[0x67, 0xaa], &[(r(RDI), WINDOW + DATA)]));
    cases.push(with(
        &[0x67, 0x0f, 0xa3, 0x08],
        &[(r(RAX), 0xffff_fff0), (r(RCX), 0x100)],
    ));
    cases.push(with(&[0x67, 0xd7], &[(r(RBX), WINDOW + DATA)]));
    // fxsave64 [rax] and, under 0x67, [eax], whose parts cross 2^32 (the ninth check's case), and in the data.
    for at in [0xffff_ffd6, DATA] {
        cases.push(with(&[0x48, 0x0f, 0xae, 0x00], &[(r(RAX), at)]));
        cases.push(with(&[0x67, 0x48, 0x0f, 0xae, 0x00], &[(r(RAX), at)]));
    }
    // maskmovq, maskmovdqu and vmaskmovdqu at rdi in the data and at each start past it.
    for &at in [DATA, DATA + (1 << 40)].iter().chain(&starts) {
        for code in [
            &[0x0f, 0xf7, 0xc1][..],
            &[0x66, 0x0f, 0xf7, 0xc1],
            &[0xc5, 0xf9, 0xf7, 0xc1],
        ] {
            cases.push(with(code, &[(r(RDI), at)]));
        }
    }
    // cmpsb, scasb and xlat with a start at or past 2^32.
    for &at in &starts {
        cases.push(with(&[0xa6], &[(r(RSI), at), (r(RDI), DATA)]));
        cases.push(with(&[0xa6], &[(r(RSI), DATA), (r(RDI), at)]));
        cases.push(with(&[0xae], &[(r(RDI), at)]));
        cases.push(with(&[0xd7], &[(r(RBX), at)]));
    }
    // The guest's 0x67 strings, plain and repeated, with edi and esi in the data and the upper halves set: the CPU
    // uses the low halves alone and leaves them zero-extended.
    let high = 0x1234_5678 << u32::BITS;
    for code in [
        &[0x67, 0xaa][..],
        &[0x67, 0xa4],
        &[0x67, 0xa6],
        &[0x67, 0xae],
        &[0x67, 0xac],
        &[0x67, 0xf3, 0xaa],
        &[0x67, 0xf3, 0xa4],
        &[0x67, 0xf3, 0xa6],
        &[0x67, 0xf2, 0xae],
        &[0x67, 0xf3, 0x48, 0xab],
    ] {
        cases.push(with(
            code,
            &[
                (r(RSI), high | (DATA + 0x800)),
                (r(RDI), high | (DATA + 0x10)),
                (r(RCX), (1 << u32::BITS) | 16),
            ],
        ));
    }
    // An `fs:` or `gs:` operand under 0x67 with a base at or past 2^32 (the meaning's fault in the exact form), with
    // one inside whose 32-bit offset wraps back into the data, and the plan's `65 67 8b 00`.
    for (field, prefix) in [(FS, 0x64), (GS, 0x65)] {
        for (base, offset) in [
            (WINDOW, 0x10),
            (WINDOW + DATA, 0),
            (DATA - 0x10, 0xffff_ffff_0000_0020),
            (DATA + 0x20, 0xffff_fff0),
        ] {
            cases.push(with(
                &[0x67, prefix, 0x48, 0x8b, 0x03],
                &[(field, base), (r(RBX), offset)],
            ));
        }
    }
    cases.push(with(
        &[0x65, 0x67, 0x8b, 0x00],
        &[(GS, 0x3_0000_0000), (r(RAX), 0x1_0000_0010)],
    ));
    // bt, bts, btr and btc (the last three also locked) of each operand size with a register bit offset of 0, -1, w,
    // -w - 1, 2^31 - 1, -2^31, 2^35, 2^60 and -2^63 (each taken at the operand size), from operands at the data's
    // first byte, its middle, the last of its writable bytes and the last of its read-only page.
    for (size, prefix) in [(16u64, &[0x66][..]), (32, &[]), (64, &[0x48])] {
        for op in [0xa3u8, 0xab, 0xb3, 0xbb] {
            for lock in [false, true] {
                if lock && op == 0xa3 {
                    continue;
                }
                let mut code = Vec::new();
                if lock {
                    code.push(0xf0);
                }
                code.extend_from_slice(prefix);
                code.extend_from_slice(&[0x0f, op, 0x08]);
                for offset in [
                    0,
                    -1,
                    size as i64,
                    -(size as i64) - 1,
                    i64::from(i32::MAX),
                    i64::from(i32::MIN),
                    1 << 35,
                    1 << 60,
                    i64::MIN,
                ] {
                    let offset = (offset as u64) & (u64::MAX >> (64 - size));
                    for at in [
                        DATA,
                        DATA + DATA_WRITABLE / 2,
                        DATA + DATA_WRITABLE - size / 8,
                        end - size / 8,
                    ] {
                        cases.push(with(&code, &[(r(RAX), at), (r(RCX), offset)]));
                    }
                }
            }
        }
    }
    // enter 16, 31 (a deep level) near the stack's bottom; call [rbx] and pushf with rsp at 2^32, 0 and the data.
    for rsp in [DATA + 0x100, 0x40, WINDOW] {
        cases.push(with(
            &[0xc8, 0x10, 0x00, 0x1f],
            &[(RSP, rsp), (rbp, DATA + 0x800)],
        ));
    }
    for rsp in [WINDOW, 0, DATA + 0x100] {
        cases.push(with(&[0xff, 0x13], &[(RSP, rsp), (r(RBX), DATA)]));
        cases.push(with(&[0x9c], &[(RSP, rsp)]));
    }
    let mut reference = Reference::start(&wcontainers_translator_test::x86_64::part::REFERENCE);
    let mut failures = Vec::new();
    for (name, check) in [("wrapping", Check::Wrapping), ("exact", Check::Exact)] {
        let (mut same, mut known, mut how) = (0, BTreeMap::<&str, u64>::new(), BTreeMap::new());
        for case in &cases {
            match compare_in_process(&mut reference, case, REFERENCE_HOST, check) {
                Ok((agreement, ended)) => {
                    *how.entry(format!("{ended:?}")).or_insert(0) += 1;
                    match agreement {
                        x86_64::Agreement::Same => same += 1,
                        x86_64::Agreement::KnownFailure(why) => *known.entry(why).or_default() += 1,
                    }
                }
                Err(report) => failures.push(format!("{name} {:02x?}: {report}", case.code.code())),
            }
        }
        eprintln!(
            "the window's cases, the {name} form: {same} as the CPU, known differences {known:?}, by how they \
             ended {how:?}"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The inside's answer to `cpuid` and `xgetbv` (docs/x86-native-output.md 3.5) is the meaning's, for every leaf
/// with each sub-leaf of its table and past it, for leaves above the highest, and for each register `xgetbv` names;
/// the upper halves of the registers read are set, as the instructions read only the lower ones.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_inside_answers_cpuid_and_xgetbv_as_the_meaning_x86_64() {
    use wcontainers_translator_test::x86_64::layer1::translated;
    use wcontainers_translator_test::x86_64::part::{native::answered, plain};
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
        let meaning = translated(&case);
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
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_inside_answers_reciprocals_flags_and_descriptors_as_the_meaning_x86_64() {
    use wcontainers_translator_test::layout::{DATA, DATA_BYTES, DATA_WRITABLE};
    use wcontainers_translator_test::x86_64::layer1::translated;
    use wcontainers_translator_test::x86_64::part::{
        FS, GS, RFLAGS, RSP, XMM, YMMH, native::answered, plain,
    };
    use wcontainers_x86::{DF, ID, NT, RCX, USER_DS, USER_END};
    let mut checked = 0;
    let mut check = |case: &x86_64::Case| {
        let answered = answered(case).unwrap_or_else(|| {
            panic!(
                "{:02x?}: an instruction the inside answers",
                case.code.code()
            )
        });
        let meaning = translated(case);
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
        let selectors = (0..=0x80u16).chain([USER_DS | 1 << u8::BITS, u16::MAX]);
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
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn answered_as_the_meaning(case: &x86_64::Case) {
    use wcontainers_translator_test::x86_64::layer1::translated;
    use wcontainers_translator_test::x86_64::part::native::answered;
    let code = case.code.code();
    let answered =
        answered(case).unwrap_or_else(|| panic!("{code:02x?}: an instruction the inside answers"));
    let meaning = translated(case);
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
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_inside_answers_far_transfers_bases_and_the_clock_as_the_meaning_x86_64() {
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_translator_test::layout::{CODE, DATA, DATA_BYTES, DATA_WRITABLE};
    use wcontainers_translator_test::x86_64::layer1::translated_with;
    use wcontainers_translator_test::x86_64::part::native::{ANSWER_CLOCK, answered};
    use wcontainers_translator_test::x86_64::part::{FS, RFLAGS, RSP, plain};
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
        let mut meaning = translated_with(&case, |machine| machine.set_clock(ANSWER_CLOCK));
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
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_inside_answers_xsave_and_xrstor_as_the_meaning_x86_64() {
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_translator_test::layout::{DATA, DATA_BYTES, DATA_WRITABLE};
    use wcontainers_translator_test::x86_64::part::{FS, FSW, R0, XMM, YMMH, plain};
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

/// The Float status route's second call (docs/x86-native-output.md 3.1, 3.5) writes a save's x87 pointers and FOP,
/// and takes a load's, as the meaning does: every save and load form run natively, uncounted and counted, after no
/// x87 instruction, a non-control one, two, one with a memory operand and a control one after a non-control one,
/// its operand on a register, rip, fs, the area's gs and a 32-bit address; each outcome and the pointers the block
/// holds after compared with the meaning's, and each save's outcome with the CPU's.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_second_call_writes_and_takes_the_x87_pointers_as_the_meaning_x86_64() {
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_translator::LookAhead;
    use wcontainers_translator_test::layout::{CODE, DATA};
    use wcontainers_translator_test::x86_64::part::native::{
        REFERENCE_HOST, compare as natively, run_with_pointers,
    };
    use wcontainers_translator_test::x86_64::part::{FS, GS, plain};
    use wcontainers_x86::x87::{BUSY, SUMMARY};
    use wcontainers_x86::{Fxsave, MXCSR_AT_START, MXCSR_WRITABLE, RCX, RDX, X86Block};
    use zerocopy::FromBytes;
    /// The control word's bits that mask the six exceptions, with the reserved bit 6 the CPU reads as one, and
    /// the bits it keeps (Intel SDM Vol. 1 8.1.5).
    const MASKED: u16 = 0x7f;
    const KEPT_CONTROL: u16 = 0x1f3f;
    const MXCSR: usize = std::mem::offset_of!(Fxsave, mxcsr);
    let mut reference = Reference::start(&wcontainers_translator_test::x86_64::part::REFERENCE);
    // The image on the writable data's second page, the prior instructions' operand on its third.
    let image_at = DATA + PAGE_SIZE;
    let operand_at = image_at + PAGE_SIZE;
    let base = 0x1234_5000_u64;
    let mut seed = 0x5eed_u64;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        seed
    };
    // fld1; fadd st0, st1; fld qword [rdx]; fnstcw [rdx]; fnclex.
    let (fld1, fadd, fld, fnstcw, fnclex) = (
        [0xd9, 0xe8],
        [0xd8, 0xc1],
        [0xdd, 0x02],
        [0xd9, 0x3a],
        [0xdb, 0xe2],
    );
    let priors: Vec<Vec<u8>> = vec![
        vec![],
        fld1.to_vec(),
        [fld1, fadd].concat(),
        fld.to_vec(),
        [fld1, fnstcw].concat(),
        [fld1, fnclex].concat(),
    ];
    // Each form on [rcx]: fxsave, fxsave64, fnstenv, fnsave, each 32-bit and 16-bit, then their loads.
    let forms: [(&[u8], bool); 12] = [
        (&[0x0f, 0xae, 0x01], false),
        (&[0x48, 0x0f, 0xae, 0x01], false),
        (&[0xd9, 0x31], false),
        (&[0x66, 0xd9, 0x31], false),
        (&[0xdd, 0x31], false),
        (&[0x66, 0xdd, 0x31], false),
        (&[0x0f, 0xae, 0x09], true),
        (&[0x48, 0x0f, 0xae, 0x09], true),
        (&[0xd9, 0x21], true),
        (&[0x66, 0xd9, 0x21], true),
        (&[0xdd, 0x21], true),
        (&[0x66, 0xdd, 0x21], true),
    ];
    let (mut checked, mut same, mut differ) = (0, 0, 0);
    for (form, restore) in forms {
        for prior in if restore { &priors[..2] } else { &priors[..] } {
            // [rcx], [rip + disp32], fs:[rcx], gs:[rcx], [ecx].
            for place in 0..5 {
                let (prefix, rip_relative): (&[u8], bool) = match place {
                    1 => (&[], true),
                    2 => (&[0x64], false),
                    3 => (&[0x65], false),
                    4 => (&[0x67], false),
                    _ => (&[], false),
                };
                let mut code = [prior.as_slice(), prefix, form].concat();
                if rip_relative {
                    let modrm = code.pop().expect("a ModRM byte");
                    code.push(modrm & 0x38 | 0x05);
                    let end = CODE + code.len() as u64 + size_of::<i32>() as u64;
                    let displacement = i32::try_from(image_at as i64 - end as i64).unwrap();
                    code.extend(displacement.to_le_bytes());
                }
                let mut case = plain(&code);
                case.tracking = true;
                let rcx = match place {
                    2 | 3 => image_at - base,
                    4 => image_at | 1 << u32::BITS,
                    _ => image_at,
                };
                case.state.0[usize::from(RCX)] = rcx.into();
                case.state.0[usize::from(RDX)] = operand_at.into();
                case.state.0[FS] = base.into();
                case.state.0[GS] = base.into();
                if restore {
                    let mut image: Vec<u8> = (0..size_of::<Fxsave>() / size_of::<u64>())
                        .flat_map(|_| next().to_le_bytes())
                        .collect();
                    // A control word with every exception masked and its reserved bits as the CPU keeps them,
                    // a status word whose summary and busy bits agree with it, and an MXCSR `fxrstor` takes.
                    let control = u16::from_le_bytes([image[0], image[1]]) & KEPT_CONTROL | MASKED;
                    image[..size_of::<u16>()].copy_from_slice(&control.to_le_bytes());
                    // The status word follows the control word's word: 16 bits in an FXSAVE image and the
                    // 16-bit environment, 32 in the 32-bit one.
                    let sixteen = form.contains(&0xae) || form[0] == 0x66;
                    let status = if sixteen {
                        size_of::<u16>()
                    } else {
                        size_of::<u32>()
                    };
                    image[status] &= !(SUMMARY as u8);
                    image[status + 1] &= !((BUSY >> u8::BITS) as u8);
                    let mxcsr = next() as u32 & MXCSR_WRITABLE | MXCSR_AT_START;
                    image[MXCSR..][..size_of::<u32>()].copy_from_slice(&mxcsr.to_le_bytes());
                    case.placed = ((image_at - DATA) as u32, image);
                }
                let (meaning, machine) = x86_64::translated_on(&case, |_| {});
                let (block, _) =
                    X86Block::read_from_prefix(machine.stopped_state()).expect("a block");
                let f = &block.fxsave;
                let pointers = (f.fip, f.fdp, f.fop);
                // The count's check at every host instruction reads a piece of one instruction.
                let counts: &[bool] = if prior.is_empty() {
                    &[false, true]
                } else {
                    &[false]
                };
                let stretch = LookAhead::Bounded(code.len() as u32);
                for &counted in counts {
                    let (outcome, ended, after) =
                        run_with_pointers(&mut reference, &case, REFERENCE_HOST, counted, stretch)
                            .unwrap_or_else(|e| panic!("{code:02x?}: {e}"));
                    assert!(
                        outcome == meaning && after == Some(pointers),
                        "{code:02x?}, counted {counted}, ended {ended:?}: natively {:?} {:x?} {:x?} \
                         {after:x?}, the meaning {:?} {:x?} {:x?} {pointers:x?}",
                        outcome.stop,
                        outcome.state.0,
                        outcome.changed,
                        meaning.stop,
                        meaning.state.0,
                        meaning.changed
                    );
                    checked += 1;
                }
                // The reference runs one instruction.
                if !restore && prior.is_empty() {
                    match natively(&mut reference, &case, REFERENCE_HOST) {
                        Ok((x86_64::Agreement::Same, _)) => same += 1,
                        other => {
                            differ += 1;
                            eprintln!("{code:02x?} against the CPU: {other:?}");
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "the second call as the meaning in {checked} runs; the saves equal to the CPU's in {same}, \
         different in {differ}"
    );
}

/// The inside's answers to `enter` and to every x87 store to memory, which hold the forms that leave part of their
/// write behind a fault (docs/x86-native-output.md 3.6), equal the meaning's, the x87 pointers included: `enter`
/// of both widths at the nesting levels that matter, its pushes, copies and final check meeting the data's ends;
/// each store of numbers, NaNs, values out of range and an empty stack across the writable data's end, the data's
/// end and the canonical range's, through 32-bit addresses, under four control words and with an exception
/// pending. The partial forms also run natively for a host whose probe finds them different, through the inside.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_inside_answers_enter_and_the_x87_stores_as_the_meaning_x86_64() {
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_translator_test::layout::{DATA, DATA_BYTES, DATA_WRITABLE};
    use wcontainers_translator_test::x86_64::part::native::{
        Ended, REFERENCE_HOST, answered_with_pointers, compare as natively,
    };
    use wcontainers_translator_test::x86_64::part::{FCW, FS, FSW, FTW, R0, RSP, plain};
    use wcontainers_x86::native::HostMatches;
    use wcontainers_x86::x87::{
        BIAS, BUSY, EXCEPTIONS, FIELD, INTEGER_BIT, INVALID, PRECISION_SHIFT, QUIET_BIT,
        ROUNDING_SHIFT, SUMMARY, TOP_SHIFT,
    };
    use wcontainers_x86::{FCW_AT_START, RBP, RCX, USER_END, X86Block, X87_REGISTER_BITS};
    use zerocopy::FromBytes;
    let different = HostMatches {
        partial_writes: false,
        ..REFERENCE_HOST
    };
    let mut reference = Reference::start(&wcontainers_translator_test::x86_64::part::REFERENCE);
    let (mut checked, mut same, mut known) = (0, 0, 0);
    let mut check = |case: &x86_64::Case, native: bool| {
        let code = case.code.code();
        let (answered, pointers) = answered_with_pointers(case)
            .unwrap_or_else(|| panic!("{code:02x?}: an instruction the inside answers"));
        let (meaning, machine) = x86_64::translated_on(case, |_| {});
        let (block, _) = X86Block::read_from_prefix(machine.stopped_state()).expect("a block");
        let f = &block.fxsave;
        let theirs = (f.fip, f.fdp, f.fop);
        assert!(
            answered == meaning && pointers == theirs,
            "{code:02x?} {:x?}: the answer {:?} {:x?} {:x?} {pointers:x?}, the meaning {:?} {:x?} {:x?} \
             {theirs:x?}",
            case.state.0,
            answered.stop,
            answered.state.0,
            answered.changed,
            meaning.stop,
            meaning.state.0,
            meaning.changed
        );
        checked += 1;
        if native {
            match natively(&mut reference, case, different) {
                Ok((x86_64::Agreement::Same, Ended::Answered(_))) => same += 1,
                Ok((x86_64::Agreement::KnownFailure(why), Ended::Answered(_))) => {
                    known += 1;
                    eprintln!("{code:02x?} natively: {why}");
                }
                other => panic!("{code:02x?} natively for a host that differs: {other:?}"),
            }
        }
    };
    // enter of 64 and 16 bits; the stack in the middle of the writable data, its pushes or its final check
    // meeting the data's start, the copies meeting it, a read-only stack and one off the canonical range.
    let middle = DATA + 2 * PAGE_SIZE;
    let stacks = [
        (middle, DATA + PAGE_SIZE + PAGE_SIZE / 2),
        (DATA + 0x18, DATA + PAGE_SIZE),
        (DATA + 2, DATA + PAGE_SIZE),
        (middle, DATA + 0x10),
        (DATA + DATA_WRITABLE + 8, DATA + PAGE_SIZE),
        (USER_END + 0x10, DATA + PAGE_SIZE),
    ];
    for prefix in [&[][..], &[0x66]] {
        for size in [0u16, 8, 0x18, PAGE_SIZE as u16, u16::MAX] {
            for level in [0u8, 1, 2, 3, 4, 31, 32, 33, u8::MAX] {
                let code = [prefix, &[0xc8], &size.to_le_bytes(), &[level]].concat();
                for (rsp, rbp) in stacks {
                    let mut case = plain(&code);
                    case.state.0[RSP] = rsp.into();
                    case.state.0[usize::from(RBP)] = rbp.into();
                    check(&case, level % 32 != 0);
                }
            }
        }
    }
    // The stores to [rcx]: fst and fstp of 32 and 64 bits, fstp of 80, fist, fistp and fisttp of 16, 32 and
    // 64 bits, fbstp.
    let stores: [[u8; 2]; 14] = [
        [0xd9, 0x11],
        [0xd9, 0x19],
        [0xdd, 0x11],
        [0xdd, 0x19],
        [0xdb, 0x39],
        [0xdf, 0x11],
        [0xdf, 0x19],
        [0xdb, 0x11],
        [0xdb, 0x19],
        [0xdf, 0x39],
        [0xdf, 0x09],
        [0xdb, 0x09],
        [0xdd, 0x09],
        [0xdf, 0x31],
    ];
    let partial = |form: [u8; 2]| matches!(form, [0xdb, 0x39] | [0xdf, 0x31]);
    let value = |sign: bool, exponent: u16, significand: u64| {
        u128::from(sign) << (X87_REGISTER_BITS - 1)
            | u128::from(exponent) << u64::BITS
            | u128::from(significand)
    };
    let bias = BIAS as u16;
    let all = u16::MAX >> 1;
    // ST(0) or none: one, a number with a fraction, a large negative one past 10^18, one past every format, a
    // tiny one, the quiet and signalling NaNs, infinity and an unnormal.
    let values = [
        Some(value(false, bias, INTEGER_BIT)),
        Some(value(false, bias + 16, 0xf120_c000_0000_0000)),
        Some(value(true, bias + 62, u64::MAX - 0xf)),
        Some(value(false, bias + 2 * f64::MAX_EXP as u16, INTEGER_BIT)),
        Some(value(false, 1, INTEGER_BIT)),
        Some(value(false, all, INTEGER_BIT | QUIET_BIT)),
        Some(value(true, all, INTEGER_BIT | 1)),
        Some(value(false, all, INTEGER_BIT)),
        Some(value(false, bias, QUIET_BIT)),
        None,
    ];
    // ST(0) is physical register 7, ST(1) register 0.
    let top = 7u16;
    let placed = |form: [u8; 2], prefix: &[u8], rcx: u64, fcw: u16, fsw: u16, st0: Option<u128>| {
        let mut case = plain(&[prefix, &form].concat());
        case.state.0[usize::from(RCX)] = rcx.into();
        case.state.0[FCW] = fcw.into();
        case.state.0[FSW] = (top << TOP_SHIFT | fsw).into();
        case.state.0[R0] = value(false, bias + 3, INTEGER_BIT);
        let mut tags = 1u8;
        if let Some(st0) = st0 {
            case.state.0[R0 + usize::from(top)] = st0;
            tags |= 1 << top;
        }
        case.state.0[FTW] = tags.into();
        case
    };
    let writable_end = DATA + DATA_WRITABLE;
    let data_end = DATA + DATA_BYTES;
    let places: Vec<u64> = [DATA + PAGE_SIZE, USER_END - 8]
        .into_iter()
        .chain((1..=10).map(|k| writable_end - k))
        .chain([1, 2, 8, 9].map(|k| data_end - k))
        .collect();
    let fs_base = (DATA + PAGE_SIZE).wrapping_sub(0xffff_fff8);
    // Rounding toward zero at single precision.
    let truncating = FCW_AT_START & !(FIELD << PRECISION_SHIFT) | FIELD << ROUNDING_SHIFT;
    for form in stores {
        for st0 in values {
            for &rcx in &places {
                let native = partial(form) && rcx != USER_END - 8;
                check(&placed(form, &[], rcx, FCW_AT_START, 0, st0), native);
            }
            // [ecx] with the upper half of rcx set, and fs:[ecx] whose last bytes pass 2^32.
            let low = 1 << u32::BITS | (DATA + PAGE_SIZE);
            check(
                &placed(form, &[0x67], low, FCW_AT_START, 0, st0),
                partial(form),
            );
            // That base is not canonical, which a thread of the reference cannot take.
            let mut case = placed(form, &[0x64, 0x67], 0xffff_fff8, FCW_AT_START, 0, st0);
            case.state.0[FS] = fs_base.into();
            check(&case, false);
            for fcw in [
                FCW_AT_START & !EXCEPTIONS,
                FCW_AT_START & !INVALID,
                truncating,
            ] {
                check(
                    &placed(form, &[], DATA + PAGE_SIZE, fcw, 0, st0),
                    partial(form),
                );
            }
        }
        // An invalid operation raised and unmasked before: pending.
        let pending = placed(
            form,
            &[],
            DATA + PAGE_SIZE,
            FCW_AT_START & !INVALID,
            INVALID | SUMMARY | BUSY,
            values[0],
        );
        check(&pending, partial(form));
    }
    eprintln!(
        "the inside's enter and x87 stores equal to the meaning's in {checked} cases; natively through the \
         inside equal to the CPU's in {same}, known failures {known}"
    );
}

/// `Condition::holds`, by which the inside takes a `jcc` exit's `int3` hit (docs/x86-native-output.md 3.2), is the
/// meaning's: `setcc al` under WebAssembly for each of the sixteen conditions on every combination of the five
/// flags conditions read.
#[test]
fn a_condition_holds_as_the_meaning_says_x86_64() {
    use wcontainers_translator_test::x86_64::layer1::translated;
    use wcontainers_translator_test::x86_64::part::{RFLAGS, plain};
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
            let outcome = translated(&case);
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
    x86_64::unoffered_forms_are_illegal();
}

#[test]
fn bytes_in_no_form_stop_as_on_the_cpu_x86_64() {
    x86_64::bytes_in_no_form_stop_as_on_the_cpu();
}

#[test]
fn sample_matches_the_cpu_in_chrome_x86_64() {
    x86_64::sample_matches_the_cpu_in_chrome();
}

#[test]
fn hand_cases_do_what_they_must_x86_64() {
    x86_64::hand_cases_do_what_they_must();
}

/// Every x86-64 code of a feature not offered has cases, but those left out by design and those the
/// translator decodes as another code, which are in that code's list.
#[test]
fn every_unoffered_code_is_drawn_x86_64() {
    use wcontainers_translator_test::x86_64::part::{NOT_DRAWN, unoffered_left_out};
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

/// The single-instruction shapes of docs/x86-native-output.md section 7 natively against the CPU: `ret`, `jmp r`
/// and `call r` (plain and 66-prefixed) to 2^47, 2^48, a valid target with each of bits 48 to 63 set alone,
/// 0xfff0000000401000, 2^63 and -1; 66-prefixed direct `jmp` and `call`; `wrfsbase`, `wrgsbase`, `rdgsbase` and
/// `mov r, gs`; `rdsspq` and `incsspq`; `movsb`, `rep movsb`, `xlat`, `maskmovq`, `maskmovdqu` and a bit test under
/// 0x67, each with a `gs:` override; `gs:` operands with the guest's 0x67 and a base at or above 2^32, RIP-relative
/// with and without it; and the independent check's cases (an `fxsave64` across offset 2^32 under 0x67, `setg`
/// with three segment prefixes, a RIP-relative bit test under 0x67 with a bit offset past 2^32). Every case ends as
/// the CPU's, or as a named known difference: the 5-level reference, or an answer that is the meaning's where the
/// meaning differs.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_shapes_of_the_plan_match_the_cpu_natively_x86_64() {
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_translator_test::layout::{CODE, DATA};
    use wcontainers_translator_test::x86_64::part::native::{
        Ended, REFERENCE_HOST, compare as natively,
    };
    use wcontainers_translator_test::x86_64::part::{FS, GS, RFLAGS, RSP, XMM, plain};
    use wcontainers_x86::{R9, R10, RAX, RBX, RCX, RDI, RSI, USER_END};
    // Where the stack, a string's destination and a bit offset are, and a GS base above 2^32 (the check's gs67.c).
    const STACK_AT: u64 = PAGE_SIZE / 2;
    const AWAY: u64 = 1 << u8::BITS;
    const HIGH_BASE: u64 = 3 << u32::BITS;
    let mut reference = Reference::start(&wcontainers_translator_test::x86_64::part::REFERENCE);
    let (mut same, mut known, mut failures) = (0, BTreeMap::<&str, u64>::new(), Vec::new());
    let mut check = |case: x86_64::Case| match natively(&mut reference, &case, REFERENCE_HOST) {
        Ok((x86_64::Agreement::Same, ended)) => {
            same += 1;
            Some(ended)
        }
        Ok((x86_64::Agreement::KnownFailure(why), ended)) => {
            *known.entry(why).or_default() += 1;
            Some(ended)
        }
        Err(report) => {
            failures.push(format!("{:02x?}: {report}", case.code.code()));
            None
        }
    };
    let with = |code: &[u8], registers: &[(usize, u64)]| {
        let mut case = plain(code);
        for &(field, value) in registers {
            case.state.0[field] = value.into();
        }
        case
    };
    let r = |n: u8| usize::from(n);
    // The transfers' targets.
    let mut targets = vec![
        1 << 47,
        1 << 48,
        0xfff0_0000_0040_1000,
        1 << 63,
        u64::MAX,
        CODE,
    ];
    targets.extend((48..64).map(|bit| CODE | 1 << bit));
    let stack = DATA + STACK_AT;
    for &target in &targets {
        for prefix in [&[][..], &[0x66]] {
            for code in [&[0xffu8, 0xe0][..], &[0xff, 0xd0]] {
                check(with(&[prefix, code].concat(), &[(r(RAX), target)]));
            }
            let mut ret = with(&[prefix, &[0xc3]].concat(), &[(RSP, stack)]);
            ret.placed = ((stack - DATA) as u32, target.to_le_bytes().to_vec());
            check(ret);
        }
    }
    for code in [
        &[0x66, 0xe9, 0x10, 0x00, 0x00, 0x00][..],
        &[0x66, 0xe8, 0x10, 0x00, 0x00, 0x00],
        &[0x66, 0xeb, 0x10],
    ] {
        check(with(code, &[(RSP, stack)]));
    }
    // The bases and the gs selector, with canonical, 5-level-only and non-canonical values.
    for value in [DATA, 1 << 47, 1 << 56, USER_END.wrapping_neg(), 1 << 63] {
        for code in [
            &[0xf3, 0x48, 0x0f, 0xae, 0xd0][..],
            &[0xf3, 0x48, 0x0f, 0xae, 0xd8],
            &[0xf3, 0x0f, 0xae, 0xd8],
        ] {
            check(with(code, &[(r(RAX), value)]));
        }
    }
    for code in [
        &[0xf3, 0x48, 0x0f, 0xae, 0xc8][..],
        &[0xf3, 0x0f, 0xae, 0xc8],
        &[0x48, 0x8c, 0xe8],
        &[0x8c, 0xe8],
        &[0xf3, 0x48, 0x0f, 0x1e, 0xc8],
        &[0xf3, 0x48, 0x0f, 0xae, 0xe8],
    ] {
        check(with(code, &[(r(RAX), 0x1234), (GS, DATA + 0x40)]));
    }
    // Implicit addresses on gs.
    let on_gs = [
        (GS, DATA),
        (r(RSI), 0x10),
        (r(RDI), DATA + AWAY),
        (r(RBX), 0x20),
    ];
    for code in [
        &[0x65, 0xa4][..],
        &[0x65, 0x49, 0xa4],
        &[0xf3, 0x65, 0xa4],
        &[0x65, 0xa6],
        &[0x65, 0xac],
        &[0x65, 0xaa],
        &[0x65, 0xd7],
        &[0x65, 0x4c, 0xd7],
        &[0x65, 0x0f, 0xf7, 0xc1],
        &[0x66, 0x65, 0x0f, 0xf7, 0xc1],
    ] {
        let mut case = with(code, &on_gs);
        case.state.0[r(RCX)] = 5;
        case.state.0[r(RAX)] = 3;
        case.state.0[XMM + 1] = u128::MAX;
        check(case);
    }
    // A bit test of gs:[eax] with ecx's offset under 0x67, where the sum carries past 2^32, plain and locked.
    for code in [
        &[0x65, 0x67, 0x0f, 0xa3, 0x08][..],
        &[0xf0, 0x65, 0x67, 0x0f, 0xab, 0x08],
    ] {
        check(with(
            code,
            &[(GS, DATA), (r(RAX), 0xffff_fff0), (r(RCX), AWAY)],
        ));
    }
    // gs: with the guest's 0x67: the check's gs67.c (a fault at 0x300000010), and one that lands in the data.
    check(with(
        &[0x65, 0x67, 0x8b, 0x00],
        &[(GS, HIGH_BASE), (r(RAX), 0x1_0000_0010)],
    ));
    check(with(
        &[0x65, 0x67, 0x8b, 0x00],
        &[(GS, DATA - 0x10), (r(RAX), 0x1_0000_0020)],
    ));
    // RIP-relative on gs, the operand's offset a page below the data, with and without 0x67.
    for prefix in [&[0x65][..], &[0x65, 0x67]] {
        let end = CODE + prefix.len() as u64 + 6;
        let displacement = (DATA - PAGE_SIZE).wrapping_sub(end) as u32;
        let code = [prefix, &[0x8b, 0x05], &displacement.to_le_bytes()].concat();
        check(with(&code, &[(GS, PAGE_SIZE)]));
    }
    // The check's fxsave64 across offset 2^32 under 0x67 (offset 0xffffffd6; the CPU faults at GS + 0x1d5).
    check(with(
        &[
            0x65, 0x67, 0x4b, 0x0f, 0xae, 0x84, 0x8a, 0x36, 0xba, 0xa5, 0x06,
        ],
        &[
            (GS, 0x5a2d_c784_37da),
            (r(R10), 0xffff_ffd6 - 0x06a5_ba36),
            (r(R9), 0),
        ],
    ));
    // Several segment prefixes: gs among others in every order lands at the data, any other segment would fault at
    // the offset (the fourteenth check's prefixes.c; the setg is the ninth check's); with fs and gs both, the last.
    const OFFSET: u64 = 0x40;
    let mut chosen = Vec::new();
    let setg = [
        0x2e, 0x65, 0x36, 0x4f, 0x0f, 0x9f, 0x81, 0xdf, 0xbc, 0x7c, 0x48,
    ];
    let displacement = u32::from_le_bytes(setg[7..].try_into().expect("a disp32"));
    let ended = check(with(
        &setg,
        &[
            (r(R9), OFFSET.wrapping_sub(displacement.into())),
            (GS, DATA),
        ],
    ));
    chosen.push((setg.to_vec(), ended, Ended::Exit));
    for prefixes in [
        &[0x65][..],
        &[0x2e, 0x65, 0x36],
        &[0x65, 0x36],
        &[0x36, 0x65],
        &[0x65, 0x2e],
        &[0x65, 0x3e],
        &[0x65, 0x26],
    ] {
        for rex in [&[][..], &[0x48]] {
            let code = [prefixes, rex, &[0x8b, 0x01]].concat();
            let ended = check(with(&code, &[(r(RCX), OFFSET), (GS, DATA)]));
            chosen.push((code, ended, Ended::Exit));
        }
    }
    for (code, ended) in [
        (&[0x64, 0x65, 0x8b, 0x01][..], Ended::Exit),
        (&[0x65, 0x64, 0x8b, 0x01], Ended::Fault),
        (&[0x64, 0x2e, 0x65, 0x36, 0x8b, 0x01], Ended::Exit),
        (&[0x65, 0x36, 0x64, 0x2e, 0x8b, 0x01], Ended::Fault),
    ] {
        let got = check(with(code, &[(r(RCX), OFFSET), (GS, DATA), (FS, 0)]));
        chosen.push((code.to_vec(), got, ended));
    }
    chosen.retain(|(_, got, expected)| *got != Some(*expected));
    // The check's RIP-relative bit test under 0x67 (at DATA + 0x100), its bit offset past 2^32 and at 2^63.
    for offset in [0x8_0000_0000, 1 << 63] {
        check(with(
            &[0x67, 0x48, 0x0f, 0xa3, 0x05, 0xf7, 0x00, 0x00, 0xf4],
            &[(r(RAX), offset)],
        ));
    }
    // Direct transfers whose rel32 wraps below zero into the upper half (the tenth check): jmp, call with a stack
    // and without one, every jcc taken and not, and the check's prefixed forms.
    use wcontainers_x86::carry::REL32;
    let unmapped_stack = PAGE_SIZE;
    let farthest = CODE.wrapping_sub(1 << (u32::BITS - 1)) + PAGE_SIZE;
    for target in [0u64.wrapping_sub(PAGE_SIZE), farthest] {
        let rel = |opcode: &[u8]| {
            let end = CODE + (opcode.len() + REL32) as u64;
            [opcode, &(target.wrapping_sub(end) as u32).to_le_bytes()].concat()
        };
        for opcode in [
            &[0xe9][..],
            &[0xe8],
            &[0x49, 0xe8],
            &[0x66, 0xe9],
            &[0x66, 0xe8],
        ] {
            for rsp in [stack, unmapped_stack] {
                check(with(&rel(opcode), &[(RSP, rsp)]));
            }
        }
        for condition in 0..16u8 {
            for flags in [0, wcontainers_x86::ARITHMETIC_FLAGS] {
                for prefix in [&[][..], &[0x3e], &[0x66]] {
                    let mut case = with(&rel(&[prefix, &[0x0f, 0x80 | condition]].concat()), &[]);
                    case.state.0[RFLAGS] |= u128::from(flags);
                    check(case);
                }
            }
        }
    }
    eprintln!("the plan's shapes natively: {same} as the CPU, known differences {known:?}");
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(
        chosen.is_empty(),
        "segment prefixes ending otherwise: {chosen:02x?}"
    );
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
mod arm64 {
    use super::*;
    pub(super) use wcontainers_translator_test::arm64::guest::Case;
    pub(super) use wcontainers_translator_test::arm64::layer1::{
        Agreement, compare, compare_outcome, translated_in_chrome, translated_on,
    };
    use wcontainers_translator_test::arm64::part;

    include!("for_guest/layer1.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    pub(super) use wcontainers_translator_test::x86_64::guest::{Case, Stop};
    pub(super) use wcontainers_translator_test::x86_64::layer1::{
        Agreement, compare, compare_outcome, on_reference, translated_in_chrome, translated_on,
        translated_with,
    };
    use wcontainers_translator_test::x86_64::part;

    include!("for_guest/layer1.rs");

    /// The forms of features not offered stop ours as illegal instruction; where the reference lacks the
    /// feature too it must stop the same, else what it did is recorded. `WC_LAYER1_UNOFFERED_CASES` cases a form
    /// (default 64), with the seed and the choice of forms as for the default forms.
    pub(super) fn unoffered_forms_are_illegal() {
        let count = setting("WC_LAYER1_UNOFFERED_CASES", 64) as u32;
        // What the reference did with the forms it has: each stop's kind, counted, and the forms it too refused.
        let recorded = Mutex::new(BTreeMap::<String, u64>::new());
        let refused_there = Mutex::new(BTreeMap::<String, u64>::new());
        let failures = run_forms(
            part::unoffered_forms(),
            count,
            " of the unoffered forms",
            |reference, form, case| {
                let actual = translated_with(case, |m| m.set_offer(Default::default()));
                if !part::reference_has(form) {
                    return compare_outcome(reference, case, actual);
                }
                if actual.stop != Stop::Illegal {
                    return Err(format!(
                        "code {:02x?} ({}): translated {:?}, not illegal instruction",
                        case.code.code(),
                        part::disassemble(case.code.code()),
                        actual.stop
                    ));
                }
                let stop = on_reference(reference, case).stop;
                let kind = format!("{stop:?}");
                let kind = kind.split('(').next().unwrap_or_default().to_string();
                *recorded.lock().unwrap().entry(kind).or_default() += 1;
                if stop == Stop::Illegal {
                    *refused_there
                        .lock()
                        .unwrap()
                        .entry(part::form_name(form).to_string())
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
}
