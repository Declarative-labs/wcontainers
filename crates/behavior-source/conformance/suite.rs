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
    check_measurements(source, &generator, &[]);
}

/// The behavior measured for each instruction agrees with the source on every generated state, but for the
/// instructions set aside, for which the source shows no one known behavior.
///
/// Each instruction set aside measures as its first known behavior, so measuring every instruction at once still gives
/// each the behavior measuring it alone gives, in the order they are listed, and one for every instruction.
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
    check_measurements(SettingAside { source, set_aside }, &generator, set_aside);
}

/// Checks the measurements of the source as the two tests above say, the states of the instructions set aside left out.
fn check_measurements<Source, Generator>(
    mut source: Source,
    generator: &Generator,
    set_aside: &[<Source::Architecture as ProcessorArchitecture>::Instruction],
) where
    Source: BehaviorSource,
    <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Source::Architecture, Error = Infallible>,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    let Ok(measurements) = source.measure_instructions(generator) else {
        panic!("an instruction shows no one known behavior");
    };
    for (index, instruction) in Source::Architecture::list_instructions().enumerate() {
        let Ok(precise) = source.measure_instruction(instruction, generator) else {
            panic!("instruction {index} shows no one known behavior");
        };
        assert!(
            measurements.get(index) == Some(&(instruction, precise)),
            "measuring every instruction gives instruction {index} another behavior than measuring it alone"
        );
        if set_aside.contains(&instruction) {
            continue;
        }
        for before in generator.generate_states(instruction) {
            let Ok(simulated) = precise.simulate_state(before.clone());
            let Ok(executed) = source.execute_single_instruction(before) else {
                panic!(
                    "the source did not execute a state of instruction {index} it was measured on"
                );
            };
            assert!(
                executed.meets_expectations(&simulated),
                "the behavior measured for instruction {index} disagrees with the source"
            );
        }
    }
    assert!(
        measurements.len() == Source::Architecture::list_instructions().count(),
        "measuring every instruction gives another number of behaviors than there are instructions"
    );
}

/// A source that measures each instruction set aside as its first known behavior without running it.
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
        if !self.set_aside.contains(&instruction) {
            return self.source.measure_instruction(instruction, generator);
        }
        Self::Architecture::list_known_behaviors(instruction)
            .next()
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
