//! Layer 2: random small programs give the same result at full look-ahead (for speed and for size, plain and
//! with the thread stopping at every stop check and carrying on), at bounded look-ahead of a random N, and
//! with pieces of all three mixed and rare events injected (see `mixed`), as at none, register by register
//! and byte by byte in the same engine, and the same output as on the real CPU.
//!
//! Knobs: `WC_LAYER2_CASES` (default 64), `WC_LAYER2_SEED` (default 1), `WC_LAYER2_FROM` (the first case,
//! default 0; case `i` is generated from the seed and `i` alone, so a failure replays with `WC_LAYER2_FROM=i`
//! and `WC_LAYER2_CASES=1`), `WC_LAYER2_JOBS` (cases run at once, default 1), `WC_LAYER2_NATIVE` (how many
//! of the cases, the first, also run on the real CPU, default all; each job keeps one reference container) and
//! `WC_LAYER2_CHROME` (how many, the first, also run in Chrome at none and at full in both settings, default
//! 1; each starts Chrome three times).

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{LookAhead, Optimise};
use wcontainers_translator_test::jobs::at_once;
use wcontainers_translator_test::seeded;

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// The random generator of case `case` of a run from `seed`, so that any case replays by itself.
fn generator(seed: u64, case: u64) -> TestRunner {
    let config = Config {
        cases: 1,
        failure_persistence: None,
        ..Config::default()
    };
    TestRunner::new_with_rng(config, seeded::generator(seed, &case.to_le_bytes(), ""))
}

#[test]
fn full_matches_none_and_the_cpu_arm64() {
    arm64::full_matches_none_and_the_cpu();
}

#[test]
fn full_matches_none_and_the_cpu_x86_64() {
    x86_64::full_matches_none_and_the_cpu();
}

/// The native output for a host process (docs/x86-native-output.md section 8): each program's pieces at none, at
/// the case's bound and at full, without and with the count, linked and run on this machine against its CPU, and
/// the count after each system call against the WebAssembly output's. Linux x86-64 only, where the reference is
/// this machine. Knobs as above.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn natively_matches_the_cpu_x86_64() {
    natively_in(&[wcontainers_x86::native::Way::Process], "natively");
}

/// The same in the in-process way's two checked forms (docs/x86-native-output.md sections 4 and 8): the program
/// and its stack in the window, the reference's own stack unmapped. `WC_LAYER2_CHECK` takes one form (`wrapping`
/// or `exact`), both otherwise.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn natively_matches_the_cpu_in_process_x86_64() {
    use wcontainers_x86::native::{Check, Way};
    let only = std::env::var("WC_LAYER2_CHECK").ok();
    let ways: Vec<Way> = [("wrapping", Check::Wrapping), ("exact", Check::Exact)]
        .into_iter()
        .filter(|(name, _)| only.as_deref().is_none_or(|only| only == *name))
        .map(|(_, check)| Way::InProcess(check))
        .collect();
    natively_in(&ways, "natively in process");
}

/// Layer 2's programs in each of `ways`, the summary line starting "layer 2 " and `what`.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn natively_in(ways: &[wcontainers_x86::native::Way], what: &str) {
    use wcontainers_translator_test::layout::SIGNALLED;
    use wcontainers_translator_test::reference::with_reference;
    use wcontainers_translator_test::x86_64::part::native_programs::compare;
    use wcontainers_translator_test::x86_64::progress::run_counting;
    use wcontainers_x86::native::{Check, Way};
    let cases = knob("WC_LAYER2_CASES", 64);
    let seed = knob("WC_LAYER2_SEED", 1);
    let from = knob("WC_LAYER2_FROM", 0);
    let jobs = knob("WC_LAYER2_JOBS", 1).max(1);
    let [ran, faulted, runs, pieces, landed] = std::array::from_fn(|_| AtomicU64::new(0));
    let failures = Mutex::new(Vec::new());
    let count = |counter: &AtomicU64, n: u64| counter.fetch_add(n, Ordering::Relaxed);
    at_once(jobs, cases, |i| {
        let case = from + i;
        let result = generator(seed, case).run(
            &(
                wcontainers_translator_test::x86_64::layer2::program(),
                1..=64u32,
                any::<u64>(),
            ),
            |(program, bound, _)| {
                let blob = wcontainers_translator_test::x86_64::part::assemble(&program);
                let cpu = wcontainers_translator_test::x86_64::programs::native(&blob);
                if cpu.status != 0 {
                    count(&faulted, 1);
                }
                let counted = run_counting(&blob, (LookAhead::None, Optimise::Speed));
                assert_eq!(counted.ran, cpu, "the WebAssembly output against the CPU");
                for &way in ways {
                    for look_ahead in [LookAhead::None, LookAhead::Bounded(bound), LookAhead::Full]
                    {
                        for counting in [false, true] {
                            let compared = |way| {
                                with_reference(
                                    &wcontainers_translator_test::x86_64::part::REFERENCE,
                                    |reference| {
                                        compare(
                                            reference,
                                            &blob,
                                            (way, look_ahead),
                                            counting,
                                            &cpu,
                                            &counted,
                                        )
                                    },
                                )
                            };
                            let ours = match compared(way) {
                                Ok(ours) => ours,
                                // The wrapping form's named difference: an address at or above 2^32 lands
                                // modulo 2^32 in the window where the CPU faults, which the exact form refuses
                                // as the CPU does.
                                Err(_)
                                    if way == Way::InProcess(Check::Wrapping)
                                        && cpu.status == SIGNALLED + libc::SIGSEGV
                                        && compared(Way::InProcess(Check::Exact)).is_ok() =>
                                {
                                    count(&landed, 1);
                                    continue;
                                }
                                Err(e) => return Err(TestCaseError::fail(e)),
                            };
                            count(&runs, ours.runs.into());
                            count(&pieces, ours.pieces as u64);
                        }
                    }
                }
                Ok(())
            },
        );
        count(&ran, 1);
        if let Err(error) = result {
            eprintln!("layer 2 {what}, seed {seed}, case {case}: {error}");
            failures.lock().unwrap().push(case);
        }
    });
    let mut failures = failures.into_inner().unwrap();
    failures.sort();
    let [ran, faulted, runs, pieces, landed] =
        [ran, faulted, runs, pieces, landed].map(|c| c.into_inner());
    eprintln!(
        "layer 2 {what} ({ways:?}): {ran} cases run from case {from}, seed {seed}, {jobs} at once, each at 3 look-aheads with and without the count; {faulted} faulted; {runs} blob runs, {pieces} pieces; {landed} runs the wrapping form's named landing; {} failing: {failures:?}",
        failures.len()
    );
    assert!(failures.is_empty());
}

/// A hand-made program of `code` at the layer's load address, linked there, with the layer's stack, its image
/// `bytes` long with zeros after the code.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn hand_made(
    name: &str,
    code: Vec<u8>,
    bytes: usize,
) -> wcontainers_translator_test::x86_64::programs::Blob {
    use wcontainers_translator_test::x86_64::programs::{Blob, LOAD, STACK_BYTES};
    let code_bytes = code.len() as u64;
    let mut image = code;
    image.resize(bytes.max(image.len()), 0);
    Blob {
        name: name.into(),
        code_bytes,
        bytes: image,
        load: LOAD,
        link_address: Some(LOAD),
        main: None,
        program_headers: None,
        stack_bytes: STACK_BYTES,
        offer: None,
    }
}

/// `blob` natively at the three look-aheads, with and without the count, against the CPU's run, the count after
/// each system call against the WebAssembly output's.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn natively_as_the_cpu(
    blob: &wcontainers_translator_test::x86_64::programs::Blob,
) -> Result<(), String> {
    use wcontainers_translator_test::reference::with_reference;
    use wcontainers_translator_test::x86_64::part::native_programs::compare;
    use wcontainers_translator_test::x86_64::progress::run_counting;
    let cpu = wcontainers_translator_test::x86_64::programs::native(blob);
    let counted = run_counting(blob, (LookAhead::None, Optimise::Speed));
    if counted.ran != cpu {
        return Err(format!(
            "{}: the WebAssembly output {:?} against the CPU's {cpu:?}",
            blob.name, counted.ran
        ));
    }
    for look_ahead in [LookAhead::None, LookAhead::Bounded(3), LookAhead::Full] {
        for counting in [false, true] {
            with_reference(
                &wcontainers_translator_test::x86_64::part::REFERENCE,
                |reference| {
                    compare(
                        reference,
                        blob,
                        (wcontainers_x86::native::Way::Process, look_ahead),
                        counting,
                        &cpu,
                        &counted,
                    )
                },
            )
            .map_err(|e| format!("{}: {e}", blob.name))?;
        }
    }
    Ok(())
}

/// The shapes docs/x86-native-output.md section 3.2 was broken with, as programs: a direct branch into the
/// middle of an instruction in the same piece (the immediate of a `mov`), and a jump through a register to the
/// same.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn branches_into_an_instruction_match_the_cpu_natively_x86_64() {
    const EXIT_GROUP: u8 = libc::SYS_exit_group as u8;
    // Each exits with status 0 only from the code it reaches in the middle of the `mov`. 0: mov edi, 1; 5: jmp 9;
    // 7: mov rax, imm64 whose bytes from 9 are `xor edi, edi; jmp 11; nop * 4`; 11: exit_group(edi).
    let direct = vec![
        0xbf, 0x01, 0x00, 0x00, 0x00, 0xeb, 0x02, 0x48, 0xb8, 0x31, 0xff, 0xeb, 0x04, 0x90, 0x90,
        0x90, 0x90, 0xb8, EXIT_GROUP, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ];
    // 0: mov edi, 1; 5: lea rax, [rip + 4] (10); c: jmp rax; e: mov rax, imm64 whose bytes from 10 are `xor edi,
    // edi; jmp 18; nop * 4`; 18: exit_group(edi).
    let through = vec![
        0xbf, 0x01, 0x00, 0x00, 0x00, 0x48, 0x8d, 0x05, 0x04, 0x00, 0x00, 0x00, 0xff, 0xe0, 0x48,
        0xb8, 0x31, 0xff, 0xeb, 0x04, 0x90, 0x90, 0x90, 0x90, 0xb8, EXIT_GROUP, 0x00, 0x00, 0x00,
        0x0f, 0x05,
    ];
    let failures: Vec<String> = [("into-a-mov", direct), ("jump-into-a-mov", through)]
        .into_iter()
        .filter_map(|(name, code)| natively_as_the_cpu(&hand_made(name, code, 0)).err())
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Repeated strings against the WebAssembly output's count natively (plan section 7): seeded over every kind,
/// element size, prefix (rep, repe, repne, the guest's 0x67), DF, rcx (0, 1, a few, the bulk pass's cap and either
/// side of it) and rdi - rsi (0, 1, an element, ten, the length, minus ten, past the cap), with the check's case
/// (`rep movsb`, rdi = rsi + 10, rcx 100: 91 passes); a system call after the string reads the count.
/// `WC_STRINGS_SEED` and `WC_STRINGS_CASES` set the sweep, `WC_STRINGS_JOBS` how many run at once. TF is left
/// out: a host process runs a guest with TF set on stepping pieces, which are not built.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn repeated_strings_count_as_the_webassembly_output_natively_x86_64() {
    use iced_x86::code_asm::*;
    use wcontainers_x86::strings::BULK_STRING_BYTES;
    let seed = knob("WC_STRINGS_SEED", 1);
    let cases = knob("WC_STRINGS_CASES", 64);
    let jobs = knob("WC_STRINGS_JOBS", 1).max(1);
    // The strings' memory is the program's own, from 64 KiB in (the code is shorter), 5 MiB of zeros; they start in
    // its middle, so that either way the cap and the farthest distance stay inside.
    const BUFFER: u64 = 1 << 16;
    const BUFFER_BYTES: usize = 5 << 20;
    const START: u64 =
        wcontainers_translator_test::x86_64::programs::LOAD + BUFFER + (BUFFER_BYTES as u64) / 2;
    const IMAGE: usize = BUFFER as usize + BUFFER_BYTES;
    // movs, stos, lods, cmps and scas of each size: the opcodes of their byte forms, the others one more.
    const KINDS: [(&str, u8); 5] = [
        ("movs", 0xa4),
        ("stos", 0xaa),
        ("lods", 0xac),
        ("cmps", 0xa6),
        ("scas", 0xae),
    ];
    let program = |kind: usize,
                   bytes: u8,
                   repeat: u8,
                   narrow: bool,
                   down: bool,
                   count: u64,
                   distance: i64,
                   value: u8|
     -> Vec<u8> {
        let mut a = CodeAssembler::new(64).unwrap();
        a.mov(rsi, START).unwrap();
        a.mov(rdi, START.wrapping_add_signed(distance)).unwrap();
        a.mov(rcx, count).unwrap();
        a.mov(eax, u32::from(value)).unwrap();
        if down {
            a.std().unwrap();
        }
        let (_, opcode) = KINDS[kind];
        let mut string = Vec::new();
        if narrow {
            string.push(wcontainers_x86::encoding::ADDRESS_SIZE);
        }
        string.push(repeat);
        match bytes {
            2 => string.push(wcontainers_x86::encoding::OPERAND_SIZE),
            8 => string.push(wcontainers_x86::encoding::REX | wcontainers_x86::encoding::REX_W),
            _ => {}
        }
        string.push(if bytes == 1 { opcode } else { opcode + 1 });
        a.db(&string).unwrap();
        a.cld().unwrap();
        a.mov(eax, libc::SYS_getpid as u32).unwrap();
        a.syscall().unwrap();
        a.mov(eax, libc::SYS_exit_group as u32).unwrap();
        a.xor(edi, edi).unwrap();
        a.syscall().unwrap();
        let code = a
            .assemble(wcontainers_translator_test::x86_64::programs::LOAD)
            .unwrap();
        assert!(
            code.len() as u64 <= BUFFER,
            "the code fits before the strings' memory"
        );
        code
    };
    let failures = Mutex::new(Vec::new());
    // The check's case first.
    let check = program(
        0,
        1,
        wcontainers_x86::encoding::REP,
        false,
        false,
        100,
        10,
        0,
    );
    if let Err(e) = natively_as_the_cpu(&hand_made("rep-movsb-10-above", check, IMAGE)) {
        failures.lock().unwrap().push(e);
    }
    at_once(jobs, cases, |case| {
        let mut rng = seeded::generator(seed, &case.to_le_bytes(), "repeated strings");
        let mut pick = |n: usize| (rng.next_u64() % n as u64) as usize;
        let kind = pick(KINDS.len());
        let bytes = [1u8, 2, 4, 8][pick(4)];
        let compares = matches!(KINDS[kind].0, "cmps" | "scas");
        let repeat = if compares && pick(2) == 0 {
            wcontainers_x86::encoding::REPNE
        } else {
            wcontainers_x86::encoding::REP
        };
        let narrow = pick(4) == 0;
        let down = pick(2) == 0;
        let cap = BULK_STRING_BYTES / u64::from(bytes);
        let count = [0, 1, 7, cap - 1, cap, cap + 1][pick(6)];
        let length = (count * u64::from(bytes)) as i64;
        let distance = [
            0,
            1,
            i64::from(bytes),
            10,
            length,
            -10,
            BULK_STRING_BYTES as i64 + 8,
        ][pick(7)];
        let value = pick(2) as u8;
        let name = format!(
            "{}{} {} {}{} rcx {count} rdi-rsi {distance} al {value}",
            if narrow { "67 " } else { "" },
            if repeat == wcontainers_x86::encoding::REP {
                "rep"
            } else {
                "repne"
            },
            KINDS[kind].0,
            bytes,
            if down { " std" } else { "" }
        );
        let code = program(kind, bytes, repeat, narrow, down, count, distance, value);
        if let Err(e) = natively_as_the_cpu(&hand_made(&name, code, IMAGE)) {
            eprintln!("strings natively, seed {seed}, case {case}: {e}");
            failures.lock().unwrap().push(e);
        }
    });
    let failures = failures.into_inner().unwrap();
    eprintln!(
        "repeated strings natively: {cases} cases and the check's, seed {seed}, {jobs} at once; {} failing",
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
mod arm64 {
    use super::*;
    pub(super) use wcontainers_translator_test::arm64::browser;
    pub(super) use wcontainers_translator_test::arm64::layer2::{area_offset, output, program};
    pub(super) use wcontainers_translator_test::arm64::mixed::mixed;
    use wcontainers_translator_test::arm64::part;
    pub(super) use wcontainers_translator_test::arm64::programs::{
        boundaries, native, ran as outcome, reachable_memory, ready, translated, translated_for,
        translated_stopping_everywhere,
    };

    include!("for_guest/layer2.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    pub(super) use wcontainers_translator_test::x86_64::browser;
    pub(super) use wcontainers_translator_test::x86_64::layer2::{area_offset, output, program};
    pub(super) use wcontainers_translator_test::x86_64::mixed::mixed;
    use wcontainers_translator_test::x86_64::part;
    pub(super) use wcontainers_translator_test::x86_64::programs::{
        boundaries, native, ran as outcome, reachable_memory, ready, translated, translated_for,
        translated_stopping_everywhere,
    };

    include!("for_guest/layer2.rs");
}
