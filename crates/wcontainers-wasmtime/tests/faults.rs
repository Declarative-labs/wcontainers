//! Registers the kernel sets in answer to a fault, as a signal's delivery does, are where the thread
//! runs from next.

use wcontainers_abi::{
    Access, Cpu, Exit, PAGE_SIZE, Permissions, RegisterBlock, Resume, ThreadId, Warning,
};
use wcontainers_arm64::Arm64;
use wcontainers_runtime_single_kernel::{Answer, Carry, Execution, Leaving, Turn};
use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise};
use zerocopy::FromZeros;

use wcontainers_abi::LOWEST_ADDRESS as CODE;
/// `ldr x0, [x1]; svc #0` at the start, and a handler `movz x0, #9; svc #0` after it.
const PROGRAM: [u32; 4] = [0xf940_0020, 0xd400_0001, 0xd280_0120, 0xd400_0001];
const HANDLER: u64 = CODE + 8;
const UNMAPPED: u64 = 0x50_0000;

struct Kernel {
    thread: ThreadId,
    exits: Vec<Exit>,
}

impl Answer for Kernel {
    fn exit(&mut self, cpu: &mut dyn Cpu, exit: Exit, _: u64) -> Carry {
        self.exits.push(exit);
        if matches!(exit, Exit::MemoryFault { .. }) {
            let mut registers = RegisterBlock::new_zeroed();
            cpu.registers(self.thread, &mut registers);
            registers.pc = HANDLER;
            cpu.set_registers(self.thread, &registers);
            return Carry::Leave(Leaving::NewRegisters);
        }
        Carry::Leave(Leaving::Finished)
    }

    fn clock(&mut self, _: u64) -> u64 {
        0
    }

    fn warn(&mut self, _: Warning<'_>) {}
}

fn handled(look_ahead: LookAhead) {
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), look_ahead, Optimise::Speed, ());
    let memory = cpu.memory_new().unwrap();
    let code: Vec<u8> = PROGRAM.iter().flat_map(|w| w.to_le_bytes()).collect();
    cpu.write(memory, CODE, &code).unwrap();
    let rx = Permissions::READ | Permissions::EXECUTE;
    cpu.set_permissions(memory, CODE, PAGE_SIZE, rx).unwrap();
    cpu.code_changed(memory, CODE, PAGE_SIZE, &[CODE]);
    let thread = cpu.thread_new(memory).unwrap();
    let mut registers = RegisterBlock::new_zeroed();
    registers.pc = CODE;
    registers.x[1] = UNMAPPED;
    cpu.set_registers(thread, &registers);
    let mut kernel = Kernel {
        thread,
        exits: Vec::new(),
    };
    let turn = Turn::Counted { limit: None };
    let Ok(()) = cpu.run(thread, Resume::FromRegisters, turn, &mut kernel);
    let Ok(()) = cpu.run(thread, Resume::FromRegisters, turn, &mut kernel);
    match kernel.exits.as_slice() {
        [
            Exit::MemoryFault {
                address: UNMAPPED,
                access: Access::Read,
            },
            Exit::Syscall(call),
        ] => assert_eq!(call.arguments[0], 9),
        other => panic!("{other:?}"),
    }
}

#[test]
fn registers_set_after_a_fault_are_run_from_full() {
    handled(LookAhead::Full);
}

#[test]
fn registers_set_after_a_fault_are_run_from_none() {
    handled(LookAhead::None);
}
