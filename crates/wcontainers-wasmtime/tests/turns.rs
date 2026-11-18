//! A counted turn ends after its progress however often the guest reads the clock, and leaving `run`
//! from a hook is counted by why.

use wcontainers_abi::{Cpu, Exit, Permissions, RegisterBlock, Resume, Warning};
use wcontainers_arm64::Arm64;
use wcontainers_runtime::{Answer, Carry, Execution, Leaving, Turn};
use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise, reason};
use zerocopy::FromZeros;

const CODE: u64 = 0x10000;
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

fn spin(look_ahead: LookAhead) {
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

    let mut recorder = Recorder::default();
    let turn = Turn { limit: Some(LIMIT) };
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
}

#[test]
fn clock_reads_do_not_hold_a_counted_turn_full() {
    spin(LookAhead::Full);
}

#[test]
fn clock_reads_do_not_hold_a_counted_turn_none() {
    spin(LookAhead::None);
}
