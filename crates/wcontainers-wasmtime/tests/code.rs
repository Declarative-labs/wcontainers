//! A notice of code counts "code changed after it ran" only when the range's bytes changed since it
//! was translated, and the thread then runs the new bytes. A page of a range that may not run code yet
//! faults when reached, and runs what it holds once it may.

use wcontainers_abi::{Access, Cpu, Exit, PAGE_SIZE, Permissions, RegisterBlock, Resume, Warning};
use wcontainers_arm64::Arm64;
use wcontainers_runtime::{Answer, Carry, Execution, Leaving, Turn};
use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise, reason};
use zerocopy::FromZeros;

const CODE: u64 = 0x10000;
/// `movz x0, #1; svc #0`.
const ONE: [u32; 2] = [0xd280_0020, 0xd400_0001];
/// `movz x0, #2; svc #0`.
const TWO: [u32; 2] = [0xd280_0040, 0xd400_0001];

#[derive(Default)]
struct Recorder {
    exits: Vec<Exit>,
}

impl Answer for Recorder {
    fn exit(&mut self, _: &mut dyn Cpu, exit: Exit, _: u64) -> Carry {
        self.exits.push(exit);
        Carry::Leave(Leaving::Finished)
    }

    fn clock(&mut self, _: u64) -> u64 {
        0
    }

    fn warn(&mut self, _: Warning<'_>) {}
}

fn bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// Runs a new thread from the code's start; x0 at its system call.
fn run(cpu: &mut CpuSide<Arm64>, memory: wcontainers_abi::MemoryId) -> u64 {
    let thread = cpu.thread_new(memory).unwrap();
    let mut registers = RegisterBlock::new_zeroed();
    registers.pc = CODE;
    cpu.set_registers(thread, &registers);
    let mut recorder = Recorder::default();
    let Ok(()) = cpu.run(
        thread,
        Resume::FromRegisters,
        Turn { limit: None },
        &mut recorder,
    );
    cpu.thread_drop(thread);
    match recorder.exits.as_slice() {
        [Exit::Syscall(call)] => call.arguments[0],
        other => panic!("{other:?}"),
    }
}

fn changed(cpu: &CpuSide<Arm64>) -> u64 {
    cpu.counters()
        .iter()
        .find(|c| c.reason == reason::CODE_CHANGED_AFTER_IT_RAN)
        .map_or(0, |c| c.count)
}

fn notices(look_ahead: LookAhead) {
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), look_ahead, Optimise::Speed, ());
    let memory = cpu.memory_new().unwrap();
    let rx = Permissions::READ | Permissions::EXECUTE;
    cpu.write(memory, CODE, &bytes(&ONE)).unwrap();
    cpu.set_permissions(memory, CODE, PAGE_SIZE, rx).unwrap();
    cpu.code_changed(memory, CODE, PAGE_SIZE, &[CODE]);
    assert_eq!(run(&mut cpu, memory), 1);

    // A permission change that leaves the bytes as they were.
    cpu.set_permissions(memory, CODE, PAGE_SIZE, Permissions::READ)
        .unwrap();
    cpu.code_changed(memory, CODE, PAGE_SIZE, &[]);
    cpu.set_permissions(memory, CODE, PAGE_SIZE, rx).unwrap();
    cpu.code_changed(memory, CODE, PAGE_SIZE, &[]);
    assert_eq!(run(&mut cpu, memory), 1);
    assert_eq!(changed(&cpu), 0, "{:?}", cpu.counters());

    // New bytes, written between two notices as a program that makes code does.
    cpu.set_permissions(
        memory,
        CODE,
        PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    )
    .unwrap();
    cpu.code_changed(memory, CODE, PAGE_SIZE, &[]);
    cpu.write(memory, CODE, &bytes(&TWO)).unwrap();
    cpu.set_permissions(memory, CODE, PAGE_SIZE, rx).unwrap();
    cpu.code_changed(memory, CODE, PAGE_SIZE, &[]);
    assert_eq!(run(&mut cpu, memory), 2);
    assert_eq!(changed(&cpu), 1, "{:?}", cpu.counters());
}

#[test]
fn only_new_bytes_count_as_changed_code_full() {
    notices(LookAhead::Full);
}

#[test]
fn only_new_bytes_count_as_changed_code_none() {
    notices(LookAhead::None);
}

/// A range whose second page comes later, as a file's page that is read when first touched: the first
/// page's code runs on into it.
fn page_comes_later(look_ahead: LookAhead) {
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), look_ahead, Optimise::Speed, ());
    let memory = cpu.memory_new().unwrap();
    let rx = Permissions::READ | Permissions::EXECUTE;
    let second = CODE + PAGE_SIZE;
    // `movz x0, #3; nop`, then on into the second page.
    cpu.write(memory, second - 8, &bytes(&[0xd280_0060, 0xd503_201f]))
        .unwrap();
    cpu.set_permissions(memory, CODE, PAGE_SIZE, rx).unwrap();
    cpu.code_changed(memory, CODE, 2 * PAGE_SIZE, &[second - 8]);
    let thread = cpu.thread_new(memory).unwrap();
    let mut registers = RegisterBlock::new_zeroed();
    registers.pc = second - 8;
    cpu.set_registers(thread, &registers);
    let mut recorder = Recorder::default();
    let turn = Turn { limit: None };
    let Ok(()) = cpu.run(thread, Resume::FromRegisters, turn, &mut recorder);
    assert_eq!(
        recorder.exits,
        [Exit::MemoryFault {
            address: second,
            access: Access::Execute
        }]
    );
    cpu.registers(thread, &mut registers);
    assert_eq!(registers.pc, second);

    // The page arrives: `svc #0`.
    cpu.write(memory, second, &bytes(&[0xd400_0001])).unwrap();
    cpu.set_permissions(memory, second, PAGE_SIZE, rx).unwrap();
    let mut recorder = Recorder::default();
    let Ok(()) = cpu.run(thread, Resume::FromRegisters, turn, &mut recorder);
    match recorder.exits.as_slice() {
        [Exit::Syscall(call)] => assert_eq!(call.arguments[0], 3),
        other => panic!("{other:?}"),
    }
    assert_eq!(changed(&cpu), 0, "{:?}", cpu.counters());
}

#[test]
fn a_page_that_comes_later_faults_then_runs_full() {
    page_comes_later(LookAhead::Full);
}

#[test]
fn a_page_that_comes_later_faults_then_runs_none() {
    page_comes_later(LookAhead::None);
}
