//! A counted turn ends after its progress however often the guest reads the clock, leaving `run`
//! from a hook is counted by why, and a counter read on an executor brought back from a save is a
//! counted slow path. A free turn counts nothing and ends when another host thread raises the stop
//! flag, or at once when it was raised before.

use wcontainers_abi::{Cpu, Exit, Permissions, RegisterBlock, Resume, Warning};
use wcontainers_arm64::Arm64;
use wcontainers_runtime::{Answer, Carry, Execution, Executor, Leaving, Turn};
use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise, reason};
use zerocopy::FromZeros;

use wcontainers_abi::LOWEST_ADDRESS as CODE;
/// `mrs x0, cntvct_el0; b .-4`: reads the counter in a loop with no system call.
const SPIN: [u32; 2] = [0xd53b_e040, 0x17ff_ffff];
const LIMIT: u64 = 1000;

#[derive(Default)]
struct Recorder {
    clock_reads: u64,
    told: u64,
    exits: Vec<(Exit, u64)>,
    warnings: Vec<(String, u64)>,
}

impl Answer for Recorder {
    fn exit(&mut self, _: &mut dyn wcontainers_abi::Cpu, exit: Exit, progress: u64) -> Carry {
        self.told += progress;
        self.exits.push((exit, progress));
        Carry::Leave(Leaving::AnotherThread)
    }

    fn clock(&mut self, progress: u64) -> u64 {
        self.clock_reads += 1;
        self.told += progress;
        assert!(self.clock_reads < 100 * LIMIT, "the turn never ended");
        self.told
    }

    fn warn(&mut self, warning: Warning<'_>) {
        self.warnings
            .push((warning.reason.to_owned(), warning.detail));
    }
}

/// Runs the spin for one counted turn, on an executor brought back from a save when `restored`; the
/// executor's warnings.
fn spin(look_ahead: LookAhead, restored: bool) -> Vec<(String, u64)> {
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), look_ahead, Optimise::Speed, ());
    let memory = cpu.memory_new().unwrap();
    let code: Vec<u8> = SPIN.iter().flat_map(|w| w.to_le_bytes()).collect();
    cpu.write(memory, CODE, &code).unwrap();
    let page = wcontainers_abi::PAGE_SIZE;
    cpu.set_permissions(memory, CODE, page, Permissions::READ | Permissions::EXECUTE)
        .unwrap();
    cpu.code_changed(memory, CODE, page, &[CODE]);
    let thread = cpu.thread_new(memory).unwrap();
    let mut registers = RegisterBlock::new_zeroed();
    cpu.registers(thread, &mut registers);
    registers.pc = CODE;
    cpu.set_registers(thread, &registers);
    if restored {
        cpu = cpu.restored();
    }

    let mut recorder = Recorder::default();
    let turn = Turn::Counted { limit: Some(LIMIT) };
    let Ok(()) = cpu.run(thread, Resume::FromRegisters, turn, &mut recorder);

    assert!(
        matches!(recorder.exits.as_slice(), [(Exit::Stop, _)]),
        "{:?}",
        recorder.exits
    );
    assert_eq!(
        recorder.told, LIMIT,
        "progress told by clock reads and the exit"
    );
    assert!(recorder.clock_reads > 0);
    assert!(
        recorder
            .warnings
            .iter()
            .any(|w| *w == (reason::LEFT_ANOTHER_THREAD.to_owned(), reason::NOT_A_CALL)),
        "{:?}",
        recorder.warnings
    );
    recorder.warnings
}

fn without_offset(warnings: &[(String, u64)]) -> bool {
    warnings
        .iter()
        .any(|(reason, _)| reason == reason::COUNTER_WITHOUT_KERNEL_OFFSET)
}

#[test]
fn clock_reads_do_not_hold_a_counted_turn_full() {
    assert!(!without_offset(&spin(LookAhead::Full, false)));
}

#[test]
fn clock_reads_do_not_hold_a_counted_turn_none() {
    assert!(!without_offset(&spin(LookAhead::None, false)));
}

#[test]
fn counter_reads_after_a_restore_are_warned() {
    let warnings = spin(LookAhead::Full, true);
    assert!(without_offset(&warnings), "{warnings:?}");
}

/// Runs `b .` in free turns: one ended by a raise from another host thread, then one raised before it
/// started.
fn free(look_ahead: LookAhead) {
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), look_ahead, Optimise::Speed, ());
    let memory = cpu.memory_new().unwrap();
    cpu.write(memory, CODE, &0x1400_0000u32.to_le_bytes())
        .unwrap();
    let page = wcontainers_abi::PAGE_SIZE;
    cpu.set_permissions(memory, CODE, page, Permissions::READ | Permissions::EXECUTE)
        .unwrap();
    cpu.code_changed(memory, CODE, page, &[CODE]);
    let thread = cpu.thread_new(memory).unwrap();
    let mut registers = RegisterBlock::new_zeroed();
    registers.pc = CODE;
    cpu.set_registers(thread, &registers);

    let flag = cpu.stop_flag();
    let raiser = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        flag.raise();
    });
    let mut recorder = Recorder::default();
    let Ok(()) = cpu.run(thread, Resume::FromRegisters, Turn::Free, &mut recorder);
    raiser.join().unwrap();
    assert_eq!(recorder.exits, [(Exit::Stop, 0)]);
    assert!(
        !recorder
            .warnings
            .iter()
            .any(|(reason, _)| reason == reason::PROGRESS_BY_STOP_CHECKS),
        "{:?}",
        recorder.warnings
    );

    cpu.stop_flag().raise();
    let mut recorder = Recorder::default();
    let Ok(()) = cpu.run(thread, Resume::Continue, Turn::Free, &mut recorder);
    assert_eq!(recorder.exits, [(Exit::Stop, 0)]);
}

#[test]
fn a_free_turn_ends_when_the_flag_is_raised_full() {
    free(LookAhead::Full);
}

#[test]
fn a_free_turn_ends_when_the_flag_is_raised_none() {
    free(LookAhead::None);
}
