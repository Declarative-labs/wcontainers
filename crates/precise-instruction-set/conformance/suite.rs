//! The tests every implementation of wcontainers-precise-instruction-set runs, written against its traits alone.

use std::convert::Infallible;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};

/// The set measured from the source with the generator turns every recorded state into the state the chip gave after.
///
/// The recordings are triples of an instruction, a state before and the chip's state after; they must cover every
/// listed instruction, so that no behavior goes unchecked.
pub fn agrees_with_recordings<Architecture, Set>(
    source: Set::Source,
    generator: &impl DistinguishingStateGenerator<Architecture>,
    recordings: impl IntoIterator<
        Item = (
            Architecture::Instruction,
            ExecutionState<'static, Architecture>,
            ExecutionState<'static, Architecture>,
        ),
    >,
) where
    Architecture: ProcessorArchitecture + Clone,
    Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
    Set: PreciseInstructionSet<Architecture>,
{
    let Ok(set) = Set::measure(source, generator) else {
        panic!("the source shows a behavior that is not one known behavior");
    };
    let mut recorded = Vec::new();
    for (case, (instruction, before, after)) in recordings.into_iter().enumerate() {
        if !recorded.contains(&instruction) {
            recorded.push(instruction);
        }
        let Ok(simulated) = set.to_precise(instruction).simulate(before);
        assert!(
            simulated == after,
            "recorded case {case} disagrees with the set"
        );
    }
    for (index, instruction) in Architecture::list_instructions().enumerate() {
        assert!(
            recorded.contains(&instruction),
            "listed instruction {index} has no recording"
        );
    }
}

/// Checks that two generators that each tell the behaviors apart measure one source as the same set.
pub fn measures_the_same_set_with_either_generator<Architecture, Set>(
    first_source: Set::Source,
    second_source: Set::Source,
    first_generator: &impl DistinguishingStateGenerator<Architecture>,
    second_generator: &impl DistinguishingStateGenerator<Architecture>,
) where
    Architecture: ProcessorArchitecture,
    Set: PreciseInstructionSet<Architecture>,
{
    let (Ok(first), Ok(second)) = (
        Set::measure(first_source, first_generator),
        Set::measure(second_source, second_generator),
    ) else {
        panic!("a generator does not tell the source's behaviors apart");
    };
    for (index, instruction) in Architecture::list_instructions().enumerate() {
        assert!(
            first.to_precise(instruction) == second.to_precise(instruction),
            "the two generators measure instruction {index} as two behaviors"
        );
    }
}

/// Runs each suite function above as its own test on a source, two generators and recordings, made anew each test.
///
/// The set is named because measuring is how a set is made, so only the source and the generators are values.
#[macro_export]
macro_rules! tests {
    ($set:ty, $source:expr, $generator:expr, $other_generator:expr, $recordings:expr) => {
        #[test]
        fn agrees_with_recordings() {
            $crate::agrees_with_recordings::<_, $set>($source, &$generator, $recordings);
        }

        #[test]
        fn measures_the_same_set_with_either_generator() {
            $crate::measures_the_same_set_with_either_generator::<_, $set>(
                $source,
                $source,
                &$generator,
                &$other_generator,
            );
        }
    };
}
