//! C programs give the same output and exit status through the translator as on real ARM64 Linux.

use wcontainers_arm64::Arm64;
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{LookAhead, Optimise};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::mixed::mixed;
use wcontainers_translator_test::programs::{
    Blob, Library, Ran, build, build_with, native, ran, reachable_memory, ready_with, translated,
    translated_for, translated_stopping_everywhere,
};
use wcontainers_translator_test::runner::Machine;

const LEVELS: [&str; 5] = ["-O0", "-O1", "-O2", "-O3", "-Os"];

/// A blob at full look-ahead in Chrome, translated for speed and for size, gives Linux's run.
fn full_matches_in_chrome(blob: &Blob, theirs: &Ran) {
    for optimise in [Optimise::Speed, Optimise::Size] {
        let ours = browser::translated_with::<Arm64>(
            blob,
            (LookAhead::Full, optimise),
            Default::default(),
        );
        assert_eq!(
            ours.ran, *theirs,
            "{} at full for {optimise:?} in Chrome",
            blob.name
        );
    }
}

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
        let mut blob = build_with::<Arm64>(name, optimisation, library, defines);
        blob.stack_bytes = 8 << 20;
        let theirs = native::<Arm64>(&blob);
        every_look_ahead_matches(&blob, &theirs, |what, machine| {
            let left = machine.count(Reason::StackNearlyFull);
            let continued = machine.count(Reason::ContinuedAfterStackLoss);
            eprintln!("{what}: left a nearly full stack {left} times, continued {continued} times");
            if !what.contains("None") {
                assert!(left > 0 && continued > 0, "{what}");
            }
        });
        for how in [
            (LookAhead::None, Optimise::Speed),
            (LookAhead::Full, Optimise::Speed),
            (LookAhead::Full, Optimise::Size),
        ] {
            let ours = browser::translated_with::<Arm64>(&blob, how, Default::default());
            let what = format!("{} at {how:?} in Chrome", blob.name);
            assert_eq!(ours.ran, theirs, "{what}");
            assert!(ours.count("engine stack nearly full") > 0, "{what}");
        }
    }
}

/// A function whose own frame at full is past the engine stack budget runs when the bottom loop calls it,
/// and leaves the stack when it calls itself, at every look-ahead as on Linux.
#[test]
fn a_frame_past_the_budget_matches_linux() {
    let blob = build::<Arm64>("big_frame", "-O0", Library::None);
    let theirs = native::<Arm64>(&blob);
    assert_eq!(theirs.status, 42);
    every_look_ahead_matches(&blob, &theirs, |what, machine| {
        if what.ends_with("Full") {
            assert!(machine.count(Reason::StackNearlyFull) > 0, "{what}");
        }
    });
}

/// setjmp and longjmp in C, C++ exceptions and Rust panics: returns that do not go where their caller
/// expects, at every look-ahead and with pieces mixed and events injected, as on Linux; and in Chrome.
#[test]
fn unwinding_matches_linux() {
    for name in ["jumps", "exceptions", "panics"] {
        for optimisation in ["-O0", "-O2"] {
            let blob = build::<Arm64>(name, optimisation, Library::C);
            let theirs = native::<Arm64>(&blob);
            assert_eq!(
                theirs.status,
                if name == "jumps" { 50 } else { 0 },
                "{}",
                blob.name
            );
            every_look_ahead_matches(&blob, &theirs, print_counters);
            full_matches_in_chrome(&blob, &theirs);
        }
    }
}

/// Programs that move to stacks they made themselves: a fiber switched with setjmp and longjmp, and two
/// coroutines switched by a hand-written context switch, as on Linux at every look-ahead and in Chrome.
#[test]
fn stacks_switched_by_the_program_match_linux() {
    for name in ["fibers", "coroutines"] {
        for optimisation in ["-O0", "-O2"] {
            let blob = build::<Arm64>(name, optimisation, Library::C);
            let theirs = native::<Arm64>(&blob);
            assert_eq!(theirs.status, 0, "{}", blob.name);
            every_look_ahead_matches(&blob, &theirs, print_counters);
            full_matches_in_chrome(&blob, &theirs);
        }
    }
}

/// Code the program writes into its own memory and calls, as a JIT does, translated from memory when it is
/// reached, and written again with other code after `ic ivau`; and a call through a null pointer, which
/// faults as on Linux. At every look-ahead and in Chrome.
#[test]
fn written_code_and_null_calls_match_linux() {
    for (name, status) in [("written_code", 0), ("null_call", 139)] {
        for optimisation in ["-O0", "-O2"] {
            let blob = build::<Arm64>(name, optimisation, Library::C);
            let theirs = native::<Arm64>(&blob);
            assert_eq!(theirs.status, status, "{}", blob.name);
            every_look_ahead_matches(&blob, &theirs, |what, machine| {
                if name == "null_call" {
                    assert!(machine.count(Reason::MemoryFault) == 1, "{what}");
                }
            });
            for look_ahead in [LookAhead::Full, LookAhead::AT_RUN_TIME] {
                for optimise in [Optimise::Speed, Optimise::Size] {
                    let ours = browser::translated_with::<Arm64>(
                        &blob,
                        (look_ahead, optimise),
                        Default::default(),
                    );
                    let what =
                        format!("{} at {look_ahead:?} for {optimise:?} in Chrome", blob.name);
                    assert_eq!(ours.ran, theirs, "{what}");
                }
            }
        }
    }
}

/// A function of more blocks than one piece may hold, cut in parts, at every look-ahead as on Linux, and in
/// Chrome. Then programs whose translation at full is split into many small code modules, so that their
/// pieces call each other and return across modules, as on Linux.
#[test]
fn cut_functions_and_split_translations_match_linux() {
    for optimisation in ["-O0", "-O2"] {
        let blob = build::<Arm64>("many_blocks", optimisation, Library::C);
        let theirs = native::<Arm64>(&blob);
        assert_eq!(theirs.status, 0, "{}", blob.name);
        every_look_ahead_matches(&blob, &theirs, |_, _| {});
        full_matches_in_chrome(&blob, &theirs);
    }
    for (name, optimisation) in [
        ("hello", "-O2"),
        ("differences", "-O0"),
        ("exceptions", "-O2"),
        ("many_blocks", "-O2"),
    ] {
        let blob = build::<Arm64>(name, optimisation, Library::C);
        let theirs = native::<Arm64>(&blob);
        let (none, mut none_machine, none_registers) = translated::<Arm64>(&blob, LookAhead::None);
        assert_eq!(none, theirs, "{} at none", blob.name);
        for module_bytes in [1 << 12, 1 << 16] {
            let modules = wcontainers_translator::translate_in_modules::<wcontainers_arm64::Arm64>(
                &blob.bytes,
                0..blob.code_bytes,
                &[0],
                LookAhead::Full,
                wcontainers_translator::Optimise::Speed,
                wcontainers_translator_test::runner::probe::<Arm64>(),
                (),
                module_bytes,
            )
            .unwrap()
            .modules
            .len();
            assert!(modules > 1, "{} in {modules} modules", blob.name);
            let mut machine = ready_with::<Arm64>(
                &blob,
                wcontainers_translator_test::runner::probe::<Arm64>(),
                |m| {
                    m.load_split(
                        &blob.bytes,
                        0..blob.code_bytes,
                        blob.load,
                        &[0],
                        module_bytes,
                    )
                },
            );
            let (stopped, registers) = machine.run();
            let what = format!("{} in modules of {module_bytes} bytes", blob.name);
            assert_eq!(
                ran::<Arm64>(&blob, &machine, stopped, &registers),
                theirs,
                "{what}"
            );
            assert_eq!(registers, none_registers, "registers of {what}");
            assert!(
                reachable_memory::<Arm64>(&mut machine, &blob)
                    == reachable_memory::<Arm64>(&mut none_machine, &blob),
                "memory of {what}"
            );
        }
    }
}

fn print_counters(what: &str, machine: &Machine<Arm64>) {
    eprintln!("{what}: {:?}", machine.counters());
}

/// A blob at none matches Linux, and at full, at bounded with every piece but the first made at run time,
/// and mixed (seeds 0 to 2, the first with no stops), it matches Linux and none's registers and memory. `check` is called on each run
/// with what it was.
fn every_look_ahead_matches(blob: &Blob, theirs: &Ran, check: impl Fn(&str, &Machine<Arm64>)) {
    let (none, mut none_machine, none_registers) = translated::<Arm64>(blob, LookAhead::None);
    assert_eq!(&none, theirs, "{} at none", blob.name);
    check(&format!("{} at None", blob.name), &none_machine);
    let mut runs = Vec::new();
    for how in [
        (LookAhead::Full, Optimise::Speed),
        (LookAhead::Full, Optimise::Size),
        (LookAhead::AT_RUN_TIME, Optimise::Speed),
    ] {
        let (ours, machine, registers) = translated_for::<Arm64>(blob, how);
        let (look_ahead, optimise) = how;
        runs.push((
            format!("{} at {look_ahead:?} for {optimise:?}", blob.name),
            ours,
            machine,
            registers,
        ));
    }
    for seed in 0..3 {
        let (ours, machine, registers, events) = mixed::<Arm64>(blob, seed);
        let what = format!("{} mixed from {seed} ({events:?})", blob.name);
        runs.push((what, ours, machine, registers));
    }
    for (what, ours, mut machine, registers) in runs {
        assert_eq!(&ours, theirs, "{what}");
        assert_eq!(registers, none_registers, "registers of {what}");
        assert!(
            reachable_memory::<Arm64>(&mut machine, blob)
                == reachable_memory::<Arm64>(&mut none_machine, blob),
            "memory of {what}"
        );
        check(&what, &machine);
    }
}

#[test]
fn blobs_are_told_the_hwcap_of_the_abi() {
    use wcontainers_translator_test::guest::GuestTest;
    use wcontainers_translator_test::layout::AT_HWCAP;
    assert!(Arm64::AUXILIARY.contains(&(AT_HWCAP, wcontainers_abi::HWCAP)));
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
        let blob = build::<Arm64>("differences", optimisation, Library::C);
        let theirs = native::<Arm64>(&blob);
        for look_ahead in [LookAhead::None, LookAhead::Full] {
            let mut machine = ready_on(
                &blob,
                (look_ahead, wcontainers_translator::Optimise::Speed),
                wcontainers_translator::Omit::default(),
                host,
            );
            let (stopped, registers) = machine.run();
            let ours = ran::<Arm64>(&blob, &machine, stopped, &registers);
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
            let blob = build::<Arm64>(name, optimisation, library);
            if let Some((same, _)) = built.iter().find(|(_, bytes)| *bytes == blob.bytes) {
                eprintln!("{name} {optimisation}: the same blob as {same}");
                continue;
            }
            built.push((optimisation, blob.bytes.clone()));
            let theirs = native::<Arm64>(&blob);
            let mut finals = Vec::new();
            for how in [
                (LookAhead::None, Optimise::Speed),
                (LookAhead::Full, Optimise::Speed),
                (LookAhead::Full, Optimise::Size),
            ] {
                let (ours, mut machine, registers) = translated_for::<Arm64>(&blob, how);
                let what = format!("{name} at {optimisation}, {how:?}");
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
                finals.push((registers, reachable_memory::<Arm64>(&mut machine, &blob)));
            }
            for (registers, memory) in &finals[1..] {
                assert_eq!(
                    finals[0].0, *registers,
                    "registers of {name} at {optimisation}, full against none"
                );
                assert!(
                    finals[0].1 == *memory,
                    "memory of {name} at {optimisation}, full against none"
                );
            }
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
            let blob = build_with::<Arm64>(name, optimisation, Library::None, defines);
            let theirs = native::<Arm64>(&blob);
            let what = format!("{name} at {optimisation}");
            let (none, mut none_machine, none_registers) =
                translated::<Arm64>(&blob, LookAhead::None);
            assert_eq!(none, theirs, "{what} at none");
            for optimise in [Optimise::Speed, Optimise::Size] {
                let (full, mut machine, registers) =
                    translated_stopping_everywhere::<Arm64>(&blob, (LookAhead::Full, optimise));
                let what = format!("{what} for {optimise:?}");
                assert_eq!(full, theirs, "{what} at full, stopping everywhere");
                assert_eq!(registers, none_registers, "registers of {what}");
                assert!(
                    reachable_memory::<Arm64>(&mut machine, &blob)
                        == reachable_memory::<Arm64>(&mut none_machine, &blob),
                    "memory of {what}"
                );
                let stops = machine.count(Reason::StoppedByFlag);
                eprintln!("{what}: {stops} stops");
                assert!(stops > 1000, "{what} stopped only {stops} times");
            }
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
        let blob = build::<Arm64>(name, optimisation, library);
        let code = &blob.bytes[..blob.code_bytes as usize];
        let starts: Vec<u64> = (0..blob.code_bytes / 4).map(|i| 4 * i).collect();
        let mut best = (Duration::MAX, Duration::MAX, Vec::new());
        for _ in 0..5 {
            let start = Instant::now();
            let mut translation = translate::<wcontainers_arm64::Arm64>(
                &blob.bytes,
                0..blob.code_bytes,
                &starts,
                LookAhead::None,
                wcontainers_translator::Optimise::Speed,
                wcontainers_translator_test::runner::probe::<Arm64>(),
                (),
            )
            .unwrap();
            let translating = start.elapsed();
            let compiling = compile_time(&translation.modules[0].wasm);
            best = (
                best.0.min(translating),
                best.1.min(compiling),
                translation.modules.remove(0).wasm,
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
    const WORKLOADS: [&str; 4] = ["n-body", "matrix", "vector matrix", "byte differences"];
    let chosen: Vec<usize> = std::env::var("WC_BENCH_WORKLOADS").map_or((1..=4).collect(), |v| {
        v.split(',').map(|n| n.parse().unwrap()).collect()
    });
    for k in chosen {
        let workload = format!("WORKLOAD={k}");
        let blob = build_with::<Arm64>("floats", "-O2", Library::C, &["SCALE=1", &workload]);
        time_against_native(&format!("floats {}", WORKLOADS[k - 1]), &blob);
    }
}

/// Timing of programs that switch stacks themselves, 2,000,000 round trips at -O2: native, and at full in
/// wasmtime and Chrome, with the time of one switch over native's. Knobs as for [`floats_speed`]. Only
/// measures; one command:
/// `nix develop -c cargo test --release -p wcontainers-translator-test --test programs switching_speed -- --ignored --nocapture`.
#[test]
#[ignore]
fn switching_speed() {
    const ROUND_TRIPS: f64 = 2_000_000.0;
    for name in ["fibers", "coroutines"] {
        let blob = build_with::<Arm64>(name, "-O2", Library::C, &["SWITCHES=2000000"]);
        let (native, wasmtime, chrome) = time_against_native(name, &blob);
        // Two switches a round trip, two coroutines in `coroutines`.
        let switches = if name == "fibers" { 2.0 } else { 4.0 } * ROUND_TRIPS;
        let each = |seconds: f64| (seconds - native) / switches * 1e9;
        eprintln!(
            "{name}: one switch costs {:.0} ns more than native in wasmtime, {} in Chrome",
            each(wasmtime),
            chrome.map_or("not run".to_string(), |c| format!("{:.0} ns", each(c)))
        );
    }
}

/// Times a blob natively and at full in wasmtime and (unless `WC_BENCH_CHROME=0`) Chrome, best of
/// `WC_BENCH_RUNS` (default 3), checking each run's result; prints and returns the seconds.
fn time_against_native(name: &str, blob: &Blob) -> (f64, f64, Option<f64>) {
    use wcontainers_translator_test::programs::{native_seconds, translated_omitting};
    let runs = std::env::var("WC_BENCH_RUNS").map_or(3, |v| v.parse().unwrap());
    let chrome = std::env::var("WC_BENCH_CHROME").map_or(true, |v| v != "0");
    let load = || {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .expect("sysctl");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    let theirs = native::<Arm64>(blob);
    let native = native_seconds::<Arm64>(blob, runs);
    let mut best = f64::MAX;
    for _ in 0..runs {
        let (ours, _, _, time) = translated_omitting::<Arm64>(
            blob,
            (LookAhead::Full, wcontainers_translator::Optimise::Speed),
            wcontainers_translator::Omit::default(),
        );
        assert_eq!(ours, theirs);
        best = best.min(time.as_secs_f64());
    }
    eprintln!(
        "{name}: native {native:.3} s, wasmtime {best:.3} s, {:.2} times native (load {})",
        best / native,
        load()
    );
    if !chrome {
        return (native, best, None);
    }
    let mut in_chrome = f64::MAX;
    for _ in 0..runs {
        let report = browser::repeated::<Arm64>(
            blob,
            (LookAhead::Full, wcontainers_translator::Optimise::Speed),
            wcontainers_translator::Omit::default(),
            1,
        );
        assert_eq!(report.ran, theirs, "{name} in Chrome");
        in_chrome = in_chrome.min(report.runs[0]);
    }
    eprintln!(
        "{name}: Chrome {in_chrome:.3} s, {:.2} times native (load {})",
        in_chrome / native,
        load()
    );
    (native, best, Some(in_chrome))
}
