//! C programs give the same output and exit status through the translator as on real Linux of the guest's
//! CPU. Each test is written once over the guest and run for each.

use wcontainers_arm64::Arm64;
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{FUNCTION_BLOCKS, LookAhead, Optimise};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::mixed::mixed;
use wcontainers_translator_test::programs::{
    Blob, Library, Position, Ran, boundaries, build, build_for, build_with, native,
    native_with_stop, ran, reachable_memory, ready_on, ready_with, run_container, translated,
    translated_for, translated_stopping_everywhere,
};
use wcontainers_translator_test::reference::{DOCKER_MOUNT, IMAGE};
use wcontainers_translator_test::runner::Machine;
use wcontainers_x86::X86_64;

const LEVELS: [&str; 5] = ["-O0", "-O1", "-O2", "-O3", "-Os"];

/// Both ways a C program is built: x86's fixed-address code takes a function's address as an absolute
/// immediate, which the translation follows through the link address, and its position independent code
/// builds every address relative to the instruction.
const POSITIONS: [Position; 2] = [Position::Fixed, Position::Independent];

/// The look-aheads every program runs at: none, full for speed and for size, and bounded with every piece but
/// the first made at run time.
const EVERY_LOOK_AHEAD: [(LookAhead, Optimise); 4] = [
    (LookAhead::None, Optimise::Speed),
    (LookAhead::Full, Optimise::Speed),
    (LookAhead::Full, Optimise::Size),
    (LookAhead::AT_RUN_TIME, Optimise::Speed),
];

/// The optimisations the rare-path programs are built at, each in both positions.
fn levels_and_positions() -> impl Iterator<Item = (&'static str, Position)> {
    ["-O0", "-O2"]
        .into_iter()
        .flat_map(|level| POSITIONS.map(|position| (level, position)))
}

#[test]
#[should_panic(expected = "Docker failed")]
fn a_container_docker_does_not_make_fails_the_case() {
    run_container("unmade", |command| {
        command.args([DOCKER_MOUNT, "/r:/r:not-a-mode", IMAGE, "true"]);
    });
}

#[test]
#[should_panic(expected = "Docker failed")]
fn a_container_that_does_not_start_fails_the_case() {
    run_container("unstarted", |command| {
        command.args([IMAGE, "/no-such-command"]);
    });
}

#[test]
fn a_programs_own_ending_is_its_result() {
    let output = run_container("own-ending", |command| {
        command.args([IMAGE, "sh", "-c", "exit 125"]);
    });
    assert_eq!(output.status.code(), Some(125));
}

/// A blob in Chrome at every look-ahead gives Linux's run. Returns each run's report, with what it was.
fn every_look_ahead_matches_in_chrome<G: GuestTest>(
    blob: &Blob,
    theirs: &Ran,
) -> Vec<(String, browser::Report)> {
    EVERY_LOOK_AHEAD
        .into_iter()
        .map(|how| {
            let ours = browser::translated_with::<G>(blob, how, Default::default());
            let what = format!("{} at {how:?} in Chrome", blob.name);
            assert_eq!(ours.ran, *theirs, "{what}");
            (what, ours)
        })
        .collect()
}

fn programs_match_linux<G: GuestTest>() {
    match_linux::<G>(&[
        ("integers", Library::None),
        ("tables", Library::None),
        ("buffers", Library::None),
        ("walks", Library::None),
    ]);
}

/// Hand-written programs whose callees give the stack pointer back moved.
fn stack_moved_by_a_callee_matches_linux<G: GuestTest>() {
    match_linux::<G>(&[
        ("callee_moves_sp", Library::None),
        ("callee_moves_sp_away", Library::None),
        ("callee_misaligns_sp", Library::None),
    ]);
}

/// FPSR's flags read without the program clearing or setting them first, as on Linux; and in Chrome.
#[test]
fn float_status_read_first_matches_linux_arm64() {
    match_linux::<Arm64>(&[("float_status", Library::C)]);
    let blob = build::<Arm64>("float_status", "-O2", Library::C);
    every_look_ahead_matches_in_chrome::<Arm64>(&blob, &native::<Arm64>(&blob));
}

fn benchmarks_match_linux<G: GuestTest>() {
    match_linux::<G>(&[("bench", Library::None)]);
}

/// Recursion far deeper than the engine's own stack allows, on Linux's 8 MiB stack: C 150,000 calls deep with
/// a frame at each level, and a million calls deep through a stack of 8 byte frames. Each look-ahead
/// matches Linux, and the thread leaves the full engine stack to the bottom loop, counted, and carries on.
fn deep_recursion_matches_linux<G: GuestTest>() {
    for (name, optimisation, library, defines) in [
        ("deep", "-O0", Library::C, &["DEPTH=150000"][..]),
        ("deep", "-O2", Library::C, &["DEPTH=150000"][..]),
        ("deep_small_frames", "-O0", Library::None, &[][..]),
    ] {
        let mut blob = build_with::<G>(name, optimisation, library, defines, Position::Fixed);
        blob.stack_bytes = 8 << 20;
        let theirs = native::<G>(&blob);
        every_look_ahead_matches::<G>(&blob, &theirs, |what, machine| {
            let left = machine.count(Reason::StackNearlyFull);
            let continued = machine.count(Reason::ContinuedAfterStackLoss);
            eprintln!("{what}: left a nearly full stack {left} times, continued {continued} times");
            if !what.contains("None") {
                assert!(left > 0 && continued > 0, "{what}");
            }
        });
        let nearly_full = Reason::StackNearlyFull.name();
        for (what, ours) in every_look_ahead_matches_in_chrome::<G>(&blob, &theirs) {
            assert!(ours.count(nearly_full) > 0, "{what}");
        }
    }
}

/// A function whose own frame at full is past the engine stack budget runs when the bottom loop calls it,
/// and leaves the stack when it calls itself, at every look-ahead as on Linux.
fn a_frame_past_the_budget_matches_linux<G: GuestTest>() {
    let blob = build::<G>("big_frame", "-O0", Library::None);
    let theirs = native::<G>(&blob);
    assert_eq!(theirs.status, 42);
    every_look_ahead_matches::<G>(&blob, &theirs, |what, machine| {
        if what.ends_with("Full") {
            assert!(machine.count(Reason::StackNearlyFull) > 0, "{what}");
        }
    });
}

/// setjmp and longjmp in C, C++ exceptions and Rust panics: returns that do not go where their caller
/// expects, at every look-ahead and with pieces mixed and events injected, as on Linux; and in Chrome.
fn unwinding_matches_linux<G: GuestTest>() {
    for name in ["jumps", "exceptions", "panics"] {
        for (optimisation, position) in levels_and_positions() {
            let blob = build_with::<G>(name, optimisation, Library::C, &[], position);
            // rustc's aarch64 musl target makes a fixed-address program whatever it is asked.
            if blob.load != position.load() {
                eprintln!("{}: not made position independent, not run", blob.name);
                continue;
            }
            let theirs = native::<G>(&blob);
            assert_eq!(
                theirs.status,
                if name == "jumps" { 50 } else { 0 },
                "{}",
                blob.name
            );
            every_look_ahead_matches::<G>(&blob, &theirs, print_counters::<G>);
            every_look_ahead_matches_in_chrome::<G>(&blob, &theirs);
        }
    }
}

/// Programs that move to stacks they made themselves: a fiber switched with setjmp and longjmp, and two
/// coroutines switched by a hand-written context switch, as on Linux at every look-ahead and in Chrome.
fn stacks_switched_by_the_program_match_linux<G: GuestTest>() {
    for name in ["fibers", "coroutines"] {
        for (optimisation, position) in levels_and_positions() {
            let blob = build_with::<G>(name, optimisation, Library::C, &[], position);
            let theirs = native::<G>(&blob);
            assert_eq!(theirs.status, 0, "{}", blob.name);
            every_look_ahead_matches::<G>(&blob, &theirs, print_counters::<G>);
            every_look_ahead_matches_in_chrome::<G>(&blob, &theirs);
        }
    }
}

/// Code the program writes into its own memory and calls, as a JIT does, translated from memory when it is
/// reached, and written again with other code after `ic ivau`; and a call through a null pointer, which
/// faults as on Linux. At every look-ahead and in Chrome.
fn written_code_and_null_calls_match_linux<G: GuestTest>() {
    for (name, status) in [("written_code", 0), ("null_call", 139)] {
        for (optimisation, position) in levels_and_positions() {
            let blob = build_with::<G>(name, optimisation, Library::C, &[], position);
            let theirs = native::<G>(&blob);
            assert_eq!(theirs.status, status, "{}", blob.name);
            every_look_ahead_matches::<G>(&blob, &theirs, |what, machine| {
                if name == "null_call" {
                    assert!(machine.count(Reason::MemoryFault) == 1, "{what}");
                }
            });
            every_look_ahead_matches_in_chrome::<G>(&blob, &theirs);
        }
    }
}

/// A function of more blocks than one piece may hold, cut in parts, at every look-ahead as on Linux, and in
/// Chrome. Then programs (`split`, each with its optimisation) whose translation at full is split into many
/// small code modules, so that their pieces call each other and return across modules, as on Linux.
fn cut_functions_and_split_translations_match_linux<G: GuestTest>(split: &[(&str, &str)]) {
    for optimisation in ["-O0", "-O2"] {
        let blob = build::<G>("many_blocks", optimisation, Library::C);
        let theirs = native::<G>(&blob);
        assert_eq!(theirs.status, 0, "{}", blob.name);
        every_look_ahead_matches::<G>(&blob, &theirs, |_, _| {});
        every_look_ahead_matches_in_chrome::<G>(&blob, &theirs);
    }
    for &(name, optimisation) in split {
        let blob = build::<G>(name, optimisation, Library::C);
        let theirs = native::<G>(&blob);
        let (none, mut none_machine, none_registers) = translated::<G>(&blob, LookAhead::None);
        assert_eq!(none, theirs, "{} at none", blob.name);
        for module_bytes in [1 << 12, 1 << 16] {
            let modules = wcontainers_translator::translate_in_modules::<G>(
                &blob.bytes,
                0..blob.code_bytes,
                blob.link_address,
                &[0],
                LookAhead::Full,
                wcontainers_translator::Optimise::Speed,
                wcontainers_translator::ProgressCount::Absent,
                wcontainers_translator_test::runner::probe::<G>(),
                G::Offer::default(),
                module_bytes,
            )
            .unwrap()
            .modules
            .len();
            assert!(modules > 1, "{} in {modules} modules", blob.name);
            let mut machine = ready_with::<G>(
                &blob,
                wcontainers_translator_test::runner::probe::<G>(),
                |m| {
                    m.load_split(
                        &blob.bytes,
                        0..blob.code_bytes,
                        (blob.load, blob.link_address),
                        &[0],
                        module_bytes,
                    )
                },
            );
            let (stopped, registers) = machine.run();
            let what = format!("{} in modules of {module_bytes} bytes", blob.name);
            assert_eq!(
                ran::<G>(&blob, &machine, stopped, &registers),
                theirs,
                "{what}"
            );
            assert_eq!(registers, none_registers, "registers of {what}");
            assert!(
                reachable_memory::<G>(&mut machine, &blob)
                    == reachable_memory::<G>(&mut none_machine, &blob),
                "memory of {what}"
            );
        }
    }
}

/// The rare-path C and C++ programs shared by every guest. (Rust's `panics` is left out: the start code
/// rustc links takes `main` from the global offset table, an address held in data, which no walk follows.)
const RARE_PATH_C_PROGRAMS: [&str; 6] = [
    "written_code",
    "null_call",
    "jumps",
    "exceptions",
    "fibers",
    "coroutines",
];

/// The rare-path C programs, those every guest has and the guest's own (`more`, with their definitions), at
/// each optimisation in each of `positions`: the translation made ahead of time at full look-ahead from the
/// entry, given the link address, has a piece for `main`, which a program linked at a fixed address may
/// reach only through an absolute address (musl's `_start_c` passes it as one on x86). Without the link
/// address a fixed build has that piece exactly when `without_link_address` says.
fn main_is_in_the_full_translation<G: GuestTest>(
    more: &[(&str, &[&str])],
    positions: &[Position],
    without_link_address: bool,
) {
    let shared = RARE_PATH_C_PROGRAMS.map(|name| (name, &[][..]));
    for &(name, defines) in shared.iter().chain(more) {
        for (optimisation, position) in levels_and_positions() {
            if !positions.contains(&position) {
                continue;
            }
            let blob = build_with::<G>(name, optimisation, Library::C, defines, position);
            let main = blob.main.expect("a main");
            let has_main = |link_address| {
                wcontainers_translator::translate::<G>(
                    &blob.bytes,
                    0..blob.code_bytes,
                    link_address,
                    &[0],
                    LookAhead::Full,
                    Optimise::Speed,
                    wcontainers_translator::ProgressCount::Absent,
                    wcontainers_translator_test::runner::probe::<G>(),
                    G::Offer::default(),
                )
                .unwrap()
                .pieces()
                .any(|piece| piece == main)
            };
            assert!(
                has_main(blob.link_address),
                "{}: no piece for main at {main:#x}",
                blob.name
            );
            if position == Position::Fixed {
                assert_eq!(
                    has_main(None),
                    without_link_address,
                    "{} without the link address",
                    blob.name
                );
            }
        }
    }
}

fn print_counters<G: GuestTest>(what: &str, machine: &Machine<G>) {
    eprintln!("{what}: {:?}", machine.counters());
}

/// A blob at none matches Linux, and at full, at bounded with every piece but the first made at run time,
/// and mixed (seeds 0 to 2, the first with no stops), it matches Linux and none's registers and memory. `check` is called on each run
/// with what it was.
fn every_look_ahead_matches<G: GuestTest>(
    blob: &Blob,
    theirs: &Ran,
    check: impl Fn(&str, &Machine<G>),
) {
    let (none, mut none_machine, none_registers) = translated::<G>(blob, LookAhead::None);
    assert_eq!(&none, theirs, "{} at none", blob.name);
    check(&format!("{} at None", blob.name), &none_machine);
    let mut runs = Vec::new();
    for &how in &EVERY_LOOK_AHEAD[1..] {
        let (ours, machine, registers) = translated_for::<G>(blob, how);
        let (look_ahead, optimise) = how;
        runs.push((
            format!("{} at {look_ahead:?} for {optimise:?}", blob.name),
            ours,
            machine,
            registers,
        ));
    }
    for seed in 0..3 {
        let (ours, machine, registers, events) = mixed::<G>(blob, seed);
        let what = format!("{} mixed from {seed} ({events:?})", blob.name);
        runs.push((what, ours, machine, registers));
    }
    for (what, ours, mut machine, registers) in runs {
        assert_eq!(&ours, theirs, "{what}");
        assert_eq!(registers, none_registers, "registers of {what}");
        assert!(
            reachable_memory::<G>(&mut machine, blob)
                == reachable_memory::<G>(&mut none_machine, blob),
            "memory of {what}"
        );
        check(&what, &machine);
    }
}

fn hello_matches_linux<G: GuestTest>() {
    match_linux::<G>(&[("hello", Library::C)]);
}

/// Each place the guest's CPU and WebAssembly differ (the guest's `differences` program), printed as bits.
fn differences_match_linux<G: GuestTest>() {
    match_linux::<G>(&[("differences", Library::C)]);
}

/// The same translated for every answer the host probe can give, at none and full, built with `defines`:
/// whatever the host lacks is made up by slow paths that give Linux's output too. `check` is called on each
/// run with its host and the blob.
fn differences_match_linux_on_every_host<G: GuestTest>(
    defines: &[&str],
    check: impl Fn(G::Host, &str, &Blob, &Machine<G>),
) {
    for optimisation in ["-O0", "-O2"] {
        let blob = build_with::<G>(
            "differences",
            optimisation,
            Library::C,
            defines,
            Position::Fixed,
        );
        let theirs = native::<G>(&blob);
        for &answer in G::PROBE_ANSWERS {
            let host = G::host_from_probe(answer);
            for look_ahead in [LookAhead::None, LookAhead::Full] {
                let mut machine = ready_on::<G>(
                    &blob,
                    (look_ahead, Optimise::Speed),
                    wcontainers_translator::Omit::default(),
                    host,
                );
                let (stopped, registers) = machine.run();
                let ours = ran::<G>(&blob, &machine, stopped, &registers);
                let what = format!(
                    "differences {defines:?} at {optimisation}, {host:?}, look-ahead {look_ahead:?}"
                );
                assert_eq!(
                    String::from_utf8_lossy(&ours.stdout),
                    String::from_utf8_lossy(&theirs.stdout),
                    "output of {what}"
                );
                assert_eq!(ours.status, theirs.status, "exit status of {what}");
                check(host, &what, &blob, &machine);
            }
        }
    }
}

/// Every program, at every optimisation level, at look-ahead none and full: the same output and exit status
/// as on Linux, and at full the same registers and memory at the end as at none. Levels that build to the
/// same bytes as an earlier one are run once.
fn match_linux<G: GuestTest>(programs: &[(&str, Library)]) {
    match_linux_at::<G>(
        programs,
        &[
            (LookAhead::None, Optimise::Speed),
            (LookAhead::Full, Optimise::Speed),
            (LookAhead::Full, Optimise::Size),
        ],
    );
}

/// The same at these look-aheads only, the first of which the others' registers and memory are compared with.
fn match_linux_at<G: GuestTest>(programs: &[(&str, Library)], hows: &[(LookAhead, Optimise)]) {
    match_linux_told::<G>(programs, hows, None);
}

/// [`match_linux_at`] for programs told the offer `offer` names (the default when `None`).
fn match_linux_told<G: GuestTest>(
    programs: &[(&str, Library)],
    hows: &[(LookAhead, Optimise)],
    offer: Option<u64>,
) {
    for &(name, library) in programs {
        let mut built: Vec<(&str, Vec<u8>)> = Vec::new();
        for optimisation in LEVELS {
            let mut blob = build::<G>(name, optimisation, library);
            blob.offer = offer;
            if let Some((same, _)) = built.iter().find(|(_, bytes)| *bytes == blob.bytes) {
                eprintln!("{name} {optimisation}: the same blob as {same}");
                continue;
            }
            built.push((optimisation, blob.bytes.clone()));
            let theirs = native::<G>(&blob);
            let mut finals = Vec::new();
            for &how in hows {
                let (ours, mut machine, registers) = translated_for::<G>(&blob, how);
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
                finals.push((registers, reachable_memory::<G>(&mut machine, &blob)));
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
/// Stops at every check change nothing in the shared programs and in the guest's own `programs`.
fn stops_at_every_check_change_nothing<G: GuestTest>(programs: &[(&str, &[&str])]) {
    let shared: [(&str, &[&str]); 2] = [("bench", &["SCALE=0"]), ("integers", &[])];
    for &(name, defines) in shared.iter().chain(programs) {
        for optimisation in ["-O0", "-O2"] {
            let blob = build_with::<G>(name, optimisation, Library::None, defines, Position::Fixed);
            let theirs = native::<G>(&blob);
            let what = format!("{name} at {optimisation}");
            let (none, mut none_machine, none_registers) = translated::<G>(&blob, LookAhead::None);
            assert_eq!(none, theirs, "{what} at none");
            for optimise in [Optimise::Speed, Optimise::Size] {
                let (full, mut machine, registers) =
                    translated_stopping_everywhere::<G>(&blob, (LookAhead::Full, optimise));
                let what = format!("{what} for {optimise:?}");
                assert_eq!(full, theirs, "{what} at full, stopping everywhere");
                assert_eq!(registers, none_registers, "registers of {what}");
                assert!(
                    reachable_memory::<G>(&mut machine, &blob)
                        == reachable_memory::<G>(&mut none_machine, &blob),
                    "memory of {what}"
                );
                let stops = machine.count(Reason::StoppedByFlag);
                eprintln!("{what}: {stops} stops");
                assert!(stops > 1000, "{what} stopped only {stops} times");
            }
        }
    }
}

/// Not a check: prints translation speed, output size raw and gzip-compressed, and compile time of each
/// distinct program at look-ahead none (every instruction) and full (from the entry), best of five runs.
fn measure<G: GuestTest>() {
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
        ("bench", "-O2", Library::None),
    ] {
        let blob = build::<G>(name, optimisation, library);
        let code = &blob.bytes[..blob.code_bytes as usize];
        let starts = boundaries::<G>(code);
        // None from every instruction, full from the entry, as programs are run.
        for (look_ahead, from) in [(LookAhead::None, &starts[..]), (LookAhead::Full, &[0][..])] {
            let mut best = (Duration::MAX, Duration::MAX, Vec::new());
            for _ in 0..5 {
                let start = Instant::now();
                let mut translation = translate::<G>(
                    &blob.bytes,
                    0..blob.code_bytes,
                    blob.link_address,
                    from,
                    look_ahead,
                    wcontainers_translator::Optimise::Speed,
                    wcontainers_translator::ProgressCount::Absent,
                    wcontainers_translator_test::runner::probe::<G>(),
                    G::Offer::default(),
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
                "{name} {optimisation} at {look_ahead:?}: {} bytes of code ({} gzipped), {} instructions; translated in {:?} ({:.1} MB/s); {} bytes of WebAssembly ({:.1} per byte of code), {} gzipped ({:.1} per gzipped byte of code); compiled by wasmtime in {:?}",
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
}

/// Timing of the floating point and vector workloads (`programs/floats.c`, SCALE=1) at -O2, each on its own:
/// native, and through the translator at full in wasmtime and Chrome, best of `WC_BENCH_RUNS` (default 3).
/// `WC_BENCH_WORKLOADS` picks workloads (default 1,2,3,4), `WC_BENCH_CHROME=0` leaves Chrome out. Only
/// measures; one command:
/// `nix develop -c cargo test --release -p wcontainers-translator-test --test programs floats_speed_arm64 -- --ignored --nocapture`.
fn floats_speed<G: GuestTest>() {
    const WORKLOADS: [&str; 4] = ["n-body", "matrix", "vector matrix", "byte differences"];
    let chosen: Vec<usize> = std::env::var("WC_BENCH_WORKLOADS").map_or((1..=4).collect(), |v| {
        v.split(',').map(|n| n.parse().unwrap()).collect()
    });
    for k in chosen {
        let workload = format!("WORKLOAD={k}");
        let blob = build_with::<G>(
            "floats",
            "-O2",
            Library::C,
            &["SCALE=1", &workload],
            Position::Fixed,
        );
        time_against_native::<G>(&format!("floats {}", WORKLOADS[k - 1]), &blob);
    }
}

/// Timing of programs that switch stacks themselves, 2,000,000 round trips at -O2: native, and at full in
/// wasmtime and Chrome, with the time of one switch over native's. Knobs as for [`floats_speed`]. Only
/// measures; one command:
/// `nix develop -c cargo test --release -p wcontainers-translator-test --test programs switching_speed_arm64 -- --ignored --nocapture`.
fn switching_speed<G: GuestTest>() {
    const ROUND_TRIPS: f64 = 2_000_000.0;
    for name in ["fibers", "coroutines"] {
        let blob = build_with::<G>(
            name,
            "-O2",
            Library::C,
            &["SWITCHES=2000000"],
            Position::Fixed,
        );
        let (native, wasmtime, chrome) = time_against_native::<G>(name, &blob);
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
fn time_against_native<G: GuestTest>(name: &str, blob: &Blob) -> (f64, f64, Option<f64>) {
    use wcontainers_translator_test::programs::{load, native_seconds, translated_omitting};
    let runs = std::env::var("WC_BENCH_RUNS").map_or(3, |v| v.parse().unwrap());
    let chrome = std::env::var("WC_BENCH_CHROME").map_or(true, |v| v != "0");
    let theirs = native::<G>(blob);
    let native = native_seconds::<G>(blob, runs);
    let mut best = f64::MAX;
    for _ in 0..runs {
        let (ours, _, _, time) = translated_omitting::<G>(
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
        let report = browser::repeated::<G>(
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

/// A jump through a register to outside its function (past the next certain start of a function: one given or
/// a direct call's target), or to its own start, is a tail call, no slow path; one to inside its function that
/// no table explains, in its own part or in another of a function cut in parts, is, counted each time it runs;
/// a switch whose table's address its function keeps in its frame is a table explained. The program makes
/// `ROUNDS` of each, for speed and for size. ARM64's own program.
#[test]
fn only_jumps_inside_their_function_are_unrecognised() {
    register_jumps_counted(&[]);
}

/// [`only_jumps_inside_their_function_are_unrecognised`] with the tail call's target right after its caller,
/// named only by data.
#[test]
#[ignore = "known failure: a tail call to a function only data names, before the next certain start, is counted as a jump inside its caller"]
fn a_tail_call_to_a_function_only_data_names_is_not_unrecognised() {
    register_jumps_counted(&["STEP_AFTER_CALLER"]);
}

/// Runs `register_jumps` with `layout` among its defines and checks only its jumps inside are counted.
fn register_jumps_counted(layout: &[&str]) {
    const ROUNDS: u64 = 1200;
    const JUMPS_INSIDE: u64 = 2;
    // The part jumped into is the last of a function cut in two.
    let far_after = FUNCTION_BLOCKS / 8;
    let sizes = [
        format!("ROUNDS={ROUNDS}"),
        format!("FAR_BEFORE={FUNCTION_BLOCKS}"),
        format!("FAR_AFTER={far_after}"),
    ];
    let defines: Vec<&str> = sizes
        .iter()
        .map(String::as_str)
        .chain(layout.iter().copied())
        .collect();
    let blob = build_with::<Arm64>(
        "register_jumps",
        "-O0",
        Library::None,
        &defines,
        Position::Fixed,
    );
    let theirs = native::<Arm64>(&blob);
    for optimise in [Optimise::Speed, Optimise::Size] {
        let (ours, machine, _) = translated_for::<Arm64>(&blob, (LookAhead::Full, optimise));
        assert_eq!(ours, theirs, "for {optimise:?}");
        assert_eq!(
            machine.count(Reason::JumpThroughRegisterNotRecognised),
            JUMPS_INSIDE * ROUNDS,
            "for {optimise:?}"
        );
    }
}

/// A switch whose cases lie in other parts of a function cut in parts jumps to them as tail calls, no slow
/// path, as on Linux: none counted, and no more code translated at run time than for hello world (the C
/// library's own).
#[test]
fn switch_cases_in_other_parts_are_not_slow() {
    for optimisation in ["-O0", "-O2"] {
        let blob = build::<Arm64>("cut_switch", optimisation, Library::C);
        let theirs = native::<Arm64>(&blob);
        let hello = build::<Arm64>("hello", optimisation, Library::C);
        for optimise in [Optimise::Speed, Optimise::Size] {
            let how = (LookAhead::Full, optimise);
            let (_, library, _) = translated_for::<Arm64>(&hello, how);
            let (ours, machine, _) = translated_for::<Arm64>(&blob, how);
            let what = format!("at {optimisation} for {optimise:?}");
            assert_eq!(ours, theirs, "{what}");
            assert_eq!(
                machine.count(Reason::JumpThroughRegisterNotRecognised),
                0,
                "{what}"
            );
            assert!(
                machine.count(Reason::TranslatedAtRunTime)
                    <= library.count(Reason::TranslatedAtRunTime),
                "{what}"
            );
        }
    }
}

/// Jumps through a register to their own function's code that exploring it did not reach (a switch bounded
/// by no check, a computed goto) are the slow path, counted each time they run. The program prints how many
/// it made of each.
#[test]
fn jumps_to_their_function_beyond_what_was_explored_are_unrecognised() {
    for optimisation in ["-O2", "-O3", "-Os"] {
        let blob = build::<Arm64>("unexplored_jumps", optimisation, Library::C);
        let theirs = native::<Arm64>(&blob);
        let made: u64 = String::from_utf8_lossy(&theirs.stdout)
            .lines()
            .last()
            .expect("the program prints its counts")
            .split_whitespace()
            .map(|n| n.parse::<u64>().expect("a count"))
            .sum();
        for optimise in [Optimise::Speed, Optimise::Size] {
            let (ours, machine, _) = translated_for::<Arm64>(&blob, (LookAhead::Full, optimise));
            let what = format!("at {optimisation} for {optimise:?}");
            assert_eq!(ours, theirs, "{what}");
            assert_eq!(
                machine.count(Reason::JumpThroughRegisterNotRecognised),
                made,
                "{what}"
            );
        }
    }
}

#[test]
fn programs_match_linux_arm64() {
    programs_match_linux::<Arm64>();
}

#[test]
fn stack_moved_by_a_callee_matches_linux_arm64() {
    stack_moved_by_a_callee_matches_linux::<Arm64>();
}

#[test]
fn benchmarks_match_linux_arm64() {
    benchmarks_match_linux::<Arm64>();
}

#[test]
fn deep_recursion_matches_linux_arm64() {
    deep_recursion_matches_linux::<Arm64>();
}

#[test]
fn a_frame_past_the_budget_matches_linux_arm64() {
    a_frame_past_the_budget_matches_linux::<Arm64>();
}

#[test]
fn unwinding_matches_linux_arm64() {
    unwinding_matches_linux::<Arm64>();
}

#[test]
fn stacks_switched_by_the_program_match_linux_arm64() {
    stacks_switched_by_the_program_match_linux::<Arm64>();
}

#[test]
fn written_code_and_null_calls_match_linux_arm64() {
    written_code_and_null_calls_match_linux::<Arm64>();
}

/// ARM64 builds an address in code relative to pc, so it needs no link address. Its position independent
/// code takes `main` from the global offset table, an address held in data, so it is not checked.
#[test]
fn main_is_in_the_full_translation_arm64() {
    main_is_in_the_full_translation::<Arm64>(&[], &[Position::Fixed], true);
}

#[test]
fn cut_functions_and_split_translations_match_linux_arm64() {
    cut_functions_and_split_translations_match_linux::<Arm64>(&[
        ("hello", "-O2"),
        ("differences", "-O0"),
        ("exceptions", "-O2"),
        ("many_blocks", "-O2"),
    ]);
}

#[test]
fn hello_matches_linux_arm64() {
    hello_matches_linux::<Arm64>();
}

#[test]
fn differences_match_linux_arm64() {
    differences_match_linux::<Arm64>();
}

/// ARM64's hosts: one whose fused multiply-add does not fuse reaches the software fused multiply-add, and
/// one with neither also the exact path, counted; at full by leaving the piece.
/// With FPSR read (the program as it is), the flags are kept from the start, so every operation done in
/// software is exact floating point instead, also reached at full by leaving the piece; built without
/// reading it (`WITHOUT_FENV`), the software fused multiply-add is reached.
#[test]
fn differences_match_linux_on_every_host_arm64() {
    for (defines, reads_float_status) in [(&[][..], true), (&["WITHOUT_FENV"][..], false)] {
        differences_match_linux_on_every_host::<Arm64>(defines, |host, what, blob, machine| {
            assert_eq!(
                blob.reads_float_status::<Arm64>(),
                reads_float_status,
                "{what}"
            );
            if host.fused_multiply_add || host.arm_nans {
                return;
            }
            let software = machine.count(Reason::SoftwareFusedMultiplyAdd);
            if reads_float_status {
                assert_eq!(software, 0, "{what}");
                assert!(
                    machine.count(Reason::ExactFloatingPointRequested) > 0,
                    "{what}"
                );
                if what.ends_with("Full") {
                    assert!(machine.count(Reason::LeftToAddress) > 0, "{what}");
                }
            } else {
                assert!(software > 0, "{what}");
            }
        });
    }
}

/// ARM64's own: a flat blob is told the HWCAP the abi gives programs.
#[test]
fn blobs_are_told_the_hwcap_of_the_abi_arm64() {
    use wcontainers_translator_test::layout::AT_HWCAP;
    assert!(Arm64::AUXILIARY.contains(&(AT_HWCAP, wcontainers_abi::HWCAP)));
}

#[test]
fn stops_at_every_check_change_nothing_arm64() {
    stops_at_every_check_change_nothing::<Arm64>(&[]);
}

#[test]
fn measure_arm64() {
    measure::<Arm64>();
}

#[test]
#[ignore]
fn floats_speed_arm64() {
    floats_speed::<Arm64>();
}

#[test]
#[ignore]
fn switching_speed_arm64() {
    switching_speed::<Arm64>();
}

#[test]
fn measure_x86_64() {
    measure::<X86_64>();
}

#[test]
fn hello_matches_linux_x86_64() {
    hello_matches_linux::<X86_64>();
}

#[test]
fn benchmarks_match_linux_x86_64() {
    benchmarks_match_linux::<X86_64>();
}

/// Flags a callee hands back to its caller, in every way a call and a return can be made.
#[test]
fn flag_returns_match_linux_x86_64() {
    match_linux::<X86_64>(&[("flag_returns", Library::None)]);
}

/// Flags live across calls: through a callee that leaves them, set in part by one, and driving a loop.
#[test]
fn flags_across_calls_match_linux_x86_64() {
    match_linux::<X86_64>(&[("flags_across", Library::None)]);
}

/// gcc's and clang's own switch tables on a 32-bit index, which full recognises: no jump leaves through
/// the lookup.
#[test]
fn compilers_switches_match_linux_x86_64() {
    const NAME: &str = "switch_tables";
    match_linux::<X86_64>(&[(NAME, Library::None)]);
    for optimisation in LEVELS {
        let blob = build::<X86_64>(NAME, optimisation, Library::None);
        for optimise in [Optimise::Speed, Optimise::Size] {
            let (_, machine, _) = translated_for::<X86_64>(&blob, (LookAhead::Full, optimise));
            assert_eq!(
                machine.count(Reason::JumpThroughRegisterNotRecognised),
                0,
                "{NAME} at {optimisation}, {optimise:?}"
            );
        }
    }
}

/// Flags of shifts, rotates and bit tests live across calls, among them a shift by a count of zero.
#[test]
fn shift_flags_across_calls_match_linux_x86_64() {
    match_linux::<X86_64>(&[("shift_flags", Library::None)]);
}

/// rep string instructions on their bulk passes and one element at a time.
#[test]
fn rep_strings_match_linux_x86_64() {
    match_linux::<X86_64>(&[("strings", Library::None)]);
}

#[test]
fn programs_match_linux_x86_64() {
    programs_match_linux::<X86_64>();
}

/// The bounds the programs of one unit of the CPU (x87, MMX, the vector extensions) run at beside none and full:
/// one instruction a piece, a few, the comparison's, and past the largest function.
const UNIT_BOUNDS: [u32; 6] = [
    1,
    5,
    17,
    64,
    wcontainers_translator::BOUND,
    2 * FUNCTION_BLOCKS as u32,
];

/// None first, full, and every bound of [`UNIT_BOUNDS`], each for speed and size.
fn every_unit_look_ahead() -> Vec<(LookAhead, Optimise)> {
    let optimised = |look_ahead| [Optimise::Speed, Optimise::Size].map(|o| (look_ahead, o));
    std::iter::once((LookAhead::None, Optimise::Speed))
        .chain(optimised(LookAhead::Full))
        .chain(
            UNIT_BOUNDS
                .into_iter()
                .flat_map(|n| optimised(LookAhead::Bounded(n))),
        )
        .collect()
}

/// C's long double through the x87 unit, at every look-ahead.
#[test]
fn long_double_matches_linux_x86_64() {
    match_linux_at::<X86_64>(&[("long_double", Library::C)], &every_unit_look_ahead());
}

/// MMX's instructions and how they meet the x87 unit, and the x87 unit's state, environment, compare,
/// integer, decimal, remainder and exception instructions, at every look-ahead.
#[test]
fn mmx_and_x87_state_match_linux_x86_64() {
    match_linux_at::<X86_64>(
        &[("mmx", Library::C), ("x87_state", Library::C)],
        &every_unit_look_ahead(),
    );
}

/// AVX2's integer, permute, gather and masked move instructions with BMI2, and FMA, F16C and AVX's floating
/// point in every rounding mode, for programs told the x86-64-v3 list, at every look-ahead.
#[test]
fn avx2_fma_and_f16c_match_linux_x86_64() {
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).expect("a built list");
    match_linux_told::<X86_64>(
        &[("avx2_integer", Library::C), ("fma_f16c", Library::C)],
        &every_unit_look_ahead(),
        Some(v3.key()),
    );
}

/// FMA and F16C in MXCSR's default mode, which the program never touches, for programs told the x86-64-v3 list,
/// at every look-ahead: the fast path, with the multiply-add in software, counted, only where the host's does not
/// fuse, and half precision rounding in software, counted.
#[test]
fn fma_and_f16c_in_the_default_mode_match_linux_x86_64() {
    use wcontainers_guest_cpu::Guest;
    use wcontainers_guest_cpu::reason::EXACT_FLOATING_POINT_REQUESTED;
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).expect("a built list");
    let fuses = wcontainers_translator_test::runner::probe::<X86_64>().fused_multiply_add;
    let (_, half) = X86_64::REASONS
        .iter()
        .find(|(reason, _)| *reason == wcontainers_x86::Reason::HalfPrecisionInSoftware)
        .expect("a named reason");
    for optimisation in LEVELS {
        let mut blob = build::<X86_64>("fma_fast", optimisation, Library::C);
        blob.offer = Some(v3.key());
        let theirs = native::<X86_64>(&blob);
        for how in every_unit_look_ahead() {
            let (ours, machine, _) = translated_for::<X86_64>(&blob, how);
            let what = format!("fma_fast at {optimisation}, {how:?}");
            assert_eq!(ours, theirs, "{what}");
            assert_eq!(
                machine.count_named(EXACT_FLOATING_POINT_REQUESTED),
                0,
                "{what}"
            );
            assert!(machine.count_named(half) > 0, "{what}");
            let software = machine.count(Reason::SoftwareFusedMultiplyAdd);
            assert_eq!(software > 0, !fuses, "{what}: {software} in software");
            eprintln!("{what}: matches, {:?}", machine.counters());
        }
    }
}

/// The levels whose code from `bench.c` built for the x86-64-v3 level holds that level's instructions.
const VECTORISED_LEVELS: [&str; 2] = ["-O2", "-O3"];

/// The benchmark set built for the x86-64-v3 level: told the v3 list, Linux's output at every look-ahead with no
/// instruction reached of a feature it was not told of; told the default list, killed by illegal instruction at
/// the first instruction beyond it, counted.
#[test]
fn benchmark_set_for_x86_64_v3_runs_only_when_told_x86_64() {
    use wcontainers_guest_cpu::Guest;
    use wcontainers_translator_test::guest::x86_64::V3_CPU;
    use wcontainers_translator_test::layout::SIGNALLED;
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).expect("a built list");
    let (_, not_told) = X86_64::REASONS
        .iter()
        .find(|(reason, _)| *reason == wcontainers_x86::Reason::NotToldFeatureReached)
        .expect("a named reason");
    let killed = SIGNALLED + linux_raw_sys::general::SIGILL as i32;
    for optimisation in VECTORISED_LEVELS {
        let mut blob = build_for::<X86_64>(
            "bench",
            optimisation,
            Library::None,
            Position::Fixed,
            V3_CPU,
        );
        blob.offer = Some(v3.key());
        let theirs = native::<X86_64>(&blob);
        for how in EVERY_LOOK_AHEAD {
            let (ours, machine, _) = translated_for::<X86_64>(&blob, how);
            let what = format!("bench {optimisation} for v3, told v3, {how:?}");
            assert_eq!(ours, theirs, "{what}");
            assert_eq!(machine.count_named(not_told), 0, "{what}");
            eprintln!("{what}: matches, {:?}", machine.counters());
        }
        blob.offer = None;
        for how in EVERY_LOOK_AHEAD {
            let (ours, machine, _) = translated_for::<X86_64>(&blob, how);
            let what = format!("bench {optimisation} for v3, told the default list, {how:?}");
            assert_eq!(ours.status, killed, "{what}");
            assert_eq!(machine.count_named(not_told), 1, "{what}");
            eprintln!("{what}: illegal instruction, {:?}", machine.counters());
        }
    }
}

/// `xsave` and `xrstor` of x87, SSE and AVX state, for a program told the x86-64-v3 list, at every look-ahead.
#[test]
fn xstate_matches_linux_x86_64() {
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).expect("a built list");
    match_linux_told::<X86_64>(
        &[("xstate", Library::C)],
        &every_unit_look_ahead(),
        Some(v3.key()),
    );
}

#[test]
fn stack_moved_by_a_callee_matches_linux_x86_64() {
    stack_moved_by_a_callee_matches_linux::<X86_64>();
}

#[test]
fn deep_recursion_matches_linux_x86_64() {
    deep_recursion_matches_linux::<X86_64>();
}

#[test]
fn a_frame_past_the_budget_matches_linux_x86_64() {
    a_frame_past_the_budget_matches_linux::<X86_64>();
}

/// x86 has no `differences` program yet (it is ARM64's list of where its CPU and WebAssembly differ).
#[test]
fn cut_functions_and_split_translations_match_linux_x86_64() {
    cut_functions_and_split_translations_match_linux::<X86_64>(&[
        ("hello", "-O2"),
        ("exceptions", "-O2"),
        ("many_blocks", "-O2"),
    ]);
}

#[test]
fn stops_at_every_check_change_nothing_x86_64() {
    stops_at_every_check_change_nothing::<X86_64>(&[("strings", &[])]);
}

#[test]
fn unwinding_matches_linux_x86_64() {
    unwinding_matches_linux::<X86_64>();
}

#[test]
fn stacks_switched_by_the_program_match_linux_x86_64() {
    stacks_switched_by_the_program_match_linux::<X86_64>();
}

/// x86's written code has no announcement: the second write is seen by the write lock on its page.
#[test]
fn written_code_and_null_calls_match_linux_x86_64() {
    written_code_and_null_calls_match_linux::<X86_64>();
}

/// The passes of `self_written`'s loop, each of which writes the page it runs from.
const SELF_WRITTEN_PASSES: u64 = 200;

/// What `self_written` is built with.
fn self_written_defines() -> [String; 2] {
    use wcontainers_abi::PAGE_SIZE;
    [
        format!("PAGE_SIZE={PAGE_SIZE}"),
        format!("PASSES={SELF_WRITTEN_PASSES}"),
    ]
}

/// Code that writes the page it runs from: the next instruction's immediate on every pass of a loop, and an
/// instruction's own first byte, which it runs as it was read. Every look-ahead as on Linux, each write
/// to a page of translated code counted, and in Chrome.
#[test]
fn self_written_code_matches_linux_x86_64() {
    let defines = self_written_defines();
    let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
    for (optimisation, position) in levels_and_positions() {
        let blob =
            build_with::<X86_64>("self_written", optimisation, Library::C, &defines, position);
        let theirs = native::<X86_64>(&blob);
        assert_eq!(theirs.status, 0, "{}", blob.name);
        every_look_ahead_matches::<X86_64>(&blob, &theirs, |what, machine| {
            let written = machine.count(Reason::CodeWrittenWithoutAnnouncement);
            eprintln!("{what}: code written without announcement {written} times");
            assert!(written >= SELF_WRITTEN_PASSES, "{what}");
        });
        let name = Reason::CodeWrittenWithoutAnnouncement.name();
        for (what, ours) in every_look_ahead_matches_in_chrome::<X86_64>(&blob, &theirs) {
            let written = ours.count(name);
            assert!(written >= SELF_WRITTEN_PASSES, "{what}: {written}");
        }
    }
}

/// Code a function writes at its own stack pointer and calls, then writes again there with other bytes and
/// calls again (`programs/x86_64/stack_code.S`): by a push, by a store through the stack pointer, and by a
/// store through the frame pointer. The page is locked for code while the function's frame is in progress, so
/// the thread gives up its frames, counted, and the second write is seen. Every look-ahead as on Linux, and in
/// Chrome.
#[test]
fn code_on_the_stack_matches_linux_x86_64() {
    for case in 0..STACK_CODE_CASES {
        let defines = [
            format!("CASE={case}"),
            format!("SYS_exit={}", X86_64::STAND_IN.exit),
            format!("FIRST={STACK_CODE_FIRST}"),
            format!("SECOND={STACK_CODE_SECOND}"),
            format!("SHIFT={STACK_CODE_SHIFT}"),
            format!("PAGE_SIZE={}", wcontainers_abi::PAGE_SIZE),
        ];
        let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
        let blob = build_with::<X86_64>(
            "stack_code",
            "-O0",
            Library::None,
            &defines,
            Position::Fixed,
        );
        let theirs = native::<X86_64>(&blob);
        let status = (STACK_CODE_FIRST << STACK_CODE_SHIFT) + STACK_CODE_SECOND;
        assert_eq!(theirs.status, status as i32, "{}", blob.name);
        every_look_ahead_matches::<X86_64>(&blob, &theirs, |what, machine| {
            let given_up = machine.count(Reason::CodeTranslatedFromLiveStackFrame);
            eprintln!("{what}: frames given up {given_up} times");
            assert!(given_up > 0, "{what}");
        });
        let given_up = Reason::CodeTranslatedFromLiveStackFrame.name();
        for (what, ours) in every_look_ahead_matches_in_chrome::<X86_64>(&blob, &theirs) {
            assert!(ours.count(given_up) > 0, "{what}");
        }
    }
}

/// What `far_frame` stores at its new stack pointer and halfway up.
const FAR_FRAME_LOW: u32 = 20;
const FAR_FRAME_HIGH: u32 = 22;
/// A descent no stack of a test reaches, as a 32-bit immediate allows.
const FAR_FRAME_PAST_THE_STACK: u64 = 0x2be2_a860;

/// A move of the stack pointer down past what one frame check covers (`FRAME_CHECK_BYTES`), within the stack
/// and far past it, translates at once and runs as on Linux at every look-ahead, and in Chrome.
#[test]
fn far_frames_match_linux_x86_64() {
    use wcontainers_translator::contract::FRAME_CHECK_BYTES;
    let within = 2 * FRAME_CHECK_BYTES;
    for descent in [within, FAR_FRAME_PAST_THE_STACK] {
        let defines = [
            format!("DESCENT={descent}"),
            format!("LOW={FAR_FRAME_LOW}"),
            format!("HIGH={FAR_FRAME_HIGH}"),
            format!("SYS_exit={}", X86_64::STAND_IN.exit),
        ];
        let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
        let mut blob =
            build_with::<X86_64>("far_frame", "-O0", Library::None, &defines, Position::Fixed);
        blob.stack_bytes = blob.stack_bytes.max(2 * within);
        let theirs = native::<X86_64>(&blob);
        if descent == within {
            assert_eq!(theirs.status, (FAR_FRAME_LOW + FAR_FRAME_HIGH) as i32);
        }
        every_look_ahead_matches::<X86_64>(&blob, &theirs, |_, _| {});
        every_look_ahead_matches_in_chrome::<X86_64>(&blob, &theirs);
    }
}

/// The cases of `programs/x86_64/stack_code.S`: the second write by a push, by a store through rsp, and by a
/// store through rbp.
const STACK_CODE_CASES: u32 = 3;
/// What the code `stack_code` writes the first time returns, and the second time; it exits with the first
/// shifted by `STACK_CODE_SHIFT`, plus the second.
const STACK_CODE_FIRST: u32 = 7;
const STACK_CODE_SECOND: u32 = 9;
const STACK_CODE_SHIFT: u32 = 4;

/// The cases of `programs/x86_64/trap_flag.S`: `popf` setting TF before a `nop`, a `jmp`, a `call`, a `ret` and
/// a `syscall`, a `popf` clearing it, a `rep movsb`, a load of ss, `iretq` setting it, and a store and a
/// `rep stosb` into the page of translated code they run from.
const TRAP_FLAG_CASES: u32 = 11;

/// The trap flag set by the program: each case stops with SIGTRAP `TRAP_TRACE` where the reference stops, with
/// its general registers and rip, at every look-ahead in wasmtime and in Chrome.
#[test]
fn trap_flag_steps_match_linux_x86_64() {
    use wcontainers_translator_test::browser::SINGLE_STEP;
    use wcontainers_translator_test::guest::x86_64::{SIGTRAP, TRAP_TRACE};
    let set = <X86_64 as wcontainers_guest_cpu::Guest>::REASONS
        .iter()
        .find(|r| r.0 == wcontainers_x86::Reason::TrapFlagSet)
        .unwrap()
        .1;
    for case in 0..TRAP_FLAG_CASES {
        let defines = [
            format!("CASE={case}"),
            format!("TF={}", wcontainers_x86::TF),
            format!("SYS_write={}", X86_64::STAND_IN.write),
            format!("SYS_exit={}", X86_64::STAND_IN.exit),
        ];
        let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
        let blob =
            build_with::<X86_64>("trap_flag", "-O0", Library::None, &defines, Position::Fixed);
        let (theirs, stop) = native_with_stop::<X86_64>(&blob);
        let (signal, code, registers) = stop.expect("the reference stops by a signal");
        assert_eq!((signal, code), (SIGTRAP, TRAP_TRACE), "case {case}");
        // The record holds the general registers in the block's order, then rip.
        let same_stop = |block: &wcontainers_x86::X86Block, what: &str| {
            let (gpr, rip) = registers.split_at(block.gpr.len());
            assert_eq!(&block.gpr[..], gpr, "general registers of {what}");
            assert_eq!(block.rip, rip[0], "rip of {what}");
        };
        for how in EVERY_LOOK_AHEAD {
            let (ours, machine, block) = translated_for::<X86_64>(&blob, how);
            let what = format!("case {case} at {how:?}");
            assert_eq!(ours, theirs, "{what}");
            same_stop(&block, &what);
            assert!(machine.count_named(set) > 0, "{what}");
        }
        for (what, ours) in every_look_ahead_matches_in_chrome::<X86_64>(&blob, &theirs) {
            let what = format!("case {case}: {what}");
            assert_eq!(ours.stop, SINGLE_STEP, "{what}");
            same_stop(&ours.block::<X86_64>().expect("a stop's state"), &what);
            assert!(ours.count(set) > 0, "{what}");
        }
    }
}

/// Where `written_by_another`'s spinning code holds the immediate the other thread writes: after the one
/// opcode byte of `mov eax, imm32` at the start of its page.
const SPIN_IMMEDIATE: u64 = 1;
/// The immediate the other thread writes there.
const SPIN_WRITTEN: u32 = 2;
/// What `written_by_another` is built with.
fn written_by_another_defines() -> [String; 3] {
    use wcontainers_abi::PAGE_SIZE;
    [
        format!("PAGE_SIZE={PAGE_SIZE}"),
        format!("IMMEDIATE={SPIN_IMMEDIATE}"),
        format!("NEW={SPIN_WRITTEN}"),
    ]
}

/// The rare-path C programs of x86 alone: its full translation has `main` only given the link address.
#[test]
fn main_is_in_the_full_translation_x86_64() {
    let self_written = self_written_defines();
    let by_another = written_by_another_defines();
    let self_written: Vec<&str> = self_written.iter().map(String::as_str).collect();
    let by_another: Vec<&str> = by_another.iter().map(String::as_str).collect();
    main_is_in_the_full_translation::<X86_64>(
        &[
            ("self_written", &self_written),
            ("written_by_another", &by_another),
        ],
        &POSITIONS,
        false,
    );
}

/// How long one run of `written_by_another` may take: past it, a thread still spinning in code that was
/// written fails the test instead of running for ever.
const SPIN_TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(120);

/// Code another thread changes while this one runs it (`programs/x86_64/written_by_another.c`). On the
/// reference a second thread writes it; the test runners give one thread, so the runner writes the same
/// bytes as that thread at the first stop of the spinning thread on the written page, which drops the
/// page's pieces and makes the thread give up its frames. Every look-ahead as on Linux, with none's
/// registers and memory, each run within `SPIN_TIME_LIMIT`. Not in Chrome: its runner has no pause for
/// another thread to act in.
#[test]
fn code_written_by_another_thread_matches_linux_x86_64() {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{RecvTimeoutError, channel};
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_translator_test::runner::Pause;
    let defines = written_by_another_defines();
    let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
    for (optimisation, position) in levels_and_positions() {
        let blob = build_with::<X86_64>(
            "written_by_another",
            optimisation,
            Library::C,
            &defines,
            position,
        );
        let theirs = native::<X86_64>(&blob);
        assert_eq!(theirs.status, 0, "{}", blob.name);
        let mut runs = Vec::new();
        for how in EVERY_LOOK_AHEAD {
            let mut machine = ready_on::<X86_64>(
                &blob,
                how,
                Default::default(),
                wcontainers_translator_test::runner::probe::<X86_64>(),
            );
            let what = format!("{} at {how:?}", blob.name);
            let code = blob.load..blob.load + blob.code_bytes;
            let written = Rc::new(Cell::new(false));
            let seen = written.clone();
            // Past the limit the watchdog raises the stop flag, and the next stop fails the test.
            let late = Arc::new(AtomicBool::new(false));
            let (finished, finishing) = channel::<()>();
            let watchdog = {
                let late = late.clone();
                let raise = machine.stop_flag_raiser();
                std::thread::spawn(move || {
                    if finishing.recv_timeout(SPIN_TIME_LIMIT) == Err(RecvTimeoutError::Timeout) {
                        late.store(true, Ordering::SeqCst);
                        raise();
                    }
                })
            };
            let late_what = what.clone();
            machine.raise_stop_flag();
            machine.on_pause(move |at, paused| {
                assert!(
                    !late.load(Ordering::SeqCst),
                    "{late_what}: still running after {SPIN_TIME_LIMIT:?}"
                );
                if seen.get() {
                    return;
                }
                let rip = paused.registers().rip;
                if at == Pause::Stop && !code.contains(&rip) {
                    let page = rip / PAGE_SIZE * PAGE_SIZE;
                    paused.write(page + SPIN_IMMEDIATE, &SPIN_WRITTEN.to_le_bytes());
                    seen.set(true);
                } else {
                    paused.raise_stop_flag();
                }
            });
            let (stopped, registers) = machine.run();
            drop(finished);
            watchdog.join().expect("the watchdog");
            assert!(written.get(), "{what}: never stopped on the written page");
            assert!(
                machine.count(Reason::CodeWrittenWithoutAnnouncement) >= 1,
                "{what}"
            );
            runs.push((
                what,
                ran::<X86_64>(&blob, &machine, stopped, &registers),
                machine,
                registers,
            ));
        }
        for (what, ours, _, _) in &runs {
            assert_eq!(*ours, theirs, "{what}");
        }
        let (_, _, mut none_machine, none_registers) = runs.remove(0);
        for (what, _, mut machine, registers) in runs {
            assert_eq!(registers, none_registers, "registers of {what}");
            assert!(
                reachable_memory::<X86_64>(&mut machine, &blob)
                    == reachable_memory::<X86_64>(&mut none_machine, &blob),
                "memory of {what}"
            );
        }
    }
}
