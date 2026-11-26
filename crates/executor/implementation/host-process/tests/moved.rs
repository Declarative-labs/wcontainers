//! The executor interface's agreement checks on form P with the ARM64 generator's states moved up into form P's guest
//! area: the generator places its code page at 4 KiB, below the 64 KiB a Linux process may map, so form P refuses every
//! state it makes where it is.
//!
//! No interface lets a source say which addresses its memory has or tells the generator where to put a state, so the
//! move is this test's own: the program counter, every memory change, the base register of an access, and every
//! register or monitor address inside the generator's pages move by [`SHIFT`]; tags and bits above 48 stay.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::fmt::Debug;
use std::iter::Map;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE};
use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_executor_host_process::executor::HostProcess;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_arm64::encoding::RN;
use wcontainers_processor_architecture_arm64::{
    Arm64, Exception, ExclusiveMonitor, INSTRUCTION_SIZE, User, VIRTUAL_ADDRESS_BITS,
};

type Chip = Arm64<User>;
type State = ExecutionState<'static, Chip>;
type Instruction = <Chip as ProcessorArchitecture>::Instruction;

/// What a Neoverse V2 under KVM did, written by the record tool: form P runs on that chip in the bar's machine.
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../behavior-source/implementation/recording/recordings/neoverse-v2"
);
/// The top byte of an address, which top-byte-ignore leaves out of the address an access uses.
const TAG_MASK: u64 = (u8::MAX as u64) << (u64::BITS - u8::BITS);
/// How far each state moves: the lowest address of form P's guest area, whole pages, so every page edge the generator
/// chose stays an edge.
const SHIFT: u64 = LOWEST_ADDRESS;
/// The generator's code page, which starts one page above zero, and its two data pages after it.
const CODE_PAGE: u64 = PAGE_SIZE;
const DATA_PAGE: u64 = CODE_PAGE + PAGE_SIZE;
const GENERATOR_PAGES: u64 = 3;
/// The generator's addresses, the first past its last page included, as an access that ends at an edge names it.
const GENERATOR_ADDRESSES: std::ops::RangeInclusive<u64> =
    CODE_PAGE..=CODE_PAGE + GENERATOR_PAGES * PAGE_SIZE;
/// The 48 bits of an address a program uses: above them a tag or the upper half's bits, which a move keeps.
const ADDRESS_BITS_MASK: u64 = (1 << VIRTUAL_ADDRESS_BITS) - 1;

static HOST: OnceLock<Arc<HostProcessHost>> = OnceLock::new();

fn executor() -> HostProcess {
    HostProcess::new(Arc::clone(HOST.get_or_init(|| {
        Arc::new(HostProcessHost::new().expect("the host is refused"))
    })))
}

fn read_recording() -> Recording {
    match Recording::read(Path::new(RECORDINGS)) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    }
}

/// The ARM64 generator with each of its states moved up by [`SHIFT`].
#[derive(Clone, Copy, Default)]
struct MovedUp(CriticalValueCut);

impl DistinguishingStateGenerator<Chip> for MovedUp {
    type States<'generator>
        = Map<
        <CriticalValueCut as DistinguishingStateGenerator<Chip>>::States<'generator>,
        fn(State) -> State,
    >
    where
        Self: 'generator;

    fn generate_states(&self, instruction: Instruction) -> Self::States<'_> {
        self.0
            .generate_states(instruction)
            .map(move_up as fn(State) -> State)
    }
}

fn is_generator_address(value: u64) -> bool {
    GENERATOR_ADDRESSES.contains(&(value & ADDRESS_BITS_MASK))
}

/// Returns the number of the register an access of the state's instruction is based on, where the state has data
/// pages: 31 names the stack pointer.
fn to_access_base(state: &State) -> Option<usize> {
    let program_counter = state.processor.program_counter;
    let word = state
        .memory_mutations
        .iter()
        .rev()
        .find_map(|delta| match delta {
            MemoryDelta::ValueAssignment { start, bytes } if *start == program_counter => {
                let bytes: [u8; INSTRUCTION_SIZE as usize] = bytes.as_ref().try_into().ok()?;
                Some(u32::from_le_bytes(bytes))
            }
            _ => None,
        });
    let data_page =
        |start: u64| start & ADDRESS_BITS_MASK == DATA_PAGE || start == DATA_PAGE + SHIFT;
    let accesses_data = state.memory_mutations.iter().any(
        |delta| matches!(delta, MemoryDelta::ValueAssignment { start, .. } if data_page(*start)),
    );
    // The generator sets an access's base to the address less the access's offset, which may lie outside its pages.
    word.filter(|_| accesses_data)
        .map(|word| RN.read(word) as usize)
}

/// Returns form P's end with its data abort's address given the top byte of the access's base, as Linux reports it to
/// a handler installed with `SA_EXPOSE_TAGBITS`, and stripped otherwise.
fn with_exposed_tag(before: &State, mut executed: State) -> State {
    let Some(base) = to_access_base(before) else {
        return executed;
    };
    let base_value = before
        .processor
        .general_purpose_registers
        .get(base)
        .copied()
        .unwrap_or(before.processor.stack_pointer);
    if let Some(Exception::DataAbort { address, .. }) = &mut executed.processor.exception {
        *address |= base_value & !ADDRESS_BITS_MASK & TAG_MASK;
    }
    executed
}

/// Returns the state with its code, its memory and the addresses its registers hold moved up by [`SHIFT`].
fn move_up(mut state: State) -> State {
    let base = to_access_base(&state);
    let processor = &mut state.processor;
    for (number, register) in processor.general_purpose_registers.iter_mut().enumerate() {
        if base == Some(number) || is_generator_address(*register) {
            *register = register.wrapping_add(SHIFT);
        }
    }
    let base_is_stack_pointer =
        base.is_some_and(|number| number >= processor.general_purpose_registers.len());
    if base_is_stack_pointer || is_generator_address(processor.stack_pointer) {
        processor.stack_pointer = processor.stack_pointer.wrapping_add(SHIFT);
    }
    processor.program_counter += SHIFT;
    if let ExclusiveMonitor::Exclusive { address } = &mut processor.exclusive_monitor
        && is_generator_address(*address)
    {
        *address += SHIFT;
    }
    for delta in &mut state.memory_mutations {
        match delta {
            MemoryDelta::PermissionAssignment { addresses, .. } => {
                *addresses = addresses.start + SHIFT..addresses.end + SHIFT;
            }
            MemoryDelta::ValueAssignment { start, .. } => *start += SHIFT,
        }
    }
    state
}

type Measured<Error> = Result<
    <Chip as ProcessorArchitecture>::PreciseInstruction,
    MeasureInstructionError<Chip, Error>,
>;

/// Names how a measurement ended, with the source's error where it gave one.
fn describe<Error: Debug>(measured: &Measured<Error>) -> String {
    match measured {
        Ok(_) => "one known behavior".to_owned(),
        Err(MeasureInstructionError::NoKnownBehavior { .. }) => "no known behavior".to_owned(),
        Err(MeasureInstructionError::ExecutionFailed { error, .. }) => {
            format!("refused ({error:?})")
        }
        Err(MeasureInstructionError::NoBehaviorAgrees { .. }) => "no behavior agrees".to_owned(),
        Err(MeasureInstructionError::IndistinguishableBehaviors { .. }) => {
            "indistinguishable behaviors".to_owned()
        }
    }
}

/// What form P did on one instruction's states.
#[derive(Default)]
struct Tally {
    states: usize,
    refused_unmoved: usize,
    refused: usize,
    /// Moved states whose end meets what some known behavior of the instruction expects.
    known: usize,
    unknown: usize,
    /// Of the states that meet none, those that meet a known behavior once the abort's address keeps its tag.
    tag_only: usize,
    /// The first state the tag does not explain: form P's exception and each behavior's.
    first_unexplained: Option<String>,
}

/// Each instruction the recording holds: measured on the recording at the generator's own addresses, then form P on
/// every state at those addresses (refused, as the check measured) and moved up, each moved state's end against the
/// instruction's known behaviors, and the one behavior form P shows on the moved states.
#[test]
fn recorded_instructions_on_form_p_when_moved() {
    let started = Instant::now();
    let mut recording = read_recording();
    let generator = CriticalValueCut::default();
    let moved = MovedUp(generator);
    let mut form_p = executor();
    let mut total = Tally::default();
    let mut report = String::new();
    for instruction in recording.instructions().collect::<Vec<_>>() {
        let behaviors: Vec<_> = Chip::list_known_behaviors(instruction).collect();
        let mut tally = Tally::default();
        for before in generator.generate_states(instruction) {
            tally.states += 1;
            if form_p.execute_single_instruction(before.clone()).is_err() {
                tally.refused_unmoved += 1;
            }
            let before = move_up(before);
            match form_p.execute_single_instruction(before.clone()) {
                Ok(executed) => {
                    let meets_one = behaviors.iter().any(|precise| {
                        let Ok(expected) = precise.simulate_state(before.clone());
                        executed.meets_expectations(&expected)
                    });
                    if meets_one {
                        tally.known += 1;
                    } else {
                        tally.unknown += 1;
                        let tagged = with_exposed_tag(&before, executed.clone());
                        if behaviors.iter().any(|precise| {
                            let Ok(expected) = precise.simulate_state(before.clone());
                            tagged.meets_expectations(&expected)
                        }) {
                            tally.tag_only += 1;
                        } else {
                            tally.first_unexplained.get_or_insert_with(|| {
                                let expected: Vec<_> = behaviors
                                    .iter()
                                    .map(|precise| {
                                        let Ok(expected) = precise.simulate_state(before.clone());
                                        format!("{:?}", expected.processor.processor.exception)
                                    })
                                    .collect();
                                format!(
                                    "pc {:#x}: form P {:?}, behaviors {expected:?}",
                                    before.processor.program_counter, executed.processor.exception
                                )
                            });
                        }
                    }
                }
                Err(_) => tally.refused += 1,
            }
        }
        let on_recording = recording.measure_instruction(instruction, &generator);
        let on_form_p = form_p.measure_instruction(instruction, &moved);
        let same = match (&on_recording, &on_form_p) {
            (Ok(recorded), Ok(shown)) => {
                if recorded == shown {
                    "the same"
                } else {
                    "another"
                }
            }
            _ => "not compared",
        };
        println!(
            "{instruction:?}: {} states; unmoved refused {}; moved: refused {}, meet a known behavior {}, meet none {} (with the tag kept {}); \
             recording: {}; form P moved: {}; behavior {same}",
            tally.states,
            tally.refused_unmoved,
            tally.refused,
            tally.known,
            tally.unknown,
            tally.tag_only,
            describe(&on_recording),
            describe(&on_form_p),
        );
        if let Some(first) = &tally.first_unexplained {
            println!("  the first the tag does not explain: {first}");
        }
        if tally.known != tally.states || on_form_p.is_err() {
            report.push_str(&format!("{instruction:?} "));
        }
        total.states += tally.states;
        total.refused_unmoved += tally.refused_unmoved;
        total.refused += tally.refused;
        total.known += tally.known;
        total.unknown += tally.unknown;
        total.tag_only += tally.tag_only;
    }
    println!(
        "total: {} states; unmoved refused {}; moved: refused {}, meet a known behavior {}, meet none {} (with the tag kept {}); {:.1} s",
        total.states,
        total.refused_unmoved,
        total.refused,
        total.known,
        total.unknown,
        total.tag_only,
        started.elapsed().as_secs_f64()
    );
    assert!(
        report.is_empty(),
        "instructions form P does not run to one known behavior: {report}"
    );
}

/// Every instruction with a known behavior measured on form P from the moved states: how many show exactly one known
/// behavior, and why each of the rest does not. A measurement of hours, so run on request.
#[test]
#[ignore = "a measurement of every instruction, hours long"]
fn every_instruction_measured_on_form_p_when_moved() {
    let started = Instant::now();
    let moved = MovedUp::default();
    let mut form_p = executor();
    let (mut one, mut none_known, mut refused, mut none_agrees, mut indistinguishable) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for instruction in Chip::list_instructions() {
        match form_p.measure_instruction(instruction, &moved) {
            Ok(_) => one.push(instruction),
            Err(MeasureInstructionError::NoKnownBehavior { .. }) => none_known.push(instruction),
            Err(MeasureInstructionError::ExecutionFailed { error, .. }) => {
                refused.push((instruction, error));
            }
            Err(MeasureInstructionError::NoBehaviorAgrees { .. }) => none_agrees.push(instruction),
            Err(MeasureInstructionError::IndistinguishableBehaviors { .. }) => {
                indistinguishable.push(instruction);
            }
        }
    }
    println!("no behavior agrees: {none_agrees:?}");
    println!("refused: {refused:?}");
    println!("indistinguishable: {indistinguishable:?}");
    println!(
        "instructions: one known behavior {}, no known behavior {}, refused {}, no behavior agrees {}, indistinguishable {}; {:.1} s",
        one.len(),
        none_known.len(),
        refused.len(),
        none_agrees.len(),
        indistinguishable.len(),
        started.elapsed().as_secs_f64()
    );
}

// The behavior source suite on form P with the moved generator: its execute, determinism and after-an-error checks
// compare form P with itself, so here they execute states and are no longer met by two refusals.
mod behavior_source {
    use super::*;

    /// The one state form P refuses: an exclusive monitor that holds an address, which no host call sets in a stub's
    /// thread.
    fn to_unexecutable() -> Vec<State> {
        let mut state = move_up(
            CriticalValueCut::default()
                .generate_states(Chip::list_instructions().next().expect("an instruction"))
                .next()
                .expect("a state"),
        );
        state.processor.exclusive_monitor = ExclusiveMonitor::Exclusive {
            address: state.processor.program_counter,
        };
        vec![state]
    }

    wcontainers_executor_conformance::wcontainers_behavior_source_conformance::tests!(
        executor(),
        MovedUp::default(),
        to_unexecutable()
    );
}
