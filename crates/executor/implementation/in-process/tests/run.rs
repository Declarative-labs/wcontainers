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
    AccessDirection, ConditionFlags, Exception, FaultStatus, Processor, SystemAccess,
    ZERO_PROCESSOR,
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
    /// Set at the first interrupt, which is then answered terminate; without it every interrupt, an end of a slice
    /// among them, is answered continue.
    interrupted: Option<&'table AtomicBool>,
    /// Interrupts answered continue before one is answered terminate, where given.
    interrupts_before_end: Option<usize>,
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
        if let Some(before_end) = self.interrupts_before_end {
            return if self.interrupts > before_end {
                InterruptResponse::Terminate
            } else {
                InterruptResponse::Continue
            };
        }
        match self.interrupted {
            Some(interrupted) => {
                interrupted.store(true, Ordering::Release);
                InterruptResponse::Terminate
            }
            None => InterruptResponse::Continue,
        }
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
    let interrupted = AtomicBool::new(false);
    let mut handler = Handler {
        interrupted: Some(&interrupted),
        ..Handler::default()
    };
    let ExecutionStop::Terminated(ended) = execution.run(&mut handler) else {
        panic!("form I never waits");
    };
    assert_eq!(handler.interrupts, 1);
    assert_eq!(ended.program_counter, CODE);
    assert_eq!(ended.exception, None);
}

/// `svc #0` then `b .-4`: a guest that makes system call after system call, so that every slice has a trap.
const BRANCH_BACK: u32 = 0x17ff_ffff;
/// System calls the guest may make before the handler ends the execution.
const MANY_CALLS: usize = 2000;

#[test]
fn an_interrupt_asked_before_the_execution_is_taken_first_by_a_guest_that_keeps_making_system_calls()
 {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &[SYSTEM_CALL, BRANCH_BACK], 0);
    // Its pieces translated first, so that no slice ends in a translation before its first system call.
    execute(&mut executor, &memory, before(10), &mut Handler::default());
    let (execution, interrupter) = executor.prepare_execution(before(10), &memory);
    interrupter.interrupt();
    let interrupted = AtomicBool::new(false);
    let mut handler = Handler {
        returning: MANY_CALLS,
        interrupted: Some(&interrupted),
        ..Handler::default()
    };
    let ExecutionStop::Terminated(ended) = execution.run(&mut handler) else {
        panic!("form I never waits");
    };
    assert_eq!((handler.calls.len(), handler.interrupts), (0, 1));
    assert_eq!(ended.program_counter, CODE);
    assert_eq!(ended.exception, None);
}

/// `ldadd x0, x3, [x1]`, `dc zva, x1` and `ic ivau, x1`
/// ([Arm ARM, C6.2](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions)).
const ATOMIC_ADD: u32 = 0xf820_0023;
const ZERO_BLOCK: u32 = 0xd50b_7421;
const CODE_CHANGED: u32 = 0xd50b_7521;

/// Runs `instruction` then a system call with x1 at `address`, and returns the faults the handler was given and the
/// execution's end.
fn fault_of(
    executor: &mut InProcess,
    instruction: u32,
    address: u64,
) -> (Vec<(u64, Access)>, Processor<()>) {
    let (memory, _table) = memory(executor, &[instruction, SYSTEM_CALL], 0);
    let mut start = before(11);
    start.general_purpose_registers[BASE_FROM] = address;
    let mut handler = Handler::default();
    let ended = execute(executor, &memory, start, &mut handler);
    (handler.faults, ended)
}

/// Linux reports an atomic access and `dc zva` as writes (the abort's WnR bit, arch/arm64/mm/fault.c
/// `is_write_abort`) and a cache maintenance by address that faults as a read (its CM bit), whether the page refuses
/// it or the address is outside the window.
#[test]
fn a_fault_is_a_write_or_a_read_as_linux_reports_it() {
    let mut executor = executor();
    let write = |address| {
        (
            vec![(address, Access::Write)],
            Some(Exception::DataAbort {
                status: if address == READ_ONLY {
                    FaultStatus::PermissionFaultLevel3
                } else {
                    FaultStatus::TranslationFaultLevel3
                },
                access: AccessDirection::Write,
                address,
            }),
        )
    };
    for (name, instruction, address, wanted) in [
        ("ldadd", ATOMIC_ADD, READ_ONLY, write(READ_ONLY)),
        ("dc zva", ZERO_BLOCK, READ_ONLY, write(READ_ONLY)),
        ("dc zva", ZERO_BLOCK, GUARD, write(GUARD)),
        ("ldadd", ATOMIC_ADD, GUARD, write(GUARD)),
        (
            "ic ivau",
            CODE_CHANGED,
            NOTHING,
            (
                vec![(NOTHING, Access::Read)],
                Some(Exception::DataAbort {
                    status: FaultStatus::TranslationFaultLevel3,
                    access: AccessDirection::Read,
                    address: NOTHING,
                }),
            ),
        ),
    ] {
        let (faults, ended) = fault_of(&mut executor, instruction, address);
        assert_eq!((faults, ended.exception), wanted, "{name} at {address:#x}");
        assert_eq!(ended.program_counter, CODE, "{name} at {address:#x}");
    }
    // On a page the guest may read, a code change is announced and the guest goes on to its system call.
    for page in [READ_ONLY, DATA] {
        let (faults, ended) = fault_of(&mut executor, CODE_CHANGED, page);
        assert_eq!(faults, [], "ic ivau at {page:#x}");
        assert_eq!(
            ended.exception,
            Some(Exception::SupervisorCall { immediate: 0 }),
            "ic ivau at {page:#x}"
        );
    }
}

/// System calls the guest makes while interrupts land: enough that some land in its calls into the floor.
const STORM_CALLS: usize = 5 * MANY_CALLS;

/// Interrupts that land anywhere while a guest makes system call after system call, in a call into the floor too, are
/// each answered continue and change nothing: every call is made with what the last one returned, and the execution
/// ends at the one the handler ends.
#[test]
fn interrupts_that_land_anywhere_change_nothing_the_guest_sees() {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &[SYSTEM_CALL, BRANCH_BACK], 0);
    let start = before(12);
    let (execution, interrupter) = executor.prepare_execution(start.clone(), &memory);
    let done = AtomicBool::new(false);
    let (stop, handler) = std::thread::scope(|scope| {
        let done = &done;
        let interrupter = &interrupter;
        scope.spawn(move || {
            while !done.load(Ordering::Acquire) {
                interrupter.interrupt();
                std::thread::yield_now();
            }
        });
        let mut handler = Handler {
            returning: STORM_CALLS,
            ..Handler::default()
        };
        let stop = execution.run(&mut handler);
        done.store(true, Ordering::Release);
        (stop, handler)
    });
    let ExecutionStop::Terminated(ended) = stop else {
        panic!("form I never waits");
    };
    eprintln!("interrupts answered: {}", handler.interrupts);
    assert_eq!(
        ended.exception,
        Some(Exception::SupervisorCall { immediate: 0 }),
        "{ended:x?}"
    );
    assert_eq!(handler.calls.len(), STORM_CALLS + 1);
    assert_eq!(
        handler.calls[0].arguments[0],
        start.general_purpose_registers[0]
    );
    assert!(
        handler.calls[1..]
            .iter()
            .all(|call| call.arguments[0] == RETURNED)
    );
    assert_eq!(ended.program_counter, CODE + size_of::<u32>() as u64);
    assert_eq!(ended.general_purpose_registers[0], RETURNED);
    assert!(handler.interrupts > 0, "no interrupt came");
}

/// The slice after which an execution that took no trap is interrupted: a tick at 1000 Hz.
const SLICE: std::time::Duration = std::time::Duration::from_millis(1);
/// Interrupts a guest that never traps is given before the handler ends it.
const SLICES: usize = 20;

/// A guest that never traps is interrupted at the end of each slice, as at a tick from outside, and goes on when the
/// handler answers continue; one that traps in every slice is not.
#[test]
fn a_guest_is_interrupted_after_each_slice_in_which_it_took_no_trap() {
    let mut executor = executor();
    let (spinning, _spinning_table) = memory(&executor, &[BRANCH_TO_ITSELF], 0);
    let mut handler = Handler {
        interrupts_before_end: Some(SLICES),
        ..Handler::default()
    };
    let started = std::time::Instant::now();
    let ended = execute(&mut executor, &spinning, before(13), &mut handler);
    let took = started.elapsed();
    assert_eq!(handler.interrupts, SLICES + 1);
    assert!(took >= SLICE * (SLICES + 1) as u32, "{took:?}");
    assert_eq!((ended.program_counter, ended.exception), (CODE, None));

    let (calling, _calling_table) = memory(&executor, &[SYSTEM_CALL, BRANCH_BACK], 0);
    let mut handler = Handler {
        returning: MANY_CALLS * 10,
        ..Handler::default()
    };
    let started = std::time::Instant::now();
    let ended = execute(&mut executor, &calling, before(14), &mut handler);
    let slices = started.elapsed().as_micros() / SLICE.as_micros();
    assert_eq!(handler.calls.len(), MANY_CALLS * 10 + 1);
    assert_eq!(
        ended.exception,
        Some(Exception::SupervisorCall { immediate: 0 })
    );
    // A slice the host took from the guest's thread may end with no trap; most do not.
    assert!(
        (handler.interrupts as u128) * 4 < slices.max(1),
        "{} interrupts in {slices} slices",
        handler.interrupts
    );
}

/// An execution on a thread that already runs one is refused: it ends at once with an SError
/// ([ESR_EL1.EC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-)
/// 0b101111, in bits 31 to 26 of the syndrome), the interface having no other way to say so, and the guest's state as
/// it was.
#[test]
fn an_execution_on_a_thread_that_already_runs_one_ends_in_an_serror() {
    const SERROR: u64 = 0b10_1111;
    const EXCEPTION_CLASS_SHIFT: u32 = 26;
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &INSTRUCTIONS, 0);
    let held = DirectLinuxProcess
        .prepare_run_thread()
        .expect("this thread is prepared");
    let start = before(15);
    let mut handler = Handler::default();
    let ended = execute(&mut executor, &memory, start.clone(), &mut handler);
    drop(held);
    assert_eq!(
        ended,
        Processor {
            exception: Some(Exception::Undecoded {
                syndrome: SERROR << EXCEPTION_CLASS_SHIFT,
                fault_address: 0,
            }),
            ..start
        }
    );
    assert!(handler.calls.is_empty());
}

/// `svc #0x1234`, `subs x0, x1, x1`, `dup v1.2d, x5`, `msr fpsr, x6`, `nop`, `mrs x0, cntvct_el0`
/// ([Arm ARM, C6.2 and C7.2](https://developer.arm.com/documentation/ddi0602/latest)).
const SYSTEM_CALL_1234: u32 = 0xd402_4681;
const IMMEDIATE: u16 = 0x1234;
const SUBTRACT_FLAGS: u32 = 0xeb01_0020;
const DUPLICATE_X5: u32 = 0x4e08_0ca1;
const WRITE_FPSR: u32 = 0xd51b_4426;
const NOTHING_DONE: u32 = 0xd503_201f;
const READ_COUNTER: u32 = 0xd53b_e040;
/// The registers those read and write.
const DUPLICATED: usize = 5;
const DUPLICATED_INTO: usize = 1;
const STATUS_FROM: usize = 6;
/// FPSR's QC and IOC, which `msr fpsr` sets.
const STATUS: u64 = 0x0800_0001;

#[test]
fn a_system_call_that_ends_the_execution_ends_at_its_own_immediate() {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &[SYSTEM_CALL_1234], 0);
    let ended = execute(&mut executor, &memory, before(16), &mut Handler::default());
    assert_eq!(
        ended.exception,
        Some(Exception::SupervisorCall {
            immediate: IMMEDIATE
        })
    );
    assert_eq!(ended.program_counter, CODE + size_of::<u32>() as u64);
}

/// The flags, vector registers and FPSR the guest leaves are the execution's: `subs` of a register from itself sets Z
/// and C, `dup` fills both halves, and `msr fpsr` sets what it is given.
#[test]
fn the_flags_vectors_and_float_status_the_guest_leaves_are_the_execution_s() {
    let mut executor = executor();
    let (memory, _table) = memory(
        &executor,
        &[SUBTRACT_FLAGS, DUPLICATE_X5, WRITE_FPSR, SYSTEM_CALL],
        0,
    );
    let mut start = before(17);
    start.general_purpose_registers[STATUS_FROM] = STATUS;
    for (v, value) in start.vector_registers.iter_mut().enumerate() {
        *value = u128::from(start.general_purpose_registers[v % 31]) << 64 | v as u128;
    }
    let ended = execute(
        &mut executor,
        &memory,
        start.clone(),
        &mut Handler::default(),
    );
    assert_eq!(
        ended.exception,
        Some(Exception::SupervisorCall { immediate: 0 })
    );
    assert_eq!(
        ended.condition_flags,
        ConditionFlags::ZERO | ConditionFlags::CARRY
    );
    let duplicated = u128::from(start.general_purpose_registers[DUPLICATED]);
    let mut vectors = start.vector_registers;
    vectors[DUPLICATED_INTO] = duplicated << 64 | duplicated;
    assert_eq!(ended.vector_registers, vectors);
    assert_eq!(ended.floating_point_status, STATUS);
}

/// A guest that loops through two pieces, landing at every one, is interrupted each slice too: a landing is no trap.
#[test]
fn a_guest_that_loops_through_two_pieces_is_interrupted_each_slice() {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &[NOTHING_DONE, BRANCH_BACK], 0);
    let mut handler = Handler {
        interrupts_before_end: Some(SLICES),
        ..Handler::default()
    };
    let ended = execute(&mut executor, &memory, before(18), &mut handler);
    assert_eq!(handler.interrupts, SLICES + 1);
    assert_eq!(ended.exception, None);
    assert!(handler.calls.is_empty());
}

/// An access through a stack pointer that is not a multiple of 16 faults as Linux sets EL0 (SCTLR_EL1.SA0), and the
/// guest's state is at the access.
#[test]
fn an_access_through_a_misaligned_stack_pointer_is_a_stack_pointer_alignment_fault() {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &[LOAD_THROUGH_SP, SYSTEM_CALL], 0);
    let start = Processor {
        stack_pointer: STACK + 1,
        ..before(19)
    };
    let ended = execute(&mut executor, &memory, start, &mut Handler::default());
    assert_eq!(ended.exception, Some(Exception::StackPointerAlignmentFault));
    assert_eq!(
        (ended.program_counter, ended.stack_pointer),
        (CODE, STACK + 1)
    );
}

/// A tagged address that faults: the handler is given it without its top byte, which the chip ignores, and the abort
/// the execution ends at keeps it.
#[test]
fn a_tagged_fault_reaches_the_handler_untagged_and_the_abort_tagged() {
    let mut executor = executor();
    let tagged = NOTHING | TAG;
    let (faults, ended) = fault_of(&mut executor, LOAD_THROUGH_X1, tagged);
    assert_eq!(faults, [(NOTHING, Access::Read)]);
    assert_eq!(
        ended.exception,
        Some(Exception::DataAbort {
            status: FaultStatus::TranslationFaultLevel3,
            access: AccessDirection::Read,
            address: tagged,
        })
    );
}

/// A read of the counter, CNTVCT_EL0, which the instruction list has EL1 trap so that the system answers it, ends at
/// its trap with the register's encoding (op0 3, op1 3, CRn 14, CRm 0, op2 2) and x0.
#[test]
fn a_read_of_the_counter_ends_at_its_trap() {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &[READ_COUNTER, SYSTEM_CALL], 0);
    let ended = execute(&mut executor, &memory, before(20), &mut Handler::default());
    assert_eq!(
        ended.exception,
        Some(Exception::SystemAccessTrapped(SystemAccess {
            op0: 3,
            op1: 3,
            crn: 14,
            crm: 0,
            op2: 2,
            rt: 0,
            direction: AccessDirection::Read,
        }))
    );
    assert_eq!(ended.program_counter, CODE);
}

/// The host addresses of every slot (from 64 KiB below slot 1's window to the last slot's end) and of every slot
/// table (from slot 0's at 2^46, 2^35 bytes for each slot).
const SLOTS: core::ops::Range<u64> = (1 << 33) - (64 << 10)..1 << 44;
const SLOT_TABLES: core::ops::Range<u64> = 1 << 46..(1 << 46) + (1 << 44 << 2);

/// The pieces of an execution are mapped where no window or slot table is, so that a guest never reaches them and
/// they take no slot from a memory.
#[test]
fn pieces_are_mapped_outside_every_slot_and_slot_table() {
    let mut executor = executor();
    let (memory, _table) = memory(&executor, &INSTRUCTIONS, 0);
    execute(&mut executor, &memory, before(21), &mut Handler::default());
    let maps = std::fs::read_to_string("/proc/self/maps").expect("the process's own maps");
    let pieces: Vec<(u64, u64)> = maps
        .lines()
        .filter(|line| line.contains("wcontainers instructions"))
        .filter_map(|line| {
            let (start, end) = line.split_whitespace().next()?.split_once('-')?;
            Some((
                u64::from_str_radix(start, 16).ok()?,
                u64::from_str_radix(end, 16).ok()?,
            ))
        })
        .collect();
    assert!(!pieces.is_empty(), "no piece mapped");
    // The floor is the one page of exits that holds its first entry.
    let floor = wcontainers_arm64::way::IN_PROCESS_ENTRIES & !(GUEST_PAGE - 1);
    assert!(
        pieces.contains(&(floor, floor + GUEST_PAGE)),
        "the floor as one page at {floor:#x}: {pieces:x?}"
    );
    for (start, end) in pieces {
        for area in [SLOTS, SLOT_TABLES] {
            assert!(
                end <= area.start || start >= area.end,
                "a piece at {start:#x}..{end:#x} inside {area:#x?}"
            );
        }
    }
}
