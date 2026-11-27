use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::OnceLock;
use std::thread;

use wcontainers_abi::{Access, LOWEST_ADDRESS, PAGE_SIZE, Permissions, SystemCallRequest, WORD};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_processor_architecture::{MemoryDelta, MemorySnapshot};
use wcontainers_processor_architecture_arm64::encoding::{RN, RT};
use wcontainers_processor_architecture_arm64::memory::read_bytes;
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, Exception, INSTRUCTION_SIZE, Instruction, Processor,
    ProcessorExpectation, User, ZERO_PROCESSOR,
};
use wcontainers_random_source::RandomSource;
use wcontainers_random_source_seeded::SeededRandomSource;
use wcontainers_volatile_bytes::VolatileBytes;

use crate::memory::{SpaceCopy, apply, create_space, refresh};
use crate::testing::{FirstBehaviors, SEED, list_first_behaviors, seed_random};
use crate::{SimulatedMemory, SimulatedPageTable};
use crate::{Simulation, SimulationInterrupter};

const PAGE: u64 = LOWEST_ADDRESS;
/// The page the interrupt tests' guest stores to, which allows nothing until the guest faults on it.
const DATA: u64 = PAGE + PAGE_SIZE;
const STORE_ADDRESS_REGISTER: u32 = 0;
const STORED_VALUE_REGISTER: u32 = STORE_ADDRESS_REGISTER + 1;
const STORED: u64 = u64::MAX;
/// The bytes of a change shorter than a word, which a test places across the end of a page.
const SHORT: u64 = size_of::<u32>() as u64;

/// Returns a chip that runs the first known behavior of each instruction.
fn simulate_first_behaviors() -> Simulation<FirstBehaviors, SeededRandomSource> {
    Simulation::new(list_first_behaviors(), seed_random(SEED))
}

#[test]
fn word_no_listed_instruction_matches_is_undefined() {
    let mut chip = simulate_first_behaviors();
    let (memory, mut table) = create_space();
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
    refresh(&memory, &mut copy);
    let mut stopped = start.clone();
    let memory = MemorySnapshot {
        mutations: copy.deltas(),
    };
    let executed = chip.execute_single_instruction_in_place(&mut stopped, memory, &mut Vec::new());
    assert!(
        matches!(executed, Ok(())),
        "the simulation gave {executed:?}"
    );
    assert_eq!(
        stopped,
        Processor {
            exception: Some(Exception::UndefinedInstruction),
            ..start
        }
    );
}

#[test]
fn undecided_values_come_from_the_random_source() {
    let chip = simulate_first_behaviors();
    let undecided = 5;
    let mut expectation = ProcessorExpectation::new(ZERO_PROCESSOR);
    expectation.uncompared.general_purpose_registers[undecided] = u64::MAX;
    expectation.uncompared.condition_flags = ConditionFlags::all();
    let same_seed = seed_random(SEED);
    let mut expected = ZERO_PROCESSOR;
    expected.general_purpose_registers[undecided] = read_random_word(&same_seed);
    let [_, _, _, _, high @ ..] = read_random_word(&same_seed).to_le_bytes();
    expected.condition_flags = ConditionFlags::from_bits_truncate(u32::from_le_bytes(high));
    let Ok(chosen) = chip.choose_processor(expectation.clone()) else {
        panic!("the seeded source gives every word");
    };
    assert_eq!(chosen, expected, "the values the seed's words give");
    let other = Simulation::new(chip.set, seed_random(SEED + 1));
    assert_ne!(
        other.choose_processor(expectation).ok(),
        Some(chosen),
        "two seeds chose the same values"
    );
}

/// Returns the source's next word.
fn read_random_word(source: &SeededRandomSource) -> u64 {
    let mut word = [0; size_of::<u64>()];
    assert_eq!(
        source.fill_random_bytes(&RefCell::new(word.as_mut_slice())),
        Ok(())
    );
    u64::from_le_bytes(word)
}

#[test]
fn page_methods_refuse_addresses_inside_a_page() {
    let (_memory, mut table) = create_space();
    let (_other, other_table) = create_space();
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
    let (memory, mut table) = create_space();
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
    let (owner, mut owner_table) = create_space();
    let (other, mut other_table) = create_space();
    let written = u64::MAX.to_le_bytes();
    let whole = PAGE..PAGE + PAGE_SIZE;
    let bytes = owner.borrow(PAGE..PAGE + WORD).expect("inside the space");
    assert_eq!(bytes.write(&written), written.len());
    assert_eq!(
        other_table.map_from(whole.clone(), &owner_table, PAGE),
        Ok(())
    );
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
    let (owner, owner_table) = create_space();
    let (other, mut other_table) = create_space();
    let whole = PAGE..PAGE + PAGE_SIZE;
    assert_eq!(other_table.map_from(whole, &owner_table, PAGE), Ok(()));
    let mut copy = SpaceCopy::default();
    refresh(&owner, &mut copy);
    let written = u64::MAX.to_le_bytes();
    let mapped = other.borrow(PAGE..PAGE + WORD).expect("inside the space");
    assert_eq!(mapped.write(&written), written.len());
    refresh(&owner, &mut copy);
    assert_eq!(
        read_bytes::<Arm64<User>, { WORD as usize }>(copy.deltas(), PAGE),
        written
    );
}

#[test]
fn copy_holds_the_changes_an_instruction_applied() {
    let (memory, _table) = create_space();
    let mut copy = SpaceCopy::default();
    refresh(&memory, &mut copy);
    let first = u64::MAX.to_le_bytes();
    let second = u32::MAX.to_le_bytes();
    // The second change crosses into the next page, which the copy holds no bytes of yet.
    let crossing = PAGE + PAGE_SIZE - SHORT;
    for (start, bytes) in [(PAGE, &first[..]), (crossing, &second[..])] {
        let delta = MemoryDelta::ValueAssignment {
            start,
            bytes: Cow::Borrowed(bytes),
        };
        apply(&memory, delta, &mut copy);
    }
    refresh(&memory, &mut copy);
    assert_eq!(
        read_bytes::<Arm64<User>, { WORD as usize }>(copy.deltas(), PAGE),
        first
    );
    assert_eq!(
        read_bytes::<Arm64<User>, { SHORT as usize }>(copy.deltas(), crossing),
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
    let (memory, mut table) = create_space();
    let start = place_store_guest(&memory, &mut table);
    let raised = OnceLock::new();
    let mut handler = InterruptingAtFault::new(&mut table, &raised, true);
    let mut chip = simulate_first_behaviors();
    let (execution, interrupter) = chip.prepare_execution(start.clone(), &memory);
    assert!(
        raised.set(interrupter).is_ok(),
        "the interrupter is set once"
    );
    let stopped = run_to_the_end(execution, &mut handler);
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
    let (memory, mut table) = create_space();
    let start = place_store_guest(&memory, &mut table);
    let raised = OnceLock::new();
    let mut handler = InterruptingAtFault::new(&mut table, &raised, false);
    let mut chip = simulate_first_behaviors();
    let (execution, interrupter) = chip.prepare_execution(start.clone(), &memory);
    assert!(
        raised.set(interrupter).is_ok(),
        "the interrupter is set once"
    );
    let stopped = run_to_the_end(execution, &mut handler);
    assert_eq!(
        (handler.faults, handler.interrupts),
        (1, 1),
        "the faults and interrupts the handler was given"
    );
    let breakpoint = start.program_counter + INSTRUCTION_SIZE;
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

/// The handler of the interrupt tests, which raises the interrupt from another thread at the first memory fault.
///
/// It then allows the page and retries.
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
            .expect("the interrupter is set before the execution runs");
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
fn place_store_guest(memory: &SimulatedMemory, table: &mut SimulatedPageTable) -> Processor<()> {
    let store = Instruction::StrImmediateUnsignedOffset64
        .to_fixed_bits()
        .value
        | RT.to_bits(STORED_VALUE_REGISTER)
        | RN.to_bits(STORE_ADDRESS_REGISTER);
    let breakpoint = Instruction::Brk.to_fixed_bits().value;
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
    start.general_purpose_registers[STORE_ADDRESS_REGISTER as usize] = DATA;
    start.general_purpose_registers[STORED_VALUE_REGISTER as usize] = STORED;
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

/// Runs the execution, which for the simulation ends without waiting, and returns the state it ended at.
fn run_to_the_end(
    execution: impl Execution<ProcessorState = Processor<()>>,
    handler: &mut impl TrapHandler,
) -> Processor<()> {
    match execution.run(handler) {
        ExecutionStop::Terminated(processor) => processor,
        ExecutionStop::Waiting { .. } => panic!("the simulation's execution waited"),
    }
}
