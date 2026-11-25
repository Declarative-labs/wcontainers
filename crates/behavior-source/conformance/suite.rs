//! The tests every implementation of wcontainers-behavior-source runs, written against its traits alone.

use std::convert::Infallible;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};

/// The provided `execute_single_instruction` gives what `execute_single_instruction_in_place` does: its processor,
/// and its memory mutations appended.
pub fn execute_agrees_with_execute_in_place<Source, Generator>(
    mut source: Source,
    generator: Generator,
) where
    Source: BehaviorSource,
    Source::Architecture: Clone,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    for instruction in Source::Architecture::list_instructions() {
        for before in generator.generate_states(instruction) {
            let mut processor = before.processor.clone();
            let mut mutated = Vec::new();
            let in_place = source.execute_single_instruction_in_place(
                &mut processor,
                &before.memory_mutations,
                &mut mutated,
            );
            let executed = source.execute_single_instruction(before.clone());
            match (in_place, executed) {
                (Ok(()), Ok(executed)) => {
                    let mut memory_mutations = before.memory_mutations;
                    memory_mutations.append(&mut mutated);
                    assert!(
                        executed
                            == ExecutionState {
                                processor,
                                memory_mutations
                            },
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

/// Executing one state twice gives the same state after.
pub fn execution_is_deterministic<Source, Generator>(mut source: Source, generator: Generator)
where
    Source: BehaviorSource,
    Source::Architecture: Clone,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    for instruction in Source::Architecture::list_instructions() {
        for before in generator.generate_states(instruction) {
            let first = source.execute_single_instruction(before.clone());
            let second = source.execute_single_instruction(before);
            assert!(
                is_same_result(first, second),
                "one state gave two states after"
            );
        }
    }
}

/// The behavior measured for each instruction agrees with the source on every generated state.
pub fn measured_behavior_agrees_with_the_source<Source, Generator>(
    mut source: Source,
    generator: Generator,
) where
    Source: BehaviorSource,
    Source::Architecture: Clone,
    <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Source::Architecture, Error = Infallible>,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    for (index, instruction) in Source::Architecture::list_instructions().enumerate() {
        let Ok(precise) = source.measure_instruction(instruction, &generator) else {
            panic!("instruction {index} shows no one known behavior");
        };
        for before in generator.generate_states(instruction) {
            let Ok(simulated) = precise.simulate(before.clone());
            let Ok(executed) = source.execute_single_instruction(before) else {
                panic!(
                    "the source did not execute a state of instruction {index} it was measured on"
                );
            };
            assert!(
                simulated == executed,
                "the behavior measured for instruction {index} disagrees with the source"
            );
        }
    }
}

/// After each state the source cannot execute, the first generated state of every instruction executes as on a source
/// that met no error.
pub fn next_state_after_an_error_executes_as_on_a_fresh_source<Source, Generator>(
    mut source: Source,
    mut unbroken: Source,
    generator: Generator,
    unexecutable: impl IntoIterator<Item = ExecutionState<'static, Source::Architecture>>,
) where
    Source: BehaviorSource,
    Source::Architecture: Clone,
    Generator: DistinguishingStateGenerator<Source::Architecture>,
{
    for (index, state) in unexecutable.into_iter().enumerate() {
        let Err(_) = source.execute_single_instruction(state) else {
            panic!("the source executed state {index}, which it names as one it cannot");
        };
        for instruction in Source::Architecture::list_instructions() {
            if let Some(before) = generator.generate_states(instruction).next() {
                let after = source.execute_single_instruction(before.clone());
                let expected = unbroken.execute_single_instruction(before);
                assert!(
                    is_same_result(after, expected),
                    "a state after the error of state {index} executes otherwise than on a source that met no error"
                );
            }
        }
    }
}

/// Returns whether two executions both give the same state after or both give an error.
fn is_same_result<'memory, Architecture: ProcessorArchitecture, Error>(
    first: Result<ExecutionState<'memory, Architecture>, Error>,
    second: Result<ExecutionState<'memory, Architecture>, Error>,
) -> bool {
    match (first, second) {
        (Ok(first), Ok(second)) => first == second,
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
