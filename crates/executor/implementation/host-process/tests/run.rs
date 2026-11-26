//! Guest code run through the executor as the kernel's loop ran it, a turn at a time: loaded, linked, its exits resolved
//! to the guest's own state, its memory copied on write after a fork, its turns ended by a stop.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::sync::Arc;
use std::time::Duration;

use wcontainers_abi::{
    Access, CpuError, Exit, MemoryId, Permissions, RegisterBlock, Resume, ThreadId,
};
use wcontainers_arm64::encode::{self, form};
use wcontainers_arm64::{Address, Instruction, Reg};
use wcontainers_executor_host_process::executor::HostProcessExecutor;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_executor_host_process::layout::THREADS_PER_PROCESS;
use wcontainers_executor_host_process::stub::StopWhen;

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
    /// Leave at every system call, not only at exit.
    leave_at_calls: bool,
}

impl Seen {
    fn new(thread: ThreadId) -> Self {
        Self {
            thread,
            exits: Vec::new(),
            warnings: Vec::new(),
            turn: None,
            leave_at_calls: false,
        }
    }
}

/// How long a turn runs: to the thread's next exit, or to a stop at the first exit at or after `limit` more guest
/// instructions, a unit of progress taken as a nanosecond of time slice, since linked pieces cannot be stopped at an
/// exact count.
#[derive(Clone, Copy)]
enum Turn {
    Counted { limit: Option<u64> },
}

/// What the loop does after an exit.
enum Carry {
    On { resume: Resume, turn: Turn },
    Leave,
}

impl Seen {
    /// Records the exit with the registers after it, and answers it: each system call returns [`RETURNED`] until
    /// `exit`, which leaves, as a stop and every other exit do.
    fn exit(&mut self, cpu: &mut HostProcessExecutor, exit: Exit, progress: u64) -> Carry {
        let mut registers = RegisterBlock::ZERO;
        cpu.registers(self.thread, &mut registers);
        self.exits.push((exit, registers, progress));
        match exit {
            Exit::Syscall(call) if call.number.0 == u64::from(EXIT) || self.leave_at_calls => {
                Carry::Leave
            }
            Exit::Syscall(_) => Carry::On {
                resume: Resume::Return(RETURNED),
                turn: self.turn.unwrap_or(Turn::Counted { limit: None }),
            },
            _ => Carry::Leave,
        }
    }
}

/// Runs `thread` from `resume` for `turn`, then from each answer `seen` gives to its exits, until `seen` leaves; the
/// slow paths taken so far are its warnings.
fn run(
    cpu: &mut HostProcessExecutor,
    thread: ThreadId,
    mut resume: Resume,
    mut turn: Turn,
    seen: &mut Seen,
) {
    loop {
        let before = cpu.guest_count(thread);
        let Turn::Counted { limit } = turn;
        let when = StopWhen {
            slice: limit.map(Duration::from_nanos),
            wanted: None,
        };
        let until = limit.map(|limit| before + limit);
        let exit = cpu.run_translated(thread, resume, until, when, &mut || 0);
        seen.warnings = cpu
            .slow_paths()
            .into_iter()
            .map(|(reason, _)| reason.to_string())
            .collect();
        let progress = cpu.guest_count(thread) - before;
        match seen.exit(cpu, exit, progress) {
            Carry::On {
                resume: next,
                turn: next_turn,
            } => {
                resume = next;
                turn = next_turn;
            }
            Carry::Leave => return,
        }
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

/// Runs `thread` from `resume` into `seen` until it leaves for anything but a stop (a counted turn is also a time
/// slice, which a loaded host can end before the code runs), continuing after each stop, which `seen` then leaves out.
fn run_through_stops(
    cpu: &mut HostProcessExecutor,
    thread: ThreadId,
    mut resume: Resume,
    seen: &mut Seen,
) {
    for _ in 0..STOPS {
        run(cpu, thread, resume, TURN, seen);
        if !matches!(seen.exits.last(), Some((Exit::Stop, _, _))) {
            return;
        }
        seen.exits.pop();
        resume = Resume::Continue;
    }
    panic!("still running after {STOPS} stops");
}

/// Runs `thread` until it leaves for anything but a stop, continuing after each stop; the exits seen, stops left out.
fn run_to_exit(cpu: &mut HostProcessExecutor, thread: ThreadId) -> Seen {
    let mut seen = Seen::new(thread);
    seen.turn = Some(TURN);
    let mut resume = Resume::FromRegisters;
    for _ in 0..STOPS {
        run(cpu, thread, resume, TURN, &mut seen);
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

/// Runs a thread of `memory` from the adding program's start with x2 at its data; the sum it stored.
fn run_adding(cpu: &mut HostProcessExecutor, memory: MemoryId, thread: ThreadId) -> u64 {
    let mut registers = RegisterBlock::ZERO;
    registers.pc = CODE;
    registers.x[2] = DATA;
    cpu.set_registers(thread, &registers);
    let seen = run_to_exit(cpu, thread);
    assert!(
        matches!(seen.exits[..], [.., (Exit::Syscall(call), _, _)] if call.number.0 == u64::from(EXIT)),
        "{:?}",
        seen.exits
    );
    read_word(cpu, memory, DATA)
}

/// The adding program with its loop adding `step` instead of 3, written over `memory`'s code as a loader does.
fn rewrite_adding(cpu: &mut HostProcessExecutor, memory: MemoryId, step: i64) {
    let mut words = adding_program();
    words[2] = encode::add_immediate(0, 0, step).expect("an add");
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    cpu.set_permissions(memory, CODE, PAGE, Permissions::READ | Permissions::WRITE)
        .expect("writable");
    cpu.write(memory, CODE, &bytes).expect("the code");
    cpu.set_permissions(memory, CODE, PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("executable");
    cpu.code_changed(memory, CODE, PAGE, &[CODE]);
}

/// A fork child runs the code its parent loaded, loading none of its own, until one of them changes its executable
/// pages: that one makes its own and the other keeps running what it ran.
#[test]
fn a_fork_child_runs_its_parent_s_code_until_either_changes_it() {
    let mut cpu = executor();
    let (parent, parent_thread) = guest(&mut cpu, &adding_program());
    assert_eq!(run_adding(&mut cpu, parent, parent_thread), SUM);
    let child = cpu.memory_duplicate(parent).expect("fork");
    let child_thread = cpu.thread_new(child).expect("thread");
    let before = cpu.counters();
    assert_eq!(run_adding(&mut cpu, child, child_thread), SUM);
    let after = cpu.counters();
    assert_eq!(
        after.pieces_loaded_ahead, before.pieces_loaded_ahead,
        "the child loaded nothing"
    );
    assert_eq!(after.codes_unshared, before.codes_unshared);
    // The child's loader writes new code: it stops sharing; the parent runs what it ran.
    rewrite_adding(&mut cpu, child, 5);
    assert_eq!(run_adding(&mut cpu, child, child_thread), 100 * 5 + 1000);
    assert_eq!(run_adding(&mut cpu, parent, parent_thread), SUM);
    assert_eq!(cpu.counters().codes_unshared, before.codes_unshared + 1);
    // A second child shares the parent's code; this time the parent changes it.
    let second = cpu.memory_duplicate(parent).expect("fork");
    let second_thread = cpu.thread_new(second).expect("thread");
    rewrite_adding(&mut cpu, parent, 7);
    assert_eq!(run_adding(&mut cpu, parent, parent_thread), 100 * 7 + 1000);
    assert_eq!(run_adding(&mut cpu, second, second_thread), SUM);
    assert_eq!(run_adding(&mut cpu, child, child_thread), 100 * 5 + 1000);
    assert_eq!(cpu.counters().codes_unshared, before.codes_unshared + 2);
}

/// A program that names its own addresses: a loop of 10 adds of 3, a call to a function that adds 1000, the next page's
/// address by `adr` in x4, a store and a load of the sum there, getpid, exit.
fn placed_program() -> Vec<u32> {
    let add = |rd, rn, value| encode::add_immediate(rd, rn, value).expect("an add");
    let mut words = vec![
        encode::movz(0, 0, 0),
        encode::movz(1, 10, 0),
        add(0, 0, 3),
        add(1, 1, -1),
        encode::compare_branch(true, 1, -2 * INSTRUCTION).expect("cbnz"),
    ];
    let call_at = words.len();
    words.push(0);
    let adr_at = words.len();
    words.push(encode::adr(4, PAGE as i64 - adr_at as i64 * INSTRUCTION).expect("adr"));
    words.push(encode::load_store(form::STR, 0, 4, 0, WORD).expect("str"));
    words.push(encode::load_store(form::LDR, 3, 4, 0, WORD).expect("ldr"));
    words.extend(syscall(GETPID));
    words.extend(syscall(EXIT));
    let function = words.len();
    words.push(add(0, 0, 1000));
    words.push(encode::branch_register(form::RET, LINK));
    words[call_at] = encode::branch(true, (function - call_at) as i64 * INSTRUCTION).expect("bl");
    words
}

/// The placed program's sum, and the words before its call.
const PLACED_SUM: u64 = 10 * 3 + 1000;
const PLACED_CALL_AT: u64 = 5;

/// How many times guest code was translated so far.
fn translations(cpu: &HostProcessExecutor) -> u64 {
    cpu.slow_paths()
        .into_iter()
        .find(|(reason, _)| *reason == "native code translated")
        .map_or(0, |(_, count)| count)
}

/// Runs the placed program with its code at `base` and its data the page after, in a memory of its own; checks every
/// address it named and its sum.
fn run_placed_at(cpu: &mut HostProcessExecutor, base: u64) {
    let memory = cpu.memory_new().expect("memory");
    let bytes: Vec<u8> = placed_program()
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    cpu.set_permissions(memory, base, PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code");
    cpu.set_permissions(
        memory,
        base + PAGE,
        PAGE,
        Permissions::READ | Permissions::WRITE,
    )
    .expect("data");
    cpu.write(memory, base, &bytes).expect("the code");
    cpu.code_changed(memory, base, PAGE, &[base]);
    let thread = cpu.thread_new(memory).expect("thread");
    let mut registers = RegisterBlock::ZERO;
    registers.pc = base;
    cpu.set_registers(thread, &registers);
    let seen = run_to_exit(cpu, thread);
    assert_eq!(seen.exits.len(), 2, "at {base:#x}: {:?}", seen.exits);
    let (exit, registers, _) = &seen.exits[0];
    assert!(
        matches!(exit, Exit::Syscall(call) if call.number.0 == u64::from(GETPID)),
        "at {base:#x}: {exit:?}"
    );
    assert_eq!(registers.x[0], PLACED_SUM, "at {base:#x}");
    assert_eq!(registers.x[3], PLACED_SUM, "at {base:#x}");
    assert_eq!(registers.x[4], base + PAGE, "adr at {base:#x}");
    assert_eq!(
        registers.x[usize::from(LINK)],
        base + (PLACED_CALL_AT + 1) * INSTRUCTION as u64,
        "the call's return at {base:#x}"
    );
    assert_eq!(
        registers.pc,
        base + 11 * INSTRUCTION as u64,
        "after the svc at {base:#x}"
    );
    assert_eq!(
        read_word(cpu, memory, base + PAGE),
        PLACED_SUM,
        "at {base:#x}"
    );
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
}

#[test]
fn code_placed_at_another_base_runs_from_its_kept_translation_moved_there() {
    let mut cpu = executor();
    // Pages every translation moves between (4 GiB to under the slots' reach of 2^44), and pages under them where
    // the kernel places a program low, from which every address this code names stays in reach.
    run_placed_at(&mut cpu, 0x2_0000_0000);
    let made = translations(&cpu);
    assert!(made > 0, "the first run translates");
    for base in [
        0x7_1234_5000,
        0x2_0000_0000,
        0xaaa_aaaa_a000,
        0xab0f_3000,
        0x1000_0000,
    ] {
        run_placed_at(&mut cpu, base);
        assert_eq!(
            translations(&cpu),
            made,
            "moved to {base:#x}, not translated"
        );
    }
    // Under 128 MiB a direct branch of the code could wrap below zero, so code there keeps a translation of its own.
    run_placed_at(&mut cpu, CODE);
    assert!(translations(&cpu) > made, "translated again at {CODE:#x}");
}

/// The pages of the chained program, and the one whose add [`chained_program`] can change.
const CHAINED_PAGES: u64 = 8;
const CHANGED_PAGE: u64 = 3;
const CHANGED_ADD: u32 = 100;

/// One page after another, the page at index i adding i + 1 to x0 and branching to the next; the last calls getpid,
/// then exit. With `changed`, page [`CHANGED_PAGE`] adds [`CHANGED_ADD`] instead.
fn chained_program(changed: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    for page in 0..CHAINED_PAGES - 1 {
        let added = if changed && page == CHANGED_PAGE {
            CHANGED_ADD
        } else {
            page as u32 + 1
        };
        let mut words = vec![
            encode::add_immediate(0, 0, i64::from(added)).expect("an add"),
            encode::branch(false, PAGE as i64 - INSTRUCTION).expect("b"),
        ];
        words.resize(PAGE as usize / size_of::<u32>(), 0);
        bytes.extend(words.iter().flat_map(|w| w.to_le_bytes()));
    }
    let [getpid, exit] = [syscall(GETPID), syscall(EXIT)];
    bytes.extend(getpid.iter().chain(&exit).flat_map(|w| w.to_le_bytes()));
    bytes
}

/// The chained program's sum in x0 at its getpid, as made.
fn chained_sum(changed: bool) -> u64 {
    let sum = (1..CHAINED_PAGES).sum::<u64>();
    if changed {
        sum - (CHANGED_PAGE + 1) + u64::from(CHANGED_ADD)
    } else {
        sum
    }
}

/// Runs the chained program at `base` in a memory of its own, `not_executable` a page left readable only; its first
/// exit, x0 there, and how many pieces were loaded ahead meanwhile.
fn run_chained_at(
    cpu: &mut HostProcessExecutor,
    base: u64,
    changed: bool,
    not_executable: Option<u64>,
) -> (Exit, u64, u64) {
    let ahead = cpu.counters().pieces_loaded_ahead;
    let memory = cpu.memory_new().expect("memory");
    cpu.set_permissions(
        memory,
        base,
        CHAINED_PAGES * PAGE,
        Permissions::READ | Permissions::EXECUTE,
    )
    .expect("code");
    if let Some(page) = not_executable {
        cpu.set_permissions(memory, base + page * PAGE, PAGE, Permissions::READ)
            .expect("not executable");
    }
    cpu.write(memory, base, &chained_program(changed))
        .expect("the code");
    cpu.code_changed(memory, base, CHAINED_PAGES * PAGE, &[base]);
    let thread = cpu.thread_new(memory).expect("thread");
    let mut registers = RegisterBlock::ZERO;
    registers.pc = base;
    cpu.set_registers(thread, &registers);
    let seen = run_to_exit(cpu, thread);
    let (exit, registers, _) = &seen.exits[0];
    let (exit, sum) = (*exit, registers.x[0]);
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
    (exit, sum, cpu.counters().pieces_loaded_ahead - ahead)
}

#[test]
fn pieces_an_earlier_memory_loaded_next_are_loaded_ahead_only_where_their_code_and_permissions_are_the_same()
 {
    let mut cpu = executor();
    let base = 0x2_0000_0000;
    let getpid =
        |exit: Exit| matches!(exit, Exit::Syscall(call) if call.number.0 == u64::from(GETPID));
    // The first memory stops at every page.
    let (exit, sum, ahead) = run_chained_at(&mut cpu, base, false, None);
    assert!(getpid(exit), "{exit:?}");
    assert_eq!((sum, ahead), (chained_sum(false), 0));
    // The next one, here and at another base, loads every later page and the code after the getpid at its first stop.
    for at in [base, 0x7_1234_5000] {
        let (exit, sum, ahead) = run_chained_at(&mut cpu, at, false, None);
        assert!(getpid(exit), "at {at:#x}: {exit:?}");
        assert_eq!(
            (sum, ahead),
            (chained_sum(false), CHAINED_PAGES),
            "at {at:#x}"
        );
    }
    // Code that differs is not loaded ahead: its own add is what runs. The pages before it are loaded at the first
    // stop, and the pages after the one after it at the stop there.
    let (exit, sum, ahead) = run_chained_at(&mut cpu, base, true, None);
    assert!(getpid(exit), "{exit:?}");
    assert_eq!((sum, ahead), (chained_sum(true), CHAINED_PAGES - 2));
    // Nor is a page the guest may not run, though the changed page now is: running into it is the guest's fault there.
    let (exit, _, ahead) = run_chained_at(&mut cpu, base, true, Some(CHANGED_PAGE + 2));
    assert_eq!(
        exit,
        Exit::MemoryFault {
            address: base + (CHANGED_PAGE + 2) * PAGE,
            access: Access::Execute
        }
    );
    assert_eq!(ahead, CHANGED_PAGE + 1);
}

#[test]
fn a_process_s_thread_past_its_last_index_is_refused_and_counted() {
    let mut cpu = executor();
    let (memory, first) = guest(&mut cpu, &adding_program());
    let mut threads = vec![first];
    let refused = loop {
        match cpu.thread_new(memory) {
            Ok(thread) => threads.push(thread),
            Err(error) => break error,
        }
        assert!(
            threads.len() <= THREADS_PER_PROCESS,
            "more threads than indices"
        );
    };
    assert_eq!(threads.len(), THREADS_PER_PROCESS);
    assert_eq!(refused, CpuError::OutOfMemory);
    assert_eq!(cpu.counters().threads_exhausted, 1);
    // The last thread made runs guest code.
    let last = *threads.last().expect("threads");
    let mut registers = RegisterBlock::ZERO;
    registers.pc = CODE;
    registers.x[2] = DATA;
    cpu.set_registers(last, &registers);
    let seen = run_to_exit(&mut cpu, last);
    assert_eq!(seen.exits[0].1.x[0], SUM, "{:?}", seen.exits);
    // A thread let go makes room for one more.
    cpu.thread_drop(first);
    let again = cpu
        .thread_new(memory)
        .expect("a thread after one was let go");
    assert_eq!(cpu.thread_new(memory), Err(CpuError::OutOfMemory));
    assert_eq!(cpu.counters().threads_exhausted, 2);
    cpu.thread_drop(again);
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

/// A jump through a register into data pages, never code, reaches an empty slot each time; the page of the slot table it
/// read is given back before the next turn, so a guest jumping to many places holds no slot pages for them.
#[test]
fn slot_pages_a_jump_reached_and_nothing_wrote_are_given_back() {
    let jumps = 16;
    let mut cpu = executor();
    let (memory, thread) = guest(&mut cpu, &[encode::branch_register(form::BR, 2)]);
    cpu.set_permissions(
        memory,
        DATA,
        jumps * PAGE,
        Permissions::READ | Permissions::WRITE,
    )
    .expect("data");
    let before = cpu.counters().slot_pages_given_back;
    for jump in 0..=jumps {
        let mut registers = RegisterBlock::ZERO;
        registers.pc = CODE;
        registers.x[2] = DATA + jump * PAGE;
        cpu.set_registers(thread, &registers);
        let seen = run_to_exit(&mut cpu, thread);
        assert!(
            matches!(seen.exits[..], [(Exit::MemoryFault { address, access: Access::Execute }, _, _)]
                if address == registers.x[2]),
            "{:?}",
            seen.exits
        );
        assert_eq!(
            cpu.counters().slot_pages_given_back - before,
            jump,
            "each turn gives back the page the last one reached"
        );
    }
}

/// Calls the function whose address is at x2's data 50 times through a register, then exits; of its two functions the
/// first adds 1 to x0 and the second 2. Where each function is.
fn calling_from_data_program() -> (Vec<u32>, [u64; 2]) {
    let add = |rd, rn, value| encode::add_immediate(rd, rn, value).expect("an add");
    let mut words = vec![
        encode::movz(0, 0, 0),
        encode::movz(1, 50, 0),
        encode::load_store(form::LDR, 9, 2, 0, WORD).expect("ldr"),
    ];
    let call = words.len();
    words.push(encode::branch_register(form::BLR, 9));
    words.push(add(1, 1, -1));
    words.push(
        encode::compare_branch(true, 1, (call as i64 - words.len() as i64) * INSTRUCTION)
            .expect("cbnz"),
    );
    words.extend(syscall(EXIT));
    let mut functions = [0; 2];
    for (function, added) in functions.iter_mut().zip([1, 2]) {
        *function = CODE + words.len() as u64 * INSTRUCTION as u64;
        words.push(add(0, 0, added));
        words.push(encode::branch_register(form::RET, LINK));
    }
    (words, functions)
}

#[test]
fn last_targets_an_earlier_memory_made_are_made_ahead_and_a_jump_elsewhere_still_goes_where_it_jumps()
 {
    let mut cpu = executor();
    let (words, functions) = calling_from_data_program();
    let mut made = Vec::new();
    for (function, sum) in [(functions[0], 50), (functions[0], 50), (functions[1], 100)] {
        let before = cpu.counters().last_targets_made_ahead;
        let (memory, thread) = guest(&mut cpu, &words);
        cpu.write(memory, DATA, &function.to_le_bytes())
            .expect("the function's address");
        let seen = run_to_exit(&mut cpu, thread);
        let (exit, registers, _) = &seen.exits[0];
        assert!(
            matches!(exit, Exit::Syscall(call) if call.number.0 == u64::from(EXIT)),
            "{exit:?}"
        );
        assert_eq!(registers.x[0], sum, "calling {function:#x}");
        made.push(cpu.counters().last_targets_made_ahead - before);
        cpu.thread_drop(thread);
        cpu.memory_drop(memory);
    }
    // The call's and the return's, made ahead in the second and the third memory, the third's call's wrong.
    assert_eq!(made[0], 0);
    assert!(made[1] >= 2, "{made:?}");
    assert!(made[2] >= 2, "{made:?}");
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
        run(
            &mut cpu,
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

/// `stp x0, x0, [sp, #8]`, a form the encoder lacks (the test checks the decoder reads it so).
const STORE_PAIR_PAST_SP: u32 = 0xa900_83e0;
/// The stack pointer's register number where `add` names it.
const SP: u8 = 31;

/// Runs `words` to their first exit, which must be their second instruction's.
fn first_exit_at_the_second_instruction(words: &[u32]) -> (Exit, HostProcessExecutor, MemoryId) {
    let mut cpu = executor();
    let (memory, thread) = guest(&mut cpu, words);
    let seen = run_to_exit(&mut cpu, thread);
    assert_eq!(seen.exits.len(), 1, "{:?}", seen.exits);
    let (exit, registers, _) = seen.exits[0];
    assert_eq!(registers.pc, CODE + INSTRUCTION as u64, "{exit:?}");
    (exit, cpu, memory)
}

#[test]
fn a_store_probed_before_it_crosses_a_page_faults_as_a_write_and_a_misaligned_stack_an_alignment() {
    // A store whose last bytes fall on the unmapped page after the data: the access is probed first.
    let (exit, mut cpu, memory) = first_exit_at_the_second_instruction(&[
        encode::add_immediate(0, 2, (PAGE - WORD / 2) as i64).expect("an add"),
        encode::load_store(form::STR, 1, 0, 0, WORD).expect("str"),
    ]);
    assert_eq!(
        exit,
        Exit::MemoryFault {
            address: DATA + PAGE,
            access: Access::Write,
        }
    );
    assert_eq!(
        read_word(&mut cpu, memory, DATA + PAGE - WORD),
        0,
        "nothing stored"
    );
    // A pair through a stack pointer that is not a multiple of 16: in a page the hardware checks it, and where the
    // pair may cross into the next page the check runs among the probes.
    assert!(matches!(
        wcontainers_arm64::decode(STORE_PAIR_PAST_SP),
        Instruction::StorePair {
            bytes: 8,
            address: Address::Offset {
                base: Reg::Sp,
                offset: 8
            },
            ..
        }
    ));
    for offset in [WORD, PAGE - WORD] {
        let (exit, _, _) = first_exit_at_the_second_instruction(&[
            encode::add_immediate(SP, 2, offset as i64).expect("an add"),
            STORE_PAIR_PAST_SP,
        ]);
        assert_eq!(
            exit,
            Exit::AlignmentFault {
                address: DATA + offset
            },
            "sp {offset:#x} into the data"
        );
    }
}

/// The general registers a thread stores, x0 to x30, and where its vector registers go after them.
const GENERAL_REGISTERS: u8 = 31;
const VECTORS: u8 = 32;
const VECTOR_BYTES: u64 = 16;
const VECTORS_AT: u64 = (GENERAL_REGISTERS as u64 * WORD).next_multiple_of(VECTOR_BYTES);

/// Stores every general register but x2 and every vector register at x2, then exits.
fn storing_program() -> Vec<u32> {
    let mut words: Vec<u32> = (0..GENERAL_REGISTERS)
        .map(|r| encode::load_store(form::STR, r, 2, u64::from(r) * WORD, WORD).expect("str"))
        .collect();
    words.extend((0..VECTORS).map(|v| {
        encode::load_store(
            form::STR_Q,
            v,
            2,
            VECTORS_AT + u64::from(v) * VECTOR_BYTES,
            VECTOR_BYTES,
        )
        .expect("str q")
    }));
    words.extend(syscall(EXIT));
    words
}

/// Where the second thread's program and data start, after the first's.
const SECOND_CODE: u64 = CODE + PAGE / 2;
const SECOND_DATA: u64 = DATA + PAGE / 2;

/// brief-native.md's register isolation on translated code: a thread that holds its own values in every register is
/// stopped at a system call; a new thread of the same process then reads zeros in every register but those it was
/// given; then the first goes on and finds exactly its own.
#[test]
fn a_new_thread_reads_zeros_and_a_thread_resumed_reads_its_own_registers() {
    let mut cpu = executor();
    // The first thread: getpid, then the storing program.
    let mut first_words = syscall(GETPID).to_vec();
    first_words.extend(storing_program());
    let mut words = storing_program();
    words.resize((PAGE / 2 / INSTRUCTION as u64) as usize, 0);
    words.extend(first_words);
    let (memory, second) = guest(&mut cpu, &words);
    let first = cpu.thread_new(memory).expect("thread");
    let mut own = RegisterBlock::ZERO;
    for (n, x) in own.x.iter_mut().enumerate() {
        *x = 0x1111_0000 + n as u64;
    }
    own.x[2] = SECOND_DATA;
    own.x[SYSCALL_NUMBER] = 0;
    for (n, v) in own.v.iter_mut().enumerate() {
        *v = [0x2222_0000 + n as u64, 0x3333_0000 + n as u64];
    }
    own.tpidr_el0 = 0x4444;
    own.pc = SECOND_CODE;
    cpu.set_registers(first, &own);
    let mut seen = Seen::new(first);
    seen.leave_at_calls = true;
    run_through_stops(&mut cpu, first, Resume::FromRegisters, &mut seen);
    assert!(
        matches!(seen.exits.last(), Some((Exit::Syscall(call), _, _)) if call.number.0 == u64::from(GETPID)),
        "{:?}",
        seen.exits
    );
    // The new thread, given only its pc and x2.
    run_to_exit(&mut cpu, second);
    for r in (0..GENERAL_REGISTERS).filter(|&r| r != 2) {
        assert_eq!(
            read_word(&mut cpu, memory, DATA + u64::from(r) * WORD),
            0,
            "x{r} of a new thread"
        );
    }
    for v in 0..VECTORS {
        for half in 0..2 {
            let at = DATA + VECTORS_AT + u64::from(v) * VECTOR_BYTES + half * WORD;
            assert_eq!(read_word(&mut cpu, memory, at), 0, "v{v} of a new thread");
        }
    }
    // The first thread goes on from its system call with its own registers.
    let mut seen = Seen::new(first);
    run_through_stops(&mut cpu, first, Resume::Return(RETURNED), &mut seen);
    assert!(
        matches!(seen.exits.last(), Some((Exit::Syscall(call), _, _)) if call.number.0 == u64::from(EXIT)),
        "{:?}",
        seen.exits
    );
    for r in (0..GENERAL_REGISTERS).filter(|&r| r != 2 && usize::from(r) != SYSCALL_NUMBER) {
        let expected = if r == 0 {
            RETURNED
        } else {
            own.x[usize::from(r)]
        };
        assert_eq!(
            read_word(&mut cpu, memory, SECOND_DATA + u64::from(r) * WORD),
            expected,
            "x{r}"
        );
    }
    for v in 0..VECTORS {
        for half in 0..2 {
            let at = SECOND_DATA + VECTORS_AT + u64::from(v) * VECTOR_BYTES + half * WORD;
            assert_eq!(
                read_word(&mut cpu, memory, at),
                own.v[usize::from(v)][half as usize],
                "v{v}"
            );
        }
    }
    let mut after = RegisterBlock::ZERO;
    cpu.registers(first, &mut after);
    assert_eq!(after.tpidr_el0, own.tpidr_el0, "the thread pointer kept");
}

/// `dc cvau, x3`, `dsb ish`, `ic ivau, x3`, `dsb ish` and `isb` (Arm ARM, DC CVAU, DSB, IC IVAU and ISB): what a program
/// that wrote code at x3 runs before running it, as the `CTR_EL0` we present asks.
const ANNOUNCE_X3: [u32; 5] = [
    0xd50b_7b23,
    0xd503_3b9f,
    0xd50b_7523,
    0xd503_3b9f,
    0xd503_3fdf,
];

/// A memory whose code page is writable and executable, with the program at its start, and a thread there.
fn writable_code(cpu: &mut HostProcessExecutor, words: &[u32]) -> (MemoryId, ThreadId) {
    let (memory, thread) = guest(cpu, words);
    let all = Permissions::READ | Permissions::WRITE | Permissions::EXECUTE;
    cpu.set_permissions(memory, CODE, PAGE, all)
        .expect("writable and executable");
    (memory, thread)
}

/// Stores of x1 into the word at x3 and the next, then the announcement of it.
fn store_and_announce(value: u64) -> Vec<u32> {
    let mut words = encode::move_constant(1, value);
    words.push(encode::load_store(form::STR, 1, 3, 0, WORD).expect("a store"));
    words.extend(ANNOUNCE_X3);
    words
}

/// A store into the writable and executable page the storing code is on completes, as on Linux, rather than fault
/// for ever: the page is left writable once written.
#[test]
fn a_store_into_the_page_its_own_code_is_on_completes() {
    let mut cpu = executor();
    let last = CODE + PAGE - WORD;
    let mut words = vec![encode::adr(3, (last - CODE) as i64).expect("an adr")];
    words.push(encode::load_store(form::STR, 3, 3, 0, WORD).expect("a store"));
    words.extend(syscall(EXIT));
    let (memory, thread) = writable_code(&mut cpu, &words);
    let before = cpu.counters();
    let seen = run_to_exit(&mut cpu, thread);
    assert!(
        matches!(seen.exits[..], [(Exit::Syscall(call), _, _)] if call.number.0 == u64::from(EXIT)),
        "{:?}",
        seen.exits
    );
    assert_eq!(read_word(&mut cpu, memory, last), last, "the stored word");
    assert_eq!(
        cpu.counters().code_pages_left_writable,
        before.code_pages_left_writable + 1
    );
}

/// A function on the page of its caller, rewritten and announced twice by the caller between calls, runs its latest
/// code at each call: the page left writable after the first store still loses its code at each announcement.
#[test]
fn a_function_rewritten_on_its_caller_s_page_runs_its_latest_code() {
    let ret = encode::branch_register(form::RET, LINK);
    let adding = |step| u64::from(encode::add_immediate(5, 5, step).expect("an add"));
    let mut words = vec![encode::movz(5, 0, 0), 0];
    let mut calls = Vec::new();
    calls.push(words.len());
    words.push(0);
    for step in [5, 9] {
        words.extend(store_and_announce(u64::from(ret) << 32 | adding(step)));
        calls.push(words.len());
        words.push(0);
    }
    words.extend(syscall(EXIT));
    // The function's two words are one aligned doubleword, stored at once.
    if words.len() % 2 == 1 {
        words.push(encode::exception(form::BRK, 0));
    }
    let function = words.len();
    words.extend([adding(1) as u32, ret]);
    words[1] = encode::adr(3, (function as i64 - 1) * INSTRUCTION).expect("an adr");
    for call in calls {
        words[call] =
            encode::branch(true, (function as i64 - call as i64) * INSTRUCTION).expect("a call");
    }
    let mut cpu = executor();
    let (_, thread) = writable_code(&mut cpu, &words);
    let seen = run_to_exit(&mut cpu, thread);
    assert!(
        matches!(seen.exits[..], [(Exit::Syscall(call), _, _)] if call.number.0 == u64::from(EXIT)),
        "{:?}",
        seen.exits
    );
    assert_eq!(
        seen.exits[0].1.x[5],
        1 + 5 + 9,
        "each call ran the latest add"
    );
}
