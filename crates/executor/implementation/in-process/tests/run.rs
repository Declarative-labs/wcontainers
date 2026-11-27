//! Form I's executions: translated code of the in-process way, run on this thread from a processor state, reads and
//! writes its own memory's window, makes its system calls through the handler, has its faults and every access outside
//! its window given to the handler, and takes every signal on the thread's own signal stack, whatever its stack pointer.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use wcontainers_abi::{Access, Permissions, SystemCallRequest};
use wcontainers_executor::memory_space::{MemorySpace, PageTable};
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_executor_in_process::execution::InProcess;
use wcontainers_executor_in_process::memory::{
    GUEST_PAGE, InProcessMemory, InProcessPageTable, InProcessPool,
};
use wcontainers_linux_process::LinuxProcess;
use wcontainers_linux_process_direct::DirectLinuxProcess;
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Exception, FaultStatus, Processor, ZERO_PROCESSOR,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// Where the guest's instructions are, its data page, a page it may only read, and its stack's page.
const CODE: u64 = 0x4_0000;
const DATA: u64 = 0x10_0000;
const READ_ONLY: u64 = 0x11_0000;
const STACK_PAGE: u64 = 0x20_0000;
/// A page inside the window the guest may do nothing with.
const NOTHING: u64 = 0x30_0000;
/// The guest's stack pointer, inside its page.
const STACK: u64 = STACK_PAGE + GUEST_PAGE / 2;
/// The guest's instructions ([Arm ARM, C6.2](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions)):
/// `ldr x1, [x2]`, `add x3, x1, #5`, `str x3, [x2, #8]`, `add x15, x24, #1`, `movz x8, #172`, `svc #0`.
const LOAD_X1: u32 = 0xf940_0041;
const ADD_X3: u32 = 0x9100_1423;
const STORE_X3: u32 = 0xf900_0443;
const ADD_X15: u32 = 0x9100_070f;
const MOVE_X8: u32 = 0xd280_1588;
const SYSTEM_CALL: u32 = 0xd400_0001;
const INSTRUCTIONS: [u32; 6] = [LOAD_X1, ADD_X3, STORE_X3, ADD_X15, MOVE_X8, SYSTEM_CALL];
/// `b .`, `br x1`, `ldr x0, [x1]`, `str x0, [x1]`, `mov sp, x1`, `mov sp, x2` and `ldr x0, [sp]`.
const BRANCH_TO_ITSELF: u32 = 0x1400_0000;
const STACK_FROM_X1: u32 = 0x9100_003f;
const STACK_FROM_X2: u32 = 0x9100_005f;
const LOAD_THROUGH_SP: u32 = 0xf940_03e0;
const BRANCH_TO_X1: u32 = 0xd61f_0020;
const LOAD_THROUGH_X1: u32 = 0xf940_0020;
const STORE_THROUGH_X1: u32 = 0xf900_0020;
/// What `add x3, x1, #5` and `add x15, x24, #1` add, `movz x8` moves, and where `str` writes past x2.
const ADDED: u64 = 5;
const ADDED_TO_BASE: u64 = 1;
const MOVED: u64 = 172;
const STORED_AT: u64 = 8;
/// The registers the instructions write, the one that holds the data page's address, and the ones the in-process way
/// keeps for itself between pieces.
const LOADED: usize = 1;
const SUM: usize = 3;
const NUMBER: usize = 8;
const ADDRESS: usize = 2;
const CHECK: usize = 15;
const BASE: usize = 24;
/// The register the access forms take their base from, and their index.
const BASE_FROM: usize = 1;
const INDEX_FROM: usize = 2;
/// What the first of two system calls returns.
const RETURNED: u64 = 0x1234;
/// Executions one after another on one thread.
const RUNS: u64 = 1000;
/// FPCR's rounding mode toward zero (RMode, bits 23 and 22), the guest's in the runs that keep the thread's own
/// ([FPCR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPCR--Floating-point-Control-Register)).
const ROUND_TOWARD_ZERO: u64 = 0b11 << 22;
/// A tenth, rounded to nearest: rounding toward zero makes it one unit in the last place smaller
/// ([binary64](https://en.wikipedia.org/wiki/Double-precision_floating-point_format)).
const TENTH_ROUNDED_TO_NEAREST: u64 = 0x3fb9_9999_9999_999a;
const TEN: f64 = 10.0;
/// A word of the other memory's data, which no access of the first may reach.
const OTHERS: u64 = 0x5a5a_5a5a_a5a5_a5a5;
/// Guest addresses past the window the outside test also reaches for: this window's guard, and this process's own
/// area far above every window.
const GUARD: u64 = (1 << 32) + GUEST_PAGE;
const FAR: u64 = 1 << 40;
/// The top byte of an address, which the guest's handler is given without.
const TAG: u64 = 0xff << 56;
/// The offset the sign-extended word index adds, and the forms' own offset.
const WORD_INDEX: i32 = -8;
const FORM_OFFSET: i64 = 8;
/// A pattern no signal frame leaves on the guest's stack page.
const PATTERN: u8 = 0xc3;

/// Every access form the translator checks, each a load or store of x0 (and x3) through x1 (and x2), with what x1 and
/// x2 must hold to reach a target, and whether it writes
/// ([Arm ARM, C6.2](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions)).
const FORMS: [(&str, u32, AddressFrom, bool); 18] = [
    ("ldr x0, [x1]", LOAD_THROUGH_X1, AddressFrom::Base(0), false),
    (
        "ldr x0, [x1, #8]",
        0xf940_0420,
        AddressFrom::Base(-FORM_OFFSET),
        false,
    ),
    (
        "ldur x0, [x1, #-8]",
        0xf85f_8020,
        AddressFrom::Base(FORM_OFFSET),
        false,
    ),
    (
        "ldr x0, [x1, x2]",
        0xf862_6820,
        AddressFrom::Index { scale: 0 },
        false,
    ),
    (
        "ldr x0, [x1, x2, lsl #3]",
        0xf862_7820,
        AddressFrom::Index { scale: 3 },
        false,
    ),
    (
        "ldr x0, [x1, w2, sxtw]",
        0xf862_c820,
        AddressFrom::NegativeWord,
        false,
    ),
    (
        "ldr x0, [x1, #8]!",
        0xf840_8c20,
        AddressFrom::Base(-FORM_OFFSET),
        false,
    ),
    ("ldr x0, [x1], #8", 0xf840_8420, AddressFrom::Base(0), false),
    ("ldp x0, x3, [x1]", 0xa940_0c20, AddressFrom::Base(0), false),
    ("ldrb w0, [x1]", 0x3940_0020, AddressFrom::Base(0), false),
    ("str x0, [x1]", STORE_THROUGH_X1, AddressFrom::Base(0), true),
    ("stp x0, x3, [x1]", 0xa900_0c20, AddressFrom::Base(0), true),
    (
        "ld1 {v0.16b}, [x1]",
        0x4c40_7020,
        AddressFrom::Base(0),
        false,
    ),
    (
        "st1 {v0.16b}, [x1]",
        0x4c00_7020,
        AddressFrom::Base(0),
        true,
    ),
    ("ldxr x0, [x1]", 0xc85f_7c20, AddressFrom::Base(0), false),
    ("stxr w4, x0, [x1]", 0xc804_7c20, AddressFrom::Base(0), true),
    ("ldar x0, [x1]", 0xc8df_fc20, AddressFrom::Base(0), false),
    (
        "ldr x0, [sp]",
        LOAD_THROUGH_SP,
        AddressFrom::StackPointer,
        false,
    ),
];

/// How an access form's registers reach its target.
#[derive(Clone, Copy, Debug)]
enum AddressFrom {
    /// x1 holds the target plus this offset.
    Base(i64),
    /// x1 holds zero and x2 the target shifted right by the scale.
    Index { scale: u32 },
    /// x1 holds the target less [`WORD_INDEX`] and w2 [`WORD_INDEX`].
    NegativeWord,
    /// The stack pointer holds the target.
    StackPointer,
}

/// What the handler was given, and how it answers.
#[derive(Default)]
struct Handler<'table> {
    calls: Vec<SystemCallRequest>,
    faults: Vec<(u64, Access)>,
    interrupts: usize,
    /// System calls answered with [`RETURNED`] before one is answered terminate.
    returning: usize,
    /// A table the handler gives the faulting page read and write permissions in, then answers retry.
    granting: Option<&'table mut InProcessPageTable>,
    /// Set at the first interrupt.
    interrupted: Option<&'table AtomicBool>,
}

impl TrapHandler for Handler<'_> {
    fn handle_system_call(&mut self, request: SystemCallRequest) -> SystemCallResponse {
        self.calls.push(request);
        if self.calls.len() <= self.returning {
            SystemCallResponse::Return(RETURNED)
        } else {
            SystemCallResponse::Terminate
        }
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        self.faults.push((address, access));
        match self.granting.take() {
            Some(table) => {
                let page = address & !(GUEST_PAGE - 1);
                table
                    .set_permissions(
                        page..page + GUEST_PAGE,
                        Permissions::READ | Permissions::WRITE,
                    )
                    .expect("inside the window");
                MemoryFaultResponse::Retry
            }
            None => MemoryFaultResponse::Terminate,
        }
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.interrupts += 1;
        if let Some(interrupted) = self.interrupted {
            interrupted.store(true, Ordering::Release);
        }
        InterruptResponse::Terminate
    }
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

/// A memory of the executor with `instructions` at [`CODE`], its data page holding `word`, a page it may only read
/// and its stack's page.
fn memory(
    executor: &InProcess,
    instructions: &[u32],
    word: u64,
) -> (InProcessMemory, InProcessPageTable) {
    let (memory, mut table) = executor.create_guest_memory().expect("a free slot");
    let code: Vec<u8> = instructions.iter().flat_map(|w| w.to_le_bytes()).collect();
    memory
        .borrow(CODE..CODE + code.len() as u64)
        .expect("inside the window")
        .write(&code);
    for (page, allowed) in [
        (CODE, Permissions::READ | Permissions::EXECUTE),
        (DATA, Permissions::READ | Permissions::WRITE),
        (READ_ONLY, Permissions::READ),
        (STACK_PAGE, Permissions::READ | Permissions::WRITE),
    ] {
        table
            .set_permissions(page..page + GUEST_PAGE, allowed)
            .expect("inside the window");
    }
    write_word(&memory, DATA, word);
    (memory, table)
}

/// The guest's registers before: each its own value, x2 the data page's address.
fn before(seed: u64) -> Processor<()> {
    let mut processor = ZERO_PROCESSOR;
    for (r, value) in processor.general_purpose_registers.iter_mut().enumerate() {
        *value = seed
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .rotate_left(r as u32)
            | 1;
    }
    processor.general_purpose_registers[ADDRESS] = DATA;
    processor.stack_pointer = STACK;
    processor.program_counter = CODE;
    processor
}

/// Runs an execution of the memory from `processor` to its end.
fn execute(
    executor: &mut InProcess,
    memory: &InProcessMemory,
    processor: Processor<()>,
    handler: &mut Handler<'_>,
) -> Processor<()> {
    let (execution, _interrupter) = executor.prepare_execution(processor, memory);
    match execution.run(handler) {
        ExecutionStop::Terminated(processor) => processor,
        ExecutionStop::Waiting { .. } => panic!("form I never waits"),
    }
}

/// The guest's registers after the instructions, from `before` and the word its data page held.
fn after(before: &Processor<()>, word: u64) -> [u64; 31] {
    let mut x = before.general_purpose_registers;
    x[LOADED] = word;
    x[SUM] = word + ADDED;
    x[CHECK] = before.general_purpose_registers[BASE] + ADDED_TO_BASE;
    x[NUMBER] = MOVED;
    x
}

/// Checks the execution ended at the system call after the instructions with the registers they leave.
fn assert_system_call(ended: &Processor<()>, before: &Processor<()>, word: u64) {
    assert_eq!(
        ended.exception,
        Some(Exception::SupervisorCall { immediate: 0 }),
        "{ended:x?}"
    );
    let wanted = after(before, word);
    let differing: Vec<usize> = (0..wanted.len())
        .filter(|&r| ended.general_purpose_registers[r] != wanted[r])
        .collect();
    assert!(
        differing.is_empty(),
        "registers {differing:?} differ: {ended:x?}"
    );
    assert_eq!(
        ended.program_counter,
        CODE + (INSTRUCTIONS.len() * size_of::<u32>()) as u64
    );
    assert_eq!(ended.stack_pointer, before.stack_pointer);
}

fn executor() -> InProcess {
    InProcess::new(InProcessPool::new().expect("a host with the guest's pages"))
}

#[test]
fn translated_code_reads_and_writes_its_window_and_makes_its_system_call() {
    let mut executor = executor();
    let word = 0x0123_4567_89ab_cdef;
    let (memory, _table) = memory(&executor, &INSTRUCTIONS, word);
    let start = before(1);
    let mut handler = Handler::default();
    let ended = execute(&mut executor, &memory, start.clone(), &mut handler);
    assert_system_call(&ended, &start, word);
    assert_eq!(handler.calls.len(), 1);
    assert_eq!(handler.calls[0].number.0, MOVED);
    assert_eq!(read_word(&memory, DATA + STORED_AT), word + ADDED);
    assert_eq!(read_word(&memory, DATA), word);
}

#[test]
fn a_system_call_s_result_is_the_guest_s_and_it_goes_on() {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &[SYSTEM_CALL, SYSTEM_CALL], 0);
    let mut handler = Handler {
        returning: 1,
        ..Handler::default()
    };
    let ended = execute(&mut executor, &memory, before(2), &mut handler);
    assert_eq!(handler.calls.len(), 2);
    assert_eq!(handler.calls[1].arguments[0], RETURNED);
    assert_eq!(ended.general_purpose_registers[0], RETURNED);
    assert_eq!(ended.program_counter, CODE + 2 * size_of::<u32>() as u64);
}

#[test]
fn two_memories_run_apart_each_in_its_own_window() {
    let mut executor = executor();
    let words = [0x1111_2222_3333_4444, 0x5555_6666_7777_8888];
    let memories = words.map(|word| memory(&executor, &INSTRUCTIONS, word));
    for k in [0, 1, 0] {
        let (memory, _) = &memories[k];
        write_word(memory, DATA + STORED_AT, 0);
        let start = before(k as u64 + 2);
        let ended = execute(
            &mut executor,
            memory,
            start.clone(),
            &mut Handler::default(),
        );
        assert_system_call(&ended, &start, words[k]);
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

/// A tenth, computed with this thread's rounding mode.
fn tenth() -> f64 {
    std::hint::black_box(1.0_f64) / std::hint::black_box(TEN)
}

#[test]
fn a_thousand_executions_leave_the_thread_its_own() {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &INSTRUCTIONS, 0);
    let own = 0x0bad_cafe;
    let guest_s = 0x0dec_afe0;
    OWN.set(own);
    let control = DirectLinuxProcess.read_fpcr();
    assert_eq!(tenth().to_bits(), TENTH_ROUNDED_TO_NEAREST);
    for run_number in 0..RUNS {
        write_word(&memory, DATA, run_number);
        let start = Processor {
            floating_point_control: ROUND_TOWARD_ZERO,
            software_thread_id: guest_s,
            ..before(run_number)
        };
        let ended = execute(
            &mut executor,
            &memory,
            start.clone(),
            &mut Handler::default(),
        );
        assert_system_call(&ended, &start, run_number);
        assert_eq!(
            ended.software_thread_id, guest_s,
            "the guest's thread pointer"
        );
        assert_eq!(
            ended.floating_point_control, ROUND_TOWARD_ZERO,
            "the guest's rounding"
        );
    }
    assert_eq!(OWN.get(), own, "the thread's own thread-local");
    assert_eq!(
        DirectLinuxProcess.read_fpcr(),
        control,
        "the thread's control"
    );
    assert_eq!(
        tenth().to_bits(),
        TENTH_ROUNDED_TO_NEAREST,
        "the thread's rounding"
    );
}

#[test]
fn a_guest_fault_reaches_the_handler_which_ends_the_execution_or_has_it_retried() {
    let mut executor = executor();
    let cases = [
        (
            LOAD_THROUGH_X1,
            NOTHING,
            Access::Read,
            FaultStatus::TranslationFaultLevel3,
        ),
        (
            STORE_THROUGH_X1,
            READ_ONLY,
            Access::Write,
            FaultStatus::PermissionFaultLevel3,
        ),
        (
            BRANCH_TO_X1,
            DATA,
            Access::Execute,
            FaultStatus::PermissionFaultLevel3,
        ),
    ];
    for (instruction, address, access, status) in cases {
        let (memory, _table) = memory(&executor, &[instruction, SYSTEM_CALL], 0);
        let mut start = before(3);
        start.general_purpose_registers[BASE_FROM] = address;
        let mut handler = Handler::default();
        let ended = execute(&mut executor, &memory, start, &mut handler);
        assert_eq!(handler.faults, [(address, access)], "{instruction:#010x}");
        let wanted = match access {
            Access::Execute => Exception::InstructionAbort { status, address },
            Access::Read => Exception::DataAbort {
                status,
                access: AccessDirection::Read,
                address,
            },
            Access::Write => Exception::DataAbort {
                status,
                access: AccessDirection::Write,
                address,
            },
        };
        assert_eq!(ended.exception, Some(wanted));
        let at = if access == Access::Execute {
            address
        } else {
            CODE
        };
        assert_eq!(ended.program_counter, at);
    }
    // Retried once the handler allows the page: the load runs again and the system call follows.
    let (memory, mut table) = memory(&executor, &[LOAD_THROUGH_X1, SYSTEM_CALL], 0);
    write_word(&memory, NOTHING, OTHERS);
    let mut start = before(4);
    start.general_purpose_registers[BASE_FROM] = NOTHING;
    let mut handler = Handler {
        granting: Some(&mut table),
        ..Handler::default()
    };
    let ended = execute(&mut executor, &memory, start, &mut handler);
    assert_eq!(handler.faults, [(NOTHING, Access::Read)]);
    assert_eq!(
        ended.exception,
        Some(Exception::SupervisorCall { immediate: 0 })
    );
    assert_eq!(ended.general_purpose_registers[0], OTHERS);
}

#[test]
fn every_access_form_that_tries_to_reach_outside_its_window_is_stopped() {
    let mut executor = executor();
    let (other, _other_table) = memory(&executor, &[], OTHERS);
    let mut stopped = 0;
    for (form, instruction, from, writes) in FORMS {
        let (memory, _table) = memory(&executor, &[instruction, SYSTEM_CALL], 0);
        // The other memory's data, by its host address less this window's base; this window's guard; far away; and
        // below the window, wrapped around.
        let into_other = (other.window_base() + DATA).wrapping_sub(memory.window_base());
        for target in [into_other, GUARD, FAR, 0_u64.wrapping_sub(GUEST_PAGE)] {
            let mut start = before(5);
            let x = &mut start.general_purpose_registers;
            match from {
                AddressFrom::Base(offset) => x[BASE_FROM] = target.wrapping_add_signed(offset),
                AddressFrom::Index { scale } => {
                    x[BASE_FROM] = target & ((1 << scale) - 1);
                    x[INDEX_FROM] = target >> scale;
                }
                AddressFrom::NegativeWord => {
                    x[BASE_FROM] = target.wrapping_sub(i64::from(WORD_INDEX) as u64);
                    x[INDEX_FROM] = u64::from(WORD_INDEX as u32);
                }
                AddressFrom::StackPointer => start.stack_pointer = target,
            }
            let mut handler = Handler::default();
            let ended = execute(&mut executor, &memory, start, &mut handler);
            assert_eq!(
                handler.calls,
                [],
                "{form} at {target:#x} went on: {ended:x?}"
            );
            assert_eq!(
                handler.faults.len(),
                1,
                "{form} at {target:#x}: {:x?}",
                handler.faults
            );
            let (address, access) = handler.faults[0];
            assert_eq!(address, target & !TAG, "{form} at {target:#x}");
            if writes {
                assert_eq!(access, Access::Write, "{form} at {target:#x}");
            }
            assert_eq!(ended.program_counter, CODE, "{form} at {target:#x}");
            assert_eq!(
                read_word(&other, DATA),
                OTHERS,
                "{form} reached the other memory"
            );
            stopped += 1;
        }
    }
    eprintln!("accesses outside the window stopped: {stopped}");
}

#[test]
fn signals_while_guest_code_runs_are_taken_on_the_thread_s_own_stack_whatever_the_guest_s() {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &[BRANCH_TO_ITSELF], 0);
    let pattern = vec![PATTERN; GUEST_PAGE as usize];
    memory
        .borrow(STACK_PAGE..STACK_PAGE + GUEST_PAGE)
        .expect("inside the window")
        .write(&pattern);
    // A stack pointer into the guest's own writable page, one into a page it may not touch, and one outside its window.
    for stack in [STACK, NOTHING + GUEST_PAGE / 2, FAR] {
        let interrupted = AtomicBool::new(false);
        let start = Processor {
            stack_pointer: stack,
            ..before(6)
        };
        let (execution, interrupter) = executor.prepare_execution(start, &memory);
        let interrupter = Arc::new(interrupter);
        let (stop, interrupts) = std::thread::scope(|scope| {
            let sender = Arc::clone(&interrupter);
            let interrupted = &interrupted;
            scope.spawn(move || {
                while !interrupted.load(Ordering::Acquire) {
                    sender.interrupt();
                    std::thread::yield_now();
                }
            });
            let mut handler = Handler {
                interrupted: Some(interrupted),
                ..Handler::default()
            };
            let stop = execution.run(&mut handler);
            interrupted.store(true, Ordering::Release);
            (stop, handler.interrupts)
        });
        let ExecutionStop::Terminated(ended) = stop else {
            panic!("form I never waits");
        };
        assert_eq!(interrupts, 1);
        assert_eq!(ended.exception, None, "{ended:x?}");
        assert_eq!(ended.program_counter, CODE);
        assert_eq!(ended.stack_pointer, stack);
        let mut seen = vec![0; GUEST_PAGE as usize];
        memory
            .borrow(STACK_PAGE..STACK_PAGE + GUEST_PAGE)
            .expect("inside the window")
            .read(&mut seen);
        assert!(
            seen == pattern,
            "a signal frame on the guest's stack at {stack:#x}"
        );
    }
}

#[test]
fn a_stack_pointer_moved_out_of_the_window_and_back_is_the_guest_s_and_reaches_nothing() {
    let mut executor = executor();
    let word = 0x7777_0000_1111_2222;
    // Out, then an access through it, which faults where Linux would.
    let (out, _out_table) = memory(&executor, &[STACK_FROM_X1, LOAD_THROUGH_SP, SYSTEM_CALL], 0);
    let mut start = before(7);
    start.general_purpose_registers[BASE_FROM] = FAR;
    let mut handler = Handler::default();
    let ended = execute(&mut executor, &out, start, &mut handler);
    assert_eq!(handler.faults, [(FAR, Access::Read)]);
    assert_eq!(ended.stack_pointer, FAR);
    assert_eq!(ended.program_counter, CODE + size_of::<u32>() as u64);
    // Out and back in: the access through it reads the window.
    let (back, _back_table) = memory(
        &executor,
        &[STACK_FROM_X1, STACK_FROM_X2, LOAD_THROUGH_SP, SYSTEM_CALL],
        0,
    );
    write_word(&back, STACK, word);
    let mut start = before(8);
    start.general_purpose_registers[BASE_FROM] = FAR;
    start.general_purpose_registers[INDEX_FROM] = STACK;
    let mut handler = Handler::default();
    let ended = execute(&mut executor, &back, start, &mut handler);
    assert_eq!(handler.faults, []);
    assert_eq!(handler.calls.len(), 1);
    assert_eq!(ended.stack_pointer, STACK);
    assert_eq!(ended.general_purpose_registers[0], word);
}

#[test]
fn an_interrupt_asked_before_the_execution_runs_is_taken_first() {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &[BRANCH_TO_ITSELF], 0);
    let (execution, interrupter) = executor.prepare_execution(before(9), &memory);
    interrupter.interrupt();
    let mut handler = Handler::default();
    let ExecutionStop::Terminated(ended) = execution.run(&mut handler) else {
        panic!("form I never waits");
    };
    assert_eq!(handler.interrupts, 1);
    assert_eq!(ended.program_counter, CODE);
    assert_eq!(ended.exception, None);
}
