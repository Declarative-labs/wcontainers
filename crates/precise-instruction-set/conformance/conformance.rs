//! The tests every implementation of wcontainers-precise-instruction-set runs, written against its traits alone.

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{ProcessorArchitecture, Simulator};
use wcontainers_processor_architecture_recording::{Recording, leave_same_memory};

/// The behavior the set gives each recorded instruction turns every recorded state before into its after.
///
/// The recordings must cover every listed instruction, so that no behavior goes unchecked.
pub fn agrees_with_recordings<Architecture, Set>(
    set: Set,
    recordings: impl IntoIterator<Item = Recording<Architecture>>,
) where
    Architecture: ProcessorArchitecture + Clone + Eq,
    Set: PreciseInstructionSet<Architecture>,
{
    let recordings: Vec<Recording<Architecture>> = recordings.into_iter().collect();
    for (index, instruction) in Architecture::list_instructions().enumerate() {
        let recording = recordings
            .iter()
            .find(|recording| recording.instruction() == instruction);
        let Some(recording) = recording else {
            panic!("listed instruction {index} has no recording");
        };
        let precise = set.to_precise(instruction);
        for (case, (before, after)) in recording.cases().iter().enumerate() {
            let Ok(simulated) = precise.simulate(before.clone());
            let start = before.memory_mutations.len();
            assert!(
                simulated.processor == after.processor
                    && leave_same_memory(
                        &before.memory_mutations,
                        &simulated.memory_mutations[start..],
                        &after.memory_mutations[start..],
                    ),
                "listed instruction {index} disagrees with recorded case {case}"
            );
        }
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
