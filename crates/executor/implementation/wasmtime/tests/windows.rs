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
    let mut registers = ZERO_PROCESSOR.general_purpose_registers;
    registers[0] = address;
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
    let address = 9 * WINDOW + 3 * PAGE_SIZE;
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
                addresses: address..address + PAGE_SIZE,
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
