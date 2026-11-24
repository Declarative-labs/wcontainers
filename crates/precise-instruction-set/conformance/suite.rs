//! The tests every implementation of wcontainers-precise-instruction-set runs, written against its traits alone.

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};
use wcontainers_processor_architecture_conformance::leave_same_memory;

/// The behavior the set gives each recorded instruction turns every state a chip was recorded from into the
/// state the chip gave after.
///
/// The recordings are triples of an instruction, a state before and the chip's state after; they must cover every
/// listed instruction, so that no behavior goes unchecked.
pub fn agrees_with_recordings<Architecture, Set>(
    set: Set,
    recordings: impl IntoIterator<
        Item = (
            Architecture::Instruction,
            ExecutionState<'static, Architecture>,
            ExecutionState<'static, Architecture>,
        ),
    >,
) where
    Architecture: ProcessorArchitecture + Clone + Eq,
    Set: PreciseInstructionSet<Architecture>,
{
    let mut recorded = Vec::new();
    for (case, (instruction, before, after)) in recordings.into_iter().enumerate() {
        if !recorded.contains(&instruction) {
            recorded.push(instruction);
        }
        let Ok(simulated) = set.to_precise(instruction).simulate(before.clone());
        let start = before.memory_mutations.len();
        assert!(
            simulated.processor == after.processor
                && leave_same_memory(
                    &before.memory_mutations,
                    &simulated.memory_mutations[start..],
                    &after.memory_mutations[start..],
                ),
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

/// Runs each suite function above as its own test on a set and recordings made anew for every test.
#[macro_export]
macro_rules! tests {
    ($set:expr, $recordings:expr) => {
        #[test]
        fn agrees_with_recordings() {
            $crate::agrees_with_recordings($set, $recordings);
        }
    };
}
