//! A page the kernel makes read-only during a system call is read-only for the very next access of the code that
//! made the call, also where that access is in the stack frame the code checked before the call.

use wcontainers_abi::LOWEST_ADDRESS as CODE;
use wcontainers_abi::{
    Access, Cpu, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, Resume, Warning,
};
use wcontainers_arm64::Arm64;
use wcontainers_executor_wasmtime::{Compiler, CpuSide, LookAhead, Optimise};
use wcontainers_runtime_single_kernel::{Answer, Carry, Execution, Leaving, Turn};
use zerocopy::FromZeros;

/// `sub sp, sp, #64; svc #0; str x1, [sp, #8]; svc #0`: a frame made and checked, a call, a store into the frame.
const PROGRAM: [u32; 4] = [0xd101_03ff, 0xd400_0001, 0xf900_07e1, 0xd400_0001];
const STACK: u64 = 0x60_0000;
const TOP: u64 = STACK + 0x800;
/// Where the store goes: 8 bytes above the stack pointer the frame left.
const STORED_AT: u64 = TOP - 64 + 8;
const VALUE: u64 = 0x5eed;

/// At the first system call the stack's page becomes read-only and the thread carries on; any other exit ends it.
struct Kernel {
    memory: MemoryId,
    exits: Vec<Exit>,
}

impl Answer for Kernel {
    fn exit(&mut self, cpu: &mut dyn Cpu, exit: Exit, _: u64) -> Carry {
        self.exits.push(exit);
        if self.exits.len() == 1 && matches!(exit, Exit::Syscall(_)) {
            cpu.set_permissions(self.memory, STACK, PAGE_SIZE, Permissions::READ)
                .expect("the stack made read-only");
            return Carry::On {
                resume: Resume::Return(0),
                turn: Turn::Counted { limit: None },
            };
        }
        Carry::Leave(Leaving::Finished)
    }

    fn clock(&mut self, _: u64) -> u64 {
        0
    }

    fn warn(&mut self, _: Warning<'_>) {}
}

fn narrowed_during_a_call(look_ahead: LookAhead) {
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), look_ahead, Optimise::Speed, ());
    let memory = cpu.memory_new().unwrap();
    let code: Vec<u8> = PROGRAM.iter().flat_map(|w| w.to_le_bytes()).collect();
    cpu.write(memory, CODE, &code).unwrap();
    let rx = Permissions::READ | Permissions::EXECUTE;
    cpu.set_permissions(memory, CODE, PAGE_SIZE, rx).unwrap();
    cpu.code_changed(memory, CODE, PAGE_SIZE, &[CODE]);
    let rw = Permissions::READ | Permissions::WRITE;
    cpu.set_permissions(memory, STACK, PAGE_SIZE, rw).unwrap();
    let thread = cpu.thread_new(memory).unwrap();
    let mut registers = RegisterBlock::new_zeroed();
    registers.pc = CODE;
    registers.sp = TOP;
    registers.x[1] = VALUE;
    cpu.set_registers(thread, &registers);
    let mut kernel = Kernel {
        memory,
        exits: Vec::new(),
    };
    let Ok(()) = cpu.run(
        thread,
        Resume::FromRegisters,
        Turn::Counted { limit: None },
        &mut kernel,
    );
    let mut word = [0; 8];
    cpu.read(memory, STORED_AT, &mut word).unwrap();
    assert_eq!(
        u64::from_le_bytes(word),
        0,
        "the store reached a read-only page; exits {:?}",
        kernel.exits
    );
    match kernel.exits.as_slice() {
        [
            Exit::Syscall(_),
            Exit::MemoryFault {
                address: STORED_AT,
                access: Access::Write,
            },
        ] => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_page_made_read_only_in_a_call_is_read_only_after_it_full() {
    narrowed_during_a_call(LookAhead::Full);
}

#[test]
fn a_page_made_read_only_in_a_call_is_read_only_after_it_none() {
    narrowed_during_a_call(LookAhead::None);
}
