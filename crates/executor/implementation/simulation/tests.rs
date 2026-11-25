use std::borrow::Cow;
use std::future::Future;
use std::pin::pin;
use std::sync::OnceLock;
use std::task::{Context, Poll, Waker};
use std::thread;

use wcontainers_abi::{Access, LOWEST_ADDRESS, PAGE_SIZE, Permissions, SystemCallRequest, WORD};
use wcontainers_arm64::encode::{self, Register, form};
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_executor::{
    ExecutionInterrupter, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_precise_instruction_set_apple_m5::AppleM5;
use wcontainers_processor_architecture::MemoryDelta;
use wcontainers_processor_architecture_arm64::memory::read_bytes;
use wcontainers_processor_architecture_arm64::syndrome::UNDEFINED_INSTRUCTION;
use wcontainers_processor_architecture_arm64::{INSTRUCTION_BYTES, Processor, ZERO_PROCESSOR};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::memory::{SimulatedMemory, SimulatedPageTable, SpaceCopy};
use crate::{Simulation, SimulationInterrupter};

const PAGE: u64 = LOWEST_ADDRESS;
/// The page the interrupt tests' guest stores to, which allows nothing until the guest faults on it.
const DATA: u64 = PAGE + PAGE_SIZE;
const ADDRESS_REGISTER: Register = 0;
const VALUE_REGISTER: Register = ADDRESS_REGISTER + 1;
const STORED: u64 = u64::MAX;
/// The bytes of a change shorter than a word, which a test places across the end of a page.
const SHORT: u64 = size_of::<u32>() as u64;

#[test]
fn word_no_listed_instruction_matches_is_undefined() {
    let mut chip = Simulation::new(AppleM5);
    let (memory, mut table) = SimulatedMemory::new();
    let allowed = Permissions::READ | Permissions::EXECUTE;
    assert_eq!(
        table.set_permissions(PAGE..PAGE + PAGE_SIZE, allowed),
        Ok(())
    );
    let start = Processor {
        program_counter: PAGE,
        ..ZERO_PROCESSOR
    };
    let mut copy = SpaceCopy::default();
    memory.refresh(&mut copy);
    let mut stopped = start.clone();
    chip.execute_single_instruction_in_place(&mut stopped, copy.deltas(), |_| {});
    assert_eq!(
        stopped,
        Processor {
            exception: Some(UNDEFINED_INSTRUCTION),
            ..start
        }
    );
}

#[test]
fn page_methods_refuse_addresses_inside_a_page() {
    let (_memory, mut table) = SimulatedMemory::new();
    let (_other, other_table) = SimulatedMemory::new();
    let inside = PAGE + WORD..PAGE + PAGE_SIZE;
    assert_eq!(
        table.set_permissions(inside.clone(), Permissions::READ),
        Err(InvalidAddressError)
    );
    assert_eq!(
        table.map_from(inside.clone(), &other_table, PAGE + WORD),
        Err(MapMemoryError::InvalidAddress(InvalidAddressError))
    );
    assert_eq!(table.unmap_from(inside.clone()), Err(InvalidAddressError));
    assert_eq!(
        table.copy_from(PAGE..PAGE + PAGE_SIZE, &other_table, PAGE + WORD),
        Err(CopyMemoryError::InvalidAddress(InvalidAddressError))
    );
}

#[test]
fn unmap_from_leaves_a_page_no_mapping_shares() {
    let (memory, mut table) = SimulatedMemory::new();
    let written = u64::MAX.to_le_bytes();
    let bytes = memory.borrow(PAGE..PAGE + WORD).expect("inside the space");
    assert_eq!(bytes.write(&written), written.len());
    assert_eq!(table.unmap_from(PAGE..PAGE + PAGE_SIZE), Ok(()));
    let mut read = [u8::MIN; WORD as usize];
    assert_eq!(bytes.read(&mut read), read.len());
    assert_eq!(read, written);
}

#[test]
fn unmap_from_leaves_a_page_another_space_maps() {
    let (owner, mut owner_table) = SimulatedMemory::new();
    let (other, mut other_table) = SimulatedMemory::new();
    let written = u64::MAX.to_le_bytes();
    let whole = PAGE..PAGE + PAGE_SIZE;
    let bytes = owner.borrow(PAGE..PAGE + WORD).expect("inside the space");
    assert_eq!(bytes.write(&written), written.len());
    assert_eq!(other_table.map_from(whole.clone(), &owner_table, PAGE), Ok(()));
    assert_eq!(owner_table.unmap_from(whole), Ok(()));
    let mut read = [u8::MIN; WORD as usize];
    assert_eq!(bytes.read(&mut read), read.len());
    assert_eq!(read, written);
    let mapped = other.borrow(PAGE..PAGE + WORD).expect("inside the space");
    assert_eq!(mapped.read(&mut read), read.len());
    assert_eq!(read, written);
}

#[test]
fn copy_follows_a_write_through_another_space_that_maps_the_page() {
    let (owner, owner_table) = SimulatedMemory::new();
    let (other, mut other_table) = SimulatedMemory::new();
    let whole = PAGE..PAGE + PAGE_SIZE;
    assert_eq!(other_table.map_from(whole, &owner_table, PAGE), Ok(()));
    let mut copy = SpaceCopy::default();
    owner.refresh(&mut copy);
    let written = u64::MAX.to_le_bytes();
    let mapped = other.borrow(PAGE..PAGE + WORD).expect("inside the space");
    assert_eq!(mapped.write(&written), written.len());
    owner.refresh(&mut copy);
    assert_eq!(
        read_bytes::<{ WORD as usize }>(copy.deltas(), PAGE),
        written
    );
}

#[test]
fn copy_holds_the_changes_an_instruction_applied() {
    let (memory, _table) = SimulatedMemory::new();
    let mut copy = SpaceCopy::default();
    memory.refresh(&mut copy);
    let first = u64::MAX.to_le_bytes();
    let second = u32::MAX.to_le_bytes();
    // The second change crosses into the next page, which the copy holds no bytes of yet.
    let crossing = PAGE + PAGE_SIZE - SHORT;
    for (start, bytes) in [(PAGE, &first[..]), (crossing, &second[..])] {
        let delta = MemoryDelta::ValueAssignment {
            start,
            bytes: Cow::Borrowed(bytes),
        };
        memory.apply(delta, &mut copy);
    }
    memory.refresh(&mut copy);
    assert_eq!(read_bytes::<{ WORD as usize }>(copy.deltas(), PAGE), first);
    assert_eq!(
        read_bytes::<{ SHORT as usize }>(copy.deltas(), crossing),
        second
    );
    let mut read = [u8::MIN; SHORT as usize];
    let bytes = memory
        .borrow(crossing..crossing + SHORT)
        .expect("inside the space");
    assert_eq!(bytes.read(&mut read), read.len());
    assert_eq!(read, second);
}

#[test]
fn interrupt_raised_while_running_answered_terminate_stops_before_the_next_instruction() {
    let (memory, mut table) = SimulatedMemory::new();
    let start = place_store_guest(&memory, &mut table);
    let raised = OnceLock::new();
    let mut handler = InterruptingAtFault::new(&mut table, &raised, true);
    let mut chip = Simulation::new(AppleM5);
    let (execution, interrupter) = chip.execute(start.clone(), &memory, &mut handler);
    assert!(
        raised.set(interrupter).is_ok(),
        "the interrupter is set once"
    );
    let stopped = poll_once(execution);
    assert_eq!(
        (handler.faults, handler.interrupts),
        (1, 1),
        "the faults and interrupts the handler was given"
    );
    assert_eq!(stopped, start, "the state execute returned");
    assert_eq!(
        read_word(&memory, DATA),
        [u8::MIN; WORD as usize],
        "the bytes the store left"
    );
}

#[test]
fn interrupt_raised_while_running_answered_continue_changes_nothing() {
    let (memory, mut table) = SimulatedMemory::new();
    let start = place_store_guest(&memory, &mut table);
    let raised = OnceLock::new();
    let mut handler = InterruptingAtFault::new(&mut table, &raised, false);
    let mut chip = Simulation::new(AppleM5);
    let (execution, interrupter) = chip.execute(start.clone(), &memory, &mut handler);
    assert!(
        raised.set(interrupter).is_ok(),
        "the interrupter is set once"
    );
    let stopped = poll_once(execution);
    assert_eq!(
        (handler.faults, handler.interrupts),
        (1, 1),
        "the faults and interrupts the handler was given"
    );
    let breakpoint = start.program_counter + INSTRUCTION_BYTES;
    assert_eq!(
        stopped.program_counter, breakpoint,
        "the address execute stopped at"
    );
    assert_eq!(
        read_word(&memory, DATA),
        STORED.to_le_bytes(),
        "the bytes the store left"
    );
}

/// The handler of the interrupt tests: at the first memory fault it raises its execution's interrupt from another
/// thread, then allows the page and retries.
struct InterruptingAtFault<'test> {
    table: &'test mut SimulatedPageTable,
    raised: &'test OnceLock<SimulationInterrupter>,
    /// Whether the interrupt terminates the guest; otherwise it continues.
    terminate: bool,
    faults: usize,
    interrupts: usize,
}

impl<'test> InterruptingAtFault<'test> {
    fn new(
        table: &'test mut SimulatedPageTable,
        raised: &'test OnceLock<SimulationInterrupter>,
        terminate: bool,
    ) -> Self {
        Self {
            table,
            raised,
            terminate,
            faults: usize::MIN,
            interrupts: usize::MIN,
        }
    }
}

impl TrapHandler for InterruptingAtFault<'_> {
    fn handle_system_call(&mut self, _request: SystemCallRequest) -> SystemCallResponse {
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        self.faults += 1;
        assert_eq!((address, access), (DATA, Access::Write), "the fault");
        let interrupter = self
            .raised
            .get()
            .expect("the interrupter is set before the first poll");
        thread::scope(|scope| {
            scope.spawn(|| interrupter.interrupt());
        });
        let allowed = Permissions::READ | Permissions::WRITE;
        assert_eq!(
            self.table.set_permissions(DATA..DATA + PAGE_SIZE, allowed),
            Ok(())
        );
        MemoryFaultResponse::Retry
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.interrupts += 1;
        if self.terminate {
            InterruptResponse::Terminate
        } else {
            InterruptResponse::Continue
        }
    }
}

/// Places a store to the data page followed by a breakpoint, and returns the state that runs it.
fn place_store_guest(memory: &SimulatedMemory, table: &mut SimulatedPageTable) -> Processor {
    let store = encode::load_store(form::STR, VALUE_REGISTER, ADDRESS_REGISTER, u64::MIN, WORD)
        .expect("a store with no offset encodes");
    let breakpoint = encode::exception(form::BRK, u16::MIN);
    let code: Vec<u8> = [store, breakpoint]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    let bytes = memory
        .borrow(PAGE..PAGE + code.len() as u64)
        .expect("inside the space");
    assert_eq!(bytes.write(&code), code.len());
    let allowed = Permissions::READ | Permissions::EXECUTE;
    assert_eq!(
        table.set_permissions(PAGE..PAGE + PAGE_SIZE, allowed),
        Ok(())
    );
    let mut start = Processor {
        program_counter: PAGE,
        ..ZERO_PROCESSOR
    };
    start.general_purpose_registers[usize::from(ADDRESS_REGISTER)] = DATA;
    start.general_purpose_registers[usize::from(VALUE_REGISTER)] = STORED;
    start
}

fn read_word(memory: &SimulatedMemory, start: u64) -> [u8; WORD as usize] {
    let mut read = [u8::MIN; WORD as usize];
    let bytes = memory
        .borrow(start..start + WORD)
        .expect("inside the space");
    assert_eq!(bytes.read(&mut read), read.len());
    read
}

/// Polls the execution once, which for the simulation runs it to its end.
fn poll_once<Execution: Future>(execution: Execution) -> Execution::Output {
    let mut context = Context::from_waker(Waker::noop());
    match pin!(execution).poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the simulation's execution ran to its end in its first poll"),
    }
}
