//! Guest code run through the executor's `Execution` as the kernel's loop runs it: loaded, linked, its exits resolved to
//! the guest's own state, its memory copied on write after a fork, its turns ended by a stop.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::sync::Arc;

use wcontainers_abi::{Cpu, Exit, MemoryId, Permissions, RegisterBlock, Resume, ThreadId, Warning};
use wcontainers_arm64::encode::{self, form};
use wcontainers_executor_host_process::executor::HostProcessExecutor;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_runtime_single_kernel::{Answer, Carry, Execution, Leaving, Turn};

static HOST: std::sync::OnceLock<Arc<HostProcessHost>> = std::sync::OnceLock::new();

fn executor() -> HostProcessExecutor {
    HostProcessExecutor::new(
        HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
            .clone(),
    )
}

const PAGE: u64 = 4096;
/// Where the guest code lies, and its data.
const CODE: u64 = 0x40_0000;
const DATA: u64 = 0x50_0000;
const INSTRUCTION: i64 = size_of::<u32>() as i64;
const WORD: u64 = size_of::<u64>() as u64;
/// Linux's numbers of two system calls the programs make, as the guest asks for them.
const GETPID: u16 = linux_raw_sys::general::__NR_getpid as u16;
const EXIT: u16 = linux_raw_sys::general::__NR_exit as u16;
/// The value the first system call returns.
const RETURNED: u64 = 42;
const SYSCALL_NUMBER: usize = 8;
const LINK: u8 = 30;

/// One memory with the guest code at [`CODE`] and a page of data at [`DATA`], and a thread at the code's start with x2
/// the data's address.
fn guest(cpu: &mut HostProcessExecutor, words: &[u32]) -> (MemoryId, ThreadId) {
    let memory = cpu.memory_new().expect("memory");
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    cpu.set_permissions(memory, CODE, PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code");
    cpu.set_permissions(memory, DATA, PAGE, Permissions::READ | Permissions::WRITE)
        .expect("data");
    cpu.write(memory, CODE, &bytes).expect("the code");
    cpu.code_changed(memory, CODE, PAGE, &[CODE]);
    let thread = cpu.thread_new(memory).expect("thread");
    let mut registers = RegisterBlock::ZERO;
    registers.pc = CODE;
    registers.x[2] = DATA;
    cpu.set_registers(thread, &registers);
    (memory, thread)
}

/// What the test's loop saw, and how it answers: each system call returns [`RETURNED`] until `exit`.
struct Seen {
    thread: ThreadId,
    exits: Vec<(Exit, RegisterBlock, u64)>,
    warnings: Vec<String>,
    /// The turn each carry asks for.
    turn: Option<Turn>,
}

impl Seen {
    fn new(thread: ThreadId) -> Self {
        Self {
            thread,
            exits: Vec::new(),
            warnings: Vec::new(),
            turn: None,
        }
    }
}

impl Answer for Seen {
    fn exit(&mut self, cpu: &mut dyn Cpu, exit: Exit, progress: u64) -> Carry {
        let mut registers = RegisterBlock::ZERO;
        self.exits.push((exit, registers, progress));
        let last = self.exits.len() - 1;
        cpu.registers(self.thread, &mut registers);
        self.exits[last].1 = registers;
        match exit {
            Exit::Syscall(call) if call.number.0 == u64::from(EXIT) => {
                Carry::Leave(Leaving::Finished)
            }
            Exit::Syscall(_) => Carry::On {
                resume: Resume::Return(RETURNED),
                turn: self.turn.unwrap_or(Turn::Counted { limit: None }),
            },
            Exit::Stop => Carry::Leave(Leaving::AnotherThread),
            _ => Carry::Leave(Leaving::Finished),
        }
    }

    fn clock(&mut self, _progress: u64) -> u64 {
        0
    }

    fn warn(&mut self, warning: Warning<'_>) {
        self.warnings.push(warning.reason.to_string());
    }
}

/// The system call `number` with x8 set first.
fn syscall(number: u16) -> [u32; 2] {
    [
        encode::movz(SYSCALL_NUMBER as u8, number, 0),
        encode::exception(form::SVC, 0),
    ]
}

/// A loop of 100 adds of 3, a call to a function that adds 1000, a store and a load of the sum, getpid, exit.
fn adding_program() -> Vec<u32> {
    let add = |rd, rn, value| encode::add_immediate(rd, rn, value).expect("an add");
    let mut words = vec![
        encode::movz(0, 0, 0),
        encode::movz(1, 100, 0),
        add(0, 0, 3),
        add(1, 1, -1),
        encode::compare_branch(true, 1, -2 * INSTRUCTION).expect("cbnz"),
    ];
    let call_at = words.len();
    words.push(0);
    words.push(encode::load_store(form::STR, 0, 2, 0, WORD).expect("str"));
    words.push(encode::load_store(form::LDR, 3, 2, 0, WORD).expect("ldr"));
    words.extend(syscall(GETPID));
    words.extend(syscall(EXIT));
    let function = words.len();
    words.push(add(0, 0, 1000));
    words.push(encode::branch_register(form::RET, LINK));
    words[call_at] = encode::branch(true, (function - call_at) as i64 * INSTRUCTION).expect("bl");
    words
}

/// The sum the adding program stores.
const SUM: u64 = 100 * 3 + 1000;

/// The turn every run takes: bounded, so a program that never exits stops.
const TURN: Turn = Turn::Counted {
    limit: Some(1_000_000),
};

/// The most stops a program that should exit soon takes.
const STOPS: usize = 200;

/// Runs `thread` until it leaves for anything but a stop, continuing after each stop; the exits seen, stops left out.
fn run_to_exit(cpu: &mut HostProcessExecutor, thread: ThreadId) -> Seen {
    let mut seen = Seen::new(thread);
    seen.turn = Some(TURN);
    let mut resume = Resume::FromRegisters;
    for _ in 0..STOPS {
        let Ok(()) = cpu.run(thread, resume, TURN, &mut seen);
        match seen.exits.last() {
            Some((Exit::Stop, _, _)) => {
                seen.exits.pop();
                resume = Resume::Continue;
            }
            _ => return seen,
        }
    }
    let mut registers = RegisterBlock::ZERO;
    cpu.registers(thread, &mut registers);
    panic!(
        "still running after {STOPS} stops: {registers:x?}, warnings {:?}",
        seen.warnings
    );
}

fn read_word(cpu: &mut HostProcessExecutor, memory: MemoryId, address: u64) -> u64 {
    let mut bytes = [0; WORD as usize];
    cpu.read(memory, address, &mut bytes).expect("read");
    u64::from_le_bytes(bytes)
}

#[test]
fn a_program_runs_to_its_system_calls_with_the_guest_registers_and_memory() {
    let mut cpu = executor();
    let (memory, thread) = guest(&mut cpu, &adding_program());
    let seen = run_to_exit(&mut cpu, thread);
    assert_eq!(seen.exits.len(), 2, "{:?}", seen.exits);
    let (exit, registers, progress) = &seen.exits[0];
    let Exit::Syscall(call) = exit else {
        panic!("not a system call: {exit:?}")
    };
    assert_eq!(call.number.0, u64::from(GETPID));
    assert_eq!(registers.x[0], SUM);
    assert_eq!(registers.x[3], SUM);
    assert_eq!(registers.x[2], DATA);
    assert_eq!(registers.x[1], 0);
    let getpid_at = adding_program().len() as u64 - 6;
    assert_eq!(
        registers.pc,
        CODE + (getpid_at + 2) * INSTRUCTION as u64,
        "after the svc"
    );
    assert!(*progress > 0, "progress counted");
    assert_eq!(read_word(&mut cpu, memory, DATA), SUM);
    let (exit, registers, _) = &seen.exits[1];
    assert!(matches!(exit, Exit::Syscall(call) if call.number.0 == u64::from(EXIT)));
    assert_eq!(registers.x[0], RETURNED, "the first call's return");
    assert_eq!(registers.x[SYSCALL_NUMBER], u64::from(EXIT));
    assert!(
        seen.warnings.iter().any(|w| w == "native code translated"),
        "{:?}",
        seen.warnings
    );
}

#[test]
fn a_fork_child_runs_its_own_copy_and_leaves_the_parent_s_memory() {
    let mut cpu = executor();
    let (parent, thread) = guest(&mut cpu, &adding_program());
    const BEFORE: u64 = 7;
    cpu.write(parent, DATA, &BEFORE.to_le_bytes())
        .expect("write");
    let child = cpu.memory_duplicate(parent).expect("fork");
    let child_thread = cpu.thread_new(child).expect("thread");
    let mut registers = RegisterBlock::ZERO;
    cpu.registers(thread, &mut registers);
    cpu.set_registers(child_thread, &registers);
    let seen = run_to_exit(&mut cpu, child_thread);
    assert!(
        matches!(seen.exits[0].0, Exit::Syscall(_)),
        "{:?}",
        seen.exits
    );
    assert_eq!(read_word(&mut cpu, child, DATA), SUM, "the child's store");
    assert_eq!(
        read_word(&mut cpu, parent, DATA),
        BEFORE,
        "the parent's page as at the fork"
    );
    // The parent runs its own code area, empty until now.
    run_to_exit(&mut cpu, thread);
    assert_eq!(read_word(&mut cpu, parent, DATA), SUM);
    assert_eq!(read_word(&mut cpu, child, DATA), SUM);
}

/// Calls a function through a register 50 times, each adding its index to x0, then exits; the function's address in
/// x9 and the count in x1.
fn calling_program() -> Vec<u32> {
    let add = |rd, rn, value| encode::add_immediate(rd, rn, value).expect("an add");
    let mut words = vec![
        encode::movz(0, 0, 0),
        encode::movz(1, 50, 0),
        encode::movz(9, 0, 0),
        encode::movz(9, (CODE >> u16::BITS) as u16, 1),
    ];
    let set_low = words.len();
    words.push(0);
    let call = words.len();
    words.push(encode::branch_register(form::BLR, 9));
    words.push(add(1, 1, -1));
    words.push(
        encode::compare_branch(true, 1, (call as i64 - words.len() as i64) * INSTRUCTION)
            .expect("cbnz"),
    );
    words.extend(syscall(EXIT));
    let function = words.len();
    words.push(add(0, 0, 1));
    words.push(encode::branch_register(form::RET, LINK));
    words[set_low] = encode::movk(9, (CODE + function as u64 * INSTRUCTION as u64) as u16, 0);
    words
}

#[test]
fn jumps_through_registers_find_their_slots_and_returns_their_last_targets() {
    let mut cpu = executor();
    let (_memory, thread) = guest(&mut cpu, &calling_program());
    let seen = run_to_exit(&mut cpu, thread);
    assert_eq!(seen.exits.len(), 1, "{:?}", seen.exits);
    let (exit, registers, _) = &seen.exits[0];
    assert!(
        matches!(exit, Exit::Syscall(call) if call.number.0 == u64::from(EXIT)),
        "{exit:?}"
    );
    assert_eq!(registers.x[0], 50);
    assert_eq!(registers.x[1], 0);
}

#[test]
fn a_counted_turn_of_a_loop_without_exits_ends_by_a_stop_and_goes_on() {
    let mut cpu = executor();
    // add x0, x0, #1; b back: forever.
    let words = [
        encode::add_immediate(0, 0, 1).expect("an add"),
        encode::branch(false, -INSTRUCTION).expect("b"),
    ];
    let (_memory, thread) = guest(&mut cpu, &words);
    let mut last = 0;
    for _ in 0..20 {
        let mut seen = Seen::new(thread);
        let Ok(()) = cpu.run(
            thread,
            Resume::Continue,
            Turn::Counted {
                limit: Some(100_000),
            },
            &mut seen,
        );
        assert_eq!(seen.exits.len(), 1);
        let (exit, registers, progress) = &seen.exits[0];
        assert_eq!(*exit, Exit::Stop);
        assert!(*progress > 0, "it ran");
        assert!(
            registers.x[0] > last,
            "x0 rose from {last} to {}",
            registers.x[0]
        );
        assert!(
            registers.pc == CODE || registers.pc == CODE + INSTRUCTION as u64,
            "at a guest instruction: {:#x}",
            registers.pc
        );
        last = registers.x[0];
    }
}
