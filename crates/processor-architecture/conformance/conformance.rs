//! The tests every implementation of wcontainers-processor-architecture runs, written against its traits alone.
//!
//! Each function takes the cases to simulate: a simulator and a state before. `Clone + Eq` on the architecture is
//! what the derives of `ExecutionState` require of it.

use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};

/// The provided `simulate` gives what `simulate_in_place` does: its processor, and its memory mutations appended.
pub fn simulate_agrees_with_simulate_in_place<Architecture, S>(
    cases: impl IntoIterator<Item = (S, ExecutionState<'static, Architecture>)>,
) where
    Architecture: ProcessorArchitecture + Clone + Eq,
    S: Simulator<Architecture>,
{
    for (simulator, before) in cases {
        let mut processor = before.processor.clone();
        let mut mutated = Vec::new();
        let in_place =
            simulator.simulate_in_place(&mut processor, &before.memory_mutations, |mutation| {
                mutated.push(mutation.into_owned())
            });
        let simulated = simulator.simulate(before.clone());
        match (in_place, simulated) {
            (Ok(()), Ok(after)) => {
                let mut memory_mutations = before.memory_mutations;
                memory_mutations.append(&mut mutated);
                assert!(
                    after
                        == ExecutionState {
                            processor,
                            memory_mutations
                        }
                );
            }
            (Err(_), Err(_)) => {}
            (Ok(()), Err(_)) | (Err(_), Ok(_)) => {
                panic!("only one of simulate and simulate_in_place failed")
            }
        }
    }
}

/// Simulating one state twice gives the same result.
pub fn simulation_is_deterministic<Architecture, S>(
    cases: impl IntoIterator<Item = (S, ExecutionState<'static, Architecture>)>,
) where
    Architecture: ProcessorArchitecture + Clone + Eq,
    S: Simulator<Architecture>,
    S::Error: PartialEq,
{
    for (simulator, before) in cases {
        assert!(simulator.simulate(before.clone()) == simulator.simulate(before));
    }
}

/// A simulation that fails leaves the processor as it was.
pub fn failed_simulation_leaves_the_processor_unchanged<Architecture, S>(
    cases: impl IntoIterator<Item = (S, ExecutionState<'static, Architecture>)>,
) where
    Architecture: ProcessorArchitecture + Clone + Eq,
    S: Simulator<Architecture>,
{
    for (simulator, before) in cases {
        let mut processor = before.processor.clone();
        if simulator
            .simulate_in_place(&mut processor, &before.memory_mutations, |_| {})
            .is_err()
        {
            assert!(processor == before.processor);
        }
    }
}

/// Runs each suite function above as its own test on the cases made anew for every test.
#[macro_export]
macro_rules! tests {
    ($cases:expr) => {
        #[test]
        fn simulate_agrees_with_simulate_in_place() {
            $crate::simulate_agrees_with_simulate_in_place($cases);
        }

        #[test]
        fn simulation_is_deterministic() {
            $crate::simulation_is_deterministic($cases);
        }

        #[test]
        fn failed_simulation_leaves_the_processor_unchanged() {
            $crate::failed_simulation_leaves_the_processor_unchanged($cases);
        }
    };
}
