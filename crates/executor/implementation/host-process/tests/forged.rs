//! A guest that forges its own exits (plan 13): after A forks B and C and A's store gives the page to B, which then
//! lends it to C, B's translated code writes forged exit reports, a forged thread id and the request word into its
//! register page, runs on, then stores to the page. Whatever the embedder makes of the forgeries, only B is hurt: C
//! keeps the page as at the fork, A keeps its own store, both still run, and the runner returns.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use wcontainers_abi::{Exit, MemoryId, Permissions, RegisterBlock, Resume, ThreadId};
use wcontainers_arm64::encode::{self, form};
use wcontainers_executor_host_process::executor::HostProcessExecutor;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_executor_host_process::layout::{OUR_AREA, REGISTER_PAGES, THREAD_PAGES};
use wcontainers_executor_host_process::pages::{control, register};
use wcontainers_executor_host_process::stub::StopWhen;

static HOST: std::sync::OnceLock<Arc<HostProcessHost>> = std::sync::OnceLock::new();

fn executor() -> HostProcessExecutor {
    HostProcessExecutor::new(
        HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
            .clone(),
    )
}

const PAGE: u64 = 4096;
const CODE: u64 = 0x40_0000;
const DATA: u64 = 0x50_0000;
const WORD: u64 = size_of::<u64>() as u64;
const INSTRUCTION: i64 = size_of::<u32>() as i64;
const EXIT: u16 = linux_raw_sys::general::__NR_exit as u16;
const SYSCALL_NUMBER: u8 = 8;
/// Where each routine starts in the code page: a store of x1 to the data, a load of it into x0, and the forger.
const WRITE_AT: u64 = CODE;
const READ_AT: u64 = CODE + 0x40;
const FORGE_AT: u64 = CODE + 0x80;
/// A store of x1 to the address in x6.
const STORE_AT: u64 = CODE + 0x100;
/// The data as written before the forks, A's store and B's store.
const BEFORE: u64 = 0x1111;
const BY_A: u64 = 0x2222;
const BY_B: u64 = 0x3333;
/// How long B runs on after its forgeries, in loop passes, and how long a case may hold the runner.
const SPIN: u64 = 1 << 30;
const WATCHDOG: Duration = Duration::from_secs(20);
/// The most runs a case takes before it is judged held.
const RUNS: usize = 200;
/// A data abort from a lower level that wrote: the exception class and the write bit of its syndrome.
const DATA_ABORT_LOWER: u64 = 0x24 << 26;
const WRITE_NOT_READ: u64 = 1 << 6;
/// The host's numbers for a fault: SIGSEGV, and SEGV_ACCERR in the detail's upper half.
const SEGV_ACCERR_DETAIL: u64 =
    (libc::SIGSEGV as u64) | ((linux_raw_sys::general::SEGV_ACCERR as u64) << 32);
/// The thread index B's thread runs on: a new memory's first host thread, kept spare until its first thread.
const FIRST_INDEX: u64 = 0;

fn exit_call() -> [u32; 2] {
    [
        encode::movz(SYSCALL_NUMBER, EXIT, 0),
        encode::exception(form::SVC, 0),
    ]
}

/// The code page: the three routines at their offsets.
fn code_page() -> Vec<u8> {
    let mut words = vec![0u32; (PAGE / 4) as usize];
    let at = |address: u64| ((address - CODE) / 4) as usize;
    let mut put = |start: u64, routine: &[u32]| {
        words[at(start)..at(start) + routine.len()].copy_from_slice(routine);
    };
    let store = |rt, rn, offset| encode::load_store(form::STR, rt, rn, offset, WORD).expect("str");
    let load = |rt, rn, offset| encode::load_store(form::LDR, rt, rn, offset, WORD).expect("ldr");
    let mut write = vec![store(1, 2, 0)];
    write.extend(exit_call());
    put(WRITE_AT, &write);
    let mut read = vec![load(0, 2, 0)];
    read.extend(exit_call());
    put(READ_AT, &read);
    // x5 the register page, x6 the control page, x7 the kind, x9 the value, x10 the detail, x11 the syndrome, x13 the
    // passes to run on.
    let mut forge = vec![
        store(7, 5, register::EXIT as u64),
        store(9, 5, register::EXIT_VALUE as u64),
        store(10, 5, register::EXIT_DETAIL as u64),
        store(11, 5, register::EXIT_SYNDROME as u64),
        load(12, 6, control::ANSWER as u64),
        // The request word and, above it, the thread id.
        store(12, 5, register::REQUEST as u64),
        encode::add_immediate(13, 13, -1).expect("an add"),
        encode::compare_branch(true, 13, -INSTRUCTION).expect("cbnz"),
        store(1, 2, 0),
    ];
    forge.extend(exit_call());
    put(FORGE_AT, &forge);
    let mut store_to = vec![store(1, 6, 0)];
    store_to.extend(exit_call());
    put(STORE_AT, &store_to);
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// What a run saw: every exit, the registers at the last, and the slow paths taken by then.
struct Seen {
    exits: Vec<Exit>,
    last: RegisterBlock,
    warnings: Vec<String>,
}

/// Runs `thread` from `registers`, answering every system call but `exit` with 0 and going on after every stop, until
/// it leaves for anything else, at most [`RUNS`] runs; what it saw.
fn run(cpu: &mut HostProcessExecutor, thread: ThreadId, registers: &RegisterBlock) -> Seen {
    cpu.set_registers(thread, registers);
    let mut seen = Seen {
        exits: Vec::new(),
        last: RegisterBlock::ZERO,
        warnings: Vec::new(),
    };
    let mut resume = Resume::FromRegisters;
    for _ in 0..RUNS {
        let exit = cpu.run_translated(thread, resume, None, StopWhen::default(), &mut || 0);
        seen.exits.push(exit);
        cpu.registers(thread, &mut seen.last);
        resume = match exit {
            Exit::Syscall(call) if call.number.0 != u64::from(EXIT) => Resume::Return(0),
            Exit::Stop => Resume::Continue,
            _ => break,
        };
    }
    seen.warnings = cpu
        .slow_paths()
        .into_iter()
        .map(|(reason, count)| format!("{reason} ({count})"))
        .collect();
    seen
}

fn registers_at(pc: u64, value: u64) -> RegisterBlock {
    let mut registers = RegisterBlock::ZERO;
    registers.pc = pc;
    registers.x[1] = value;
    registers.x[2] = DATA;
    registers
}

/// Runs the read routine on a new thread of `memory`, giving the word it loaded.
fn guest_reads(cpu: &mut HostProcessExecutor, memory: MemoryId) -> u64 {
    let thread = cpu.thread_new(memory).expect("thread");
    let seen = run(cpu, thread, &registers_at(READ_AT, 0));
    assert!(
        matches!(seen.exits.last(), Some(Exit::Syscall(call)) if call.number.0 == u64::from(EXIT)),
        "{:?}",
        seen.exits
    );
    cpu.thread_drop(thread);
    seen.last.x[0]
}

fn word(cpu: &mut HostProcessExecutor, memory: MemoryId) -> u64 {
    let mut bytes = [0; WORD as usize];
    cpu.read(memory, DATA, &mut bytes).expect("read");
    u64::from_le_bytes(bytes)
}

/// One case: B forges an exit of `kind` with `value`, the fault's detail and `syndrome`.
fn case(kind: u64, value: u64, syndrome: u64) {
    let mut cpu = executor();
    let a = cpu.memory_new().expect("memory");
    cpu.set_permissions(a, CODE, PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code");
    cpu.set_permissions(a, DATA, PAGE, Permissions::READ | Permissions::WRITE)
        .expect("data");
    cpu.write(a, CODE, &code_page()).expect("the code");
    cpu.code_changed(a, CODE, PAGE, &[WRITE_AT, READ_AT, FORGE_AT, STORE_AT]);
    cpu.write(a, DATA, &BEFORE.to_le_bytes()).expect("write");
    let b = cpu.memory_duplicate(a).expect("fork B");
    let c = cpu.memory_duplicate(a).expect("fork C");
    // A's store gives the page to its first borrower, B, from which C now borrows.
    let a_thread = cpu.thread_new(a).expect("thread");
    let seen = run(&mut cpu, a_thread, &registers_at(WRITE_AT, BY_A));
    assert!(
        matches!(seen.exits.last(), Some(Exit::Syscall(_))),
        "{:?}",
        seen.exits
    );
    assert_eq!(word(&mut cpu, a), BY_A);
    assert_eq!(word(&mut cpu, b), BEFORE);
    assert_eq!(word(&mut cpu, c), BEFORE);
    // B forges, runs on and stores, watched so a held runner fails the case instead of the run.
    let b_thread = cpu.thread_new(b).expect("thread");
    let mut forging = registers_at(FORGE_AT, BY_B);
    forging.x[5] = REGISTER_PAGES as u64 + FIRST_INDEX * PAGE;
    forging.x[6] = THREAD_PAGES as u64 + FIRST_INDEX * PAGE;
    forging.x[7] = kind;
    forging.x[9] = value;
    forging.x[10] = SEGV_ACCERR_DETAIL;
    forging.x[11] = syndrome;
    forging.x[13] = SPIN;
    let stub = cpu.stub_process(b);
    let done = Arc::new(AtomicBool::new(false));
    let watchdog = {
        let done = Arc::clone(&done);
        std::thread::spawn(move || {
            let start = Instant::now();
            while start.elapsed() < WATCHDOG {
                if done.load(Ordering::Acquire) {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let _ = rustix::process::kill_process(stub, rustix::process::Signal::KILL);
            true
        })
    };
    let started = Instant::now();
    let seen = run(&mut cpu, b_thread, &forging);
    done.store(true, Ordering::Release);
    let held = watchdog.join().expect("the watchdog");
    let ran = started.elapsed();
    println!(
        "kind {kind} value {value:#x} syndrome {syndrome:#x}: {} exits, last {:?}, {ran:?}, B's word {:#x}, warnings {:?}",
        seen.exits.len(),
        seen.exits.last(),
        word(&mut cpu, b),
        seen.warnings
    );
    assert!(
        !held,
        "the forged exit held the runner for {WATCHDOG:?}: {:?}",
        seen.exits
    );
    let b_word = word(&mut cpu, b);
    assert!(
        b_word == BEFORE || b_word == BY_B,
        "B's page holds {b_word:#x}, neither its own store nor the fork's"
    );
    cpu.thread_drop(b_thread);
    // A and C are untouched, and their guests still run and read their own.
    assert_eq!(
        word(&mut cpu, c),
        BEFORE,
        "C's copy is the page at the fork"
    );
    assert_eq!(word(&mut cpu, a), BY_A, "A's store stays A's");
    assert_eq!(guest_reads(&mut cpu, c), BEFORE);
    assert_eq!(guest_reads(&mut cpu, a), BY_A);
    if b_word == BY_B {
        assert_eq!(guest_reads(&mut cpu, b), BY_B, "B's own store stays B's");
    }
    cpu.thread_drop(a_thread);
    for memory in [c, b, a] {
        cpu.memory_drop(memory);
    }
}

/// Every exit kind the stub writes and two it never does.
const KINDS: std::ops::RangeInclusive<u64> = 0..=9;

#[test]
fn forged_exits_naming_the_lent_page_hurt_only_the_forger() {
    for kind in KINDS {
        case(kind, DATA, DATA_ABORT_LOWER | WRITE_NOT_READ);
        case(kind, DATA, DATA_ABORT_LOWER);
    }
}

#[test]
fn forged_exits_naming_other_addresses_hurt_only_the_forger() {
    for kind in KINDS {
        for value in [
            0,
            DATA + PAGE,
            CODE,
            REGISTER_PAGES as u64,
            OUR_AREA as u64,
            u64::MAX,
        ] {
            case(kind, value, DATA_ABORT_LOWER | WRITE_NOT_READ);
        }
    }
}

#[test]
fn a_guest_store_to_its_control_pages_is_its_own_fault_there() {
    let mut cpu = executor();
    let memory = cpu.memory_new().expect("memory");
    cpu.set_permissions(memory, CODE, PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code");
    cpu.set_permissions(memory, DATA, PAGE, Permissions::READ | Permissions::WRITE)
        .expect("data");
    cpu.write(memory, CODE, &code_page()).expect("the code");
    cpu.code_changed(memory, CODE, PAGE, &[STORE_AT, READ_AT]);
    let thread = cpu.thread_new(memory).expect("thread");
    for page in [FIRST_INDEX, FIRST_INDEX + 1, 1 << 14] {
        let address = THREAD_PAGES as u64 + page * PAGE + control::ANSWER as u64;
        let mut registers = registers_at(STORE_AT, BY_B);
        registers.x[6] = address;
        let seen = run(&mut cpu, thread, &registers);
        assert_eq!(
            seen.exits,
            [Exit::MemoryFault {
                address,
                access: wcontainers_abi::Access::Write
            }],
            "control page {page}"
        );
        assert_eq!(seen.last.pc, STORE_AT, "at the store");
    }
    // The thread still takes turns: it runs on to its exit from the next routine.
    let seen = run(&mut cpu, thread, &registers_at(READ_AT, 0));
    assert!(
        matches!(seen.exits.last(), Some(Exit::Syscall(call)) if call.number.0 == u64::from(EXIT)),
        "{:?}",
        seen.exits
    );
}
