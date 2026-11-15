//! The stand-in CPU comes back for each reason the abi names, with exact registers.

use wcontainers_abi::{Access, Cpu, Exit, Permissions, RegisterBlock, Resume, Syscall};
use wcontainers_kernel_test::standin::StandIn;

const CODE: u64 = 0x10000;

// Instruction words.
const SVC_0: u32 = 0xd400_0001;
const BRK_5: u32 = 0xd420_00a0;
const LDR_X0_X1: u32 = 0xf940_0020;
const UDF_0: u32 = 0x0000_0000;
const B_SELF: u32 = 0x1400_0000;

fn registers(cpu: &mut StandIn, thread: wcontainers_abi::ThreadId) -> RegisterBlock {
    let mut registers = RegisterBlock::ZERO;
    cpu.registers(thread, &mut registers);
    registers
}

#[test]
fn exits_are_exact() {
    let mut cpu = StandIn::new();
    let memory = cpu.memory_new().unwrap();
    let code: Vec<u8> = [SVC_0, BRK_5, LDR_X0_X1, UDF_0, B_SELF]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    cpu.write(memory, CODE, &code).unwrap();
    cpu.set_permissions(memory, CODE, 4096, Permissions::READ | Permissions::EXECUTE)
        .unwrap();
    let thread = cpu.thread_new(memory).unwrap();
    let mut start = RegisterBlock::ZERO;
    start.pc = CODE;
    start.x[8] = 64;
    start.x[0] = 1;
    start.x[1] = 0x10;
    start.x[5] = 6;
    start.v[3] = [0x1122_3344_5566_7788, 0x99aa_bbcc_ddee_ff00];
    start.tpidr_el0 = 0x1234_5000;
    start.pstate = wcontainers_abi::pstate::Z | wcontainers_abi::pstate::C;
    cpu.set_registers(thread, &start);

    let exit = cpu.run(thread, Resume::FromRegisters, None);
    assert_eq!(
        exit,
        Exit::Syscall(Syscall {
            number: 64,
            arguments: [1, 0x10, 0, 0, 0, 6]
        })
    );
    let after = registers(&mut cpu, thread);
    assert_eq!(after.pc, CODE + 4);
    assert_eq!(after.v[3], start.v[3]);
    assert_eq!(after.tpidr_el0, start.tpidr_el0);
    assert_eq!(after.pstate, start.pstate);

    let exit = cpu.run(thread, Resume::Return(7), None);
    assert_eq!(exit, Exit::Breakpoint { comment: 5 });
    let after = registers(&mut cpu, thread);
    assert_eq!(after.pc, CODE + 4);
    assert_eq!(after.x[0], 7);

    let mut next = after;
    next.pc = CODE + 8;
    cpu.set_registers(thread, &next);
    let exit = cpu.run(thread, Resume::FromRegisters, None);
    assert_eq!(
        exit,
        Exit::MemoryFault {
            address: 0x10,
            access: Access::Read
        }
    );
    assert_eq!(registers(&mut cpu, thread).pc, CODE + 8);

    next.pc = CODE + 12;
    cpu.set_registers(thread, &next);
    assert_eq!(
        cpu.run(thread, Resume::FromRegisters, None),
        Exit::IllegalInstruction
    );
    assert_eq!(registers(&mut cpu, thread).pc, CODE + 12);

    next.pc = CODE + 16;
    cpu.set_registers(thread, &next);
    assert_eq!(
        cpu.run(thread, Resume::FromRegisters, Some(1_000_000)),
        Exit::Stop
    );
    assert_eq!(registers(&mut cpu, thread).pc, CODE + 16);
}

#[test]
fn permissions_change_while_running() {
    let mut cpu = StandIn::new();
    let memory = cpu.memory_new().unwrap();
    // str x0, [x1]; svc 0
    let code: Vec<u8> = [0xf900_0020u32, SVC_0]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    cpu.write(memory, CODE, &code).unwrap();
    cpu.set_permissions(memory, CODE, 4096, Permissions::READ | Permissions::EXECUTE)
        .unwrap();
    let data = CODE + 0x10000;
    cpu.set_permissions(memory, data, 3 * 4096, Permissions::READ)
        .unwrap();
    let thread = cpu.thread_new(memory).unwrap();
    let mut start = RegisterBlock::ZERO;
    start.pc = CODE;
    start.x[0] = 0xfeed;
    start.x[1] = data + 4096;
    cpu.set_registers(thread, &start);
    assert_eq!(
        cpu.run(thread, Resume::FromRegisters, None),
        Exit::MemoryFault {
            address: data + 4096,
            access: Access::Write
        }
    );
    // Only the middle page becomes writable; the split must keep the pages around it as they were.
    cpu.set_permissions(
        memory,
        data + 4096,
        4096,
        Permissions::READ | Permissions::WRITE,
    )
    .unwrap();
    assert!(matches!(
        cpu.run(thread, Resume::FromRegisters, None),
        Exit::Syscall(_)
    ));
    let mut stored = [0u8; 8];
    cpu.read(memory, data + 4096, &mut stored).unwrap();
    assert_eq!(u64::from_le_bytes(stored), 0xfeed);

    start.x[1] = data + 2 * 4096;
    cpu.set_registers(thread, &start);
    assert!(matches!(
        cpu.run(thread, Resume::FromRegisters, None),
        Exit::MemoryFault {
            access: Access::Write,
            ..
        }
    ));
}
