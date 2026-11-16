//! C programs give the same output and exit status through the translator as on real ARM64 Linux.

use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::browser;
use wcontainers_translator_test::mixed::mixed;
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

/// Recursion far deeper than the engine's own stack allows, on Linux's 8 MiB stack: C 150,000 calls deep with
/// a frame at each level, and a million calls deep through a stack of 8 byte frames. Each look-ahead
/// matches Linux, and the thread leaves the full engine stack to the bottom loop, counted, and carries on.
#[test]
fn deep_recursion_matches_linux() {
    for (name, optimisation, library, defines) in [
        ("deep", "-O0", Library::C, &["DEPTH=150000"][..]),
        ("deep", "-O2", Library::C, &["DEPTH=150000"][..]),
        ("deep_small_frames", "-O0", Library::None, &[][..]),
    ] {
        let mut blob = build_with(name, optimisation, library, defines);
        blob.stack_bytes = 8 << 20;
        let theirs = native(&blob);
        let (none, mut none_machine, none_registers) = translated(&blob, LookAhead::None);
        assert_eq!(none, theirs, "{} at none", blob.name);
        for look_ahead in [LookAhead::Full, LookAhead::AT_RUN_TIME] {
            let what = format!("{} at {look_ahead:?}", blob.name);
            let (ours, mut machine, registers) = translated(&blob, look_ahead);
            assert_eq!(ours, theirs, "{what}");
            assert_eq!(registers, none_registers, "registers of {what}");
            assert!(
                reachable_memory(&mut machine, &blob) == reachable_memory(&mut none_machine, &blob),
                "memory of {what}"
            );
            let left = machine.count(Reason::StackNearlyFull);
            let continued = machine.count(Reason::ContinuedAfterStackLoss);
            eprintln!("{what}: left a nearly full stack {left} times, continued {continued} times");
            assert!(left > 0 && continued > 0, "{what}");
        }
        for look_ahead in [LookAhead::None, LookAhead::Full] {
            let ours = browser::translated_with(&blob, look_ahead, Default::default());
            let what = format!("{} at {look_ahead:?} in Chrome", blob.name);
            assert_eq!(ours.ran, theirs, "{what}");
            assert!(ours.count("engine stack nearly full") > 0, "{what}");
        }
    }
}

/// setjmp and longjmp in C, C++ exceptions and Rust panics: returns that do not go where their caller
/// expects, at every look-ahead and with pieces mixed and events injected, as on Linux; and in Chrome.
#[test]
fn unwinding_matches_linux() {
    for name in ["jumps", "exceptions", "panics"] {
        for optimisation in ["-O0", "-O2"] {
            let blob = build(name, optimisation, Library::C);
            let theirs = native(&blob);
            assert_eq!(
                theirs.status,
                if name == "jumps" { 50 } else { 0 },
                "{}",
                blob.name
            );
            let (none, mut none_machine, none_registers) = translated(&blob, LookAhead::None);
            assert_eq!(none, theirs, "{} at none", blob.name);
            for look_ahead in [LookAhead::Full, LookAhead::AT_RUN_TIME] {
                let what = format!("{} at {look_ahead:?}", blob.name);
                let (ours, mut machine, registers) = translated(&blob, look_ahead);
                assert_eq!(ours, theirs, "{what}");
                assert_eq!(registers, none_registers, "registers of {what}");
                assert!(
                    reachable_memory(&mut machine, &blob)
                        == reachable_memory(&mut none_machine, &blob),
                    "memory of {what}"
                );
                eprintln!(
                    "{what}: left to an address {} times, continued after stack loss {} times",
                    machine.count(Reason::LeftToAddress),
                    machine.count(Reason::ContinuedAfterStackLoss)
                );
            }
            for seed in 0..2 {
                let (ours, mut machine, registers, events) = mixed(&blob, seed);
                let what = format!("{} mixed from {seed} ({events:?})", blob.name);
                assert_eq!(ours, theirs, "{what}");
                assert_eq!(registers, none_registers, "registers of {what}");
                assert!(
                    reachable_memory(&mut machine, &blob)
                        == reachable_memory(&mut none_machine, &blob),
                    "memory of {what}"
                );
            }
            let ours = browser::translated_with(&blob, LookAhead::Full, Default::default());
            assert_eq!(ours.ran, theirs, "{} at full in Chrome", blob.name);
        }
    }
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
            // At full the slow path is reached by leaving the piece.
            if look_ahead == LookAhead::Full {
                assert!(machine.count(Reason::LeftToAddress) > 0, "{what}");
            }
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

/// Timing of the floating point and vector workloads (`programs/floats.c`, SCALE=1) at -O2, each on its own:
/// native, and through the translator at full in wasmtime and Chrome, best of `WC_BENCH_RUNS` (default 3).
/// `WC_BENCH_WORKLOADS` picks workloads (default 1,2,3,4), `WC_BENCH_CHROME=0` leaves Chrome out. Only
/// measures; one command:
/// `nix develop -c cargo test --release -p wcontainers-translator-test --test programs floats_speed -- --ignored --nocapture`.
#[test]
#[ignore]
fn floats_speed() {
    use wcontainers_translator_test::browser;
    use wcontainers_translator_test::programs::{native_seconds, translated_omitting};
    const WORKLOADS: [&str; 4] = ["n-body", "matrix", "vector matrix", "byte differences"];
    let runs = std::env::var("WC_BENCH_RUNS").map_or(3, |v| v.parse().unwrap());
    let chosen: Vec<usize> = std::env::var("WC_BENCH_WORKLOADS").map_or((1..=4).collect(), |v| {
        v.split(',').map(|n| n.parse().unwrap()).collect()
    });
    let chrome = std::env::var("WC_BENCH_CHROME").map_or(true, |v| v != "0");
    let load = || {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .expect("sysctl");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    for k in chosen {
        let name = WORKLOADS[k - 1];
        let workload = format!("WORKLOAD={k}");
        let blob = build_with("floats", "-O2", Library::C, &["SCALE=1", &workload]);
        let theirs = native(&blob);
        let native = native_seconds(&blob, runs);
        let mut best = f64::MAX;
        for _ in 0..runs {
            let (ours, _, _, time) = translated_omitting(
                &blob,
                LookAhead::Full,
                wcontainers_translator::Omit::default(),
            );
            assert_eq!(ours, theirs);
            best = best.min(time.as_secs_f64());
        }
        eprintln!(
            "floats {name}: native {native:.3} s, wasmtime {best:.3} s, {:.2} times native (load {})",
            best / native,
            load()
        );
        if !chrome {
            continue;
        }
        let mut best = f64::MAX;
        for _ in 0..runs {
            let report = browser::repeated(
                &blob,
                LookAhead::Full,
                wcontainers_translator::Omit::default(),
                1,
            );
            assert_eq!(report.ran, theirs, "{name} in Chrome");
            best = best.min(report.runs[0]);
        }
        eprintln!(
            "floats {name}: Chrome {best:.3} s, {:.2} times native (load {})",
            best / native,
            load()
        );
    }
}
