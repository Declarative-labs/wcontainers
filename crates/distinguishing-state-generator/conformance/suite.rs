//! The tests every implementation of wcontainers-distinguishing-state-generator runs, written against its traits alone.

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};

/// For every two distinct candidates of one instruction, some generated state gives different states after.
pub fn distinguishes_every_two_candidates<Architecture, Generator>(
    generator: Generator,
    candidates: impl IntoIterator<
        Item = (
            Architecture::Instruction,
            Vec<Architecture::PreciseInstruction>,
        ),
    >,
) where
    Architecture: ProcessorArchitecture + Clone + Eq,
    Generator: DistinguishingStateGenerator<Architecture>,
{
    for (instruction, precise) in candidates {
        let states: Vec<ExecutionState<'static, Architecture>> =
            generator.generate_states(instruction).collect();
        for (first_index, first) in precise.iter().enumerate() {
            for (second_index, second) in precise.iter().enumerate().skip(first_index + 1) {
                let distinguished = first == second
                    || states.iter().any(|before| {
                        let Ok(after_first) = first.simulate(before.clone());
                        let Ok(after_second) = second.simulate(before.clone());
                        after_first != after_second
                    });
                assert!(
                    distinguished,
                    "no generated state tells candidates {first_index} and {second_index} apart"
                );
            }
        }
    }
}

/// Runs each suite function above as its own test on a generator and candidates made anew for every test.
///
/// The architecture is named because a generator may serve several, so the generator does not determine it.
#[macro_export]
macro_rules! tests {
    ($architecture:ty, $generator:expr, $candidates:expr) => {
        #[test]
        fn distinguishes_every_two_candidates() {
            $crate::distinguishes_every_two_candidates::<$architecture, _>($generator, $candidates);
        }
    };
}
