//! What the tests give the simulation: the first known behavior of each instruction, and seeded random bytes.

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_arm64::{Arm64, User};
use wcontainers_random_source::StandaloneRandomSource;
use wcontainers_random_source_seeded::{SeededRandomSource, SeededRandomSourceConfig};

/// The seed of the values the simulation chooses where a behavior leaves them undecided.
pub const SEED: u64 = 0;

/// The behaviors the tests' simulation runs: the first the architecture lists for each instruction.
pub type FirstBehaviors = MeasuredInstructionSet<Arm64<User>>;

/// Returns the first known behavior of every instruction that has one.
pub fn list_first_behaviors() -> FirstBehaviors {
    let first: Vec<_> = Arm64::<User>::list_instructions()
        .filter_map(|instruction| {
            Arm64::<User>::list_known_behaviors(instruction)
                .next()
                .map(|precise| (instruction, precise))
        })
        .collect();
    FirstBehaviors::from_measurements(&first)
}

/// Returns the tests' random source, which gives the same words in the same order for the same seed.
pub fn seed_random(seed: u64) -> SeededRandomSource {
    SeededRandomSource::new(SeededRandomSourceConfig { seed })
}
