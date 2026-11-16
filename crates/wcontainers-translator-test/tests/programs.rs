//! C programs give the same output and exit status through the translator as on real ARM64 Linux.

use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::programs::{
    Library, build, build_with, native, reachable_memory, translated,
    translated_stopping_everywhere,
};

const LEVELS: [&str; 5] = ["-O0", "-O1", "-O2", "-O3", "-Os"];

#[test]
fn programs_match_linux() {
    match_linux(&[
        ("integers", Library::None),
        ("tables", Library::None),
        ("buffers", Library::None),
        ("walks", Library::None),
    ]);
}

/// Hand-written programs whose callees give the stack pointer back moved.
#[test]
fn stack_moved_by_a_callee_matches_linux() {
    match_linux(&[
        ("callee_moves_sp", Library::None),
        ("callee_moves_sp_away", Library::None),
        ("callee_misaligns_sp", Library::None),
    ]);
}

#[test]
fn benchmarks_match_linux() {
    match_linux(&[("bench", Library::None)]);
}

#[test]
fn blobs_are_told_the_hwcap_of_the_abi() {
    assert_eq!(
        wcontainers_translator_test::layout::BLOB_HWCAP,
        wcontainers_abi::HWCAP
    );
}

#[test]
fn hello_matches_linux() {
    match_linux(&[("hello", Library::C)]);
}

/// Section 8 of the plan: each place ARM64 and WebAssembly differ, printed as bits.
#[test]
fn differences_match_linux() {
    match_linux(&[("differences", Library::C)]);
}

/// The same on a host whose fused multiply-add does not fuse and whose not-a-number results do not
/// follow Arm's rules: the software fused multiply-add and the fix-up give Linux's output too, and the
/// slow paths are counted.
#[test]
fn differences_match_linux_on_other_hosts() {
    use wcontainers_translator_test::programs::{ran, ready_on};
    let host = wcontainers_arm64::Host {
        fused_multiply_add: false,
        arm_nans: false,
    };
    for optimisation in ["-O0", "-O2"] {
        let blob = build("differences", optimisation, Library::C);
        let theirs = native(&blob);
        for look_ahead in [LookAhead::None, LookAhead::Full] {
            let mut machine = ready_on(
                &blob,
                look_ahead,
                wcontainers_translator::Omit::default(),
                host,
            );
            let (stopped, registers) = machine.run();
            let ours = ran(&blob, &machine, stopped, &registers);
            let what = format!("differences at {optimisation}, look-ahead {look_ahead:?}");
            assert_eq!(
                String::from_utf8_lossy(&ours.stdout),
                String::from_utf8_lossy(&theirs.stdout),
                "output of {what}"
            );
            assert_eq!(ours.status, theirs.status, "exit status of {what}");
            assert!(
                machine.count(Reason::SoftwareFusedMultiplyAdd) > 0,
                "{what}"
            );
            assert!(
                machine.count(Reason::ExactFloatingPointRequested) > 0,
                "{what}"
            );
        }
    }
}

/// Every program, at every optimisation level, at look-ahead none and full: the same output and exit status
/// as on Linux, and at full the same registers and memory at the end as at none. Levels that build to the
/// same bytes as an earlier one are run once.
fn match_linux(programs: &[(&str, Library)]) {
    for &(name, library) in programs {
        let mut built: Vec<(&str, Vec<u8>)> = Vec::new();
        for optimisation in LEVELS {
            let blob = build(name, optimisation, library);
            if let Some((same, _)) = built.iter().find(|(_, bytes)| *bytes == blob.bytes) {
                eprintln!("{name} {optimisation}: the same blob as {same}");
                continue;
            }
            built.push((optimisation, blob.bytes.clone()));
            let theirs = native(&blob);
            let mut finals = Vec::new();
            for look_ahead in [LookAhead::None, LookAhead::Full] {
                let (ours, mut machine, registers) = translated(&blob, look_ahead);
                let what = format!("{name} at {optimisation}, look-ahead {look_ahead:?}");
                assert_eq!(
                    String::from_utf8_lossy(&ours.stdout),
                    String::from_utf8_lossy(&theirs.stdout),
                    "output of {what}"
                );
                assert_eq!(ours.stdout, theirs.stdout, "output bytes of {what}");
                assert_eq!(
                    String::from_utf8_lossy(&ours.stderr),
                    String::from_utf8_lossy(&theirs.stderr),
                    "error output of {what}"
                );
                assert_eq!(ours.status, theirs.status, "exit status of {what}");
                eprintln!(
                    "{what}: {} bytes of code, matches, {:?}",
                    blob.code_bytes,
                    machine.counters()
                );
                assert_eq!(machine.count(Reason::UnsupportedInstructionReached), 0);
                finals.push((registers, reachable_memory(&mut machine, &blob)));
            }
            assert_eq!(
                finals[0].0, finals[1].0,
                "registers of {name} at {optimisation}, full against none"
            );
            assert!(
                finals[0].1 == finals[1].1,
                "memory of {name} at {optimisation}, full against none"
            );
        }
        eprintln!(
            "{name}: {} distinct blobs from {} levels",
            built.len(),
            LEVELS.len()
        );
    }
}

/// Stopping at every stop check (so full pieces carry on after the stop hook at every loop turn and entry,
/// entering themselves again) changes nothing: the same output as Linux, and the same registers and memory
/// at the end as at none with no stops.
#[test]
fn stops_at_every_check_change_nothing() {
    for (name, defines) in [("bench", &["SCALE=0"][..]), ("integers", &[][..])] {
        for optimisation in ["-O0", "-O2"] {
            let blob = build_with(name, optimisation, Library::None, defines);
            let theirs = native(&blob);
            let what = format!("{name} at {optimisation}");
            let (none, mut none_machine, none_registers) = translated(&blob, LookAhead::None);
            assert_eq!(none, theirs, "{what} at none");
            let (full, mut machine, registers) =
                translated_stopping_everywhere(&blob, LookAhead::Full);
            assert_eq!(full, theirs, "{what} at full, stopping everywhere");
            assert_eq!(registers, none_registers, "registers of {what}");
            assert!(
                reachable_memory(&mut machine, &blob) == reachable_memory(&mut none_machine, &blob),
                "memory of {what}"
            );
            let stops = machine.count(Reason::StoppedByFlag);
            eprintln!("{what}: {stops} stops");
            assert!(stops > 1000, "{what} stopped only {stops} times");
        }
    }
}

/// Not a check: prints translation speed, output size raw and gzip-compressed, and compile time for every
/// instruction of each distinct program at look-ahead none, best of five runs.
#[test]
fn measure() {
    use std::io::Write;
    use std::time::{Duration, Instant};
    use wcontainers_translator::translate;
    use wcontainers_translator_test::runner::compile_time;
    let gzip = |bytes: &[u8]| {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap().len()
    };
    for (name, optimisation, library) in [
        ("integers", "-O0", Library::None),
        ("integers", "-O2", Library::None),
        ("tables", "-O0", Library::None),
        ("tables", "-O2", Library::None),
        ("hello", "-O2", Library::C),
    ] {
        let blob = build(name, optimisation, library);
        let code = &blob.bytes[..blob.code_bytes as usize];
        let starts: Vec<u64> = (0..blob.code_bytes / 4).map(|i| 4 * i).collect();
        let mut best = (Duration::MAX, Duration::MAX, Vec::new());
        for _ in 0..5 {
            let start = Instant::now();
            let translation = translate(
                &blob.bytes,
                0..blob.code_bytes,
                &starts,
                LookAhead::None,
                wcontainers_translator_test::runner::probe(),
            )
            .unwrap();
            let translating = start.elapsed();
            let compiling = compile_time(&translation.wasm);
            best = (
                best.0.min(translating),
                best.1.min(compiling),
                translation.wasm,
            );
        }
        let wasm = &best.2;
        eprintln!(
            "{name} {optimisation}: {} bytes of code ({} gzipped), {} instructions; translated in {:?} ({:.1} MB/s); {} bytes of WebAssembly ({:.1} per byte of code), {} gzipped ({:.1} per gzipped byte of code); compiled by wasmtime in {:?}",
            code.len(),
            gzip(code),
            starts.len(),
            best.0,
            code.len() as f64 / best.0.as_secs_f64() / 1e6,
            wasm.len(),
            wasm.len() as f64 / code.len() as f64,
            gzip(wasm),
            gzip(wasm) as f64 / gzip(code) as f64,
            best.1
        );
    }
}
