//! The runner and look-ahead none on a few hand-encoded programs.

use wcontainers_abi::{Permissions, RegisterBlock};
use wcontainers_translator_test::runner::{Machine, Stopped, Syscalls};

const LOAD: u64 = 0x10000;

fn run(words: &[u32]) -> (Stopped, RegisterBlock, Machine) {
    let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let mut machine = Machine::new(1 << 20, LOAD, 1024, Syscalls::StandIn);
    machine.set_permissions(
        LOAD..LOAD + 0x1000,
        Permissions::READ | Permissions::EXECUTE,
    );
    machine.load(&blob, 0..blob.len() as u64, LOAD, &[0]);
    let mut registers = RegisterBlock::ZERO;
    registers.pc = LOAD;
    machine.set_registers(&registers);
    let (stopped, registers) = machine.run();
    (stopped, registers, machine)
}

#[test]
fn exit_status() {
    // movz x0, #42; movz x8, #93; svc #0
    let (stopped, _, _) = run(&[0xd280_0540, 0xd280_0ba8, 0xd400_0001]);
    assert_eq!(stopped, Stopped::Finished(42));
}

#[test]
fn loop_and_call() {
    // x0 = 0; x1 = 10; loop: bl add; subs x1, x1, #1; b.ne loop; exit(x0)
    // add: add x0, x0, x1; ret
    let words = [
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
    let (stopped, _, _) = run(&words);
    assert_eq!(stopped, Stopped::Finished(55));
}

#[test]
fn slow_paths_are_counted_and_warned_once_per_place() {
    use wcontainers_translator::contract::Reason;
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
