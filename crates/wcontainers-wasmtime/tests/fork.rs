//! A duplicated memory holds what the source held, shows the same blocks of shared pages, runs the
//! same translated code, and goes its own way afterwards; a back end brought back from another holds
//! the same memories and threads and translates again.

use wcontainers_abi::{
    Cpu, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, Resume, SharedId, ThreadId, Warning,
};
use wcontainers_arm64::Arm64;
use wcontainers_runtime::{Answer, Carry, Execution, Leaving, Turn};
use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise, Origin};
use zerocopy::FromZeros;

const CODE: u64 = 0x10000;
/// `ldr x0, [x1]; svc #0`: gives the word at x1 to a system call.
const PROGRAM: [u32; 2] = [0xf940_0020, 0xd400_0001];
const HEAP: u64 = 0x40_0000;
const SHOWN: u64 = 0x80_0000;
const HIDDEN: u64 = 0xc0_0000;
const RW: Permissions = Permissions::READ.union(Permissions::WRITE);

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

/// Runs a new thread of `memory` reading the word at `address`; what it read.
fn load(cpu: &mut CpuSide<Arm64>, memory: MemoryId, address: u64) -> u64 {
    let thread = cpu.thread_new(memory).unwrap();
    let value = run(cpu, thread, address);
    cpu.thread_drop(thread);
    value
}

fn run(cpu: &mut CpuSide<Arm64>, thread: ThreadId, address: u64) -> u64 {
    let mut registers = RegisterBlock::new_zeroed();
    registers.pc = CODE;
    registers.x[1] = address;
    cpu.set_registers(thread, &registers);
    let mut recorder = Recorder::default();
    let turn = Turn::Counted { limit: None };
    let Ok(()) = cpu.run(thread, Resume::FromRegisters, turn, &mut recorder);
    let [Exit::Syscall(call)] = recorder.exits.as_slice() else {
        panic!("{:?}", recorder.exits);
    };
    call.arguments[0]
}

fn word(cpu: &mut CpuSide<Arm64>, memory: MemoryId, address: u64) -> u64 {
    let mut bytes = [0u8; 8];
    cpu.read(memory, address, &mut bytes).unwrap();
    u64::from_le_bytes(bytes)
}

/// A parent with code it has run, a heap, a page written and then made inaccessible, and a block of
/// shared pages shown.
fn parent(cpu: &mut CpuSide<Arm64>) -> (MemoryId, SharedId) {
    let memory = cpu.memory_new().unwrap();
    let code: Vec<u8> = PROGRAM.iter().flat_map(|w| w.to_le_bytes()).collect();
    cpu.write(memory, CODE, &code).unwrap();
    let rx = Permissions::READ | Permissions::EXECUTE;
    cpu.set_permissions(memory, CODE, PAGE_SIZE, rx).unwrap();
    cpu.code_changed(memory, CODE, PAGE_SIZE, &[CODE]);
    cpu.set_permissions(memory, HEAP, 64 * PAGE_SIZE, RW)
        .unwrap();
    for page in 0..64 {
        cpu.write(memory, HEAP + page * PAGE_SIZE, &(page + 1).to_le_bytes())
            .unwrap();
    }
    cpu.set_permissions(memory, HIDDEN, PAGE_SIZE, RW).unwrap();
    cpu.write(memory, HIDDEN, &77u64.to_le_bytes()).unwrap();
    cpu.set_permissions(memory, HIDDEN, PAGE_SIZE, Permissions::empty())
        .unwrap();
    let block = cpu.shared_new(4 * PAGE_SIZE).unwrap();
    cpu.shared_write(block, PAGE_SIZE, &5u64.to_le_bytes())
        .unwrap();
    cpu.shared_map(block, 0, memory, SHOWN, 4 * PAGE_SIZE)
        .unwrap();
    cpu.set_permissions(memory, SHOWN, 4 * PAGE_SIZE, RW)
        .unwrap();
    assert_eq!(load(cpu, memory, HEAP + 3 * PAGE_SIZE), 4);
    (memory, block)
}

fn duplicate(look_ahead: LookAhead) {
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), look_ahead, Optimise::Speed, ());
    let (memory, _) = parent(&mut cpu);
    let child = cpu.memory_duplicate(memory).unwrap();

    for page in 0..64 {
        assert_eq!(word(&mut cpu, child, HEAP + page * PAGE_SIZE), page + 1);
    }
    assert_eq!(word(&mut cpu, child, HIDDEN), 77);
    assert_eq!(word(&mut cpu, child, SHOWN + PAGE_SIZE), 5);
    // The child runs the parent's translation, and reads through its own permissions.
    let made = cpu.translated().len();
    assert_eq!(load(&mut cpu, child, HEAP + 9 * PAGE_SIZE), 10);
    assert_eq!(cpu.translated().len(), made, "the child translated again");

    // Each goes its own way.
    cpu.write(memory, HEAP, &100u64.to_le_bytes()).unwrap();
    cpu.write(child, HEAP + PAGE_SIZE, &200u64.to_le_bytes())
        .unwrap();
    assert_eq!(word(&mut cpu, child, HEAP), 1);
    assert_eq!(word(&mut cpu, memory, HEAP + PAGE_SIZE), 2);
    assert_eq!(load(&mut cpu, child, HEAP + PAGE_SIZE), 200);
    assert_eq!(load(&mut cpu, memory, HEAP), 100);

    // The block stays shared by both.
    cpu.write(child, SHOWN + PAGE_SIZE, &6u64.to_le_bytes())
        .unwrap();
    assert_eq!(word(&mut cpu, memory, SHOWN + PAGE_SIZE), 6);
    assert_eq!(load(&mut cpu, memory, SHOWN + PAGE_SIZE), 6);
    cpu.memory_drop(child);
    assert_eq!(load(&mut cpu, memory, SHOWN + PAGE_SIZE), 6);
}

#[test]
fn a_duplicate_is_exact_and_its_own_full() {
    duplicate(LookAhead::Full);
}

#[test]
fn a_duplicate_is_exact_and_its_own_none() {
    duplicate(LookAhead::None);
}

#[test]
fn a_back_end_brought_back_holds_the_same_and_translates_again() {
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), LookAhead::Full, Optimise::Speed, ());
    let (memory, block) = parent(&mut cpu);
    let thread = cpu.thread_new(memory).unwrap();
    let mut registers = RegisterBlock::new_zeroed();
    registers.x[5] = 55;
    cpu.set_registers(thread, &registers);

    let mut back = cpu.restored();
    drop(cpu);
    let mut kept = RegisterBlock::new_zeroed();
    back.registers(thread, &mut kept);
    assert_eq!(kept.x[5], 55);
    assert_eq!(word(&mut back, memory, HIDDEN), 77);
    let made = back.translated().len();
    assert_eq!(run(&mut back, thread, HEAP + 7 * PAGE_SIZE), 8);
    let again = &back.translated()[made..];
    assert!(
        again
            .iter()
            .any(|t| matches!(t.origin, Origin::Made { .. })),
        "nothing translated again: {again:?}"
    );
    // A block shown before is still shared after.
    back.shared_write(block, PAGE_SIZE, &9u64.to_le_bytes())
        .unwrap();
    assert_eq!(load(&mut back, memory, SHOWN + PAGE_SIZE), 9);
}

/// In a copy of this process made by fork, which holds only the forking thread, a back end brought
/// back translates and compiles new code in its own pool and runs it.
#[test]
fn a_back_end_brought_back_in_a_copy_of_the_process_compiles_there() {
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), LookAhead::Full, Optimise::Speed, ());
    let (memory, _) = parent(&mut cpu);
    let thread = cpu.thread_new(memory).unwrap();
    // Rayon's global pool has threads here, which the copy will not have.
    rayon::join(|| {}, || {});
    // SAFETY: the copy only runs the back end and leaves with `_exit`.
    let child = unsafe { libc::fork() };
    assert!(child >= 0, "fork");
    if child == 0 {
        let mut back = cpu.restored();
        let value = run(&mut back, thread, HEAP + 11 * PAGE_SIZE);
        let compiled = back
            .translated()
            .iter()
            .any(|t| matches!(t.origin, Origin::Made { .. }));
        // SAFETY: ends the copy without running anything of the parent's.
        unsafe { libc::_exit(if value == 12 && compiled { 0 } else { 1 }) };
    }
    wait(child);
}

/// Waits up to 120 s for a copy of this process to exit with 0.
fn wait(child: libc::pid_t) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut status = 0;
    loop {
        // SAFETY: waits for the caller's copy.
        let done = unsafe { libc::waitpid(child, &mut status, libc::WNOHANG) };
        if done == child {
            break;
        }
        if std::time::Instant::now() > deadline {
            // SAFETY: the caller's copy.
            unsafe { libc::kill(child, libc::SIGKILL) };
            panic!("the copy did not finish in 120 s");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0,
        "status {status:#x}"
    );
}

/// After the host process has forked, a duplicate made in the copy of memory written only there is
/// exact.
#[test]
fn a_duplicate_made_after_the_host_process_forked_is_exact() {
    const PAGES: u64 = 256;
    let mut cpu = CpuSide::<Arm64>::new(Compiler::new(None), LookAhead::Full, Optimise::Speed, ());
    let (memory, _) = parent(&mut cpu);
    cpu.memory_duplicate(memory).unwrap();
    // SAFETY: the copy only uses the back end and leaves with `_exit`.
    let child = unsafe { libc::fork() };
    assert!(child >= 0, "fork");
    if child == 0 {
        let memory = cpu.memory_new().unwrap();
        cpu.set_permissions(memory, HIDDEN, PAGES * PAGE_SIZE, RW)
            .unwrap();
        for page in 0..PAGES {
            let at = HIDDEN + page * PAGE_SIZE;
            cpu.write(memory, at, &(page + 7).to_le_bytes()).unwrap();
        }
        let copy = cpu.memory_duplicate(memory).unwrap();
        let wrong = (0..PAGES)
            .filter(|page| word(&mut cpu, copy, HIDDEN + page * PAGE_SIZE) != page + 7)
            .count();
        // SAFETY: ends the copy without running anything of the parent's.
        unsafe { libc::_exit(i32::from(wrong != 0)) };
    }
    wait(child);
}
