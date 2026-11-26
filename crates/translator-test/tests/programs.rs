//! C programs give the same output and exit status through the translator as on real Linux of the guest's
//! CPU. Each test is written once over the guest and run for each.

use wcontainers_translator::contract::Reason;
use wcontainers_translator::{FUNCTION_BLOCKS, LookAhead, Optimise};
use wcontainers_translator_test::reference::{DOCKER_MOUNT, IMAGE};
use wcontainers_x86::X86_64;

const LEVELS: [&str; 5] = ["-O0", "-O1", "-O2", "-O3", "-Os"];

/// The look-aheads every program runs at: none, full for speed and for size, and bounded with every piece but
/// the first made at run time.
const EVERY_LOOK_AHEAD: [(LookAhead, Optimise); 4] = [
    (LookAhead::None, Optimise::Speed),
    (LookAhead::Full, Optimise::Speed),
    (LookAhead::Full, Optimise::Size),
    (LookAhead::AT_RUN_TIME, Optimise::Speed),
];

#[test]
#[should_panic(expected = "Docker failed")]
fn a_container_docker_does_not_make_fails_the_case() {
    arm64::run_container("unmade", |command| {
        command.args([DOCKER_MOUNT, "/r:/r:not-a-mode", IMAGE, "true"]);
    });
}

#[test]
#[should_panic(expected = "Docker failed")]
fn a_container_that_does_not_start_fails_the_case() {
    arm64::run_container("unstarted", |command| {
        command.args([IMAGE, "/no-such-command"]);
    });
}

#[test]
fn a_programs_own_ending_is_its_result() {
    let output = arm64::run_container("own-ending", |command| {
        command.args([IMAGE, "sh", "-c", "exit 125"]);
    });
    assert_eq!(output.status.code(), Some(125));
}

/// FPSR's flags read without the program clearing or setting them first, as on Linux; and in Chrome.
#[test]
fn float_status_read_first_matches_linux_arm64() {
    arm64::match_linux(&[("float_status", arm64::Library::C)]);
    let blob = arm64::build("float_status", "-O2", arm64::Library::C);
    arm64::every_look_ahead_matches_in_chrome(&blob, &arm64::native(&blob));
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
    let blob = arm64::build_with(
        "register_jumps",
        "-O0",
        arm64::Library::None,
        &defines,
        arm64::Position::Fixed,
    );
    let theirs = arm64::native(&blob);
    for optimise in [Optimise::Speed, Optimise::Size] {
        let (ours, machine, _) = arm64::translated_for(&blob, (LookAhead::Full, optimise));
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
        let blob = arm64::build("cut_switch", optimisation, arm64::Library::C);
        let theirs = arm64::native(&blob);
        let hello = arm64::build("hello", optimisation, arm64::Library::C);
        for optimise in [Optimise::Speed, Optimise::Size] {
            let how = (LookAhead::Full, optimise);
            let (_, library, _) = arm64::translated_for(&hello, how);
            let (ours, machine, _) = arm64::translated_for(&blob, how);
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
        let blob = arm64::build("unexplored_jumps", optimisation, arm64::Library::C);
        let theirs = arm64::native(&blob);
        let made: u64 = String::from_utf8_lossy(&theirs.stdout)
            .lines()
            .last()
            .expect("the program prints its counts")
            .split_whitespace()
            .map(|n| n.parse::<u64>().expect("a count"))
            .sum();
        for optimise in [Optimise::Speed, Optimise::Size] {
            let (ours, machine, _) = arm64::translated_for(&blob, (LookAhead::Full, optimise));
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
    arm64::programs_match_linux();
}

#[test]
fn stack_moved_by_a_callee_matches_linux_arm64() {
    arm64::stack_moved_by_a_callee_matches_linux();
}

#[test]
fn benchmarks_match_linux_arm64() {
    arm64::benchmarks_match_linux();
}

#[test]
fn deep_recursion_matches_linux_arm64() {
    arm64::deep_recursion_matches_linux();
}

#[test]
fn a_frame_past_the_budget_matches_linux_arm64() {
    arm64::a_frame_past_the_budget_matches_linux();
}

#[test]
fn unwinding_matches_linux_arm64() {
    arm64::unwinding_matches_linux();
}

#[test]
fn stacks_switched_by_the_program_match_linux_arm64() {
    arm64::stacks_switched_by_the_program_match_linux();
}

#[test]
fn written_code_and_null_calls_match_linux_arm64() {
    arm64::written_code_and_null_calls_match_linux();
}

/// ARM64 builds an address in code relative to pc, so it needs no link address. Its position independent
/// code takes `main` from the global offset table, an address held in data, so it is not checked.
#[test]
fn main_is_in_the_full_translation_arm64() {
    arm64::main_is_in_the_full_translation(&[], &[arm64::Position::Fixed], true);
}

#[test]
fn cut_functions_and_split_translations_match_linux_arm64() {
    arm64::cut_functions_and_split_translations_match_linux(&[
        ("hello", "-O2"),
        ("differences", "-O0"),
        ("exceptions", "-O2"),
        ("many_blocks", "-O2"),
    ]);
}

#[test]
fn hello_matches_linux_arm64() {
    arm64::hello_matches_linux();
}

#[test]
fn differences_match_linux_arm64() {
    arm64::differences_match_linux();
}

/// ARM64's hosts: one whose fused multiply-add does not fuse reaches the software fused multiply-add, and
/// one with neither also the exact path, counted; at full by leaving the piece.
/// With FPSR read (the program as it is), the flags are kept from the start, so every operation done in
/// software is exact floating point instead, also reached at full by leaving the piece; built without
/// reading it (`WITHOUT_FENV`), the software fused multiply-add is reached.
#[test]
fn differences_match_linux_on_every_host_arm64() {
    for (defines, reads_float_status) in [(&[][..], true), (&["WITHOUT_FENV"][..], false)] {
        arm64::differences_match_linux_on_every_host(defines, |host, what, blob, machine| {
            assert_eq!(blob.reads_float_status(), reads_float_status, "{what}");
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
    assert!(
        wcontainers_translator_test::arm64::part::AUXILIARY
            .contains(&(AT_HWCAP, wcontainers_abi::HWCAP))
    );
}

#[test]
fn stops_at_every_check_change_nothing_arm64() {
    arm64::stops_at_every_check_change_nothing(&[]);
}

#[test]
fn measure_arm64() {
    arm64::measure();
}

#[test]
#[ignore]
fn floats_speed_arm64() {
    arm64::floats_speed();
}

#[test]
#[ignore]
fn switching_speed_arm64() {
    arm64::switching_speed();
}

#[test]
fn measure_x86_64() {
    x86_64::measure();
}

#[test]
fn hello_matches_linux_x86_64() {
    x86_64::hello_matches_linux();
}

#[test]
fn benchmarks_match_linux_x86_64() {
    x86_64::benchmarks_match_linux();
}

/// Flags a callee hands back to its caller, in every way a call and a return can be made.
#[test]
fn flag_returns_match_linux_x86_64() {
    x86_64::match_linux(&[("flag_returns", x86_64::Library::None)]);
}

/// Flags live across calls: through a callee that leaves them, set in part by one, and driving a loop.
#[test]
fn flags_across_calls_match_linux_x86_64() {
    x86_64::match_linux(&[("flags_across", x86_64::Library::None)]);
}

/// gcc's and clang's own switch tables on a 32-bit index, which full recognises: no jump leaves through
/// the lookup.
#[test]
fn compilers_switches_match_linux_x86_64() {
    const NAME: &str = "switch_tables";
    x86_64::match_linux(&[(NAME, x86_64::Library::None)]);
    for optimisation in LEVELS {
        let blob = x86_64::build(NAME, optimisation, x86_64::Library::None);
        for optimise in [Optimise::Speed, Optimise::Size] {
            let (_, machine, _) = x86_64::translated_for(&blob, (LookAhead::Full, optimise));
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
    x86_64::match_linux(&[("shift_flags", x86_64::Library::None)]);
}

/// rep string instructions on their bulk passes and one element at a time.
#[test]
fn rep_strings_match_linux_x86_64() {
    x86_64::match_linux(&[("strings", x86_64::Library::None)]);
}

#[test]
fn programs_match_linux_x86_64() {
    x86_64::programs_match_linux();
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
    x86_64::match_linux_at(
        &[("long_double", x86_64::Library::C)],
        &every_unit_look_ahead(),
    );
}

/// MMX's instructions and how they meet the x87 unit, and the x87 unit's state, environment, compare,
/// integer, decimal, remainder and exception instructions, at every look-ahead.
#[test]
fn mmx_and_x87_state_match_linux_x86_64() {
    x86_64::match_linux_at(
        &[
            ("mmx", x86_64::Library::C),
            ("x87_state", x86_64::Library::C),
        ],
        &every_unit_look_ahead(),
    );
}

/// AVX2's integer, permute, gather and masked move instructions with BMI2, and FMA, F16C and AVX's floating
/// point in every rounding mode, for programs told the x86-64-v3 list, at every look-ahead.
#[test]
fn avx2_fma_and_f16c_match_linux_x86_64() {
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).expect("a built list");
    x86_64::match_linux_told(
        &[
            ("avx2_integer", x86_64::Library::C),
            ("fma_f16c", x86_64::Library::C),
        ],
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
    let fuses = wcontainers_translator_test::x86_64::runner::probe().fused_multiply_add;
    let (_, half) = X86_64::REASONS
        .iter()
        .find(|(reason, _)| *reason == wcontainers_x86::Reason::HalfPrecisionInSoftware)
        .expect("a named reason");
    for optimisation in LEVELS {
        let mut blob = x86_64::build("fma_fast", optimisation, x86_64::Library::C);
        blob.offer = Some(v3.key());
        let theirs = x86_64::native(&blob);
        for how in every_unit_look_ahead() {
            let (ours, machine, _) = x86_64::translated_for(&blob, how);
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
    use wcontainers_translator_test::layout::SIGNALLED;
    use wcontainers_translator_test::x86_64::part::V3_CPU;
    let v3 = wcontainers_x86::Offer::new(wcontainers_x86::V3).expect("a built list");
    let (_, not_told) = X86_64::REASONS
        .iter()
        .find(|(reason, _)| *reason == wcontainers_x86::Reason::NotToldFeatureReached)
        .expect("a named reason");
    let killed = SIGNALLED + linux_raw_sys::general::SIGILL as i32;
    for optimisation in VECTORISED_LEVELS {
        let mut blob = x86_64::build_for(
            "bench",
            optimisation,
            x86_64::Library::None,
            x86_64::Position::Fixed,
            V3_CPU,
        );
        blob.offer = Some(v3.key());
        let theirs = x86_64::native(&blob);
        for how in EVERY_LOOK_AHEAD {
            let (ours, machine, _) = x86_64::translated_for(&blob, how);
            let what = format!("bench {optimisation} for v3, told v3, {how:?}");
            assert_eq!(ours, theirs, "{what}");
            assert_eq!(machine.count_named(not_told), 0, "{what}");
            eprintln!("{what}: matches, {:?}", machine.counters());
        }
        blob.offer = None;
        for how in EVERY_LOOK_AHEAD {
            let (ours, machine, _) = x86_64::translated_for(&blob, how);
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
    x86_64::match_linux_told(
        &[("xstate", x86_64::Library::C)],
        &every_unit_look_ahead(),
        Some(v3.key()),
    );
}

#[test]
fn stack_moved_by_a_callee_matches_linux_x86_64() {
    x86_64::stack_moved_by_a_callee_matches_linux();
}

#[test]
fn deep_recursion_matches_linux_x86_64() {
    x86_64::deep_recursion_matches_linux();
}

#[test]
fn a_frame_past_the_budget_matches_linux_x86_64() {
    x86_64::a_frame_past_the_budget_matches_linux();
}

/// x86 has no `differences` program yet (it is ARM64's list of where its CPU and WebAssembly differ).
#[test]
fn cut_functions_and_split_translations_match_linux_x86_64() {
    x86_64::cut_functions_and_split_translations_match_linux(&[
        ("hello", "-O2"),
        ("exceptions", "-O2"),
        ("many_blocks", "-O2"),
    ]);
}

#[test]
fn stops_at_every_check_change_nothing_x86_64() {
    x86_64::stops_at_every_check_change_nothing(&[("strings", &[])]);
}

#[test]
fn unwinding_matches_linux_x86_64() {
    x86_64::unwinding_matches_linux();
}

#[test]
fn stacks_switched_by_the_program_match_linux_x86_64() {
    x86_64::stacks_switched_by_the_program_match_linux();
}

/// x86's written code has no announcement: the second write is seen by the write lock on its page.
#[test]
fn written_code_and_null_calls_match_linux_x86_64() {
    x86_64::written_code_and_null_calls_match_linux();
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
    for (optimisation, position) in x86_64::levels_and_positions() {
        let blob = x86_64::build_with(
            "self_written",
            optimisation,
            x86_64::Library::C,
            &defines,
            position,
        );
        let theirs = x86_64::native(&blob);
        assert_eq!(theirs.status, 0, "{}", blob.name);
        x86_64::every_look_ahead_matches(&blob, &theirs, |what, machine| {
            let written = machine.count(Reason::CodeWrittenWithoutAnnouncement);
            eprintln!("{what}: code written without announcement {written} times");
            assert!(written >= SELF_WRITTEN_PASSES, "{what}");
        });
        let name = Reason::CodeWrittenWithoutAnnouncement.name();
        for (what, ours) in x86_64::every_look_ahead_matches_in_chrome(&blob, &theirs) {
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
            format!(
                "SYS_exit={}",
                wcontainers_translator_test::x86_64::part::STAND_IN.exit
            ),
            format!("FIRST={STACK_CODE_FIRST}"),
            format!("SECOND={STACK_CODE_SECOND}"),
            format!("SHIFT={STACK_CODE_SHIFT}"),
            format!("PAGE_SIZE={}", wcontainers_abi::PAGE_SIZE),
        ];
        let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
        let blob = x86_64::build_with(
            "stack_code",
            "-O0",
            x86_64::Library::None,
            &defines,
            x86_64::Position::Fixed,
        );
        let theirs = x86_64::native(&blob);
        let status = (STACK_CODE_FIRST << STACK_CODE_SHIFT) + STACK_CODE_SECOND;
        assert_eq!(theirs.status, status as i32, "{}", blob.name);
        x86_64::every_look_ahead_matches(&blob, &theirs, |what, machine| {
            let given_up = machine.count(Reason::CodeTranslatedFromLiveStackFrame);
            eprintln!("{what}: frames given up {given_up} times");
            assert!(given_up > 0, "{what}");
        });
        let given_up = Reason::CodeTranslatedFromLiveStackFrame.name();
        for (what, ours) in x86_64::every_look_ahead_matches_in_chrome(&blob, &theirs) {
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
            format!(
                "SYS_exit={}",
                wcontainers_translator_test::x86_64::part::STAND_IN.exit
            ),
        ];
        let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
        let mut blob = x86_64::build_with(
            "far_frame",
            "-O0",
            x86_64::Library::None,
            &defines,
            x86_64::Position::Fixed,
        );
        blob.stack_bytes = blob.stack_bytes.max(2 * within);
        let theirs = x86_64::native(&blob);
        if descent == within {
            assert_eq!(theirs.status, (FAR_FRAME_LOW + FAR_FRAME_HIGH) as i32);
        }
        x86_64::every_look_ahead_matches(&blob, &theirs, |_, _| {});
        x86_64::every_look_ahead_matches_in_chrome(&blob, &theirs);
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
    use wcontainers_translator_test::x86_64::browser::SINGLE_STEP;
    use wcontainers_translator_test::x86_64::part::{SIGTRAP, TRAP_TRACE};
    let set = <X86_64 as wcontainers_guest_cpu::Guest>::REASONS
        .iter()
        .find(|r| r.0 == wcontainers_x86::Reason::TrapFlagSet)
        .unwrap()
        .1;
    for case in 0..TRAP_FLAG_CASES {
        let defines = [
            format!("CASE={case}"),
            format!("TF={}", wcontainers_x86::TF),
            format!(
                "SYS_write={}",
                wcontainers_translator_test::x86_64::part::STAND_IN.write
            ),
            format!(
                "SYS_exit={}",
                wcontainers_translator_test::x86_64::part::STAND_IN.exit
            ),
        ];
        let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
        let blob = x86_64::build_with(
            "trap_flag",
            "-O0",
            x86_64::Library::None,
            &defines,
            x86_64::Position::Fixed,
        );
        let (theirs, stop) = x86_64::native_with_stop(&blob);
        let (signal, code, registers) = stop.expect("the reference stops by a signal");
        assert_eq!((signal, code), (SIGTRAP, TRAP_TRACE), "case {case}");
        // The record holds the general registers in the block's order, then rip.
        let same_stop = |block: &wcontainers_x86::X86Block, what: &str| {
            let (gpr, rip) = registers.split_at(block.gpr.len());
            assert_eq!(&block.gpr[..], gpr, "general registers of {what}");
            assert_eq!(block.rip, rip[0], "rip of {what}");
        };
        for how in EVERY_LOOK_AHEAD {
            let (ours, machine, block) = x86_64::translated_for(&blob, how);
            let what = format!("case {case} at {how:?}");
            assert_eq!(ours, theirs, "{what}");
            same_stop(&block, &what);
            assert!(machine.count_named(set) > 0, "{what}");
        }
        for (what, ours) in x86_64::every_look_ahead_matches_in_chrome(&blob, &theirs) {
            let what = format!("case {case}: {what}");
            assert_eq!(ours.stop, SINGLE_STEP, "{what}");
            same_stop(&ours.block().expect("a stop's state"), &what);
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
    x86_64::main_is_in_the_full_translation(
        &[
            ("self_written", &self_written),
            ("written_by_another", &by_another),
        ],
        &x86_64::POSITIONS,
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
    use wcontainers_translator_test::x86_64::runner::Pause;
    let defines = written_by_another_defines();
    let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
    for (optimisation, position) in x86_64::levels_and_positions() {
        let blob = x86_64::build_with(
            "written_by_another",
            optimisation,
            x86_64::Library::C,
            &defines,
            position,
        );
        let theirs = x86_64::native(&blob);
        assert_eq!(theirs.status, 0, "{}", blob.name);
        let mut runs = Vec::new();
        for how in EVERY_LOOK_AHEAD {
            let mut machine = x86_64::ready_on(
                &blob,
                how,
                Default::default(),
                wcontainers_translator_test::x86_64::runner::probe(),
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
                x86_64::ran(&blob, &machine, stopped, &registers),
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
                x86_64::reachable_memory(&mut machine, &blob)
                    == x86_64::reachable_memory(&mut none_machine, &blob),
                "memory of {what}"
            );
        }
    }
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
#[allow(
    clippy::unit_arg,
    reason = "ARM64 offers no feature lists, so its offer is (), passed where x86-64 passes its list"
)]
mod arm64 {
    use super::*;
    use wcontainers_guest_cpu::Guest as _;
    use wcontainers_translator_test::arm64 as harness;
    pub(super) use wcontainers_translator_test::arm64::browser;
    pub(super) use wcontainers_translator_test::arm64::mixed::mixed;
    pub(super) use wcontainers_translator_test::arm64::programs::{
        Blob, Library, Position, Ran, boundaries, build, build_with, native, ran, reachable_memory,
        ready_on, ready_with, run_container, translated, translated_for,
        translated_stopping_everywhere,
    };
    pub(super) use wcontainers_translator_test::arm64::runner::Machine;
    use wcontainers_translator_test::arm64::{Cpu, part};

    include!("for_guest/programs.rs");

    /// Each place the guest's CPU and WebAssembly differ (the guest's `differences` program), printed as bits.
    pub(super) fn differences_match_linux() {
        match_linux(&[("differences", Library::C)]);
    }
    /// The same translated for every answer the host probe can give, at none and full, built with `defines`:
    /// whatever the host lacks is made up by slow paths that give Linux's output too. `check` is called on each
    /// run with its host and the blob.
    pub(super) fn differences_match_linux_on_every_host(
        defines: &[&str],
        check: impl Fn(<Cpu as wcontainers_guest_cpu::Guest>::Host, &str, &Blob, &Machine),
    ) {
        for optimisation in ["-O0", "-O2"] {
            let blob = build_with(
                "differences",
                optimisation,
                Library::C,
                defines,
                Position::Fixed,
            );
            let theirs = native(&blob);
            for &answer in part::PROBE_ANSWERS {
                let host = Cpu::host_from_probe(answer);
                for look_ahead in [LookAhead::None, LookAhead::Full] {
                    let mut machine = ready_on(
                        &blob,
                        (look_ahead, Optimise::Speed),
                        wcontainers_translator::Omit::default(),
                        host,
                    );
                    let (stopped, registers) = machine.run();
                    let ours = ran(&blob, &machine, stopped, &registers);
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
    /// Timing of the floating point and vector workloads (`programs/floats.c`, SCALE=1) at -O2, each on its own:
    /// native, and through the translator at full in wasmtime and Chrome, best of `WC_BENCH_RUNS` (default 3).
    /// `WC_BENCH_WORKLOADS` picks workloads (default 1,2,3,4), `WC_BENCH_CHROME=0` leaves Chrome out. Only
    /// measures; one command:
    /// `nix develop -c cargo test --release -p wcontainers-translator-test --test programs floats_speed_arm64 -- --ignored --nocapture`.
    pub(super) fn floats_speed() {
        const WORKLOADS: [&str; 4] = ["n-body", "matrix", "vector matrix", "byte differences"];
        let chosen: Vec<usize> = std::env::var("WC_BENCH_WORKLOADS")
            .map_or((1..=4).collect(), |v| {
                v.split(',').map(|n| n.parse().unwrap()).collect()
            });
        for k in chosen {
            let workload = format!("WORKLOAD={k}");
            let blob = build_with(
                "floats",
                "-O2",
                Library::C,
                &["SCALE=1", &workload],
                Position::Fixed,
            );
            time_against_native(&format!("floats {}", WORKLOADS[k - 1]), &blob);
        }
    }
    /// Timing of programs that switch stacks themselves, 2,000,000 round trips at -O2: native, and at full in
    /// wasmtime and Chrome, with the time of one switch over native's. Knobs as for [`floats_speed`]. Only
    /// measures; one command:
    /// `nix develop -c cargo test --release -p wcontainers-translator-test --test programs switching_speed_arm64 -- --ignored --nocapture`.
    pub(super) fn switching_speed() {
        const ROUND_TRIPS: f64 = 2_000_000.0;
        for name in ["fibers", "coroutines"] {
            let blob = build_with(
                name,
                "-O2",
                Library::C,
                &["SWITCHES=2000000"],
                Position::Fixed,
            );
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
    pub(super) fn time_against_native(name: &str, blob: &Blob) -> (f64, f64, Option<f64>) {
        use harness::programs::{load, native_seconds, translated_omitting};
        let runs = std::env::var("WC_BENCH_RUNS").map_or(3, |v| v.parse().unwrap());
        let chrome = std::env::var("WC_BENCH_CHROME").map_or(true, |v| v != "0");
        let theirs = native(blob);
        let native = native_seconds(blob, runs);
        let mut best = f64::MAX;
        for _ in 0..runs {
            let (ours, _, _, time) = translated_omitting(
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
            let report = browser::repeated(
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
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    use wcontainers_translator_test::x86_64 as harness;
    use wcontainers_translator_test::x86_64::Cpu;
    pub(super) use wcontainers_translator_test::x86_64::browser;
    pub(super) use wcontainers_translator_test::x86_64::mixed::mixed;
    pub(super) use wcontainers_translator_test::x86_64::programs::{
        Blob, Library, Position, Ran, boundaries, build, build_for, build_with, native,
        native_with_stop, ran, reachable_memory, ready_on, ready_with, translated, translated_for,
        translated_stopping_everywhere,
    };
    pub(super) use wcontainers_translator_test::x86_64::runner::Machine;

    include!("for_guest/programs.rs");
}
