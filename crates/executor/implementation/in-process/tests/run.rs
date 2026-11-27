//! Form I's runs: translated code of the in-process way, run on a thread of this process from a guest state, reads
//! and writes its own memory's window, keeps the guest's values of the way's own registers, and calls the floor, whose
//! call the stop crate resolves to the guest's system call with the state the guest's instructions leave.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::cell::Cell;
use std::sync::Arc;

use wcontainers_abi::{PAGE_SIZE, Permissions};
use wcontainers_arm64::thread_state::{PAGE, RegisterPage};
use wcontainers_arm64::way::{BASE, CHECK, Kept};
use wcontainers_arm64::{Arm64, Way};
use wcontainers_executor::memory_space::{MemorySpace, PageTable};
use wcontainers_executor_arm64_stop::in_process::{self, Resolution};
use wcontainers_executor_arm64_stop::{
    FloatStatus, GENERAL, GuestState, Piece, Resolved, Stop, Taken,
};
use wcontainers_executor_in_process::memory::{InProcessMemory, InProcessPageTable, InProcessPool};
use wcontainers_executor_in_process::run::{Exit, InProcessThread};
use wcontainers_guest_cpu::HostCode;
use wcontainers_linux_process::LinuxProcess;
use wcontainers_linux_process_direct::{DirectCodeMapping, DirectLinuxProcess};
use wcontainers_translator::{LookAhead, translate_native_for};
use wcontainers_volatile_bytes::VolatileBytes;

/// Where the guest's instructions are, and its data page.
const CODE: u64 = 0x4_0000;
const DATA: u64 = 0x10_0000;
/// The guest's stack pointer, inside the window.
const STACK: u64 = 0x20_0000;
/// The guest's instructions (Arm ARM, C6.2): `ldr x1, [x2]`, `add x3, x1, #5`, `str x3, [x2, #8]`,
/// `add x15, x24, #1`, `movz x8, #172`, `svc #0`.
const LOAD_X1: u32 = 0xf940_0041;
const ADD_X3: u32 = 0x9100_1423;
const STORE_X3: u32 = 0xf900_0443;
const ADD_X15: u32 = 0x9100_070f;
const MOVE_X8: u32 = 0xd280_1588;
const SYSTEM_CALL: u32 = 0xd400_0001;
const INSTRUCTIONS: [u32; 6] = [LOAD_X1, ADD_X3, STORE_X3, ADD_X15, MOVE_X8, SYSTEM_CALL];
/// What `add x3, x1, #5` and `add x15, x24, #1` add, `movz x8` moves, and where `str` writes past x2.
const ADDED: u64 = 5;
const ADDED_TO_BASE: u64 = 1;
const MOVED: u64 = 172;
const STORED_AT: u64 = 8;
/// The registers the instructions write, and the one that holds the data page's address.
const LOADED: usize = 1;
const SUM: usize = 3;
const NUMBER: usize = 8;
const ADDRESS: usize = 2;
/// The count as a run starts.
const COUNT_START: u64 = 1 << 40;
/// Where each test puts its translated code in this process: apart from every other test's.
const CODE_AREA: u64 = (1 << 45) + (1 << 40);
const CODE_ROOM: u64 = 1 << 24;
/// Runs one after another on one thread.
const RUNS: u64 = 1000;

fn way(memory: &InProcessMemory) -> Way {
    Way::InProcess {
        slot: memory.slot(),
        displaced: false,
        kept: Kept::USUAL,
    }
}

/// A memory whose data page holds `word` and allows reading and writing.
fn memory(pool: &Arc<InProcessPool>, word: u64) -> (InProcessMemory, InProcessPageTable) {
    let (memory, mut table) = pool.create_memory().expect("a free slot");
    table
        .set_permissions(
            DATA..DATA + PAGE_SIZE,
            Permissions::READ | Permissions::WRITE,
        )
        .expect("inside the window");
    write_word(&memory, DATA, word);
    (memory, table)
}

fn write_word(memory: &InProcessMemory, at: u64, word: u64) {
    let bytes = memory.borrow(at..at + 8).expect("inside the window");
    assert_eq!(bytes.write(&word.to_le_bytes()), 8);
}

fn read_word(memory: &InProcessMemory, at: u64) -> u64 {
    let mut word = [0; 8];
    let bytes = memory.borrow(at..at + 8).expect("inside the window");
    assert_eq!(bytes.read(&mut word), 8);
    u64::from_le_bytes(word)
}

/// The instructions translated for the memory's way, and mapped as code at the `room`th place of the test's area.
fn translated(
    memory: &InProcessMemory,
    area: u64,
    room: u64,
) -> (HostCode, DirectCodeMapping, u64) {
    let bytes: Vec<u8> = INSTRUCTIONS.iter().flat_map(|w| w.to_le_bytes()).collect();
    let code =
        translate_native_for::<Arm64>(&bytes, CODE, LookAhead::Full, true, &way(memory), &())
            .expect("translated");
    let mut pages = code.bytes.clone();
    pages.resize(code.bytes.len().next_multiple_of(PAGE_SIZE as usize), 0);
    let at = CODE_AREA + area * CODE_ROOM + room * PAGE_SIZE * 4;
    let mapping = DirectLinuxProcess
        .map_code(at as usize, &pages)
        .expect("the code is mapped");
    (code, mapping, at)
}

/// The guest's registers before: each its own value, x2 the data page's address.
fn before(seed: u64) -> GuestState {
    let mut x = [0; GENERAL];
    for (r, value) in x.iter_mut().enumerate() {
        *value = seed
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .rotate_left(r as u32)
            | 1;
    }
    x[ADDRESS] = DATA;
    GuestState {
        x,
        sp: STACK,
        pc: CODE,
        flags: 0,
        count: COUNT_START,
        exclusive: [0; 4],
    }
}

/// Runs the translated code from `state` and resolves its call into the floor.
fn run(
    thread: &mut InProcessThread,
    memory: &InProcessMemory,
    code: &HostCode,
    at: u64,
    state: &GuestState,
) -> Resolution {
    let way = way(memory);
    let float = FloatStatus::default();
    thread.enter(way, state, RegisterPage::default(), &float, at);
    let Exit::Call(call) = thread.run() else {
        panic!("the run ended at no entry of the floor");
    };
    let piece = Piece {
        base: at,
        length: code.bytes.len(),
        spans: &code.spans,
    };
    in_process::resolve(
        way,
        &piece,
        Taken::Call(call),
        &thread.hardware(),
        &thread.page(),
        &thread.after(),
        &mut FloatStatus::default(),
        &mut |address, into| {
            let bytes = memory
                .borrow(address..address + into.len() as u64)
                .expect("inside the window");
            bytes.read(into);
        },
    )
    .expect("the call resolves")
}

/// The guest's state after the instructions, from `before` and the word its data page held.
fn after(before: &GuestState, word: u64) -> GuestState {
    let mut x = before.x;
    x[LOADED] = word;
    x[SUM] = word + ADDED;
    x[usize::from(CHECK)] = before.x[usize::from(BASE)] + ADDED_TO_BASE;
    x[NUMBER] = MOVED;
    GuestState {
        x,
        pc: CODE + (INSTRUCTIONS.len() * size_of::<u32>()) as u64,
        ..before.clone()
    }
}

fn assert_system_call(resolution: Resolution, wanted: &GuestState) {
    let Resolution::Resolved(Resolved::Stop(Stop::Syscall, state)) = resolution else {
        panic!("resolved to {resolution:?}");
    };
    let differing: Vec<usize> = (0..GENERAL)
        .filter(|&r| state.x[r] != wanted.x[r])
        .collect();
    assert!(
        differing.is_empty(),
        "registers {differing:?} differ: {state:x?}, wanted {wanted:x?}"
    );
    assert_eq!(state.pc, wanted.pc);
    assert_eq!(state.sp, wanted.sp);
}

#[test]
fn translated_code_reads_and_writes_its_window_and_calls_the_floor() {
    let pool = InProcessPool::new().expect("a host with the guest's pages");
    let word = 0x0123_4567_89ab_cdef;
    let (memory, _table) = memory(&pool, word);
    let (code, _mapping, at) = translated(&memory, 0, 0);
    let mut thread = InProcessThread::new().expect("the floor is mapped");
    let state = before(1);
    let resolution = run(&mut thread, &memory, &code, at, &state);
    assert_system_call(resolution, &after(&state, word));
    assert_eq!(read_word(&memory, DATA + STORED_AT), word + ADDED);
    assert_eq!(read_word(&memory, DATA), word);
    // The frame's address was the code's x28, which the guest never sees.
    assert_eq!(
        thread.hardware().x[usize::from(PAGE)],
        thread.register_page()
    );
}

#[test]
fn two_memories_run_apart_each_in_its_own_window() {
    let pool = InProcessPool::new().expect("a host with the guest's pages");
    let words = [0x1111_2222_3333_4444, 0x5555_6666_7777_8888];
    let memories = words.map(|word| memory(&pool, word));
    let codes: Vec<_> = (0..2)
        .map(|k| translated(&memories[k].0, 1, k as u64))
        .collect();
    let mut thread = InProcessThread::new().expect("the floor is mapped");
    for k in [0, 1, 0] {
        let (memory, _) = &memories[k];
        let (code, _, at) = &codes[k];
        let state = before(k as u64 + 2);
        write_word(memory, DATA + STORED_AT, 0);
        let resolution = run(&mut thread, memory, code, *at, &state);
        assert_system_call(resolution, &after(&state, words[k]));
        assert_eq!(read_word(memory, DATA + STORED_AT), words[k] + ADDED);
        let (other, _) = &memories[1 - k];
        assert_eq!(
            read_word(other, DATA),
            words[1 - k],
            "the other memory's data"
        );
    }
}

thread_local! {
    static OWN: Cell<u64> = const { Cell::new(0) };
}

#[test]
fn a_thousand_runs_leave_the_thread_its_own() {
    let pool = InProcessPool::new().expect("a host with the guest's pages");
    let (memory, _table) = memory(&pool, 0);
    let (code, _mapping, at) = translated(&memory, 2, 0);
    let mut thread = InProcessThread::new().expect("the floor is mapped");
    let own = 0x0bad_cafe;
    OWN.set(own);
    thread.set_thread_pointer(own);
    let third = std::hint::black_box(1.0_f64) / std::hint::black_box(3.0_f64);
    for run_number in 0..RUNS {
        write_word(&memory, DATA, run_number);
        let state = before(run_number);
        let resolution = run(&mut thread, &memory, &code, at, &state);
        assert_system_call(resolution, &after(&state, run_number));
        assert_eq!(read_word(&memory, DATA + STORED_AT), run_number + ADDED);
    }
    assert_eq!(OWN.get(), own, "the thread's own thread-local");
    assert_eq!(thread.thread_pointer(), own, "the guest's thread pointer");
    assert_eq!(
        std::hint::black_box(1.0_f64) / std::hint::black_box(3.0_f64),
        third,
        "the thread's rounding"
    );
}
