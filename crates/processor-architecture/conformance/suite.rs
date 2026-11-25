//! The tests every implementation of wcontainers-processor-architecture runs, written against its traits alone.
//!
//! Each function of an architecture takes the cases to simulate: a simulator and a state before. `Eq` on the
//! architecture is what the derives of `ExecutionState` require of it. The function of a memory sink takes the sink.

use std::borrow::Cow;

use wcontainers_processor_architecture::{
    ExecutionState, ExecutionStateExpectation, MemoryDelta, MemorySink, ProcessorArchitecture,
    Simulator,
};

/// Lists each instruction once, and each behavior known for an instruction once.
pub fn lists_each_instruction_and_known_behavior_once<Architecture: ProcessorArchitecture>() {
    let instructions: Vec<Architecture::Instruction> = Architecture::list_instructions().collect();
    for (index, instruction) in instructions.iter().enumerate() {
        assert!(
            !instructions[..index].contains(instruction),
            "instruction {index} is listed twice"
        );
        let behaviors: Vec<Architecture::PreciseInstruction> =
            Architecture::list_known_behaviors(*instruction).collect();
        for (behavior_index, behavior) in behaviors.iter().enumerate() {
            assert!(
                !behaviors[..behavior_index].contains(behavior),
                "behavior {behavior_index} of instruction {index} is listed twice"
            );
        }
    }
}

/// The provided `simulate_state` gives what `simulate` does: its expectation, and its memory mutations appended.
pub fn simulate_state_agrees_with_simulate<Architecture, S>(
    cases: impl IntoIterator<Item = (S, ExecutionState<'static, Architecture>)>,
) where
    Architecture: ProcessorArchitecture + Eq,
    S: Simulator<Architecture>,
{
    for (simulator, before) in cases {
        let mut mutated = Vec::new();
        let simulated = simulator.simulate(&before.processor, before.memory(), &mut mutated);
        let simulated_state = simulator.simulate_state(before.clone());
        match (simulated, simulated_state) {
            (Ok(processor), Ok(after)) => {
                let mut memory_mutations = before.memory_mutations;
                memory_mutations.append(&mut mutated);
                assert!(
                    after
                        == ExecutionStateExpectation {
                            processor,
                            memory_mutations
                        }
                );
            }
            (Err(_), Err(_)) => {}
            (Ok(_), Err(_)) | (Err(_), Ok(_)) => {
                panic!("only one of simulate_state and simulate failed")
            }
        }
    }
}

/// Simulating one state twice gives the same result.
pub fn simulation_is_deterministic<Architecture, S>(
    cases: impl IntoIterator<Item = (S, ExecutionState<'static, Architecture>)>,
) where
    Architecture: ProcessorArchitecture + Eq,
    S: Simulator<Architecture>,
    S::Error: PartialEq,
{
    for (simulator, before) in cases {
        assert!(simulator.simulate_state(before.clone()) == simulator.simulate_state(before));
    }
}

/// Checks that the sink takes every kind of mutation.
///
/// Value assignments empty, borrowed, and owned running past the last address; permission assignments empty and over
/// every address a program can use.
pub fn accepts_every_kind_of_mutation<Architecture, Sink>(mut sink: Sink)
where
    Architecture: ProcessorArchitecture,
    Sink: MemorySink<Architecture>,
{
    let bytes = [u8::MAX; size_of::<u64>()];
    let mutations = [
        MemoryDelta::ValueAssignment {
            start: u64::MIN,
            bytes: Cow::Borrowed(&[]),
        },
        MemoryDelta::ValueAssignment {
            start: u64::MIN,
            bytes: Cow::Borrowed(&bytes),
        },
        MemoryDelta::ValueAssignment {
            start: u64::MAX,
            bytes: Cow::Owned(bytes.to_vec()),
        },
        MemoryDelta::PermissionAssignment {
            addresses: u64::MIN..u64::MIN,
            permission: Architecture::DEFAULT_PERMISSION,
        },
        MemoryDelta::PermissionAssignment {
            addresses: Architecture::ADDRESSABLE,
            permission: Architecture::DEFAULT_PERMISSION,
        },
    ];
    for mutation in mutations {
        sink.on_memory_mutated(mutation);
    }
}

/// Runs each suite function above as its own test on an architecture's cases or on a memory sink, each made anew.
#[macro_export]
macro_rules! tests {
    ($architecture:ty, $cases:expr) => {
        #[test]
        fn lists_each_instruction_and_known_behavior_once() {
            $crate::lists_each_instruction_and_known_behavior_once::<$architecture>();
        }

        #[test]
        fn simulate_state_agrees_with_simulate() {
            $crate::simulate_state_agrees_with_simulate($cases);
        }

        #[test]
        fn simulation_is_deterministic() {
            $crate::simulation_is_deterministic($cases);
        }
    };
    ($sink:expr) => {
        #[test]
        fn accepts_every_kind_of_mutation() {
            $crate::accepts_every_kind_of_mutation($sink);
        }
    };
}
