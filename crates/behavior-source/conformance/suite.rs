//! The tests every implementation of wcontainers-behavior-source runs, written against its traits alone.

use std::convert::Infallible;

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{
    ExecutionState, MemorySink, MemorySnapshot, ProcessorArchitecture, Simulator,
};

/// The provided `execute_single_instruction` gives what `execute_single_instruction_in_place` does: its processor,
/// and its memory mutations appended, but in what a known behavior leaves undecided.
pub fn execute_agrees_with_execute_in_place<Source, Generator>(
    mut source: Source,
    generator: Generator,
) where
    Source: BehaviorSource,
    <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Source::Architecture, Error = Infallible>,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    for instruction in Source::Architecture::list_instructions() {
        for before in generator.generate_states(instruction) {
            let mut processor = before.processor.clone();
            let mut mutated = Vec::new();
            let in_place = source.execute_single_instruction_in_place(
                &mut processor,
                before.memory(),
                &mut mutated,
            );
            let executed = source.execute_single_instruction(before.clone());
            match (in_place, executed) {
                (Ok(()), Ok(executed)) => {
                    let mut memory_mutations = before.memory_mutations.clone();
                    memory_mutations.append(&mut mutated);
                    let in_place = ExecutionState {
                        processor,
                        memory_mutations,
                    };
                    assert!(
                        is_same_outcome::<_, Infallible>(
                            instruction,
                            &before,
                            Ok(executed),
                            Ok(in_place)
                        ),
                        "execute and execute in place disagree"
                    );
                }
                (Err(_), Err(_)) => {}
                (Ok(()), Err(_)) | (Err(_), Ok(_)) => {
                    panic!("execute and execute in place disagree on whether the state executes")
                }
            }
        }
    }
}

/// Executing one state twice gives the same state after, but in what a known behavior leaves undecided.
pub fn execution_is_deterministic<Source, Generator>(mut source: Source, generator: Generator)
where
    Source: BehaviorSource,
    <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Source::Architecture, Error = Infallible>,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    for instruction in Source::Architecture::list_instructions() {
        for before in generator.generate_states(instruction) {
            let first = source.execute_single_instruction(before.clone());
            let second = source.execute_single_instruction(before.clone());
            assert!(
                is_same_outcome(instruction, &before, first, second),
                "one state gave two states after"
            );
        }
    }
}

/// The behavior measured for each instruction agrees with the source on every generated state.
///
/// Measuring every instruction at once gives each the behavior measuring it alone gives, in the order they are listed.
pub fn measured_behavior_agrees_with_the_source<Source, Generator>(
    source: Source,
    generator: Generator,
) where
    Source: BehaviorSource,
    <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Source::Architecture, Error = Infallible>,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    check_measurements(source, &generator, None);
}

/// Checks the test above on a source that shows no one known behavior for the instructions set aside.
///
/// Each of them measures as its first known behavior and is not checked. A state the source refuses is left out and
/// counted; where only refused states tell behaviors apart, the first that agrees is measured.
pub fn measured_behavior_agrees_with_the_source_but_set_aside<Source, Generator>(
    source: Source,
    generator: Generator,
    set_aside: &[<Source::Architecture as ProcessorArchitecture>::Instruction],
) where
    Source: BehaviorSource,
    <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Source::Architecture, Error = Infallible>,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    check_measurements(
        SettingAside { source, set_aside },
        &generator,
        Some(set_aside),
    );
}

/// Checks the measurements of the source as the two tests above say, with what an instruction set aside leaves out.
fn check_measurements<Source, Generator>(
    mut source: Source,
    generator: &Generator,
    set_aside: Option<&[<Source::Architecture as ProcessorArchitecture>::Instruction]>,
) where
    Source: BehaviorSource,
    <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Source::Architecture, Error = Infallible>,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    let measurements = match source.measure_instructions(generator) {
        Ok(measurements) => measurements,
        Err(error) => panic!("measuring every instruction: {}", describe(&error)),
    };
    let (mut refused_states, mut refusing_instructions) = (usize::MIN, usize::MIN);
    for (index, instruction) in Source::Architecture::list_instructions().enumerate() {
        let precise = match source.measure_instruction(instruction, generator) {
            Ok(precise) => precise,
            Err(error) => panic!("measuring instruction {index} alone: {}", describe(&error)),
        };
        assert!(
            measurements.get(index) == Some(&(instruction, precise)),
            "measuring every instruction gives instruction {index} another behavior than measuring it alone"
        );
        if set_aside.is_some_and(|set_aside| set_aside.contains(&instruction)) {
            continue;
        }
        let mut refused = usize::MIN;
        for before in generator.generate_states(instruction) {
            let Ok(simulated) = precise.simulate_state(before.clone());
            let Ok(executed) = source.execute_single_instruction(before) else {
                assert!(
                    set_aside.is_some(),
                    "the source did not execute a state of instruction {index} it was measured on"
                );
                refused += 1;
                continue;
            };
            assert!(
                executed.meets_expectations(&simulated),
                "the behavior measured for instruction {index} disagrees with the source"
            );
        }
        if refused > usize::MIN {
            refused_states += refused;
            refusing_instructions += 1;
        }
    }
    eprintln!("{refused_states} states refused, of {refusing_instructions} instructions");
    assert!(
        measurements.len() == Source::Architecture::list_instructions().count(),
        "measuring every instruction gives another number of behaviors than there are instructions"
    );
}

const NO_KNOWN_BEHAVIOR: &str = "it has no known behavior";
const EXECUTION_FAILED: &str = "the source did not execute one of its states";
const NO_BEHAVIOR_AGREES: &str = "no known behavior agrees with the source on one of its states";
const INDISTINGUISHABLE_BEHAVIORS: &str = "no generated state tells two behaviors apart";

/// Returns which instruction, by its place in the list, shows no one known behavior, and why.
fn describe<Architecture: ProcessorArchitecture, Error>(
    error: &MeasureInstructionError<Architecture, Error>,
) -> String {
    let (instruction, why) = match error {
        MeasureInstructionError::NoKnownBehavior { instruction } => {
            (instruction, NO_KNOWN_BEHAVIOR)
        }
        MeasureInstructionError::ExecutionFailed { instruction, .. } => {
            (instruction, EXECUTION_FAILED)
        }
        MeasureInstructionError::NoBehaviorAgrees { instruction, .. } => {
            (instruction, NO_BEHAVIOR_AGREES)
        }
        MeasureInstructionError::IndistinguishableBehaviors { instruction, .. } => {
            (instruction, INDISTINGUISHABLE_BEHAVIORS)
        }
    };
    let index = Architecture::list_instructions().position(|listed| listed == *instruction);
    format!("instruction {index:?} shows no one known behavior: {why}")
}

/// A source that measures an instruction set aside as its first known behavior, and any other on the states it executes.
struct SettingAside<'list, Source: BehaviorSource> {
    source: Source,
    set_aside: &'list [<Source::Architecture as ProcessorArchitecture>::Instruction],
}

impl<Source: BehaviorSource> BehaviorSource for SettingAside<'_, Source> {
    type Architecture = Source::Architecture;
    type Error = Source::Error;

    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut <Self::Architecture as ProcessorArchitecture>::ProcessorState,
        memory: MemorySnapshot<'_, '_, Self::Architecture>,
        mutation_hook: &mut impl MemorySink<Self::Architecture>,
    ) -> Result<(), Self::Error> {
        self.source
            .execute_single_instruction_in_place(processor, memory, mutation_hook)
    }

    fn measure_instruction(
        &mut self,
        instruction: <Self::Architecture as ProcessorArchitecture>::Instruction,
        generator: &impl DistinguishingStateGenerator<Self::Architecture>,
    ) -> Result<
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction,
        MeasureInstructionError<Self::Architecture, Self::Error>,
    >
    where
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction:
            Simulator<Self::Architecture, Error = Infallible>,
    {
        if self.set_aside.contains(&instruction) {
            return Self::Architecture::list_known_behaviors(instruction)
                .next()
                .ok_or(MeasureInstructionError::NoKnownBehavior { instruction });
        }
        match self.source.measure_instruction(instruction, generator) {
            Err(MeasureInstructionError::ExecutionFailed { .. }) => {
                self.measure_on_executed_states(instruction, generator)
            }
            measured => measured,
        }
    }
}

impl<Source: BehaviorSource> SettingAside<'_, Source> {
    /// Measures the instruction as `measure_instruction` does, on the generated states the source executes.
    ///
    /// Where only states the source refuses tell the behaviors left apart, each agrees with all it shows: the first is
    /// taken.
    fn measure_on_executed_states(
        &mut self,
        instruction: <Source::Architecture as ProcessorArchitecture>::Instruction,
        generator: &impl DistinguishingStateGenerator<Source::Architecture>,
    ) -> Result<
        <Source::Architecture as ProcessorArchitecture>::PreciseInstruction,
        MeasureInstructionError<Source::Architecture, Source::Error>,
    >
    where
        <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
            Simulator<Source::Architecture, Error = Infallible>,
    {
        let mut candidates: Vec<_> =
            Source::Architecture::list_known_behaviors(instruction).collect();
        for before in generator.generate_states(instruction) {
            let Ok(observed) = self.source.execute_single_instruction(before.clone()) else {
                continue;
            };
            let behaviors: Vec<_> = candidates
                .iter()
                .map(|&precise| {
                    let Ok(after) = precise.simulate_state(before.clone());
                    (precise, after)
                })
                .collect();
            candidates = behaviors
                .iter()
                .filter(|(_, after)| observed.meets_expectations(after))
                .map(|&(precise, _)| precise)
                .collect();
            if candidates.is_empty() {
                return Err(MeasureInstructionError::NoBehaviorAgrees {
                    instruction,
                    before,
                    behaviors,
                    observed,
                });
            }
        }
        candidates
            .first()
            .copied()
            .ok_or(MeasureInstructionError::NoKnownBehavior { instruction })
    }
}

/// Checks that the states after an error execute as on a source that met no error.
///
/// After each state the source cannot execute, it runs the first generated state of every instruction.
pub fn next_state_after_an_error_executes_as_on_a_fresh_source<Source, Generator>(
    mut source: Source,
    mut unbroken: Source,
    generator: Generator,
    unexecutable: impl IntoIterator<Item = ExecutionState<'static, Source::Architecture>>,
) where
    Source: BehaviorSource,
    <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Source::Architecture, Error = Infallible>,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    for (index, state) in unexecutable.into_iter().enumerate() {
        let Err(_) = source.execute_single_instruction(state) else {
            panic!("the source executed state {index}, which it names as one it cannot");
        };
        for instruction in Source::Architecture::list_instructions() {
            if let Some(before) = generator.generate_states(instruction).next() {
                let after = source.execute_single_instruction(before.clone());
                let expected = unbroken.execute_single_instruction(before.clone());
                assert!(
                    is_same_outcome(instruction, &before, after, expected),
                    "a state after the error of state {index} executes otherwise than on a source that met no error"
                );
            }
        }
    }
}

/// Returns whether two executions of the state both give an error, or states after that are the same or both meet
/// what one known behavior of the instruction expects, which may leave some values undecided.
fn is_same_outcome<Architecture, Error>(
    instruction: Architecture::Instruction,
    before: &ExecutionState<'static, Architecture>,
    first: Result<ExecutionState<'static, Architecture>, Error>,
    second: Result<ExecutionState<'static, Architecture>, Error>,
) -> bool
where
    Architecture: ProcessorArchitecture,
    Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
{
    match (first, second) {
        (Ok(first), Ok(second)) => {
            first == second
                || Architecture::list_known_behaviors(instruction).any(|precise| {
                    let Ok(expected) = precise.simulate_state(before.clone());
                    first.meets_expectations(&expected) && second.meets_expectations(&expected)
                })
        }
        (Err(_), Err(_)) => true,
        (Ok(_), Err(_)) | (Err(_), Ok(_)) => false,
    }
}

/// Runs each suite function above as its own test on a source and a generator made anew for every test.
///
/// The third argument lists states the source cannot execute, none for a source whose error is `Infallible`.
#[macro_export]
macro_rules! tests {
    ($source:expr, $generator:expr, $unexecutable:expr) => {
        #[test]
        fn execute_agrees_with_execute_in_place() {
            $crate::execute_agrees_with_execute_in_place($source, $generator);
        }

        #[test]
        fn execution_is_deterministic() {
            $crate::execution_is_deterministic($source, $generator);
        }

        #[test]
        fn measured_behavior_agrees_with_the_source() {
            $crate::measured_behavior_agrees_with_the_source($source, $generator);
        }

        #[test]
        fn next_state_after_an_error_executes_as_on_a_fresh_source() {
            $crate::next_state_after_an_error_executes_as_on_a_fresh_source(
                $source,
                $source,
                $generator,
                $unexecutable,
            );
        }
    };
}
