//! The tests every implementation of wcontainers-distinguishing-state-generator runs, written against its traits alone.

use std::convert::Infallible;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};

/// For every two distinct known behaviors of one instruction, some generated state gives different states after.
pub fn distinguishes_every_two_known_behaviors<Architecture, Generator>(generator: Generator)
where
    Architecture: ProcessorArchitecture + Clone,
    Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
    Generator: DistinguishingStateGenerator<Architecture>,
{
    for instruction in Architecture::list_instructions() {
        let behaviors: Vec<Architecture::PreciseInstruction> =
            Architecture::list_known_behaviors(instruction).collect();
        let states: Vec<ExecutionState<'static, Architecture>> =
            generator.generate_states(instruction).collect();
        for (first_index, first) in behaviors.iter().enumerate() {
            for (second_index, second) in behaviors.iter().enumerate().skip(first_index + 1) {
                let distinguished = first == second
                    || states.iter().any(|before| {
                        let Ok(after_first) = first.simulate(before.clone());
                        let Ok(after_second) = second.simulate(before.clone());
                        after_first != after_second
                    });
                assert!(
                    distinguished,
                    "no generated state tells behaviors {first_index} and {second_index} apart"
                );
            }
        }
    }
}

/// Runs each suite function above as its own test on a generator made anew for every test.
///
/// The architecture is named because a generator may serve several, so the generator does not determine it.
#[macro_export]
macro_rules! tests {
    ($architecture:ty, $generator:expr) => {
        #[test]
        fn distinguishes_every_two_known_behaviors() {
            $crate::distinguishes_every_two_known_behaviors::<$architecture, _>($generator);
        }
    };
}
