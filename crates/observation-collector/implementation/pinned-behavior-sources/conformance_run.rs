//! The interface's suite and this collector's own checks, on four simulated accumulator machine cores of two kinds.

use std::convert::Infallible;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_accumulator_machine::AccumulatorMachineInstructionInputDomain;
use wcontainers_observation_collector::ObservationCollector;
use wcontainers_observation_collector_pinned_behavior_sources::{
    CollectSamplesError, PinnedBehaviorSourcesObservationCollector,
};
use wcontainers_processor_architecture::{
    MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_accumulator_machine::{
    AccumulatorMachineProcessorArchitecture, CoreDesign, CoreIdentity, CoreRevision, INSTRUCTIONS,
    Instruction, PreciseInstruction, Processor,
};
use wcontainers_processor_cores::ProcessorCores;

/// The revision of each core, by its number: two kinds of two cores each, listed alternately.
const CORE_REVISIONS: [CoreRevision; 4] = [
    CoreRevision::First,
    CoreRevision::Second,
    CoreRevision::First,
    CoreRevision::Second,
];

/// Four cores that run the work on the calling thread, as cores of one machine would.
struct AlternatingRevisionCores;

impl ProcessorCores for AlternatingRevisionCores {
    type Core = usize;
    type CoreIdentity = CoreIdentity;
    type ListCoresError = Infallible;
    type RunOnCoreError = Infallible;

    fn list_cores(&self) -> Result<Vec<(usize, CoreIdentity)>, Infallible> {
        Ok(CORE_REVISIONS
            .into_iter()
            .enumerate()
            .map(|(core, revision)| {
                (
                    core,
                    CoreIdentity {
                        design: CoreDesign::Reference,
                        revision,
                    },
                )
            })
            .collect())
    }

    fn run_on_core<Output>(
        &self,
        _: &usize,
        work: impl FnOnce() -> Output,
    ) -> Result<Output, Infallible> {
        Ok(work())
    }
}

/// Returns the behavior a core of the revision shows: the first known one on the first revision, else the last.
fn choose_behavior(revision: CoreRevision, instruction: Instruction) -> Option<PreciseInstruction> {
    let mut known = AccumulatorMachineProcessorArchitecture::list_known_behaviors(instruction);
    match revision {
        CoreRevision::First => known.next(),
        CoreRevision::Second => known.last(),
    }
}

/// A core that runs the instruction at its program counter with the behavior its revision chooses.
struct SimulatedCore {
    revision: CoreRevision,
    /// The instruction it shows no known behavior for, refusing its states.
    refused: Option<Instruction>,
}

/// The core could not run the state.
#[derive(Debug)]
struct Refusal;

impl BehaviorSource for SimulatedCore {
    type Architecture = AccumulatorMachineProcessorArchitecture;
    type Error = Refusal;

    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor,
        memory: MemorySnapshot<'_, '_, AccumulatorMachineProcessorArchitecture>,
        mutation_hook: &mut impl MemorySink<AccumulatorMachineProcessorArchitecture>,
    ) -> Result<(), Refusal> {
        let address = u64::from(processor.program_counter);
        let opcode = memory
            .mutations
            .iter()
            .rev()
            .find_map(|mutation| match mutation {
                MemoryDelta::ValueAssignment { start, bytes } => address
                    .checked_sub(*start)
                    .and_then(|offset| bytes.get(usize::try_from(offset).ok()?).copied()),
                MemoryDelta::PermissionAssignment { .. } => None,
            });
        let instruction = opcode
            .and_then(|opcode| INSTRUCTIONS.get(usize::from(opcode)).copied())
            .ok_or(Refusal)?;
        if self.refused == Some(instruction) {
            return Err(Refusal);
        }
        let behavior = choose_behavior(self.revision, instruction).ok_or(Refusal)?;
        let Ok(after) = behavior.simulate(processor, memory, mutation_hook);
        *processor = after;
        Ok(())
    }
}

const GENERATOR: CriticalValueProduct<AccumulatorMachineInstructionInputDomain> =
    CriticalValueProduct::new(AccumulatorMachineInstructionInputDomain);

/// Returns a collector of the four cores whose sources refuse the states of one instruction, or none.
fn make_collector(
    refused: Option<Instruction>,
) -> impl ObservationCollector<
    Architecture = AccumulatorMachineProcessorArchitecture,
    CoreIdentity = CoreIdentity,
    Error = CollectSamplesError<
        CoreIdentity,
        Infallible,
        Infallible,
        Infallible,
        AccumulatorMachineProcessorArchitecture,
        Refusal,
    >,
> {
    PinnedBehaviorSourcesObservationCollector::new(
        AlternatingRevisionCores,
        move |core: CoreIdentity| {
            Ok::<_, Infallible>(SimulatedCore {
                revision: core.revision,
                refused,
            })
        },
        GENERATOR,
    )
}

wcontainers_observation_collector_conformance::tests!(make_collector(None));

/// Checks that each kind's sample holds the behavior its cores show, the kinds in the order the cores list them.
#[test]
fn each_sample_holds_what_its_kind_shows() {
    let Ok(samples) = make_collector(None).collect_samples() else {
        panic!("the samples could not be collected");
    };
    let revisions: Vec<CoreRevision> = samples.iter().map(|sample| sample.core.revision).collect();
    assert!(revisions == [CoreRevision::First, CoreRevision::Second]);
    for sample in &samples {
        for &(instruction, behavior) in &sample.measurements {
            assert!(choose_behavior(sample.core.revision, instruction) == Some(behavior));
        }
    }
}

/// Checks that a refused instruction fails the whole collection with the core and the instruction it failed on.
#[test]
fn a_failed_instruction_fails_the_collection() {
    let refused = Instruction::Increment;
    match make_collector(Some(refused)).collect_samples() {
        Err(CollectSamplesError::MeasurementFailed {
            core,
            error: MeasureInstructionError::ExecutionFailed { instruction, .. },
        }) => assert!(core.design == CoreDesign::Reference && instruction == refused),
        _ => panic!("the collection did not fail on the refused instruction"),
    }
}

/// Checks that the cores of a kind share its queue: together they measure each instruction once.
#[test]
fn the_cores_of_a_kind_measure_each_instruction_once() {
    let made = AtomicUsize::new(usize::MIN);
    let measured = Mutex::new(Vec::new());
    let collector = PinnedBehaviorSourcesObservationCollector::new(
        AlternatingRevisionCores,
        |core: CoreIdentity| {
            made.fetch_add(1, Ordering::Relaxed);
            Ok::<_, Infallible>(CountingCore {
                core: SimulatedCore {
                    revision: core.revision,
                    refused: None,
                },
                measured: &measured,
            })
        },
        GENERATOR,
    );
    let Ok(samples) = collector.collect_samples() else {
        panic!("the samples could not be collected");
    };
    assert!(made.load(Ordering::Relaxed) == CORE_REVISIONS.len());
    let measured = measured
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let states_of_one_pass: usize = AccumulatorMachineProcessorArchitecture::list_instructions()
        .map(|instruction| DistinguishingStateGenerator::<AccumulatorMachineProcessorArchitecture>::generate_states(&GENERATOR, instruction).count())
        .sum();
    assert!(measured.len() == states_of_one_pass * samples.len());
    for sample in &samples {
        let of_kind = measured
            .iter()
            .filter(|revision| **revision == sample.core.revision)
            .count();
        assert!(of_kind == states_of_one_pass);
    }
}

/// A core that records the revision of each state it runs.
struct CountingCore<'record> {
    core: SimulatedCore,
    measured: &'record Mutex<Vec<CoreRevision>>,
}

impl BehaviorSource for CountingCore<'_> {
    type Architecture = AccumulatorMachineProcessorArchitecture;
    type Error = Refusal;

    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor,
        memory: MemorySnapshot<'_, '_, AccumulatorMachineProcessorArchitecture>,
        mutation_hook: &mut impl MemorySink<AccumulatorMachineProcessorArchitecture>,
    ) -> Result<(), Refusal> {
        self.measured
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(self.core.revision);
        self.core
            .execute_single_instruction_in_place(processor, memory, mutation_hook)
    }
}
