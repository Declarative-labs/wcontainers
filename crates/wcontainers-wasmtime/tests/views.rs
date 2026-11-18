//! One block of shared pages shown at two places of one memory: what the guest writes through one is
//! what it reads through the other, with no exit between.

use wcontainers_abi::{Cpu, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, Resume};
use wcontainers_abi::{SharedId, Warning};
use wcontainers_arm64::Arm64;
use wcontainers_runtime::{Answer, Carry, Execution, Leaving, Turn};
use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise, reason};
use zerocopy::FromZeros;

const CODE: u64 = 0x10000;
const VIEWS: u64 = 0x40_0000;

/// The system calls the guest made, as their argument registers.
struct Kernel {
    calls: Vec<[u64; 6]>,
}

impl Answer for Kernel {
    fn exit(&mut self, _: &mut dyn Cpu, exit: Exit, _: u64) -> Carry {
        match exit {
            Exit::Syscall(call) => self.calls.push(call.arguments),
            other => panic!("{other:?}"),
        }
        Carry::Leave(Leaving::Finished)
    }

    fn clock(&mut self, _: u64) -> u64 {
        0
    }

    fn warn(&mut self, _: Warning<'_>) {}
}

fn side(look_ahead: LookAhead) -> (CpuSide<Arm64>, MemoryId) {
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), look_ahead, Optimise::Speed, ());
    let memory = cpu.memory_new().unwrap();
    (cpu, memory)
}

/// Runs `program` from its start with the registers given and returns the call it ends with.
fn run(
    cpu: &mut CpuSide<Arm64>,
    memory: MemoryId,
    program: &[u32],
    set: &[(usize, u64)],
) -> [u64; 6] {
    let code: Vec<u8> = program.iter().flat_map(|w| w.to_le_bytes()).collect();
    cpu.write(memory, CODE, &code).unwrap();
    let rx = Permissions::READ | Permissions::EXECUTE;
    cpu.set_permissions(memory, CODE, PAGE_SIZE, rx).unwrap();
    cpu.code_changed(memory, CODE, PAGE_SIZE, &[CODE]);
    let thread = cpu.thread_new(memory).unwrap();
    let mut registers = RegisterBlock::new_zeroed();
    registers.pc = CODE;
    for &(at, value) in set {
        registers.x[at] = value;
    }
    cpu.set_registers(thread, &registers);
    let mut kernel = Kernel { calls: Vec::new() };
    let Ok(()) = cpu.run(
        thread,
        Resume::FromRegisters,
        Turn { limit: None },
        &mut kernel,
    );
    kernel.calls[0]
}

fn show(
    cpu: &mut CpuSide<Arm64>,
    memory: MemoryId,
    block: SharedId,
    offset: u64,
    at: u64,
    pages: u64,
) {
    let length = pages * PAGE_SIZE;
    let rw = Permissions::READ | Permissions::WRITE;
    cpu.set_permissions(memory, at, length, rw).unwrap();
    cpu.shared_map(block, offset, memory, at, length).unwrap();
}

fn read_through_the_other(look_ahead: LookAhead) {
    let (mut cpu, memory) = side(look_ahead);
    let block = cpu.shared_new(2 * PAGE_SIZE).unwrap();
    let (a, b) = (VIEWS, VIEWS + 0x10_0000);
    show(&mut cpu, memory, block, 0, a, 2);
    show(&mut cpu, memory, block, 0, b, 2);
    // movz w3, #0x11; strb w3, [x1, #10]; ldrb w0, [x2, #10];
    // movz w3, #0x22; strb w3, [x2, #20]; ldrb w4, [x1, #20]; svc #0
    let program = [
        0x5280_0223,
        0x3900_2823,
        0x3940_2840,
        0x5280_0443,
        0x3900_5043,
        0x3940_5024,
        0xd400_0001,
    ];
    let call = run(&mut cpu, memory, &program, &[(1, a), (2, b)]);
    assert_eq!((call[0], call[4]), (0x11, 0x22));
    let mut held = [0u8; 21];
    cpu.shared_read(block, 0, &mut held).unwrap();
    assert_eq!((held[10], held[20]), (0x11, 0x22));
    let counters = cpu.counters();
    let count = |name: &str| {
        counters
            .iter()
            .find(|c| c.reason == name)
            .map_or(0, |c| c.count)
    };
    assert!(count(reason::SHARED_PAGE_WRITTEN_ELSEWHERE) >= 2);
}

#[test]
fn a_write_through_one_view_is_read_through_the_other_full() {
    read_through_the_other(LookAhead::Full);
}

#[test]
fn a_write_through_one_view_is_read_through_the_other_none() {
    read_through_the_other(LookAhead::None);
}

/// A ring of `pages` pages shown twice side by side: a store across the seam is read from the first
/// view, and the store and the read again from the second.
fn ring(look_ahead: LookAhead, pages: u64) {
    let (mut cpu, memory) = side(look_ahead);
    let size = pages * PAGE_SIZE;
    let block = cpu.shared_new(size).unwrap();
    show(&mut cpu, memory, block, 0, VIEWS, pages);
    show(&mut cpu, memory, block, 0, VIEWS + size, pages);
    // stp x3, x4, [x1]; ldr x0, [x2]; ldr x5, [x1]; svc #0
    let program = [0xa900_1023, 0xf940_0040, 0xf940_0025, 0xd400_0001];
    let (low, high) = (0x0102_0304_0506_0708, 0x1112_1314_1516_1718);
    let set = [(1, VIEWS + size - 8), (2, VIEWS), (3, low), (4, high)];
    let call = run(&mut cpu, memory, &program, &set);
    assert_eq!((call[0], call[5]), (high, low));
    let mut held = [0u8; 8];
    cpu.shared_read(block, 0, &mut held).unwrap();
    assert_eq!(u64::from_le_bytes(held), high);
    cpu.shared_read(block, size - 8, &mut held).unwrap();
    assert_eq!(u64::from_le_bytes(held), low);
    let counters = cpu.counters();
    let stepped = counters
        .iter()
        .find(|c| c.reason == reason::WRITE_STEPPED)
        .map_or(0, |c| c.count);
    assert_eq!(stepped > 0, pages == 1);
}

#[test]
fn a_store_across_a_rings_seam_is_seen_through_both_views_full() {
    ring(LookAhead::Full, 2);
}

#[test]
fn a_store_across_a_rings_seam_is_seen_through_both_views_none() {
    ring(LookAhead::None, 2);
}

#[test]
fn a_store_across_a_one_page_rings_seam_is_made_one_instruction_at_a_time_full() {
    ring(LookAhead::Full, 1);
}

#[test]
fn a_store_across_a_one_page_rings_seam_is_made_one_instruction_at_a_time_none() {
    ring(LookAhead::None, 1);
}
