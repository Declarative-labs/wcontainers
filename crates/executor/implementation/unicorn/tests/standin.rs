//! The stand-in CPU comes back for each reason the abi names, with exact registers.

use wcontainers_abi::{
    Access, Cpu, Exit, LOWEST_ADDRESS, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, Resume,
    SharedId, SystemCallNumber, SystemCallRequest,
};
use wcontainers_executor_unicorn::standin::{Refused, StandIn, Unsupported};

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
    cpu.write(memory, LOWEST_ADDRESS, &code).unwrap();
    cpu.set_permissions(
        memory,
        LOWEST_ADDRESS,
        PAGE_SIZE,
        Permissions::READ | Permissions::EXECUTE,
    )
    .unwrap();
    let thread = cpu.thread_new(memory).unwrap();
    let mut start = RegisterBlock::ZERO;
    start.pc = LOWEST_ADDRESS;
    start.x[8] = 64;
    start.x[0] = 1;
    start.x[1] = 0x10;
    start.x[5] = 6;
    start.v[3] = [0x1122_3344_5566_7788, 0x99aa_bbcc_ddee_ff00];
    start.tpidr_el0 = 0x1234_5000;
    start.pstate = wcontainers_abi::pstate::Z | wcontainers_abi::pstate::C;
    cpu.set_registers(thread, &start);

    let exit = cpu.run(thread, Resume::FromRegisters, None).unwrap();
    assert_eq!(
        exit,
        Exit::Syscall(SystemCallRequest {
            number: SystemCallNumber(64),
            arguments: [1, 0x10, 0, 0, 0, 6]
        })
    );
    let after = registers(&mut cpu, thread);
    assert_eq!(after.pc, LOWEST_ADDRESS + 4);
    assert_eq!(after.v[3], start.v[3]);
    assert_eq!(after.tpidr_el0, start.tpidr_el0);
    assert_eq!(after.pstate, start.pstate);

    let exit = cpu.run(thread, Resume::Return(7), None).unwrap();
    assert_eq!(exit, Exit::Breakpoint { comment: 5 });
    let after = registers(&mut cpu, thread);
    assert_eq!(after.pc, LOWEST_ADDRESS + 4);
    assert_eq!(after.x[0], 7);

    let mut next = after;
    next.pc = LOWEST_ADDRESS + 8;
    cpu.set_registers(thread, &next);
    let exit = cpu.run(thread, Resume::FromRegisters, None).unwrap();
    assert_eq!(
        exit,
        Exit::MemoryFault {
            address: 0x10,
            access: Access::Read
        }
    );
    assert_eq!(registers(&mut cpu, thread).pc, LOWEST_ADDRESS + 8);

    next.pc = LOWEST_ADDRESS + 12;
    cpu.set_registers(thread, &next);
    assert_eq!(
        cpu.run(thread, Resume::FromRegisters, None).unwrap(),
        Exit::IllegalInstruction
    );
    assert_eq!(registers(&mut cpu, thread).pc, LOWEST_ADDRESS + 12);

    next.pc = LOWEST_ADDRESS + 16;
    cpu.set_registers(thread, &next);
    assert_eq!(
        cpu.run(thread, Resume::FromRegisters, Some(1_000_000))
            .unwrap(),
        Exit::Stop
    );
    assert_eq!(registers(&mut cpu, thread).pc, LOWEST_ADDRESS + 16);
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
    cpu.write(memory, LOWEST_ADDRESS, &code).unwrap();
    cpu.set_permissions(
        memory,
        LOWEST_ADDRESS,
        PAGE_SIZE,
        Permissions::READ | Permissions::EXECUTE,
    )
    .unwrap();
    let data = LOWEST_ADDRESS + 0x10000;
    cpu.set_permissions(memory, data, 3 * PAGE_SIZE, Permissions::READ)
        .unwrap();
    let thread = cpu.thread_new(memory).unwrap();
    let mut start = RegisterBlock::ZERO;
    start.pc = LOWEST_ADDRESS;
    start.x[0] = 0xfeed;
    start.x[1] = data + PAGE_SIZE;
    cpu.set_registers(thread, &start);
    assert_eq!(
        cpu.run(thread, Resume::FromRegisters, None).unwrap(),
        Exit::MemoryFault {
            address: data + PAGE_SIZE,
            access: Access::Write
        }
    );
    // Only the middle page becomes writable; the split must keep the pages around it as they were.
    cpu.set_permissions(
        memory,
        data + PAGE_SIZE,
        PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    )
    .unwrap();
    assert!(matches!(
        cpu.run(thread, Resume::FromRegisters, None).unwrap(),
        Exit::Syscall(_)
    ));
    let mut stored = [0u8; 8];
    cpu.read(memory, data + PAGE_SIZE, &mut stored).unwrap();
    assert_eq!(u64::from_le_bytes(stored), 0xfeed);

    start.x[1] = data + 2 * PAGE_SIZE;
    cpu.set_registers(thread, &start);
    assert!(matches!(
        cpu.run(thread, Resume::FromRegisters, None).unwrap(),
        Exit::MemoryFault {
            access: Access::Write,
            ..
        }
    ));
}

/// An exclusive load from an unaligned address runs on the reference's hardware but faults in the
/// emulator; the stand-in says so instead of giving an exit.
#[test]
fn unaligned_exclusive_is_reported_as_unsupported() {
    const LDXR_X0_X1: u32 = 0xc85f_7c20;
    let mut cpu = StandIn::new();
    let memory = cpu.memory_new().unwrap();
    cpu.write(memory, LOWEST_ADDRESS, &LDXR_X0_X1.to_le_bytes())
        .unwrap();
    cpu.set_permissions(
        memory,
        LOWEST_ADDRESS,
        PAGE_SIZE,
        Permissions::READ | Permissions::EXECUTE,
    )
    .unwrap();
    cpu.set_permissions(
        memory,
        0x20000,
        PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    )
    .unwrap();
    let thread = cpu.thread_new(memory).unwrap();
    let mut start = RegisterBlock::ZERO;
    start.pc = LOWEST_ADDRESS;
    start.x[1] = 0x20001;
    cpu.set_registers(thread, &start);
    assert_eq!(
        cpu.run(thread, Resume::FromRegisters, None),
        Err(Unsupported::AlignmentFault { pc: LOWEST_ADDRESS })
    );
}

/// A block a memory still shows is not freed under it: the drop is refused by name and the block kept.
/// Before, the block's host memory was unmapped here and the read below reached it, which ended the
/// test process with SIGSEGV.
#[test]
fn a_block_a_memory_shows_is_not_dropped() {
    let mut cpu = StandIn::new();
    let memory = cpu.memory_new().unwrap();
    let block = cpu.shared_new(PAGE_SIZE).unwrap();
    cpu.shared_write(block, 0, &[7]).unwrap();
    cpu.shared_map(block, 0, memory, 0x20000, PAGE_SIZE).unwrap();
    cpu.set_permissions(memory, 0x20000, PAGE_SIZE, Permissions::READ)
        .unwrap();
    cpu.shared_drop(block);
    assert_eq!(
        cpu.refused(),
        [Refused::DroppingAShownBlock {
            block,
            shown_in: vec![memory]
        }]
    );
    let mut byte = [0];
    cpu.read(memory, 0x20000, &mut byte).unwrap();
    assert_eq!(byte, [7], "the read reached the kept block");
    assert!(cpu.shared_read(block, 0, &mut byte).is_err(), "the kernel's handle is gone");
    // A copy, as bringing back a saved container makes, keeps the block under its memory too.
    let mut copy = cpu.restored();
    copy.read(memory, 0x20000, &mut byte).unwrap();
    assert_eq!(byte, [7]);
    // Once its memory goes, so does the kept block.
    cpu.memory_drop(memory);
    assert_eq!(cpu.refused().len(), 1);
}

/// The drop's check counts only ranges a memory shows now: each way of ending the sharing (unmapping
/// it, zeroing over it, mapping another block over it, dropping the memory) lets the block go with
/// nothing refused.
#[test]
fn a_block_whose_sharing_ended_is_dropped_without_a_refusal() {
    let mut cpu = StandIn::new();
    let other = cpu.shared_new(PAGE_SIZE).unwrap();
    type End = fn(&mut StandIn, MemoryId, SharedId);
    let ends: [End; 4] = [
        |cpu, memory, _| cpu.shared_unmap(memory, 0x20000, PAGE_SIZE).unwrap(),
        |cpu, memory, _| cpu.zero(memory, 0x20000, PAGE_SIZE).unwrap(),
        |cpu, memory, other| cpu.shared_map(other, 0, memory, 0x20000, PAGE_SIZE).unwrap(),
        |cpu, memory, _| cpu.memory_drop(memory),
    ];
    for end in ends {
        let memory = cpu.memory_new().unwrap();
        let block = cpu.shared_new(2 * PAGE_SIZE).unwrap();
        cpu.shared_map(block, PAGE_SIZE, memory, 0x20000, PAGE_SIZE)
            .unwrap();
        end(&mut cpu, memory, other);
        cpu.shared_drop(block);
        assert_eq!(cpu.refused(), [], "the block's sharing had ended");
    }
}

/// Once no memory shows it, a block goes, and a memory that showed it keeps a copy of its bytes.
#[test]
fn a_block_no_memory_shows_is_dropped() {
    let mut cpu = StandIn::new();
    let memory = cpu.memory_new().unwrap();
    let block = cpu.shared_new(2 * PAGE_SIZE).unwrap();
    cpu.shared_write(block, PAGE_SIZE, &[9]).unwrap();
    cpu.shared_map(block, PAGE_SIZE, memory, 0x20000, PAGE_SIZE)
        .unwrap();
    cpu.shared_unmap(memory, 0x20000, PAGE_SIZE).unwrap();
    cpu.shared_drop(block);
    let mut byte = [0];
    cpu.read(memory, 0x20000, &mut byte).unwrap();
    assert_eq!(byte, [9]);
    assert!(cpu.shared_read(block, 0, &mut byte).is_err(), "the block is gone");
}
