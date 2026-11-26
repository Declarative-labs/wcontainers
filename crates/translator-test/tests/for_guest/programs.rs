// Written once for both guests; each guest's module in `programs.rs` includes it.

/// Both ways a C program is built: x86's fixed-address code takes a function's address as an absolute
/// immediate, which the translation follows through the link address, and its position independent code
/// builds every address relative to the instruction.
pub(super) const POSITIONS: [Position; 2] = [Position::Fixed, Position::Independent];
/// The optimisations the rare-path programs are built at, each in both positions.
pub(super) fn levels_and_positions() -> impl Iterator<Item = (&'static str, Position)> {
    ["-O0", "-O2"]
        .into_iter()
        .flat_map(|level| POSITIONS.map(|position| (level, position)))
}
/// A blob in Chrome at every look-ahead gives Linux's run. Returns each run's report, with what it was.
pub(super) fn every_look_ahead_matches_in_chrome(
    blob: &Blob,
    theirs: &Ran,
) -> Vec<(String, browser::Report)> {
    EVERY_LOOK_AHEAD
        .into_iter()
        .map(|how| {
            let ours = browser::translated_with(blob, how, Default::default());
            let what = format!("{} at {how:?} in Chrome", blob.name);
            assert_eq!(ours.ran, *theirs, "{what}");
            (what, ours)
        })
        .collect()
}
pub(super) fn programs_match_linux() {
    match_linux(&[
        ("integers", Library::None),
        ("tables", Library::None),
        ("buffers", Library::None),
        ("mappings", Library::None),
        ("walks", Library::None),
    ]);
}
/// Hand-written programs whose callees give the stack pointer back moved.
pub(super) fn stack_moved_by_a_callee_matches_linux() {
    match_linux(&[
        ("callee_moves_sp", Library::None),
        ("callee_moves_sp_away", Library::None),
        ("callee_misaligns_sp", Library::None),
    ]);
}
pub(super) fn benchmarks_match_linux() {
    match_linux(&[("bench", Library::None)]);
}
/// Recursion far deeper than the engine's own stack allows, on Linux's 8 MiB stack: C 150,000 calls deep with
/// a frame at each level, and a million calls deep through a stack of 8 byte frames. Each look-ahead
/// matches Linux, and the thread leaves the full engine stack to the bottom loop, counted, and carries on.
pub(super) fn deep_recursion_matches_linux() {
    for (name, optimisation, library, defines) in [
        ("deep", "-O0", Library::C, &["DEPTH=150000"][..]),
        ("deep", "-O2", Library::C, &["DEPTH=150000"][..]),
        ("deep_small_frames", "-O0", Library::None, &[][..]),
    ] {
        let mut blob = build_with(name, optimisation, library, defines, Position::Fixed);
        blob.stack_bytes = 8 << 20;
        let theirs = native(&blob);
        every_look_ahead_matches(&blob, &theirs, |what, machine| {
            let left = machine.count(Reason::StackNearlyFull);
            let continued = machine.count(Reason::ContinuedAfterStackLoss);
            eprintln!("{what}: left a nearly full stack {left} times, continued {continued} times");
            if !what.contains("None") {
                assert!(left > 0 && continued > 0, "{what}");
            }
        });
        let nearly_full = Reason::StackNearlyFull.name();
        for (what, ours) in every_look_ahead_matches_in_chrome(&blob, &theirs) {
            assert!(ours.count(nearly_full) > 0, "{what}");
        }
    }
}
/// A function whose own frame at full is past the engine stack budget runs when the bottom loop calls it,
/// and leaves the stack when it calls itself, at every look-ahead as on Linux.
pub(super) fn a_frame_past_the_budget_matches_linux() {
    let blob = build("big_frame", "-O0", Library::None);
    let theirs = native(&blob);
    assert_eq!(theirs.status, 42);
    every_look_ahead_matches(&blob, &theirs, |what, machine| {
        if what.ends_with("Full") {
            assert!(machine.count(Reason::StackNearlyFull) > 0, "{what}");
        }
    });
}
/// setjmp and longjmp in C, C++ exceptions and Rust panics: returns that do not go where their caller
/// expects, at every look-ahead and with pieces mixed and events injected, as on Linux; and in Chrome.
pub(super) fn unwinding_matches_linux() {
    for name in ["jumps", "exceptions", "panics"] {
        for (optimisation, position) in levels_and_positions() {
            let blob = build_with(name, optimisation, Library::C, &[], position);
            // rustc's aarch64 musl target makes a fixed-address program whatever it is asked.
            if blob.load != position.load() {
                eprintln!("{}: not made position independent, not run", blob.name);
                continue;
            }
            let theirs = native(&blob);
            assert_eq!(
                theirs.status,
                if name == "jumps" { 50 } else { 0 },
                "{}",
                blob.name
            );
            every_look_ahead_matches(&blob, &theirs, print_counters);
            every_look_ahead_matches_in_chrome(&blob, &theirs);
        }
    }
}
/// Programs that move to stacks they made themselves: a fiber switched with setjmp and longjmp, and two
/// coroutines switched by a hand-written context switch, as on Linux at every look-ahead and in Chrome.
pub(super) fn stacks_switched_by_the_program_match_linux() {
    for name in ["fibers", "coroutines"] {
        for (optimisation, position) in levels_and_positions() {
            let blob = build_with(name, optimisation, Library::C, &[], position);
            let theirs = native(&blob);
            assert_eq!(theirs.status, 0, "{}", blob.name);
            every_look_ahead_matches(&blob, &theirs, print_counters);
            every_look_ahead_matches_in_chrome(&blob, &theirs);
        }
    }
}
/// Code the program writes into its own memory and calls, as a JIT does, translated from memory when it is
/// reached, and written again with other code after `ic ivau`; and a call through a null pointer, which
/// faults as on Linux. At every look-ahead and in Chrome.
pub(super) fn written_code_and_null_calls_match_linux() {
    for (name, status) in [("written_code", 0), ("null_call", 139)] {
        for (optimisation, position) in levels_and_positions() {
            let blob = build_with(name, optimisation, Library::C, &[], position);
            let theirs = native(&blob);
            assert_eq!(theirs.status, status, "{}", blob.name);
            every_look_ahead_matches(&blob, &theirs, |what, machine| {
                if name == "null_call" {
                    assert!(machine.count(Reason::MemoryFault) == 1, "{what}");
                }
            });
            every_look_ahead_matches_in_chrome(&blob, &theirs);
        }
    }
}
/// A function of more blocks than one piece may hold, cut in parts, at every look-ahead as on Linux, and in
/// Chrome. Then programs (`split`, each with its optimisation) whose translation at full is split into many
/// small code modules, so that their pieces call each other and return across modules, as on Linux.
pub(super) fn cut_functions_and_split_translations_match_linux(split: &[(&str, &str)]) {
    for optimisation in ["-O0", "-O2"] {
        let blob = build("many_blocks", optimisation, Library::C);
        let theirs = native(&blob);
        assert_eq!(theirs.status, 0, "{}", blob.name);
        every_look_ahead_matches(&blob, &theirs, |_, _| {});
        every_look_ahead_matches_in_chrome(&blob, &theirs);
    }
    for &(name, optimisation) in split {
        let blob = build(name, optimisation, Library::C);
        let theirs = native(&blob);
        let (none, mut none_machine, none_registers) = translated(&blob, LookAhead::None);
        assert_eq!(none, theirs, "{} at none", blob.name);
        for module_bytes in [1 << 12, 1 << 16] {
            let modules = wcontainers_translator::translate_in_modules::<Cpu>(
                &blob.bytes,
                0..blob.code_bytes,
                blob.link_address,
                &[0],
                LookAhead::Full,
                wcontainers_translator::Optimise::Speed,
                wcontainers_translator::ProgressCount::Absent,
                wcontainers_translator::AccessOrdering::Unordered,
                harness::runner::probe(),
                <Cpu as wcontainers_guest_cpu::Guest>::Offer::default(),
                module_bytes,
            )
            .unwrap()
            .modules
            .len();
            assert!(modules > 1, "{} in {modules} modules", blob.name);
            let mut machine = ready_with(&blob, harness::runner::probe(), |m| {
                m.load_split(
                    &blob.bytes,
                    0..blob.code_bytes,
                    (blob.load, blob.link_address),
                    &[0],
                    module_bytes,
                )
            });
            let (stopped, registers) = machine.run();
            let what = format!("{} in modules of {module_bytes} bytes", blob.name);
            assert_eq!(ran(&blob, &machine, stopped, &registers), theirs, "{what}");
            assert_eq!(registers, none_registers, "registers of {what}");
            assert!(
                reachable_memory(&mut machine, &blob) == reachable_memory(&mut none_machine, &blob),
                "memory of {what}"
            );
        }
    }
}
/// The rare-path C programs, those every guest has and the guest's own (`more`, with their definitions), at
/// each optimisation in each of `positions`: the translation made ahead of time at full look-ahead from the
/// entry, given the link address, has a piece for `main`, which a program linked at a fixed address may
/// reach only through an absolute address (musl's `_start_c` passes it as one on x86). Without the link
/// address a fixed build has that piece exactly when `without_link_address` says.
pub(super) fn main_is_in_the_full_translation(
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
            let blob = build_with(name, optimisation, Library::C, defines, position);
            let main = blob.main.expect("a main");
            let has_main = |link_address| {
                wcontainers_translator::translate::<Cpu>(
                    &blob.bytes,
                    0..blob.code_bytes,
                    link_address,
                    &[0],
                    LookAhead::Full,
                    Optimise::Speed,
                    wcontainers_translator::ProgressCount::Absent,
                    wcontainers_translator::AccessOrdering::Unordered,
                    harness::runner::probe(),
                    <Cpu as wcontainers_guest_cpu::Guest>::Offer::default(),
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
pub(super) fn print_counters(what: &str, machine: &Machine) {
    eprintln!("{what}: {:?}", machine.counters());
}
/// A blob at none matches Linux, and at full, at bounded with every piece but the first made at run time,
/// and mixed (seeds 0 to 2, the first with no stops), it matches Linux and none's registers and memory. `check` is called on each run
/// with what it was.
pub(super) fn every_look_ahead_matches(blob: &Blob, theirs: &Ran, check: impl Fn(&str, &Machine)) {
    let (none, mut none_machine, none_registers) = translated(blob, LookAhead::None);
    assert_eq!(&none, theirs, "{} at none", blob.name);
    check(&format!("{} at None", blob.name), &none_machine);
    let mut runs = Vec::new();
    for &how in &EVERY_LOOK_AHEAD[1..] {
        let (ours, machine, registers) = translated_for(blob, how);
        let (look_ahead, optimise) = how;
        runs.push((
            format!("{} at {look_ahead:?} for {optimise:?}", blob.name),
            ours,
            machine,
            registers,
        ));
    }
    for seed in 0..3 {
        let (ours, machine, registers, events) = mixed(blob, seed);
        let what = format!("{} mixed from {seed} ({events:?})", blob.name);
        runs.push((what, ours, machine, registers));
    }
    for (what, ours, mut machine, registers) in runs {
        assert_eq!(&ours, theirs, "{what}");
        assert_eq!(registers, none_registers, "registers of {what}");
        assert!(
            reachable_memory(&mut machine, blob) == reachable_memory(&mut none_machine, blob),
            "memory of {what}"
        );
        check(&what, &machine);
    }
}
pub(super) fn hello_matches_linux() {
    match_linux(&[("hello", Library::C)]);
}
/// Every program, at every optimisation level, at look-ahead none and full: the same output and exit status
/// as on Linux, and at full the same registers and memory at the end as at none. Levels that build to the
/// same bytes as an earlier one are run once.
pub(super) fn match_linux(programs: &[(&str, Library)]) {
    match_linux_at(
        programs,
        &[
            (LookAhead::None, Optimise::Speed),
            (LookAhead::Full, Optimise::Speed),
            (LookAhead::Full, Optimise::Size),
        ],
    );
}
/// The same at these look-aheads only, the first of which the others' registers and memory are compared with.
pub(super) fn match_linux_at(programs: &[(&str, Library)], hows: &[(LookAhead, Optimise)]) {
    match_linux_told(programs, hows, None);
}
/// [`match_linux_at`] for programs told the offer `offer` names (the default when `None`).
pub(super) fn match_linux_told(
    programs: &[(&str, Library)],
    hows: &[(LookAhead, Optimise)],
    offer: Option<u64>,
) {
    for &(name, library) in programs {
        let mut built: Vec<(&str, Vec<u8>)> = Vec::new();
        for optimisation in LEVELS {
            let mut blob = build(name, optimisation, library);
            blob.offer = offer;
            if let Some((same, _)) = built.iter().find(|(_, bytes)| *bytes == blob.bytes) {
                eprintln!("{name} {optimisation}: the same blob as {same}");
                continue;
            }
            built.push((optimisation, blob.bytes.clone()));
            let theirs = native(&blob);
            let mut finals = Vec::new();
            for &how in hows {
                let (ours, mut machine, registers) = translated_for(&blob, how);
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
                finals.push((registers, reachable_memory(&mut machine, &blob)));
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
pub(super) fn stops_at_every_check_change_nothing(programs: &[(&str, &[&str])]) {
    let shared: [(&str, &[&str]); 2] = [("bench", &["SCALE=0"]), ("integers", &[])];
    for &(name, defines) in shared.iter().chain(programs) {
        for optimisation in ["-O0", "-O2"] {
            let blob = build_with(name, optimisation, Library::None, defines, Position::Fixed);
            let theirs = native(&blob);
            let what = format!("{name} at {optimisation}");
            let (none, mut none_machine, none_registers) = translated(&blob, LookAhead::None);
            assert_eq!(none, theirs, "{what} at none");
            for optimise in [Optimise::Speed, Optimise::Size] {
                let (full, mut machine, registers) =
                    translated_stopping_everywhere(&blob, (LookAhead::Full, optimise));
                let what = format!("{what} for {optimise:?}");
                assert_eq!(full, theirs, "{what} at full, stopping everywhere");
                assert_eq!(registers, none_registers, "registers of {what}");
                assert!(
                    reachable_memory(&mut machine, &blob)
                        == reachable_memory(&mut none_machine, &blob),
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
pub(super) fn measure() {
    use harness::runner::compile_time;
    use std::io::Write;
    use std::time::{Duration, Instant};
    use wcontainers_translator::translate;
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
        let blob = build(name, optimisation, library);
        let code = &blob.bytes[..blob.code_bytes as usize];
        let starts = boundaries(code);
        // None from every instruction, full from the entry, as programs are run.
        for (look_ahead, from) in [(LookAhead::None, &starts[..]), (LookAhead::Full, &[0][..])] {
            let mut best = (Duration::MAX, Duration::MAX, Vec::new());
            for _ in 0..5 {
                let start = Instant::now();
                let mut translation = translate::<Cpu>(
                    &blob.bytes,
                    0..blob.code_bytes,
                    blob.link_address,
                    from,
                    look_ahead,
                    wcontainers_translator::Optimise::Speed,
                    wcontainers_translator::ProgressCount::Absent,
                    wcontainers_translator::AccessOrdering::Unordered,
                    harness::runner::probe(),
                    <Cpu as wcontainers_guest_cpu::Guest>::Offer::default(),
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
