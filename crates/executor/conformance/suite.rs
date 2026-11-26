//! The tests every implementation of wcontainers-executor runs, written against its traits alone.
//!
//! The guests the suite runs are written for ARM64, the one architecture an executor runs today: the architecture's
//! interface has no way to place a value in a register or to name a system call instruction.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::fmt::{Debug, Write};
use std::ops::Range;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};
use std::time::Duration;

use wcontainers_abi::{
    Access, LOWEST_ADDRESS, PAGE_MASK, PAGE_SIZE, Permissions, SYSTEM_CALL_ARGUMENT_COUNT,
    SystemCallNumber, SystemCallRequest, WORD,
};
pub use wcontainers_behavior_source_conformance;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, PageTableOf, ProcessorStateOf, SystemCallResponse, TrapHandler,
};
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{
    ExecutionState, ExecutionStateExpectation, MemoryDelta, ProcessorArchitecture, Simulator,
};
use wcontainers_volatile_bytes::VolatileBytes;

mod arm64;

/// A guest program: its code, placed at the address its start state names, and the states it starts and ends at.
struct Guest<Architecture: ProcessorArchitecture> {
    code: Vec<u8>,
    start: Architecture::ProcessorState,
    /// The state at the trap that ends the guest, when every trap before it lets it go on.
    finished: Architecture::ProcessorState,
}

/// What stops a guest at its first instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FirstTrap {
    SystemCall,
    MemoryFault { address: u64, access: Access },
}

/// Where each guest's code starts: the lowest address anything is placed at.
const CODE: u64 = LOWEST_ADDRESS;
/// The page each guest's data is in.
const DATA: u64 = CODE + PAGE_SIZE;
/// The page the mapping tests map.
const MAPPED: u64 = DATA + PAGE_SIZE;
/// The end of the pages the suite uses in one space.
const END: u64 = MAPPED + PAGE_SIZE;
/// How far a second space's addresses lie from the first's in the mapping and copying tests: past all of them.
const SHIFT: u64 = END - CODE;
/// Addresses that start inside a page and end inside another, more than a page apart.
const LONG: Range<u64> = CODE + WORD..END - WORD;
/// The guests' system call: a number no call has, and arguments other than zero, [`RETURNED`] and each other.
const REQUEST: SystemCallRequest = SystemCallRequest {
    number: SystemCallNumber(u16::MAX as u64),
    arguments: {
        let mut arguments = [u64::MIN; SYSTEM_CALL_ARGUMENT_COUNT];
        let mut place = usize::MIN;
        while place < SYSTEM_CALL_ARGUMENT_COUNT {
            arguments[place] = (u32::MAX as u64) << place;
            place += 1;
        }
        arguments
    },
};
const RETURNED: u64 = u64::MAX;
/// The value the store guests store and the load guests find.
const STORED: u64 = u32::MAX as u64;
/// How many times the counting guest counts down: millions, so that interrupts from another thread come while it runs.
const COUNT: u64 = u32::MAX as u64 >> u8::BITS;
/// How many times the guest an interrupt ends counts down: seconds of work, so that it ends by the interrupt.
const LONG_COUNT: u64 = u32::MAX as u64;
/// How long the interrupting thread waits between interrupts: one tick of a kernel built with HZ at 1000.
///
/// <https://github.com/torvalds/linux/blob/master/kernel/Kconfig.hz>
const INTERRUPT_PERIOD: Duration = Duration::from_millis(1);
/// The seeds of the patterns a test writes, each pattern other than the others at every byte.
const FIRST_PATTERN: u8 = u8::MIN;
const SECOND_PATTERN: u8 = FIRST_PATTERN + 1;
const THIRD_PATTERN: u8 = SECOND_PATTERN + 1;
const FOURTH_PATTERN: u8 = THIRD_PATTERN + 1;
/// What a list of no memory changes is described as.
const NO_CHANGE: &str = "memory unchanged";
const CHANGE_SEPARATOR: &str = "; ";
const CODE_PERMISSIONS: Permissions = Permissions::READ.union(Permissions::EXECUTE);
const DATA_PERMISSIONS: Permissions = Permissions::READ.union(Permissions::WRITE);

/// How many executors the instruction agreement runs at once, each on a host thread of its own: the share of a build
/// machine one job takes.
pub const AGREEMENT_EXECUTORS: usize = 12;

/// What one instruction's states gave in the instruction agreement.
#[derive(Default)]
struct InstructionOutcome {
    executed: usize,
    refused: usize,
    /// The line of the refusals, when there were any or the instruction was not compared.
    refusal: Option<String>,
    /// The line of the disagreement, when there was one.
    disagreement: Option<String>,
}

/// Checks that on every state the generator makes for every instruction the executor ends where the set says.
///
/// The executor's state must meet the behavior's expectation, memory by the bytes and permissions it ends with, not
/// the list of changes: an executor reports a store of the bytes already there as no change, where a simulation may.
///
/// A state the executor gives an error for is its refusal: counted and listed on the standard error, and not compared.
/// The instructions are shared out among [`AGREEMENT_EXECUTORS`] executors `make_executor` makes.
///
/// An instruction `set_aside` names, with the measurement that shows the executor is wrong there, is run and listed on
/// the standard error but does not fail the test; one that agrees fails it, so the list holds only real faults.
pub fn agrees_with_precise_instruction_set<Chip, Set, Generator>(
    make_executor: impl Fn() -> Chip + Sync,
    set: Set,
    generator: Generator,
    set_aside: &[(
        <Chip::Architecture as ProcessorArchitecture>::Instruction,
        &str,
    )],
) where
    Chip: Executor,
    <Chip::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Chip::Architecture, Error = Infallible>,
    <Chip::Architecture as ProcessorArchitecture>::ProcessorState: Debug,
    <Chip::Architecture as ProcessorArchitecture>::ProcessorStateExpectation: Debug,
    <Chip::Architecture as ProcessorArchitecture>::Instruction: Debug + PartialEq + Send + Sync,
    <Chip::Architecture as ProcessorArchitecture>::PagePermission: Debug,
    Set: PreciseInstructionSet<Chip::Architecture> + Sync,
    Generator: DistinguishingStateGenerator<Chip::Architecture> + Sync,
{
    let instructions: Vec<_> = Chip::Architecture::list_instructions().collect();
    for (instruction, _) in set_aside {
        assert!(
            instructions.contains(instruction),
            "{instruction:?} is set aside but not listed"
        );
    }
    let mut outcomes: Vec<(usize, InstructionOutcome)> = thread::scope(|scope| {
        let runs: Vec<_> = (0..AGREEMENT_EXECUTORS)
            .map(|share| {
                let (instructions, set, generator, make_executor) =
                    (&instructions, &set, &generator, &make_executor);
                scope.spawn(move || {
                    let mut executor = make_executor();
                    instructions
                        .iter()
                        .enumerate()
                        .skip(share)
                        .step_by(AGREEMENT_EXECUTORS)
                        .map(|(index, instruction)| {
                            let outcome =
                                agree_on_instruction(&mut executor, set, generator, *instruction);
                            (index, outcome)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        runs.into_iter()
            .flat_map(|run| run.join().expect("an agreement run ends"))
            .collect()
    });
    outcomes.sort_by_key(|(index, _)| *index);
    let mut report = String::new();
    let mut refusals = String::new();
    let mut set_aside_report = String::new();
    let mut executed_states = usize::MIN;
    let mut refused_states = usize::MIN;
    let mut refusing_instructions = usize::MIN;
    let mut disagreeing_instructions = usize::MIN;
    for (index, outcome) in outcomes {
        executed_states += outcome.executed;
        if outcome.refused > usize::MIN {
            refused_states += outcome.refused;
            refusing_instructions += 1;
        }
        if let Some(line) = outcome.refusal {
            let _ = writeln!(refusals, "{line}");
        }
        let measurement = set_aside
            .iter()
            .find(|(instruction, _)| *instruction == instructions[index])
            .map(|(_, measurement)| measurement);
        match (outcome.disagreement, measurement) {
            (Some(line), Some(measurement)) => {
                let _ = writeln!(set_aside_report, "{line}\nset aside: {measurement}");
            }
            (Some(line), None) => {
                disagreeing_instructions += 1;
                let _ = writeln!(report, "{line}");
            }
            (None, Some(_)) => {
                let _ = writeln!(
                    report,
                    "{:?} is set aside but agrees on every state it executed",
                    instructions[index]
                );
            }
            (None, None) => {}
        }
    }
    eprintln!(
        "{executed_states} states executed; {refused_states} refused, of {refusing_instructions} instructions:\n{refusals}"
    );
    eprintln!(
        "{} instructions set aside, where the executor is wrong:\n{set_aside_report}",
        set_aside.len()
    );
    eprintln!("{disagreeing_instructions} instructions disagree");
    assert!(
        executed_states > usize::MIN,
        "the executor executed no state"
    );
    assert!(report.is_empty(), "{report}");
}

/// Runs every state the generator makes for the instruction on the executor and compares it with the set's behavior.
fn agree_on_instruction<Chip, Set, Generator>(
    executor: &mut Chip,
    set: &Set,
    generator: &Generator,
    instruction: <Chip::Architecture as ProcessorArchitecture>::Instruction,
) -> InstructionOutcome
where
    Chip: Executor,
    <Chip::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Chip::Architecture, Error = Infallible>,
    <Chip::Architecture as ProcessorArchitecture>::ProcessorState: Debug,
    <Chip::Architecture as ProcessorArchitecture>::ProcessorStateExpectation: Debug,
    <Chip::Architecture as ProcessorArchitecture>::Instruction: Debug,
    <Chip::Architecture as ProcessorArchitecture>::PagePermission: Debug,
    Set: PreciseInstructionSet<Chip::Architecture>,
    Generator: DistinguishingStateGenerator<Chip::Architecture>,
{
    // An instruction the architecture knows no behavior of has none in any set to agree with.
    if Chip::Architecture::list_known_behaviors(instruction)
        .next()
        .is_none()
    {
        return InstructionOutcome {
            refusal: Some(format!("{instruction:?}: no known behavior, not compared")),
            ..InstructionOutcome::default()
        };
    }
    let precise = set.to_precise(instruction);
    let mut outcome = InstructionOutcome::default();
    let mut states = usize::MIN;
    let mut disagreeing = usize::MIN;
    let mut first = None;
    for before in generator.generate_states(instruction) {
        states += 1;
        let Ok(simulated) = precise.simulate_state(before.clone());
        let Ok(executed) = executor.execute_single_instruction(before.clone()) else {
            outcome.refused += 1;
            continue;
        };
        outcome.executed += 1;
        if !executed.meets_expectations(&simulated) {
            disagreeing += 1;
            first.get_or_insert_with(|| describe(&before, &simulated, &executed));
        }
    }
    assert!(
        states > usize::MIN,
        "the generator gives no state for {instruction:?}"
    );
    if outcome.refused > usize::MIN {
        outcome.refusal = Some(format!(
            "{instruction:?}: the executor refused {} of {states} states",
            outcome.refused
        ));
    }
    outcome.disagreement = first.map(|first| {
        format!("{instruction:?}: {disagreeing} of {states} states disagree; the first:\n{first}")
    });
    outcome
}

/// Checks that an executor, its guest memory, its page table and its interrupter may move to another thread, as the
/// kernel's runners move them: the bounds fail the build of an executor that may not, and a guest then runs there.
pub fn executor_and_its_memory_move_between_threads<Chip>(executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip> + Send,
    Chip::GuestMemory: Send,
    PageTableOf<Chip>: Send,
    Chip::Interrupter: Send,
{
    let (memory, mut table) = create_memory(&executor);
    let store = arm64::build_store(CODE, DATA, STORED);
    let (stopped, finished) = thread::scope(|scope| {
        scope
            .spawn(move || {
                let mut executor = executor;
                place(&memory, &mut table, &store);
                assert_eq!(table.set_permissions(page(DATA), DATA_PERMISSIONS), Ok(()));
                let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
                let stopped = execute(&mut executor, store.start.clone(), &memory, &mut handler);
                handler.assert_no_trap();
                assert_eq!(
                    read_bytes(&memory, DATA..DATA + WORD),
                    STORED.to_le_bytes(),
                    "the bytes the store on another thread left"
                );
                (stopped, store.finished)
            })
            .join()
            .expect("the executor ran on another thread")
    });
    assert_eq!(
        stopped, finished,
        "the state the store on another thread returned"
    );
}

/// Checks that a system call answered with a value goes on after the call with the value in the result register.
pub fn system_call_answered_return_goes_on_with_the_value<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (memory, mut table) = create_memory(&executor);
    let guest = arm64::build_system_call(CODE, REQUEST, RETURNED);
    place(&memory, &mut table, &guest);
    let mut handler = Recorder::new(Some(RETURNED), None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, guest.start.clone(), &memory, &mut handler);
    assert_eq!(
        handler.system_calls,
        [REQUEST],
        "the system calls the handler was given"
    );
    assert!(
        handler.memory_faults.is_empty(),
        "the handler was given memory faults: {:?}",
        handler.memory_faults
    );
    assert_eq!(stopped, guest.finished, "the state execute returned");
}

/// Checks that a system call answered terminate ends execute at the call.
pub fn system_call_answered_terminate_stops_at_the_call<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (memory, mut table) = create_memory(&executor);
    let guest = arm64::build_system_call(CODE, REQUEST, RETURNED);
    place(&memory, &mut table, &guest);
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, guest.start.clone(), &memory, &mut handler);
    assert_eq!(
        handler.system_calls,
        [REQUEST],
        "the system calls the handler was given"
    );
    assert_stops_at_first_instruction(&guest, FirstTrap::SystemCall, &stopped);
}

/// Checks that a store to a page that allows nothing faults and, retried after the handler allows the page, stores.
pub fn memory_fault_answered_retry_runs_the_access_again<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (memory, mut table) = create_memory(&executor);
    let guest = arm64::build_store(CODE, DATA, STORED);
    place(&memory, &mut table, &guest);
    let mut handler = Recorder::new(None, Some((&mut table, DATA_PERMISSIONS)));
    let stopped = execute(&mut executor, guest.start.clone(), &memory, &mut handler);
    assert_eq!(
        handler.memory_faults,
        [(DATA, Access::Write)],
        "the memory faults the handler was given"
    );
    assert_eq!(stopped, guest.finished, "the state execute returned");
    assert_eq!(
        read_bytes(&memory, DATA..DATA + WORD),
        STORED.to_le_bytes(),
        "the bytes the store left"
    );
}

/// Checks that a store to a page that allows nothing, answered terminate, stops at the store and writes nothing.
pub fn memory_fault_answered_terminate_stops_at_the_access<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (memory, mut table) = create_memory(&executor);
    let guest = arm64::build_store(CODE, DATA, STORED);
    place(&memory, &mut table, &guest);
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, guest.start.clone(), &memory, &mut handler);
    assert_eq!(
        handler.memory_faults,
        [(DATA, Access::Write)],
        "the memory faults the handler was given"
    );
    let fault = FirstTrap::MemoryFault {
        address: DATA,
        access: Access::Write,
    };
    assert_stops_at_first_instruction(&guest, fault, &stopped);
    assert_eq!(
        read_bytes(&memory, page(DATA)),
        zeros(page(DATA)),
        "the bytes the store left"
    );
}

/// Checks that an undefined instruction ends execute with its fault in the state returned and calls no handler.
pub fn undefined_instruction_ends_execute_with_the_fault<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (memory, mut table) = create_memory(&executor);
    let guest = arm64::build_undefined_instruction(CODE);
    place(&memory, &mut table, &guest);
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, guest.start.clone(), &memory, &mut handler);
    handler.assert_no_trap();
    assert_eq!(stopped, guest.finished, "the state execute returned");
}

/// Checks that a guest interrupted from another thread, every interrupt answered continue, ends as it would without.
pub fn interrupts_answered_continue_change_nothing<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (memory, mut table) = create_memory(&executor);
    let guest = arm64::build_count_down(CODE, COUNT);
    place(&memory, &mut table, &guest);
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute_interrupted(&mut executor, guest.start.clone(), &memory, &mut handler);
    handler.assert_no_trap();
    assert_eq!(
        stopped, guest.finished,
        "the state execute returned after {} interrupts",
        handler.interrupts
    );
}

/// Checks that an interrupt from another thread, answered terminate, ends a guest that would run on long after it.
pub fn interrupt_answered_terminate_ends_execute<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (memory, mut table) = create_memory(&executor);
    let guest = arm64::build_count_down(CODE, LONG_COUNT);
    place(&memory, &mut table, &guest);
    let mut handler =
        Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>).terminating_at_interrupts();
    let stopped = execute_interrupted(&mut executor, guest.start.clone(), &memory, &mut handler);
    handler.assert_no_trap();
    assert_eq!(
        handler.interrupts, 1,
        "the interrupts the handler was given"
    );
    assert_ne!(
        stopped, guest.finished,
        "execute ran to the end after an interrupt answered terminate"
    );
}

/// Checks that an interrupt raised before the execution runs is answered at its start, before the guest runs.
pub fn interrupt_before_the_run_is_answered_at_the_start<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (memory, mut table) = create_memory(&executor);
    let guest = arm64::build_count_down(CODE, COUNT);
    place(&memory, &mut table, &guest);
    let mut handler =
        Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>).terminating_at_interrupts();
    let mut first = true;
    let stopped = run_to_the_end(
        &mut executor,
        guest.start.clone(),
        &memory,
        &mut handler,
        |interrupter| {
            if std::mem::take(&mut first) {
                interrupter.interrupt();
            }
        },
    );
    handler.assert_no_trap();
    assert_eq!(
        handler.interrupts, 1,
        "the interrupts the handler was given"
    );
    assert_eq!(stopped, guest.start, "the state execute returned");
}

/// Checks that an interrupt raised after its execution ended reaches neither it nor the next execution.
pub fn interrupt_after_the_end_does_nothing<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (memory, mut table) = create_memory(&executor);
    let guest = arm64::build_count_down(CODE, COUNT);
    place(&memory, &mut table, &guest);
    let mut first = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let mut last = None;
    let stopped = run_to_the_end(
        &mut executor,
        guest.start.clone(),
        &memory,
        &mut first,
        |interrupter| last = Some(interrupter),
    );
    last.expect("the execution was prepared").interrupt();
    assert_eq!(
        stopped, guest.finished,
        "the state the first execution returned"
    );
    let mut next = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, guest.start.clone(), &memory, &mut next);
    assert_eq!(
        (first.interrupts, next.interrupts),
        (usize::MIN, usize::MIN),
        "the interrupts the handlers were given"
    );
    assert_eq!(
        stopped, guest.finished,
        "the state the next execution returned"
    );
}

/// Checks that a new space reads as zeros and that guest code placed in it without permissions faults on its fetch.
pub fn new_space_reads_as_zeros_and_allows_nothing<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (memory, _table) = create_memory(&executor);
    assert_eq!(
        read_bytes(&memory, CODE..END),
        zeros(CODE..END),
        "the bytes of a new space"
    );
    let guest = arm64::build_store(CODE, DATA, STORED);
    write_bytes(&memory, CODE, &guest.code);
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, guest.start.clone(), &memory, &mut handler);
    assert_eq!(
        handler.memory_faults,
        [(CODE, Access::Execute)],
        "the memory faults the handler was given"
    );
    let fault = FirstTrap::MemoryFault {
        address: CODE,
        access: Access::Execute,
    };
    assert_stops_at_first_instruction(&guest, fault, &stopped);
}

/// Checks that every method of the space and its table refuses addresses that end past those a program can use.
pub fn addresses_outside_the_space_are_refused<Chip: Executor>(executor: Chip) {
    let (memory, mut table) = create_memory(&executor);
    let (_other, other_table) = create_memory(&executor);
    let end = Chip::Architecture::ADDRESSABLE.end;
    let outside = end..end.saturating_add(PAGE_SIZE);
    for addresses in [outside.clone(), end - PAGE_SIZE..outside.end] {
        assert!(
            matches!(memory.borrow(addresses.clone()), Err(InvalidAddressError)),
            "a borrow of {addresses:#x?} was not refused"
        );
        assert_eq!(memory.zero(addresses.clone()), Err(InvalidAddressError));
        assert_eq!(
            table.set_permissions(addresses.clone(), DATA_PERMISSIONS),
            Err(InvalidAddressError)
        );
        assert_eq!(
            table.map_from(addresses.clone(), &other_table, CODE),
            Err(MapMemoryError::InvalidAddress(InvalidAddressError))
        );
        assert_eq!(
            table.unmap_from(addresses.clone()),
            Err(InvalidAddressError)
        );
        assert_eq!(
            table.copy_from(addresses, &other_table, CODE),
            Err(CopyMemoryError::InvalidAddress(InvalidAddressError))
        );
    }
    assert_eq!(
        table.map_from(page(CODE), &other_table, outside.start),
        Err(MapMemoryError::InvalidAddress(InvalidAddressError))
    );
    assert_eq!(
        table.copy_from(page(CODE), &other_table, outside.start),
        Err(CopyMemoryError::InvalidAddress(InvalidAddressError))
    );
}

/// Checks that zero clears the bytes of its addresses, across pages, and no others.
pub fn zero_clears_only_its_addresses<Chip: Executor>(executor: Chip) {
    let (memory, _table) = create_memory(&executor);
    let written = pattern(FIRST_PATTERN, length(&(CODE..END)));
    write_bytes(&memory, CODE, &written);
    assert_eq!(memory.zero(LONG), Ok(()));
    let mut expected = written;
    expected[length(&(CODE..LONG.start))..length(&(CODE..LONG.end))].fill(u8::MIN);
    assert_eq!(read_bytes(&memory, CODE..END), expected);
}

/// Checks that permissions bind guest code, a load allowed and a store and a fetch refused, but not borrowed bytes.
pub fn permissions_bind_guest_code_and_not_borrowed_bytes<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (readable, mut readable_table) = create_memory(&executor);
    let load = arm64::build_load(CODE, DATA, STORED);
    place(&readable, &mut readable_table, &load);
    write_bytes(&readable, DATA, &STORED.to_le_bytes());
    assert_eq!(
        readable_table.set_permissions(page(DATA), Permissions::READ),
        Ok(())
    );
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let loaded = execute(&mut executor, load.start.clone(), &readable, &mut handler);
    handler.assert_no_trap();
    assert_eq!(loaded, load.finished, "the state the load returned");

    let (read_only, mut read_only_table) = create_memory(&executor);
    let store = arm64::build_store(CODE, DATA, STORED);
    place(&read_only, &mut read_only_table, &store);
    let before = pattern(FIRST_PATTERN, length(&page(DATA)));
    write_bytes(&read_only, DATA, &before);
    assert_eq!(
        read_only_table.set_permissions(page(DATA), Permissions::READ),
        Ok(())
    );
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, store.start.clone(), &read_only, &mut handler);
    assert_eq!(
        handler.memory_faults,
        [(DATA, Access::Write)],
        "the memory faults the handler was given"
    );
    let fault = FirstTrap::MemoryFault {
        address: DATA,
        access: Access::Write,
    };
    assert_stops_at_first_instruction(&store, fault, &stopped);
    assert_eq!(
        read_bytes(&read_only, page(DATA)),
        before,
        "the bytes the refused store left"
    );
    let after = pattern(SECOND_PATTERN, length(&page(DATA)));
    write_bytes(&read_only, DATA, &after);
    assert_eq!(
        read_only_table.set_permissions(page(DATA), Permissions::empty()),
        Ok(())
    );
    assert_eq!(
        read_bytes(&read_only, page(DATA)),
        after,
        "the borrowed bytes of a page that allows nothing"
    );

    let (not_executable, mut not_executable_table) = create_memory(&executor);
    place(&not_executable, &mut not_executable_table, &store);
    assert_eq!(
        not_executable_table.set_permissions(page(CODE), Permissions::READ),
        Ok(())
    );
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(
        &mut executor,
        store.start.clone(),
        &not_executable,
        &mut handler,
    );
    assert_eq!(
        handler.memory_faults,
        [(CODE, Access::Execute)],
        "the memory faults the handler was given"
    );
    let fault = FirstTrap::MemoryFault {
        address: CODE,
        access: Access::Execute,
    };
    assert_stops_at_first_instruction(&store, fault, &stopped);
}

/// Checks that a table changes the permissions of the space made with it and of no other space.
pub fn page_table_changes_its_own_space_only<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (own, mut own_table) = create_memory(&executor);
    let (other, _other_table) = create_memory(&executor);
    let store = arm64::build_store(CODE, DATA, STORED);
    write_bytes(&other, CODE, &store.code);
    place(&own, &mut own_table, &store);
    assert_eq!(
        own_table.set_permissions(page(DATA), DATA_PERMISSIONS),
        Ok(())
    );
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, store.start.clone(), &other, &mut handler);
    assert_eq!(
        handler.memory_faults,
        [(CODE, Access::Execute)],
        "the memory faults of a space whose table gave no permission"
    );
    let fault = FirstTrap::MemoryFault {
        address: CODE,
        access: Access::Execute,
    };
    assert_stops_at_first_instruction(&store, fault, &stopped);
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, store.start.clone(), &own, &mut handler);
    handler.assert_no_trap();
    assert_eq!(
        stopped, store.finished,
        "the state the store in its own space returned"
    );
    assert_eq!(
        read_bytes(&other, page(DATA)),
        zeros(page(DATA)),
        "the other space's bytes after the store"
    );
}

/// Checks that after a table maps another table's page, a write on either side, borrowed or by a guest, is seen on both.
pub fn map_from_shares_the_bytes_both_ways<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (first, first_table) = create_memory(&executor);
    let (second, mut second_table) = create_memory(&executor);
    let shared = pattern(FIRST_PATTERN, length(&page(DATA)));
    write_bytes(&first, DATA, &shared);
    assert_eq!(
        second_table.map_from(page(DATA + SHIFT), &first_table, DATA),
        Ok(())
    );
    assert_eq!(
        read_bytes(&second, page(DATA + SHIFT)),
        shared,
        "the bytes the mapping shows"
    );
    let from_second = pattern(SECOND_PATTERN, length(&page(DATA)));
    write_bytes(&second, DATA + SHIFT, &from_second);
    assert_eq!(
        read_bytes(&first, page(DATA)),
        from_second,
        "the bytes a write through the mapping left"
    );
    let from_first = pattern(THIRD_PATTERN, length(&page(DATA)));
    write_bytes(&first, DATA, &from_first);
    assert_eq!(
        read_bytes(&second, page(DATA + SHIFT)),
        from_first,
        "the bytes the mapping shows after a write"
    );

    let store = arm64::build_store(CODE, DATA + SHIFT, STORED);
    place(&second, &mut second_table, &store);
    assert_eq!(
        second_table.set_permissions(page(DATA + SHIFT), DATA_PERMISSIONS),
        Ok(())
    );
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, store.start.clone(), &second, &mut handler);
    handler.assert_no_trap();
    assert_eq!(
        stopped, store.finished,
        "the state the store through the mapping returned"
    );
    assert_eq!(
        read_bytes(&first, DATA..DATA + WORD),
        STORED.to_le_bytes(),
        "the bytes the guest's store left"
    );
}

/// Checks that where a space maps one page of another twice, a guest's store through either view is what a load
/// through the other finds, right after it and after a system call between them.
pub fn two_views_of_one_page_show_each_store<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let views = [(DATA, MAPPED), (MAPPED, DATA)];
    let calls = [None, Some((REQUEST.number, RETURNED))];
    for ((stored_at, loaded_from), call) in views
        .into_iter()
        .flat_map(|view| calls.map(|call| (view, call)))
    {
        let (block, block_table) = create_memory(&executor);
        let (viewing, mut viewing_table) = create_memory(&executor);
        write_bytes(&block, DATA, &pattern(FIRST_PATTERN, length(&page(DATA))));
        for view in [DATA, MAPPED] {
            assert_eq!(
                viewing_table.map_from(page(view), &block_table, DATA),
                Ok(())
            );
            assert_eq!(
                viewing_table.set_permissions(page(view), DATA_PERMISSIONS),
                Ok(())
            );
        }
        let guest = arm64::build_store_then_load(CODE, stored_at, loaded_from, STORED, call);
        place(&viewing, &mut viewing_table, &guest);
        let returned = call.map(|(_, returned)| returned);
        let mut handler = Recorder::new(returned, None::<(&mut PageTableOf<Chip>, _)>);
        let stopped = execute(&mut executor, guest.start.clone(), &viewing, &mut handler);
        assert!(
            handler.memory_faults.is_empty(),
            "the handler was given memory faults: {:?}",
            handler.memory_faults
        );
        assert_eq!(
            stopped,
            guest.finished,
            "the state after a store at {stored_at:#x} and a load at {loaded_from:#x}, a system call between: {}",
            call.is_some()
        );
        assert_eq!(
            read_bytes(&block, DATA..DATA + WORD),
            STORED.to_le_bytes(),
            "the bytes the guest's store left in the mapped page"
        );
    }
}

/// Checks that an unmapped page reads as zeros, not shared any more, while the other space keeps its bytes.
pub fn unmap_from_zeroes_this_side_only<Chip: Executor>(executor: Chip) {
    let (first, first_table) = create_memory(&executor);
    let (second, mut second_table) = create_memory(&executor);
    let kept = pattern(FIRST_PATTERN, length(&page(DATA)));
    write_bytes(&first, DATA, &kept);
    assert_eq!(
        second_table.map_from(page(DATA + SHIFT), &first_table, DATA),
        Ok(())
    );
    assert_eq!(second_table.unmap_from(page(DATA + SHIFT)), Ok(()));
    assert_eq!(
        read_bytes(&second, page(DATA + SHIFT)),
        zeros(page(DATA)),
        "the bytes of the unmapped page"
    );
    assert_eq!(
        read_bytes(&first, page(DATA)),
        kept,
        "the bytes the other space kept"
    );
    write_bytes(
        &second,
        DATA + SHIFT,
        &pattern(SECOND_PATTERN, length(&page(DATA))),
    );
    assert_eq!(
        read_bytes(&first, page(DATA)),
        kept,
        "the bytes after a write to the unmapped page"
    );
}

/// Checks that a copy carries bytes, permissions and mappings, and that later writes to either side stay there.
pub fn copy_from_carries_bytes_permissions_and_mappings<Chip>(mut executor: Chip)
where
    Chip: Executor<Architecture = arm64::Chip>,
{
    let (source, mut source_table) = create_memory(&executor);
    let (copy, mut copy_table) = create_memory(&executor);
    let (mapped, mapped_table) = create_memory(&executor);
    let store = arm64::build_store(CODE, DATA, STORED);
    place(&source, &mut source_table, &store);
    let data = pattern(FIRST_PATTERN, length(&page(DATA)));
    write_bytes(&source, DATA, &data);
    assert_eq!(
        source_table.set_permissions(page(DATA), DATA_PERMISSIONS),
        Ok(())
    );
    assert_eq!(
        source_table.map_from(page(MAPPED), &mapped_table, MAPPED),
        Ok(())
    );
    write_bytes(
        &mapped,
        MAPPED,
        &pattern(SECOND_PATTERN, length(&page(MAPPED))),
    );

    assert_eq!(
        copy_table.copy_from(CODE + SHIFT..END + SHIFT, &source_table, CODE),
        Ok(())
    );
    assert_eq!(
        read_bytes(&copy, page(DATA + SHIFT)),
        data,
        "the bytes the copy carried"
    );
    let through_mapping = pattern(THIRD_PATTERN, length(&page(MAPPED)));
    write_bytes(&mapped, MAPPED, &through_mapping);
    assert_eq!(
        read_bytes(&copy, page(MAPPED + SHIFT)),
        through_mapping,
        "the bytes of the copied mapping"
    );

    let moved = arm64::build_store(CODE + SHIFT, DATA + SHIFT, STORED);
    assert_eq!(
        moved.code, store.code,
        "the store guest's code depends on where it is"
    );
    let mut handler = Recorder::new(None, None::<(&mut PageTableOf<Chip>, _)>);
    let stopped = execute(&mut executor, moved.start.clone(), &copy, &mut handler);
    handler.assert_no_trap();
    assert_eq!(
        stopped, moved.finished,
        "the state the store in the copy returned"
    );
    let mut stored = data.clone();
    stored[..STORED.to_le_bytes().len()].copy_from_slice(&STORED.to_le_bytes());
    assert_eq!(
        read_bytes(&copy, page(DATA + SHIFT)),
        stored,
        "the bytes the store in the copy left"
    );
    assert_eq!(
        read_bytes(&source, page(DATA)),
        data,
        "the source's bytes after a store in the copy"
    );
    write_bytes(&source, DATA, &pattern(FOURTH_PATTERN, length(&page(DATA))));
    assert_eq!(
        read_bytes(&copy, page(DATA + SHIFT)),
        stored,
        "the copy's bytes after a write to the source"
    );
}

/// Checks that borrowed bytes have the length of their addresses.
pub fn borrowed_bytes_have_the_length_of_their_addresses<Chip: Executor>(executor: Chip) {
    let (memory, _table) = create_memory(&executor);
    for addresses in [page(DATA), LONG] {
        let bytes = memory
            .borrow(addresses.clone())
            .expect("the suite's addresses lie inside the space");
        assert_eq!(
            bytes.len(),
            addresses.end - addresses.start,
            "the length of {addresses:#x?}"
        );
        assert!(!bytes.is_empty(), "{addresses:#x?} borrowed as empty");
    }
    let none = memory
        .borrow(DATA..DATA)
        .expect("an empty borrow inside the space");
    assert_eq!(none.len(), u64::MIN, "the length of an empty borrow");
    assert!(none.is_empty(), "an empty borrow was not empty");
}

/// Checks that read and write copy as many bytes as the shorter of borrow and buffer holds, across pages too.
pub fn read_and_write_copy_as_many_as_the_shorter_holds<Chip: Executor>(executor: Chip) {
    let (memory, _table) = create_memory(&executor);
    let bytes = memory
        .borrow(LONG)
        .expect("the suite's addresses lie inside the space");
    let long = length(&LONG);
    let longer = pattern(FIRST_PATTERN, long + length(&(u64::MIN..WORD)));
    assert_eq!(bytes.write(&longer), long, "the bytes a longer write took");
    let mut buffer = vec![u8::MIN; longer.len()];
    assert_eq!(
        bytes.read(&mut buffer),
        long,
        "the bytes a read into a longer buffer gave"
    );
    assert_eq!(buffer[..long], longer[..long], "the bytes read");
    assert_eq!(
        buffer[long..],
        zeros(u64::MIN..WORD),
        "the end of the buffer past the bytes read"
    );
    assert_eq!(
        read_bytes(&memory, LONG.end..END),
        zeros(LONG.end..END),
        "the bytes past the borrow"
    );
    let shorter = pattern(SECOND_PATTERN, length(&(u64::MIN..WORD)));
    assert_eq!(
        bytes.write(&shorter),
        shorter.len(),
        "the bytes a shorter write took"
    );
    let mut start = vec![u8::MIN; shorter.len()];
    assert_eq!(
        bytes.read(&mut start),
        shorter.len(),
        "the bytes a read into a shorter buffer gave"
    );
    assert_eq!(start, shorter, "the bytes at the start");
    let mut whole = longer[..long].to_vec();
    whole[..shorter.len()].copy_from_slice(&shorter);
    assert_eq!(
        read_bytes(&memory, LONG),
        whole,
        "the bytes after the shorter write"
    );
}

/// Checks that bytes across pages, read to a vector, a queue and a slice, arrive whole in one read each.
pub fn read_to_fills_the_standard_sinks<Chip: Executor>(executor: Chip) {
    let (memory, _table) = create_memory(&executor);
    let written = pattern(FIRST_PATTERN, length(&LONG));
    write_bytes(&memory, LONG.start, &written);
    let mut vector = Vec::new();
    read_all_to(&memory, &mut vector);
    assert_eq!(vector, written, "the bytes read to a vector");
    let mut queue = VecDeque::new();
    read_all_to(&memory, &mut queue);
    assert_eq!(queue, written, "the bytes read to a queue");
    let mut room = vec![u8::MIN; written.len()];
    let mut slice = room.as_mut_slice();
    read_all_to(&memory, &mut slice);
    assert_eq!(room, written, "the bytes read to a slice");
}

/// Checks that bytes written across pages from a slice and a queue arrive whole in one write each.
pub fn write_from_drains_the_standard_sources<Chip: Executor>(executor: Chip) {
    let (memory, _table) = create_memory(&executor);
    let from_slice = pattern(FIRST_PATTERN, length(&LONG));
    write_all_from(&memory, &mut from_slice.as_slice());
    assert_eq!(
        read_bytes(&memory, LONG),
        from_slice,
        "the bytes written from a slice"
    );
    let from_queue = pattern(SECOND_PATTERN, length(&LONG));
    write_all_from(&memory, &mut VecDeque::from(from_queue.clone()));
    assert_eq!(
        read_bytes(&memory, LONG),
        from_queue,
        "the bytes written from a queue"
    );
}

fn describe<Architecture>(
    before: &ExecutionState<'_, Architecture>,
    simulated: &ExecutionStateExpectation<'_, Architecture>,
    executed: &ExecutionState<'_, Architecture>,
) -> String
where
    Architecture: ProcessorArchitecture,
    Architecture::ProcessorState: Debug,
    Architecture::ProcessorStateExpectation: Debug,
    Architecture::PagePermission: Debug,
{
    let start = before.memory_mutations.len();
    format!(
        "  before: {:?}\n    {}\n  simulated: {:?}\n    {}\n  executed: {:?}\n    {}",
        before.processor,
        describe_deltas(&before.memory_mutations),
        simulated.processor,
        describe_deltas(&simulated.memory_mutations[start..]),
        executed.processor,
        describe_deltas(&executed.memory_mutations[start..]),
    )
}

fn describe_deltas<Architecture>(deltas: &[MemoryDelta<'_, Architecture>]) -> String
where
    Architecture: ProcessorArchitecture,
    Architecture::PagePermission: Debug,
{
    let described: Vec<String> = deltas
        .iter()
        .map(|delta| match delta {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => format!("{addresses:#x?} becomes {permission:?}"),
            MemoryDelta::ValueAssignment { start, bytes } => {
                format!("{start:#x} gets {bytes:02x?}")
            }
        })
        .collect();
    if described.is_empty() {
        NO_CHANGE.to_string()
    } else {
        described.join(CHANGE_SEPARATOR)
    }
}

/// Records every trap of a guest and answers each as the test chose.
struct Recorder<'table, Table> {
    /// The value every system call returns; none terminates the guest.
    returned: Option<u64>,
    /// The table whose faulting page gets the permissions before the first fault is retried; later faults terminate.
    retry_after: Option<(&'table mut Table, Permissions)>,
    system_calls: Vec<SystemCallRequest>,
    memory_faults: Vec<(u64, Access)>,
    interrupts: usize,
    /// Whether an interrupt terminates the guest; otherwise it continues.
    terminate_at_interrupts: bool,
}

impl<'table, Table: PageTable> Recorder<'table, Table> {
    const fn new(
        returned: Option<u64>,
        retry_after: Option<(&'table mut Table, Permissions)>,
    ) -> Self {
        Self {
            returned,
            retry_after,
            system_calls: Vec::new(),
            memory_faults: Vec::new(),
            interrupts: usize::MIN,
            terminate_at_interrupts: false,
        }
    }

    fn terminating_at_interrupts(mut self) -> Self {
        self.terminate_at_interrupts = true;
        self
    }

    fn assert_no_trap(&self) {
        assert!(
            self.system_calls.is_empty(),
            "the handler was given system calls: {:?}",
            self.system_calls
        );
        assert!(
            self.memory_faults.is_empty(),
            "the handler was given memory faults: {:?}",
            self.memory_faults
        );
    }
}

impl<Table: PageTable> TrapHandler for Recorder<'_, Table> {
    fn handle_system_call(&mut self, request: SystemCallRequest) -> SystemCallResponse {
        self.system_calls.push(request);
        match self.returned {
            Some(value) => SystemCallResponse::Return(value),
            None => SystemCallResponse::Terminate,
        }
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        self.memory_faults.push((address, access));
        match self.retry_after.take() {
            Some((table, allowed)) => {
                let faulting = page(address & !PAGE_MASK);
                assert_eq!(
                    table.set_permissions(faulting, allowed),
                    Ok(()),
                    "the handler's permissions"
                );
                MemoryFaultResponse::Retry
            }
            None => MemoryFaultResponse::Terminate,
        }
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.interrupts += 1;
        if self.terminate_at_interrupts {
            InterruptResponse::Terminate
        } else {
            InterruptResponse::Continue
        }
    }
}

/// Runs an execution to its end on this thread and returns the processor state it completed with.
fn execute<Chip: Executor>(
    executor: &mut Chip,
    processor: ProcessorStateOf<Chip>,
    memory: &Chip::GuestMemory,
    handler: &mut impl TrapHandler,
) -> ProcessorStateOf<Chip> {
    run_to_the_end(executor, processor, memory, handler, drop)
}

/// Runs an execution until a trap terminates it, preparing it again after each wait, and returns the state it ended at.
///
/// Each interrupter is handed over as its execution is prepared.
fn run_to_the_end<Chip: Executor>(
    executor: &mut Chip,
    mut processor: ProcessorStateOf<Chip>,
    memory: &Chip::GuestMemory,
    handler: &mut impl TrapHandler,
    mut on_prepared: impl FnMut(Chip::Interrupter),
) -> ProcessorStateOf<Chip> {
    loop {
        let (execution, interrupter) = executor.prepare_execution(processor, memory);
        on_prepared(interrupter);
        match execution.run(handler) {
            ExecutionStop::Terminated(stopped) => return stopped,
            ExecutionStop::Waiting {
                processor: waited,
                wait,
            } => {
                block_on(wait);
                processor = waited;
            }
        }
    }
}

/// Runs an execution to its end while another thread interrupts it, and returns the processor state it completed with.
fn execute_interrupted<Chip: Executor>(
    executor: &mut Chip,
    processor: ProcessorStateOf<Chip>,
    memory: &Chip::GuestMemory,
    handler: &mut impl TrapHandler,
) -> ProcessorStateOf<Chip> {
    let current: Mutex<Option<Chip::Interrupter>> = Mutex::new(None);
    let ended = AtomicBool::new(false);
    thread::scope(|scope| {
        scope.spawn(|| {
            loop {
                thread::sleep(INTERRUPT_PERIOD);
                if ended.load(Ordering::Acquire) {
                    break;
                }
                if let Some(interrupter) = &*current.lock().unwrap_or_else(PoisonError::into_inner)
                {
                    interrupter.interrupt();
                }
            }
        });
        let _ending = SetOnDrop(&ended);
        run_to_the_end(executor, processor, memory, handler, |interrupter| {
            *current.lock().unwrap_or_else(PoisonError::into_inner) = Some(interrupter);
        })
    })
}

/// Sets the flag when dropped, so that a panic too ends the thread that waits on it.
struct SetOnDrop<'flag>(&'flag AtomicBool);

impl Drop for SetOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// Wakes the thread that waits on a future.
struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Polls the future on this thread, parked between polls, and returns its output.
fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        thread::park();
    }
}

/// Returns a new space of the executor and the table of its pages.
fn create_memory<Chip: Executor>(executor: &Chip) -> (Chip::GuestMemory, PageTableOf<Chip>) {
    match executor.create_guest_memory() {
        Ok(memory) => memory,
        Err(_) => panic!("the executor made no guest memory"),
    }
}

/// Writes the guest's code where its start state names and allows the code's page to be read and executed.
fn place<Architecture: ProcessorArchitecture, Space: MemorySpace>(
    space: &Space,
    table: &mut Space::PageTable,
    guest: &Guest<Architecture>,
) {
    write_bytes(space, CODE, &guest.code);
    assert_eq!(table.set_permissions(page(CODE), CODE_PERMISSIONS), Ok(()));
}

fn assert_stops_at_first_instruction(
    guest: &Guest<arm64::Chip>,
    trap: FirstTrap,
    stopped: &<arm64::Chip as ProcessorArchitecture>::ProcessorState,
) {
    assert!(
        arm64::is_stop_at_first_instruction(guest, trap, stopped),
        "execute returned {stopped:?} for {trap:?}, from {:?}",
        guest.start
    );
}

/// Reads the suite's long addresses into the sink in one call, which must move all of them.
fn read_all_to<Space: MemorySpace, Sink: ByteSink>(space: &Space, sink: &mut Sink)
where
    Sink::Error: Debug,
{
    let bytes = space
        .borrow(LONG)
        .expect("the suite's addresses lie inside the space");
    let count = bytes.read_to(sink).expect("the sink took the bytes");
    assert_eq!(count, length(&LONG), "the bytes one read to a sink gave");
}

/// Writes the source into the suite's long addresses in one call, which must move all of them.
fn write_all_from<Space: MemorySpace, Source: ByteSource>(space: &Space, source: &mut Source)
where
    Source::Error: Debug,
{
    let bytes = space
        .borrow(LONG)
        .expect("the suite's addresses lie inside the space");
    let count = bytes.write_from(source).expect("the source gave the bytes");
    assert_eq!(
        count,
        length(&LONG),
        "the bytes one write from a source took"
    );
}

fn write_bytes<Space: MemorySpace>(space: &Space, start: u64, bytes: &[u8]) {
    let end = start + u64::try_from(bytes.len()).expect("a count of bytes fits in an address");
    let borrowed = space
        .borrow(start..end)
        .expect("the suite's addresses lie inside the space");
    assert_eq!(
        borrowed.write(bytes),
        bytes.len(),
        "the bytes a write to {start:#x} took"
    );
}

fn read_bytes<Space: MemorySpace>(space: &Space, addresses: Range<u64>) -> Vec<u8> {
    let mut bytes = zeros(addresses.clone());
    let borrowed = space
        .borrow(addresses.clone())
        .expect("the suite's addresses lie inside the space");
    assert_eq!(
        borrowed.read(&mut bytes),
        bytes.len(),
        "the bytes a read of {addresses:#x?} gave"
    );
    bytes
}

fn zeros(addresses: Range<u64>) -> Vec<u8> {
    vec![u8::MIN; length(&addresses)]
}

/// Returns the bytes for the length, none of them zero, each other than the byte at its place for any other seed.
fn pattern(seed: u8, length: usize) -> Vec<u8> {
    let period = usize::from(u8::MAX);
    (0..length)
        .map(|place| {
            let below_period =
                u8::try_from((place + usize::from(seed)) % period).expect("below the period");
            below_period + 1
        })
        .collect()
}

fn page(start: u64) -> Range<u64> {
    start..start + PAGE_SIZE
}

/// Returns the number of bytes of the addresses.
fn length(addresses: &Range<u64>) -> usize {
    usize::try_from(addresses.end - addresses.start)
        .expect("the suite's addresses fit in host memory")
}

/// Runs each suite function above as its own test on its values, made anew for every test, and the behavior source's.
///
/// The values are an executor, the precise instruction set it should agree with, the generator of the states and the
/// states the executor cannot execute, none for an executor whose error is `Infallible`; and, where the executor is
/// wrong, the instructions its agreement sets aside, none when left out.
#[macro_export]
macro_rules! tests {
    ($executor:expr, $set:expr, $generator:expr, $unexecutable:expr) => {
        $crate::tests!($executor, $set, $generator, $unexecutable, &[]);
    };
    ($executor:expr, $set:expr, $generator:expr, $unexecutable:expr, $set_aside:expr) => {
        #[test]
        fn agrees_with_precise_instruction_set() {
            $crate::agrees_with_precise_instruction_set(|| $executor, $set, $generator, $set_aside);
        }

        #[test]
        fn executor_and_its_memory_move_between_threads() {
            $crate::executor_and_its_memory_move_between_threads($executor);
        }

        #[test]
        fn system_call_answered_return_goes_on_with_the_value() {
            $crate::system_call_answered_return_goes_on_with_the_value($executor);
        }

        #[test]
        fn system_call_answered_terminate_stops_at_the_call() {
            $crate::system_call_answered_terminate_stops_at_the_call($executor);
        }

        #[test]
        fn memory_fault_answered_retry_runs_the_access_again() {
            $crate::memory_fault_answered_retry_runs_the_access_again($executor);
        }

        #[test]
        fn memory_fault_answered_terminate_stops_at_the_access() {
            $crate::memory_fault_answered_terminate_stops_at_the_access($executor);
        }

        #[test]
        fn undefined_instruction_ends_execute_with_the_fault() {
            $crate::undefined_instruction_ends_execute_with_the_fault($executor);
        }

        #[test]
        fn interrupts_answered_continue_change_nothing() {
            $crate::interrupts_answered_continue_change_nothing($executor);
        }

        #[test]
        fn interrupt_answered_terminate_ends_execute() {
            $crate::interrupt_answered_terminate_ends_execute($executor);
        }

        #[test]
        fn interrupt_before_the_run_is_answered_at_the_start() {
            $crate::interrupt_before_the_run_is_answered_at_the_start($executor);
        }

        #[test]
        fn interrupt_after_the_end_does_nothing() {
            $crate::interrupt_after_the_end_does_nothing($executor);
        }

        #[test]
        fn new_space_reads_as_zeros_and_allows_nothing() {
            $crate::new_space_reads_as_zeros_and_allows_nothing($executor);
        }

        #[test]
        fn addresses_outside_the_space_are_refused() {
            $crate::addresses_outside_the_space_are_refused($executor);
        }

        #[test]
        fn zero_clears_only_its_addresses() {
            $crate::zero_clears_only_its_addresses($executor);
        }

        #[test]
        fn permissions_bind_guest_code_and_not_borrowed_bytes() {
            $crate::permissions_bind_guest_code_and_not_borrowed_bytes($executor);
        }

        #[test]
        fn page_table_changes_its_own_space_only() {
            $crate::page_table_changes_its_own_space_only($executor);
        }

        #[test]
        fn map_from_shares_the_bytes_both_ways() {
            $crate::map_from_shares_the_bytes_both_ways($executor);
        }

        #[test]
        fn two_views_of_one_page_show_each_store() {
            $crate::two_views_of_one_page_show_each_store($executor);
        }

        #[test]
        fn unmap_from_zeroes_this_side_only() {
            $crate::unmap_from_zeroes_this_side_only($executor);
        }

        #[test]
        fn copy_from_carries_bytes_permissions_and_mappings() {
            $crate::copy_from_carries_bytes_permissions_and_mappings($executor);
        }

        #[test]
        fn borrowed_bytes_have_the_length_of_their_addresses() {
            $crate::borrowed_bytes_have_the_length_of_their_addresses($executor);
        }

        #[test]
        fn read_and_write_copy_as_many_as_the_shorter_holds() {
            $crate::read_and_write_copy_as_many_as_the_shorter_holds($executor);
        }

        #[test]
        fn read_to_fills_the_standard_sinks() {
            $crate::read_to_fills_the_standard_sinks($executor);
        }

        #[test]
        fn write_from_drains_the_standard_sources() {
            $crate::write_from_drains_the_standard_sources($executor);
        }

        mod behavior_source {
            use super::*;

            $crate::wcontainers_behavior_source_conformance::tests!(
                $executor,
                $generator,
                $unexecutable
            );
        }
    };
}
