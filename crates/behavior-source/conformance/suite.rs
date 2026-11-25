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
            source.execute_single_instruction_in_place(
                &mut processor,
                &before.memory_mutations,
                &mut mutated,
            );
            let executed = source.execute_single_instruction(before.clone());
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
            assert!(first == second, "one state gave two states after");
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
            let executed = source.execute_single_instruction(before);
            assert!(
                simulated == executed,
                "the behavior measured for instruction {index} disagrees with the source"
            );
        }
    }
}

/// Runs each suite function above as its own test on a source and a generator made anew for every test.
#[macro_export]
macro_rules! tests {
    ($source:expr, $generator:expr) => {
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
    };
}
