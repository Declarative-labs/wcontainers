//! The runner and look-ahead none on a few hand-made programs, each guest's own (`GuestTest::smoke`).

use wcontainers_abi::{Exit, Permissions};
use wcontainers_arm64::Arm64;
use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::guest::{GuestTest, UNKNOWN_SYSTEM_CALL};
use wcontainers_translator_test::runner::{Machine, Stopped, Syscalls, fields};
use wcontainers_x86::X86_64;

const LOAD: u64 = 0x10000;
/// The one executable page the programs run in, and the stack's page below it.
const PAGE: u64 = wcontainers_abi::PAGE_SIZE;

/// Load `code` at `LOAD` at `look_ahead`, every register 0 but the pc and the stack pointer, which is at the
/// top of the writable page below.
fn ready<G: GuestTest>(code: &[u8], look_ahead: LookAhead) -> Machine<G> {
    let mut machine = Machine::<G>::new(1 << 20, LOAD, (PAGE / G::ALIGN) as u32, Syscalls::StandIn);
    machine.set_permissions(LOAD..LOAD + PAGE, Permissions::READ | Permissions::EXECUTE);
    machine.set_permissions(LOAD - PAGE..LOAD, Permissions::READ | Permissions::WRITE);
    machine.load(code, 0..code.len() as u64, (LOAD, None), &[0], look_ahead);
    machine.set_registers(&G::initial_block(LOAD, LOAD));
    machine
}

/// Run at none after `prepare` has done something to the machine; the stop and the fields after.
fn run<G: GuestTest>(
    code: &[u8],
    prepare: impl FnOnce(&Machine<G>),
) -> (Stopped, Vec<u128>, Machine<G>) {
    let mut machine = ready::<G>(code, LookAhead::None);
    prepare(&machine);
    let (stopped, registers) = machine.run();
    (stopped, fields::<G>(&registers).0, machine)
}

fn exit_status<G: GuestTest>() {
    let (stopped, _, _) = run::<G>(&G::smoke().exit, |_| {});
    assert_eq!(stopped, Stopped::Finished(42));
}

fn loop_and_call<G: GuestTest>() {
    let (stopped, _, machine) = run::<G>(&G::smoke().loop_and_call, |_| {});
    assert_eq!(stopped, Stopped::Finished(55));
    assert_eq!(machine.count(Reason::StoppedByFlag), 0);
}

/// The stand-in behind the stop hook clears the flag and the thread carries on.
fn a_raised_stop_flag_is_cleared_and_the_thread_carries_on<G: GuestTest>() {
    let (stopped, _, machine) = run::<G>(&G::smoke().loop_and_call, Machine::raise_stop_flag);
    assert_eq!(stopped, Stopped::Finished(55));
    assert_eq!(machine.count(Reason::StoppedByFlag), 1);
    let warnings: Vec<_> = machine
        .warnings()
        .iter()
        .filter(|w| w.reason == Reason::StoppedByFlag.name())
        .map(|w| w.detail)
        .collect();
    assert_eq!(warnings, [LOAD]);
}

fn slow_paths_are_counted_and_warned_once_per_place<G: GuestTest>() {
    let (stopped, _, machine) = run::<G>(&G::smoke().unknown_call, |_| {});
    assert_eq!(stopped, Stopped::Finished(3));
    assert_eq!(machine.count(Reason::SystemCallNotImplemented), 2);
    let warnings: Vec<_> = machine
        .warnings()
        .iter()
        .filter(|w| w.reason == Reason::SystemCallNotImplemented.name())
        .map(|w| (w.detail, w.count))
        .collect();
    assert_eq!(warnings, [(UNKNOWN_SYSTEM_CALL, 1)]);
}

/// A flag raised by another thread while a translated loop runs is seen, at every look-ahead: the check must
/// read the flag from memory each time round.
fn stop_flag_raised_while_running<G: GuestTest>() {
    let smoke = G::smoke();
    for look_ahead in [LookAhead::None, LookAhead::Full] {
        let mut machine = ready::<G>(&smoke.long_loop, look_ahead);
        let raise = machine.stop_flag_raiser();
        let raiser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            raise();
        });
        let (stopped, registers) = machine.run();
        raiser.join().unwrap();
        assert_eq!(stopped, Stopped::Finished(0), "{look_ahead:?}");
        assert_eq!(machine.count(Reason::StoppedByFlag), 1, "{look_ahead:?}");
        // It was seen inside the loop, which then carried on to the end.
        let seen_at = machine.warnings()[0].detail - LOAD;
        assert!(
            smoke.long_loop_body.contains(&seen_at),
            "{look_ahead:?}: {seen_at:#x}"
        );
        assert_eq!(fields::<G>(&registers).0[smoke.long_loop_counter], 0);
    }
}

/// Checks that a stop raised by another thread with the progress count kept is seen in the loop, as the flag is.
fn stop_raised_while_running_with_the_count_kept<G: GuestTest>() {
    let smoke = G::smoke();
    for look_ahead in [LookAhead::None, LookAhead::Full] {
        let mut machine =
            Machine::<G>::new(1 << 20, LOAD, (PAGE / G::ALIGN) as u32, Syscalls::StandIn);
        machine.keep_progress_count();
        machine.set_permissions(LOAD..LOAD + PAGE, Permissions::READ | Permissions::EXECUTE);
        machine.set_permissions(LOAD - PAGE..LOAD, Permissions::READ | Permissions::WRITE);
        let code = &smoke.long_loop;
        machine.load(code, 0..code.len() as u64, (LOAD, None), &[0], look_ahead);
        machine.set_registers(&G::initial_block(LOAD, LOAD));
        let raise = machine.stop_flag_raiser();
        let raiser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            raise();
        });
        let (stopped, registers) = machine.run();
        raiser.join().unwrap();
        assert_eq!(stopped, Stopped::Finished(0), "{look_ahead:?}");
        assert_eq!(machine.count(Reason::StoppedByFlag), 1, "{look_ahead:?}");
        let seen_at = machine.warnings()[0].detail - LOAD;
        assert!(
            smoke.long_loop_body.contains(&seen_at),
            "{look_ahead:?}: {seen_at:#x}"
        );
        assert_eq!(fields::<G>(&registers).0[smoke.long_loop_counter], 0);
    }
}

/// An instruction of a feature we do not offer raises illegal instruction, at the instruction, and is
/// counted as unsupported; bytes in no form raise it uncounted, as on any CPU.
fn instructions_not_offered_are_illegal_and_counted<G: GuestTest>() {
    let smoke = G::smoke();
    for (code, counted) in &smoke.illegal {
        let what = G::disassemble(code);
        let mut program = smoke.mark.clone();
        program.extend(code);
        let (stopped, fields, machine) = run::<G>(&program, |_| {});
        assert_eq!(stopped, Stopped::Exit(Exit::IllegalInstruction), "{what}");
        assert_eq!(
            fields[G::PC_FIELD],
            u128::from(LOAD) + smoke.mark.len() as u128,
            "{what}"
        );
        assert_eq!(fields[smoke.marked], 1, "{what}");
        assert_eq!(
            machine.count(Reason::UnsupportedInstructionReached),
            *counted,
            "{what}"
        );
    }
}

#[test]
fn exit_status_arm64() {
    exit_status::<Arm64>();
}

#[test]
fn loop_and_call_arm64() {
    loop_and_call::<Arm64>();
}

#[test]
fn a_raised_stop_flag_is_cleared_and_the_thread_carries_on_arm64() {
    a_raised_stop_flag_is_cleared_and_the_thread_carries_on::<Arm64>();
}

#[test]
fn slow_paths_are_counted_and_warned_once_per_place_arm64() {
    slow_paths_are_counted_and_warned_once_per_place::<Arm64>();
}

#[test]
fn stop_flag_raised_while_running_arm64() {
    stop_flag_raised_while_running::<Arm64>();
}

#[test]
fn stop_raised_while_running_with_the_count_kept_arm64() {
    stop_raised_while_running_with_the_count_kept::<Arm64>();
}

#[test]
fn instructions_not_offered_are_illegal_and_counted_arm64() {
    instructions_not_offered_are_illegal_and_counted::<Arm64>();
}

#[test]
fn exit_status_x86_64() {
    exit_status::<X86_64>();
}

#[test]
fn loop_and_call_x86_64() {
    loop_and_call::<X86_64>();
}

#[test]
fn a_raised_stop_flag_is_cleared_and_the_thread_carries_on_x86_64() {
    a_raised_stop_flag_is_cleared_and_the_thread_carries_on::<X86_64>();
}

#[test]
fn slow_paths_are_counted_and_warned_once_per_place_x86_64() {
    slow_paths_are_counted_and_warned_once_per_place::<X86_64>();
}

#[test]
fn stop_flag_raised_while_running_x86_64() {
    stop_flag_raised_while_running::<X86_64>();
}

#[test]
fn stop_raised_while_running_with_the_count_kept_x86_64() {
    stop_raised_while_running_with_the_count_kept::<X86_64>();
}

#[test]
fn instructions_not_offered_are_illegal_and_counted_x86_64() {
    instructions_not_offered_are_illegal_and_counted::<X86_64>();
}
