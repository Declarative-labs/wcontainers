//! A space's 2^48 bytes of addresses: its top page, window ends, faults, copies and the most windows it holds.
//!
//! An access across the end of a window reaches the next one, and a fault outside the pages given names its address.

use std::borrow::Cow;
use wcontainers_abi::{Access, LOWEST_ADDRESS, PAGE_SIZE, Permissions, SystemCallRequest};
use wcontainers_executor::memory_space::{InvalidAddressError, MemorySpace, PageTable};
use wcontainers_executor::{
    Execution, ExecutionStop, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_executor_wasmtime::{WasmtimeExecutor, WasmtimeTranslatorConfig};

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_arm64::{
    DataAccess, ExclusiveMonitor, InstructionFetch, PagePermission, Processor, ZERO_PROCESSOR,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// LDR X1, [X0] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate-->).
const LOAD: u32 = 0xF940_0001;
/// ADD X1, X1, #1 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--immediate---Add-immediate-value->).
const ADD_ONE: u32 = 0x9100_0421;
/// STR X1, [X0] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-register--immediate-->).
const STORE: u32 = 0xF900_0001;
/// LDXR X1, [X0] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDXR--Load-exclusive-register->).
const LOAD_EXCLUSIVE: u32 = 0xC85F_7C01;
/// SVC #0 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SVC--Supervisor-call->).
const SUPERVISOR_CALL: u32 = 0xD400_0001;
/// The bytes of address one window holds.
const WINDOW: u64 = 1 << u32::BITS;
/// The addresses a space has: those of a 48-bit virtual address.
const SPACE_END: u64 = 1 << 48;
/// The most windows a space's guest memory holds at once, window 0 among them.
const HOST_WINDOWS: u64 = 8;
/// A value the tests store, one less than the code leaves.
const STORED: u64 = 0x0123_4567_89AB_CDEF;
/// LDR X2, [X0, #8] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate-->).
const LOAD_NEXT: u32 = 0xF940_0402;
/// LDR X1, [X0], #8 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate-->).
const LOAD_AND_STEP: u32 = 0xF840_8401;
/// ADD X3, X3, X1 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--shifted-register---Add-optionally-shifted-register->).
const ADD_TO_SUM: u32 = 0x8B01_0063;
/// SUBS X2, X2, #1 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SUBS--immediate---Subtract-immediate-value--setting-flags->).
const COUNT_DOWN: u32 = 0xF100_0442;
/// B.NE back three instructions (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/B-cond--Branch-conditionally->).
const BACK_THREE: u32 = 0x54FF_FFA1;
/// B.NE back four instructions.
const BACK_FOUR: u32 = 0x54FF_FF81;
/// ADD X0, X0, #0x10, LSL #12: a step of 64 KiB, the bytes after each window's place.
const STEP_SPILL: u32 = 0x9140_4000;
/// ADD X0, X0, #0x11, LSL #12: a step of 68 KiB.
const STEP_PAST_SPILL: u32 = 0x9140_4400;
/// The bytes of the steps the two step instructions add.
const SPILL_STEP: u64 = 0x1_0000;
const PAST_SPILL_STEP: u64 = 0x1_1000;
/// The bytes of a word, which [`fill`] writes each address of divided by.
const WORD: u64 = size_of::<u64>() as u64;
/// The bytes [`fill`] writes.
const FILL: u64 = 1 << 20;
/// An offset in window 1 past the 64 KiB after its place, where its address as computed is another place of it.
const PAST_SPILL: u64 = 0x2_0000;
/// An offset inside a page, a multiple of a word.
const IN_PAGE: u64 = 0x48;
/// A page of window 0 the fault tests allow, so that an address landing in window 0 would not fault.
const LOW_PAGE: u64 = 0x20_0000;

/// Writes into each word from the address, for [`FILL`] bytes, its own address divided by 8: a word read back names
/// where it was read from.
fn fill(memory: &Memory, start: u64) {
    let words: Vec<u8> = (start / WORD..(start + FILL) / WORD)
        .flat_map(u64::to_le_bytes)
        .collect();
    let bytes = memory
        .borrow(start..start + FILL)
        .expect("the filled addresses");
    assert_eq!(bytes.write(&words), words.len());
}

/// Gives the pages of the addresses reading and writing.
fn writable_range(table: &mut Table, addresses: std::ops::Range<u64>) {
    table
        .set_permissions(addresses, Permissions::READ | Permissions::WRITE)
        .expect("pages of the space");
}

/// Ends the run at the first system call, and keeps the address of a memory fault, which also ends it.
#[derive(Default)]
struct Recording {
    fault: Option<(u64, Access)>,
}

impl TrapHandler for Recording {
    fn handle_system_call(&mut self, _: SystemCallRequest) -> SystemCallResponse {
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

/// Returns an executor and a space whose code page holds the words and ends with a system call.
fn with_code(words: &[u32]) -> (WasmtimeExecutor, Memory, Table) {
    let executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let code: Vec<u8> = words
        .iter()
        .chain(&[SUPERVISOR_CALL])
        .flat_map(|word| word.to_le_bytes())
        .collect();
    let bytes = memory
        .borrow(LOWEST_ADDRESS..LOWEST_ADDRESS + code.len() as u64)
        .expect("the code's addresses");
    assert_eq!(bytes.write(&code), code.len(), "the code written");
    table
        .set_permissions(
            LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE,
            Permissions::READ | Permissions::EXECUTE,
        )
        .expect("the code's page");
    (executor, memory, table)
}

/// Runs the code from its start with X0 the address, and returns the processor it ended with and what the handler kept.
fn run(
    executor: &mut WasmtimeExecutor,
    memory: &Memory,
    address: u64,
) -> (Processor<()>, Recording) {
    run_counted(executor, memory, address, u64::MIN)
}

/// Runs the code from its start with X0 the address and X2 the count, and returns the processor it ended with and what
/// the handler kept.
fn run_counted(
    executor: &mut WasmtimeExecutor,
    memory: &Memory,
    address: u64,
    count: u64,
) -> (Processor<()>, Recording) {
    let mut registers = ZERO_PROCESSOR.general_purpose_registers;
    registers[0] = address;
    registers[2] = count;
    let start = Processor {
        program_counter: LOWEST_ADDRESS,
        general_purpose_registers: registers,
        ..ZERO_PROCESSOR
    };
    let mut handler = Recording::default();
    let (execution, _interrupter) = executor.prepare_execution(start, memory);
    let ExecutionStop::Terminated(processor) = execution.run(&mut handler) else {
        panic!("the executor waited");
    };
    (processor, handler)
}

/// Writes the word at the address through a borrow.
fn write_word(memory: &Memory, address: u64, word: u64) {
    let bytes = memory
        .borrow(address..address + size_of::<u64>() as u64)
        .expect("the word's addresses");
    assert_eq!(bytes.write(&word.to_le_bytes()), size_of::<u64>());
}

/// Reads the word at the address through a borrow.
fn read_word(memory: &Memory, address: u64) -> u64 {
    let bytes = memory
        .borrow(address..address + size_of::<u64>() as u64)
        .expect("the word's addresses");
    let mut word = [0; size_of::<u64>()];
    assert_eq!(bytes.read(&mut word), word.len());
    u64::from_le_bytes(word)
}

/// Gives the page of the address reading and writing.
fn writable(table: &mut Table, address: u64) {
    let page = address - address % PAGE_SIZE;
    table
        .set_permissions(
            page..page + PAGE_SIZE,
            Permissions::READ | Permissions::WRITE,
        )
        .expect("a page of the space");
}

#[test]
fn the_top_page_is_loaded_and_stored() {
    let (mut executor, memory, mut table) = with_code(&[LOAD, ADD_ONE, STORE]);
    let address = SPACE_END - size_of::<u64>() as u64;
    writable(&mut table, address);
    write_word(&memory, address, STORED);
    let (processor, handler) = run(&mut executor, &memory, address);
    assert_eq!(handler.fault, None);
    assert_eq!(processor.general_purpose_registers[1], STORED + 1);
    assert_eq!(read_word(&memory, address), STORED + 1);
}

#[test]
fn an_access_across_the_end_of_a_window_reaches_the_next_one() {
    // The later window is given a place first, so the two are not next to each other in the guest memory.
    for (first, second) in [
        (WINDOW, WINDOW - PAGE_SIZE),
        (6 * WINDOW - PAGE_SIZE, 6 * WINDOW),
    ] {
        let (mut executor, memory, mut table) = with_code(&[LOAD, ADD_ONE, STORE]);
        writable(&mut table, first);
        writable(&mut table, second);
        let address = first.max(second) - size_of::<u32>() as u64;
        write_word(&memory, address, STORED);
        let (processor, handler) = run(&mut executor, &memory, address);
        assert_eq!(handler.fault, None, "at {address:#x}");
        assert_eq!(processor.general_purpose_registers[1], STORED + 1);
        assert_eq!(read_word(&memory, address), STORED + 1, "at {address:#x}");
    }
}

#[test]
fn an_access_across_the_end_of_a_window_faults_where_the_next_one_allows_nothing() {
    let (mut executor, memory, mut table) = with_code(&[LOAD]);
    let end = 5 * WINDOW;
    writable(&mut table, end - PAGE_SIZE);
    let (_, handler) = run(&mut executor, &memory, end - size_of::<u32>() as u64);
    assert_eq!(handler.fault, Some((end, Access::Read)));
}

#[test]
fn a_fault_outside_the_pages_given_names_the_address() {
    for address in [
        3 * WINDOW + 2 * PAGE_SIZE,
        0x7777_0000_0000,
        SPACE_END - PAGE_SIZE,
    ] {
        let (mut executor, memory, mut table) = with_code(&[STORE]);
        // A page of the same window is given, so that its window has a place.
        writable(&mut table, address ^ PAGE_SIZE);
        let (_, handler) = run(&mut executor, &memory, address);
        assert_eq!(handler.fault, Some((address, Access::Write)));
    }
}

#[test]
fn an_exclusive_load_keeps_the_address() {
    // A system call clears the monitor, so the load runs as one instruction, as the behavior source runs it.
    let mut executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
    // An address inside its page, so that its place's offset in the page carries over to the address kept.
    let page = 9 * WINDOW + 3 * PAGE_SIZE;
    let address = page + IN_PAGE;
    let mut registers = ZERO_PROCESSOR.general_purpose_registers;
    registers[0] = address;
    let mapped = |data_access| PagePermission::Mapped {
        data_access,
        instruction_fetch: InstructionFetch::Forbidden,
    };
    let before = ExecutionState {
        processor: Processor {
            program_counter: LOWEST_ADDRESS,
            general_purpose_registers: registers,
            ..ZERO_PROCESSOR
        },
        memory_mutations: vec![
            MemoryDelta::PermissionAssignment {
                addresses: LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE,
                permission: PagePermission::Mapped {
                    data_access: DataAccess::ReadOnly,
                    instruction_fetch: InstructionFetch::Allowed,
                },
            },
            MemoryDelta::ValueAssignment {
                start: LOWEST_ADDRESS,
                bytes: Cow::Owned(LOAD_EXCLUSIVE.to_le_bytes().to_vec()),
            },
            MemoryDelta::PermissionAssignment {
                addresses: page..page + PAGE_SIZE,
                permission: mapped(DataAccess::ReadWrite),
            },
        ],
    };
    let after = executor
        .execute_single_instruction(before)
        .expect("a state of the space");
    assert!(
        matches!(after.processor.exclusive_monitor, ExclusiveMonitor::Exclusive { address: kept } if kept == address),
        "{:?}",
        after.processor.exclusive_monitor
    );
}

#[test]
fn single_loads_in_more_windows_than_a_space_holds_each_read_their_word() {
    // Each state uses its own window, one after another, more of them than one space holds at once.
    let mut executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
    for window in 1..=2 * HOST_WINDOWS {
        let address = window * WINDOW + PAGE_SIZE;
        let value = STORED + window;
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
                    addresses: LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE,
                    permission: PagePermission::Mapped {
                        data_access: DataAccess::ReadOnly,
                        instruction_fetch: InstructionFetch::Allowed,
                    },
                },
                MemoryDelta::ValueAssignment {
                    start: LOWEST_ADDRESS,
                    bytes: Cow::Owned(LOAD.to_le_bytes().to_vec()),
                },
                MemoryDelta::PermissionAssignment {
                    addresses: address..address + PAGE_SIZE,
                    permission: PagePermission::Mapped {
                        data_access: DataAccess::ReadOnly,
                        instruction_fetch: InstructionFetch::Forbidden,
                    },
                },
                MemoryDelta::ValueAssignment {
                    start: address,
                    bytes: Cow::Owned(value.to_le_bytes().to_vec()),
                },
            ],
        };
        let after = executor
            .execute_single_instruction(before)
            .expect("a state of the space");
        assert_eq!(
            after.processor.general_purpose_registers[1], value,
            "window {window}"
        );
        assert_eq!(
            after.processor.program_counter,
            LOWEST_ADDRESS + size_of::<u32>() as u64
        );
    }
}

#[test]
fn one_permission_change_across_window_ends_gives_each_window_its_place() {
    // Window 1 and window 3 have no place before the calls, which each reach across a window's end.
    for (end, lower) in [(WINDOW, u64::MIN), (3 * WINDOW, 2)] {
        let (mut executor, memory, mut table) = with_code(&[LOAD]);
        writable_range(&mut table, end - PAGE_SIZE..end + PAGE_SIZE);
        for address in [end - WORD, end, end + PAGE_SIZE - WORD] {
            write_word(&memory, address, address / WORD);
            let (processor, handler) = run(&mut executor, &memory, address);
            assert_eq!(handler.fault, None, "at {address:#x} below window {lower}");
            assert_eq!(processor.general_purpose_registers[1], address / WORD);
        }
        // The page after the range allows nothing.
        let past = end + PAGE_SIZE;
        let (_, handler) = run(&mut executor, &memory, past);
        assert_eq!(handler.fault, Some((past, Access::Read)));
    }
}

#[test]
fn pages_of_window_0_given_and_taken_back_leave_every_other_window_unplaced() {
    // The window table lies just before the permission table, so pages' permission bytes sit beside its entries: those
    // of the first pages and of many pages, given and taken back, leave zeros there.
    let (mut executor, memory, mut table) = with_code(&[LOAD]);
    for pages in [u64::MIN..LOWEST_ADDRESS, 0x1000_0000..0x2000_0000] {
        writable_range(&mut table, pages.clone());
        table
            .set_permissions(pages, Permissions::empty())
            .expect("pages of the space");
    }
    // A page of window 0 with the low bits of each address below is allowed, so landing in window 0 would not fault.
    writable(&mut table, LOW_PAGE);
    for window in [0x4100, 0x5000, 0xFFFF, 1 << 16] {
        let address = window * WINDOW + LOW_PAGE;
        let (_, handler) = run(&mut executor, &memory, address);
        assert_eq!(
            handler.fault,
            Some((address, Access::Read)),
            "window {window:#x}"
        );
    }
}

#[test]
fn addresses_past_the_space_are_refused() {
    let executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    assert_eq!(
        table.set_permissions(SPACE_END..SPACE_END + PAGE_SIZE, Permissions::READ),
        Err(InvalidAddressError)
    );
    assert_eq!(
        table.set_permissions(
            SPACE_END - PAGE_SIZE..SPACE_END + PAGE_SIZE,
            Permissions::READ
        ),
        Err(InvalidAddressError)
    );
    assert!(memory.borrow(SPACE_END - 1..SPACE_END + 1).is_err());
    assert!(memory.borrow(SPACE_END..SPACE_END + 1).is_err());
    // The last page of the space is its own.
    writable(&mut table, SPACE_END - PAGE_SIZE);
    write_word(&memory, SPACE_END - WORD, STORED);
    assert_eq!(read_word(&memory, SPACE_END - WORD), STORED);
}

#[test]
fn bytes_are_copied_and_mapped_between_windows() {
    let executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
    let (from, mut from_table) = executor.create_guest_memory().expect("a space");
    let (copy, mut copy_table) = executor.create_guest_memory().expect("a space");
    let (mapped, mut mapped_table) = executor.create_guest_memory().expect("a space");
    let top = SPACE_END - PAGE_SIZE;
    writable(&mut from_table, top);
    write_word(&from, top, STORED);
    copy_table
        .copy_from(4 * WINDOW..4 * WINDOW + PAGE_SIZE, &from_table, top)
        .expect("a copy");
    assert_eq!(read_word(&copy, 4 * WINDOW), STORED);
    mapped_table
        .map_from(PAGE_SIZE * 16..PAGE_SIZE * 17, &from_table, top)
        .expect("a mapping");
    write_word(&mapped, PAGE_SIZE * 16, STORED + 1);
    assert_eq!(read_word(&from, top), STORED + 1, "a mapped page is shared");
    assert_eq!(read_word(&copy, 4 * WINDOW), STORED, "a copied page is not");
}

#[test]
fn a_space_holds_its_most_windows_and_refuses_one_more() {
    let executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    for window in 1..HOST_WINDOWS {
        writable(&mut table, window * WINDOW);
    }
    let one_more = HOST_WINDOWS * WINDOW;
    assert_eq!(
        table.set_permissions(one_more..one_more + PAGE_SIZE, Permissions::READ),
        Err(InvalidAddressError)
    );
    assert!(memory.borrow(one_more..one_more + 1).is_err());
    // Taking every permission back from a window it never gave a place is allowed.
    assert_eq!(table.unmap_from(one_more..one_more + PAGE_SIZE), Ok(()));
}

#[test]
fn an_address_at_or_above_two_to_the_48_faults_at_that_address() {
    // A 48-bit virtual address has no bits 48 to 55 (the top byte is ignored), so these are outside the space even
    // though their low 32 bits name a page window 0 allows.
    for high in [1 << 48, 2 << 48, 1 << 55, 0xFF << 48] {
        let address = high | LOW_PAGE;
        for (code, access) in [(LOAD, Access::Read), (STORE, Access::Write)] {
            let (mut executor, memory, mut table) = with_code(&[code]);
            writable(&mut table, LOW_PAGE);
            let (_, handler) = run(&mut executor, &memory, address);
            assert_eq!(handler.fault, Some((address, access)), "at {address:#x}");
        }
    }
}

#[test]
fn a_window_never_given_a_place_faults_at_the_address() {
    // Only a page of window 0 with the same low 32 bits is allowed; window 0x5000 has nothing.
    let address = 0x5000_0000_0000 | LOW_PAGE;
    for (code, access) in [(LOAD, Access::Read), (STORE, Access::Write)] {
        let (mut executor, memory, mut table) = with_code(&[code]);
        writable(&mut table, LOW_PAGE);
        write_word(&memory, LOW_PAGE, STORED);
        let (_, handler) = run(&mut executor, &memory, address);
        assert_eq!(handler.fault, Some((address, access)));
        assert_eq!(
            read_word(&memory, LOW_PAGE),
            STORED,
            "window 0 is unchanged"
        );
    }
}

#[test]
fn a_load_in_window_one_past_its_first_64_kib_reads_its_own_word() {
    let (mut executor, memory, mut table) = with_code(&[LOAD]);
    writable_range(&mut table, WINDOW..WINDOW + FILL);
    fill(&memory, WINDOW);
    let address = WINDOW + PAST_SPILL;
    let (processor, handler) = run(&mut executor, &memory, address);
    assert_eq!(handler.fault, None);
    assert_eq!(processor.general_purpose_registers[1], address / WORD);
}

#[test]
fn two_loads_in_window_one_past_its_first_64_kib_read_their_own_words() {
    let (mut executor, memory, mut table) = with_code(&[LOAD, LOAD_NEXT]);
    writable_range(&mut table, WINDOW..WINDOW + FILL);
    fill(&memory, WINDOW);
    let address = WINDOW + PAST_SPILL;
    let (processor, handler) = run(&mut executor, &memory, address);
    assert_eq!(handler.fault, None);
    assert_eq!(processor.general_purpose_registers[1], address / WORD);
    assert_eq!(processor.general_purpose_registers[2], address / WORD + 1);
}

#[test]
fn a_store_in_window_one_past_its_first_64_kib_writes_its_own_word() {
    let (mut executor, memory, mut table) = with_code(&[STORE]);
    writable_range(&mut table, WINDOW..WINDOW + FILL);
    fill(&memory, WINDOW);
    let address = WINDOW + PAST_SPILL;
    let (_, handler) = run(&mut executor, &memory, address);
    assert_eq!(handler.fault, None);
    // X1 is zero at the start, so the word becomes zero and the word 64 KiB lower keeps its own address.
    assert_eq!(read_word(&memory, address), u64::MIN);
    let lower = address - SPILL_STEP;
    assert_eq!(read_word(&memory, lower), lower / WORD);
}

#[test]
fn a_loop_in_window_one_past_its_first_64_kib_sums_its_own_words() {
    let (mut executor, memory, mut table) =
        with_code(&[LOAD_AND_STEP, ADD_TO_SUM, COUNT_DOWN, BACK_THREE]);
    writable_range(&mut table, WINDOW..WINDOW + FILL);
    fill(&memory, WINDOW);
    let address = WINDOW + PAST_SPILL;
    let count = PAGE_SIZE;
    let (processor, handler) = run_counted(&mut executor, &memory, address, count);
    assert_eq!(handler.fault, None);
    let first = address / WORD;
    assert_eq!(
        processor.general_purpose_registers[3],
        (first..first + count).sum::<u64>()
    );
}

#[test]
fn a_loop_in_window_one_stepping_64_kib_or_more_sums_its_own_words() {
    // A step of 64 KiB takes an address as computed to the place of the address before it, and one of 68 KiB past it.
    for (step, step_bytes) in [(STEP_SPILL, SPILL_STEP), (STEP_PAST_SPILL, PAST_SPILL_STEP)] {
        let (mut executor, memory, mut table) =
            with_code(&[LOAD, ADD_TO_SUM, step, COUNT_DOWN, BACK_FOUR]);
        writable_range(&mut table, WINDOW..WINDOW + FILL);
        fill(&memory, WINDOW);
        let address = WINDOW + SPILL_STEP;
        let count = (FILL - SPILL_STEP) / step_bytes;
        let (processor, handler) = run_counted(&mut executor, &memory, address, count);
        assert_eq!(handler.fault, None, "a step of {step_bytes:#x}");
        assert_eq!(
            processor.general_purpose_registers[3],
            (0..count)
                .map(|index| (address + index * step_bytes) / WORD)
                .sum::<u64>(),
            "a step of {step_bytes:#x}"
        );
    }
}

#[test]
fn a_borrow_across_a_window_end_past_the_64_kib_after_it_holds_each_address_once() {
    let half = FILL / 2;
    let start = WINDOW - half;
    // Window 1 is given its place first, then the end of window 0.
    for (written_whole, read_whole) in [(true, false), (false, true)] {
        let (mut executor, memory, mut table) = with_code(&[LOAD]);
        writable_range(&mut table, WINDOW..WINDOW + half);
        writable_range(&mut table, start..WINDOW);
        if written_whole {
            fill(&memory, start);
        } else {
            for part in [start, WINDOW] {
                let words: Vec<u8> = (part / WORD..(part + half) / WORD)
                    .flat_map(u64::to_le_bytes)
                    .collect();
                let bytes = memory
                    .borrow(part..part + half)
                    .expect("one window's addresses");
                assert_eq!(bytes.write(&words), words.len());
            }
        }
        let mut whole = vec![u8::MIN; usize::try_from(FILL).expect("a length")];
        if read_whole {
            let bytes = memory.borrow(start..start + FILL).expect("the addresses");
            assert_eq!(bytes.read(&mut whole), whole.len());
        }
        for address in [
            WINDOW - WORD,
            WINDOW,
            WINDOW + SPILL_STEP,
            WINDOW + PAST_SPILL + SPILL_STEP,
        ] {
            let expected = address / WORD;
            if read_whole {
                let at = usize::try_from(address - start).expect("an offset");
                let word: [u8; size_of::<u64>()] =
                    whole[at..at + size_of::<u64>()].try_into().expect("a word");
                assert_eq!(
                    u64::from_le_bytes(word),
                    expected,
                    "read whole at {address:#x}"
                );
            } else {
                assert_eq!(read_word(&memory, address), expected, "at {address:#x}");
                let (processor, handler) = run(&mut executor, &memory, address);
                assert_eq!(handler.fault, None);
                assert_eq!(
                    processor.general_purpose_registers[1], expected,
                    "loaded at {address:#x}"
                );
            }
        }
    }
}
