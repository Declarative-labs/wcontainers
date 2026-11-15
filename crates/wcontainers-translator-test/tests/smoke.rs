//! The runner and look-ahead none on a few hand-encoded programs.

use wcontainers_abi::{Permissions, RegisterBlock};
use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::runner::{Machine, Stopped, Syscalls};

const LOAD: u64 = 0x10000;

fn run(words: &[u32]) -> (Stopped, RegisterBlock, Machine) {
    run_with(words, |_| {})
}

/// Run after `prepare` has done something to the machine.
fn run_with(words: &[u32], prepare: impl FnOnce(&Machine)) -> (Stopped, RegisterBlock, Machine) {
    let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let mut machine = Machine::new(1 << 20, LOAD, 1024, Syscalls::StandIn);
    machine.set_permissions(
        LOAD..LOAD + 0x1000,
        Permissions::READ | Permissions::EXECUTE,
    );
    machine.load(&blob, 0..blob.len() as u64, LOAD, &[0], LookAhead::None);
    let mut registers = RegisterBlock::ZERO;
    registers.pc = LOAD;
    machine.set_registers(&registers);
    prepare(&machine);
    let (stopped, registers) = machine.run();
    (stopped, registers, machine)
}

#[test]
fn exit_status() {
    // movz x0, #42; movz x8, #93; svc #0
    let (stopped, _, _) = run(&[0xd280_0540, 0xd280_0ba8, 0xd400_0001]);
    assert_eq!(stopped, Stopped::Finished(42));
}

// x0 = 0; x1 = 10; loop: bl add; subs x1, x1, #1; b.ne loop; exit(x0)
// add: add x0, x0, x1; ret
const LOOP_AND_CALL: [u32; 9] = [
    0xd280_0000, // movz x0, #0
    0xd280_0141, // movz x1, #10
    0x9400_0005, // bl +5 (add)
    0xf100_0421, // subs x1, x1, #1
    0x54ff_ffc1, // b.ne -2
    0xd280_0ba8, // movz x8, #93
    0xd400_0001, // svc #0
    0x8b01_0000, // add x0, x0, x1
    0xd65f_03c0, // ret
];

#[test]
fn loop_and_call() {
    let (stopped, _, machine) = run(&LOOP_AND_CALL);
    assert_eq!(stopped, Stopped::Finished(55));
    assert_eq!(machine.count(Reason::StoppedByFlag), 0);
}

/// The stand-in behind the stop hook clears the flag and the thread carries on.
#[test]
fn a_raised_stop_flag_is_cleared_and_the_thread_carries_on() {
    let (stopped, _, machine) = run_with(&LOOP_AND_CALL, Machine::raise_stop_flag);
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

#[test]
fn slow_paths_are_counted_and_warned_once_per_place() {
    // movz x8, #999; svc #0; svc #0; movz x0, #3; movz x8, #93; svc #0
    let words = [
        0xd280_7ce8,
        0xd400_0001,
        0xd400_0001,
        0xd280_0060,
        0xd280_0ba8,
        0xd400_0001,
    ];
    let (stopped, _, machine) = run(&words);
    assert_eq!(stopped, Stopped::Finished(3));
    assert_eq!(machine.count(Reason::SystemCallNotImplemented), 2);
    let warnings: Vec<_> = machine
        .warnings()
        .iter()
        .filter(|w| w.reason == Reason::SystemCallNotImplemented.name())
        .map(|w| (w.detail, w.count))
        .collect();
    assert_eq!(warnings, [(999, 1)]);
}

/// A flag raised by another thread while a translated loop runs is seen, at every look-ahead: the check must
/// read the flag from memory each time round.
#[test]
fn stop_flag_raised_while_running() {
    // movz x0, #0x4000, lsl #16; loop: subs x0, x0, #1; b.ne loop; movz x8, #93; svc #0
    let words = [
        0xd2a8_0000,
        0xf100_0400,
        0x54ff_ffe1,
        0xd280_0ba8,
        0xd400_0001,
    ];
    let blob: Vec<u8> = words.iter().flat_map(|w: &u32| w.to_le_bytes()).collect();
    for look_ahead in [LookAhead::None, LookAhead::Full] {
        let mut machine = Machine::new(1 << 20, LOAD, 1024, Syscalls::StandIn);
        machine.set_permissions(
            LOAD..LOAD + 0x1000,
            Permissions::READ | Permissions::EXECUTE,
        );
        machine.load(&blob, 0..blob.len() as u64, LOAD, &[0], look_ahead);
        let mut registers = RegisterBlock::ZERO;
        registers.pc = LOAD;
        machine.set_registers(&registers);
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
        let seen_at = machine.warnings()[0].detail;
        assert!(
            [LOAD + 4, LOAD + 8].contains(&seen_at),
            "{look_ahead:?}: {seen_at:#x}"
        );
        assert_eq!(registers.x[0], 0);
    }
}
