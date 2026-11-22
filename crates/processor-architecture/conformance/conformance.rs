//! The tests every implementation of wcontainers-processor-architecture runs, written against its traits alone.
//!
//! Each function takes the cases to simulate: a simulator and a state before. `Clone + Eq` on the architecture is
//! what the derives of `ExecutionState` require of it.

use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};

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

/// Whether two lists of changes, each applied after `memory`, leave the same bytes and permissions.
///
/// The lists themselves may differ: a chip reports a byte rewritten with the value it held as no change.
///
/// TODO: temporary placeholder until the interface says when two execution states hold the same memory; the
/// executor and precise instruction set suites and the measured set use it.
pub fn leave_same_memory<Architecture: ProcessorArchitecture>(
    memory: &[MemoryDelta<'_, Architecture>],
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
) -> bool {
    let all = [memory, first, second];
    first.iter().chain(second).all(|mutation| match mutation {
        MemoryDelta::ValueAssignment { start, bytes } => (0..bytes.len() as u64).all(|offset| {
            let address = start.wrapping_add(offset);
            read_byte([memory, first], address) == read_byte([memory, second], address)
        }),
        // A permission is the same across each run of addresses that no assignment starts or ends inside.
        MemoryDelta::PermissionAssignment { addresses, .. } => {
            let boundaries = all
                .into_iter()
                .flatten()
                .filter_map(|mutation| match mutation {
                    MemoryDelta::PermissionAssignment { addresses, .. } => {
                        Some([addresses.start, addresses.end])
                    }
                    MemoryDelta::ValueAssignment { .. } => None,
                });
            let inside = boundaries
                .flatten()
                .filter(|boundary| addresses.contains(boundary));
            [addresses.start].into_iter().chain(inside).all(|address| {
                read_permission([memory, first], address)
                    == read_permission([memory, second], address)
            })
        }
    })
}

/// Reads a byte of the memory the second list of changes leaves after the first.
fn read_byte<Architecture: ProcessorArchitecture>(
    lists: [&[MemoryDelta<'_, Architecture>]; 2],
    address: u64,
) -> u8 {
    lists
        .into_iter()
        .rev()
        .flat_map(|list| list.iter().rev())
        .find_map(|mutation| match mutation {
            MemoryDelta::ValueAssignment { start, bytes } => {
                usize::try_from(address.wrapping_sub(*start))
                    .ok()
                    .and_then(|offset| bytes.get(offset))
                    .copied()
            }
            MemoryDelta::PermissionAssignment { .. } => None,
        })
        .unwrap_or_default()
}

/// Reads the permission of an address, or none when no change gave it one.
fn read_permission<Architecture: ProcessorArchitecture>(
    lists: [&[MemoryDelta<'_, Architecture>]; 2],
    address: u64,
) -> Option<Architecture::PagePermission> {
    lists
        .into_iter()
        .rev()
        .flat_map(|list| list.iter().rev())
        .find_map(|mutation| match mutation {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => addresses.contains(&address).then_some(*permission),
            MemoryDelta::ValueAssignment { .. } => None,
        })
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
