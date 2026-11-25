//! The tests every implementation of wcontainers-precise-instruction-set runs, written against its traits alone.

use std::convert::Infallible;

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};

/// The set measured from the source turns every state a chip was recorded from into the state the chip gave after.
///
/// The recordings are triples of an instruction, a state before and the chip's state after; they must cover every
/// listed instruction, so that no behavior goes unchecked.
pub fn agrees_with_recordings<Architecture, Set>(
    source: Set::Source,
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
    let Ok(set) = Set::measure(source) else {
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

/// Runs each suite function above as its own test on a source and recordings made anew for every test.
///
/// The set is named because measuring is how a set is made, so only the source is a value.
#[macro_export]
macro_rules! tests {
    ($set:ty, $source:expr, $recordings:expr) => {
        #[test]
        fn agrees_with_recordings() {
            $crate::agrees_with_recordings::<_, $set>($source, $recordings);
        }
    };
}
