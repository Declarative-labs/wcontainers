use std::cmp::Reverse;
use std::convert::Infallible;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_executor_simulation::Simulation;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::memory::read_bytes;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, User};

#[path = "testing.rs"]
mod testing;

use testing::{SEED, list_first_behaviors, seed_random};

/// Returns the generated states whose word, read as the encoding fixing most bits, has no known behavior.
fn list_states_without_behavior() -> Vec<ExecutionState<'static, Arm64<User>>> {
    let generator = CriticalValueCut::default();
    Arm64::<User>::list_instructions()
        .filter(|&instruction| has_no_behavior(instruction))
        .flat_map(|instruction| generator.generate_states(instruction).collect::<Vec<_>>())
        .filter(|state| {
            let word = u32::from_le_bytes(read_bytes(
                &state.memory_mutations,
                state.processor.program_counter,
            ));
            Arm64::<User>::list_instructions()
                .filter(|instruction| {
                    let fixed = instruction.to_fixed_bits();
                    word & fixed.mask == fixed.value
                })
                .min_by_key(|instruction| Reverse(instruction.to_fixed_bits().mask.count_ones()))
                .is_some_and(has_no_behavior)
        })
        .collect()
}

fn has_no_behavior(instruction: Instruction<Infallible>) -> bool {
    Arm64::<User>::list_known_behaviors(instruction)
        .next()
        .is_none()
}

wcontainers_executor_conformance::tests!(
    Simulation::new(list_first_behaviors(), seed_random(SEED)),
    list_first_behaviors(),
    CriticalValueCut::default(),
    list_states_without_behavior()
);
