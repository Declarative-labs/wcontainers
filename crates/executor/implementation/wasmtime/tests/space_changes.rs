//! Changes to a space while code runs on it: new code over run code, zeroed, copied, mapped and unmapped bytes across
//! window ends, a frame pushed onto a page, a rounding mode, cache maintenance and two threads on one space.
//!
//! Each test states the bytes, registers or fault the executor interface and Arm's rules give.

use std::borrow::Cow;
use std::collections::VecDeque;
use wcontainers_abi::{Access, LOWEST_ADDRESS, PAGE_SIZE, Permissions, SystemCallRequest};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_executor::memory_space::{MapMemoryError, MemorySpace, PageTable};
use wcontainers_executor::{
    Execution, ExecutionStop, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_executor_wasmtime::{WasmtimeExecutor, WasmtimeTranslatorConfig};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_arm64::{
    DataAccess, Exception, InstructionFetch, PagePermission, Processor, ZERO_PROCESSOR,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// ADD X1, X1, #1 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--immediate---Add-immediate-value->).
const ADD_ONE: u32 = 0x9100_0421;
/// ADD X1, X1, #2.
const ADD_TWO: u32 = 0x9100_0821;
/// LDR X1, [X0] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate-->).
const LOAD: u32 = 0xF940_0001;
/// STR W3, [X0] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-register--immediate-->).
const STORE_WORD: u32 = 0xB900_0003;
/// STP X29, X30, [SP, #-16]! (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STP--Store-pair-of-registers->).
const PUSH_PAIR: u32 = 0xA9BF_7BFD;
/// LDP X0, X1, [SP] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDP--Load-pair-of-registers->).
const LOAD_PAIR: u32 = 0xA940_07E0;
/// FADD S2, S0, S1 (<https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/FADD--scalar---Floating-point-add--scalar-->).
const ADD_SINGLES: u32 = 0x1E21_2802;
/// DC CVAC, X0 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/DC--Data-cache-operation--an-alias-of-SYS->).
const CLEAN_CACHE: u32 = 0xD50B_7A20;
/// SVC #0 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SVC--Supervisor-call->).
const SUPERVISOR_CALL: u32 = 0xD400_0001;
/// The bytes of address one window holds.
const WINDOW: u64 = 1 << u32::BITS;
/// The bytes of a word.
const WORD: u64 = size_of::<u64>() as u64;
/// The bytes of an instruction.
const INSTRUCTION: u64 = size_of::<u32>() as u64;
/// Where the second code page of a test starts.
const SECOND_CODE: u64 = LOWEST_ADDRESS + PAGE_SIZE;
/// The most windows a space's guest memory holds at once, window 0 among them.
const HOST_WINDOWS: u64 = 8;
/// A value the tests store.
const STORED: u64 = 0x0123_4567_89AB_CDEF;
/// The single-precision 1.0 (<https://en.wikipedia.org/wiki/Single-precision_floating-point_format>).
const ONE: u32 = 0x3F80_0000;
/// Three quarters of the distance from 1.0 to the next single, 3 × 2^-25.
const THREE_QUARTERS_OF_A_STEP: u32 = 0x33C0_0000;
/// The single after 1.0.
const ONE_AND_A_STEP: u32 = 0x3F80_0001;
/// FPCR's RMode field set to round toward zero (<https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPCR--Floating-point-Control-Register>).
const ROUND_TOWARD_ZERO: u64 = 0b11 << 22;
/// FPSR's IXC bit: the result was rounded (<https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPSR--Floating-point-Status-Register>).
const INEXACT: u64 = 1 << 4;
/// ESR_EL1 of a data abort from EL0 by cache maintenance, without its status code: EC 0x24, IL, CM and WnR
/// (<https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1->).
const CACHE_MAINTENANCE_ABORT: u64 = 0x9200_0140;
/// ESR_EL1's data fault status code bits.
const FAULT_STATUS_BITS: u64 = 0x3F;

/// Ends the run at the first system call, and keeps how many it saw and the first memory fault, which also ends it.
#[derive(Default)]
struct Recording {
    system_calls: usize,
    fault: Option<(u64, Access)>,
}

impl TrapHandler for Recording {
    fn handle_system_call(&mut self, _: SystemCallRequest) -> SystemCallResponse {
        self.system_calls += 1;
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        self.fault = Some((address, access));
        MemoryFaultResponse::Terminate
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        InterruptResponse::Continue
    }
}

type Memory = <WasmtimeExecutor as Executor>::GuestMemory;
type Table = <Memory as MemorySpace>::PageTable;

/// Returns a new executor.
fn executor() -> WasmtimeExecutor {
    WasmtimeExecutor::new(WasmtimeTranslatorConfig::default())
}

/// Returns the bytes of the words, each instruction little-endian.
fn code_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

/// Writes the bytes at the address through a borrow.
fn write(memory: &Memory, address: u64, bytes: &[u8]) {
    let view = memory
        .borrow(address..address + bytes.len() as u64)
        .expect("the bytes' addresses");
    assert_eq!(view.write(bytes), bytes.len());
}

/// Writes the code at the address, ending with a system call.
fn write_code(memory: &Memory, address: u64, words: &[u32]) {
    let mut words = words.to_vec();
    words.push(SUPERVISOR_CALL);
    write(memory, address, &code_bytes(&words));
}

/// Reads the word at the address through a borrow.
fn read_word(memory: &Memory, address: u64) -> u64 {
    let view = memory
        .borrow(address..address + WORD)
        .expect("the word's addresses");
    let mut word = [u8::MIN; size_of::<u64>()];
    assert_eq!(view.read(&mut word), word.len());
    u64::from_le_bytes(word)
}

/// Gives the pages of the addresses the permissions.
fn allow(table: &mut Table, addresses: std::ops::Range<u64>, allowed: Permissions) {
    table
        .set_permissions(addresses, allowed)
        .expect("pages of the space");
}

/// Returns a space whose first code page holds the words and allows reading and running them.
fn with_code(executor: &WasmtimeExecutor, words: &[u32]) -> (Memory, Table) {
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    write_code(&memory, LOWEST_ADDRESS, words);
    allow(
        &mut table,
        LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE,
        Permissions::READ | Permissions::EXECUTE,
    );
    (memory, table)
}

/// Runs from the address with the registers given, and returns the processor it ended with and what the handler kept.
fn run_from(
    executor: &mut WasmtimeExecutor,
    memory: &Memory,
    start: Processor<()>,
) -> (Processor<()>, Recording) {
    let mut handler = Recording::default();
    let (execution, _interrupter) = executor.prepare_execution(start, memory);
    let ExecutionStop::Terminated(processor) = execution.run(&mut handler) else {
        panic!("the executor waited");
    };
    (processor, handler)
}

/// Runs from the first code page with X0 the address and every other register zero.
fn run(
    executor: &mut WasmtimeExecutor,
    memory: &Memory,
    address: u64,
) -> (Processor<()>, Recording) {
    let mut registers = ZERO_PROCESSOR.general_purpose_registers;
    registers[0] = address;
    let start = Processor {
        program_counter: LOWEST_ADDRESS,
        general_purpose_registers: registers,
        ..ZERO_PROCESSOR
    };
    run_from(executor, memory, start)
}

/// Returns X1 after a run of the first code page, and how many system calls it made.
fn added(executor: &mut WasmtimeExecutor, memory: &Memory) -> (u64, usize) {
    let (processor, handler) = run(executor, memory, u64::MIN);
    (processor.general_purpose_registers[1], handler.system_calls)
}

/// Writes into each word of the addresses its own address divided by 8 plus the base: a word read back names where
/// it was written and in which space.
fn fill(memory: &Memory, addresses: std::ops::Range<u64>, base: u64) {
    let words: Vec<u8> = (addresses.start / WORD..addresses.end / WORD)
        .flat_map(|word| (base + word).to_le_bytes())
        .collect();
    write(memory, addresses.start, &words);
}

/// Returns the value [`fill`] writes at the address.
fn filled(address: u64, base: u64) -> u64 {
    base + address / WORD
}

/// Returns the 8 bytes from the middle of the word before the address to the middle of the word at it, as [`fill`]
/// wrote them with the base: what a load 4 bytes before a filled word reads.
fn straddling(address: u64, base: u64) -> u64 {
    (filled(address - WORD, base) >> u32::BITS) | (filled(address, base) << u32::BITS)
}

#[test]
fn new_code_written_over_run_code_runs() {
    let mut executor = executor();
    let (memory, _table) = with_code(&executor, &[ADD_ONE]);
    assert_eq!(added(&mut executor, &memory), (1, 1));
    write(&memory, LOWEST_ADDRESS, &ADD_TWO.to_le_bytes());
    assert_eq!(
        added(&mut executor, &memory),
        (2, 1),
        "written through a borrow"
    );
    let view = memory
        .borrow(LOWEST_ADDRESS..LOWEST_ADDRESS + INSTRUCTION)
        .expect("the code's addresses");
    let mut source: VecDeque<u8> = ADD_ONE.to_le_bytes().into_iter().collect();
    assert_eq!(view.write_from(&mut source), Ok(size_of::<u32>()));
    assert_eq!(
        added(&mut executor, &memory),
        (1, 1),
        "written from a source"
    );
}

#[test]
fn code_copied_over_run_code_runs() {
    let mut executor = executor();
    let (memory, mut table) = with_code(&executor, &[ADD_ONE]);
    let (_other, other_table) = with_code(&executor, &[ADD_TWO]);
    assert_eq!(added(&mut executor, &memory), (1, 1));
    table
        .copy_from(LOWEST_ADDRESS..SECOND_CODE, &other_table, LOWEST_ADDRESS)
        .expect("a copy");
    assert_eq!(added(&mut executor, &memory), (2, 1));
}

#[test]
fn code_mapped_over_run_code_runs() {
    let mut executor = executor();
    let (memory, mut table) = with_code(&executor, &[ADD_ONE]);
    let (_other, other_table) = with_code(&executor, &[ADD_TWO]);
    assert_eq!(added(&mut executor, &memory), (1, 1));
    table
        .map_from(LOWEST_ADDRESS..SECOND_CODE, &other_table, LOWEST_ADDRESS)
        .expect("a mapping");
    assert_eq!(added(&mut executor, &memory), (2, 1));
}

#[test]
fn run_code_unmapped_or_zeroed_runs_no_more() {
    // The zero word is UDF #0, an undefined instruction, so the run ends before X1 changes or a system call.
    for unmapped in [true, false] {
        let mut executor = executor();
        let (memory, mut table) = executor.create_guest_memory().expect("a space");
        let (_other, other_table) = with_code(&executor, &[ADD_ONE]);
        // A mapping brings bytes, not permissions.
        if unmapped {
            table
                .map_from(LOWEST_ADDRESS..SECOND_CODE, &other_table, LOWEST_ADDRESS)
                .expect("a mapping");
        } else {
            write_code(&memory, LOWEST_ADDRESS, &[ADD_ONE]);
        }
        allow(
            &mut table,
            LOWEST_ADDRESS..SECOND_CODE,
            Permissions::READ | Permissions::EXECUTE,
        );
        assert_eq!(added(&mut executor, &memory), (1, 1));
        if unmapped {
            table
                .unmap_from(LOWEST_ADDRESS..SECOND_CODE)
                .expect("an unmapping");
        } else {
            memory
                .zero(LOWEST_ADDRESS..SECOND_CODE)
                .expect("the code's addresses");
        }
        assert_eq!(read_word(&memory, LOWEST_ADDRESS), u64::MIN);
        let (processor, handler) = run(&mut executor, &memory, u64::MIN);
        assert_eq!(handler.system_calls, 0, "unmapped {unmapped}");
        assert_eq!(processor.general_purpose_registers[1], u64::MIN);
        assert_eq!(processor.exception, Some(Exception::UndefinedInstruction));
    }
}

#[test]
fn run_code_that_loses_running_runs_no_more() {
    let mut executor = executor();
    let (memory, mut table) = with_code(&executor, &[ADD_ONE]);
    assert_eq!(added(&mut executor, &memory), (1, 1));
    allow(&mut table, LOWEST_ADDRESS..SECOND_CODE, Permissions::READ);
    let (processor, handler) = run(&mut executor, &memory, u64::MIN);
    assert_eq!(handler.system_calls, 0);
    assert_eq!(processor.general_purpose_registers[1], u64::MIN);
    assert!(
        matches!(processor.exception, Some(Exception::InstructionAbort { address, .. }) if address == LOWEST_ADDRESS),
        "{:?}",
        processor.exception
    );
}

#[test]
fn code_another_run_stores_over_runs_the_new_instruction() {
    let mut executor = executor();
    let (memory, mut table) = with_code(&executor, &[ADD_ONE]);
    allow(
        &mut table,
        LOWEST_ADDRESS..SECOND_CODE,
        Permissions::READ | Permissions::WRITE | Permissions::EXECUTE,
    );
    write_code(&memory, SECOND_CODE, &[STORE_WORD]);
    allow(
        &mut table,
        SECOND_CODE..SECOND_CODE + PAGE_SIZE,
        Permissions::READ | Permissions::EXECUTE,
    );
    assert_eq!(added(&mut executor, &memory), (1, 1));
    let mut registers = ZERO_PROCESSOR.general_purpose_registers;
    registers[0] = LOWEST_ADDRESS;
    registers[3] = u64::from(ADD_TWO);
    let storing = Processor {
        program_counter: SECOND_CODE,
        general_purpose_registers: registers,
        ..ZERO_PROCESSOR
    };
    let (_, handler) = run_from(&mut executor, &memory, storing);
    assert_eq!((handler.fault, handler.system_calls), (None, 1));
    assert_eq!(added(&mut executor, &memory), (2, 1));
}

#[test]
fn two_executors_on_one_space_keep_their_own_registers() {
    let mut first = executor();
    let mut second = executor();
    let (memory, mut table) = with_code(&first, &[LOAD]);
    let words = LOWEST_ADDRESS + 4 * PAGE_SIZE;
    allow(&mut table, words..words + PAGE_SIZE, Permissions::READ);
    write(&memory, words, &STORED.to_le_bytes());
    write(&memory, words + WORD, &(STORED + 1).to_le_bytes());
    let start = |address| {
        let mut registers = ZERO_PROCESSOR.general_purpose_registers;
        registers[0] = address;
        Processor {
            program_counter: LOWEST_ADDRESS,
            general_purpose_registers: registers,
            ..ZERO_PROCESSOR
        }
    };
    // Both are made before either runs, so each thread's registers wait in its own block.
    let (first_run, _first_interrupter) = first.prepare_execution(start(words), &memory);
    let (second_run, _second_interrupter) = second.prepare_execution(start(words + WORD), &memory);
    let mut handler = Recording::default();
    let ExecutionStop::Terminated(first_end) = first_run.run(&mut handler) else {
        panic!("the executor waited");
    };
    let ExecutionStop::Terminated(second_end) = second_run.run(&mut handler) else {
        panic!("the executor waited");
    };
    assert_eq!(first_end.general_purpose_registers[1], STORED);
    assert_eq!(second_end.general_purpose_registers[1], STORED + 1);
}

#[test]
fn a_push_onto_a_page_that_forbids_writing_faults_at_its_address() {
    for allowed in [Permissions::READ, Permissions::empty()] {
        let mut executor = executor();
        let (memory, mut table) = with_code(&executor, &[PUSH_PAIR]);
        let stack = LOWEST_ADDRESS + 8 * PAGE_SIZE;
        if !allowed.is_empty() {
            allow(&mut table, stack - PAGE_SIZE..stack, allowed);
        }
        let start = Processor {
            program_counter: LOWEST_ADDRESS,
            stack_pointer: stack,
            ..ZERO_PROCESSOR
        };
        let (_, handler) = run_from(&mut executor, &memory, start);
        assert_eq!(
            handler.fault,
            Some((stack - 2 * WORD, Access::Write)),
            "{allowed:?}"
        );
        assert_eq!(handler.system_calls, 0);
    }
}

#[test]
fn a_frame_pushed_onto_a_page_that_only_allows_writing_faults_when_read_back() {
    // The push writes, which the page allows; reading the pair back is a read, which it does not.
    let mut executor = executor();
    let (memory, mut table) = with_code(&executor, &[PUSH_PAIR, LOAD_PAIR]);
    let stack = LOWEST_ADDRESS + 8 * PAGE_SIZE;
    allow(&mut table, stack - PAGE_SIZE..stack, Permissions::WRITE);
    let mut registers = ZERO_PROCESSOR.general_purpose_registers;
    registers[29] = STORED;
    registers[30] = STORED + 1;
    let start = Processor {
        program_counter: LOWEST_ADDRESS,
        general_purpose_registers: registers,
        stack_pointer: stack,
        ..ZERO_PROCESSOR
    };
    let (processor, handler) = run_from(&mut executor, &memory, start);
    assert_eq!(
        (handler.fault, handler.system_calls),
        (Some((stack - 2 * WORD, Access::Read)), 0)
    );
    assert_eq!(
        processor.general_purpose_registers[..2],
        [u64::MIN, u64::MIN],
        "nothing read"
    );
    assert_eq!(read_word(&memory, stack - 2 * WORD), STORED, "the push stored");
    assert_eq!(read_word(&memory, stack - WORD), STORED + 1, "the push stored");
}

#[test]
fn a_push_in_window_one_stores_at_its_address() {
    let mut executor = executor();
    let (memory, mut table) = with_code(&executor, &[PUSH_PAIR]);
    // Past window 1's first 64 KiB, where its address as computed is another place of it.
    let stack = WINDOW + 0x2_0000;
    allow(
        &mut table,
        stack - PAGE_SIZE..stack,
        Permissions::READ | Permissions::WRITE,
    );
    let mut registers = ZERO_PROCESSOR.general_purpose_registers;
    registers[29] = STORED;
    registers[30] = STORED + 1;
    let start = Processor {
        program_counter: LOWEST_ADDRESS,
        general_purpose_registers: registers,
        stack_pointer: stack,
        ..ZERO_PROCESSOR
    };
    let (processor, handler) = run_from(&mut executor, &memory, start);
    assert_eq!((handler.fault, handler.system_calls), (None, 1));
    assert_eq!(processor.stack_pointer, stack - 2 * WORD);
    assert_eq!(read_word(&memory, stack - 2 * WORD), STORED);
    assert_eq!(read_word(&memory, stack - WORD), STORED + 1);
    assert_eq!(read_word(&memory, stack - 2 * WORD - 0x1_0000), u64::MIN);
}

#[test]
fn a_rounding_mode_reaches_a_floating_point_addition() {
    // 1.0 + 3 × 2^-25 lies three quarters of the way to the next single: to nearest it rounds up, toward zero down.
    for (control, sum) in [(u64::MIN, ONE_AND_A_STEP), (ROUND_TOWARD_ZERO, ONE)] {
        let mut executor = executor();
        let (memory, _table) = with_code(&executor, &[ADD_SINGLES]);
        let mut vectors = ZERO_PROCESSOR.vector_registers;
        vectors[0] = u128::from(ONE);
        vectors[1] = u128::from(THREE_QUARTERS_OF_A_STEP);
        let start = Processor {
            program_counter: LOWEST_ADDRESS,
            vector_registers: vectors,
            floating_point_control: control,
            ..ZERO_PROCESSOR
        };
        let (processor, handler) = run_from(&mut executor, &memory, start);
        assert_eq!(handler.system_calls, 1);
        assert_eq!(
            processor.vector_registers[2],
            u128::from(sum),
            "FPCR {control:#x}"
        );
        assert_eq!(processor.floating_point_status & INEXACT, INEXACT);
    }
}

#[test]
fn cache_maintenance_on_an_unmapped_page_reports_a_cache_maintenance_abort() {
    let mut executor = executor();
    let address = LOWEST_ADDRESS + 16 * PAGE_SIZE + 0x40;
    let mut registers = ZERO_PROCESSOR.general_purpose_registers;
    registers[0] = address;
    let before = ExecutionState {
        processor: Processor {
            program_counter: LOWEST_ADDRESS,
            general_purpose_registers: registers,
            ..ZERO_PROCESSOR
        },
        memory_mutations: vec![
            MemoryDelta::PermissionAssignment {
                addresses: LOWEST_ADDRESS..SECOND_CODE,
                permission: PagePermission::Mapped {
                    data_access: DataAccess::ReadOnly,
                    instruction_fetch: InstructionFetch::Allowed,
                },
            },
            MemoryDelta::ValueAssignment {
                start: LOWEST_ADDRESS,
                bytes: Cow::Owned(CLEAN_CACHE.to_le_bytes().to_vec()),
            },
        ],
    };
    let after = executor
        .execute_single_instruction(before)
        .expect("a state of the space");
    let Some(Exception::Undecoded {
        syndrome,
        fault_address,
    }) = after.processor.exception
    else {
        panic!("{:?}", after.processor.exception);
    };
    assert_eq!(fault_address, address);
    assert_eq!(syndrome & !FAULT_STATUS_BITS, CACHE_MAINTENANCE_ABORT);
}

#[test]
fn zero_clears_exactly_its_bytes_across_a_window_end() {
    let mut executor = executor();
    let (memory, mut table) = with_code(&executor, &[LOAD]);
    let filled_range = WINDOW - 3 * PAGE_SIZE..WINDOW + 3 * PAGE_SIZE;
    allow(
        &mut table,
        filled_range.clone(),
        Permissions::READ | Permissions::WRITE,
    );
    fill(&memory, filled_range.clone(), u64::MIN);
    // Part of a page, two whole pages either side of the window end, and part of a page.
    let cleared = WINDOW - 2 * PAGE_SIZE - 3 * WORD..WINDOW + 2 * PAGE_SIZE + 5 * WORD;
    memory.zero(cleared.clone()).expect("the addresses");
    for address in (filled_range.start..filled_range.end).step_by(WORD as usize) {
        let expected = if cleared.contains(&address) {
            u64::MIN
        } else {
            filled(address, u64::MIN)
        };
        assert_eq!(read_word(&memory, address), expected, "at {address:#x}");
    }
    // A load across the window end reads the zeros of both windows.
    let (processor, handler) = run(&mut executor, &memory, WINDOW - INSTRUCTION);
    assert_eq!(handler.fault, None);
    assert_eq!(processor.general_purpose_registers[1], u64::MIN);
}

#[test]
fn zero_over_a_mapped_page_clears_the_shared_bytes() {
    let executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let (other, mut other_table) = executor.create_guest_memory().expect("a space");
    let from = 3 * WINDOW + 5 * PAGE_SIZE;
    allow(
        &mut other_table,
        from..from + 2 * PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&other, from..from + 2 * PAGE_SIZE, STORED);
    let at = 2 * WINDOW + PAGE_SIZE;
    table
        .map_from(at..at + 2 * PAGE_SIZE, &other_table, from)
        .expect("a mapping");
    memory.zero(at..at + PAGE_SIZE).expect("the addresses");
    assert_eq!(read_word(&memory, at), u64::MIN);
    assert_eq!(
        read_word(&other, from + PAGE_SIZE - WORD),
        u64::MIN,
        "the page is shared"
    );
    assert_eq!(
        read_word(&memory, at + PAGE_SIZE),
        filled(from + PAGE_SIZE, STORED)
    );
}

#[test]
fn bytes_mapped_across_window_ends_show_the_other_space_and_its_next_page() {
    let mut executor = executor();
    let (memory, mut table) = with_code(&executor, &[LOAD]);
    let (other, mut other_table) = executor.create_guest_memory().expect("a space");
    // The other space's pages also cross a window end, at another offset.
    let from = 5 * WINDOW - PAGE_SIZE;
    allow(
        &mut other_table,
        from - PAGE_SIZE..from + 3 * PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&other, from - PAGE_SIZE..from + 3 * PAGE_SIZE, STORED);
    let at = WINDOW - 2 * PAGE_SIZE;
    table
        .map_from(at..at + 3 * PAGE_SIZE, &other_table, from)
        .expect("a mapping");
    allow(
        &mut table,
        at..at + 3 * PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    for offset in (0..3 * PAGE_SIZE).step_by(WORD as usize * 64) {
        assert_eq!(
            read_word(&memory, at + offset),
            filled(from + offset, STORED),
            "at {:#x}",
            at + offset
        );
    }
    // A load across window 0's end reads its last page and window 1's first, both shown from the other space.
    let end = WINDOW;
    let (processor, handler) = run(&mut executor, &memory, end - INSTRUCTION);
    assert_eq!(handler.fault, None);
    assert_eq!(
        processor.general_purpose_registers[1],
        straddling(from + (end - at), STORED)
    );
    // A store through the mapping is the other space's.
    write(&memory, end, &STORED.to_le_bytes());
    assert_eq!(read_word(&other, from + (end - at)), STORED);
}

#[test]
fn a_window_whose_first_page_is_mapped_shows_it_after_the_window_before() {
    // Window 2 has its place before its first page is mapped, which then shows after window 2's place too.
    let mut executor = executor();
    let (memory, mut table) = with_code(&executor, &[LOAD]);
    let (other, mut other_table) = executor.create_guest_memory().expect("a space");
    let from = 7 * WINDOW + 9 * PAGE_SIZE;
    allow(
        &mut other_table,
        from..from + PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&other, from..from + PAGE_SIZE, STORED);
    let end = 3 * WINDOW;
    allow(
        &mut table,
        end - PAGE_SIZE..end,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&memory, end - PAGE_SIZE..end, u64::MIN);
    table
        .map_from(end..end + PAGE_SIZE, &other_table, from)
        .expect("a mapping");
    allow(&mut table, end..end + PAGE_SIZE, Permissions::READ);
    let (processor, handler) = run(&mut executor, &memory, end - INSTRUCTION);
    assert_eq!(handler.fault, None);
    let low = filled(end - WORD, u64::MIN) >> u32::BITS;
    let high = filled(from, STORED) << u32::BITS;
    assert_eq!(processor.general_purpose_registers[1], low | high);
}

#[test]
fn unmapping_the_middle_of_a_mapping_zeroes_only_those_pages() {
    let executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let (other, mut other_table) = executor.create_guest_memory().expect("a space");
    let from = 6 * WINDOW + 3 * PAGE_SIZE;
    allow(
        &mut other_table,
        from..from + 4 * PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&other, from..from + 4 * PAGE_SIZE, STORED);
    let at = 2 * WINDOW + 7 * PAGE_SIZE;
    table
        .map_from(at..at + 4 * PAGE_SIZE, &other_table, from)
        .expect("a mapping");
    table
        .unmap_from(at + PAGE_SIZE..at + 3 * PAGE_SIZE)
        .expect("an unmapping");
    for page in 0..4 {
        let address = at + page * PAGE_SIZE + WORD;
        let expected = if (1..3).contains(&page) {
            u64::MIN
        } else {
            filled(from + page * PAGE_SIZE + WORD, STORED)
        };
        assert_eq!(read_word(&memory, address), expected, "page {page}");
    }
    // An unmapped page is this space's own again.
    write(&memory, at + PAGE_SIZE, &u64::MAX.to_le_bytes());
    assert_eq!(
        read_word(&other, from + PAGE_SIZE),
        filled(from + PAGE_SIZE, STORED)
    );
}

#[test]
fn a_copy_across_window_ends_at_other_offsets_lands_each_word() {
    let mut executor = executor();
    let (memory, mut table) = with_code(&executor, &[LOAD]);
    let (other, mut other_table) = executor.create_guest_memory().expect("a space");
    // The other space's data runs past the copied pages on both sides, and this space's past where they land.
    let from = WINDOW - 2 * PAGE_SIZE;
    let length = 4 * PAGE_SIZE;
    allow(
        &mut other_table,
        from - PAGE_SIZE..from + length + PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&other, from - PAGE_SIZE..from + length + PAGE_SIZE, STORED);
    let at = 3 * WINDOW - PAGE_SIZE;
    allow(
        &mut table,
        at - PAGE_SIZE..at + length + PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&memory, at - PAGE_SIZE..at + length + PAGE_SIZE, u64::MIN);
    table
        .copy_from(at..at + length, &other_table, from)
        .expect("a copy");
    for offset in (0..length).step_by(WORD as usize * 32) {
        assert_eq!(
            read_word(&memory, at + offset),
            filled(from + offset, STORED),
            "at {:#x}",
            at + offset
        );
    }
    for outside in [at - WORD, at + length] {
        assert_eq!(
            read_word(&memory, outside),
            filled(outside, u64::MIN),
            "at {outside:#x}"
        );
    }
    // A load across the window end inside the copy reads the other space's bytes.
    let end = 3 * WINDOW;
    let (processor, handler) = run(&mut executor, &memory, end - INSTRUCTION);
    assert_eq!(handler.fault, None);
    assert_eq!(
        processor.general_purpose_registers[1],
        straddling(from + (end - at), STORED)
    );
    // The copy is this space's own.
    write(&memory, at, &u64::MAX.to_le_bytes());
    assert_eq!(read_word(&other, from), filled(from, STORED));
}

#[test]
fn a_copy_of_a_page_never_written_is_zero_and_of_a_mapped_page_shares_it() {
    let executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let (_other, mut other_table) = executor.create_guest_memory().expect("a space");
    let (third, mut third_table) = executor.create_guest_memory().expect("a space");
    let from = 4 * WINDOW + 2 * PAGE_SIZE;
    // Page 0 is given permissions but never written; pages 1 and 2 are mapped from two places of the third space,
    // not one after another, and page 3 from a fourth.
    allow(
        &mut other_table,
        from..from + PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    let third_at = 5 * WINDOW;
    allow(
        &mut third_table,
        third_at..third_at + 8 * PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&third, third_at..third_at + 8 * PAGE_SIZE, STORED);
    other_table
        .map_from(
            from + PAGE_SIZE..from + 2 * PAGE_SIZE,
            &third_table,
            third_at + 5 * PAGE_SIZE,
        )
        .expect("a mapping");
    other_table
        .map_from(
            from + 2 * PAGE_SIZE..from + 3 * PAGE_SIZE,
            &third_table,
            third_at + 2 * PAGE_SIZE,
        )
        .expect("a mapping");
    let (fourth, mut fourth_table) = executor.create_guest_memory().expect("a space");
    allow(
        &mut fourth_table,
        third_at..third_at + PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&fourth, third_at..third_at + PAGE_SIZE, STORED * 2);
    other_table
        .map_from(
            from + 3 * PAGE_SIZE..from + 4 * PAGE_SIZE,
            &fourth_table,
            third_at,
        )
        .expect("a mapping");
    // This space held other bytes there, partly mapped from the third space.
    let at = 2 * WINDOW + 9 * PAGE_SIZE;
    allow(
        &mut table,
        at..at + 4 * PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&memory, at..at + 4 * PAGE_SIZE, u64::MIN);
    table
        .map_from(at..at + PAGE_SIZE, &third_table, third_at + 7 * PAGE_SIZE)
        .expect("a mapping");
    table
        .copy_from(at..at + 4 * PAGE_SIZE, &other_table, from)
        .expect("a copy");
    assert_eq!(
        read_word(&memory, at + WORD),
        u64::MIN,
        "a page never written"
    );
    assert_eq!(
        read_word(&memory, at + PAGE_SIZE + WORD),
        filled(third_at + 5 * PAGE_SIZE + WORD, STORED)
    );
    assert_eq!(
        read_word(&memory, at + 2 * PAGE_SIZE + WORD),
        filled(third_at + 2 * PAGE_SIZE + WORD, STORED)
    );
    assert_eq!(
        read_word(&memory, at + 3 * PAGE_SIZE + WORD),
        filled(third_at + WORD, STORED * 2)
    );
    // A copied mapping still shares the bytes it was mapped from.
    write(&memory, at + 2 * PAGE_SIZE, &u64::MAX.to_le_bytes());
    assert_eq!(read_word(&third, third_at + 2 * PAGE_SIZE), u64::MAX);
    // The page this space mapped before the copy no longer shows the third space.
    write(&memory, at, &u64::MAX.to_le_bytes());
    assert_eq!(
        read_word(&third, third_at + 7 * PAGE_SIZE),
        filled(third_at + 7 * PAGE_SIZE, STORED)
    );
}

#[test]
fn bytes_across_a_window_end_go_to_a_sink_and_come_from_a_source() {
    let executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let around = WINDOW - 2 * WORD..WINDOW + 2 * WORD;
    allow(
        &mut table,
        WINDOW - PAGE_SIZE..WINDOW + PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    );
    fill(&memory, around.clone(), STORED);
    let view = memory.borrow(around.clone()).expect("the addresses");
    let mut sink = Vec::new();
    assert_eq!(view.read_to(&mut sink), Ok(4 * size_of::<u64>()));
    let expected: Vec<u8> = (around.start / WORD..around.end / WORD)
        .flat_map(|word| (STORED + word).to_le_bytes())
        .collect();
    assert_eq!(sink, expected);
    let mut source: VecDeque<u8> = (0..4 * WORD).map(|byte| byte as u8).collect();
    assert_eq!(view.write_from(&mut source), Ok(4 * size_of::<u64>()));
    assert_eq!(read_word(&memory, WINDOW + WORD), 0x1F1E_1D1C_1B1A_1918);
}

/// Gives the pages of the addresses reading and writing.
fn writable(table: &mut Table, addresses: std::ops::Range<u64>) {
    allow(table, addresses, Permissions::READ | Permissions::WRITE);
}

/// Maps one page of the other table's space at the address.
fn map_page(table: &mut Table, at: u64, other: &Table, from: u64) {
    table
        .map_from(at..at + PAGE_SIZE, other, from)
        .expect("a mapping");
}

#[test]
fn a_copy_over_mapped_pages_from_pages_never_written_leaves_them_zero_and_its_own() {
    let executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let (other, mut other_table) = executor.create_guest_memory().expect("a space");
    let (third, mut third_table) = executor.create_guest_memory().expect("a space");
    let at = 2 * WINDOW + 16 * PAGE_SIZE;
    // This space: data before and in the first two pages, the last two mapped from the third space.
    writable(&mut table, at - 3 * PAGE_SIZE..at + 4 * PAGE_SIZE);
    fill(&memory, at - 3 * PAGE_SIZE..at + 4 * PAGE_SIZE, u64::MIN);
    let third_at = 6 * WINDOW;
    writable(&mut third_table, third_at..third_at + 2 * PAGE_SIZE);
    fill(&third, third_at..third_at + 2 * PAGE_SIZE, STORED);
    table
        .map_from(
            at + 2 * PAGE_SIZE..at + 4 * PAGE_SIZE,
            &third_table,
            third_at,
        )
        .expect("a mapping");
    // The other space: data in the first two pages, the last two given permissions but never written.
    let from = 4 * WINDOW + 8 * PAGE_SIZE;
    writable(&mut other_table, from..from + 4 * PAGE_SIZE);
    fill(&other, from..from + 2 * PAGE_SIZE, STORED * 3);
    table
        .copy_from(at..at + 4 * PAGE_SIZE, &other_table, from)
        .expect("a copy");
    for page in 0..4 {
        for offset in [WORD, PAGE_SIZE - WORD] {
            let address = at + page * PAGE_SIZE + offset;
            let expected = if page < 2 {
                filled(from + page * PAGE_SIZE + offset, STORED * 3)
            } else {
                u64::MIN
            };
            assert_eq!(read_word(&memory, address), expected, "at {address:#x}");
        }
    }
    // The last two pages are this space's own: the third space keeps its bytes, and a copy of them is not shared.
    write(&memory, at + 2 * PAGE_SIZE, &STORED.to_le_bytes());
    assert_eq!(read_word(&third, third_at), filled(third_at, STORED));
    let (copy, mut copy_table) = executor.create_guest_memory().expect("a space");
    copy_table
        .copy_from(
            at + 2 * PAGE_SIZE..at + 3 * PAGE_SIZE,
            &table,
            at + 2 * PAGE_SIZE,
        )
        .expect("a copy");
    assert_eq!(read_word(&copy, at + 2 * PAGE_SIZE), STORED);
    write(&copy, at + 2 * PAGE_SIZE, &u64::MAX.to_le_bytes());
    assert_eq!(read_word(&memory, at + 2 * PAGE_SIZE), STORED);
    // Zeroing the page clears what this space shows there.
    memory
        .zero(at + 2 * PAGE_SIZE..at + 3 * PAGE_SIZE)
        .expect("the addresses");
    assert_eq!(read_word(&memory, at + 2 * PAGE_SIZE), u64::MIN);
}

#[test]
fn a_copy_of_pages_mapped_from_out_of_order_places_shows_each_one() {
    let executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let (_other, mut other_table) = executor.create_guest_memory().expect("a space");
    let (third, mut third_table) = executor.create_guest_memory().expect("a space");
    let third_at = 3 * WINDOW;
    writable(&mut third_table, third_at..third_at + 8 * PAGE_SIZE);
    fill(&third, third_at..third_at + 8 * PAGE_SIZE, STORED);
    // The other space's pages show the third space's pages 5, 4, then 2 twice.
    let from = 5 * WINDOW + 2 * PAGE_SIZE;
    for (page, third_page) in [(0, 5), (1, 4), (2, 2), (3, 2)] {
        map_page(
            &mut other_table,
            from + page * PAGE_SIZE,
            &third_table,
            third_at + third_page * PAGE_SIZE,
        );
    }
    writable(&mut other_table, from..from + 4 * PAGE_SIZE);
    let at = WINDOW + 3 * PAGE_SIZE;
    table
        .copy_from(at..at + 4 * PAGE_SIZE, &other_table, from)
        .expect("a copy");
    for (page, third_page) in [(0, 5), (1, 4), (2, 2), (3, 2)] {
        let address = at + page * PAGE_SIZE + WORD;
        assert_eq!(
            read_word(&memory, address),
            filled(third_at + third_page * PAGE_SIZE + WORD, STORED),
            "page {page}"
        );
    }
}

#[test]
fn a_copy_of_a_page_mapped_then_unmapped_is_not_shared() {
    let executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let (other, mut other_table) = executor.create_guest_memory().expect("a space");
    let (third, mut third_table) = executor.create_guest_memory().expect("a space");
    let third_at = 7 * WINDOW;
    writable(&mut third_table, third_at..third_at + PAGE_SIZE);
    fill(&third, third_at..third_at + PAGE_SIZE, STORED);
    let from = 2 * WINDOW + 5 * PAGE_SIZE;
    map_page(&mut other_table, from, &third_table, third_at);
    other_table
        .unmap_from(from..from + PAGE_SIZE)
        .expect("an unmapping");
    writable(&mut other_table, from..from + PAGE_SIZE);
    write(&other, from, &STORED.to_le_bytes());
    let at = 4 * WINDOW + 5 * PAGE_SIZE;
    table
        .copy_from(at..at + PAGE_SIZE, &other_table, from)
        .expect("a copy");
    assert_eq!(read_word(&memory, at), STORED);
    write(&memory, at, &u64::MAX.to_le_bytes());
    assert_eq!(read_word(&other, from), STORED, "the copy is not shared");
    assert_eq!(read_word(&third, third_at), filled(third_at, STORED));
}

#[test]
fn a_mapping_either_space_has_no_window_left_for_is_refused_and_changes_nothing() {
    let executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let (other, mut other_table) = executor.create_guest_memory().expect("a space");
    // Each space gives every window its guest memory holds; the other's last page holds a word.
    for window in 1..HOST_WINDOWS {
        writable(&mut table, window * WINDOW..window * WINDOW + PAGE_SIZE);
        writable(
            &mut other_table,
            window * WINDOW..window * WINDOW + PAGE_SIZE,
        );
    }
    let from = WINDOW;
    write(&other, from, &STORED.to_le_bytes());
    let past = HOST_WINDOWS * WINDOW;
    assert_eq!(
        table.map_from(past..past + PAGE_SIZE, &other_table, from),
        Err(MapMemoryError::OutOfMemory),
        "this space has no window left"
    );
    assert_eq!(
        table.map_from(WINDOW..WINDOW + PAGE_SIZE, &other_table, past),
        Err(MapMemoryError::OutOfMemory),
        "the other space has no window left"
    );
    assert_eq!(read_word(&memory, WINDOW), u64::MIN, "nothing was mapped");
    write(&memory, WINDOW, &u64::MAX.to_le_bytes());
    assert_eq!(read_word(&other, from), STORED);
}

#[test]
fn a_copy_across_a_window_the_other_space_never_placed_lands_the_pages_after_it() {
    let executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let (other, mut other_table) = executor.create_guest_memory().expect("a space");
    // The other space has the last page of window 2 and the first of window 4; window 3 has no place.
    let from = 3 * WINDOW - PAGE_SIZE;
    writable(&mut other_table, from..3 * WINDOW);
    writable(&mut other_table, 4 * WINDOW..4 * WINDOW + PAGE_SIZE);
    fill(&other, from..3 * WINDOW, STORED);
    fill(&other, 4 * WINDOW..4 * WINDOW + PAGE_SIZE, STORED);
    let at = 5 * WINDOW - PAGE_SIZE;
    let length = WINDOW + 2 * PAGE_SIZE;
    table
        .copy_from(at..at + length, &other_table, from)
        .expect("a copy");
    assert_eq!(read_word(&memory, at + WORD), filled(from + WORD, STORED));
    let after = at + WINDOW + PAGE_SIZE;
    assert_eq!(
        read_word(&memory, after + WORD),
        filled(4 * WINDOW + WORD, STORED),
        "the page after the gap"
    );
    assert_eq!(
        read_word(&memory, 5 * WINDOW + PAGE_SIZE),
        u64::MIN,
        "the gap"
    );
}

#[test]
fn a_copy_of_more_bytes_than_one_read_takes_lands_every_word() {
    // Three MiB of data in one run, more than the copy reads at once.
    let executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let (other, mut other_table) = executor.create_guest_memory().expect("a space");
    let length = 3 << 20;
    let from = 2 * WINDOW + 7 * PAGE_SIZE;
    writable(&mut other_table, from..from + length);
    fill(&other, from..from + length, STORED);
    let at = 3 * WINDOW + 5 * PAGE_SIZE;
    table
        .copy_from(at..at + length, &other_table, from)
        .expect("a copy");
    for offset in (0..length).step_by(PAGE_SIZE as usize * 16) {
        for within in [u64::MIN, PAGE_SIZE * 16 - WORD] {
            let address = at + offset + within;
            assert_eq!(
                read_word(&memory, address),
                filled(from + offset + within, STORED),
                "at {address:#x}"
            );
        }
    }
}
