//! A translation kept in the cache's directory is found there by a new compiler, and runs the same.

use wcontainers_abi::{
    Cpu, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, Resume, Warning,
};
use wcontainers_arm64::Arm64;
use wcontainers_runtime::{Answer, Carry, Execution, Leaving, Turn};
use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise, Origin};
use zerocopy::FromZeros;

const CODE: u64 = 0x10000;
/// `movz x0, #7; add x0, x0, #5; svc #0`.
const PROGRAM: [u32; 3] = [0xd280_00e0, 0x9100_1400, 0xd400_0001];

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

/// Runs the program on a new back end of `compiler`; x0 at its system call and where its code came
/// from.
fn run(compiler: std::sync::Arc<Compiler<Arm64>>, look_ahead: LookAhead) -> (u64, Origin) {
    let mut cpu = CpuSide::<Arm64>::new(compiler, look_ahead, Optimise::Speed, ());
    let memory: MemoryId = cpu.memory_new().unwrap();
    let code: Vec<u8> = PROGRAM.iter().flat_map(|w| w.to_le_bytes()).collect();
    cpu.write(memory, CODE, &code).unwrap();
    let rx = Permissions::READ | Permissions::EXECUTE;
    cpu.set_permissions(memory, CODE, PAGE_SIZE, rx).unwrap();
    cpu.code_changed(memory, CODE, PAGE_SIZE, &[CODE]);
    let thread = cpu.thread_new(memory).unwrap();
    let mut registers = RegisterBlock::new_zeroed();
    registers.pc = CODE;
    cpu.set_registers(thread, &registers);
    let mut recorder = Recorder::default();
    let turn = Turn { limit: None };
    let Ok(()) = cpu.run(thread, Resume::FromRegisters, turn, &mut recorder);
    let [Exit::Syscall(call)] = recorder.exits.as_slice() else {
        panic!("{:?}", recorder.exits);
    };
    (call.arguments[0], cpu.translated()[0].origin)
}

#[test]
fn a_new_compiler_finds_translations_on_disk() {
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("cache-test");
    let _ = std::fs::remove_dir_all(&directory);
    for look_ahead in [LookAhead::Full, LookAhead::None] {
        let (value, origin) = run(Compiler::new(Some(directory.clone())), look_ahead);
        assert_eq!(value, 12);
        assert!(matches!(origin, Origin::Made { .. }), "{origin:?}");
        let compiler = Compiler::new(Some(directory.clone()));
        let (value, origin) = run(compiler.clone(), look_ahead);
        assert_eq!(value, 12);
        assert!(matches!(origin, Origin::Disk(_)), "{origin:?}");
        assert_eq!(run(compiler, look_ahead), (12, Origin::Memory));
    }
    std::fs::remove_dir_all(&directory).unwrap();
}
